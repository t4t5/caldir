//! Delete an event from iCloud Calendar.
//!
//! Uses CalDAV DELETE to remove the .ics resource.

use anyhow::{Context, Result};
use caldir_core::remote::protocol::DeleteEvent;

use crate::caldav::event_url;
use crate::remote_config::ICloudRemoteConfig;
use crate::session::Session;

pub async fn handle(cmd: DeleteEvent) -> Result<()> {
    let config = ICloudRemoteConfig::try_from(&cmd.remote_config)?;
    let apple_id = &config.icloud_account;
    let calendar_url = &config.icloud_calendar_url;

    let session = Session::load(apple_id)?;

    // Delete event using blocking HTTP
    tokio::task::spawn_blocking({
        let session = session.clone();
        let calendar_url = calendar_url.clone();
        let event_id = cmd.event_id.clone();
        move || delete_event_caldav(&session, &calendar_url, &event_id)
    })
    .await
    .context("Task join error")??;

    Ok(())
}

/// Delete event via CalDAV DELETE.
fn delete_event_caldav(session: &Session, calendar_url: &str, event_id: &str) -> Result<()> {
    let client = reqwest::blocking::Client::new();

    // Build URL for the event
    let url = event_url(calendar_url, event_id);

    let (username, password) = session.credentials();

    let response = client
        .delete(&url)
        .basic_auth(username, Some(password))
        .send()
        .context("Failed to delete event")?;

    let status = response.status();
    // 204 No Content is the expected success response for DELETE
    // 404 Not Found is also acceptable (event already deleted)
    if !status.is_success() && status.as_u16() != 204 && status.as_u16() != 404 {
        let error_body = response.text().unwrap_or_default();
        anyhow::bail!(
            "Failed to delete event (status {}): {}",
            status,
            error_body
        );
    }

    Ok(())
}
