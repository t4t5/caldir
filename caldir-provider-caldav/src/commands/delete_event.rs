//! Delete an event from a CalDAV calendar.

use anyhow::Result;
use caldir_core::rpc::DeleteEvent;
use caldir_provider_caldav::caldav::ops;

use crate::remote_config::CaldavRemoteConfig;

pub async fn handle(cmd: DeleteEvent) -> Result<()> {
    let config = CaldavRemoteConfig::try_from(&cmd.remote)?;
    let (username, password) = crate::commands::credentials(&config.caldav_account).await?;

    ops::delete_event(
        &username,
        &password,
        &config.caldav_calendar_url,
        &cmd.event.event_instance_id(),
    )
    .await
}
