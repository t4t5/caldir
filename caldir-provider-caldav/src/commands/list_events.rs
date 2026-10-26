//! List events within a time range from a CalDAV calendar.

use anyhow::Result;
use caldir_core::Event;
use caldir_core::rpc::ListEvents;
use caldir_provider_caldav::caldav::ops;

use crate::remote_config::CaldavRemoteConfig;

pub async fn handle(cmd: ListEvents) -> Result<Vec<Event>> {
    let config = CaldavRemoteConfig::try_from(&cmd.remote)?;
    let (username, password) = crate::commands::credentials(&config.caldav_account).await?;

    ops::fetch_events(
        &username,
        &password,
        &config.caldav_calendar_url,
        &cmd.from,
        &cmd.to,
    )
    .await
}
