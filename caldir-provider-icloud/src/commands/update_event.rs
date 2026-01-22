//! Update an existing event on iCloud Calendar.
//!
//! Uses CalDAV PUT to update an existing .ics resource.

use anyhow::{Context, Result};
use caldir_core::event::Event;
use caldir_core::ics::{generate_ics, parse_event};
use caldir_core::remote::protocol::UpdateEvent;

use crate::caldav::event_url;
use crate::remote_config::ICloudRemoteConfig;
use crate::session::Session;

pub async fn handle(cmd: UpdateEvent) -> Result<Event> {
    let config = ICloudRemoteConfig::try_from(&cmd.remote_config)?;
    let apple_id = &config.icloud_account;
    let calendar_url = &config.icloud_calendar_url;

    let session = Session::load(apple_id)?;

    // Update event using blocking HTTP
    let updated_event = tokio::task::spawn_blocking({
        let session = session.clone();
        let calendar_url = calendar_url.clone();
        let event = cmd.event.clone();
        move || update_event_caldav(&session, &calendar_url, event)
    })
    .await
    .context("Task join error")??;

    Ok(updated_event)
}

/// Update event via CalDAV PUT.
fn update_event_caldav(session: &Session, calendar_url: &str, event: Event) -> Result<Event> {
    let client = reqwest::blocking::Client::new();

    // Generate ICS content
    let ics_content = generate_ics(&event)?;

    // Build URL for the event
    let url = event_url(calendar_url, &event.id);

    let (username, password) = session.credentials();

    let response = client
        .put(&url)
        .basic_auth(username, Some(password))
        .header("Content-Type", "text/calendar; charset=utf-8")
        // Note: We could use If-Match with etag for conditional update,
        // but for simplicity we overwrite unconditionally
        .body(ics_content.clone())
        .send()
        .context("Failed to update event")?;

    let status = response.status();
    if !status.is_success() && status.as_u16() != 201 && status.as_u16() != 204 {
        let error_body = response.text().unwrap_or_default();
        anyhow::bail!(
            "Failed to update event (status {}): {}",
            status,
            error_body
        );
    }

    // Fetch the updated event to get server-assigned values
    let fetched_event = fetch_event(session, &url)?;

    Ok(fetched_event.unwrap_or(event))
}

/// Fetch a single event by URL.
fn fetch_event(session: &Session, url: &str) -> Result<Option<Event>> {
    let client = reqwest::blocking::Client::new();
    let (username, password) = session.credentials();

    let response = client
        .get(url)
        .basic_auth(username, Some(password))
        .header("Accept", "text/calendar")
        .send()
        .context("Failed to fetch updated event")?;

    if !response.status().is_success() {
        return Ok(None);
    }

    let ics_content = response.text().context("Failed to read event body")?;

    Ok(parse_event(&ics_content))
}
