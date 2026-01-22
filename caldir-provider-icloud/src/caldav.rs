//! CalDAV client helpers for iCloud.
//!
//! Provides utilities for CalDAV operations using reqwest.

/// Build the URL for an event resource.
pub fn event_url(calendar_url: &str, event_uid: &str) -> String {
    let base = calendar_url.trim_end_matches('/');
    format!("{}/{}.ics", base, event_uid)
}
