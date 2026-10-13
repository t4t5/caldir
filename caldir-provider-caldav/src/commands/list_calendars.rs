//! List CalDAV calendars for a given account.

use anyhow::Result;
use caldir_core::provider::ProviderStorage;
use caldir_core::rpc::ListCalendars;
use caldir_core::{CalendarConfig, ProviderSlug, RemoteConfig};
use caldir_provider_caldav::caldav::ops::{self, RawCalendar};

use crate::constants::PROVIDER_NAME;
use crate::remote_config::CaldavRemoteConfig;
use crate::session::{Session, SessionStore};

pub async fn handle(cmd: ListCalendars) -> Result<Vec<CalendarConfig>> {
    let store = SessionStore::new(ProviderStorage::for_provider(PROVIDER_NAME)?);
    let session = store.load(&cmd.account_identifier)?;
    let (username, password) = session.credentials();

    let raw_calendars =
        ops::list_calendars_raw(username, password, &session.calendar_home_url).await?;

    let account_id = Session::account_identifier(&session.username, &session.server_url);

    // Best effort: without it, invites can't be matched but sync still works.
    let email = ops::find_user_email(username, password, &session.principal_url)
        .await
        .ok()
        .flatten();

    Ok(raw_calendars
        .into_iter()
        .map(|cal| raw_to_config(&account_id, email.as_deref(), cal))
        .collect())
}

/// Build a caldir CalendarConfig from a raw CalDAV calendar entry.
///
/// Pure transformation — no IO — so it can be unit-tested without a server.
fn raw_to_config(account_id: &str, email: Option<&str>, cal: RawCalendar) -> CalendarConfig {
    let params = CaldavRemoteConfig::new(account_id, &cal.url, email.map(str::to_string))
        .into_remote_config_params();
    let remote_config = RemoteConfig::new(ProviderSlug::from(PROVIDER_NAME), params);

    CalendarConfig::new(
        Some(cal.name),
        cal.color,
        cal.read_only,
        Some(remote_config),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(name: &str, url: &str, color: Option<&str>, read_only: Option<bool>) -> RawCalendar {
        RawCalendar {
            href: "/dav/cal/1/".to_string(),
            name: name.to_string(),
            color: color.map(str::to_string),
            url: url.to_string(),
            read_only,
        }
    }

    #[test]
    fn carries_name_and_read_only() {
        let cfg = raw_to_config(
            "me@fastmail.com",
            None,
            raw(
                "Personal",
                "https://server/cal/1/",
                Some("#0099ff"),
                Some(false),
            ),
        );

        assert_eq!(cfg.name(), Some("Personal"));
        assert_eq!(cfg.read_only(), Some(false));
    }

    #[test]
    fn remote_config_carries_account_url_and_provider_slug() {
        let cfg = raw_to_config(
            "me@fastmail.com",
            None,
            raw("Personal", "https://server/cal/1/", None, None),
        );

        let remote = cfg.remote_config().unwrap();
        assert_eq!(remote.provider_slug().to_string(), PROVIDER_NAME);
        assert_eq!(
            remote.get("caldav_account").and_then(|v| v.as_str()),
            Some("me@fastmail.com")
        );
        assert_eq!(
            remote.get("caldav_calendar_url").and_then(|v| v.as_str()),
            Some("https://server/cal/1/")
        );
    }

    #[test]
    fn read_only_unknown_passes_through_as_none() {
        let cfg = raw_to_config(
            "me@fastmail.com",
            None,
            raw("Personal", "https://server/cal/1/", None, None),
        );

        assert_eq!(cfg.read_only(), None);
    }

    // Regression: logins like Infomaniak's "AB12345" aren't the invitee address (rencal#157).
    #[test]
    fn account_email_uses_discovered_address_over_login() {
        let cfg = raw_to_config(
            "AB12345@sync.infomaniak.com",
            Some("alice@example.com"),
            raw("Personal", "https://server/cal/1/", None, None),
        );

        let remote = cfg.remote_config().unwrap();
        assert_eq!(remote.account_email(), Some("alice@example.com"));
    }
}
