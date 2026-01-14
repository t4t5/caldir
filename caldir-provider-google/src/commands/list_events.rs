use anyhow::{Context, Result};
use caldir_core::Event;
use caldir_core::constants::DEFAULT_SYNC_DAYS;
use google_calendar::types::OrderBy;
use serde::Deserialize;

use crate::DEFAULT_CALENDAR_ID;
use crate::commands::authed_client;
use crate::parser::FromGoogle;

#[derive(Debug, Deserialize)]
struct ListEventsParams {
    google_account: String,
    google_calendar_id: Option<String>,
    #[serde(default)]
    time_min: Option<String>,
    #[serde(default)]
    time_max: Option<String>,
}

pub async fn handle(params: &serde_json::Value) -> Result<serde_json::Value> {
    let params: ListEventsParams = serde_json::from_value(params.clone())?;

    let account_email = &params.google_account;

    let calendar_id = params
        .google_calendar_id
        .as_deref()
        .unwrap_or(DEFAULT_CALENDAR_ID);

    let client = authed_client(account_email).await?;

    // Default to ±1 year if not specified
    let now = chrono::Utc::now();
    let default_time_min = (now - chrono::Duration::days(DEFAULT_SYNC_DAYS)).to_rfc3339();
    let default_time_max = (now + chrono::Duration::days(DEFAULT_SYNC_DAYS)).to_rfc3339();

    let time_min = params.time_min.unwrap_or(default_time_min);
    let time_max = params.time_max.unwrap_or(default_time_max);

    let response = client
        .events()
        .list_all(
            calendar_id,
            "",
            0,
            OrderBy::default(),
            &[],
            "", // search query
            &[],
            false,
            false,
            false,
            &time_max,
            &time_min,
            "",
            "",
        )
        .await
        .context("Failed to fetch events")?;

    let events: Vec<Event> = response
        .body
        .into_iter()
        .map(Event::from_google)
        .collect::<Result<_, _>>()?;

    Ok(serde_json::to_value(events)?)
}
