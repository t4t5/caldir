use anyhow::{Context, Result};
use caldir_core::Event;
use google_calendar::types::SendUpdates;
use serde::Deserialize;

use crate::DEFAULT_CALENDAR_ID;
use crate::google_auth::client_for_account;
use crate::parser::{from_google_event, to_google_event};

#[derive(Debug, Deserialize)]
struct CreateEventParams {
    google_account: String,
    google_calendar_id: Option<String>,
    event: Event,
}

pub async fn handle_create_event(params: &serde_json::Value) -> Result<serde_json::Value> {
    let params: CreateEventParams = serde_json::from_value(params.clone())?;

    let account = &params.google_account;

    let calendar_id = params
        .google_calendar_id
        .as_deref()
        .unwrap_or(DEFAULT_CALENDAR_ID);

    let client = client_for_account(account).await?;

    let event = params.event;

    let mut google_event = to_google_event(&event);
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

    let created_event = from_google_event(response.body)?;
    Ok(serde_json::to_value(created_event)?)
}
