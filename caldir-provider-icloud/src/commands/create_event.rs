//! Create a new event on iCloud Calendar.
//!
//! Uses libdav PutResource to create a new .ics resource.

use anyhow::{Context, Result};
use caldir_core::event::Event;
use caldir_core::ics::{generate_ics, parse_event};
use caldir_core::remote::protocol::CreateEvent;
use http::Request;
use libdav::caldav::GetCalendarResources;

use crate::caldav::{create_caldav_client, event_url, url_to_href};
use crate::remote_config::ICloudRemoteConfig;
use crate::session::Session;

pub async fn handle(cmd: CreateEvent) -> Result<Event> {
    let config = ICloudRemoteConfig::try_from(&cmd.remote_config)?;
    let apple_id = &config.icloud_account;
    let calendar_url = &config.icloud_calendar_url;

    let session = Session::load(apple_id)?;

    let created_event = create_event_caldav(&session, calendar_url, cmd.event).await?;

    Ok(created_event)
}

/// Create event via CalDAV using a raw PUT request (with debug output).
async fn create_event_caldav(
    session: &Session,
    calendar_url: &str,
    event: Event,
) -> Result<Event> {
    let (username, password) = session.credentials();

    // Create CalDAV client
    let caldav = create_caldav_client(calendar_url, username, password)?;

    // Generate ICS content
    let ics_content = generate_ics(&event)?;

    // Build href for the new event (use uid as the filename, per CalDAV convention)
    let full_url = event_url(calendar_url, &event.uid);
    let href = url_to_href(&full_url);

    // Use request_raw instead of libdav's PutResource to avoid a duplicate
    // Content-Type header. WebDavClient::request() sets a default
    // "Content-Type: application/xml", then PutResource appends
    // "Content-Type: text/calendar" — iCloud rejects the duplicate with 501.
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
        anyhow::bail!(
            "Failed to create event: server returned {}",
            parts.status
        );
    }

    // Fetch the created event to get server-assigned values
    let calendar_href = url_to_href(calendar_url);
    let get_response = caldav
        .request(GetCalendarResources::new(&calendar_href).with_hrefs([&href]))
        .await
        .ok();

    // Try to parse the fetched event, fall back to original if fetch fails
    if let Some(response) = get_response {
        if let Some(resource) = response.resources.into_iter().next() {
            if let Ok(content) = resource.content {
                if let Some(fetched_event) = parse_event(&content.data) {
                    return Ok(fetched_event);
                }
            }
        }
    }

    Ok(event)
}
