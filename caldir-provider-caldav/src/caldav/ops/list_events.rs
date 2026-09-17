//! Fetch every logical event in CalDAV resources within a time range.

use anyhow::{Context, Result};
use caldir_core::Event;

use crate::caldav::{create_caldav_client, format_caldav_datetime};

use super::resource::{parse_events, query};

/// Fetch masters and overrides without discarding recurrence components.
pub async fn fetch_events(
    username: &str,
    password: &str,
    calendar_url: &str,
    from: &str,
    to: &str,
) -> Result<Vec<Event>> {
    let caldav = create_caldav_client(calendar_url, username, password)?;
    let filter = format!(
        r#"<C:time-range start="{}" end="{}"/>"#,
        format_caldav_datetime(from),
        format_caldav_datetime(to)
    );
    let mut events = Vec::new();
    for resource in query(&caldav, calendar_url, &filter).await? {
        events.extend(
            parse_events(&resource.data)
                .with_context(|| format!("Invalid CalDAV resource {}", resource.href))?,
        );
    }
    Ok(events)
}
