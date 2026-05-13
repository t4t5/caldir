//! Update an existing event on a CalDAV calendar.

use anyhow::Result;
use caldir_core::Event;
use caldir_core::rpc::UpdateEvent;
use caldir_provider_caldav::ops;

use crate::remote_config::CaldavRemoteConfig;
use crate::session::Session;

pub async fn handle(cmd: UpdateEvent) -> Result<Event> {
    let config = CaldavRemoteConfig::try_from(&cmd.remote)?;
    let session = Session::load(&config.caldav_account)?;
    let (username, password) = session.credentials();

    ops::update_event(username, password, &config.caldav_calendar_url, cmd.event).await
}
