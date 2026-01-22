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
use crate::remote::protocol::{
    AuthInit, AuthInitResponse, AuthSubmit, Command, ProviderCommand, Request, Response,
};
use crate::remote::provider_account::ProviderAccount;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command as TokioCommand;
use tokio::time::timeout;

const PROVIDER_TIMEOUT: Duration = Duration::from_secs(10);
/// No timeout for auth commands since they involve user interaction.
const AUTH_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Clone, Debug, Serialize, Deserialize)]
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

    /// Initialize authentication - provider returns what auth method it needs.
    pub async fn auth_init(&self, redirect_uri: Option<String>) -> CalDirResult<AuthInitResponse> {
        self.call_no_timeout(AuthInit { redirect_uri }).await
    }

    /// Submit gathered credentials to complete authentication.
    pub async fn auth_submit(
        &self,
        credentials: serde_json::Map<String, serde_json::Value>,
    ) -> CalDirResult<ProviderAccount> {
        let identifier = self.call_no_timeout(AuthSubmit { credentials }).await?;
        Ok(ProviderAccount::new(self.clone(), identifier))
    }

    /// Call a typed provider command and return the result.
    ///
    /// The response type is inferred from the command's associated type,
    /// ensuring compile-time type safety.
    pub async fn call<C: ProviderCommand>(&self, cmd: C) -> CalDirResult<C::Response> {
        timeout(PROVIDER_TIMEOUT, self.call_raw(C::command(), cmd))
            .await
            .map_err(|_| CalDirError::ProviderTimeout(PROVIDER_TIMEOUT.as_secs()))?
    }

    /// Call a typed provider command without timeout (for auth commands that involve user interaction).
    pub async fn call_no_timeout<C: ProviderCommand>(&self, cmd: C) -> CalDirResult<C::Response> {
        timeout(AUTH_TIMEOUT, self.call_raw(C::command(), cmd))
            .await
            .map_err(|_| CalDirError::ProviderTimeout(AUTH_TIMEOUT.as_secs()))?
    }

    /// Low-level call that sends a command with params and deserializes the response.
    async fn call_raw<P: Serialize, R: serde::de::DeserializeOwned>(
        &self,
        command: Command,
        params: P,
    ) -> CalDirResult<R> {
        let params = serde_json::to_value(params)
            .map_err(|e| CalDirError::Serialization(e.to_string()))?;
        let request = Request { command, params };
        let request_json = serde_json::to_string(&request)
            .map_err(|e| CalDirError::Serialization(e.to_string()))?;

        let binary_path = self.binary_path()?;

        let mut child = TokioCommand::new(&binary_path)
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
