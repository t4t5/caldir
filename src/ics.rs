use crate::event::{Attendee, Event, EventStatus, EventTime, Reminder, Transparency};
use anyhow::Result;
use chrono::{NaiveDate, TimeZone, Utc};
use icalendar::{Alarm, Calendar, Component, EventLike, Property, Trigger};

/// Metadata about the calendar source (for sync tracking)
#[derive(Debug, Clone)]
pub struct CalendarMetadata {
    /// Calendar ID (e.g., "user@gmail.com")
    pub calendar_id: String,
    /// Human-readable calendar name (e.g., "Personal Calendar")
    pub calendar_name: String,
    /// Source URL for this calendar (provider-specific)
    pub source_url: Option<String>,
}

/// Generate .ics content for an event with calendar metadata
pub fn generate_ics(event: &Event, metadata: &CalendarMetadata) -> Result<String> {
    let mut cal = Calendar::new();

    // Add calendar-level metadata properties
    // SOURCE (RFC 7986) - URL identifying the calendar source
    if let Some(ref source_url) = metadata.source_url {
        cal.append_property(Property::new("SOURCE", source_url));
    }

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
        (EventTime::Date(start_date), EventTime::Date(_end_date)) => {
            // All-day event
            ics_event.all_day(*start_date);
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
        let organizer_value = format_organizer_value(org);
        ics_event.add_property("ORGANIZER", &organizer_value);
    }

    // ATTENDEE
    for attendee in &event.attendees {
        let attendee_value = format_attendee_value(attendee);
        ics_event.add_property("ATTENDEE", &attendee_value);
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

/// Format an organizer for iCalendar ORGANIZER property
/// Format: CN=Name:mailto:email@example.com
/// Note: ORGANIZER doesn't have PARTSTAT (only ATTENDEE does)
fn format_organizer_value(organizer: &Attendee) -> String {
    if let Some(ref name) = organizer.name {
        format!("CN={}:mailto:{}", name, organizer.email)
    } else {
        format!("mailto:{}", organizer.email)
    }
}

/// Format an attendee for iCalendar ATTENDEE property
/// Format: CN=Name;PARTSTAT=ACCEPTED:mailto:email@example.com
fn format_attendee_value(attendee: &Attendee) -> String {
    let mut parts = Vec::new();

    // Add CN (common name) parameter if available
    if let Some(ref name) = attendee.name {
        parts.push(format!("CN={}", name));
    }

    // Add PARTSTAT (participation status) parameter if available
    if let Some(ref status) = attendee.response_status {
        let partstat = match status.as_str() {
            "accepted" => "ACCEPTED",
            "declined" => "DECLINED",
            "tentative" => "TENTATIVE",
            "needsAction" => "NEEDS-ACTION",
            _ => "NEEDS-ACTION",
        };
        parts.push(format!("PARTSTAT={}", partstat));
    }

    // Build the value
    if parts.is_empty() {
        format!("mailto:{}", attendee.email)
    } else {
        format!("{}:mailto:{}", parts.join(";"), attendee.email)
    }
}

/// Convert a string to a filename-safe slug
fn slugify(s: &str) -> String {
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
    // Take first 8 chars of the ID
    id.chars().take(8).collect()
}

/// Parse the UID from an .ics file
pub fn parse_uid(content: &str) -> Option<String> {
    for line in content.lines() {
        if let Some(stripped) = line.strip_prefix("UID:") {
            return Some(stripped.trim().to_string());
        }
    }
    None
}

/// Parse the DTSTART from an .ics file content, returning it as DateTime<Utc>.
/// All-day events are converted to midnight UTC of that date.
pub fn parse_dtstart_utc(content: &str) -> Option<chrono::DateTime<Utc>> {
    use icalendar::{CalendarComponent, DatePerhapsTime, CalendarDateTime};
    use std::str::FromStr;

    let calendar = Calendar::from_str(content).ok()?;

    for component in calendar.iter() {
        if let CalendarComponent::Event(event) = component {
            match event.get_start()? {
                DatePerhapsTime::DateTime(cal_dt) => {
                    // Try to convert to UTC; for Floating/WithTimezone, fall back to naive conversion
                    match cal_dt {
                        CalendarDateTime::Utc(dt) => return Some(dt),
                        CalendarDateTime::Floating(naive) => return Some(naive.and_utc()),
                        CalendarDateTime::WithTimezone { date_time, .. } => {
                            return Some(date_time.and_utc())
                        }
                    }
                }
                DatePerhapsTime::Date(date) => {
                    // All-day event: use midnight UTC
                    return Some(date.and_hms_opt(0, 0, 0)?.and_utc());
                }
            }
        }
    }
    None
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
// ICS Parsing (for property-level diff)
// ============================================================================

use std::collections::HashMap;

/// Properties to skip when computing property-level diffs
const SKIP_PROPERTIES: &[&str] = &[
    "DTSTAMP", "LAST-MODIFIED", "BEGIN", "END", "VERSION", "PRODID", "CALSCALE",
];

/// Parse ICS content into property key-value pairs (for the VEVENT component).
/// Also extracts alarm triggers as a special "ALARMS" property.
pub fn parse_properties(content: &str) -> HashMap<String, String> {
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
        if !current_line.is_empty() && in_vevent {
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
        // Handle line folding
        if line.starts_with(' ') || line.starts_with('\t') {
            current_line.push_str(line.trim_start());
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
                        recurrence.push(format!("{}:{}", key, value));
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
            // Convert ICS PARTSTAT to Google's format
            response_status = Some(match partstat {
                "ACCEPTED" => "accepted".to_string(),
                "DECLINED" => "declined".to_string(),
                "TENTATIVE" => "tentative".to_string(),
                "NEEDS-ACTION" => "needsAction".to_string(),
                _ => partstat.to_lowercase(),
            });
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

// ============================================================================
// ICS Formatting (for human-readable display)
// ============================================================================

/// Human-readable names for ICS properties
pub fn property_display_name(prop: &str) -> &str {
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
pub fn format_property_value(prop: &str, value: &str) -> String {
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

/// Format a TRIGGER value for display (e.g., "-PT86400S" -> "1 day before")
fn format_trigger_value(value: &str) -> String {
    // Parse ISO 8601 duration format: -PT{n}S, -PT{n}M, -PT{n}H, -P{n}D, etc.
    let is_before = value.starts_with('-');
    let duration_part = value
        .trim_start_matches('-')
        .trim_start_matches('P')
        .trim_start_matches('T');

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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, Timelike};

    #[test]
    fn test_format_organizer_with_name() {
        let organizer = Attendee {
            name: Some("John Doe".to_string()),
            email: "john@example.com".to_string(),
            response_status: None,
        };
        assert_eq!(
            format_organizer_value(&organizer),
            "CN=John Doe:mailto:john@example.com"
        );
    }

    #[test]
    fn test_format_organizer_without_name() {
        let organizer = Attendee {
            name: None,
            email: "john@example.com".to_string(),
            response_status: None,
        };
        assert_eq!(
            format_organizer_value(&organizer),
            "mailto:john@example.com"
        );
    }

    #[test]
    fn test_format_organizer_ignores_response_status() {
        // ORGANIZER should NOT include PARTSTAT even if response_status is set
        let organizer = Attendee {
            name: Some("John Doe".to_string()),
            email: "john@example.com".to_string(),
            response_status: Some("accepted".to_string()),
        };
        let result = format_organizer_value(&organizer);
        assert!(!result.contains("PARTSTAT"), "ORGANIZER should not have PARTSTAT");
        assert_eq!(result, "CN=John Doe:mailto:john@example.com");
    }

    #[test]
    fn test_format_attendee_with_name_and_status() {
        let attendee = Attendee {
            name: Some("Jane Doe".to_string()),
            email: "jane@example.com".to_string(),
            response_status: Some("accepted".to_string()),
        };
        assert_eq!(
            format_attendee_value(&attendee),
            "CN=Jane Doe;PARTSTAT=ACCEPTED:mailto:jane@example.com"
        );
    }

    #[test]
    fn test_format_attendee_with_name_only() {
        let attendee = Attendee {
            name: Some("Jane Doe".to_string()),
            email: "jane@example.com".to_string(),
            response_status: None,
        };
        assert_eq!(
            format_attendee_value(&attendee),
            "CN=Jane Doe:mailto:jane@example.com"
        );
    }

    #[test]
    fn test_format_attendee_with_status_only() {
        let attendee = Attendee {
            name: None,
            email: "jane@example.com".to_string(),
            response_status: Some("declined".to_string()),
        };
        assert_eq!(
            format_attendee_value(&attendee),
            "PARTSTAT=DECLINED:mailto:jane@example.com"
        );
    }

    #[test]
    fn test_format_attendee_email_only() {
        let attendee = Attendee {
            name: None,
            email: "jane@example.com".to_string(),
            response_status: None,
        };
        assert_eq!(
            format_attendee_value(&attendee),
            "mailto:jane@example.com"
        );
    }

    #[test]
    fn test_format_attendee_partstat_values() {
        let test_cases = vec![
            ("accepted", "ACCEPTED"),
            ("declined", "DECLINED"),
            ("tentative", "TENTATIVE"),
            ("needsAction", "NEEDS-ACTION"),
            ("unknown", "NEEDS-ACTION"), // Unknown values default to NEEDS-ACTION
        ];

        for (input, expected) in test_cases {
            let attendee = Attendee {
                name: None,
                email: "test@example.com".to_string(),
                response_status: Some(input.to_string()),
            };
            let result = format_attendee_value(&attendee);
            assert!(
                result.contains(&format!("PARTSTAT={}", expected)),
                "Input '{}' should produce PARTSTAT={}, got: {}",
                input, expected, result
            );
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
    fn test_short_id() {
        assert_eq!(short_id("abc12345xyz"), "abc12345");
        assert_eq!(short_id("short"), "short");
        assert_eq!(short_id(""), "");
    }

    #[test]
    fn test_parse_dtstart_utc_datetime_with_z() {
        let ics = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nDTSTART:20241217T180000Z\nEND:VEVENT\nEND:VCALENDAR";
        let result = parse_dtstart_utc(ics);
        assert!(result.is_some());
        let dt = result.unwrap();
        assert_eq!(dt.year(), 2024);
        assert_eq!(dt.month(), 12);
        assert_eq!(dt.day(), 17);
        assert_eq!(dt.hour(), 18);
        assert_eq!(dt.minute(), 0);
    }

    #[test]
    fn test_parse_dtstart_utc_all_day_event() {
        let ics = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nDTSTART;VALUE=DATE:20241217\nEND:VEVENT\nEND:VCALENDAR";
        let result = parse_dtstart_utc(ics);
        assert!(result.is_some());
        let dt = result.unwrap();
        assert_eq!(dt.year(), 2024);
        assert_eq!(dt.month(), 12);
        assert_eq!(dt.day(), 17);
        assert_eq!(dt.hour(), 0); // All-day events become midnight UTC
    }

    #[test]
    fn test_parse_dtstart_utc_floating_datetime() {
        let ics = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nDTSTART:20241217T140000\nEND:VEVENT\nEND:VCALENDAR";
        let result = parse_dtstart_utc(ics);
        assert!(result.is_some());
        let dt = result.unwrap();
        assert_eq!(dt.hour(), 14); // Floating treated as UTC
    }

    #[test]
    fn test_parse_dtstart_utc_with_timezone() {
        let ics = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nDTSTART;TZID=America/New_York:20241217T100000\nEND:VEVENT\nEND:VCALENDAR";
        let result = parse_dtstart_utc(ics);
        assert!(result.is_some());
        // Note: timezone is ignored, naive datetime treated as UTC
        let dt = result.unwrap();
        assert_eq!(dt.hour(), 10);
    }

    #[test]
    fn test_parse_dtstart_utc_missing_dtstart() {
        let ics = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nSUMMARY:No start time\nEND:VEVENT\nEND:VCALENDAR";
        let result = parse_dtstart_utc(ics);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_dtstart_utc_invalid_ics() {
        let ics = "not valid ics content";
        let result = parse_dtstart_utc(ics);
        assert!(result.is_none());
    }
}
