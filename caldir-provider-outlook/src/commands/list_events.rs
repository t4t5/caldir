use std::collections::HashMap;

use anyhow::{Context, Result};
use caldir_core::event::Event;
use caldir_core::remote::protocol::ListEvents;
use chrono::{DateTime, Utc};

use crate::graph_client::GraphClient;
use crate::graph_types::{GraphEvent, GraphResponse};
use crate::outlook_event::from_outlook::from_outlook;
use crate::remote_config::OutlookRemoteConfig;
use crate::session::Session;

pub async fn handle(cmd: ListEvents) -> Result<Vec<Event>> {
    let config = OutlookRemoteConfig::try_from(&cmd.remote_config)?;
    let session = Session::load_valid(&config.outlook_account).await?;
    let graph = GraphClient::new(session.access_token());

    // `/events` returns single events and recurring series masters (with
    // their RRULE) but NOT exceptions to those series — Microsoft Graph
    // surfaces exceptions only via `/instances` or `/calendarView`. We
    // intentionally avoid `/calendarView`'s blanket expansion (which would
    // turn one weekly meeting into ~50 indistinguishable rows) and instead
    // call `/instances` once per master, picking out only `type=exception`.
    // Date filtering on `/events` is intentionally omitted: OData's
    // `start/dateTime` filter only sees a series master's first occurrence,
    // so a long-running meeting started years ago would be excluded even if
    // it has occurrences in our window.
    let path = format!(
        "/me/calendars/{}/events?$top=100&$select=id,iCalUId,subject,body,start,end,location,isAllDay,isCancelled,recurrence,attendees,organizer,reminderMinutesBeforeStart,showAs,lastModifiedDateTime,onlineMeeting,originalStart,responseStatus,type",
        config.outlook_calendar_id
    );

    let mut all_events = Vec::new();
    // Map outlook event id → master's iCalUId, used to rewrite each
    // exception's iCalUId so masters and their overrides share the same UID
    // locally (RFC 5545 — Graph mints unique iCalUIds per exception, which
    // would otherwise break the (uid, recurrence_id) sync key).
    let mut master_ids: HashMap<String, String> = HashMap::new();
    let mut next_link: Option<String> = None;
    let mut first = true;

    loop {
        let response = if first {
            first = false;
            graph.get(&path).await?
        } else if let Some(ref url) = next_link {
            graph.get_url(url).await?
        } else {
            break;
        };

        let page: GraphResponse<GraphEvent> = response
            .json()
            .await
            .context("Failed to parse events response")?;

        for graph_event in page.value {
            let outlook_id = graph_event.id.clone();
            let master_uid = graph_event.i_cal_uid.clone();
            let is_master =
                graph_event.event_type == "seriesMaster" || graph_event.recurrence.is_some();

            match from_outlook(graph_event, &config.outlook_account) {
                Ok(event) => {
                    if is_master {
                        master_ids.insert(outlook_id, master_uid);
                    }
                    all_events.push(event);
                }
                Err(_) => continue, // Skip malformed events
            }
        }

        next_link = page.next_link;
        if next_link.is_none() {
            break;
        }
    }

    let from = normalize_window_bound(&cmd.from)
        .with_context(|| format!("Invalid `from` timestamp: {}", cmd.from))?;
    let to = normalize_window_bound(&cmd.to)
        .with_context(|| format!("Invalid `to` timestamp: {}", cmd.to))?;

    for (master_id, master_uid) in &master_ids {
        let exceptions = fetch_exceptions(&graph, master_id, master_uid, &from, &to).await?;
        for exception in exceptions {
            if let Ok(event) = from_outlook(exception, &config.outlook_account) {
                all_events.push(event);
            }
        }
    }

    Ok(all_events)
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
        "/me/events/{}/instances?$top=100&startDateTime={}&endDateTime={}&$select=id,iCalUId,subject,body,start,end,location,isAllDay,isCancelled,recurrence,attendees,organizer,reminderMinutesBeforeStart,showAs,lastModifiedDateTime,onlineMeeting,originalStart,responseStatus,type",
        master_id, from, to
    );

    let mut out = Vec::new();
    let mut next_link: Option<String> = None;
    let mut first = true;

    loop {
        let response = if first {
            first = false;
            graph.get(&path).await?
        } else if let Some(ref url) = next_link {
            graph.get_url(url).await?
        } else {
            break;
        };

        let page: GraphResponse<GraphEvent> = response
            .json()
            .await
            .with_context(|| format!("Failed to parse instances of master {}", master_id))?;

        for mut instance in page.value {
            if instance.event_type != "exception" {
                continue;
            }
            instance.i_cal_uid = master_uid.to_string();
            out.push(instance);
        }

        next_link = page.next_link;
        if next_link.is_none() {
            break;
        }
    }

    Ok(out)
}
