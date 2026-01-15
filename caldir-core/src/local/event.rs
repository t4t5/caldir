use crate::constants::DEFAULT_SYNC_DAYS;
use crate::event::Event;
use crate::ics;

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use std::path::PathBuf;

/// A local calendar event (stored as an ics file)
#[derive(Debug, Clone)]
pub struct LocalEvent {
    /// Path to the .ics file
    pub path: PathBuf,
    /// The event data
    pub event: Event,
    /// File modification time (used for sync direction detection)
    pub modified: Option<DateTime<Utc>>,
}

impl LocalEvent {
    pub fn from_file(path: PathBuf) -> Result<Self> {
        let content = std::fs::read_to_string(&path)?;

        let Some(event) = ics::parse_event(&content) else {
            return Err(anyhow::anyhow!(
                "Failed to parse event from {}",
                path.display()
            ));
        };

        // Get file modification time for sync direction detection
        let modified = std::fs::metadata(&path)
            .ok()
            .and_then(|m| m.modified().ok())
            .map(DateTime::<Utc>::from);

        Ok(LocalEvent {
            path,
            event,
            modified,
        })
    }

    /// Check if this event falls within the sync window (±SYNC_DAYS from now).
    pub fn is_in_sync_range(&self) -> bool {
        let now = Utc::now();
        let range_start = now - Duration::days(DEFAULT_SYNC_DAYS);
        let range_end = now + Duration::days(DEFAULT_SYNC_DAYS);

        match self.event.start.to_utc() {
            Some(start) => start >= range_start && start <= range_end,
            None => true,
        }
    }

    /// Check if local file was modified after the remote event was updated.
    pub fn is_newer_than(&self, remote: &Event) -> bool {
        match (self.modified, remote.updated) {
            (Some(local_mtime), Some(remote_updated)) => local_mtime > remote_updated,
            _ => false,
        }
    }
}
