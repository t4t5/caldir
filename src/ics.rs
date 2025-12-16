use crate::providers::gcal::{Attendee, Event, EventStatus, EventTime, Transparency};
use anyhow::Result;
use icalendar::{Alarm, Calendar, Component, EventLike, Trigger};

/// Generate .ics content for an event
pub fn generate_ics(event: &Event) -> Result<String> {
    let mut cal = Calendar::new();

    let mut ics_event = icalendar::Event::new();
    ics_event.uid(&event.id);
    ics_event.summary(&event.summary);

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
    for reminder in &event.reminders {
        let trigger = Trigger::before_start(chrono::Duration::minutes(reminder.minutes));
        let alarm = Alarm::display(&event.summary, trigger);
        ics_event.alarm(alarm);
    }

    // ORGANIZER
    if let Some(ref org) = event.organizer {
        let organizer_value = format_attendee_value(org);
        ics_event.add_property("ORGANIZER", &organizer_value);
    }

    // ATTENDEE
    for attendee in &event.attendees {
        let attendee_value = format_attendee_value(attendee);
        ics_event.add_property("ATTENDEE", &attendee_value);
    }

    // Conference URL (preserve as X-GOOGLE-CONFERENCE or standard URL)
    if let Some(ref url) = event.conference_url {
        // Add as both standard URL and vendor extension for compatibility
        ics_event.add_property("URL", url);
        ics_event.add_property("X-GOOGLE-CONFERENCE", url);
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

/// Check if an event is a recurring master (has RRULE)
pub fn is_recurring_master(event: &Event) -> bool {
    event.recurrence.is_some()
}

/// Check if an event is an instance override of a recurring event
pub fn is_instance_override(event: &Event) -> bool {
    event.recurring_event_id.is_some() && event.original_start.is_some()
}

/// Format an attendee for iCalendar ORGANIZER/ATTENDEE properties
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
        format!("{};:mailto:{}", parts.join(";"), attendee.email)
    }
}

/// Convert a string to a filename-safe slug
fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c
            } else if c.is_whitespace() || c == '-' || c == '_' {
                '-'
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
        if line.starts_with("UID:") {
            return Some(line[4..].trim().to_string());
        }
    }
    None
}
