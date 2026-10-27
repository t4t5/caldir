//! Update an existing event on a CalDAV calendar.

use anyhow::Result;
use caldir_core::Event;
use caldir_core::rpc::UpdateEvent;
use caldir_provider_caldav::caldav::ops;

use crate::remote_config::CaldavRemoteConfig;

pub async fn handle(cmd: UpdateEvent) -> Result<Event> {
    let config = CaldavRemoteConfig::try_from(&cmd.remote)?;
    let (username, password) = crate::commands::credentials(&config).await?;

    ops::update_event(&username, &password, &config.caldav_calendar_url, cmd.event).await
}
