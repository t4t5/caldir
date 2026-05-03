//! Create a new event on a CalDAV calendar.

use anyhow::Result;
use caldir_core::event::Event;
use caldir_core::remote::protocol::{CreateEvent, ProviderRequestContext};
use caldir_provider_caldav::ops;

use crate::remote_config::CaldavRemoteConfig;
use crate::session::Session;

pub async fn handle(context: ProviderRequestContext, cmd: CreateEvent) -> Result<Event> {
    let config = CaldavRemoteConfig::try_from(&cmd.remote_config)?;
    let session = Session::load(&context, &config.caldav_account)?;
    let (username, password) = session.credentials();

    ops::create_event(username, password, &config.caldav_calendar_url, cmd.event).await
}
