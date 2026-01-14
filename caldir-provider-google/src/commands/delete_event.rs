use anyhow::{Context, Result};
use google_calendar::Client;
use google_calendar::types::SendUpdates;
use serde::Deserialize;

use crate::DEFAULT_CALENDAR_ID;
use crate::config;
use crate::google_auth::{get_valid_tokens, redirect_uri};

#[derive(Debug, Deserialize)]
struct DeleteEventParams {
    google_calendar_id: Option<String>,
    event_id: String,
}

pub async fn handle_delete_event(params: &serde_json::Value) -> Result<serde_json::Value> {
    let params: DeleteEventParams = serde_json::from_value(params.clone())?;

    let calendar_id = params
        .google_calendar_id
        .as_deref()
        .unwrap_or(DEFAULT_CALENDAR_ID);

    let creds = config::load_credentials()?;
    let tokens = get_valid_tokens(calendar_id).await?;

    let client = Client::new(
        creds.client_id.clone(),
        creds.client_secret.clone(),
        redirect_uri(),
        tokens.access_token,
        tokens.refresh_token,
    );

    let event_id = params.event_id;

    let result = client
        .events()
        .delete(calendar_id, &event_id, false, SendUpdates::None)
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
