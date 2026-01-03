//! Diff computation between local and remote calendar state.
//!
//! This module computes what's different between two sets of events
//! without applying any changes. Used by status, pull, and push commands.

use crate::caldir::LocalEvent;
use crate::event::{Event, EventTime};
use crate::ics;
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

/// Check if two Events have meaningful differences.
fn events_differ(local: &Event, remote: &Event) -> bool {
    // Compare key fields that matter for sync
    local.summary != remote.summary
        || local.description != remote.description
        || local.location != remote.location
        || !event_times_equal(&local.start, &remote.start)
        || !event_times_equal(&local.end, &remote.end)
        || local.status != remote.status
        || local.recurrence != remote.recurrence
        || local.reminders != remote.reminders
        || local.transparency != remote.transparency
        || local.organizer != remote.organizer
        || local.attendees != remote.attendees
        || local.conference_url != remote.conference_url
}

/// Compare EventTime values for equality
fn event_times_equal(a: &EventTime, b: &EventTime) -> bool {
    match (a, b) {
        (EventTime::DateTime(dt1), EventTime::DateTime(dt2)) => dt1 == dt2,
        (EventTime::Date(d1), EventTime::Date(d2)) => d1 == d2,
        _ => false,
    }
}

/// Format an EventTime for display
fn format_event_time(time: &EventTime) -> String {
    match time {
        EventTime::DateTime(dt) => dt.format("%Y-%m-%d %H:%M").to_string(),
        EventTime::Date(d) => d.format("%Y-%m-%d").to_string(),
    }
}

/// Compute property-level differences between two Events
fn compute_event_diff(old: &Event, new: &Event) -> Vec<PropertyChange> {
    let mut changes = Vec::new();

    // Compare each field
    if old.summary != new.summary {
        changes.push(PropertyChange {
            property: "Summary".to_string(),
            old_value: Some(old.summary.clone()),
            new_value: Some(new.summary.clone()),
        });
    }

    if old.description != new.description {
        changes.push(PropertyChange {
            property: "Description".to_string(),
            old_value: old.description.clone(),
            new_value: new.description.clone(),
        });
    }

    if old.location != new.location {
        changes.push(PropertyChange {
            property: "Location".to_string(),
            old_value: old.location.clone(),
            new_value: new.location.clone(),
        });
    }

    if !event_times_equal(&old.start, &new.start) {
        changes.push(PropertyChange {
            property: "Start".to_string(),
            old_value: Some(format_event_time(&old.start)),
            new_value: Some(format_event_time(&new.start)),
        });
    }

    if !event_times_equal(&old.end, &new.end) {
        changes.push(PropertyChange {
            property: "End".to_string(),
            old_value: Some(format_event_time(&old.end)),
            new_value: Some(format_event_time(&new.end)),
        });
    }

    if old.status != new.status {
        changes.push(PropertyChange {
            property: "Status".to_string(),
            old_value: Some(format!("{:?}", old.status)),
            new_value: Some(format!("{:?}", new.status)),
        });
    }

    if old.recurrence != new.recurrence {
        changes.push(PropertyChange {
            property: "Recurrence".to_string(),
            old_value: old.recurrence.as_ref().map(|r| r.join(", ")),
            new_value: new.recurrence.as_ref().map(|r| r.join(", ")),
        });
    }

    if old.reminders != new.reminders {
        let format_reminders = |reminders: &[crate::event::Reminder]| -> String {
            reminders
                .iter()
                .map(|r| format!("{}m before", r.minutes))
                .collect::<Vec<_>>()
                .join(", ")
        };
        changes.push(PropertyChange {
            property: "Reminders".to_string(),
            old_value: Some(format_reminders(&old.reminders)),
            new_value: Some(format_reminders(&new.reminders)),
        });
    }

    if old.transparency != new.transparency {
        changes.push(PropertyChange {
            property: "Availability".to_string(),
            old_value: Some(format!("{:?}", old.transparency)),
            new_value: Some(format!("{:?}", new.transparency)),
        });
    }

    if old.organizer != new.organizer {
        let format_attendee = |a: &Option<crate::event::Attendee>| -> Option<String> {
            a.as_ref().map(|att| {
                att.name
                    .clone()
                    .unwrap_or_else(|| att.email.clone())
            })
        };
        changes.push(PropertyChange {
            property: "Organizer".to_string(),
            old_value: format_attendee(&old.organizer),
            new_value: format_attendee(&new.organizer),
        });
    }

    if old.attendees != new.attendees {
        let format_attendees = |attendees: &[crate::event::Attendee]| -> String {
            attendees
                .iter()
                .map(|a| a.name.clone().unwrap_or_else(|| a.email.clone()))
                .collect::<Vec<_>>()
                .join(", ")
        };
        changes.push(PropertyChange {
            property: "Attendees".to_string(),
            old_value: Some(format_attendees(&old.attendees)),
            new_value: Some(format_attendees(&new.attendees)),
        });
    }

    if old.conference_url != new.conference_url {
        changes.push(PropertyChange {
            property: "Conference URL".to_string(),
            old_value: old.conference_url.clone(),
            new_value: new.conference_url.clone(),
        });
    }

    // Sort by property name for consistent output
    changes.sort_by(|a, b| a.property.cmp(&b.property));

    changes
}

