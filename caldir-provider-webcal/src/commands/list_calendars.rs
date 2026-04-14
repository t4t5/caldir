//! List the single calendar represented by a webcal subscription.

use anyhow::Result;
use caldir_core::calendar::config::CalendarConfig;
use caldir_core::remote::{Remote, protocol::ListCalendars, provider::Provider};

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

    let remote_config = WebcalRemoteConfig::new(url);
    let remote = Remote::new(Provider::from_name(PROVIDER_NAME), remote_config.into());

    let config = CalendarConfig {
        name: Some(name),
        color,
        read_only: Some(true),
        remote: Some(remote),
        groups: vec![],
    };

    Ok(vec![config])
}

/// Extract a top-level ICS property value by simple string search.
fn extract_property(ics_body: &str, property: &str) -> Option<String> {
    let prefix = format!("{}:", property);
    for line in ics_body.lines() {
        if let Some(value) = line.strip_prefix(&prefix) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}
