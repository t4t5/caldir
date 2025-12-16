//! Diff computation between local and remote calendar state.
//!
//! This module computes what's different between two sets of events
//! without applying any changes. Used by status, pull, and push commands.

use crate::caldir::LocalEvent;
use crate::ics::{self, CalendarMetadata};
use crate::providers::gcal::Event;
use anyhow::Result;
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
    pub filename: String,
    /// Property-level changes (populated when verbose mode is enabled)
    pub property_changes: Vec<PropertyChange>,
}

/// Result of comparing local directory against remote state
pub struct SyncDiff {
    pub to_create: Vec<SyncChange>,
    pub to_update: Vec<SyncChange>,
    pub to_delete: Vec<SyncChange>,
}

/// Normalize ICS content for comparison by removing non-deterministic fields.
/// This ensures we only detect real content changes, not timestamp variations.
fn normalize_for_comparison(content: &str) -> String {
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
/// Returns a SyncDiff describing what needs to be created, updated, or deleted
/// locally to match the remote state.
pub fn compute(
    remote_events: &[Event],
    local_events: &HashMap<String, LocalEvent>,
    dir: &Path,
    metadata: &CalendarMetadata,
    verbose: bool,
) -> Result<SyncDiff> {
    let mut diff = SyncDiff {
        to_create: Vec::new(),
        to_update: Vec::new(),
        to_delete: Vec::new(),
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
            let local_normalized = normalize_for_comparison(&local.content);
            let new_normalized = normalize_for_comparison(&new_content);
            let content_changed = local_normalized.trim() != new_normalized.trim();
            let filename_changed = local.path != new_path;

            if content_changed || filename_changed {
                // Compute property-level changes if verbose mode is enabled
                let property_changes = if verbose && content_changed {
                    compute_property_diff(&local.content, &new_content)
                } else if verbose && filename_changed {
                    vec![PropertyChange {
                        property: "Filename".to_string(),
                        old_value: Some(
                            local
                                .path
                                .file_name()
                                .map(|s| s.to_string_lossy().to_string())
                                .unwrap_or_default(),
                        ),
                        new_value: Some(new_filename.clone()),
                    }]
                } else {
                    Vec::new()
                };

                diff.to_update.push(SyncChange {
                    filename: new_filename,
                    property_changes,
                });
            }
        } else {
            // New event
            diff.to_create.push(SyncChange {
                filename: new_filename,
                property_changes: Vec::new(),
            });
        }
    }

    // Find local files that no longer exist in the remote
    for (uid, local) in local_events {
        if !seen_uids.contains(uid) {
            let filename = local
                .path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();

            diff.to_delete.push(SyncChange {
                filename,
                property_changes: Vec::new(),
            });
        }
    }

    Ok(diff)
}
