//! List events within a time range from an iCloud calendar.

use anyhow::Result;
use caldir_core::event::Event;
use caldir_core::remote::protocol::ListEvents;
use caldir_provider_caldav::ops;

use crate::remote_config::ICloudRemoteConfig;
use crate::session::Session;

pub async fn handle(cmd: ListEvents) -> Result<Vec<Event>> {
    let config = ICloudRemoteConfig::try_from(&cmd.remote_config)?;
    let session = Session::load(&config.icloud_account)?;
    let (username, password) = session.credentials();

    ops::fetch_events(
        username,
        password,
        &config.icloud_calendar_url,
        &cmd.from,
        &cmd.to,
    )
    .await
}
