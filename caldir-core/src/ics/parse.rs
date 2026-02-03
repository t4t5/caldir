//! ICS file parsing using the icalendar crate's parser.

use crate::event::{
    Attendee, Event, EventStatus, EventTime, ParticipationStatus, Reminder, Transparency,
};
use icalendar::{
    DatePerhapsTime,
    parser::{Property, read_calendar, unfold},
};

/// Parse ICS content into an Event struct
pub fn parse_event(content: &str) -> Option<Event> {
    let unfolded = unfold(content);
    let calendar = read_calendar(&unfolded).ok()?;
    let vevent = calendar.components.iter().find(|c| c.name == "VEVENT")?;

    // Required fields
    let uid = vevent.find_prop("UID")?.val.to_string();
    let summary = vevent
        .find_prop("SUMMARY")
        .map(|p| p.val.to_string())
        .unwrap_or_else(|| "(No title)".to_string());
    let start = to_event_time(DatePerhapsTime::try_from(vevent.find_prop("DTSTART")?).ok()?);
    let end = to_event_time(DatePerhapsTime::try_from(vevent.find_prop("DTEND")?).ok()?);

    // Optional simple fields
    let description = vevent.find_prop("DESCRIPTION").map(|p| p.val.to_string());
    let location = vevent.find_prop("LOCATION").map(|p| p.val.to_string());
    let conference_url = vevent.find_prop("URL").map(|p| p.val.to_string());
    let sequence = vevent
        .find_prop("SEQUENCE")
        .and_then(|p| p.val.as_ref().parse().ok());

    let status = vevent
        .find_prop("STATUS")
        .map(|p| match p.val.as_ref() {
            "TENTATIVE" => EventStatus::Tentative,
            "CANCELLED" => EventStatus::Cancelled,
            _ => EventStatus::Confirmed,
        })
        .unwrap_or(EventStatus::Confirmed);

    let transparency = vevent
        .find_prop("TRANSP")
        .map(|p| {
            if p.val == "TRANSPARENT" {
                Transparency::Transparent
            } else {
                Transparency::Opaque
            }
        })
        .unwrap_or(Transparency::Opaque);

    // Recurrence (RRULE, EXDATE)
    let recurrence: Vec<String> = vevent
        .properties
        .iter()
        .filter(|p| p.name == "RRULE" || p.name == "EXDATE")
        .map(format_property)
        .collect();
    let recurrence = if recurrence.is_empty() {
        None
    } else {
        Some(recurrence)
    };

    // RECURRENCE-ID for instance overrides
    let original_start = vevent
        .find_prop("RECURRENCE-ID")
        .and_then(|p| DatePerhapsTime::try_from(p).ok())
        .map(to_event_time);

    // Attendees
    let organizer = vevent.find_prop("ORGANIZER").map(parse_attendee);
    let attendees: Vec<Attendee> = vevent
        .properties
        .iter()
        .filter(|p| p.name == "ATTENDEE")
        .map(parse_attendee)
        .collect();

    // Reminders from VALARM components
    let reminders: Vec<Reminder> = vevent
        .components
        .iter()
        .filter(|c| c.name == "VALARM")
        .filter_map(|alarm| {
            let trigger = alarm.find_prop("TRIGGER")?.val.as_ref();
            let minutes = parse_trigger_minutes(trigger)?;
            Some(Reminder { minutes })
        })
        .collect();

    // Custom X- properties
    let custom_properties: Vec<(String, String)> = vevent
        .properties
        .iter()
        .filter(|p| p.name.as_ref().starts_with("X-"))
        .map(|p| (p.name.to_string(), p.val.to_string()))
        .collect();

    Some(Event {
        id: uid,
        summary,
        description,
        location,
        start,
        end,
        status,
        recurrence,
        original_start,
        reminders,
        transparency,
        organizer,
        attendees,
        conference_url,
        updated: None,
        sequence,
        custom_properties,
    })
}

