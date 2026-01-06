//! Create event files in a calendar directory.

use super::LocalEvent;
use crate::event::{Event, EventTime};
use crate::ics::{self, CalendarMetadata};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use std::path::Path;

/// Create a new event file in the calendar directory.
///
/// Generates the ICS content and a human-readable filename based on the event's
/// date/time and title, handling collisions with numeric suffixes (-2, -3, etc).
///
/// Returns the created LocalEvent.
pub fn create(dir: &Path, event: &Event, metadata: &CalendarMetadata) -> Result<LocalEvent> {
    let content = ics::generate_ics(event, metadata)?;
    let filename = filename_for(event, dir)?;
    let path = dir.join(&filename);

    std::fs::write(&path, &content)
        .with_context(|| format!("Failed to write {}", path.display()))?;

    let modified = std::fs::metadata(&path)
        .ok()
        .and_then(|m| m.modified().ok())
        .map(DateTime::<Utc>::from);

    Ok(LocalEvent {
        path,
        event: event.clone(),
        modified,
    })
}

/// Get the expected filename for an event (for display purposes).
/// This is the base filename without collision suffixes.
pub fn expected_filename(event: &Event) -> String {
    generate_base_filename(event)
}

// =============================================================================
// Internal: Filename generation
// =============================================================================

/// Generate the filename to use for an event in a directory.
/// Handles collisions by adding numeric suffixes (-2, -3, etc).
pub fn filename_for(event: &Event, dir: &Path) -> Result<String> {
    let base_filename = generate_base_filename(event);
    unique_filename(&base_filename, dir, &event.id)
}

/// Generate the base filename for an event (without collision suffix).
fn generate_base_filename(event: &Event) -> String {
    let slug = slugify(&event.summary);

    // Recurring master events (have RRULE) get a special prefix instead of date
    if event.recurrence.is_some() {
        return format!("_recurring__{}.ics", slug);
    }

    // Regular events and instance overrides get date-based filenames
    let date_part = match &event.start {
        EventTime::Date(d) => d.format("%Y-%m-%d").to_string(),
        EventTime::DateTimeUtc(dt) => dt.format("%Y-%m-%dT%H%M").to_string(),
        EventTime::DateTimeFloating(dt) => dt.format("%Y-%m-%dT%H%M").to_string(),
        EventTime::DateTimeZoned { datetime, .. } => datetime.format("%Y-%m-%dT%H%M").to_string(),
    };

    format!("{}__{}.ics", date_part, slug)
}

/// Convert a string to a filename-safe slug
fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
        .chars()
        .take(50)
        .collect()
}

/// Generate a unique filename, adding -2, -3, etc. suffix if there's a collision.
pub fn unique_filename(base_filename: &str, dir: &Path, own_uid: &str) -> Result<String> {
    let base = base_filename.trim_end_matches(".ics");

    // Check if base filename is available
    let base_path = dir.join(base_filename);
    if !base_path.exists() {
        return Ok(base_filename.to_string());
    }

    // File exists - check if it's the same event (same UID)
    if let Ok(content) = std::fs::read_to_string(&base_path) {
        if let Some(event) = ics::parse_event(&content) {
            if event.id == own_uid {
                return Ok(base_filename.to_string());
            }
        }
    }

    // Collision detected - find an available suffix
    for n in 2..=100 {
        let suffixed = format!("{}-{}.ics", base, n);
        let suffixed_path = dir.join(&suffixed);

        if !suffixed_path.exists() {
            return Ok(suffixed);
        }

        // Check if this suffixed file is the same event
        if let Ok(content) = std::fs::read_to_string(&suffixed_path) {
            if let Some(event) = ics::parse_event(&content) {
                if event.id == own_uid {
                    return Ok(suffixed);
                }
            }
        }
    }

    anyhow::bail!("Too many filename collisions for {}", base_filename)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{EventStatus, Transparency};
    use chrono::{NaiveDate, TimeZone, Utc};

    fn make_test_event() -> Event {
        Event {
            id: "test-event-123".to_string(),
            summary: "Test Event".to_string(),
            description: None,
            location: None,
            start: EventTime::DateTimeUtc(Utc.with_ymd_and_hms(2025, 3, 20, 15, 0, 0).unwrap()),
            end: EventTime::DateTimeUtc(Utc.with_ymd_and_hms(2025, 3, 20, 16, 0, 0).unwrap()),
            status: EventStatus::Confirmed,
            recurrence: None,
            original_start: None,
            reminders: vec![],
            transparency: Transparency::Opaque,
            organizer: None,
            attendees: vec![],
            conference_url: None,
            updated: None,
            sequence: None,
            custom_properties: vec![],
        }
    }

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Team Standup"), "team-standup");
        assert_eq!(slugify("Meeting: Q4 Review!"), "meeting-q4-review");
        assert_eq!(slugify("  Lots   of   spaces  "), "lots-of-spaces");
        assert_eq!(slugify("Special@#$%Characters"), "special-characters");
    }

    #[test]
    fn test_slugify_truncates_long_titles() {
        let long_title = "a".repeat(100);
        assert_eq!(slugify(&long_title).len(), 50);
    }

    #[test]
    fn test_generate_base_filename() {
        let event = make_test_event();
        assert_eq!(
            generate_base_filename(&event),
            "2025-03-20T1500__test-event.ics"
        );
    }

    #[test]
    fn test_generate_base_filename_all_day() {
        let mut event = make_test_event();
        event.start = EventTime::Date(NaiveDate::from_ymd_opt(2025, 3, 20).unwrap());
        assert_eq!(generate_base_filename(&event), "2025-03-20__test-event.ics");
    }

    #[test]
    fn test_generate_base_filename_recurring() {
        let mut event = make_test_event();
        event.recurrence = Some(vec!["RRULE:FREQ=WEEKLY".to_string()]);
        assert_eq!(generate_base_filename(&event), "_recurring__test-event.ics");
    }
}
