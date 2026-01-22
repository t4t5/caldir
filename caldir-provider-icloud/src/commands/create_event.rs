//! Create a new event on iCloud Calendar.
//!
//! Uses CalDAV PUT to create a new .ics resource.

use anyhow::{Context, Result};
use caldir_core::event::Event;
use caldir_core::ics::{generate_ics, parse_event};
use caldir_core::remote::protocol::CreateEvent;

use crate::caldav::event_url;
use crate::remote_config::ICloudRemoteConfig;
use crate::session::Session;

pub async fn handle(cmd: CreateEvent) -> Result<Event> {
    let config = ICloudRemoteConfig::try_from(&cmd.remote_config)?;
    let apple_id = &config.icloud_account;
    let calendar_url = &config.icloud_calendar_url;

    let session = Session::load(apple_id)?;

    // Create event using blocking HTTP
    let created_event = tokio::task::spawn_blocking({
        let session = session.clone();
        let calendar_url = calendar_url.clone();
        let event = cmd.event.clone();
        move || create_event_caldav(&session, &calendar_url, event)
    })
    .await
    .context("Task join error")??;

    Ok(created_event)
}

/// Create event via CalDAV PUT.
fn create_event_caldav(session: &Session, calendar_url: &str, event: Event) -> Result<Event> {
    let client = reqwest::blocking::Client::new();

    // Generate ICS content
    let ics_content = generate_ics(&event)?;

    // Build URL for the new event
    let url = event_url(calendar_url, &event.id);

    let (username, password) = session.credentials();

    let response = client
        .put(&url)
        .basic_auth(username, Some(password))
        .header("Content-Type", "text/calendar; charset=utf-8")
        .header("If-None-Match", "*") // Fail if resource already exists
        .body(ics_content.clone())
        .send()
        .context("Failed to create event")?;

    let status = response.status();
    if !status.is_success() && status.as_u16() != 201 {
        let error_body = response.text().unwrap_or_default();
        anyhow::bail!(
            "Failed to create event (status {}): {}",
            status,
            error_body
        );
    }

    // Fetch the created event to get server-assigned values
    // (Some CalDAV servers may modify the event)
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
        .context("Failed to fetch created event")?;

    if !response.status().is_success() {
        return Ok(None);
    }

    let ics_content = response.text().context("Failed to read event body")?;

    Ok(parse_event(&ics_content))
}
