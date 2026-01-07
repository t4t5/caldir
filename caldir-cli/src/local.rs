//! Per-calendar local state (stored in .caldir/ directory).

use anyhow::Result;
use std::collections::HashSet;
use std::path::Path;

/// Sync state for a calendar - tracks which event UIDs have been synced.
/// Used to detect local deletions (UID was synced before but file is now gone).
pub struct LocalState {
    synced_uids: HashSet<String>,
}

impl LocalState {
    /// Load sync state from .caldir/state/synced_uids
    pub fn load(calendar_dir: &Path) -> Result<Self> {
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
    pub fn save(calendar_dir: &Path, uids: &HashSet<String>) -> Result<()> {
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
