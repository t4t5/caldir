use anyhow::{Context, Result};
use chrono::{Duration, NaiveDateTime};
use owo_colors::OwoColorize;

use caldir_lib::EventTime;

use crate::client::{Client, CreateEventRequest};

pub async fn run(summary: String, start: String, calendar: Option<String>) -> Result<()> {
    let client = Client::connect().await?;

    // Get default calendar if not specified
    let calendar_name = match calendar {
        Some(name) => name,
        None => {
            let calendars = client.list_calendars().await?;
            calendars
                .first()
                .map(|c| c.name.clone())
                .ok_or_else(|| anyhow::anyhow!("No calendars found"))?
        }
    };

    // Parse start time (e.g. "2025-03-20T15:00")
    let start_dt = NaiveDateTime::parse_from_str(&start, "%Y-%m-%dT%H:%M")
        .context("Invalid start time format. Use: 2025-03-20T15:00")?;

    // Assume all events are 1h long for now:
    let end_dt = start_dt + Duration::hours(1);

    let req = CreateEventRequest {
        summary: summary.clone(),
        start: EventTime::DateTimeFloating(start_dt),
        end: EventTime::DateTimeFloating(end_dt),
        description: None,
        location: None,
    };

    client.create_event(&calendar_name, req).await?;

    println!("{}", format!("Created: {}", summary).green());

    Ok(())
}
