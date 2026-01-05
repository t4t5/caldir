use crate::diff_new::{Change, ChangeKind, Source};
use crate::ics;
use anyhow::Result;
use std::path::PathBuf;

use caldir_core::Event;
use chrono::{DateTime, Utc};

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

    /// Compare this local event against a remote event.
    /// Returns Some(Change) if they differ, None if identical.
    pub fn diff_with(&self, remote: &Event) -> Option<Change> {
        if !self.differs_from(remote) {
            return None;
        }

        let local_is_newer = match (self.modified, remote.updated) {
            (Some(local_mtime), Some(remote_updated)) => local_mtime > remote_updated,
            _ => false,
        };

        Some(Change {
            source: if local_is_newer { Source::Local } else { Source::Remote },
            kind: ChangeKind::Update,
            local: Some(self.clone()),
            remote: Some(remote.clone()),
        })
    }

    fn differs_from(&self, remote: &Event) -> bool {
        self.event.summary != remote.summary
            || self.event.description != remote.description
            || self.event.location != remote.location
            || self.event.start != remote.start
            || self.event.end != remote.end
    }
}
