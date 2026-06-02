//! List Google Calendars (name + config) for a given account.

use anyhow::{Context, Result};
use caldir_core::provider::ProviderStorage;
use caldir_core::rpc::ListCalendars;
use caldir_core::{CalendarConfig, ProviderSlug, RemoteConfig};
use google_calendar::types::MinAccessRole;

use crate::app_config::AppConfigStore;
use crate::constants::PROVIDER_NAME;
use crate::remote_config::GoogleRemoteConfig;
use crate::session::SessionStore;

pub async fn handle(cmd: ListCalendars) -> Result<Vec<CalendarConfig>> {
    let account_email = &cmd.account_identifier;

    let storage = ProviderStorage::for_provider(PROVIDER_NAME)?;
    let session_store = SessionStore::new(storage.clone());
    let app_config_store = AppConfigStore::new(storage);

    let session = session_store
        .load_valid(account_email, &app_config_store)
        .await?;
    let client = session_store.client(&session, &app_config_store)?;

    let google_calendars = client
        .calendar_list()
        .list_all(MinAccessRole::default(), false, false)
        .await
        .context("Failed to fetch calendars")?
        .body;

    let calendar_configs = google_calendars
        .iter()
        .map(|cal| {
            let params =
                GoogleRemoteConfig::new(account_email, &cal.id).into_remote_config_params();
            let remote_config = RemoteConfig::new(ProviderSlug::from(PROVIDER_NAME), params);
            let read_only = !matches!(cal.access_role.as_str(), "writer" | "owner");

            CalendarConfig::new(
                Some(cal.summary.clone()),
                Some(cal.background_color.clone()),
                Some(read_only),
                Some(remote_config),
            )
        })
        .collect();

    Ok(calendar_configs)
}
