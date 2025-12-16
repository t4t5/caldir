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

/// A single property change (for verbose output)
#[derive(Debug, Clone)]
pub struct PropertyChange {
    pub property: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
}

/// A single change detected between local and remote
pub struct SyncChange {
    pub event_id: String,
    pub filename: String,
    pub summary: String,
    /// Property-level changes (populated when verbose mode is enabled)
    pub property_changes: Vec<PropertyChange>,
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

/// Properties to skip when computing property-level diffs
const SKIP_PROPERTIES: &[&str] = &[
    "DTSTAMP",
    "BEGIN",
    "END",
    "VERSION",
    "PRODID",
    "CALSCALE",
];

/// Parse ICS content into property key-value pairs (for the VEVENT component)
/// Also extracts alarm triggers as a special "ALARMS" property
fn parse_ics_properties(content: &str) -> HashMap<String, String> {
    let mut props = HashMap::new();
    let mut in_vevent = false;
    let mut in_valarm = false;
    let mut current_line = String::new();
    let mut alarm_triggers: Vec<String> = Vec::new();

    for line in content.lines() {
        // Handle line folding (lines starting with space are continuations)
        if line.starts_with(' ') || line.starts_with('\t') {
            current_line.push_str(line.trim_start());
            continue;
        }

        // Process the completed line
        if !current_line.is_empty() {
            if in_vevent {
                if in_valarm {
                    // Extract TRIGGER from alarms
                    if let Some((key, value)) = parse_property_line(&current_line) {
                        if key == "TRIGGER" {
                            alarm_triggers.push(format_trigger_value(&value));
                        }
                    }
                } else if let Some((key, value)) = parse_property_line(&current_line) {
                    if !SKIP_PROPERTIES.contains(&key.as_str()) {
                        props.insert(key, value);
                    }
                }
            }
        }

        current_line = line.to_string();

        // Track which component we're in
        if line == "BEGIN:VEVENT" {
            in_vevent = true;
        } else if line == "END:VEVENT" {
            in_vevent = false;
        } else if line == "BEGIN:VALARM" {
            in_valarm = true;
        } else if line == "END:VALARM" {
            in_valarm = false;
        }
    }

    // Process last line
    if !current_line.is_empty() && in_vevent && !in_valarm {
        if let Some((key, value)) = parse_property_line(&current_line) {
            if !SKIP_PROPERTIES.contains(&key.as_str()) {
                props.insert(key, value);
            }
        }
    }

    // Add alarms as a combined property if present
    if !alarm_triggers.is_empty() {
        alarm_triggers.sort();
        props.insert("ALARMS".to_string(), alarm_triggers.join(", "));
    }

    props
}

/// Format a TRIGGER value for display (e.g., "-PT86400S" -> "1 day before")
fn format_trigger_value(value: &str) -> String {
    // Parse ISO 8601 duration format: -PT{n}S, -PT{n}M, -PT{n}H, -P{n}D, etc.
    let is_before = value.starts_with('-');
    let duration_part = value.trim_start_matches('-').trim_start_matches('P').trim_start_matches('T');

    // Try to parse common formats
    if let Some(seconds) = duration_part.strip_suffix('S') {
        if let Ok(s) = seconds.parse::<i64>() {
            let minutes = s / 60;
            if minutes >= 60 && minutes % 60 == 0 {
                let hours = minutes / 60;
                if hours >= 24 && hours % 24 == 0 {
                    let days = hours / 24;
                    return format_duration(days, "day", is_before);
                }
                return format_duration(hours, "hour", is_before);
            }
            return format_duration(minutes, "min", is_before);
        }
    }
    if let Some(minutes) = duration_part.strip_suffix('M') {
        if let Ok(m) = minutes.parse::<i64>() {
            if m >= 60 && m % 60 == 0 {
                return format_duration(m / 60, "hour", is_before);
            }
            return format_duration(m, "min", is_before);
        }
    }
    if let Some(hours) = duration_part.strip_suffix('H') {
        if let Ok(h) = hours.parse::<i64>() {
            if h >= 24 && h % 24 == 0 {
                return format_duration(h / 24, "day", is_before);
            }
            return format_duration(h, "hour", is_before);
        }
    }

    // Fallback to raw value
    value.to_string()
}

fn format_duration(value: i64, unit: &str, is_before: bool) -> String {
    let plural = if value == 1 { "" } else { "s" };
    let direction = if is_before { "before" } else { "after" };
    format!("{} {}{} {}", value, unit, plural, direction)
}

/// Parse a single ICS property line into key and value
fn parse_property_line(line: &str) -> Option<(String, String)> {
    // Properties can be "KEY:VALUE" or "KEY;PARAM=X:VALUE"
    let colon_pos = line.find(':')?;
    let key_part = &line[..colon_pos];
    let value = &line[colon_pos + 1..];

    // Extract just the property name (before any parameters)
    let key = key_part.split(';').next()?.to_string();

    Some((key, value.to_string()))
}

/// Human-readable names for ICS properties
fn property_display_name(prop: &str) -> &str {
    match prop {
        "SUMMARY" => "Title",
        "DESCRIPTION" => "Description",
        "LOCATION" => "Location",
        "DTSTART" => "Start",
        "DTEND" => "End",
        "STATUS" => "Status",
        "TRANSP" => "Show as",
        "RRULE" => "Recurrence",
        "EXDATE" => "Excluded dates",
        "RECURRENCE-ID" => "Instance",
        "ORGANIZER" => "Organizer",
        "ATTENDEE" => "Attendee",
        "URL" => "URL",
        "SEQUENCE" => "Version",
        "LAST-MODIFIED" => "Last modified",
        "ALARMS" => "Reminders",
        _ => prop,
    }
}

/// Format a property value for display (truncate long values, format dates)
fn format_property_value(prop: &str, value: &str) -> String {
    // Format datetime values
    if prop == "DTSTART" || prop == "DTEND" || prop == "LAST-MODIFIED" || prop == "RECURRENCE-ID" {
        return format_datetime_value(value);
    }

    // Format transparency
    if prop == "TRANSP" {
        return match value {
            "OPAQUE" => "Busy".to_string(),
            "TRANSPARENT" => "Free".to_string(),
            _ => value.to_string(),
        };
    }

    // Truncate long values
    if value.len() > 60 {
        format!("{}...", &value[..57])
    } else {
        value.to_string()
    }
}

/// Format an ICS datetime value for display
fn format_datetime_value(value: &str) -> String {
    // Handle VALUE=DATE format (all-day): 20250320
    if value.len() == 8 && value.chars().all(|c| c.is_ascii_digit()) {
        if let (Ok(y), Ok(m), Ok(d)) = (
            value[0..4].parse::<i32>(),
            value[4..6].parse::<u32>(),
            value[6..8].parse::<u32>(),
        ) {
            return format!("{}-{:02}-{:02}", y, m, d);
        }
    }

    // Handle datetime format: 20250320T150000Z
    if value.len() >= 15 && value.contains('T') {
        let date_part = &value[0..8];
        let time_part = &value[9..15];
        if let (Ok(y), Ok(mo), Ok(d), Ok(h), Ok(mi)) = (
            date_part[0..4].parse::<i32>(),
            date_part[4..6].parse::<u32>(),
            date_part[6..8].parse::<u32>(),
            time_part[0..2].parse::<u32>(),
            time_part[2..4].parse::<u32>(),
        ) {
            return format!("{}-{:02}-{:02} {:02}:{:02}", y, mo, d, h, mi);
        }
    }

    value.to_string()
}

/// Compute property-level differences between local and new ICS content
fn compute_property_diff(local_content: &str, new_content: &str) -> Vec<PropertyChange> {
    let local_props = parse_ics_properties(local_content);
    let new_props = parse_ics_properties(new_content);

    let mut changes = Vec::new();

    // Find changed and removed properties
    for (key, local_value) in &local_props {
        match new_props.get(key) {
            Some(new_value) if local_value != new_value => {
                changes.push(PropertyChange {
                    property: property_display_name(key).to_string(),
                    old_value: Some(format_property_value(key, local_value)),
                    new_value: Some(format_property_value(key, new_value)),
                });
            }
            None => {
                changes.push(PropertyChange {
                    property: property_display_name(key).to_string(),
                    old_value: Some(format_property_value(key, local_value)),
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
                property: property_display_name(key).to_string(),
                old_value: None,
                new_value: Some(format_property_value(key, new_value)),
            });
        }
    }

    // Sort by property name for consistent output
    changes.sort_by(|a, b| a.property.cmp(&b.property));

    changes
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
    verbose: bool,
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
                // Compute property-level changes if verbose mode is enabled
                let property_changes = if verbose && content_changed {
                    compute_property_diff(&local_info.content, &new_content)
                } else if verbose && filename_changed {
                    // Just filename changed - note that in property changes
                    vec![PropertyChange {
                        property: "Filename".to_string(),
                        old_value: Some(local_info.path.file_name()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_default()),
                        new_value: Some(new_filename.clone()),
                    }]
                } else {
                    Vec::new()
                };

                diff.to_update.push(SyncChange {
                    event_id: event.id.clone(),
                    filename: new_filename,
                    summary: event.summary.clone(),
                    property_changes,
                });
            }
        } else {
            // New event
            diff.to_create.push(SyncChange {
                event_id: event.id.clone(),
                filename: new_filename,
                summary: event.summary.clone(),
                property_changes: Vec::new(),
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
                property_changes: Vec::new(),
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
