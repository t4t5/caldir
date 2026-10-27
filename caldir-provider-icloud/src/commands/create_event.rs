//! Create a new event on iCloud Calendar.

use anyhow::Result;
use caldir_core::Event;
use caldir_core::rpc::CreateEvent;
use caldir_provider_caldav::caldav::ops;

use crate::remote_config::ICloudRemoteConfig;

pub async fn handle(cmd: CreateEvent) -> Result<Event> {
    let config = ICloudRemoteConfig::try_from(&cmd.remote)?;
    let (username, password) = crate::commands::credentials(&config).await?;

    ops::create_event(
        &username,
        &password,
        &config.icloud_calendar_url,
        super::normalize_event(cmd.event),
    )
    .await
    .map(super::normalize_event)
}
