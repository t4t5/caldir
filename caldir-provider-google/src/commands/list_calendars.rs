//! List Google Calendars (name + config) for a given account.
//! The config should contain the minimum data needed to identify each calendar on your local
//! system.
//! In this case: google_account + google_calendar_id.

use anyhow::{Context, Result};
use caldir_core::calendar::config::CalendarConfig;
use caldir_core::remote::{Remote, protocol::ListCalendars, provider::Provider};
use google_calendar::types::MinAccessRole;

use crate::remote_config::GoogleRemoteConfig;
use crate::session::Session;

pub async fn handle(cmd: ListCalendars) -> Result<Vec<CalendarConfig>> {
    let account_email = &cmd.account_identifier;

    let client = Session::load_valid(account_email).await?.client()?;

    let google_calendars = client
        .calendar_list()
        .list_all(MinAccessRole::default(), false, false)
        .await
        .context("Failed to fetch calendars")?
        .body;

    let calendar_configs = google_calendars
        .iter()
        .map(|cal| {
            let remote_config = GoogleRemoteConfig::new(account_email, &cal.id);
            let remote = Remote::new(Provider::from_name("google"), remote_config.into());

            CalendarConfig {
                name: Some(cal.summary.clone()),
                color: Some(cal.background_color.clone()),
                remote: Some(remote),
            }
        })
        .collect();

    Ok(calendar_configs)
}
