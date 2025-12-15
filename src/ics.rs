use crate::providers::gcal::{Event, EventStatus, EventTime};
use anyhow::Result;
use icalendar::{Calendar, Component, EventLike};

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

    let ics_event = ics_event.done();
    cal.push(ics_event);
    let cal = cal.done();

    Ok(cal.to_string())
}

/// Generate the caldir filename for an event
pub fn generate_filename(event: &Event) -> String {
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

    let slug = slugify(&event.summary);

    format!("{}__{}_{}.ics", date_part, slug, short_id(&event.id))
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
