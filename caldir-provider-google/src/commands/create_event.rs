use anyhow::{Context, Result};
use caldir_core::Event;
use google_calendar::Client;
use google_calendar::types::SendUpdates;
use serde::Deserialize;

use crate::commands::authenticate::redirect_uri;
use crate::config;
use crate::google::actions::get_valid_tokens;
use crate::google::from_google::from_google_event;
use crate::google::to_google::to_google_event;
use crate::{DEFAULT_CALENDAR_ID, types};

#[derive(Debug, Deserialize)]
struct CreateEventParams {
    google_calendar_id: Option<String>,
    event: types::Event,
}

pub async fn handle_create_event(params: &serde_json::Value) -> Result<Event> {
    let params: CreateEventParams = serde_json::from_value(params.clone())?;

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

    from_google_event(response.body)
}
