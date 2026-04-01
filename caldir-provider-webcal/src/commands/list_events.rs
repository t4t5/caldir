//! List events within a time range from a webcal subscription.

use anyhow::Result;
use chrono::{DateTime, NaiveDateTime, Utc};
use caldir_core::event::{Event, EventTime};
use caldir_core::ics::parse_events;
use caldir_core::remote::protocol::ListEvents;

use crate::remote_config::WebcalRemoteConfig;

pub async fn handle(cmd: ListEvents) -> Result<Vec<Event>> {
    let config = WebcalRemoteConfig::try_from(&cmd.remote_config)?;

    // Fetch the ICS feed
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("caldir-provider-webcal")
        .build()?;

    let response = client.get(&config.webcal_url).send().await?;

    if !response.status().is_success() {
        anyhow::bail!(
            "Failed to fetch webcal feed: HTTP {}",
            response.status()
        );
    }

    let body = response.text().await?;

    // Parse all events from the ICS body
    let all_events =
        parse_events(&body).map_err(|e| anyhow::anyhow!("Failed to parse webcal feed: {e}"))?;

    // Parse the from/to date range
    let from_dt = DateTime::parse_from_rfc3339(&cmd.from)
        .map(|dt| dt.with_timezone(&Utc))?;
    let to_dt = DateTime::parse_from_rfc3339(&cmd.to)
        .map(|dt| dt.with_timezone(&Utc))?;

    let from_naive = from_dt.naive_utc();
    let to_naive = to_dt.naive_utc();

    // Filter events by date range
    let filtered: Vec<Event> = all_events
        .into_iter()
        .filter(|event| {
            // Always include master recurring events (caldir-core handles expansion)
            if event.recurrence.is_some() {
                return true;
            }

            let start_naive = event_time_to_naive(&event.start);
            let end_naive = event_time_to_naive(&event.end);

            // Include if the event overlaps the requested range: start < to && end > from
            start_naive < to_naive && end_naive > from_naive
        })
        .collect();

    Ok(filtered)
}

/// Convert an EventTime to a NaiveDateTime for comparison purposes.
fn event_time_to_naive(time: &EventTime) -> NaiveDateTime {
    match time {
        EventTime::Date(date) => date.and_hms_opt(0, 0, 0).unwrap(),
        EventTime::DateTimeUtc(dt) => dt.naive_utc(),
        EventTime::DateTimeFloating(dt) => *dt,
        EventTime::DateTimeZoned { datetime, .. } => *datetime,
    }
}
