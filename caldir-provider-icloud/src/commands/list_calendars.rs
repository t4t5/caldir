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
    let color = cal.color.map(|c| normalize_color(&c));

    let params = ICloudRemoteConfig::new(account_id, &cal.url).into_remote_config_params();
    let remote_config = RemoteConfig::new(ProviderSlug::from(PROVIDER_NAME), params);

    CalendarConfig::new(Some(cal.name), color, cal.read_only, Some(remote_config))
}

/// iCloud returns colors as `#RRGGBBAA`. Strip the alpha so caldir
/// stores the standard `#RRGGBB` form.
fn normalize_color(color: &str) -> String {
    if color.len() == 9 && color.starts_with('#') {
        color[..7].to_string()
    } else {
        color.to_string()
    }
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
    fn normalize_color_strips_alpha_from_rrggbbaa() {
        assert_eq!(normalize_color("#0099ffaa"), "#0099ff");
    }

    #[test]
    fn normalize_color_passes_through_rrggbb() {
        assert_eq!(normalize_color("#0099ff"), "#0099ff");
    }

    #[test]
    fn normalize_color_passes_through_unrecognized() {
        // Anything that's not `#` + 8 hex chars is left alone.
        assert_eq!(normalize_color("blue"), "blue");
        assert_eq!(normalize_color("#abc"), "#abc");
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
