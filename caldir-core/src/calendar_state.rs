//! Sync state tracking for calendars.

use std::{collections::HashSet, path::Path};

use crate::error::CalDirResult;

/// Tracks which events have been synced (for delete detection).
pub struct CalendarState {
    synced_uids: HashSet<String>,
}

impl CalendarState {
    /// Load sync state from .caldir/state/synced_uids
    pub fn load(calendar_dir: &Path) -> CalDirResult<Self> {
        let path = calendar_dir.join(".caldir/state/synced_uids");
        let synced_uids = if path.exists() {
            std::fs::read_to_string(&path)?
                .lines()
                .filter(|l| !l.is_empty())
                .map(String::from)
                .collect()
        } else {
            HashSet::new()
        };
        Ok(Self { synced_uids })
    }

    pub fn synced_uids(&self) -> &HashSet<String> {
        &self.synced_uids
    }

    /// Save sync state to .caldir/state/synced_uids (atomic write)
    pub fn save(calendar_dir: &Path, uids: &HashSet<String>) -> CalDirResult<()> {
        let dir = calendar_dir.join(".caldir/state");
        std::fs::create_dir_all(&dir)?;

        let path = dir.join("synced_uids");
        let temp = dir.join("synced_uids.tmp");

        // Sort for deterministic output
        let mut sorted: Vec<_> = uids.iter().map(|s| s.as_str()).collect();
        sorted.sort();
        let content = sorted.join("\n");

        std::fs::write(&temp, content)?;
        std::fs::rename(&temp, &path)?;
        Ok(())
    }
}
