use anyhow::Result;
use caldir_core::provider::ProviderStorage;
use caldir_core::rpc::UpdateEvent;
use caldir_core::{Event, ParticipationStatus};

use crate::app_config::AppConfigStore;
use crate::constants::{PROVIDER_EVENT_ID_PROPERTY, PROVIDER_NAME};
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
        .x_property(PROVIDER_EVENT_ID_PROPERTY)
        .ok_or_else(|| {
            anyhow::anyhow!("Cannot update event without {PROVIDER_EVENT_ID_PROPERTY}")
        })?;

    if cmd.event.is_invite_for(account_email) {
        // Non-organizer: use dedicated RSVP endpoints
        respond_to_invite(&graph, outlook_event_id, &cmd.event, account_email).await
    } else {
        // Organizer or own event: full PATCH update
        let body = to_outlook(&cmd.event);
        let path = format!("/me/events/{}", outlook_event_id);
        let response = graph.patch(&path, &body).await?;
        let updated: GraphEvent = response.json().await?;
        from_outlook(updated, account_email)
    }
}

/// Non-organizer: use POST /me/events/{id}/accept|decline|tentativelyAccept.
/// Graph ignores attendee status changes via PATCH — dedicated endpoints are required.
async fn respond_to_invite(
    graph: &GraphClient,
    event_id: &str,
    event: &Event,
    account_email: &str,
) -> Result<Event> {
    let status = event
        .attendee_status(account_email)
        .unwrap_or(ParticipationStatus::NeedsAction);

    let action = match status {
        ParticipationStatus::Accepted => "accept",
        ParticipationStatus::Declined => "decline",
        ParticipationStatus::Tentative => "tentativelyAccept",
        ParticipationStatus::NeedsAction => {
            // Nothing to do — just fetch and return the current state
            let path = format!("/me/events/{}", event_id);
            let response = graph.get(&path).await?;
            let graph_event: GraphEvent = response.json().await?;
            return from_outlook(graph_event, account_email);
        }
    };

    let body = serde_json::json!({ "sendResponse": true });
    let path = format!("/me/events/{}/{}", event_id, action);
    graph.post(&path, &body).await?;

    if status == ParticipationStatus::Declined {
        // Outlook removes declined events from the calendar, so GET would 404.
        // Return the local event as-is — next pull will clean it up.
        return Ok(event.clone());
    }

    // Response endpoints return 202 with no body — fetch the updated event
    let get_path = format!("/me/events/{}", event_id);
    let response = graph.get(&get_path).await?;
    let graph_event: GraphEvent = response.json().await?;
    from_outlook(graph_event, account_email)
}
