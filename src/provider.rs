//! Provider subprocess protocol.
//!
//! This module handles communication with external provider binaries
//! (e.g., `caldir-provider-google`) using JSON over stdin/stdout.
//!
//! The protocol is designed to be language-agnostic: any executable
//! that speaks the JSON protocol can be a provider.

use crate::config::AccountTokens;
use crate::event::Event;
use anyhow::{Context, Result};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

// =============================================================================
// Protocol Types
// =============================================================================

/// A calendar from a provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Calendar {
    pub id: String,
    pub name: String,
    pub primary: bool,
}

/// Request sent to a provider subprocess
#[derive(Debug, Serialize)]
struct ProviderRequest<'a, P: Serialize> {
    command: &'a str,
    config: &'a serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    tokens: Option<&'a AccountTokens>,
    params: P,
}

/// Response from a provider subprocess
#[derive(Debug, Deserialize)]
#[serde(tag = "status")]
enum ProviderResponse<T> {
    #[serde(rename = "success")]
    Success { data: T },
    #[serde(rename = "error")]
    Error { error: String },
    #[serde(rename = "tokens_updated")]
    TokensUpdated { tokens: AccountTokens, data: T },
}

/// Empty params for commands that don't need parameters
#[derive(Debug, Serialize)]
struct EmptyParams {}

/// Parameters for fetch_events command
#[derive(Debug, Serialize)]
struct FetchEventsParams<'a> {
    calendar_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    time_min: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    time_max: Option<String>,
}

/// Parameters for create_event command
#[derive(Debug, Serialize)]
struct CreateEventParams<'a> {
    calendar_id: &'a str,
    event: &'a Event,
}

/// Parameters for update_event command
#[derive(Debug, Serialize)]
struct UpdateEventParams<'a> {
    calendar_id: &'a str,
    event: &'a Event,
}

/// Parameters for delete_event command
#[derive(Debug, Serialize)]
struct DeleteEventParams<'a> {
    calendar_id: &'a str,
    event_id: &'a str,
}

// =============================================================================
// Provider Client
// =============================================================================

/// A client for communicating with a provider subprocess
pub struct Provider {
    binary_path: PathBuf,
    config: serde_json::Value,
}

impl Provider {
    /// Create a new provider client.
    ///
    /// Looks for an executable named `caldir-provider-{name}` in PATH.
    /// The config is passed to every request (e.g., OAuth credentials).
    pub fn new(name: &str, config: serde_json::Value) -> Result<Self> {
        let binary_name = format!("caldir-provider-{}", name);
        let binary_path = which::which(&binary_name).with_context(|| {
            format!(
                "Provider '{}' not found. Install it with:\n  cargo install {}",
                name, binary_name
            )
        })?;

        Ok(Self {
            binary_path,
            config,
        })
    }

    /// Call a provider command and return the result.
    ///
    /// Returns (data, Option<updated_tokens>). If tokens were refreshed
    /// during the request, the new tokens are returned.
    async fn call<P: Serialize, R: DeserializeOwned>(
        &self,
        command: &str,
        tokens: Option<&AccountTokens>,
        params: P,
    ) -> Result<(R, Option<AccountTokens>)> {
        let request = ProviderRequest {
            command,
            config: &self.config,
            tokens,
            params,
        };

        let request_json =
            serde_json::to_string(&request).context("Failed to serialize provider request")?;

        // Spawn the provider process
        let mut child = Command::new(&self.binary_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit()) // Let provider errors show in terminal
            .spawn()
            .with_context(|| {
                format!(
                    "Failed to spawn provider: {}",
                    self.binary_path.display()
                )
            })?;

        // Write request to stdin
        {
            let mut stdin = child.stdin.take().unwrap();
            stdin
                .write_all(request_json.as_bytes())
                .await
                .context("Failed to write to provider stdin")?;
            stdin
                .write_all(b"\n")
                .await
                .context("Failed to write newline to provider stdin")?;
            stdin.flush().await.context("Failed to flush provider stdin")?;
            // Drop stdin to signal EOF
        }

        // Read response from stdout
        let stdout = child.stdout.take().unwrap();
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .await
            .context("Failed to read provider response")?;

        if line.is_empty() {
            anyhow::bail!("Provider returned no response");
        }

        // Wait for process to exit
        let status = child.wait().await.context("Failed to wait for provider")?;
        if !status.success() {
            anyhow::bail!(
                "Provider exited with status: {}",
                status.code().unwrap_or(-1)
            );
        }

        // Parse response
        let response: ProviderResponse<R> =
            serde_json::from_str(&line).with_context(|| {
                format!("Failed to parse provider response: {}", line)
            })?;

        match response {
            ProviderResponse::Success { data } => Ok((data, None)),
            ProviderResponse::TokensUpdated { tokens, data } => Ok((data, Some(tokens))),
            ProviderResponse::Error { error } => Err(anyhow::anyhow!("{}", error)),
        }
    }