/// Get the start time as UTC DateTime for time range checking
fn event_start_utc(event: &Event) -> Option<DateTime<Utc>> {
    match &event.start {
        EventTime::DateTime(dt) => Some(*dt),
        EventTime::Date(d) => d.and_hms_opt(0, 0, 0).map(|dt| dt.and_utc()),
    }
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
    for remote_event in remote_events {
        seen_uids.insert(remote_event.id.clone());

        let new_filename = ics::generate_filename(remote_event);
        let new_path = dir.join(&new_filename);

        if let Some(local) = local_events.get(&remote_event.id) {
            // Event exists locally - check if it changed
            let content_changed = events_differ(&local.event, remote_event);
            let filename_changed = local.path != new_path;

            if content_changed || filename_changed {
                // Determine direction based on timestamps
                let is_push = match (local.modified, remote_event.updated) {
                    (Some(local_mtime), Some(remote_updated)) => local_mtime > remote_updated,
                    // If we can't compare timestamps, default to pull (safer)
                    _ => false,
                };

                // Compute property-level changes if verbose mode is enabled
                // For pull: local (old) → remote (new)
                // For push: remote (old) → local (new)
                let property_changes = if verbose && content_changed {
                    if is_push {
                        compute_event_diff(remote_event, &local.event)
                    } else {
                        compute_event_diff(&local.event, remote_event)
                    }
                } else if verbose && filename_changed {
                    let (old_name, new_name) = if is_push {
                        (
                            new_filename.clone(),
                            local
                                .path
                                .file_name()
                                .map(|s| s.to_string_lossy().to_string())
                                .unwrap_or_default(),
                        )
                    } else {
                        (
                            local
                                .path
                                .file_name()
                                .map(|s| s.to_string_lossy().to_string())
                                .unwrap_or_default(),
                            new_filename.clone(),
                        )
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
                    uid: remote_event.id.clone(),
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
            if synced_uids.contains(&remote_event.id) {
                // User deleted this locally → push delete
                diff.to_push_delete.push(SyncChange {
                    uid: remote_event.id.clone(),
                    filename: new_filename,
                    property_changes: Vec::new(),
                });
            } else {
                // New remote event - pull it
                diff.to_pull_create.push(SyncChange {
                    uid: remote_event.id.clone(),
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

            // Distinguish between locally-created events and remotely-deleted events
            // using the sync state:
            // - If UID was never synced → locally created → push candidate
            // - If UID was synced before → remote deleted it → pull delete candidate
            let was_synced = synced_uids.contains(uid);

            if !was_synced {
                diff.to_push_create.push(SyncChange {
                    uid: uid.clone(),
                    filename,
                    property_changes: Vec::new(),
                });
            } else {
                // Only consider deletion if the event falls within the queried time range.
                // Events outside the range weren't fetched, so we can't know if they
                // still exist on the remote.
                let in_range = match (time_range, event_start_utc(&local.event)) {
                    (Some((time_min, time_max)), Some(event_start)) => {
                        event_start >= time_min && event_start <= time_max
                    }
                    // No time range specified, or couldn't get date - assume in range
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
