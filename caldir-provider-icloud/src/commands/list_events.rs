//! List events within a time range from an iCloud calendar.
//!
//! Uses libdav with a custom time-range filtered calendar-query.

use anyhow::{Context, Result};
use caldir_core::event::Event;
use caldir_core::ics::parse_event;
use caldir_core::remote::protocol::ListEvents;

use crate::caldav::{
    create_caldav_client, format_caldav_datetime, url_to_href, GetCalendarResourcesInRange,
};
use crate::remote_config::ICloudRemoteConfig;
use crate::session::Session;

pub async fn handle(cmd: ListEvents) -> Result<Vec<Event>> {
    let config = ICloudRemoteConfig::try_from(&cmd.remote_config)?;
    let apple_id = &config.icloud_account;
    let calendar_url = &config.icloud_calendar_url;

    let session = Session::load(apple_id)?;

    let events = fetch_events_caldav(&session, calendar_url, &cmd.from, &cmd.to).await?;

    Ok(events)
}

/// Fetch events via CalDAV using libdav with server-side time-range filtering.
async fn fetch_events_caldav(
    session: &Session,
    calendar_url: &str,
    from: &str,
    to: &str,
) -> Result<Vec<Event>> {
    let (username, password) = session.credentials();

    // Create CalDAV client
    let caldav = create_caldav_client(calendar_url, username, password)?;

    // Convert calendar URL to href (path only)
    let calendar_href = url_to_href(calendar_url);

    // Format dates for CalDAV (needs UTC format: YYYYMMDDTHHMMSSZ)
    let from_caldav = format_caldav_datetime(from);
    let to_caldav = format_caldav_datetime(to);

    // Fetch calendar resources with server-side time-range filtering
    let response = caldav
        .request(GetCalendarResourcesInRange::new(
            &calendar_href,
            &from_caldav,
            &to_caldav,
        ))
        .await
        .context("Failed to fetch calendar resources")?;

    // Parse ICS content into events
    let mut events = Vec::new();
    for resource in response.resources {
        if let Some(event) = parse_event(&resource.data) {
            events.push(event);
        }
    }

    Ok(events)
}
