//! HTTP client for communicating with caldir-server

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::time::Duration;

use caldir_lib::diff::EventDiff;
use caldir_lib::{Event, EventTime};

const SERVER_URL: &str = "http://127.0.0.1:4096";
const MAX_RETRIES: u32 = 10;
const RETRY_DELAY_MS: u64 = 200;

/// HTTP client for caldir-server
pub struct Client {
    http: reqwest::Client,
    base_url: String,
}

// Response types matching server API

#[derive(Deserialize)]
pub struct CalendarInfo {
    pub name: String,
    pub path: String,
    pub has_remote: bool,
}

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

#[derive(Serialize)]
pub struct CreateEventRequest {
    pub summary: String,
    pub start: EventTime,
    pub end: EventTime,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
}

#[derive(Deserialize)]
pub struct ErrorResponse {
    pub error: String,
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

    /// GET /calendars
    pub async fn list_calendars(&self) -> Result<Vec<CalendarInfo>> {
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

        Ok(resp.json().await?)
    }

    /// GET /calendars/:id/events
    pub async fn list_events(&self, calendar: &str) -> Result<Vec<Event>> {
        let resp = self
            .http
            .get(format!("{}/calendars/{}/events", self.base_url, calendar))
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
    pub async fn create_event(&self, calendar: &str, req: CreateEventRequest) -> Result<Event> {
        let resp = self
            .http
            .post(format!("{}/calendars/{}/events", self.base_url, calendar))
            .json(&req)
            .send()
            .await
            .context("Failed to connect to server")?;

        if !resp.status().is_success() {
            let err: ErrorResponse = resp.json().await?;
            anyhow::bail!("{}", err.error);
        }

        Ok(resp.json().await?)
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
}

/// Start the caldir-server process
fn start_server() -> Result<()> {
    Command::new("caldir-server")
        .spawn()
        .context("Failed to start caldir-server. Is it installed?")?;
    Ok(())
}
