use anyhow::{Context, Result};
use google_calendar::Client;
use google_calendar::types::MinAccessRole;
use serde::Deserialize;

use crate::commands::authenticate::redirect_uri;
use crate::config;
use crate::google::actions::get_valid_tokens;
use crate::types::GoogleCalendar;

#[derive(Debug, Deserialize)]
struct ListCalendarsParams {
    google_account: String,
}

pub async fn handle_list_calendars(params: &serde_json::Value) -> Result<serde_json::Value> {
    let params: ListCalendarsParams = serde_json::from_value(params.clone())?;

    let account = &params.google_account;

    let creds = config::load_credentials()?;
    let tokens = get_valid_tokens(account).await?;

    let client = Client::new(
        creds.client_id.clone(),
        creds.client_secret.clone(),
        redirect_uri(),
        tokens.access_token,
        tokens.refresh_token,
    );

    let response = client
        .calendar_list()
        .list_all(MinAccessRole::default(), false, false)
        .await
        .context("Failed to fetch calendars")?;

    let calendars: Vec<String> = response
        .body
        .into_iter()
        .filter(|c| !c.id.is_empty())
        .map(|c| GoogleCalendar::from_calendar_list_entry(c).name)
        .collect();

    Ok(serde_json::to_value(calendars)?)
}
