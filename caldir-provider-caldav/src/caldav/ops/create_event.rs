//! Create a logical event within a CalDAV resource.

use anyhow::Result;
use caldir_core::Event;

/// Create the matching component, preserving the rest of its resource.
pub async fn create_event(
    username: &str,
    password: &str,
    calendar_url: &str,
    event: Event,
) -> Result<Event> {
    super::resource::write_event(username, password, calendar_url, event, true).await
}
