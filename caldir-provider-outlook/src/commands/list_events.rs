use std::collections::HashSet;

use anyhow::{Context, Result, bail};
use caldir_core::Event;
use caldir_core::provider::ProviderStorage;
use caldir_core::rpc::ListEvents;
use chrono::{DateTime, Utc};
use futures::stream::{self, StreamExt, TryStreamExt};

use crate::app_config::AppConfigStore;
use crate::constants::PROVIDER_NAME;
use crate::graph_api::client::GraphClient;
use crate::graph_api::types::{CalendarViewRow, GraphEvent};
use crate::outlook_event::from_outlook::from_outlook;
use crate::remote_config::OutlookRemoteConfig;
use crate::session::SessionStore;

pub async fn handle(cmd: ListEvents) -> Result<Vec<Event>> {
    let config = OutlookRemoteConfig::try_from(&cmd.remote)?;

    let storage = ProviderStorage::for_provider(PROVIDER_NAME)?;
    let session_store = SessionStore::new(storage.clone());
    let app_config_store = AppConfigStore::new(storage);

    let session = session_store
        .load_valid(&config.outlook_account, &app_config_store)
        .await?;
    let graph = GraphClient::new(session.access_token());

    let from = normalize_window_bound(&cmd.from)
        .with_context(|| format!("Invalid `from` timestamp: {}", cmd.from))?;
    let to = normalize_window_bound(&cmd.to)
        .with_context(|| format!("Invalid `to` timestamp: {}", cmd.to))?;

    // Discover IDs with `/calendarView`, then fetch only those logical events
    // in full. Recurring exceptions still come from each master's `/instances`.
    let event_ids =
        discover_window_event_ids(&graph, &config.outlook_calendar_id, &from, &to).await?;

    let graph_events: Vec<GraphEvent> = stream::iter(event_ids.into_iter().map(|event_id| {
        let graph = &graph;
        async move { fetch_event(graph, &event_id).await }
    }))
    .buffered(4)
    .try_collect()
    .await?;

    let mut all_events = Vec::new();
    // Outlook event id and iCalUId, used to rewrite each
    // exception's iCalUId so masters and their overrides share the same UID
    // locally (RFC 5545 — Graph mints unique iCalUIds per exception, which
    // would otherwise break the (uid, recurrence_id) sync key).
    let mut master_ids = Vec::new();

    for graph_event in graph_events {
        let outlook_id = graph_event.id.clone();
        let master_uid = graph_event.i_cal_uid.clone();
        let is_master =
            graph_event.event_type == "seriesMaster" || graph_event.recurrence.is_some();

        match from_outlook(graph_event, &config.outlook_account) {
            Ok(event) => {
                if is_master {
                    master_ids.push((outlook_id, master_uid));
                }
                all_events.push(event);
            }
            Err(_) => continue, // Skip malformed events
        }
    }

    let exception_sets: Vec<Vec<GraphEvent>> =
        stream::iter(master_ids.into_iter().map(|(master_id, master_uid)| {
            let graph = &graph;
            let from = &from;
            let to = &to;
            async move { fetch_exceptions(graph, &master_id, &master_uid, from, to).await }
        }))
        .buffered(4)
        .try_collect()
        .await?;

    for exceptions in exception_sets {
        for exception in exceptions {
            if let Ok(event) = from_outlook(exception, &config.outlook_account) {
                all_events.push(event);
            }
        }
    }

    Ok(all_events)
}

const EVENT_SELECT: &str = "id,iCalUId,subject,body,start,end,originalStartTimeZone,originalEndTimeZone,location,isAllDay,isCancelled,recurrence,attendees,organizer,reminderMinutesBeforeStart,showAs,sensitivity,lastModifiedDateTime,onlineMeeting,originalStart,responseStatus,type";

async fn discover_window_event_ids(
    graph: &GraphClient,
    calendar_id: &str,
    from: &str,
    to: &str,
) -> Result<Vec<String>> {
    let path = format!(
        "/me/calendars/{calendar_id}/calendarView?startDateTime={from}&endDateTime={to}&$select=id,type,seriesMasterId&$top=999"
    );
    let rows = graph
        .get_paged(&path)
        .await
        .context("Failed to discover events in calendar window")?;
    collect_window_event_ids(rows)
}

