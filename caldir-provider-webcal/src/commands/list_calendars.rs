//! List the single calendar represented by a webcal subscription.

use anyhow::Result;
use caldir_core::rpc::ListCalendars;
use caldir_core::{CalendarConfig, ProviderSlug, RemoteConfig};

use crate::constants::PROVIDER_NAME;
use crate::remote_config::WebcalRemoteConfig;

pub async fn handle(cmd: ListCalendars) -> Result<Vec<CalendarConfig>> {
    // The account_identifier IS the URL for webcal subscriptions
    let url = &cmd.account_identifier;

    // Fetch the ICS feed to extract calendar metadata
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("caldir-provider-webcal")
        .build()?;

    let response = client.get(url).send().await?;

    if !response.status().is_success() {
        anyhow::bail!("Failed to fetch webcal feed: HTTP {}", response.status());
    }

    let body = response.text().await?;

    let display_name = extract_property(&body, "X-WR-CALNAME");
    let color = extract_property(&body, "X-APPLE-CALENDAR-COLOR");

    // Fall back to the URL host for the display name
    let name = display_name.unwrap_or_else(|| {
        url::Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(|h| h.to_string()))
            .unwrap_or_else(|| "Webcal".to_string())
    });

    let params = WebcalRemoteConfig::new(url).into_remote_config_params();
    let remote_config = RemoteConfig::new(ProviderSlug::from(PROVIDER_NAME), params);

    let config = CalendarConfig::new(Some(name), color, Some(true), Some(remote_config));

    Ok(vec![config])
}

/// Extract a top-level ICS property value by simple string search.
///
/// Handles RFC 5545 line folding: continuation lines start with a single
/// space or tab and are appended (after stripping the leading whitespace)
/// to the previous logical line.
fn extract_property(ics_body: &str, property: &str) -> Option<String> {
    let prefix = format!("{property}:");
    let mut lines = ics_body.lines();

    while let Some(raw) = lines.next() {
        let Some(value_start) = raw.strip_prefix(&prefix) else {
            continue;
        };

        let mut value = value_start.to_string();
        // Pull in any continuation lines (RFC 5545 line folding).
        for cont in lines.by_ref() {
            if let Some(rest) = cont.strip_prefix(' ').or_else(|| cont.strip_prefix('\t')) {
                value.push_str(rest);
            } else {
                break;
            }
        }

        let trimmed = value.trim();
        if trimmed.is_empty() {
            return None;
        }
        return Some(trimmed.to_string());
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_property_returns_value_when_present() {
        let ics =
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nX-WR-CALNAME:Bank Holidays\r\nEND:VCALENDAR\r\n";
        assert_eq!(
            extract_property(ics, "X-WR-CALNAME"),
            Some("Bank Holidays".to_string())
        );
    }

    #[test]
    fn extract_property_returns_none_when_missing() {
        let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nEND:VCALENDAR\r\n";
        assert_eq!(extract_property(ics, "X-WR-CALNAME"), None);
    }

    #[test]
    fn extract_property_unfolds_continuation_lines() {
        // RFC 5545 line folding: long lines split with a leading space/tab
        // on the continuation. Common in real feeds when X-WR-CALNAME is long.
        let ics = "BEGIN:VCALENDAR\r\nX-WR-CALNAME:UK Government Bank Holidays\r\n England and Wales\r\nEND:VCALENDAR\r\n";
        assert_eq!(
            extract_property(ics, "X-WR-CALNAME"),
            Some("UK Government Bank HolidaysEngland and Wales".to_string())
        );
    }

    #[test]
    fn extract_property_handles_tab_continuation() {
        let ics = "BEGIN:VCALENDAR\r\nX-WR-CALNAME:Hello\r\n\tWorld\r\nEND:VCALENDAR\r\n";
        assert_eq!(
            extract_property(ics, "X-WR-CALNAME"),
            Some("HelloWorld".to_string())
        );
    }
}
