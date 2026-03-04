//! List CalDAV calendars for a given account.

use anyhow::Result;
use caldir_core::calendar::config::CalendarConfig;
use caldir_core::remote::{protocol::ListCalendars, provider::Provider, Remote};
use caldir_provider_caldav::ops;

use crate::constants::PROVIDER_NAME;
use crate::remote_config::CaldavRemoteConfig;
use crate::session::Session;

pub async fn handle(cmd: ListCalendars) -> Result<Vec<CalendarConfig>> {
    let session = Session::load(&cmd.account_identifier)?;
    let (username, password) = session.credentials();

    let raw_calendars =
        ops::list_calendars_raw(username, password, &session.calendar_home_url).await?;

    let account_id = Session::account_identifier(&session.username, &session.server_url);

    let configs = raw_calendars
        .into_iter()
        .map(|cal| {
            let remote_config = CaldavRemoteConfig::new(&account_id, &cal.url);
            let remote = Remote::new(Provider::from_name(PROVIDER_NAME), remote_config.into());

            CalendarConfig {
                name: Some(cal.name),
                color: cal.color,
                read_only: None,
                remote: Some(remote),
            }
        })
        .collect();

    Ok(configs)
}
