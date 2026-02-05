//! ICS file parsing using the icalendar crate's parser.

use crate::event::{
    Attendee, Event, EventStatus, EventTime, ParticipationStatus, Recurrence, Reminder,
    Transparency,
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
    let rrule = vevent.find_prop("RRULE").map(|p| p.val.to_string());
    let exdates: Vec<EventTime> = vevent
        .properties
        .iter()
        .filter(|p| p.name == "EXDATE")
        .flat_map(parse_exdate_property)
        .collect();
    let recurrence = rrule.map(|rrule| Recurrence { rrule, exdates });

    // RECURRENCE-ID for instance overrides
    let recurrence_id = vevent
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

    // Custom X- properties (preserved for round-tripping provider-specific data)
    let custom_properties: Vec<(String, String)> = vevent
        .properties
        .iter()
        .filter(|p| p.name.as_ref().starts_with("X-"))
        .map(|p| (p.name.to_string(), p.val.to_string()))
        .collect();

    Some(Event {
        uid,
        summary,
        description,
        location,
        start,
        end,
        status,
        recurrence,
        recurrence_id,
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

/// Parse an EXDATE property into a list of EventTime values.
///
/// Handles:
/// - TZID parameter: `EXDATE;TZID=America/New_York:20240108T100000`
/// - VALUE=DATE: `EXDATE;VALUE=DATE:20240108`
/// - UTC: `EXDATE:20240108T100000Z`
/// - Floating: `EXDATE:20240108T100000`
/// - Comma-separated values: `EXDATE;TZID=...:20240108T100000,20240115T100000`
fn parse_exdate_property(prop: &Property) -> Vec<EventTime> {
    let tzid = prop
        .params
        .iter()
        .find(|p| p.key == "TZID")
        .and_then(|p| p.val.as_ref().map(|v| v.to_string()));

    let is_date = prop
        .params
        .iter()
        .any(|p| p.key == "VALUE" && p.val.as_ref().map(|v| v.as_ref()) == Some("DATE"));

    let val_str = prop.val.as_ref();
    val_str
        .split(',')
        .filter_map(|s| {
            let s = s.trim();
            if s.is_empty() {
                return None;
            }
            if is_date {
                chrono::NaiveDate::parse_from_str(s, "%Y%m%d")
                    .ok()
                    .map(EventTime::Date)
            } else if let Some(ref tz) = tzid {
                chrono::NaiveDateTime::parse_from_str(s, "%Y%m%dT%H%M%S")
                    .ok()
                    .map(|dt| EventTime::DateTimeZoned {
                        datetime: dt,
                        tzid: tz.clone(),
                    })
            } else if s.ends_with('Z') {
                let s = s.trim_end_matches('Z');
                chrono::NaiveDateTime::parse_from_str(s, "%Y%m%dT%H%M%S")
                    .ok()
                    .map(|dt| EventTime::DateTimeUtc(dt.and_utc()))
            } else {
                chrono::NaiveDateTime::parse_from_str(s, "%Y%m%dT%H%M%S")
                    .ok()
                    .map(EventTime::DateTimeFloating)
            }
        })
        .collect()
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
        assert_eq!(recurrence.rrule, "FREQ=WEEKLY;BYDAY=MO");
        assert_eq!(recurrence.exdates.len(), 2);
        // Both exdates should be zoned with America/New_York
        for exdate in &recurrence.exdates {
            match exdate {
                EventTime::DateTimeZoned { tzid, .. } => {
                    assert_eq!(tzid, "America/New_York");
                }
                other => panic!("Expected DateTimeZoned, got {:?}", other),
            }
        }
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
        assert_eq!(recurrence.rrule, "FREQ=WEEKLY;BYDAY=MO");
        assert_eq!(recurrence.exdates.len(), 2, "Should have 2 exdates after round-trip");
        for exdate in &recurrence.exdates {
            match exdate {
                EventTime::DateTimeZoned { tzid, .. } => {
                    assert_eq!(tzid, "America/New_York");
                }
                other => panic!("Expected DateTimeZoned, got {:?}", other),
            }
        }
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

        // Check that BOTH EXDATEs are preserved
        let recurrence = reparsed.recurrence.expect("Should have recurrence");
        assert_eq!(recurrence.rrule, "FREQ=WEEKLY;BYDAY=MO");
        assert_eq!(
            recurrence.exdates.len(), 2,
            "Should preserve both EXDATE values. Got: {:?}",
            recurrence.exdates
        );

        // Check that both dates are present as zoned datetimes
        let dates: Vec<String> = recurrence.exdates.iter().map(|e| format!("{}", e)).collect();
        assert!(
            dates.iter().any(|d| d.contains("2024-01-08")),
            "Should have first EXDATE date. Got: {:?}", dates
        );
        assert!(
            dates.iter().any(|d| d.contains("2024-01-15")),
            "Should have second EXDATE date. Got: {:?}", dates
        );
    }
}
