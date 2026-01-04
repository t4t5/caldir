//! List events from a calendar directory.

use super::LocalEvent;
use crate::ics;
use anyhow::Result;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::path::Path;

/// List all events in a calendar directory.
///
/// Returns a map of UID -> LocalEvent for all .ics files found.
pub fn list(dir: &Path) -> Result<HashMap<String, LocalEvent>> {
    let mut events: HashMap<String, LocalEvent> = HashMap::new();

    if !dir.exists() {
        return Ok(events);
    }

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().map(|e| e == "ics").unwrap_or(false)
            && let Ok(content) = std::fs::read_to_string(&path)
            && let Some(event) = ics::parse_event(&content)
        {
            // Get file modification time for sync direction detection
            let modified = std::fs::metadata(&path)
                .ok()
                .and_then(|m| m.modified().ok())
                .map(DateTime::<Utc>::from);

            let uid = event.id.clone();
            events.insert(uid, LocalEvent { path, event, modified });
        }
    }

    Ok(events)
}
