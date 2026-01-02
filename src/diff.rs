//! Diff computation between local and remote calendar state.
//!
//! This module computes what's different between two sets of events
//! without applying any changes. Used by status, pull, and push commands.

use crate::caldir::LocalEvent;
use crate::ics::{self, CalendarMetadata};
use crate::event::Event;
use anyhow::Result;
use chrono::{DateTime, Utc};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// A single property change (for verbose output)
#[derive(Debug, Clone)]
pub struct PropertyChange {
    pub property: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
}

/// A single change detected between local and remote
pub struct SyncChange {
    pub uid: String,
    pub filename: String,
    /// Property-level changes (populated when verbose mode is enabled)
    pub property_changes: Vec<PropertyChange>,
}

/// Result of comparing local directory against remote state
pub struct SyncDiff {
    // Pull changes (remote → local)
    pub to_pull_create: Vec<SyncChange>,
    pub to_pull_update: Vec<SyncChange>,
    pub to_pull_delete: Vec<SyncChange>,

    // Push changes (local → remote)
    pub to_push_create: Vec<SyncChange>,
    pub to_push_update: Vec<SyncChange>,
    pub to_push_delete: Vec<SyncChange>,
}

/// Check if two ICS contents have meaningful property differences.
/// Uses the same property parsing as compute_property_diff to ensure consistency.
fn has_property_changes(local_content: &str, new_content: &str) -> bool {
    let local_props = ics::parse_properties(local_content);
    let new_props = ics::parse_properties(new_content);

    // Check for any difference in properties
    if local_props.len() != new_props.len() {
        return true;
    }

    for (key, local_value) in &local_props {
        match new_props.get(key) {
            Some(new_value) if local_value != new_value => return true,
            None => return true,
            _ => {}
        }
    }

    // Check for properties in new that aren't in local
    for key in new_props.keys() {
        if !local_props.contains_key(key) {
            return true;
        }
    }

    false
}

/// Compute property-level differences between local and new ICS content
fn compute_property_diff(local_content: &str, new_content: &str) -> Vec<PropertyChange> {
    let local_props = ics::parse_properties(local_content);
    let new_props = ics::parse_properties(new_content);

    let mut changes = Vec::new();

    // Find changed and removed properties
    for (key, local_value) in &local_props {
        match new_props.get(key) {
            Some(new_value) if local_value != new_value => {
                changes.push(PropertyChange {
                    property: ics::property_display_name(key).to_string(),
                    old_value: Some(ics::format_property_value(key, local_value)),
                    new_value: Some(ics::format_property_value(key, new_value)),
                });
            }
            None => {
                changes.push(PropertyChange {
                    property: ics::property_display_name(key).to_string(),
                    old_value: Some(ics::format_property_value(key, local_value)),
                    new_value: None,
                });
            }
            _ => {}
        }
    }

    // Find added properties
    for (key, new_value) in &new_props {
        if !local_props.contains_key(key) {
            changes.push(PropertyChange {
                property: ics::property_display_name(key).to_string(),
                old_value: None,
                new_value: Some(ics::format_property_value(key, new_value)),
            });
        }
    }

    // Sort by property name for consistent output
    changes.sort_by(|a, b| a.property.cmp(&b.property));

    changes
}

