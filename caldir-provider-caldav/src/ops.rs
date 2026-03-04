//! Pure CalDAV operations taking credentials and URLs as parameters.
//!
//! These functions are provider-agnostic and can be used by any CalDAV-based
//! provider (iCloud, generic CalDAV, etc.).

use anyhow::{Context, Result};
use caldir_core::event::Event;
use caldir_core::ics::{generate_ics, parse_event};
use http::Request;
use libdav::caldav::{FindCalendarHomeSet, FindCalendars, GetCalendarResources};
use libdav::dav::GetProperty;
use libdav::names;

use crate::caldav::{
    absolute_url, create_caldav_client, event_url, format_caldav_datetime, url_to_href,
    FindEventByUid, GetCalendarResourcesInRange,
};

/// Discovered CalDAV endpoints from the connect flow.
pub struct DiscoveredEndpoints {
    pub principal_url: String,
    pub calendar_home_url: String,
}

/// A raw calendar entry returned by CalDAV discovery.
pub struct RawCalendar {
    /// The href path of the calendar collection.
    pub href: String,
    /// Display name of the calendar.
    pub name: String,
    /// Calendar color (may be in #RRGGBBAA or #RRGGBB format depending on server).
    pub color: Option<String>,
    /// Absolute URL of the calendar.
    pub url: String,
}

/// Discover CalDAV principal and calendar-home URLs.
///
/// Performs PROPFIND requests to find the current user principal and calendar home set.
pub async fn discover_endpoints(
    base_url: &str,
    username: &str,
    password: &str,
) -> Result<DiscoveredEndpoints> {
    let caldav = create_caldav_client(base_url, username, password)?;

    let principal = caldav
        .find_current_user_principal()
        .await
        .context("Failed to find current user principal")?
        .ok_or_else(|| {
            anyhow::anyhow!("Authentication failed. Check your username and password.")
        })?;

    let principal_url = absolute_url(&caldav, principal.path());

    let home_set_response = caldav
        .request(FindCalendarHomeSet::new(&principal))
        .await
        .context("Failed to find calendar home set")?;

    let calendar_home = home_set_response
        .home_sets
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("No calendar home set found for this account"))?;

    let calendar_home_url = absolute_url(&caldav, calendar_home.path());

    Ok(DiscoveredEndpoints {
        principal_url,
        calendar_home_url,
    })
}

/// List all calendars under a calendar home URL.
///
/// Returns raw calendar data without provider-specific wrapping, so each
/// provider can apply its own color normalization and remote config.
pub async fn list_calendars_raw(
    username: &str,
    password: &str,
    calendar_home_url: &str,
) -> Result<Vec<RawCalendar>> {
    let caldav = create_caldav_client(calendar_home_url, username, password)?;

    let calendar_home_uri: http::Uri = calendar_home_url
        .parse()
        .context("Invalid calendar home URL")?;

    let response = caldav
        .request(FindCalendars::new(&calendar_home_uri))
        .await
        .context("Failed to list calendars")?;

    if response.calendars.is_empty() {
        anyhow::bail!("No calendars found for this account");
    }

    let mut calendars = Vec::new();

    for calendar in response.calendars {
        // Get displayname
        let display_name = caldav
            .request(GetProperty::new(&calendar.href, &names::DISPLAY_NAME))
            .await
            .ok()
            .and_then(|r| r.value)
            .unwrap_or_else(|| {
                calendar
                    .href
                    .trim_end_matches('/')
                    .rsplit('/')
                    .next()
                    .unwrap_or("Calendar")
                    .to_string()
            });

        // Get calendar color
        let color = caldav
            .request(GetProperty::new(&calendar.href, &names::CALENDAR_COLOUR))
            .await
            .ok()
            .and_then(|r| r.value);

        let url = absolute_url(&caldav, &calendar.href);

        calendars.push(RawCalendar {
            href: calendar.href,
            name: display_name,
            color,
            url,
        });
    }

    Ok(calendars)
}

/// Fetch events from a CalDAV calendar within a time range.
pub async fn fetch_events(
    username: &str,
    password: &str,
    calendar_url: &str,
    from: &str,
    to: &str,
) -> Result<Vec<Event>> {
    let caldav = create_caldav_client(calendar_url, username, password)?;

    let calendar_href = url_to_href(calendar_url);

    let from_caldav = format_caldav_datetime(from);
    let to_caldav = format_caldav_datetime(to);

    let response = caldav
        .request(GetCalendarResourcesInRange::new(
            &calendar_href,
            &from_caldav,
            &to_caldav,
        ))
        .await
        .context("Failed to fetch calendar resources")?;

    let mut events = Vec::new();
    for resource in response.resources {
        if let Some(event) = parse_event(&resource.data) {
            events.push(event);
        }
    }

    Ok(events)
}

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

    let ics_content = generate_ics(&event)?;

    let full_url = event_url(calendar_url, &event.uid);
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

    if let Some(response) = get_response
        && let Some(resource) = response.resources.into_iter().next()
        && let Ok(content) = resource.content
        && let Some(fetched_event) = parse_event(&content.data)
    {
        return Ok(fetched_event);
    }

    Ok(event)
}

/// Update an existing event on a CalDAV calendar.
///
/// Finds the event's server-assigned href by UID (not all servers use {uid}.ics),
/// then uses a conditional PUT with the current etag.
/// Fetches the updated event back to get server-assigned values.
pub async fn update_event(
    username: &str,
    password: &str,
    calendar_url: &str,
    event: Event,
) -> Result<Event> {
    let caldav = create_caldav_client(calendar_url, username, password)?;

    let ics_content = generate_ics(&event)?;

    let calendar_href = url_to_href(calendar_url);

    // Find the event's actual href and etag by UID (servers may use arbitrary filenames)
    let location = caldav
        .request(FindEventByUid::new(&calendar_href, &event.uid))
        .await
        .context("Failed to find event on server - it may have been deleted")?;

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
        anyhow::bail!(
            "Failed to update event: server returned {}",
            parts.status
        );
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

/// Delete an event from a CalDAV calendar.
///
/// Finds the event's server-assigned href by UID, then deletes it.
/// Treats "not found" (event already deleted) as success.
pub async fn delete_event(
    username: &str,
    password: &str,
    calendar_url: &str,
    event_id: &str,
) -> Result<()> {
    let caldav = create_caldav_client(calendar_url, username, password)?;
    let calendar_href = url_to_href(calendar_url);

    // Find the event's actual href by UID
    let location = caldav
        .request(FindEventByUid::new(&calendar_href, event_id))
        .await;

    let href = match location {
        Ok(loc) => loc.href,
        Err(_) => {
            // Event not found on server — treat as already deleted
            return Ok(());
        }
    };

    let result = caldav
        .request(libdav::dav::Delete::new(&href).force())
        .await;

    match result {
        Ok(_) => Ok(()),
        Err(e) => {
            let error_string = format!("{:?}", e);
            if error_string.contains("404") || error_string.contains("NOT_FOUND") {
                Ok(())
            } else {
                Err(anyhow::anyhow!("Failed to delete event: {}", e))
            }
        }
    }
}
