//! ICS file generation.

use crate::error::CalDirResult;
use crate::event::{Event, EventStatus, EventTime, Transparency};
use icalendar::{Alarm, Calendar, Component, EventLike, Property, Trigger, ValueType};

/// Generate .ics content for an event with calendar metadata
pub fn generate_ics(event: &Event) -> CalDirResult<String> {
    let mut cal = Calendar::new();

    let mut ics_event = icalendar::Event::new();
    ics_event.uid(&event.uid);
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

    // Status - only emit if not CONFIRMED (the implied default)
    match event.status {
        EventStatus::Confirmed => {}
        EventStatus::Tentative => {
            ics_event.add_property("STATUS", "TENTATIVE");
        }
        EventStatus::Cancelled => {
            ics_event.add_property("STATUS", "CANCELLED");
        }
    }

    // Recurrence rules (for master events)
    if let Some(ref recurrence) = event.recurrence {
        ics_event.add_property("RRULE", &recurrence.rrule);
        for exdate in &recurrence.exdates {
            add_exdate_property(&mut ics_event, exdate);
        }
    }

    // RECURRENCE-ID (for instance overrides of recurring events)
    if let Some(ref recurrence_id) = event.recurrence_id {
        add_datetime_property(&mut ics_event, "RECURRENCE-ID", recurrence_id);
    }

    // TRANSP - only emit if TRANSPARENT (OPAQUE is the default)
    if event.transparency == Transparency::Transparent {
        ics_event.add_property("TRANSP", "TRANSPARENT");
    }

    // Add alarms (VALARM components) - minimal per RFC 5545
    for reminder in &event.reminders {
        let trigger = Trigger::before_start(chrono::Duration::minutes(reminder.minutes));
        let alarm = Alarm::display("Reminder", trigger);
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

    // Post-process to remove unnecessary bloat from the icalendar crate's output
    let output = strip_ics_bloat(&cal.to_string());

    Ok(output)
}

/// Clean up ICS output from the icalendar crate
/// - Replace PRODID with CALDIR (we post-process the output)
/// - Remove CALSCALE:GREGORIAN (it's the default)
/// - Remove DTSTAMP and UID inside VALARM sections (not required by RFC 5545)
fn strip_ics_bloat(ics: &str) -> String {
    let mut result = String::with_capacity(ics.len());
    let mut in_valarm = false;

    for line in ics.lines() {
        // Replace PRODID with CALDIR
        if line.starts_with("PRODID:") {
            result.push_str("PRODID:CALDIR\r\n");
            continue;
        }

        // Skip CALSCALE:GREGORIAN (it's the default)
        if line == "CALSCALE:GREGORIAN" {
            continue;
        }

        if line == "BEGIN:VALARM" {
            in_valarm = true;
        } else if line == "END:VALARM" {
            in_valarm = false;
        }

        // Skip DTSTAMP and UID lines inside VALARM
        if in_valarm && (line.starts_with("DTSTAMP:") || line.starts_with("UID:")) {
            continue;
        }

        result.push_str(line);
        result.push_str("\r\n");
    }

    result
}

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

/// Add an EXDATE property for a single exception date
fn add_exdate_property(ics_event: &mut icalendar::Event, time: &EventTime) {
    match time {
        EventTime::Date(d) => {
            let mut prop = Property::new("EXDATE", d.format("%Y%m%d").to_string());
            prop.append_parameter(ValueType::Date);
            ics_event.append_multi_property(prop);
        }
        EventTime::DateTimeUtc(dt) => {
            let prop = Property::new("EXDATE", dt.format("%Y%m%dT%H%M%SZ").to_string());
            ics_event.append_multi_property(prop);
        }
        EventTime::DateTimeFloating(dt) => {
            let prop = Property::new("EXDATE", dt.format("%Y%m%dT%H%M%S").to_string());
            ics_event.append_multi_property(prop);
        }
        EventTime::DateTimeZoned { datetime, tzid } => {
            let mut prop =
                Property::new("EXDATE", datetime.format("%Y%m%dT%H%M%S").to_string());
            prop.add_parameter("TZID", tzid);
            ics_event.append_multi_property(prop);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Attendee, EventStatus, ParticipationStatus};
    use chrono::{NaiveDate, TimeZone, Utc};

    fn make_test_event() -> Event {
        Event {
            uid: "test-event-123@caldir".to_string(),
            summary: "Test Event".to_string(),
            description: None,
            location: None,
            start: EventTime::DateTimeUtc(Utc.with_ymd_and_hms(2025, 3, 20, 15, 0, 0).unwrap()),
            end: EventTime::DateTimeUtc(Utc.with_ymd_and_hms(2025, 3, 20, 16, 0, 0).unwrap()),
            status: EventStatus::Confirmed,
            recurrence: None,
            recurrence_id: None,
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

        let ics = generate_ics(&event).unwrap();

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

        let ics = generate_ics(&event).unwrap();

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
    fn test_generate_ics_alarm_is_minimal() {
        use crate::event::Reminder;
        let mut event = make_test_event();
        event.reminders = vec![Reminder { minutes: 30 }];

        let ics = generate_ics(&event).unwrap();
        println!("Generated ICS:\n{}", ics);

        // Should have VALARM
        assert!(ics.contains("BEGIN:VALARM"), "Should have VALARM");
        assert!(ics.contains("ACTION:DISPLAY"), "Should have ACTION:DISPLAY");
        assert!(ics.contains("TRIGGER"), "Should have TRIGGER");
        assert!(
            ics.contains("DESCRIPTION:Reminder"),
            "Should have generic DESCRIPTION:Reminder"
        );
        // Should NOT have UID or DTSTAMP inside VALARM (they're not required)
        let valarm_section: String = ics
            .split("BEGIN:VALARM")
            .nth(1)
            .unwrap()
            .split("END:VALARM")
            .next()
            .unwrap()
            .to_string();
        assert!(
            !valarm_section.contains("UID:"),
            "VALARM should not have UID. Got:\n{}",
            valarm_section
        );
        assert!(
            !valarm_section.contains("DTSTAMP:"),
            "VALARM should not have DTSTAMP. Got:\n{}",
            valarm_section
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

        let ics = generate_ics(&event).unwrap();

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
}
