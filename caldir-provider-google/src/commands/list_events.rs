use anyhow::{Context, Result};
use caldir_core::Event;
use caldir_core::constants::DEFAULT_SYNC_DAYS;
use google_calendar::Client;
use google_calendar::types::OrderBy;
use serde::Deserialize;

use crate::DEFAULT_CALENDAR_ID;
use crate::commands::authenticate::redirect_uri;
use crate::config;
use crate::google::actions::get_valid_tokens;
use crate::google::from_google::from_google_event;

#[derive(Debug, Deserialize)]
struct ListEventsParams {
    google_account: String,
    google_calendar_id: Option<String>,
    #[serde(default)]
    time_min: Option<String>,
    #[serde(default)]
    time_max: Option<String>,
}

pub async fn handle_list_events(params: &serde_json::Value) -> Result<serde_json::Value> {
    let params: ListEventsParams = serde_json::from_value(params.clone())?;

    let account = &params.google_account;

    let creds = config::load_credentials()?;
    let tokens = get_valid_tokens(account).await?;

    let calendar_id = params
        .google_calendar_id
        .as_deref()
        .unwrap_or(DEFAULT_CALENDAR_ID);

    let client = Client::new(
        creds.client_id.clone(),
        creds.client_secret.clone(),
        redirect_uri(),
        tokens.access_token,
        tokens.refresh_token,
    );

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

    let mut events: Vec<Event> = Vec::new();

    for google_event in response.body {
        let event = from_google_event(google_event)?;
        events.push(event);
    }

    Ok(serde_json::to_value(events)?)
}
