use anyhow::{Context, Result};
use chrono::{Duration, NaiveDateTime};
use owo_colors::OwoColorize;

use caldir_lib::EventTime;

use crate::client::Client;

pub async fn run(summary: String, start: String, calendar: Option<String>) -> Result<()> {
    let client = Client::connect().await?;

    // Get default calendar if not specified
    let calendar_name = match calendar {
        Some(name) => name,
        None => client
            .default_calendar()
            .await?
            .ok_or_else(|| anyhow::anyhow!("No default calendar configured. Set default_calendar in ~/.config/caldir/config.toml"))?,
    };

    // Parse start time (e.g. "2025-03-20T15:00")
    let start_dt = NaiveDateTime::parse_from_str(&start, "%Y-%m-%dT%H:%M")
        .context("Invalid start time format. Use: 2025-03-20T15:00")?;

    // Assume all events are 1h long for now
    let end_dt = start_dt + Duration::hours(1);

    client
        .create_event(
            &calendar_name,
            summary.clone(),
            EventTime::DateTimeFloating(start_dt),
            EventTime::DateTimeFloating(end_dt),
        )
        .await?;

    println!("{}", format!("Created: {}", summary).green());

    Ok(())
}
