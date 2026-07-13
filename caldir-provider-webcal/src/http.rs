//! Thin HTTP wrapper for fetching ICS feeds.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use reqwest::header::LAST_MODIFIED;

const USER_AGENT: &str = "caldir-provider-webcal";
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

pub struct FeedResponse {
    pub body: String,
    pub last_modified: Option<DateTime<Utc>>,
}

pub async fn fetch_feed(url: &str) -> Result<FeedResponse> {
    let client = reqwest::Client::builder()
        .timeout(TIMEOUT)
        .user_agent(USER_AGENT)
        .build()
        .context("Failed to build HTTP client")?;

    let response = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("Failed to fetch {url}"))?;

    if !response.status().is_success() {
        anyhow::bail!("Failed to fetch {url}: HTTP {}", response.status());
    }

    let last_modified = response
        .headers()
        .get(LAST_MODIFIED)
        .and_then(|value| value.to_str().ok())
        .and_then(parse_last_modified);

    let body = response
        .text()
        .await
        .with_context(|| format!("Failed to read response body from {url}"))?;

    Ok(FeedResponse {
        body,
        last_modified,
    })
}

fn parse_last_modified(value: &str) -> Option<DateTime<Utc>> {
    httpdate::parse_http_date(value)
        .ok()
        .map(DateTime::<Utc>::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_http_last_modified() {
        let parsed = parse_last_modified("Mon, 13 Jul 2026 06:00:11 GMT").unwrap();
        assert_eq!(parsed.to_rfc3339(), "2026-07-13T06:00:11+00:00");
    }
}
