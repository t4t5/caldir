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

use crate::error::{CalDirError, CalDirResult};
use crate::protocol::{Command as ProviderCommand, Request, Response};
use crate::provider_account::ProviderAccount;
use serde::de::DeserializeOwned;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::timeout;

const PROVIDER_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone)]
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

    pub async fn authenticate(&self) -> CalDirResult<ProviderAccount> {
        let identifier: String = self
            .call(ProviderCommand::Authenticate, serde_json::json!({}))
            .await?;

        Ok(ProviderAccount::new(self.clone(), identifier))
    }

    /// Call a provider command and return the result.
    pub async fn call_with_timeout<R: DeserializeOwned>(
        &self,
        command: ProviderCommand,
        params: serde_json::Value,
    ) -> CalDirResult<R> {
        timeout(PROVIDER_TIMEOUT, self.call(command, params))
            .await
            .map_err(|_| CalDirError::ProviderTimeout(PROVIDER_TIMEOUT.as_secs()))?
    }

    pub async fn call<R: DeserializeOwned>(
        &self,
        command: ProviderCommand,
        params: serde_json::Value,
    ) -> CalDirResult<R> {
        let request = Request { command, params };
        let request_json = serde_json::to_string(&request)
            .map_err(|e| CalDirError::Serialization(e.to_string()))?;

        let binary_path = self.binary_path()?;

        let mut child = Command::new(&binary_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .map_err(|e| {
                CalDirError::Provider(format!("Failed to spawn {}: {}", binary_path.display(), e))
            })?;

        // Write request to stdin (unwrap safe: we piped stdin above)
        let mut stdin = child.stdin.take().unwrap();
        stdin
            .write_all(format!("{request_json}\n").as_bytes())
            .await?;
        drop(stdin);

        // Wait for process and collect output
        let output = child.wait_with_output().await?;

        if !output.status.success() {
            return Err(CalDirError::Provider(format!(
                "Provider exited with status: {}",
                output.status.code().unwrap_or(-1)
            )));
        }

        let response_str = String::from_utf8_lossy(&output.stdout);
        if response_str.is_empty() {
            return Err(CalDirError::Provider(
                "Provider returned no response".into(),
            ));
        }

        let response: Response<R> = serde_json::from_str(&response_str)
            .map_err(|e| CalDirError::Provider(format!("Failed to parse response: {}", e)))?;

        match response {
            Response::Success { data } => Ok(data),
            Response::Error { error } => Err(CalDirError::Provider(error)),
        }
    }
}
