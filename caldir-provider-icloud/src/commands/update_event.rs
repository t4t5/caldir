//! Update an existing event on iCloud Calendar.
//!
//! Uses libdav PutResource to update an existing .ics resource.

use anyhow::{Context, Result};
use caldir_core::event::Event;
use caldir_core::ics::{generate_ics, parse_event};
use caldir_core::remote::protocol::UpdateEvent;
use libdav::caldav::GetCalendarResources;
use libdav::dav::{GetEtag, PutResource, mime_types};

use crate::caldav::{create_caldav_client, event_url, url_to_href};
use crate::remote_config::ICloudRemoteConfig;
use crate::session::Session;

pub async fn handle(cmd: UpdateEvent) -> Result<Event> {
    let config = ICloudRemoteConfig::try_from(&cmd.remote_config)?;
    let apple_id = &config.icloud_account;
    let calendar_url = &config.icloud_calendar_url;

    let session = Session::load(apple_id)?;

    let updated_event = update_event_caldav(&session, calendar_url, cmd.event).await?;

    Ok(updated_event)
}

/// Update event via CalDAV using libdav.
async fn update_event_caldav(
    session: &Session,
    calendar_url: &str,
    event: Event,
) -> Result<Event> {
    let (username, password) = session.credentials();

    // Create CalDAV client
    let caldav = create_caldav_client(calendar_url, username, password)?;

    // Generate ICS content
    let ics_content = generate_ics(&event)?;

    // Build href for the event (use uid as the filename, per CalDAV convention)
    let full_url = event_url(calendar_url, &event.uid);
    let href = url_to_href(&full_url);

    // Get current etag for conditional update
    let etag_response = caldav
        .request(GetEtag::new(&href))
        .await
        .context("Failed to get event etag - event may not exist")?;

    // Update the resource using PUT with If-Match (conditional update)
    caldav
        .request(PutResource::new(&href).update(&ics_content, mime_types::CALENDAR, &etag_response.etag))
        .await
        .context("Failed to update event")?;

    // Fetch the updated event to get server-assigned values
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
