//! Update an existing event on a CalDAV calendar.

use anyhow::{Context, Result};
use caldir_core::Event;
use http::Request;
use libdav::caldav::GetCalendarResources;

use crate::caldav::{create_caldav_client, url_to_href};

use super::{find_event_location, parse_event};

/// Update an existing event on a CalDAV calendar.
///
/// Tries the standard `{uid}.ics` URL first (works for iCloud and most servers),
/// then falls back to a UID-based REPORT query for servers that use non-standard hrefs.
/// Fetches the updated event back to get server-assigned values.
pub async fn update_event(
    username: &str,
    password: &str,
    calendar_url: &str,
    event: Event,
) -> Result<Event> {
    let caldav = create_caldav_client(calendar_url, username, password)?;

    let ics_content = event.to_ics_string();
    let calendar_href = url_to_href(calendar_url);

    // Try {uid}.ics first (most servers), fall back to UID-based REPORT query
    let location =
        find_event_location(&caldav, &calendar_href, calendar_url, event.uid.as_str()).await?;

    // Use request_raw to avoid duplicate Content-Type header (see create_event).
    let uri = caldav.relative_uri(&location.href)?;
    let request = Request::builder()
        .method(http::Method::PUT)
        .uri(&uri)
        .header("Content-Type", "text/calendar")
        .header("If-Match", &location.etag)
        .body(ics_content)?;

    let (parts, _body) = caldav
        .request_raw(request)
        .await
        .context("Failed to update event")?;

    if !parts.status.is_success() {
        anyhow::bail!("Failed to update event: server returned {}", parts.status);
    }

    // Fetch the updated event to get server-assigned values
    let get_response = caldav
        .request(GetCalendarResources::new(&calendar_href).with_hrefs([&location.href]))
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
