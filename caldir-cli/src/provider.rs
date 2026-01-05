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
//!
use anyhow::{Context, Result};
use serde::de::DeserializeOwned;

use caldir_core::protocol::{Command as ProviderCommand, Request, Response};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

pub struct Provider(String);

impl Provider {
    pub fn from_name(name: &str) -> Self {
        Provider(name.to_string())
    }

    pub fn binary_path(&self) -> Result<std::path::PathBuf> {
        let binary_name = format!("caldir-provider-{}", self.0);
        let binary_path = which::which(&binary_name).with_context(|| {
            format!(
                "Provider '{}' not found. Install it with:\n  cargo install {}",
                self.0, binary_name
            )
        })?;
        Ok(binary_path)
    }

    /// Call a provider command and return the result.
    pub async fn call<R: DeserializeOwned>(
        &self,
        command: ProviderCommand,
        params: serde_json::Value,
    ) -> Result<R> {
        let request = Request { command, params };

        let request_json =
            serde_json::to_string(&request).context("Failed to serialize provider request")?;

        let binary_path = self.binary_path()?;

        // Spawn the provider process
        let mut child = Command::new(&binary_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit()) // Let provider errors show in terminal
            .spawn()
            .with_context(|| format!("Failed to spawn provider: {}", &binary_path.display()))?;

        // Write request to stdin
        {
            let mut stdin = child
                .stdin
                .take()
                .context("Failed to get provider stdin handle")?;
            stdin
                .write_all(request_json.as_bytes())
                .await
                .context("Failed to write to provider stdin")?;
            stdin
                .write_all(b"\n")
                .await
                .context("Failed to write newline to provider stdin")?;
            stdin
                .flush()
                .await
                .context("Failed to flush provider stdin")?;
            // Drop stdin to signal EOF
        }

        // Read response from stdout
        let stdout = child
            .stdout
            .take()
            .context("Failed to get provider stdout handle")?;
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
        let response: Response<R> = serde_json::from_str(&line)
            .with_context(|| format!("Failed to parse provider response: {}", line))?;

        match response {
            Response::Success { data } => Ok(data),
            Response::Error { error } => Err(anyhow::anyhow!("{}", error)),
        }
    }
}
