use anyhow::{Context, Result};
use caldir_core::calendar::ProviderCalendar;
use google_calendar::types::MinAccessRole;
use serde::Deserialize;

use crate::commands::authed_client;
use crate::parser::from_google_calendar;

#[derive(Debug, Deserialize)]
struct ListCalendarsParams {
    google_account: String,
}

pub async fn handle_list_calendars(params: &serde_json::Value) -> Result<serde_json::Value> {
    let params: ListCalendarsParams = serde_json::from_value(params.clone())?;

    let account_email = &params.google_account;

    let client = authed_client(account_email).await?;

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
