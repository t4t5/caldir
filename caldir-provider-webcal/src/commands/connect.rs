//! Handle the connect flow for webcal subscriptions.
//!
//! Single credential field: the ICS feed URL.
//! On submit: fetches the URL once, validates it's ICS, and returns the
//! resulting calendar directly in `Done` — webcal is single-calendar, so the
//! CLI never needs to call `list_calendars`.

use anyhow::Result;
use caldir_core::rpc::{
    Connect, ConnectResponse, ConnectStepKind, CredentialField, CredentialsData, FieldType,
};
use caldir_core::{CalendarConfig, ProviderSlug, RemoteConfig};

use crate::constants::PROVIDER_NAME;
use crate::remote_config::WebcalRemoteConfig;

pub async fn handle(cmd: Connect) -> Result<ConnectResponse> {
    // If data contains the URL, this is the submit step.
    if cmd.data.contains_key("url") {
        let raw_url = cmd
            .data
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'url' in credentials"))?;

        // Normalize webcal:// to https://
        let url = raw_url.replacen("webcal://", "https://", 1);

        // Fetch the ICS feed to validate it
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("caldir-provider-webcal")
            .build()?;

        let response = client.get(&url).send().await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to fetch calendar URL: HTTP {}", response.status());
        }

        let body = response.text().await?;

        if !body.contains("BEGIN:VCALENDAR") {
            anyhow::bail!(
                "The URL does not appear to be a valid ICS calendar feed (no BEGIN:VCALENDAR found)"
            );
        }

        let calendar_config = build_calendar_config(&body, &url)?;

        return Ok(ConnectResponse::Done {
            account_identifier: None,
            calendars: Some(vec![calendar_config]),
        });
    }

    // Init step: return credential field requirements
    let fields = vec![CredentialField {
        id: "url".to_string(),
        label: "Calendar URL".to_string(),
        field_type: FieldType::Url,
        required: true,
        help: Some("URL to an .ics calendar feed (webcal:// or https://)".to_string()),
    }];

    let creds_data = CredentialsData { fields };

    Ok(ConnectResponse::NeedsInput {
        step: ConnectStepKind::Credentials,
        data: serde_json::to_value(creds_data)?,
    })
}

fn build_calendar_config(body: &str, url: &str) -> Result<CalendarConfig> {
    let cal: icalendar::Calendar = body
        .parse()
        .map_err(|e| anyhow::anyhow!("Failed to parse ICS feed: {e}"))?;

    let name = cal.get_name().map(str::to_string).unwrap_or_else(|| {
        url::Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(|h| h.to_string()))
            .unwrap_or_else(|| "Webcal".to_string())
    });

    let color = cal
        .property_value("X-APPLE-CALENDAR-COLOR")
        .map(str::to_string);

    let params = WebcalRemoteConfig::new(url).into_remote_config_params();
    let remote_config = RemoteConfig::new(ProviderSlug::from(PROVIDER_NAME), params);

    Ok(CalendarConfig::new(
        Some(name),
        color,
        Some(true),
        Some(remote_config),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ics(properties: &str) -> String {
        format!("BEGIN:VCALENDAR\nVERSION:2.0\n{properties}END:VCALENDAR\n").replace('\n', "\r\n")
    }

    fn expected(name: &str, color: Option<&str>, url: &str) -> CalendarConfig {
        let params = WebcalRemoteConfig::new(url).into_remote_config_params();
        let remote_config = RemoteConfig::new(ProviderSlug::from(PROVIDER_NAME), params);
        CalendarConfig::new(
            Some(name.to_string()),
            color.map(str::to_string),
            Some(true),
            Some(remote_config),
        )
    }

    #[test]
    fn name_comes_from_x_wr_calname() {
        let body = ics("X-WR-CALNAME:Bank Holidays\n");
        let url = "https://example.com/cal.ics";

        let config = build_calendar_config(&body, url).unwrap();

        assert_eq!(config, expected("Bank Holidays", None, url));
    }

    #[test]
    fn name_falls_back_to_url_host_when_calendar_unnamed() {
        let body = ics("");
        let url = "https://feeds.example.com/holidays.ics";

        let config = build_calendar_config(&body, url).unwrap();

        assert_eq!(config.name(), Some("feeds.example.com"));
    }

    #[test]
    fn name_falls_back_to_literal_webcal_when_url_unparseable() {
        let body = ics("");
        let url = "not a url";

        let config = build_calendar_config(&body, url).unwrap();

        assert_eq!(config.name(), Some("Webcal"));
    }

    #[test]
    fn color_comes_from_x_apple_calendar_color() {
        let body = ics("X-WR-CALNAME:Holidays\nX-APPLE-CALENDAR-COLOR:#FF5733\n");
        let url = "https://example.com/cal.ics";

        let config = build_calendar_config(&body, url).unwrap();

        assert_eq!(config, expected("Holidays", Some("#FF5733"), url));
    }

    #[test]
    fn remote_config_carries_webcal_url_and_provider_slug() {
        let body = ics("X-WR-CALNAME:Holidays\n");
        let url = "https://example.com/cal.ics";

        let config = build_calendar_config(&body, url).unwrap();

        let remote = config.remote_config().unwrap();
        assert_eq!(remote.provider_slug().to_string(), PROVIDER_NAME);
        assert_eq!(remote.get("webcal_url").and_then(|v| v.as_str()), Some(url));
    }

    #[test]
    fn read_only_is_true() {
        let body = ics("X-WR-CALNAME:Holidays\n");

        let config = build_calendar_config(&body, "https://example.com/cal.ics").unwrap();

        assert_eq!(config.read_only(), Some(true));
    }
}
