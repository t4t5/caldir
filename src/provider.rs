//! Provider subprocess protocol.
//!
//! This module handles communication with external provider binaries
//! (e.g., `caldir-provider-google`) using JSON over stdin/stdout.
//!
//! The protocol is designed to be language-agnostic: any executable
//! that speaks the JSON protocol can be a provider.
//!
//! Providers manage their own credentials and tokens. Core just passes
//! provider-specific parameters from the calendar config.

use crate::event::Event;
use anyhow::{Context, Result};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

// =============================================================================
// Protocol Types
// =============================================================================

/// Request sent to a provider subprocess
#[derive(Debug, Serialize)]
struct ProviderRequest<'a, P: Serialize> {
    command: &'a str,
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
}

// =============================================================================
// Provider Client
// =============================================================================

/// A client for communicating with a provider subprocess.
///
/// Providers are discovered by looking for executables named
/// `caldir-provider-{name}` in PATH.
pub struct Provider {
    binary_path: PathBuf,
}

impl Provider {
    /// Create a new provider client.
    ///
    /// Looks for an executable named `caldir-provider-{name}` in PATH.
    pub fn new(name: &str) -> Result<Self> {
        let binary_name = format!("caldir-provider-{}", name);
        let binary_path = which::which(&binary_name).with_context(|| {
            format!(
                "Provider '{}' not found. Install it with:\n  cargo install {}",
                name, binary_name
            )
        })?;

        Ok(Self { binary_path })
    }

    /// Call a provider command and return the result.
    async fn call<P: Serialize, R: DeserializeOwned>(
        &self,
        command: &str,
        params: P,
    ) -> Result<R> {
        let request = ProviderRequest { command, params };

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
            ProviderResponse::Success { data } => Ok(data),
            ProviderResponse::Error { error } => Err(anyhow::anyhow!("{}", error)),
        }
    }

    // =========================================================================
    // Provider Commands
    // =========================================================================

    /// Authenticate with the provider.
    ///
    /// Provider handles the full auth flow (OAuth, etc.) and stores
    /// credentials/tokens in its own config directory.
    ///
    /// Returns the account identifier (e.g., email for Google).
    pub async fn authenticate(&self) -> Result<String> {
        self.call("authenticate", ()).await
    }

    /// Fetch events from a calendar.
    ///
    /// The params should include account and calendar identifiers.
    /// Additional params (time_min, time_max) can be merged in.
    pub async fn fetch_events(
        &self,
        params: serde_json::Value,
    ) -> Result<Vec<Event>> {
        self.call("fetch_events", params).await
    }

    /// Create a new event on a calendar.
    ///
    /// Returns the created event with provider-assigned ID.
    pub async fn create_event(
        &self,
        params: serde_json::Value,
    ) -> Result<Event> {
        self.call("create_event", params).await
    }

    /// Update an existing event.
    pub async fn update_event(
        &self,
        params: serde_json::Value,
    ) -> Result<()> {
        self.call("update_event", params).await
    }

    /// Delete an event.
    pub async fn delete_event(
        &self,
        params: serde_json::Value,
    ) -> Result<()> {
        self.call("delete_event", params).await
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Convert calendar config params to JSON and merge with additional params.
pub fn build_params(
    config_params: &HashMap<String, toml::Value>,
    additional: &[(&str, serde_json::Value)],
) -> serde_json::Value {
    let mut params = serde_json::Map::new();

    // Add config params (toml::Value implements Serialize)
    for (key, value) in config_params {
        if let Ok(json_value) = serde_json::to_value(value) {
            params.insert(key.clone(), json_value);
        }
    }

    // Add additional params
    for (key, value) in additional {
        params.insert((*key).to_string(), value.clone());
    }

    serde_json::Value::Object(params)
}
