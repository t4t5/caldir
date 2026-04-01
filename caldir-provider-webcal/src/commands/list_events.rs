//! List events within a time range from a webcal subscription.

use anyhow::Result;
use chrono::{DateTime, Utc};
use caldir_core::event::Event;
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
    let from_utc = DateTime::parse_from_rfc3339(&cmd.from)
        .map(|dt| dt.with_timezone(&Utc))?;
    let to_utc = DateTime::parse_from_rfc3339(&cmd.to)
        .map(|dt| dt.with_timezone(&Utc))?;

    // Filter events by date range
    let filtered: Vec<Event> = all_events
        .into_iter()
        .filter(|event| {
            // Always include master recurring events (caldir-core handles expansion)
            if event.recurrence.is_some() {
                return true;
            }

            // Use EventTime::to_utc() for proper timezone-aware comparison
            let Some(start) = event.start.to_utc() else {
                return true;
            };
            let Some(end) = event.end.to_utc() else {
                return true;
            };

            // Include if the event overlaps the requested range
            start < to_utc && end > from_utc
        })
        .collect();

    Ok(filtered)
}
