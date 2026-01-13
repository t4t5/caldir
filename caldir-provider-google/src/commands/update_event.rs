use anyhow::Result;
use google_calendar::{Client, types::SendUpdates};
use serde::Deserialize;

use crate::commands::authenticate::redirect_uri;
use crate::config;
use crate::google::actions::get_valid_tokens;
use crate::google::{from_google::from_google_event, to_google::to_google_event};
use crate::{DEFAULT_CALENDAR_ID, types};

#[derive(Debug, Deserialize)]
struct UpdateEventParams {
    google_account: String,
    google_calendar_id: Option<String>,
    event: types::Event,
}

pub async fn handle_update_event(params: &serde_json::Value) -> Result<serde_json::Value> {
    let params: UpdateEventParams = serde_json::from_value(params.clone())?;

    let account = &params.google_account;

    let calendar_id = params
        .google_calendar_id
        .as_deref()
        .unwrap_or(DEFAULT_CALENDAR_ID);

    let creds = config::load_credentials()?;
    let tokens = get_valid_tokens(account).await?;

    let client = Client::new(
        creds.client_id.clone(),
        creds.client_secret.clone(),
        redirect_uri(),
        tokens.access_token,
        tokens.refresh_token,
    );

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
