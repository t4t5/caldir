//! ICS file generation.

use super::CalendarMetadata;
use crate::event::{Event, EventTime, Transparency};
use anyhow::Result;
use icalendar::{Alarm, Calendar, Component, EventLike, Property, Trigger, ValueType};

/// Add a datetime property with proper formatting based on EventTime variant
fn add_datetime_property(ics_event: &mut icalendar::Event, name: &str, time: &EventTime) {
    match time {
        EventTime::Date(d) => {
            let mut prop = Property::new(name, d.format("%Y%m%d").to_string());
            prop.append_parameter(ValueType::Date);
            ics_event.append_property(prop);
        }
        EventTime::DateTimeUtc(dt) => {
            // UTC datetime with Z suffix
            ics_event.add_property(name, dt.format("%Y%m%dT%H%M%SZ").to_string());
        }
        EventTime::DateTimeFloating(dt) => {
            // Floating datetime (no Z, no TZID)
            ics_event.add_property(name, dt.format("%Y%m%dT%H%M%S").to_string());
        }
        EventTime::DateTimeZoned { datetime, tzid } => {
            // Datetime with TZID parameter
            let mut prop = Property::new(name, datetime.format("%Y%m%dT%H%M%S").to_string());
            prop.add_parameter("TZID", tzid);
            ics_event.append_property(prop);
        }
    }
}

/// Generate .ics content for an event with calendar metadata
pub fn generate_ics(event: &Event, metadata: &CalendarMetadata) -> Result<String> {
    let mut cal = Calendar::new();

    // Add calendar-level metadata properties
    // X-WR-CALNAME - Human-readable calendar name (de facto standard)
    cal.append_property(Property::new("X-WR-CALNAME", &metadata.calendar_name));

    // X-WR-RELCALID - Calendar identifier (de facto standard)
    cal.append_property(Property::new("X-WR-RELCALID", &metadata.calendar_id));

    let mut ics_event = icalendar::Event::new();
    ics_event.uid(&event.id);
    ics_event.summary(&event.summary);

    // DTSTAMP - required by RFC 5545, use updated timestamp or current time
    // Note: comparison logic filters this out since it's non-deterministic
    let dtstamp = event
        .updated
        .unwrap_or_else(chrono::Utc::now)
        .format("%Y%m%dT%H%M%SZ")
        .to_string();
    ics_event.add_property("DTSTAMP", &dtstamp);

    // LAST-MODIFIED
    if let Some(updated) = event.updated {
        let last_modified = updated.format("%Y%m%dT%H%M%SZ").to_string();
        ics_event.add_property("LAST-MODIFIED", &last_modified);
    }

    // SEQUENCE
    if let Some(seq) = event.sequence {
        ics_event.add_property("SEQUENCE", seq.to_string());
    }

    // Set start/end times
    add_datetime_property(&mut ics_event, "DTSTART", &event.start);
    add_datetime_property(&mut ics_event, "DTEND", &event.end);

    // Optional fields
    if let Some(ref desc) = event.description {
        ics_event.description(desc);
    }

    if let Some(ref loc) = event.location {
        ics_event.location(loc);
    }

    // Status
    let status = match event.status {
        crate::event::EventStatus::Confirmed => "CONFIRMED",
        crate::event::EventStatus::Tentative => "TENTATIVE",
        crate::event::EventStatus::Cancelled => "CANCELLED",
    };
    ics_event.add_property("STATUS", status);

    // Recurrence rules (for master events)
    if let Some(ref recurrence) = event.recurrence {
        for rule in recurrence {
            // Each rule is like "RRULE:FREQ=WEEKLY;BYDAY=MO" or "EXDATE:20250320"
            if let Some((key, value)) = rule.split_once(':') {
                ics_event.add_property(key, value);
            }
        }
    }

    // RECURRENCE-ID (for instance overrides of recurring events)
    if let Some(ref original_start) = event.original_start {
        add_datetime_property(&mut ics_event, "RECURRENCE-ID", original_start);
    }

    // TRANSP (transparency/busy-free status)
    let transp = match event.transparency {
        Transparency::Opaque => "OPAQUE",
        Transparency::Transparent => "TRANSPARENT",
    };
    ics_event.add_property("TRANSP", transp);

    // Add alarms (VALARM components)
    // We set deterministic UIDs and DSTAMPs to avoid random generation by the icalendar crate
    for reminder in &event.reminders {
        let trigger = Trigger::before_start(chrono::Duration::minutes(reminder.minutes));
        let mut alarm = Alarm::display(&event.summary, trigger);
        // Deterministic alarm UID from event ID and reminder minutes
        let alarm_uid = format!("{}_alarm_{}", event.id, reminder.minutes);
        alarm.add_property("UID", &alarm_uid);
        // Use same DTSTAMP as the event (or a fixed timestamp) for determinism
        alarm.add_property("DTSTAMP", &dtstamp);
        ics_event.alarm(alarm);
    }

    // ORGANIZER
    if let Some(ref org) = event.organizer {
        let mut prop = Property::new("ORGANIZER", format!("mailto:{}", org.email));
        if let Some(ref name) = org.name {
            prop.add_parameter("CN", name);
        }
        ics_event.append_property(prop);
    }

    // ATTENDEE (multi-property - can appear multiple times)
    for attendee in &event.attendees {
        let mut prop = Property::new("ATTENDEE", format!("mailto:{}", attendee.email));
        if let Some(ref name) = attendee.name {
            prop.add_parameter("CN", name);
        }
        if let Some(partstat) = attendee.response_status {
            prop.add_parameter("PARTSTAT", partstat.as_ics_str());
        }
        ics_event.append_multi_property(prop);
    }

    // Conference URL
    if let Some(ref url) = event.conference_url {
        ics_event.add_property("URL", url);
    }

    // Custom properties (provider-specific, preserved for round-tripping)
    for (key, value) in &event.custom_properties {
        ics_event.add_property(key, value);
    }

    let ics_event = ics_event.done();
    cal.push(ics_event);
    let cal = cal.done();

    Ok(cal.to_string())
}

