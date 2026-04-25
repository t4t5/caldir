//! List iCloud Calendars (name + config) for a given account.

use anyhow::Result;
use caldir_core::calendar::config::CalendarConfig;
use caldir_core::remote::{Remote, protocol::ListCalendars, provider::Provider};
use caldir_provider_caldav::ops;

use crate::remote_config::ICloudRemoteConfig;
use crate::session::Session;

pub async fn handle(cmd: ListCalendars) -> Result<Vec<CalendarConfig>> {
    let session = Session::load(&cmd.account_identifier)?;
    let (username, password) = session.credentials();

    let raw_calendars =
        ops::list_calendars_raw(username, password, &session.calendar_home_url).await?;

    let configs = raw_calendars
        .into_iter()
        .map(|cal| {
            // iCloud returns colors as #RRGGBBAA, convert to #RRGGBB
            let color = cal.color.map(|c| {
                if c.len() == 9 && c.starts_with('#') {
                    c[..7].to_string()
                } else {
                    c
                }
            });

            let remote_config = ICloudRemoteConfig::new(&session.apple_id, &cal.url);
            let remote = Remote::new(Provider::from_name("icloud"), remote_config.into());

            CalendarConfig {
                name: Some(cal.name),
                color,
                read_only: cal.read_only,
                remote: Some(remote),
            }
        })
        .collect();

    Ok(configs)
}
