use crate::ics;
use crate::providers::gcal::Event;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;

/// Statistics from a sync operation
pub struct SyncStats {
    pub created: usize,
    pub updated: usize,
    pub deleted: usize,
}

/// Sync events to a directory
pub fn sync_events_to_dir(events: &[Event], dir: &Path) -> Result<SyncStats> {
    let mut stats = SyncStats {
        created: 0,
        updated: 0,
        deleted: 0,
    };

    // Build a map of existing files: UID -> filepath
    let mut existing_files: HashMap<String, std::path::PathBuf> = HashMap::new();

    if dir.exists() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().map(|e| e == "ics").unwrap_or(false) {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Some(uid) = ics::parse_uid(&content) {
                        existing_files.insert(uid, path);
                    }
                }
            }
        }
    }

    // Track which UIDs we've seen from the cloud
    let mut seen_uids: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Process each event from the cloud
    for event in events {
        seen_uids.insert(event.id.clone());

        let new_filename = ics::generate_filename(event);
        let new_path = dir.join(&new_filename);
        let new_content = ics::generate_ics(event)?;

        if let Some(existing_path) = existing_files.get(&event.id) {
            // Event exists locally
            let existing_content = std::fs::read_to_string(existing_path)
                .with_context(|| format!("Failed to read {}", existing_path.display()))?;

            // Check if content changed or filename needs updating
            let content_changed = existing_content.trim() != new_content.trim();
            let filename_changed = existing_path != &new_path;

            if content_changed || filename_changed {
                // Remove old file if filename changed
                if filename_changed {
                    std::fs::remove_file(existing_path)
                        .with_context(|| format!("Failed to remove {}", existing_path.display()))?;
                }

                // Write new content
                std::fs::write(&new_path, &new_content)
                    .with_context(|| format!("Failed to write {}", new_path.display()))?;

                stats.updated += 1;
            }
        } else {
            // New event - create file
            std::fs::write(&new_path, &new_content)
                .with_context(|| format!("Failed to write {}", new_path.display()))?;

            stats.created += 1;
        }
    }

    // Delete local files for events that no longer exist in the cloud
    for (uid, path) in &existing_files {
        if !seen_uids.contains(uid) {
            std::fs::remove_file(path)
                .with_context(|| format!("Failed to delete {}", path.display()))?;

            stats.deleted += 1;
        }
    }

    Ok(stats)
}
