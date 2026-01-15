use anyhow::{Context, Result};
use caldir_lib::Event;
use chrono::{DateTime, Utc};
use google_calendar::types::OrderBy;
use serde::Deserialize;

use crate::google_event::FromGoogle;
use crate::session::Session;

#[derive(Debug, Deserialize)]
struct ListEventsParams {
    google_account: String,
    google_calendar_id: String,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
}

pub async fn handle(params: serde_json::Value) -> Result<serde_json::Value> {
    let params: ListEventsParams = serde_json::from_value(params)?;

    let account_email = params.google_account;
    let calendar_id = params.google_calendar_id;
    let time_min = params.from.to_rfc3339();
    let time_max = params.to.to_rfc3339();

    let client = Session::load_valid(&account_email).await?.client()?;

    let google_events = client
        .events()
        .list_all(
            &calendar_id,
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
        .context("Failed to fetch events")?
        .body;

    let events: Vec<Event> = google_events
        .into_iter()
        .map(Event::from_google)
        .collect::<Result<_, _>>()?;

    Ok(serde_json::to_value(events)?)
}
