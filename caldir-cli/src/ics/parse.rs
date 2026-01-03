//! ICS file parsing using the icalendar crate's parser.

use crate::event::{
    Attendee, Event, EventStatus, EventTime, ParticipationStatus, Reminder, Transparency,
};
use chrono::{NaiveDate, NaiveDateTime};
use icalendar::parser::{read_calendar, unfold, Property};

/// Parse ICS content into an Event struct
pub fn parse_event(content: &str) -> Option<Event> {
    let unfolded = unfold(content);
    let calendar = read_calendar(&unfolded).ok()?;

    // Find the VEVENT component
    let vevent = calendar.components.iter().find(|c| c.name == "VEVENT")?;

    // Extract required fields
    let uid = vevent.find_prop("UID")?.val.to_string();
    let summary = vevent
        .find_prop("SUMMARY")
        .map(|p| p.val.to_string())
        .unwrap_or_else(|| "(No title)".to_string());

    let start = parse_datetime_prop(vevent.find_prop("DTSTART")?)?;
    let end = parse_datetime_prop(vevent.find_prop("DTEND")?)?;

    // Extract optional fields
    let description = vevent.find_prop("DESCRIPTION").map(|p| p.val.to_string());
    let location = vevent.find_prop("LOCATION").map(|p| p.val.to_string());

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

    let sequence = vevent
        .find_prop("SEQUENCE")
        .and_then(|p| p.val.as_ref().parse().ok());

    let conference_url = vevent.find_prop("URL").map(|p| p.val.to_string());

    // Recurrence rules (RRULE, EXDATE)
    let recurrence: Vec<String> = vevent
        .properties
        .iter()
        .filter(|p| p.name == "RRULE" || p.name == "EXDATE")
        .map(|p| format_property_with_params(p))
        .collect();
    let recurrence = if recurrence.is_empty() {
        None
    } else {
        Some(recurrence)
    };

    // RECURRENCE-ID for instance overrides
    let original_start = vevent
        .find_prop("RECURRENCE-ID")
        .and_then(parse_datetime_prop);

    // ORGANIZER
    let organizer = vevent.find_prop("ORGANIZER").map(parse_attendee_prop);

    // ATTENDEE (can appear multiple times)
    let attendees: Vec<Attendee> = vevent
        .properties
        .iter()
        .filter(|p| p.name == "ATTENDEE")
        .map(parse_attendee_prop)
        .collect();

    // VALARM components for reminders
    let reminders: Vec<Reminder> = vevent
        .components
        .iter()
        .filter(|c| c.name == "VALARM")
        .filter_map(|alarm| {
            alarm
                .find_prop("TRIGGER")
                .and_then(|p| parse_trigger_minutes(p.val.as_ref()))
                .map(|minutes| Reminder { minutes })
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

/// Format a property with its parameters back to ICS format (e.g., "RRULE:FREQ=WEEKLY")
fn format_property_with_params(prop: &Property) -> String {
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

/// Parse a DTSTART/DTEND property into EventTime
fn parse_datetime_prop(prop: &Property) -> Option<EventTime> {
    let value = prop.val.as_ref();
    let is_date_only = prop
        .params
        .iter()
        .any(|p| p.key == "VALUE" && p.val.as_ref().map(|v| v.as_ref()) == Some("DATE"));

    if is_date_only || value.len() == 8 {
        // Date format: YYYYMMDD
        let date = NaiveDate::parse_from_str(value, "%Y%m%d").ok()?;
        Some(EventTime::Date(date))
    } else {
        // DateTime format: YYYYMMDDTHHMMSS or YYYYMMDDTHHMMSSZ
        let value = value.trim_end_matches('Z');
        let naive = NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%S").ok()?;
        Some(EventTime::DateTime(naive.and_utc()))
    }
}

/// Parse ATTENDEE/ORGANIZER property into Attendee struct
fn parse_attendee_prop(prop: &Property) -> Attendee {
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

/// Parse TRIGGER value to minutes before event
/// Format: -PT30M, -PT1H, -P1D, etc.
fn parse_trigger_minutes(value: &str) -> Option<i64> {
    let is_before = value.starts_with('-');
    let duration_part = value
        .trim_start_matches('-')
        .trim_start_matches('P')
        .trim_start_matches('T');

    let minutes = if let Some(s) = duration_part.strip_suffix('S') {
        s.parse::<i64>().ok()? / 60
    } else if let Some(m) = duration_part.strip_suffix('M') {
        m.parse::<i64>().ok()?
    } else if let Some(h) = duration_part.strip_suffix('H') {
        h.parse::<i64>().ok()? * 60
    } else if let Some(d) = duration_part.strip_suffix('D') {
        d.parse::<i64>().ok()? * 24 * 60
    } else {
        return None;
    };

    Some(if is_before { minutes } else { -minutes })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::ParticipationStatus;
    use crate::ics::{generate_ics, CalendarMetadata};
    use chrono::TimeZone;

    fn make_test_metadata() -> CalendarMetadata {
        CalendarMetadata {
            calendar_id: "test@example.com".to_string(),
            calendar_name: "Test Calendar".to_string(),
        }
    }

    #[test]
    fn test_parse_and_generate_roundtrip_multiple_attendees() {
        let event = Event {
            id: "test-event-123".to_string(),
            summary: "Test Event".to_string(),
            description: None,
            location: None,
            start: EventTime::DateTime(
                chrono::Utc.with_ymd_and_hms(2025, 3, 20, 15, 0, 0).unwrap(),
            ),
            end: EventTime::DateTime(chrono::Utc.with_ymd_and_hms(2025, 3, 20, 16, 0, 0).unwrap()),
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

        let ics = generate_ics(&event, &make_test_metadata()).unwrap();
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
        // The icalendar parser handles line folding via unfold()
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
}
