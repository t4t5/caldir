use anyhow::{Context, Result};
use google_calendar::types::SendUpdates;
use serde::Deserialize;

use crate::session::Session;

#[derive(Debug, Deserialize)]
struct DeleteEventParams {
    google_account: String,
    google_calendar_id: String,
    event_id: String,
}

pub async fn handle(params: serde_json::Value) -> Result<serde_json::Value> {
    let params: DeleteEventParams = serde_json::from_value(params)?;

    let account_email = params.google_account;
    let event_id = params.event_id;
    let calendar_id = params.google_calendar_id;

    let client = Session::load_valid(&account_email).await?.client()?;

    client
        .events()
        .delete(&calendar_id, &event_id, false, SendUpdates::None)
        .await
        .context("Failed to delete event")?;

    Ok(serde_json::Value::Null)
}
