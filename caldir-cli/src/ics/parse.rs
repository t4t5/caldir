//! ICS file parsing.

use crate::event::{
    Attendee, Event, EventStatus, EventTime, ParticipationStatus, Reminder, Transparency,
};
use chrono::{NaiveDate, TimeZone, Utc};

/// Parse ICS content into an Event struct
pub fn parse_event(content: &str) -> Option<Event> {
    let mut in_vevent = false;
    let mut in_valarm = false;
    let mut current_line = String::new();

    // Collected properties
    let mut uid = None;
    let mut summary = None;
    let mut description = None;
    let mut location = None;
    let mut dtstart = None;
    let mut dtend = None;
    let mut status = EventStatus::Confirmed;
    let mut transparency = Transparency::Opaque;
    let mut recurrence: Vec<String> = Vec::new();
    let mut recurrence_id = None;
    let mut organizer = None;
    let mut attendees: Vec<Attendee> = Vec::new();
    let mut reminders: Vec<Reminder> = Vec::new();
    let mut conference_url = None;
    let mut sequence = None;
    let mut custom_properties: Vec<(String, String)> = Vec::new();

    for line in content.lines() {
        // Handle line folding (RFC 5545: continuation lines start with single space or tab)
        // Only remove the first character (the continuation indicator), preserve other whitespace
        if line.starts_with(' ') || line.starts_with('\t') {
            current_line.push_str(&line[1..]);
            continue;
        }

        // Process completed line
        if !current_line.is_empty() && in_vevent {
            if in_valarm {
                // Extract TRIGGER from alarms
                if let Some((key, _, value)) = parse_property_line_with_params(&current_line)
                    && key == "TRIGGER"
                    && let Some(minutes) = parse_trigger_minutes(&value)
                {
                    reminders.push(Reminder { minutes });
                }
            } else if let Some((key, params, value)) = parse_property_line_with_params(&current_line)
            {
                match key.as_str() {
                    "UID" => uid = Some(value),
                    "SUMMARY" => summary = Some(value),
                    "DESCRIPTION" => description = Some(value),
                    "LOCATION" => location = Some(value),
                    "DTSTART" => dtstart = parse_datetime(&value, &params),
                    "DTEND" => dtend = parse_datetime(&value, &params),
                    "STATUS" => {
                        status = match value.as_str() {
                            "TENTATIVE" => EventStatus::Tentative,
                            "CANCELLED" => EventStatus::Cancelled,
                            _ => EventStatus::Confirmed,
                        };
                    }
                    "TRANSP" => {
                        transparency = if value == "TRANSPARENT" {
                            Transparency::Transparent
                        } else {
                            Transparency::Opaque
                        };
                    }
                    "RRULE" | "EXDATE" => {
                        // Preserve parameters (e.g., TZID) for EXDATE
                        if params.is_empty() {
                            recurrence.push(format!("{}:{}", key, value));
                        } else {
                            recurrence.push(format!("{};{}:{}", key, params, value));
                        }
                    }
                    "RECURRENCE-ID" => {
                        recurrence_id = parse_datetime(&value, &params);
                    }
                    "ORGANIZER" => {
                        organizer = Some(parse_attendee_value(&params, &value));
                    }
                    "ATTENDEE" => {
                        attendees.push(parse_attendee_value(&params, &value));
                    }
                    "URL" => {
                        conference_url = Some(value);
                    }
                    "SEQUENCE" => {
                        sequence = value.parse().ok();
                    }
                    _ if key.starts_with("X-") => {
                        custom_properties.push((key, value));
                    }
                    _ => {}
                }
            }
        }

        current_line = line.to_string();

        // Track components
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

    // Require at minimum UID, summary, start, and end
    let uid = uid?;
    let summary = summary.unwrap_or_else(|| "(No title)".to_string());
    let start = dtstart?;
    let end = dtend?;

    Some(Event {
        id: uid,
        summary,
        description,
        location,
        start,
        end,
        status,
        recurrence: if recurrence.is_empty() {
            None
        } else {
            Some(recurrence)
        },
        original_start: recurrence_id,
        reminders,
        transparency,
        organizer,
        attendees,
        conference_url,
        updated: None, // Not stored in ICS we generate
        sequence,
        custom_properties,
    })
}

/// Parse a single ICS property line into key, parameters, and value
fn parse_property_line_with_params(line: &str) -> Option<(String, String, String)> {
    let colon_pos = line.find(':')?;
    let key_part = &line[..colon_pos];
    let value = &line[colon_pos + 1..];

    let mut parts = key_part.splitn(2, ';');
    let key = parts.next()?.to_string();
    let params = parts.next().unwrap_or("").to_string();

    // Unescape ICS values (reverse of RFC 5545 escaping)
    let unescaped_value = unescape_ics_value(value);

    Some((key, params, unescaped_value))
}

/// Unescape ICS property values per RFC 5545
/// Reverses: \, → , and \; → ; and \\ → \ and \n → newline
fn unescape_ics_value(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.peek() {
                Some(',') => {
                    result.push(',');
                    chars.next();
                }
                Some(';') => {
                    result.push(';');
                    chars.next();
                }
                Some('\\') => {
                    result.push('\\');
                    chars.next();
                }
                Some('n') | Some('N') => {
                    result.push('\n');
                    chars.next();
                }
                _ => result.push(c), // Keep backslash if not a recognized escape
            }
        } else {
            result.push(c);
        }
    }

    result
}

