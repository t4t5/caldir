use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result, bail};
use caldir_core::Event;
use caldir_core::provider::ProviderStorage;
use caldir_core::rpc::ListEvents;
use chrono::{DateTime, Utc};
use futures::stream::{self, StreamExt, TryStreamExt};

use crate::app_config::AppConfigStore;
use crate::constants::PROVIDER_NAME;
use crate::graph_api::client::GraphClient;
use crate::graph_api::types::{CalendarViewRow, GraphEvent, SeriesMasterIndexRow};
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

    let (window, indexed_master_ids) = tokio::try_join!(
        discover_window_events(&graph, &config.outlook_calendar_id, &from, &to),
        discover_series_master_ids(&graph, &config.outlook_calendar_id),
    )?;
    let event_ids = merge_event_ids(&window.event_ids, &indexed_master_ids);
    let expected_master_ids: HashSet<&str> = indexed_master_ids
        .iter()
        .map(String::as_str)
        .chain(window.master_ids.iter().map(String::as_str))
        .collect();

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
    let mut master_uids = HashMap::new();

    for graph_event in graph_events {
        let outlook_id = graph_event.id.clone();
        let master_uid = graph_event.i_cal_uid.clone();
        let is_master =
            graph_event.event_type == "seriesMaster" || graph_event.recurrence.is_some();

        if expected_master_ids.contains(outlook_id.as_str())
            && graph_event.event_type != "seriesMaster"
        {
            bail!("Expected event {outlook_id} to be a series master");
        }

        match from_outlook(graph_event, &config.outlook_account) {
            Ok(event) => {
                if is_master {
                    master_uids.insert(outlook_id, master_uid);
                }
                all_events.push(event);
            }
            Err(error) if expected_master_ids.contains(outlook_id.as_str()) => {
                return Err(error).with_context(|| {
                    format!("Failed to convert expected series master {outlook_id}")
                });
            }
            Err(_) => continue,
        }
    }

    let window_masters: Vec<(String, String)> = window
        .master_ids
        .iter()
        .map(|master_id| {
            let master_uid = master_uids.get(master_id).with_context(|| {
                format!("Window series master {master_id} was not fetched successfully")
            })?;
            Ok((master_id.clone(), master_uid.clone()))
        })
        .collect::<Result<_>>()?;

    let exception_sets: Vec<Vec<GraphEvent>> =
        stream::iter(window_masters.into_iter().map(|(master_id, master_uid)| {
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

const EVENT_SELECT: &str = "id,iCalUId,subject,body,start,end,originalStartTimeZone,originalEndTimeZone,location,isAllDay,isCancelled,recurrence,attendees,organizer,reminderMinutesBeforeStart,isReminderOn,showAs,sensitivity,lastModifiedDateTime,onlineMeeting,originalStart,responseStatus,type";

#[derive(Debug, PartialEq, Eq)]
struct WindowDiscovery {
    event_ids: Vec<String>,
    master_ids: HashSet<String>,
}

async fn discover_window_events(
    graph: &GraphClient,
    calendar_id: &str,
    from: &str,
    to: &str,
) -> Result<WindowDiscovery> {
    let path = format!(
        "/me/calendars/{calendar_id}/calendarView?startDateTime={from}&endDateTime={to}&$select=id,type,seriesMasterId&$top=999"
    );
    let rows = graph
        .get_paged(&path)
        .await
        .context("Failed to discover events in calendar window")?;
    collect_window_events(rows)
}

fn collect_window_events(rows: Vec<CalendarViewRow>) -> Result<WindowDiscovery> {
    let mut ids = Vec::new();
    let mut seen = HashSet::new();
    let mut master_ids = HashSet::new();

    for row in rows {
        let id = match row.event_type.as_str() {
            "singleInstance" => row.id,
            "seriesMaster" => {
                master_ids.insert(row.id.clone());
                row.id
            }
            "occurrence" | "exception" => {
                let master_id = row.series_master_id.with_context(|| {
                    format!(
                        "Calendar view {} {} is missing seriesMasterId",
                        row.event_type, row.id
                    )
                })?;
                master_ids.insert(master_id.clone());
                master_id
            }
            event_type => bail!("Unknown calendar view event type `{event_type}`"),
        };

        if seen.insert(id.clone()) {
            ids.push(id);
        }
    }

    Ok(WindowDiscovery {
        event_ids: ids,
        master_ids,
    })
}

async fn discover_series_master_ids(graph: &GraphClient, calendar_id: &str) -> Result<Vec<String>> {
    let path = format!(
        "/me/calendars/{calendar_id}/events?$select=id,type&$filter=type%20eq%20%27seriesMaster%27&$top=999"
    );
    let rows = graph
        .get_paged(&path)
        .await
        .context("Failed to discover Outlook series masters")?;
    collect_series_master_ids(rows)
}

fn collect_series_master_ids(rows: Vec<SeriesMasterIndexRow>) -> Result<Vec<String>> {
    let mut ids = Vec::new();
    let mut seen = HashSet::new();

    for row in rows {
        match row.event_type.as_str() {
            "seriesMaster" if seen.insert(row.id.clone()) => ids.push(row.id),
            "seriesMaster" | "singleInstance" | "occurrence" | "exception" => {}
            event_type => bail!("Unknown event index type `{event_type}`"),
        }
    }

    Ok(ids)
}

fn merge_event_ids(window_ids: &[String], master_ids: &[String]) -> Vec<String> {
    let mut ids = Vec::with_capacity(window_ids.len() + master_ids.len());
    let mut seen = HashSet::new();

    for id in window_ids.iter().chain(master_ids) {
        if seen.insert(id.as_str()) {
            ids.push(id.clone());
        }
    }

    ids
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

    fn index_row(id: &str, event_type: &str) -> SeriesMasterIndexRow {
        SeriesMasterIndexRow {
            id: id.to_string(),
            event_type: event_type.to_string(),
        }
    }

    #[test]
    fn master_index_schedules_master_when_window_is_empty() {
        let window = collect_window_events(Vec::new()).unwrap();
        let masters = collect_series_master_ids(vec![index_row("master", "seriesMaster")]).unwrap();

        assert_eq!(merge_event_ids(&window.event_ids, &masters), vec!["master"]);
    }

    #[test]
    fn master_referenced_by_multiple_occurrences_is_fetched_once() {
        let rows = vec![
            row("occurrence-1", "occurrence", Some("recurring")),
            row("occurrence-2", "occurrence", Some("recurring")),
            row("exception", "exception", Some("recurring")),
        ];

        let window = collect_window_events(rows).unwrap();

        assert_eq!(window.event_ids, vec!["recurring"]);
    }

    #[test]
    fn master_present_in_both_sources_is_fetched_once() {
        let window = collect_window_events(vec![row("master", "seriesMaster", None)]).unwrap();
        let masters = collect_series_master_ids(vec![index_row("master", "seriesMaster")]).unwrap();

        assert_eq!(merge_event_ids(&window.event_ids, &masters), vec!["master"]);
    }

    #[test]
    fn standalone_index_rows_are_not_scheduled() {
        let window = collect_window_events(Vec::new()).unwrap();
        let masters =
            collect_series_master_ids(vec![index_row("single", "singleInstance")]).unwrap();

        assert!(merge_event_ids(&window.event_ids, &masters).is_empty());
    }

    #[test]
    fn only_window_masters_are_selected_for_exception_fetching() {
        let window = collect_window_events(vec![
            row("single", "singleInstance", None),
            row("occurrence", "occurrence", Some("window-master")),
        ])
        .unwrap();
        let masters = collect_series_master_ids(vec![
            index_row("window-master", "seriesMaster"),
            index_row("historical-master", "seriesMaster"),
        ])
        .unwrap();

        assert_eq!(
            window.master_ids,
            HashSet::from(["window-master".to_string()])
        );
        assert_eq!(
            merge_event_ids(&window.event_ids, &masters),
            vec!["single", "window-master", "historical-master"]
        );
    }

    #[test]
    fn rejects_recurring_instance_without_master_id() {
        for event_type in ["occurrence", "exception"] {
            let error = collect_window_events(vec![row("instance", event_type, None)]).unwrap_err();

            assert!(error.to_string().contains("missing seriesMasterId"));
        }
    }

    #[test]
    fn rejects_unknown_event_type() {
        let error = collect_window_events(vec![row("event", "futureType", None)]).unwrap_err();

        assert_eq!(
            error.to_string(),
            "Unknown calendar view event type `futureType`"
        );
    }

    #[test]
    fn rejects_unknown_master_index_type() {
        let error = collect_series_master_ids(vec![index_row("event", "futureType")]).unwrap_err();

        assert_eq!(error.to_string(), "Unknown event index type `futureType`");
    }

    #[test]
    fn event_projection_includes_reminder_state() {
        assert!(EVENT_SELECT.split(',').any(|field| field == "isReminderOn"));
    }
}
