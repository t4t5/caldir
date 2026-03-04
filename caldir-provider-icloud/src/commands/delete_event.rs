//! Delete an event from iCloud Calendar.

use anyhow::Result;
use caldir_core::remote::protocol::DeleteEvent;
use caldir_provider_caldav::ops;

use crate::remote_config::ICloudRemoteConfig;
use crate::session::Session;

pub async fn handle(cmd: DeleteEvent) -> Result<()> {
    let config = ICloudRemoteConfig::try_from(&cmd.remote_config)?;
    let session = Session::load(&config.icloud_account)?;
    let (username, password) = session.credentials();

    ops::delete_event(username, password, &config.icloud_calendar_url, &cmd.event.uid).await
}
