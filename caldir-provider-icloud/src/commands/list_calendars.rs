//! List iCloud Calendars (name + config) for a given account.

use anyhow::Result;
use caldir_core::provider::ProviderStorage;
use caldir_core::rpc::ListCalendars;
use caldir_core::{CalendarConfig, ProviderSlug, RemoteConfig};
use caldir_provider_caldav::caldav::ops::{self, RawCalendar};

use crate::constants::PROVIDER_NAME;
use crate::remote_config::ICloudRemoteConfig;
use crate::session::SessionStore;

pub async fn handle(cmd: ListCalendars) -> Result<Vec<CalendarConfig>> {
    let store = SessionStore::new(ProviderStorage::for_provider(PROVIDER_NAME)?);
    let session = store.load(&cmd.account_identifier)?;
    let (username, password) = session.credentials();

    let raw_calendars =
        ops::list_calendars_raw(username, password, &session.calendar_home_url).await?;

    Ok(raw_calendars
        .into_iter()
        .map(|cal| raw_to_config(&session.apple_id, cal))
        .collect())
}

/// Build a caldir CalendarConfig from a raw CalDAV calendar entry.
///
/// Pure transformation — no IO — so it can be unit-tested without a server.
fn raw_to_config(account_id: &str, cal: RawCalendar) -> CalendarConfig {
    // iCloud returns colors as `#RRGGBBAA` — strip the alpha so caldir
    // stores the standard `#RRGGBB` form.
    let color = cal.color.map(|c| {
        if c.len() == 9 && c.starts_with('#') {
            c[..7].to_string()
        } else {
            c
        }
    });

    let params = ICloudRemoteConfig::new(account_id, &cal.url).into_remote_config_params();
    let remote_config = RemoteConfig::new(ProviderSlug::from(PROVIDER_NAME), params);

    CalendarConfig::new(Some(cal.name), color, cal.read_only, Some(remote_config))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(name: &str, url: &str, color: Option<&str>, read_only: Option<bool>) -> RawCalendar {
        RawCalendar {
            href: "/123/calendars/home/cal-1/".to_string(),
            name: name.to_string(),
            color: color.map(str::to_string),
            url: url.to_string(),
            read_only,
        }
    }

    #[test]
    fn normalizes_rrggbbaa_color_to_rrggbb() {
        let cfg = raw_to_config(
            "me@icloud.com",
            raw(
                "Personal",
                "https://p01-caldav.icloud.com/123/calendars/personal/",
                Some("#0099ffaa"),
                Some(false),
            ),
        );
        assert_eq!(cfg.color(), Some("#0099ff"));
    }

    #[test]
    fn passes_through_short_color_unchanged() {
        let cfg = raw_to_config(
            "me@icloud.com",
            raw(
                "Personal",
                "https://p01-caldav.icloud.com/123/calendars/personal/",
                Some("#0099ff"),
                None,
            ),
        );
        assert_eq!(cfg.color(), Some("#0099ff"));
    }

    #[test]
    fn passes_through_no_color() {
        let cfg = raw_to_config(
            "me@icloud.com",
            raw(
                "Personal",
                "https://p01-caldav.icloud.com/123/calendars/personal/",
                None,
                None,
            ),
        );
        assert_eq!(cfg.color(), None);
    }

    #[test]
    fn carries_name_and_read_only() {
        let cfg = raw_to_config(
            "me@icloud.com",
            raw(
                "Work",
                "https://p01-caldav.icloud.com/123/calendars/work/",
                None,
                Some(true),
            ),
        );
        assert_eq!(cfg.name(), Some("Work"));
        assert_eq!(cfg.read_only(), Some(true));
    }

    #[test]
    fn remote_config_uses_icloud_field_names_and_slug() {
        let cfg = raw_to_config(
            "me@icloud.com",
            raw(
                "Personal",
                "https://p01-caldav.icloud.com/123/calendars/personal/",
                None,
                None,
            ),
        );

        let remote = cfg.remote_config().unwrap();
        assert_eq!(remote.provider_slug().to_string(), PROVIDER_NAME);
        assert_eq!(
            remote.get("icloud_account").and_then(|v| v.as_str()),
            Some("me@icloud.com")
        );
        assert_eq!(
            remote.get("icloud_calendar_url").and_then(|v| v.as_str()),
            Some("https://p01-caldav.icloud.com/123/calendars/personal/")
        );
    }
}
