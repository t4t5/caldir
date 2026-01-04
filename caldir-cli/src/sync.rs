//! Sync state tracking.
//!
//! Tracks which event UIDs have been synced and sync operation statistics.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Tracks which event UIDs have been synced for a calendar.
/// Used to detect local deletions (UID in state but no local file).
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SyncState {
    pub synced_uids: HashSet<String>,
}

/// Get sync state file path for a calendar directory
pub fn state_path(calendar_dir: &Path) -> PathBuf {
    calendar_dir.join(".caldir-sync")
}

/// Load sync state from calendar directory
pub fn load_state(calendar_dir: &Path) -> Result<SyncState> {
    let path = state_path(calendar_dir);
    if !path.exists() {
        return Ok(SyncState::default());
    }
    let contents = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read sync state at {}", path.display()))?;
    let state: SyncState = serde_json::from_str(&contents)
        .with_context(|| format!("Failed to parse sync state at {}", path.display()))?;
    Ok(state)
}

/// Save sync state to calendar directory (atomic write via temp file + rename)
pub fn save_state(calendar_dir: &Path, state: &SyncState) -> Result<()> {
    let path = state_path(calendar_dir);
    let temp_path = calendar_dir.join(".caldir-sync.tmp");

    let contents =
        serde_json::to_string_pretty(state).context("Failed to serialize sync state")?;

    // Write to temp file first
    std::fs::write(&temp_path, contents)
        .with_context(|| format!("Failed to write temp sync state at {}", temp_path.display()))?;

    // Atomic rename (on POSIX systems, rename is atomic if same filesystem)
    std::fs::rename(&temp_path, &path)
        .with_context(|| format!("Failed to rename sync state to {}", path.display()))?;

    Ok(())
}

/// Statistics from sync operations
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
