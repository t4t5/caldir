use anyhow::{Context, Result};
use caldir_core::caldir::Caldir;
use caldir_core::event::{Event, EventTime};
use chrono::{Duration, NaiveDateTime};
use owo_colors::OwoColorize;

pub fn run(summary: String, start: String) -> Result<()> {
    let caldir = Caldir::load()?;

    let calendar = caldir.default_calendar();

    // Parse start time (e.g. "2025-03-20T15:00")
    let start_dt = NaiveDateTime::parse_from_str(&start, "%Y-%m-%dT%H:%M")
        .context("Invalid start time format. Use: 2025-03-20T15:00")?;

    // Assume all events are 1h long for now:
    let end_dt = start_dt + Duration::hours(1);

    match calendar {
        Some(cal) => {
            let event = Event::new(
                summary,
                EventTime::DateTimeFloating(start_dt),
                EventTime::DateTimeFloating(end_dt),
                None,
                None,
                None,
                Vec::new(),
            );

            cal.create_event(&event)?;

            println!("{}", format!("Created: {}", event.summary).green());
        }
        None => {
            println!("{}", "Default calendar not found.".red());
        }
    }

    Ok(())
}
