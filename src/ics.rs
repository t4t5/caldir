use crate::event::{Attendee, Event, EventStatus, EventTime, ParticipationStatus, Reminder, Transparency};
use anyhow::Result;
use chrono::{NaiveDate, TimeZone, Utc};
use icalendar::{Alarm, Calendar, Component, EventLike, Property, Trigger, ValueType};

/// Metadata about the calendar source (embedded in .ics files)
#[derive(Debug, Clone)]
pub struct CalendarMetadata {
    /// Calendar ID (e.g., "user@gmail.com")
    pub calendar_id: String,
    /// Human-readable calendar name (e.g., "Personal Calendar")
    pub calendar_name: String,
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
    match (&event.start, &event.end) {
        (EventTime::Date(start_date), EventTime::Date(end_date)) => {
            // All-day event - set both DTSTART and DTEND with VALUE=DATE parameter
            // DTEND is exclusive (day after last day of event)
            let mut dtstart = Property::new("DTSTART", start_date.format("%Y%m%d").to_string());
            dtstart.append_parameter(ValueType::Date);
            ics_event.append_property(dtstart);

            let mut dtend = Property::new("DTEND", end_date.format("%Y%m%d").to_string());
            dtend.append_parameter(ValueType::Date);
            ics_event.append_property(dtend);
        }
        (EventTime::DateTime(start_dt), EventTime::DateTime(end_dt)) => {
            // Timed event
            ics_event.starts(*start_dt);
            ics_event.ends(*end_dt);
        }
        // Mixed cases - treat as timed event with date at midnight
        (EventTime::Date(d), EventTime::DateTime(end_dt)) => {
            ics_event.starts(d.and_hms_opt(0, 0, 0).unwrap().and_utc());
            ics_event.ends(*end_dt);
        }
        (EventTime::DateTime(start_dt), EventTime::Date(d)) => {
            ics_event.starts(*start_dt);
            ics_event.ends(d.and_hms_opt(0, 0, 0).unwrap().and_utc());
        }
    }

    // Optional fields
    if let Some(ref desc) = event.description {
        ics_event.description(desc);
    }

    if let Some(ref loc) = event.location {
        ics_event.location(loc);
    }

    // Status
    let status = match event.status {
        EventStatus::Confirmed => "CONFIRMED",
        EventStatus::Tentative => "TENTATIVE",
        EventStatus::Cancelled => "CANCELLED",
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
        let recurrence_id = match original_start {
            EventTime::DateTime(dt) => dt.format("%Y%m%dT%H%M%SZ").to_string(),
            EventTime::Date(d) => d.format("%Y%m%d").to_string(),
        };
        ics_event.add_property("RECURRENCE-ID", &recurrence_id);
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
        EventTime::DateTime(dt) => {
            // Format: 2025-03-20T1500
            dt.format("%Y-%m-%dT%H%M").to_string()
        }
        EventTime::Date(d) => {
            // Format: 2025-03-20
            d.format("%Y-%m-%d").to_string()
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
// ============================================================================
// CLI Input Parsing
// ============================================================================

/// Parse a datetime string from CLI input into an EventTime.
/// Supports:
/// - Date only: "2025-03-20" (all-day event)
/// - Date with time: "2025-03-20T15:00" or "2025-03-20T15:00:00"
pub fn parse_cli_datetime(s: &str) -> Result<EventTime> {
    // Try date-only format first: YYYY-MM-DD
    if s.len() == 10 && s.chars().filter(|&c| c == '-').count() == 2 {
        let date = NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .map_err(|_| anyhow::anyhow!("Invalid date format: {}. Expected YYYY-MM-DD", s))?;
        return Ok(EventTime::Date(date));
    }

    // Try datetime format: YYYY-MM-DDTHH:MM or YYYY-MM-DDTHH:MM:SS
    if s.contains('T') {
        // Handle both with and without seconds
        let formats = ["%Y-%m-%dT%H:%M:%S", "%Y-%m-%dT%H:%M"];
        for fmt in formats {
            if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(s, fmt) {
                let dt = naive.and_utc();
                return Ok(EventTime::DateTime(dt));
            }
        }
        anyhow::bail!(
            "Invalid datetime format: {}. Expected YYYY-MM-DDTHH:MM or YYYY-MM-DDTHH:MM:SS",
            s
        );
    }

    anyhow::bail!(
        "Invalid date/time format: {}. Expected YYYY-MM-DD or YYYY-MM-DDTHH:MM",
        s
    )
}

/// Parse a duration string from CLI input.
/// Supports: "30m", "1h", "2h30m", "90m"
pub fn parse_cli_duration(s: &str) -> Result<chrono::Duration> {
    let s = s.trim().to_lowercase();
    let mut total_minutes: i64 = 0;
    let mut current_num = String::new();

    for c in s.chars() {
        if c.is_ascii_digit() {
            current_num.push(c);
        } else if c == 'h' {
            let hours: i64 = current_num
                .parse()
                .map_err(|_| anyhow::anyhow!("Invalid duration: {}", s))?;
            total_minutes += hours * 60;
            current_num.clear();
        } else if c == 'm' {
            let mins: i64 = current_num
                .parse()
                .map_err(|_| anyhow::anyhow!("Invalid duration: {}", s))?;
            total_minutes += mins;
            current_num.clear();
        } else {
            anyhow::bail!("Invalid duration character '{}' in: {}", c, s);
        }
    }

    // If there's a trailing number without unit, treat as minutes
    if !current_num.is_empty() {
        let mins: i64 = current_num
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid duration: {}", s))?;
        total_minutes += mins;
    }

    if total_minutes == 0 {
        anyhow::bail!("Duration must be greater than 0: {}", s);
    }

    Ok(chrono::Duration::minutes(total_minutes))
}

// ============================================================================
// ICS Parsing Helpers
// ============================================================================

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
                if let Some((key, _, value)) = parse_property_line_with_params(&current_line) {
                    if key == "TRIGGER" {
                        if let Some(minutes) = parse_trigger_minutes(&value) {
                            reminders.push(Reminder { minutes });
                        }
                    }
                }
            } else if let Some((key, params, value)) = parse_property_line_with_params(&current_line) {
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
        recurrence: if recurrence.is_empty() { None } else { Some(recurrence) },
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
    use crate::event::EventStatus;

    fn make_test_event() -> Event {
        Event {
            id: "test-event-123".to_string(),
            summary: "Test Event".to_string(),
            description: None,
            location: None,
            start: EventTime::DateTime(
                chrono::Utc.with_ymd_and_hms(2025, 3, 20, 15, 0, 0).unwrap(),
            ),
            end: EventTime::DateTime(
                chrono::Utc.with_ymd_and_hms(2025, 3, 20, 16, 0, 0).unwrap(),
            ),
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
    fn test_parse_and_generate_roundtrip_multiple_attendees() {
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
                response_status: Some(ParticipationStatus::Declined),
            },
        ];

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
