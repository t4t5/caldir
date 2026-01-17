//! List Google Calendars (name + config) for a given account.
//! The config should contain the minimum data needed to identify each calendar on your local
//! system.
//! In this case: google_account + google_calendar_id.

use anyhow::{Context, Result};
use caldir_core::{
    calendar::Calendar,
    config::calendar_config::CalendarConfig,
    provider::Provider,
    remote::{Remote, RemoteConfig},
};
use google_calendar::types::MinAccessRole;
use serde::Deserialize;
use std::collections::HashMap;

use crate::session::Session;

#[derive(Debug, Deserialize)]
struct ListCalendarsParams {
    account_identifier: String,
}

pub async fn handle(params: serde_json::Value) -> Result<serde_json::Value> {
    let params: ListCalendarsParams = serde_json::from_value(params)?;

    let account_email = &params.account_identifier;

    let client = Session::load_valid(account_email).await?.client()?;

    let google_calendars = client
        .calendar_list()
        .list_all(MinAccessRole::default(), false, false)
        .await
        .context("Failed to fetch calendars")?
        .body;

    let calendars = google_calendars
        .iter()
        .map(|cal| {
            let mut config = HashMap::new();
            config.insert(
                "google_account".to_string(),
                toml::Value::String(account_email.clone()),
            );
            config.insert(
                "google_calendar_id".to_string(),
                toml::Value::String(cal.id.clone()),
            );

            Calendar {
                name: cal.summary.clone(),
                config: CalendarConfig {
                    remote: Some(Remote {
                        provider: Provider::from_name("google"),
                        config: RemoteConfig(config),
                    }),
                },
            }
        })
        .collect::<Vec<Calendar>>();

    Ok(serde_json::to_value(calendars)?)
}
