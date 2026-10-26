//! Delete an event from iCloud Calendar.

use anyhow::Result;
use caldir_core::rpc::DeleteEvent;
use caldir_provider_caldav::caldav::ops;

use crate::remote_config::ICloudRemoteConfig;

pub async fn handle(cmd: DeleteEvent) -> Result<()> {
    let config = ICloudRemoteConfig::try_from(&cmd.remote)?;
    let (username, password) = crate::commands::credentials(&config.icloud_account).await?;

    ops::delete_event(
        &username,
        &password,
        &config.icloud_calendar_url,
        &cmd.event.event_instance_id(),
    )
    .await
}
