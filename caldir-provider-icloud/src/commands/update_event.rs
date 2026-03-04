//! Update an existing event on iCloud Calendar.

use anyhow::Result;
use caldir_core::event::Event;
use caldir_core::remote::protocol::UpdateEvent;
use caldir_provider_caldav::ops;

use crate::remote_config::ICloudRemoteConfig;
use crate::session::Session;

pub async fn handle(cmd: UpdateEvent) -> Result<Event> {
    let config = ICloudRemoteConfig::try_from(&cmd.remote_config)?;
    let session = Session::load(&config.icloud_account)?;
    let (username, password) = session.credentials();

    ops::update_event(username, password, &config.icloud_calendar_url, cmd.event).await
}
