//! List iCloud Calendars (name + config) for a given account.
//!
//! Uses libdav to discover calendar collections and fetch their properties.

use anyhow::{Context, Result};
use caldir_core::calendar::config::CalendarConfig;
use caldir_core::remote::{protocol::ListCalendars, provider::Provider, Remote};
use http::Uri;
use libdav::caldav::FindCalendars;
use libdav::dav::GetProperty;
use libdav::names;

use crate::caldav::create_caldav_client;
use crate::remote_config::ICloudRemoteConfig;
use crate::session::Session;

pub async fn handle(cmd: ListCalendars) -> Result<Vec<CalendarConfig>> {
    let apple_id = &cmd.account_identifier;

    let session = Session::load(apple_id)?;

    let calendars = list_calendars_caldav(&session).await?;

    Ok(calendars)
}

/// List calendars via CalDAV using libdav.
async fn list_calendars_caldav(session: &Session) -> Result<Vec<CalendarConfig>> {
    let (username, password) = session.credentials();

    // Create CalDAV client
    let caldav = create_caldav_client(&session.calendar_home_url, username, password)?;

    // Parse calendar home as URI
    let calendar_home_uri: Uri = session
        .calendar_home_url
        .parse()
        .context("Invalid calendar home URL")?;

    // Find all calendars under the calendar home
    let response = caldav
        .request(FindCalendars::new(&calendar_home_uri))
        .await
        .context("Failed to list calendars")?;

    if response.calendars.is_empty() {
        anyhow::bail!("No calendars found for this account");
    }

    // For each calendar, fetch displayname and color
    let mut configs = Vec::new();

    for calendar in response.calendars {
        // Get displayname
        let display_name = caldav
            .request(GetProperty::new(&calendar.href, &names::DISPLAY_NAME))
            .await
            .ok()
            .and_then(|r| r.value)
            .unwrap_or_else(|| {
                // Fallback: use last path segment as name
                calendar
                    .href
                    .trim_end_matches('/')
                    .rsplit('/')
                    .next()
                    .unwrap_or("Calendar")
                    .to_string()
            });

        // Get calendar color (Apple extension)
        let color = caldav
            .request(GetProperty::new(&calendar.href, &names::CALENDAR_COLOUR))
            .await
            .ok()
            .and_then(|r| r.value)
            .map(|c| {
                // iCloud returns colors as #RRGGBBAA, convert to #RRGGBB
                if c.len() == 9 && c.starts_with('#') {
                    c[..7].to_string()
                } else {
                    c
                }
            });

        // Build absolute URL for the calendar
        let calendar_url = format!(
            "{}://{}{}",
            caldav.base_url().scheme_str().unwrap_or("https"),
            caldav
                .base_url()
                .authority()
                .map(|a| a.as_str())
                .unwrap_or("caldav.icloud.com"),
            calendar.href
        );

        let remote_config = ICloudRemoteConfig::new(&session.apple_id, &calendar_url);
        let remote = Remote::new(Provider::from_name("icloud"), remote_config.into());

        configs.push(CalendarConfig {
            name: Some(display_name),
            color,
            read_only: None,
            remote: Some(remote),
        });
    }

    Ok(configs)
}
