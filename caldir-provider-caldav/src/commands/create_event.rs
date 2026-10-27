//! Create a new event on a CalDAV calendar.

use anyhow::Result;
use caldir_core::Event;
use caldir_core::rpc::CreateEvent;
use caldir_provider_caldav::caldav::ops;

use crate::remote_config::CaldavRemoteConfig;

pub async fn handle(cmd: CreateEvent) -> Result<Event> {
    let config = CaldavRemoteConfig::try_from(&cmd.remote)?;
    let (username, password) = crate::commands::credentials(&config).await?;

    ops::create_event(&username, &password, &config.caldav_calendar_url, cmd.event).await
}
