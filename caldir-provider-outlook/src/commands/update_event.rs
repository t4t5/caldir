use std::collections::HashSet;

use anyhow::{Context, Result};
use caldir_core::provider::ProviderStorage;
use caldir_core::rpc::UpdateEvent;
use caldir_core::{Event, EventTime, ParticipationStatus};
use chrono::{DateTime, Utc};

use crate::app_config::AppConfigStore;
use crate::commands::create_event::find_instance_id;
use crate::commands::list_events::fetch_event;
use crate::constants::{OUTLOOK_EVENT_ID_PROPERTY, PROVIDER_NAME};
use crate::graph_api::client::GraphClient;
use crate::graph_api::types::GraphEvent;
use crate::outlook_event::from_outlook::from_outlook;
use crate::outlook_event::to_outlook::to_outlook;
use crate::remote_config::OutlookRemoteConfig;
use crate::session::SessionStore;

pub async fn handle(cmd: UpdateEvent) -> Result<Event> {
    let config = OutlookRemoteConfig::try_from(&cmd.remote)?;
    let account_email = &config.outlook_account;

    let storage = ProviderStorage::for_provider(PROVIDER_NAME)?;
    let session_store = SessionStore::new(storage.clone());
    let app_config_store = AppConfigStore::new(storage);

    let session = session_store
        .load_valid(account_email, &app_config_store)
        .await?;
    let graph = GraphClient::new(session.access_token());

    let outlook_event_id = cmd
        .event
        .x_property(OUTLOOK_EVENT_ID_PROPERTY)
        .ok_or_else(|| {
            anyhow::anyhow!("Cannot update event without {OUTLOOK_EVENT_ID_PROPERTY}")
        })?;

    if cmd.event.is_invite_for(account_email) {
        // Non-organizer: use dedicated RSVP endpoints
        if respond_to_invite(&graph, outlook_event_id, &cmd.event, account_email).await?
            == ParticipationStatus::Declined
        {
            // Outlook removes declined events from the calendar, so GET would 404.
            // Return the local event as-is — next pull will clean it up.
            return Ok(cmd.event.clone());
        }
    } else {
        // Organizer or own event: full PATCH update
        let body = to_outlook(&cmd.event);
        let path = format!("/me/events/{}", outlook_event_id);
        let response = graph.patch(&path, &body).await?;
        if cmd.event.recurrence.is_none() {
            let updated: GraphEvent = response.json().await?;
            return from_outlook(updated, account_email);
        }
    }

    if cmd.event.recurrence.is_some() {
        return cancel_exdated_occurrences(&graph, outlook_event_id, &cmd.event, account_email)
            .await;
    }

    from_outlook(fetch_event(&graph, outlook_event_id).await?, account_email)
}

/// Delete the Outlook occurrence behind each local EXDATE the series doesn't
/// already cancel, then return the refetched master.
async fn cancel_exdated_occurrences(
    graph: &GraphClient,
    master_id: &str,
    event: &Event,
    account_email: &str,
) -> Result<Event> {
    let remote = from_outlook(fetch_event(graph, master_id).await?, account_email)?;

    let cancelled: HashSet<DateTime<Utc>> = remote
        .recurrence
        .iter()
        .flat_map(|rec| &rec.exdates)
        .map(EventTime::to_utc)
        .collect();

    let pending: Vec<&EventTime> = event
        .recurrence
        .iter()
        .flat_map(|rec| &rec.exdates)
        .filter(|exdate| !cancelled.contains(&exdate.to_utc()))
        .collect();

    if pending.is_empty() {
        return Ok(remote);
    }

    for exdate in pending {
        // No live instance means the EXDATE falls outside the series.
        if let Some(instance_id) = find_instance_id(graph, master_id, exdate).await? {
            graph
                .delete(&format!("/me/events/{instance_id}"))
                .await
                .context("Failed to delete recurring occurrence")?;
        }
    }

    from_outlook(fetch_event(graph, master_id).await?, account_email)
}

/// Non-organizer: use POST /me/events/{id}/accept|decline|tentativelyAccept.
/// Graph ignores attendee status changes via PATCH — dedicated endpoints are required.
async fn respond_to_invite(
    graph: &GraphClient,
    event_id: &str,
    event: &Event,
    account_email: &str,
) -> Result<ParticipationStatus> {
    let status = event
        .attendee_status(account_email)
        .unwrap_or(ParticipationStatus::NeedsAction);

    let action = match status {
        ParticipationStatus::Accepted => "accept",
        ParticipationStatus::Declined => "decline",
        ParticipationStatus::Tentative => "tentativelyAccept",
        ParticipationStatus::NeedsAction => return Ok(status),
    };

    let body = serde_json::json!({ "sendResponse": true });
    let path = format!("/me/events/{}/{}", event_id, action);
    graph.post(&path, &body).await?;

    Ok(status)
}