/// Compute the diff between remote events and local directory.
///
/// Returns a SyncDiff describing:
/// - Pull changes: what needs to be created/updated/deleted locally to match remote
/// - Push changes: what needs to be created/updated/deleted on remote to match local
///
/// Uses timestamp comparison: if local file mtime > remote updated, it's a push candidate.
///
/// The `time_range` parameter specifies the window of events that were queried from
/// the remote. Events outside this range won't be flagged for deletion since we
/// don't know their remote status.
///
/// The `synced_uids` parameter contains UIDs that have been previously synced.
/// If a UID is in synced_uids but has no local file, it means the user deleted
/// it locally → push delete candidate.
pub fn compute(
    remote_events: &[Event],
    local_events: &HashMap<String, LocalEvent>,
    dir: &Path,
    metadata: &CalendarMetadata,
    verbose: bool,
    time_range: Option<(DateTime<Utc>, DateTime<Utc>)>,
    synced_uids: &HashSet<String>,
) -> Result<SyncDiff> {
    let mut diff = SyncDiff {
        to_pull_create: Vec::new(),
        to_pull_update: Vec::new(),
        to_pull_delete: Vec::new(),
        to_push_create: Vec::new(),
        to_push_update: Vec::new(),
        to_push_delete: Vec::new(),
    };

    let mut seen_uids: HashSet<String> = HashSet::new();

    // Process each event from the remote
    for event in remote_events {
        seen_uids.insert(event.id.clone());

        let new_filename = ics::generate_filename(event);
        let new_path = dir.join(&new_filename);
        let new_content = ics::generate_ics(event, metadata)?;

        if let Some(local) = local_events.get(&event.id) {
            // Event exists locally - check if it changed
            let content_changed = has_property_changes(&local.content, &new_content);
            let filename_changed = local.path != new_path;

            if content_changed || filename_changed {
                // Determine direction based on timestamps
                let is_push = match (local.modified, event.updated) {
                    (Some(local_mtime), Some(remote_updated)) => local_mtime > remote_updated,
                    // If we can't compare timestamps, default to pull (safer)
                    _ => false,
                };

                // Compute property-level changes if verbose mode is enabled
                // For pull: local (old) → remote (new)
                // For push: remote (old) → local (new)
                let property_changes = if verbose && content_changed {
                    if is_push {
                        compute_property_diff(&new_content, &local.content)
                    } else {
                        compute_property_diff(&local.content, &new_content)
                    }
                } else if verbose && filename_changed {
                    let (old_name, new_name) = if is_push {
                        (new_filename.clone(), local.path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default())
                    } else {
                        (local.path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(), new_filename.clone())
                    };
                    vec![PropertyChange {
                        property: "Filename".to_string(),
                        old_value: Some(old_name),
                        new_value: Some(new_name),
                    }]
                } else {
                    Vec::new()
                };

                let change = SyncChange {
                    uid: event.id.clone(),
                    filename: new_filename,
                    property_changes,
                };

                if is_push {
                    diff.to_push_update.push(change);
                } else {
                    diff.to_pull_update.push(change);
                }
            }
        } else {
            // Check if this remote event was previously synced but deleted locally
            if synced_uids.contains(&event.id) {
                // User deleted this locally → push delete
                diff.to_push_delete.push(SyncChange {
                    uid: event.id.clone(),
                    filename: new_filename,
                    property_changes: Vec::new(),
                });
            } else {
                // New remote event - pull it
                diff.to_pull_create.push(SyncChange {
                    uid: event.id.clone(),
                    filename: new_filename,
                    property_changes: Vec::new(),
                });
            }
        }
    }

    // Find local files that don't exist in the remote
    for (uid, local) in local_events {
        if !seen_uids.contains(uid) {
            let filename = local
                .path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();

            // Distinguish between locally-created events and remotely-deleted events:
            // - Events with X-CALDIR-ORIGIN:local were created locally → push candidate
            // - Other events came from remote and are now missing → delete candidate
            let is_local_origin = local.content.contains("X-CALDIR-ORIGIN:local");

            if is_local_origin {
                diff.to_push_create.push(SyncChange {
                    uid: uid.clone(),
                    filename,
                    property_changes: Vec::new(),
                });
            } else {
                // Only consider deletion if the event falls within the queried time range.
                // Events outside the range weren't fetched, so we can't know if they
                // still exist on the remote.
                let in_range = match (time_range, ics::parse_dtstart_utc(&local.content)) {
                    (Some((time_min, time_max)), Some(event_start)) => {
                        event_start >= time_min && event_start <= time_max
                    }
                    // No time range specified, or couldn't parse date - assume in range
                    _ => true,
                };

                if in_range {
                    diff.to_pull_delete.push(SyncChange {
                        uid: uid.clone(),
                        filename,
                        property_changes: Vec::new(),
                    });
                }
            }
        }
    }

    Ok(diff)
}
