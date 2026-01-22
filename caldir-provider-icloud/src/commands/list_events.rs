//! List events within a time range from an iCloud calendar.
//!
//! Uses libdav to query CalDAV resources.

use anyhow::{Context, Result};
use caldir_core::event::Event;
use caldir_core::ics::parse_event;
use caldir_core::remote::protocol::ListEvents;
use chrono::{DateTime, NaiveDate, Utc};
use libdav::caldav::GetCalendarResources;

use crate::caldav::{create_caldav_client, url_to_href};
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

/// Fetch events via CalDAV using libdav.
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

    // Parse date range for filtering
    let from_date = parse_date(from)?;
    let to_date = parse_date(to)?;

    // Fetch all calendar resources
    // Note: libdav 0.10 doesn't support time range filtering in GetCalendarResources,
    // so we fetch all events and filter locally
    let response = caldav
        .request(GetCalendarResources::new(&calendar_href))
        .await
        .context("Failed to fetch calendar resources")?;

    // Parse ICS content into events and filter by date range
    let mut events = Vec::new();
    for resource in response.resources {
        if let Ok(content) = resource.content {
            if let Some(event) = parse_event(&content.data) {
                // Filter by date range
                if event_in_range(&event, &from_date, &to_date) {
                    events.push(event);
                }
            }
        }
    }

    Ok(events)
}

/// Parse a date string into a DateTime for comparison.
fn parse_date(date_str: &str) -> Result<DateTime<Utc>> {
    // Try parsing as full RFC3339 datetime
    if let Ok(dt) = DateTime::parse_from_rfc3339(date_str) {
        return Ok(dt.with_timezone(&Utc));
    }

    // Try parsing as date only (YYYY-MM-DD)
    if let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
        return Ok(date.and_hms_opt(0, 0, 0).unwrap().and_utc());
    }

    anyhow::bail!("Invalid date format: {}", date_str)
}

/// Check if an event falls within the given date range.
fn event_in_range(event: &Event, from: &DateTime<Utc>, to: &DateTime<Utc>) -> bool {
    use caldir_core::event::EventTime;

    // Get event start time as UTC
    let event_start = match &event.start {
        EventTime::DateTimeUtc(dt) => *dt,
        EventTime::Date(date) => date.and_hms_opt(0, 0, 0).unwrap().and_utc(),
        EventTime::DateTimeFloating(dt) => dt.and_utc(),
        EventTime::DateTimeZoned { datetime, .. } => datetime.and_utc(),
    };

    // Get event end time as UTC
    let event_end = match &event.end {
        EventTime::DateTimeUtc(dt) => *dt,
        EventTime::Date(date) => date.and_hms_opt(23, 59, 59).unwrap().and_utc(),
        EventTime::DateTimeFloating(dt) => dt.and_utc(),
        EventTime::DateTimeZoned { datetime, .. } => datetime.and_utc(),
    };

    // Event is in range if it overlaps with [from, to]
    // Event overlaps if: event_start < to AND event_end >= from
    event_start < *to && event_end >= *from
}