/// Parse a datetime or date value from ICS
fn parse_datetime(value: &str, params: &str) -> Option<EventTime> {
    // Check if it's a date-only value (VALUE=DATE)
    let is_date = params.contains("VALUE=DATE");

    if is_date || (value.len() == 8 && value.chars().all(|c| c.is_ascii_digit())) {
        // Date format: YYYYMMDD
        let y = value.get(0..4)?.parse().ok()?;
        let m = value.get(4..6)?.parse().ok()?;
        let d = value.get(6..8)?.parse().ok()?;
        let date = NaiveDate::from_ymd_opt(y, m, d)?;
        return Some(EventTime::Date(date));
    }

    // DateTime format: YYYYMMDDTHHMMSSZ or YYYYMMDDTHHMMSS
    if value.len() >= 15 && value.contains('T') {
        let y: i32 = value.get(0..4)?.parse().ok()?;
        let mo: u32 = value.get(4..6)?.parse().ok()?;
        let d: u32 = value.get(6..8)?.parse().ok()?;
        let h: u32 = value.get(9..11)?.parse().ok()?;
        let mi: u32 = value.get(11..13)?.parse().ok()?;
        let s: u32 = value.get(13..15)?.parse().ok()?;

        let dt = Utc.with_ymd_and_hms(y, mo, d, h, mi, s).single()?;
        return Some(EventTime::DateTime(dt));
    }

    None
}

/// Parse ATTENDEE/ORGANIZER parameters and value into an Attendee struct
fn parse_attendee_value(params: &str, value: &str) -> Attendee {
    // Extract email from mailto:email@example.com
    let email = value
        .strip_prefix("mailto:")
        .unwrap_or(value)
        .to_string();

    // Parse parameters like CN=Name;PARTSTAT=ACCEPTED
    let mut name = None;
    let mut response_status = None;

    for param in params.split(';') {
        if let Some(cn) = param.strip_prefix("CN=") {
            name = Some(cn.to_string());
        } else if let Some(partstat) = param.strip_prefix("PARTSTAT=") {
            response_status = ParticipationStatus::from_ics_str(partstat);
        }
    }

    Attendee {
        name,
        email,
        response_status,
    }
}

/// Parse TRIGGER value to minutes before event
fn parse_trigger_minutes(value: &str) -> Option<i64> {
    // Format: -PT{n}M, -PT{n}H, -P{n}D, -PT{n}S, etc.
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
            start: EventTime::DateTime(Utc.with_ymd_and_hms(2025, 3, 20, 15, 0, 0).unwrap()),
            end: EventTime::DateTime(Utc.with_ymd_and_hms(2025, 3, 20, 16, 0, 0).unwrap()),
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
        // ICS with EXDATE that has TZID parameter
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
            recurrence.iter().any(|r| r.contains("TZID=America/New_York")),
            "Should preserve TZID parameter. Got: {:?}",
            recurrence
        );
    }

    #[test]
    fn test_parse_line_folding_preserves_whitespace() {
        // ICS with folded description line - the fold happens in the middle of text
        // After "Hello " the line is folded, and continues with "world"
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
