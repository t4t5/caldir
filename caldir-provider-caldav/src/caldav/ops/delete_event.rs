//! Delete an event from a CalDAV calendar.

use anyhow::Result;

use crate::caldav::{create_caldav_client, url_to_href};

use super::find_event_location;

/// Delete an event from a CalDAV calendar.
///
/// Tries the standard `{uid}.ics` URL first, falls back to UID-based REPORT query.
/// Treats "not found" (event already deleted) as success.
pub async fn delete_event(
    username: &str,
    password: &str,
    calendar_url: &str,
    event_id: &str,
) -> Result<()> {
    let caldav = create_caldav_client(calendar_url, username, password)?;
    let calendar_href = url_to_href(calendar_url);

    // Find the event's actual href (try {uid}.ics first, then UID-based REPORT)
    let location = find_event_location(&caldav, &calendar_href, calendar_url, event_id).await;

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
