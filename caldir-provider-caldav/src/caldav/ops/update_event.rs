//! Update a logical event within a CalDAV resource.

use anyhow::Result;
use caldir_core::Event;

/// Update the matching component, preserving the rest of its resource.
pub async fn update_event(
    username: &str,
    password: &str,
    calendar_url: &str,
    event: Event,
) -> Result<Event> {
    super::resource::write_event(username, password, calendar_url, event, false).await
}
