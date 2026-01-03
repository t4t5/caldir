use anyhow::Result;

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
    let start_time = ics::parse_cli_datetime(&start)?;

    // Calculate end time from --end, --duration, or default
    let end_time = if let Some(end_str) = end {
        ics::parse_cli_datetime(&end_str)?
    } else if let Some(dur_str) = duration {
        let dur = ics::parse_cli_duration(&dur_str)?;
        match &start_time {
            EventTime::DateTime(dt) => EventTime::DateTime(*dt + dur),
            EventTime::Date(d) => {
                let days = dur.num_days().max(1) as i64;
                EventTime::Date(*d + chrono::Duration::days(days))
            }
        }
    } else {
        match &start_time {
            EventTime::DateTime(dt) => EventTime::DateTime(*dt + chrono::Duration::hours(1)),
            EventTime::Date(d) => EventTime::Date(*d + chrono::Duration::days(1)),
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
    let filename = ics::generate_filename(&event);

    // Write to disk
    caldir::write_event(&calendar_dir, &filename, &ics_content)?;

    println!("Created in {}: {}", calendar_name, filename);

    Ok(())
}