    // =========================================================================
    // Provider Commands
    // =========================================================================

    /// Authenticate with the provider (OAuth flow).
    ///
    /// Opens browser, waits for callback, returns tokens.
    pub async fn authenticate(&self) -> Result<AccountTokens> {
        let (tokens, _): (AccountTokens, _) =
            self.call("authenticate", None, EmptyParams {}).await?;
        Ok(tokens)
    }

    /// Refresh expired access token.
    pub async fn refresh_token(&self, tokens: &AccountTokens) -> Result<AccountTokens> {
        let (new_tokens, _): (AccountTokens, _) =
            self.call("refresh_token", Some(tokens), EmptyParams {}).await?;
        Ok(new_tokens)
    }

    /// Get the authenticated user's email address.
    pub async fn fetch_user_email(&self, tokens: &AccountTokens) -> Result<(String, Option<AccountTokens>)> {
        self.call("fetch_user_email", Some(tokens), EmptyParams {}).await
    }

    /// List all calendars for the authenticated account.
    pub async fn fetch_calendars(&self, tokens: &AccountTokens) -> Result<(Vec<Calendar>, Option<AccountTokens>)> {
        self.call("fetch_calendars", Some(tokens), EmptyParams {}).await
    }

    /// Fetch events from a calendar.
    ///
    /// The time_min and time_max parameters are optional ISO 8601 timestamps.
    pub async fn fetch_events(
        &self,
        tokens: &AccountTokens,
        calendar_id: &str,
        time_min: Option<&str>,
        time_max: Option<&str>,
    ) -> Result<(Vec<Event>, Option<AccountTokens>)> {
        self.call(
            "fetch_events",
            Some(tokens),
            FetchEventsParams {
                calendar_id,
                time_min: time_min.map(String::from),
                time_max: time_max.map(String::from),
            },
        )
        .await
    }

    /// Create a new event on a calendar.
    ///
    /// Returns the created event with provider-assigned ID.
    pub async fn create_event(
        &self,
        tokens: &AccountTokens,
        calendar_id: &str,
        event: &Event,
    ) -> Result<(Event, Option<AccountTokens>)> {
        self.call(
            "create_event",
            Some(tokens),
            CreateEventParams { calendar_id, event },
        )
        .await
    }

    /// Update an existing event.
    pub async fn update_event(
        &self,
        tokens: &AccountTokens,
        calendar_id: &str,
        event: &Event,
    ) -> Result<((), Option<AccountTokens>)> {
        self.call(
            "update_event",
            Some(tokens),
            UpdateEventParams { calendar_id, event },
        )
        .await
    }

    /// Delete an event.
    pub async fn delete_event(
        &self,
        tokens: &AccountTokens,
        calendar_id: &str,
        event_id: &str,
    ) -> Result<((), Option<AccountTokens>)> {
        self.call(
            "delete_event",
            Some(tokens),
            DeleteEventParams {
                calendar_id,
                event_id,
            },
        )
        .await
    }
}
