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

use crate::calendar_config::CalendarConfig;
use crate::error::{CalDirError, CalDirResult};
use crate::protocol::{Command as ProviderCommand, Request, Response};
use serde::de::DeserializeOwned;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::time::timeout;

const PROVIDER_TIMEOUT: Duration = Duration::from_secs(10);

pub struct Provider(String);

impl Provider {
    pub fn from_name(name: &str) -> Self {
        Provider(name.to_string())
    }

    pub fn name(&self) -> &str {
        &self.0
    }

    fn binary_path(&self) -> CalDirResult<std::path::PathBuf> {
        let binary_name = format!("caldir-provider-{}", self.0);
        let binary_path = which::which(&binary_name).map_err(|_| {
            CalDirError::ProviderNotInstalled(format!(
                "Provider '{}' not found. Install it with:\n  cargo install {}",
                self.0, binary_name
            ))
        })?;
        Ok(binary_path)
    }

    pub async fn authenticate(&self) -> CalDirResult<String> {
        self.call_inner(ProviderCommand::Authenticate, serde_json::json!({}))
            .await
    }

    /// List all calendars for an account
    pub async fn list_calendars(&self, account: &str) -> CalDirResult<Vec<CalendarConfig>> {
        let param_key = format!("{}_account", self.0);
        let params = serde_json::json!({ param_key: account });
        self.call(ProviderCommand::ListCalendars, params).await
    }

    /// Call a provider command and return the result.
    pub async fn call<R: DeserializeOwned>(
        &self,
        command: ProviderCommand,
        params: serde_json::Value,
    ) -> CalDirResult<R> {
        timeout(PROVIDER_TIMEOUT, self.call_inner(command, params))
            .await
            .map_err(|_| CalDirError::ProviderTimeout(PROVIDER_TIMEOUT.as_secs()))?
    }

    async fn call_inner<R: DeserializeOwned>(
        &self,
        command: ProviderCommand,
        params: serde_json::Value,
    ) -> CalDirResult<R> {
        let request = Request { command, params };

        let request_json = serde_json::to_string(&request)
            .map_err(|e| CalDirError::Serialization(e.to_string()))?;

        let binary_path = self.binary_path()?;

        // Spawn the provider process
        let mut child = Command::new(&binary_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit()) // Let provider errors show in terminal
            .spawn()
            .map_err(|e| {
                CalDirError::Provider(format!(
                    "Failed to spawn provider {}: {}",
                    binary_path.display(),
                    e
                ))
            })?;

        // Write request to stdin
        {
            let mut stdin = child
                .stdin
                .take()
                .ok_or_else(|| CalDirError::Provider("Failed to get provider stdin handle".into()))?;
            stdin
                .write_all(request_json.as_bytes())
                .await
                .map_err(|e| CalDirError::Provider(format!("Failed to write to provider stdin: {}", e)))?;
            stdin
                .write_all(b"\n")
                .await
                .map_err(|e| CalDirError::Provider(format!("Failed to write newline: {}", e)))?;
            stdin
                .flush()
                .await
                .map_err(|e| CalDirError::Provider(format!("Failed to flush stdin: {}", e)))?;
            // Drop stdin to signal EOF
        }

        // Read response from stdout
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| CalDirError::Provider("Failed to get provider stdout handle".into()))?;
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .await
            .map_err(|e| CalDirError::Provider(format!("Failed to read provider response: {}", e)))?;

        if line.is_empty() {
            return Err(CalDirError::Provider("Provider returned no response".into()));
        }

        // Wait for process to exit
        let status = child
            .wait()
            .await
            .map_err(|e| CalDirError::Provider(format!("Failed to wait for provider: {}", e)))?;
        if !status.success() {
            return Err(CalDirError::Provider(format!(
                "Provider exited with status: {}",
                status.code().unwrap_or(-1)
            )));
        }

        // Parse response
        let response: Response<R> = serde_json::from_str(&line)
            .map_err(|e| CalDirError::Provider(format!("Failed to parse provider response: {}", e)))?;

        match response {
            Response::Success { data } => Ok(data),
            Response::Error { error } => Err(CalDirError::Provider(error)),
        }
    }
}
