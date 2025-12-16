use crate::ics::{self, CalendarMetadata};
use crate::providers::gcal::Event;
use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Statistics from a sync operation
pub struct SyncStats {
    pub created: usize,
    pub updated: usize,
    pub deleted: usize,
}

/// A single change detected between local and remote
pub struct SyncChange {
    pub event_id: String,
    pub filename: String,
    pub summary: String,
}

/// Result of comparing local directory against remote state
pub struct SyncDiff {
    pub to_create: Vec<SyncChange>,
    pub to_update: Vec<SyncChange>,
    pub to_delete: Vec<SyncChange>,
}

impl SyncDiff {
    pub fn is_empty(&self) -> bool {
        self.to_create.is_empty() && self.to_update.is_empty() && self.to_delete.is_empty()
    }
}

/// Local file info parsed from .ics content
struct LocalEventInfo {
    path: std::path::PathBuf,
    summary: String,
    content: String,
}

/// Normalize ICS content for comparison by removing non-deterministic fields.
/// This ensures we only detect real content changes, not timestamp variations.
fn normalize_ics_for_comparison(content: &str) -> String {
    content
        .lines()
        .filter(|line| {
            // Skip DTSTAMP lines - these can vary between generations when
            // the event has no 'updated' timestamp from the provider
            !line.starts_with("DTSTAMP:")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Build a map of existing local .ics files
fn build_local_file_map(dir: &Path) -> Result<HashMap<String, LocalEventInfo>> {
    let mut existing_files: HashMap<String, LocalEventInfo> = HashMap::new();

    if dir.exists() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().map(|e| e == "ics").unwrap_or(false) {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Some(uid) = ics::parse_uid(&content) {
                        let summary = ics::parse_summary(&content).unwrap_or_default();
                        existing_files.insert(
                            uid,
                            LocalEventInfo {
                                path,
                                summary,
                                content,
                            },
                        );
                    }
                }
            }
        }
    }

    Ok(existing_files)
}

/// Compute the diff between remote events and local directory (without applying changes)
pub fn compute_sync_diff(
    events: &[Event],
    dir: &Path,
    metadata: &CalendarMetadata,
    debug: bool,
) -> Result<SyncDiff> {
    let mut diff = SyncDiff {
        to_create: Vec::new(),
        to_update: Vec::new(),
        to_delete: Vec::new(),
    };

    let existing_files = build_local_file_map(dir)?;
    let mut seen_uids: HashSet<String> = HashSet::new();

    // Process each event from the cloud
    for event in events {
        seen_uids.insert(event.id.clone());

        let new_filename = ics::generate_filename(event);
        let new_path = dir.join(&new_filename);
        let new_content = ics::generate_ics(event, metadata)?;

        if let Some(local_info) = existing_files.get(&event.id) {
            // Event exists locally - check if it changed
            // Use normalized comparison to ignore non-deterministic fields like DTSTAMP
            let local_normalized = normalize_ics_for_comparison(&local_info.content);
            let new_normalized = normalize_ics_for_comparison(&new_content);
            let content_changed = local_normalized.trim() != new_normalized.trim();
            let filename_changed = local_info.path != new_path;

            if content_changed || filename_changed {
                // Debug: show what's different
                if debug && content_changed {
                    eprintln!("DEBUG: Content difference for '{}':", event.summary);
                    let local_lines: Vec<&str> = local_normalized.lines().collect();
                    let new_lines: Vec<&str> = new_normalized.lines().collect();
                    for (i, (local, new)) in local_lines.iter().zip(new_lines.iter()).enumerate() {
                        if local != new {
                            eprintln!("  Line {}: LOCAL: {}", i + 1, local);
                            eprintln!("  Line {}: NEW:   {}", i + 1, new);
                            break;
                        }
                    }
                    if local_lines.len() != new_lines.len() {
                        eprintln!("  Line count differs: LOCAL={}, NEW={}", local_lines.len(), new_lines.len());
                    }
                }
                if debug && filename_changed {
                    eprintln!("DEBUG: Filename changed for '{}':", event.summary);
                    eprintln!("  LOCAL: {}", local_info.path.display());
                    eprintln!("  NEW:   {}", new_path.display());
                }

                diff.to_update.push(SyncChange {
                    event_id: event.id.clone(),
                    filename: new_filename,
                    summary: event.summary.clone(),
                });
            }
        } else {
            // New event
            diff.to_create.push(SyncChange {
                event_id: event.id.clone(),
                filename: new_filename,
                summary: event.summary.clone(),
            });
        }
    }

    // Find local files that no longer exist in the cloud
    for (uid, local_info) in &existing_files {
        if !seen_uids.contains(uid) {
            let filename = local_info
                .path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();

            diff.to_delete.push(SyncChange {
                event_id: uid.clone(),
                filename,
                summary: local_info.summary.clone(),
            });
        }
    }

    Ok(diff)
}

/// Sync events to a directory
pub fn sync_events_to_dir(
    events: &[Event],
    dir: &Path,
    metadata: &CalendarMetadata,
) -> Result<SyncStats> {
    let mut stats = SyncStats {
        created: 0,
        updated: 0,
        deleted: 0,
    };

    // Build map of existing local files (reuses same logic as compute_sync_diff)
    let existing_files = build_local_file_map(dir)?;
    let mut seen_uids: HashSet<String> = HashSet::new();

    // Process each event from the cloud
    for event in events {
        seen_uids.insert(event.id.clone());

        let new_filename = ics::generate_filename(event);
        let new_path = dir.join(&new_filename);
        let new_content = ics::generate_ics(event, metadata)?;

        if let Some(local_info) = existing_files.get(&event.id) {
            // Event exists locally - check if it changed
            // Use normalized comparison to ignore non-deterministic fields like DTSTAMP
            let local_normalized = normalize_ics_for_comparison(&local_info.content);
            let new_normalized = normalize_ics_for_comparison(&new_content);
            let content_changed = local_normalized.trim() != new_normalized.trim();
            let filename_changed = local_info.path != new_path;

            if content_changed || filename_changed {
                // Remove old file if filename changed
                if filename_changed {
                    std::fs::remove_file(&local_info.path)
                        .with_context(|| format!("Failed to remove {}", local_info.path.display()))?;
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
    for (uid, local_info) in &existing_files {
        if !seen_uids.contains(uid) {
            std::fs::remove_file(&local_info.path)
                .with_context(|| format!("Failed to delete {}", local_info.path.display()))?;

            stats.deleted += 1;
        }
    }

    Ok(stats)
}