/// Generate the caldir filename for an event
pub fn generate_filename(event: &Event) -> String {
    let slug = slugify(&event.summary);

    // Recurring master events (have RRULE) get a special prefix instead of date
    if event.recurrence.is_some() {
        return format!("_recurring__{}_{}.ics", slug, short_id(&event.id));
    }

    // Regular events and instance overrides get date-based filenames
    let date_part = match &event.start {
        EventTime::Date(d) => {
            // Format: 2025-03-20
            d.format("%Y-%m-%d").to_string()
        }
        EventTime::DateTimeUtc(dt) => {
            // Format: 2025-03-20T1500
            dt.format("%Y-%m-%dT%H%M").to_string()
        }
        EventTime::DateTimeFloating(dt) => {
            // Format: 2025-03-20T1500
            dt.format("%Y-%m-%dT%H%M").to_string()
        }
        EventTime::DateTimeZoned { datetime, .. } => {
            // Format: 2025-03-20T1500 (use local time for filename)
            datetime.format("%Y-%m-%dT%H%M").to_string()
        }
    };

    format!("{}__{}_{}.ics", date_part, slug, short_id(&event.id))
}

/// Convert a string to a filename-safe slug
pub fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
        .chars()
        .take(50) // Limit slug length
        .collect()
}

/// Get a short version of the event ID for uniqueness
fn short_id(id: &str) -> String {
    // Use a hash to ensure uniqueness regardless of where the differentiating
    // characters are in the ID (Google recurring instance IDs share prefixes
    // but differ in suffixes like _R20240920T153000)
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    id.hash(&mut hasher);
    let hash = hasher.finish();

    // Format as 8-char hex string
    format!("{:08x}", hash as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Attendee, EventStatus, ParticipationStatus};
    use crate::ics::CalendarMetadata;
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

    fn make_test_metadata() -> CalendarMetadata {
        CalendarMetadata {
            calendar_id: "test@example.com".to_string(),
            calendar_name: "Test Calendar".to_string(),
        }
    }

    #[test]
    fn test_generate_ics_multiple_attendees() {
        let mut event = make_test_event();
        event.attendees = vec![
            Attendee {
                name: Some("Alice".to_string()),
                email: "alice@example.com".to_string(),
                response_status: Some(ParticipationStatus::Accepted),
            },
            Attendee {
                name: Some("Bob".to_string()),
                email: "bob@example.com".to_string(),
                response_status: Some(ParticipationStatus::Tentative),
            },
            Attendee {
                name: None,
                email: "charlie@example.com".to_string(),
                response_status: None,
            },
        ];

        let ics = generate_ics(&event, &make_test_metadata()).unwrap();

        // Count ATTENDEE lines - should be 3
        let attendee_count = ics.lines().filter(|l| l.starts_with("ATTENDEE")).count();
        assert_eq!(
            attendee_count, 3,
            "Should have 3 ATTENDEE lines, got {}. ICS:\n{}",
            attendee_count, ics
        );

        // Verify each attendee is present
        assert!(ics.contains("alice@example.com"), "Missing Alice");
        assert!(ics.contains("bob@example.com"), "Missing Bob");
        assert!(ics.contains("charlie@example.com"), "Missing Charlie");
    }

    #[test]
    fn test_generate_ics_all_day_event_has_value_date() {
        let mut event = make_test_event();
        event.start = EventTime::Date(NaiveDate::from_ymd_opt(2025, 3, 20).unwrap());
        event.end = EventTime::Date(NaiveDate::from_ymd_opt(2025, 3, 21).unwrap());

        let ics = generate_ics(&event, &make_test_metadata()).unwrap();

        // Should have VALUE=DATE parameter
        assert!(
            ics.contains("DTSTART;VALUE=DATE:20250320"),
            "DTSTART should have VALUE=DATE parameter. ICS:\n{}",
            ics
        );
        assert!(
            ics.contains("DTEND;VALUE=DATE:20250321"),
            "DTEND should have VALUE=DATE parameter. ICS:\n{}",
            ics
        );
    }

    #[test]
    fn test_generate_ics_organizer_has_proper_parameters() {
        let mut event = make_test_event();
        event.organizer = Some(Attendee {
            name: Some("Organizer Name".to_string()),
            email: "organizer@example.com".to_string(),
            response_status: None,
        });

        let ics = generate_ics(&event, &make_test_metadata()).unwrap();

        // Find the ORGANIZER line
        let organizer_line = ics
            .lines()
            .find(|l| l.starts_with("ORGANIZER"))
            .expect("Should have ORGANIZER line");

        // Should have CN as a parameter (semicolon-separated), not in value
        assert!(
            organizer_line.contains(";CN="),
            "CN should be a parameter (;CN=), not part of value. Got: {}",
            organizer_line
        );
        assert!(
            organizer_line.contains("mailto:organizer@example.com"),
            "Should have mailto value. Got: {}",
            organizer_line
        );
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
    fn test_short_id() {
        // short_id returns an 8-char hex hash of the input
        assert_eq!(short_id("abc12345xyz").len(), 8);
        assert_eq!(short_id("short").len(), 8);
        assert_eq!(short_id("").len(), 8);

        // Same input should produce same hash
        assert_eq!(short_id("test-id"), short_id("test-id"));

        // Different inputs should produce different hashes
        assert_ne!(short_id("event-1"), short_id("event-2"));
    }
}
