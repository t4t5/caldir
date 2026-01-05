use crate::ics;
use anyhow::Result;
use std::path::PathBuf;

use caldir_core::Event;
use chrono::{DateTime, Utc};

/// A local calendar event (stored as an ics file)
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
}
