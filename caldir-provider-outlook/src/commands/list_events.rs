use anyhow::{Context, Result};
use caldir_core::event::Event;
use caldir_core::remote::protocol::ListEvents;
use url::form_urlencoded;

use crate::graph_client::GraphClient;
use crate::graph_types::{GraphEvent, GraphResponse};
use crate::outlook_event::from_outlook::from_outlook;
use crate::remote_config::OutlookRemoteConfig;
use crate::session::Session;

pub async fn handle(cmd: ListEvents) -> Result<Vec<Event>> {
    let config = OutlookRemoteConfig::try_from(&cmd.remote_config)?;
    let session = Session::load_valid(&config.outlook_account).await?;
    let graph = GraphClient::new(session.access_token());

    let query: String = form_urlencoded::Serializer::new(String::new())
        .append_pair("startDateTime", &cmd.from)
        .append_pair("endDateTime", &cmd.to)
        .append_pair("$top", "100")
        .append_pair("$select", "id,iCalUId,subject,body,start,end,location,isAllDay,isCancelled,recurrence,attendees,organizer,reminderMinutesBeforeStart,showAs,lastModifiedDateTime,onlineMeeting,originalStart,responseStatus")
        .finish();

    let path = format!(
        "/me/calendars/{}/calendarView?{}",
        config.outlook_calendar_id, query
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
