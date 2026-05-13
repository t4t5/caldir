//! Thin HTTP wrapper for fetching ICS feeds.

use anyhow::{Context, Result};

const USER_AGENT: &str = "caldir-provider-webcal";
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Fetch the body of an ICS feed.
///
/// Errors if the HTTP request fails or the server returns a non-2xx status.
/// The body is returned verbatim — parsing/validation is the caller's job.
pub async fn fetch_feed(url: &str) -> Result<String> {
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

    response
        .text()
        .await
        .with_context(|| format!("Failed to read response body from {url}"))
}
