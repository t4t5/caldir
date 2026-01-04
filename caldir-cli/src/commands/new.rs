use anyhow::Result;
use chrono::NaiveDate;

use crate::event::{Event, EventStatus, EventTime, Transparency};
use crate::{caldir, config, ics};

pub async fn run(
    title: String,
    start: String,
    end: Option<String>,
    duration: Option<String>,
    description: Option<String>,
    location: Option<String>,
    calendar: Option<String>,
) -> Result<()> {
    let cfg = config::load_config()?;

    // Determine which calendar to use
    let calendar_name = calendar.or(cfg.default_calendar.clone()).ok_or_else(|| {
        anyhow::anyhow!(
            "No calendar specified and no default_calendar in config.\n\
            Use --calendar <name> or set default_calendar in config.toml"
        )
    })?;

    // Verify the calendar exists in config
    if !cfg.calendars.contains_key(&calendar_name) {
        anyhow::bail!(
            "Calendar '{}' not found in config.\n\
            Available calendars: {}",
            calendar_name,
            cfg.calendars
                .keys()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    // Get calendar-specific directory
    let calendar_dir = config::calendar_path(&cfg, &calendar_name);
    std::fs::create_dir_all(&calendar_dir)?;

    // Parse start time
    let start_time = parse_datetime(&start)?;

    // Calculate end time from --end, --duration, or default
    let end_time = if let Some(end_str) = end {
        parse_datetime(&end_str)?
    } else if let Some(dur_str) = duration {
        let dur = parse_duration(&dur_str)?;
        match &start_time {
            EventTime::DateTimeFloating(dt) => EventTime::DateTimeFloating(*dt + dur),
            EventTime::Date(d) => {
                let days = dur.num_days().max(1) as i64;
                EventTime::Date(*d + chrono::Duration::days(days))
            }
            // These won't be created by parse_datetime in new command, but handle them
            EventTime::DateTimeUtc(dt) => EventTime::DateTimeUtc(*dt + dur),
            EventTime::DateTimeZoned { datetime, tzid } => EventTime::DateTimeZoned {
                datetime: *datetime + dur,
                tzid: tzid.clone(),
            },
        }
    } else {
        match &start_time {
            EventTime::DateTimeFloating(dt) => {
                EventTime::DateTimeFloating(*dt + chrono::Duration::hours(1))
            }
            EventTime::Date(d) => EventTime::Date(*d + chrono::Duration::days(1)),
            EventTime::DateTimeUtc(dt) => EventTime::DateTimeUtc(*dt + chrono::Duration::hours(1)),
            EventTime::DateTimeZoned { datetime, tzid } => EventTime::DateTimeZoned {
                datetime: *datetime + chrono::Duration::hours(1),
                tzid: tzid.clone(),
            },
        }
    };

    // Generate a unique local ID
    let event_id = format!("local-{}", uuid::Uuid::new_v4());

    // Create the event
    let event = Event {
        id: event_id,
        summary: title,
        description,
        location,
        start: start_time,
        end: end_time,
        status: EventStatus::Confirmed,
        recurrence: None,
        original_start: None,
        reminders: Vec::new(),
        transparency: Transparency::Opaque,
        organizer: None,
        attendees: Vec::new(),
        conference_url: None,
        updated: Some(chrono::Utc::now()),
        sequence: Some(0),
        custom_properties: Vec::new(),
    };

    // Generate ICS content and filename
    let metadata = ics::CalendarMetadata {
        calendar_id: "local".to_string(),
        calendar_name: calendar_name.clone(),
    };

    let ics_content = ics::generate_ics(&event, &metadata)?;
    let base_filename = ics::generate_filename(&event);
    let filename = caldir::unique_filename(&base_filename, &calendar_dir, &event.id)?;

    // Write to disk
    caldir::write_event(&calendar_dir, &filename, &ics_content)?;

    println!("Created in {}: {}", calendar_name, filename);

    Ok(())
}

// =============================================================================
// CLI Input Parsing
// =============================================================================

/// Parse a datetime string from CLI input into an EventTime.
/// Supports:
/// - Date only: "2025-03-20" (all-day event)
/// - Date with time: "2025-03-20T15:00" or "2025-03-20T15:00:00"
///
/// Returns DateTimeFloating for timed events since locally-created events
/// are in local time (the user's current timezone, not explicitly UTC).
fn parse_datetime(s: &str) -> Result<EventTime> {
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
                // Use floating time for locally-created events
                return Ok(EventTime::DateTimeFloating(naive));
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
fn parse_duration(s: &str) -> Result<chrono::Duration> {
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
