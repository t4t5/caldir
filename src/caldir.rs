//! Local calendar directory operations.
//!
//! The "caldir" is a directory of .ics files, one per event.
//! This module handles reading from and writing to this directory.

use crate::event::Event;
use crate::ics;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Information about a local .ics file
pub struct LocalEvent {
    /// Path to the .ics file
    pub path: PathBuf,
    /// Parsed event data
    pub event: Event,
    /// File modification time (for push detection)
    pub modified: Option<DateTime<Utc>>,
}

/// Read all .ics files from the calendar directory.
/// Returns a map of UID -> LocalEvent.
pub fn read_all(dir: &Path) -> Result<HashMap<String, LocalEvent>> {
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
            // Get file modification time for push detection
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

/// Write an .ics file to the calendar directory.
pub fn write_event(dir: &Path, filename: &str, content: &str) -> Result<()> {
    let path = dir.join(filename);
    std::fs::write(&path, content)
        .with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

/// Delete an .ics file.
pub fn delete_event(path: &Path) -> Result<()> {
    std::fs::remove_file(path)
        .with_context(|| format!("Failed to delete {}", path.display()))?;
    Ok(())
}

/// Statistics from applying changes to the local directory
#[derive(Default)]
pub struct ApplyStats {
    pub created: usize,
    pub updated: usize,
    pub deleted: usize,
}

impl ApplyStats {
    /// Accumulate stats from another ApplyStats
    pub fn add(&mut self, other: &ApplyStats) {
        self.created += other.created;
        self.updated += other.updated;
        self.deleted += other.deleted;
    }

    pub fn has_changes(&self) -> bool {
        self.created > 0 || self.updated > 0 || self.deleted > 0
    }
}
