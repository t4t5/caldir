use anyhow::{Context, Result};
use caldir_core::Event;
use google_calendar::types::SendUpdates;
use serde::Deserialize;

use crate::DEFAULT_CALENDAR_ID;
use crate::convert::{FromGoogle, ToGoogle};
use crate::session::Session;

#[derive(Debug, Deserialize)]
struct CreateEventParams {
    google_account: String,
    google_calendar_id: Option<String>,
    event: Event,
}

pub async fn handle(params: &serde_json::Value) -> Result<serde_json::Value> {
    let params: CreateEventParams = serde_json::from_value(params.clone())?;

    let calendar_id = params
        .google_calendar_id
        .as_deref()
        .unwrap_or(DEFAULT_CALENDAR_ID);

    let mut session = Session::load(&params.google_account)?;
    session.refresh_if_needed().await?;

    let client = session.client();

    let event = params.event;

    let mut google_event = event.to_google();
    google_event.id = String::new(); // Let Google assign the ID

    let response = client
        .events()
        .insert(
            calendar_id,
            0,
            0,
            false,
            SendUpdates::None,
            false,
            &google_event,
        )
        .await
        .with_context(|| format!("Failed to create event: {}", event.summary))?;

    let created_event = Event::from_google(response.body)?;
    Ok(serde_json::to_value(created_event)?)
}
