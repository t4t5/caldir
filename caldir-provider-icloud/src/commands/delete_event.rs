//! Delete an event from iCloud Calendar.
//!
//! Uses libdav Delete to remove the .ics resource.

use anyhow::Result;
use caldir_core::remote::protocol::DeleteEvent;
use libdav::dav::Delete;

use crate::caldav::{create_caldav_client, event_url, url_to_href};
use crate::remote_config::ICloudRemoteConfig;
use crate::session::Session;

pub async fn handle(cmd: DeleteEvent) -> Result<()> {
    let config = ICloudRemoteConfig::try_from(&cmd.remote_config)?;
    let apple_id = &config.icloud_account;
    let calendar_url = &config.icloud_calendar_url;

    let session = Session::load(apple_id)?;

    delete_event_caldav(&session, calendar_url, &cmd.event_id).await?;

    Ok(())
}

/// Delete event via CalDAV using libdav.
async fn delete_event_caldav(
    session: &Session,
    calendar_url: &str,
    event_id: &str,
) -> Result<()> {
    let (username, password) = session.credentials();

    // Create CalDAV client
    let caldav = create_caldav_client(calendar_url, username, password)?;

    // Build href for the event
    let full_url = event_url(calendar_url, event_id);
    let href = url_to_href(&full_url);

    // Delete the resource (force = unconditional, no etag check)
    // Note: 404 (already deleted) is handled by ignoring the error
    let result = caldav.request(Delete::new(&href).force()).await;

    // Accept success or "not found" (event already deleted)
    match result {
        Ok(_) => Ok(()),
        Err(e) => {
            let error_string = format!("{:?}", e);
            if error_string.contains("404") || error_string.contains("NOT_FOUND") {
                // Event already deleted, treat as success
                Ok(())
            } else {
                Err(anyhow::anyhow!("Failed to delete event: {}", e))
            }
        }
    }
}
