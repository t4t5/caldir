//! Delete an event from a CalDAV calendar.

use anyhow::Result;
use caldir_core::remote::protocol::DeleteEvent;
use caldir_provider_caldav::ops;

use crate::remote_config::CaldavRemoteConfig;
use crate::session::Session;

pub async fn handle(cmd: DeleteEvent) -> Result<()> {
    let config = CaldavRemoteConfig::try_from(&cmd.remote_config)?;
    let session = Session::load(&config.caldav_account)?;
    let (username, password) = session.credentials();

    ops::delete_event(username, password, &config.caldav_calendar_url, &cmd.event_id).await
}
