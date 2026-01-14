use anyhow::Result;
use caldir_core::Event;
use google_calendar::types::SendUpdates;
use serde::Deserialize;

use crate::DEFAULT_CALENDAR_ID;
use crate::google_auth::client_for_account;
use crate::parser::{from_google_event, to_google_event};

#[derive(Debug, Deserialize)]
struct UpdateEventParams {
    google_account: String,
    google_calendar_id: Option<String>,
    event: Event,
}

pub async fn handle_update_event(params: &serde_json::Value) -> Result<serde_json::Value> {
    let params: UpdateEventParams = serde_json::from_value(params.clone())?;

    let account = &params.google_account;

    let calendar_id = params
        .google_calendar_id
        .as_deref()
        .unwrap_or(DEFAULT_CALENDAR_ID);

    let client = client_for_account(account).await?;

    let event = params.event;

    let google_event = to_google_event(&event);

    let response = client
        .events()
        .update(
            calendar_id,
            &event.id,
            0,
            0,
            false,
            SendUpdates::None,
            false,
            &google_event,
        )
        .await?;

    let updated_event = from_google_event(response.body)?;
    Ok(serde_json::to_value(updated_event)?)
}
