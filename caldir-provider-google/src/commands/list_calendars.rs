use anyhow::{Context, Result};
use caldir_core::calendar::ProviderCalendar;
use google_calendar::Client;
use google_calendar::types::MinAccessRole;
use serde::Deserialize;

use crate::config;
use crate::google_auth::{get_valid_tokens, redirect_uri};
use crate::parser::from_google_calendar;

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

    let google_calendars = client
        .calendar_list()
        .list_all(MinAccessRole::default(), false, false)
        .await
        .context("Failed to fetch calendars")?
        .body;

    let calendars: Vec<ProviderCalendar> = google_calendars
        .into_iter()
        .map(|cal| from_google_calendar(&cal))
        .collect();

    Ok(serde_json::to_value(calendars)?)
}
