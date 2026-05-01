use anyhow::{Context, Result};
use caldir_core::event::Event;
use caldir_core::remote::protocol::ListEvents;

use crate::graph_client::GraphClient;
use crate::graph_types::{GraphEvent, GraphResponse};
use crate::outlook_event::from_outlook::from_outlook;
use crate::remote_config::OutlookRemoteConfig;
use crate::session::Session;

pub async fn handle(cmd: ListEvents) -> Result<Vec<Event>> {
    let config = OutlookRemoteConfig::try_from(&cmd.remote_config)?;
    let session = Session::load_valid(&config.outlook_account).await?;
    let graph = GraphClient::new(session.access_token());

    // `/events` returns single events, recurring series masters (with their
    // RRULE), and exception instances — without expanding occurrences. We
    // intentionally avoid `/calendarView`, which expands every recurring
    // instance into its own event with a unique iCalUId, turning one weekly
    // meeting into ~50 indistinguishable rows. Date filtering on `/events` is
    // intentionally omitted: OData's `start/dateTime` filter only sees a
    // series master's first occurrence, so a long-running meeting started
    // years ago would be excluded even if it has occurrences in our window.
    let path = format!(
        "/me/calendars/{}/events?$top=100&$select=id,iCalUId,subject,body,start,end,location,isAllDay,isCancelled,recurrence,attendees,organizer,reminderMinutesBeforeStart,showAs,lastModifiedDateTime,onlineMeeting,originalStart,responseStatus,type",
        config.outlook_calendar_id
    );

    let mut all_events = Vec::new();
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
            match from_outlook(graph_event, &config.outlook_account) {
                Ok(event) => all_events.push(event),
                Err(_) => continue, // Skip malformed events
            }
        }

        next_link = page.next_link;
        if next_link.is_none() {
            break;
        }
    }

    Ok(all_events)
}
