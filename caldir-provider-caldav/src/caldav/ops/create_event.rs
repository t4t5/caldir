//! Create a new event on a CalDAV calendar.

use anyhow::{Context, Result};
use caldir_core::Event;
use http::Request;
use libdav::caldav::GetCalendarResources;

use crate::caldav::{create_caldav_client, event_url, url_to_href};

use super::parse_event;

/// Create a new event on a CalDAV calendar.
///
/// Uses a raw PUT request to avoid duplicate Content-Type headers (a libdav quirk).
/// Fetches the created event back to get server-assigned values.
pub async fn create_event(
    username: &str,
    password: &str,
    calendar_url: &str,
    event: Event,
) -> Result<Event> {
    let caldav = create_caldav_client(calendar_url, username, password)?;

    let ics_content = event.to_ics_string();

    let full_url = event_url(calendar_url, event.uid.as_str());
    let href = url_to_href(&full_url);

    // Use request_raw instead of libdav's PutResource to avoid a duplicate
    // Content-Type header. WebDavClient::request() sets a default
    // "Content-Type: application/xml", then PutResource appends
    // "Content-Type: text/calendar" — some servers reject the duplicate.
    let uri = caldav.relative_uri(&href)?;
    let request = Request::builder()
        .method(http::Method::PUT)
        .uri(&uri)
        .header("Content-Type", "text/calendar")
        .header("If-None-Match", "*")
        .body(ics_content)?;

    let (parts, _body) = caldav
        .request_raw(request)
        .await
        .context("Failed to create event")?;

    if !parts.status.is_success() {
        anyhow::bail!("Failed to create event: server returned {}", parts.status);
    }

    // Fetch the created event to get server-assigned values
    let calendar_href = url_to_href(calendar_url);
    let get_response = caldav
        .request(GetCalendarResources::new(&calendar_href).with_hrefs([&href]))
        .await
        .ok();

    if let Some(response) = get_response
        && let Some(resource) = response.resources.into_iter().next()
        && let Ok(content) = resource.content
        && let Some(fetched_event) = parse_event(&content.data)
    {
        return Ok(fetched_event);
    }

    Ok(event)
}