/// Convert icalendar's DatePerhapsTime to our EventTime, preserving timezone info
fn to_event_time(dpt: DatePerhapsTime) -> EventTime {
    match dpt {
        DatePerhapsTime::Date(d) => EventTime::Date(d),
        DatePerhapsTime::DateTime(cal_dt) => match cal_dt {
            icalendar::CalendarDateTime::Utc(dt) => EventTime::DateTimeUtc(dt),
            icalendar::CalendarDateTime::Floating(naive) => EventTime::DateTimeFloating(naive),
            icalendar::CalendarDateTime::WithTimezone { date_time, tzid } => {
                EventTime::DateTimeZoned {
                    datetime: date_time,
                    tzid,
                }
            }
        },
    }
}

/// Format a property back to ICS format with params (e.g., "EXDATE;TZID=America/New_York:...")
fn format_property(prop: &Property) -> String {
    if prop.params.is_empty() {
        format!("{}:{}", prop.name, prop.val)
    } else {
        let params: Vec<String> = prop
            .params
            .iter()
            .map(|p| match &p.val {
                Some(v) => format!("{}={}", p.key, v),
                None => p.key.to_string(),
            })
            .collect();
        format!("{};{}:{}", prop.name, params.join(";"), prop.val)
    }
}

/// Parse ATTENDEE/ORGANIZER property
fn parse_attendee(prop: &Property) -> Attendee {
    let email = prop
        .val
        .as_ref()
        .strip_prefix("mailto:")
        .unwrap_or(prop.val.as_ref())
        .to_string();

    let name = prop
        .params
        .iter()
        .find(|p| p.key == "CN")
        .and_then(|p| p.val.as_ref().map(|v| v.to_string()));

    let response_status = prop
        .params
        .iter()
        .find(|p| p.key == "PARTSTAT")
        .and_then(|p| p.val.as_ref())
        .and_then(|v| ParticipationStatus::from_ics_str(v.as_ref()));

    Attendee {
        name,
        email,
        response_status,
    }
}

