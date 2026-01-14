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

pub async fn handle(params: &serde_json::Value) -> Result<serde_json::Value> {
    let params: DeleteEventParams = serde_json::from_value(params.clone())?;

    let account_email = &params.google_account;
    let event_id = params.event_id;
    let calendar_id = params.google_calendar_id;

    let mut session = Session::load(account_email)?;
    session.refresh_if_needed().await?;

    let client = session.client();

    let result = client
        .events()
        .delete(&calendar_id, &event_id, false, SendUpdates::None)
        .await;

    match result {
        Ok(_) => Ok(serde_json::Value::Null),
        Err(e) => {
            let error_str = e.to_string();
            if error_str.contains("410") || error_str.contains("Gone") {
                Ok(serde_json::Value::Null)
            } else {
                Err(e).with_context(|| format!("Failed to delete event: {}", event_id))
            }
        }
    }
}
