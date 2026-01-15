//! HTTP client for communicating with caldir-server

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::time::Duration;

use caldir_core::diff::EventDiff;
use caldir_core::{Event, EventTime};

const SERVER_URL: &str = "http://127.0.0.1:4096";
const MAX_RETRIES: u32 = 10;
const RETRY_DELAY_MS: u64 = 200;

/// HTTP client for caldir-server
pub struct Client {
    http: reqwest::Client,
    base_url: String,
}

// Minimal response types - just enough to deserialize server responses
// The server defines the canonical types; these mirror the JSON structure

#[derive(Deserialize)]
pub struct SyncResult {
    pub calendar: String,
    pub events: Vec<EventDiff>,
    pub error: Option<String>,
}

#[derive(Deserialize)]
pub struct StatusResult {
    pub calendar: String,
    pub to_push: Vec<EventDiff>,
    pub to_pull: Vec<EventDiff>,
    pub error: Option<String>,
}

#[derive(Deserialize)]
pub struct AuthResponse {
    pub account: String,
    pub calendars_created: Vec<String>,
    pub calendars_existing: Vec<String>,
}

#[derive(Deserialize)]
struct ErrorResponse {
    error: String,
}

impl Client {
    /// Connect to existing server or start one
    pub async fn connect() -> Result<Self> {
        let http = reqwest::Client::new();
        let client = Self {
            http,
            base_url: SERVER_URL.to_string(),
        };

        // Try to connect to existing server
        if client.health_check().await.is_ok() {
            return Ok(client);
        }

        // Server not running - start it
        start_server()?;

        // Wait for server to be ready
        for _ in 0..MAX_RETRIES {
            tokio::time::sleep(Duration::from_millis(RETRY_DELAY_MS)).await;
            if client.health_check().await.is_ok() {
                return Ok(client);
            }
        }

        anyhow::bail!("Failed to connect to caldir-server after starting it")
    }

    async fn health_check(&self) -> Result<()> {
        self.http
            .get(format!("{}/calendars", self.base_url))
            .timeout(Duration::from_secs(2))
            .send()
            .await?;
        Ok(())
    }

    /// POST /remote/pull
    pub async fn pull(&self) -> Result<Vec<SyncResult>> {
        let resp = self
            .http
            .post(format!("{}/remote/pull", self.base_url))
            .send()
            .await
            .context("Failed to connect to server")?;

        if !resp.status().is_success() {
            let err: ErrorResponse = resp.json().await?;
            anyhow::bail!("{}", err.error);
        }

        Ok(resp.json().await?)
    }

    /// POST /remote/push
    pub async fn push(&self) -> Result<Vec<SyncResult>> {
        let resp = self
            .http
            .post(format!("{}/remote/push", self.base_url))
            .send()
            .await
            .context("Failed to connect to server")?;

        if !resp.status().is_success() {
            let err: ErrorResponse = resp.json().await?;
            anyhow::bail!("{}", err.error);
        }

        Ok(resp.json().await?)
    }

    /// GET /remote/status
    pub async fn status(&self) -> Result<Vec<StatusResult>> {
        let resp = self
            .http
            .get(format!("{}/remote/status", self.base_url))
            .send()
            .await
            .context("Failed to connect to server")?;

        if !resp.status().is_success() {
            let err: ErrorResponse = resp.json().await?;
            anyhow::bail!("{}", err.error);
        }

        Ok(resp.json().await?)
    }

    /// POST /auth/:provider
    pub async fn authenticate(&self, provider: &str) -> Result<AuthResponse> {
        let resp = self
            .http
            .post(format!("{}/auth/{}", self.base_url, provider))
            .send()
            .await
            .context("Failed to connect to server")?;

        if !resp.status().is_success() {
            let err: ErrorResponse = resp.json().await?;
            anyhow::bail!("{}", err.error);
        }

        Ok(resp.json().await?)
    }

    /// POST /calendars/:id/events
    pub async fn create_event(
        &self,
        calendar: &str,
        summary: String,
        start: EventTime,
        end: EventTime,
    ) -> Result<Event> {
        #[derive(Serialize)]
        struct Request {
            summary: String,
            start: EventTime,
            end: EventTime,
        }

        let resp = self
            .http
            .post(format!("{}/calendars/{}/events", self.base_url, calendar))
            .json(&Request { summary, start, end })
            .send()
            .await
            .context("Failed to connect to server")?;

        if !resp.status().is_success() {
            let err: ErrorResponse = resp.json().await?;
            anyhow::bail!("{}", err.error);
        }

        Ok(resp.json().await?)
    }

    /// GET /calendars - returns calendar names
    pub async fn list_calendars(&self) -> Result<Vec<String>> {
        #[derive(Deserialize)]
        struct CalendarInfo {
            name: String,
        }

        let resp = self
            .http
            .get(format!("{}/calendars", self.base_url))
            .send()
            .await
            .context("Failed to connect to server")?;

        if !resp.status().is_success() {
            let err: ErrorResponse = resp.json().await?;
            anyhow::bail!("{}", err.error);
        }

        let calendars: Vec<CalendarInfo> = resp.json().await?;
        Ok(calendars.into_iter().map(|c| c.name).collect())
    }

    /// GET /calendars - returns the default calendar name
    pub async fn default_calendar(&self) -> Result<Option<String>> {
        #[derive(Deserialize)]
        struct CalendarInfo {
            name: String,
            is_default: bool,
        }

        let resp = self
            .http
            .get(format!("{}/calendars", self.base_url))
            .send()
            .await
            .context("Failed to connect to server")?;

        if !resp.status().is_success() {
            let err: ErrorResponse = resp.json().await?;
            anyhow::bail!("{}", err.error);
        }

        let calendars: Vec<CalendarInfo> = resp.json().await?;
        Ok(calendars.into_iter().find(|c| c.is_default).map(|c| c.name))
    }
}

/// Start the caldir-server process
fn start_server() -> Result<()> {
    Command::new("caldir-server")
        .spawn()
        .context("Failed to start caldir-server. Is it installed? Run 'just install'")?;
    Ok(())
}