/// Parse TRIGGER value to minutes before event (-PT30M, -P1D, etc.)
fn parse_trigger_minutes(value: &str) -> Option<i64> {
    let is_before = value.starts_with('-');
    let duration_str = value.trim_start_matches('-');

    // Use iso8601 crate via icalendar's internal helper if available,
    // otherwise parse manually
    let duration = iso8601::duration(duration_str).ok()?;
    let std_duration: std::time::Duration = duration.into();
    let minutes = (std_duration.as_secs() / 60) as i64;

    Some(if is_before { minutes } else { -minutes })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::ParticipationStatus;
    use crate::ics::generate_ics;
    use chrono::{TimeZone, Utc};

    #[test]
    fn test_parse_and_generate_roundtrip_multiple_attendees() {
        let event = Event {
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
            attendees: vec![
                Attendee {
                    name: Some("Alice".to_string()),
                    email: "alice@example.com".to_string(),
                    response_status: Some(ParticipationStatus::Accepted),
                },
                Attendee {
                    name: Some("Bob".to_string()),
                    email: "bob@example.com".to_string(),
                    response_status: Some(ParticipationStatus::Declined),
                },
            ],
            conference_url: None,
            updated: None,
            sequence: None,
            custom_properties: vec![],
        };

        let ics = generate_ics(&event).unwrap();
        let parsed = parse_event(&ics).expect("Should parse generated ICS");

        assert_eq!(
            parsed.attendees.len(),
            2,
            "Should have 2 attendees after roundtrip"
        );
    }

    #[test]
    fn test_parse_exdate_preserves_tzid_parameter() {
        let ics = r#"BEGIN:VCALENDAR
VERSION:2.0
PRODID:TEST
BEGIN:VEVENT
UID:test-123
SUMMARY:Recurring Event
DTSTART:20240101T100000Z
DTEND:20240101T110000Z
RRULE:FREQ=WEEKLY;BYDAY=MO
EXDATE;TZID=America/New_York:20240108T100000,20240115T100000
END:VEVENT
END:VCALENDAR"#;

        let event = parse_event(ics).expect("Should parse");

        let recurrence = event.recurrence.expect("Should have recurrence");
        assert!(
            recurrence
                .iter()
                .any(|r| r.contains("TZID=America/New_York")),
            "Should preserve TZID parameter. Got: {:?}",
            recurrence
        );
    }

    #[test]
    fn test_parse_line_folding_preserves_whitespace() {
        let ics = "BEGIN:VCALENDAR\r\n\
VERSION:2.0\r\n\
PRODID:TEST\r\n\
BEGIN:VEVENT\r\n\
UID:test-123\r\n\
SUMMARY:Test\r\n\
DTSTART:20240101T100000Z\r\n\
DTEND:20240101T110000Z\r\n\
DESCRIPTION:Hello \r\n world and \r\n more text\r\n\
END:VEVENT\r\n\
END:VCALENDAR";

        let event = parse_event(ics).expect("Should parse");

        let desc = event.description.expect("Should have description");
        assert_eq!(
            desc, "Hello world and more text",
            "Line folding should preserve the space before 'world'"
        );
    }

    #[test]
    fn test_exdate_roundtrip_preserves_tzid() {
        let ics = r#"BEGIN:VCALENDAR
VERSION:2.0
PRODID:TEST
BEGIN:VEVENT
UID:test-123
SUMMARY:Recurring Event
DTSTART:20240101T100000Z
DTEND:20240101T110000Z
RRULE:FREQ=WEEKLY;BYDAY=MO
EXDATE;TZID=America/New_York:20240108T100000,20240115T100000
END:VEVENT
END:VCALENDAR"#;

        let event = parse_event(ics).expect("Should parse");
        let generated = generate_ics(&event).expect("Should generate");

        println!("Generated ICS:\n{}", generated);

        let reparsed = parse_event(&generated).expect("Should reparse");

        // Check that EXDATE with TZID is preserved through round-trip
        let recurrence = reparsed.recurrence.expect("Should have recurrence");
        assert!(
            recurrence.iter().any(|r| r.contains("EXDATE") && r.contains("TZID=America/New_York")),
            "Should preserve EXDATE with TZID parameter after round-trip. Got: {:?}",
            recurrence
        );
    }

    #[test]
    fn test_multiple_exdate_properties_roundtrip() {
        // Test with multiple separate EXDATE properties
        let ics = r#"BEGIN:VCALENDAR
VERSION:2.0
PRODID:TEST
BEGIN:VEVENT
UID:test-123
SUMMARY:Recurring Event
DTSTART:20240101T100000Z
DTEND:20240101T110000Z
RRULE:FREQ=WEEKLY;BYDAY=MO
EXDATE;TZID=America/New_York:20240108T100000
EXDATE;TZID=America/New_York:20240115T100000
END:VEVENT
END:VCALENDAR"#;

        let event = parse_event(ics).expect("Should parse");
        println!("Parsed recurrence: {:?}", event.recurrence);

        let generated = generate_ics(&event).expect("Should generate");
        println!("Generated ICS:\n{}", generated);

        let reparsed = parse_event(&generated).expect("Should reparse");
        println!("Reparsed recurrence: {:?}", reparsed.recurrence);

        // Check that BOTH EXDATEs are preserved as separate properties
        let recurrence = reparsed.recurrence.expect("Should have recurrence");
        let exdate_count = recurrence.iter().filter(|r| r.contains("EXDATE")).count();
        assert_eq!(
            exdate_count, 2,
            "Should preserve both EXDATE properties. Got: {:?}",
            recurrence
        );

        // Check that both dates are present
        assert!(
            recurrence.iter().any(|r| r.contains("20240108T100000")),
            "Should have first EXDATE date"
        );
        assert!(
            recurrence.iter().any(|r| r.contains("20240115T100000")),
            "Should have second EXDATE date"
        );
    }
}
