//! List Google Calendars (name + config) for a given account.
//! The config should contain the minimum data needed to identify each calendar on your local
//! system.
//! In this case: google_account + google_calendar_id.

use anyhow::{Context, Result};
use caldir_core::calendar_config::CalendarConfig;
use google_calendar::types::MinAccessRole;
use serde::Deserialize;
use std::collections::HashMap;

use crate::session::Session;

#[derive(Debug, Deserialize)]
struct ListCalendarsParams {
    google_account: String,
}

pub async fn handle(params: &serde_json::Value) -> Result<serde_json::Value> {
    let params: ListCalendarsParams = serde_json::from_value(params.clone())?;

    let account_email = &params.google_account;

    let mut session = Session::load(account_email)?;
    session.refresh_if_needed().await?;

    let client = session.client();

    let google_calendars = client
        .calendar_list()
        .list_all(MinAccessRole::default(), false, false)
        .await
        .context("Failed to fetch calendars")?
        .body;

    let calendar_configs: Vec<CalendarConfig> = google_calendars
        .iter()
        .map(|cal| {
            let mut config = HashMap::new();
            config.insert(
                "google_account".to_string(),
                serde_json::Value::String(account_email.clone()),
            );
            config.insert(
                "google_calendar_id".to_string(),
                serde_json::Value::String(cal.id.clone()),
            );

            Ok(CalendarConfig {
                name: cal.summary.clone(),
                config,
            })
        })
        .collect::<Result<_, anyhow::Error>>()?;

    Ok(serde_json::to_value(calendar_configs)?)
}
