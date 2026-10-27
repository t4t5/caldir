//! List events within a time range from an iCloud calendar.

use anyhow::Result;
use caldir_core::Event;
use caldir_core::rpc::ListEvents;
use caldir_provider_caldav::caldav::ops;

use crate::remote_config::ICloudRemoteConfig;

pub async fn handle(cmd: ListEvents) -> Result<Vec<Event>> {
    let config = ICloudRemoteConfig::try_from(&cmd.remote)?;
    let (username, password) = crate::commands::credentials(&config).await?;

    ops::fetch_events(
        &username,
        &password,
        &config.icloud_calendar_url,
        &cmd.from,
        &cmd.to,
    )
    .await
    .map(|events| events.into_iter().map(super::normalize_event).collect())
}