fn collect_window_event_ids(rows: Vec<CalendarViewRow>) -> Result<Vec<String>> {
    let mut ids = Vec::new();
    let mut seen = HashSet::new();

    for row in rows {
        let id = match row.event_type.as_str() {
            "singleInstance" | "seriesMaster" => row.id,
            "occurrence" | "exception" => row.series_master_id.with_context(|| {
                format!(
                    "Calendar view {} {} is missing seriesMasterId",
                    row.event_type, row.id
                )
            })?,
            event_type => bail!("Unknown calendar view event type `{event_type}`"),
        };

        if seen.insert(id.clone()) {
            ids.push(id);
        }
    }

    Ok(ids)
}

async fn fetch_event(graph: &GraphClient, event_id: &str) -> Result<GraphEvent> {
    let path = format!("/me/events/{event_id}?$select={EVENT_SELECT}");
    graph
        .get(&path)
        .await?
        .json()
        .await
        .with_context(|| format!("Failed to parse event {event_id}"))
}

/// Reformat an RFC3339 timestamp as `YYYY-MM-DDTHH:MM:SSZ` for embedding in
/// a Graph URL query string. The raw RFC3339 form contains `+` for the UTC
/// offset, which a URL decoder reads as a space and Graph rejects.
fn normalize_window_bound(s: &str) -> Result<String> {
    let dt: DateTime<Utc> = DateTime::parse_from_rfc3339(s)?.with_timezone(&Utc);
    Ok(dt.format("%Y-%m-%dT%H:%M:%SZ").to_string())
}

/// Pull `type=exception` instances for a single series master in the given
/// window. Exceptions get their `iCalUId` rewritten to the master's so the
/// resulting `Event.uid` matches the master's `Event.uid`.
async fn fetch_exceptions(
    graph: &GraphClient,
    master_id: &str,
    master_uid: &str,
    from: &str,
    to: &str,
) -> Result<Vec<GraphEvent>> {
    let path = format!(
        "/me/events/{master_id}/instances?$top=999&startDateTime={from}&endDateTime={to}&$select={EVENT_SELECT}"
    );

    let instances: Vec<GraphEvent> = graph
        .get_paged(&path)
        .await
        .with_context(|| format!("Failed to fetch instances of master {master_id}"))?;

    Ok(instances
        .into_iter()
        .filter_map(|mut instance| {
            if instance.event_type == "exception" {
                instance.i_cal_uid = master_uid.to_string();
                Some(instance)
            } else {
                None
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str, event_type: &str, series_master_id: Option<&str>) -> CalendarViewRow {
        CalendarViewRow {
            id: id.to_string(),
            event_type: event_type.to_string(),
            series_master_id: series_master_id.map(str::to_string),
        }
    }

    #[test]
    fn collects_and_deduplicates_logical_event_ids() {
        let rows = vec![
            row("single", "singleInstance", None),
            row("master-row", "seriesMaster", None),
            row("occurrence-1", "occurrence", Some("recurring")),
            row("occurrence-2", "occurrence", Some("recurring")),
            row("exception", "exception", Some("recurring")),
            row("single", "singleInstance", None),
        ];

        let ids = collect_window_event_ids(rows).unwrap();

        assert_eq!(ids, vec!["single", "master-row", "recurring"]);
    }

    #[test]
    fn rejects_recurring_instance_without_master_id() {
        let error =
            collect_window_event_ids(vec![row("occurrence", "occurrence", None)]).unwrap_err();

        assert!(error.to_string().contains("missing seriesMasterId"));
    }

    #[test]
    fn rejects_unknown_event_type() {
        let error = collect_window_event_ids(vec![row("event", "futureType", None)]).unwrap_err();

        assert_eq!(
            error.to_string(),
            "Unknown calendar view event type `futureType`"
        );
    }
}
