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
    Command, Connect, ConnectResponse, ProviderCommand, ProviderRequestContext, Request, Response,
};
use crate::remote::provider_account::ProviderAccount;
use serde::Serialize;
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command as TokioCommand;
use tokio::time::timeout;

const PROVIDER_TIMEOUT: Duration = Duration::from_secs(15);
/// No timeout for auth commands since they involve user interaction.
const AUTH_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Clone, Debug)]
pub struct ProviderRegistry {
    providers_dir: PathBuf,
    installed: BTreeMap<String, PathBuf>,
}

#[derive(Clone, Debug)]
pub struct Provider {
    name: String,
    binary_path: PathBuf,
    context: ProviderRequestContext,
}

impl fmt::Display for Provider {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl ProviderRegistry {
    pub fn discover(providers_dir: PathBuf) -> Self {
        let mut installed = BTreeMap::new();
        let prefix = "caldir-provider-";

        for dir in Self::provider_search_dirs() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };

            for entry in entries.flatten() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                let Some(provider_name) = name.strip_prefix(prefix) else {
                    continue;
                };

                let path = entry.path();
                if path.is_file() {
                    installed.entry(provider_name.to_string()).or_insert(path);
                }
            }
        }

        ProviderRegistry {
            providers_dir,
            installed,
        }
    }

    pub fn from_installed(
        providers_dir: PathBuf,
        installed: impl IntoIterator<Item = (String, PathBuf)>,
    ) -> Self {
        ProviderRegistry {
            providers_dir,
            installed: installed.into_iter().collect(),
        }
    }

    /// Returns directories from `CALDIR_PROVIDER_PATH` followed by `PATH`.
    fn provider_search_dirs() -> impl Iterator<Item = PathBuf> {
        let provider_path = std::env::var_os("CALDIR_PROVIDER_PATH");
        let system_path = std::env::var_os("PATH");
        provider_path
            .into_iter()
            .flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
            .chain(
                system_path
                    .into_iter()
                    .flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>()),
            )
    }

    pub fn providers_dir(&self) -> &Path {
        &self.providers_dir
    }

    pub fn provider_dir(&self, name: &str) -> PathBuf {
        self.providers_dir.join(name)
    }

    pub fn installed_names(&self) -> Vec<String> {
        self.installed.keys().cloned().collect()
    }

    pub fn get(&self, name: &str) -> CalDirResult<Provider> {
        let binary_path = self
            .installed
            .get(name)
            .ok_or_else(|| CalDirError::ProviderNotInstalled(name.to_string()))?
            .clone();

        Ok(Provider {
            name: name.to_string(),
            binary_path,
            context: ProviderRequestContext {
                provider_dir: self.provider_dir(name),
            },
        })
    }
}

impl Provider {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn binary_path(&self) -> &Path {
        &self.binary_path
    }

    pub fn provider_dir(&self) -> &Path {
        &self.context.provider_dir
    }

    /// Advance the connect flow by one step.
    ///
    /// Returns `ConnectResponse::NeedsInput` if the provider needs more data,
    /// or `ConnectResponse::Done` with the account identifier when complete.
    pub async fn connect(
        &self,
        options: serde_json::Map<String, serde_json::Value>,
        data: serde_json::Map<String, serde_json::Value>,
    ) -> CalDirResult<ConnectResponse> {
        self.call_no_timeout(Connect { options, data }).await
    }

    /// Wrap a `ConnectResponse::Done` into a `ProviderAccount`.
    pub fn provider_account(&self, identifier: String) -> ProviderAccount {
        ProviderAccount::new(self.clone(), identifier)
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
        let params =
            serde_json::to_value(params).map_err(|e| CalDirError::Serialization(e.to_string()))?;
        let request = Request {
            command,
            context: self.context.clone(),
            params,
        };
        let request_json = serde_json::to_string(&request)
            .map_err(|e| CalDirError::Serialization(e.to_string()))?;

        let mut child = TokioCommand::new(&self.binary_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .map_err(|e| {
                CalDirError::Provider(format!(
                    "Failed to spawn {}: {}",
                    self.binary_path.display(),
                    e
                ))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_resolves_provider_with_runtime_context() {
        let providers_dir = PathBuf::from("/tmp/caldir/providers");
        let binary_path = PathBuf::from("/tmp/bin/caldir-provider-google");
        let registry = ProviderRegistry::from_installed(
            providers_dir.clone(),
            vec![("google".to_string(), binary_path.clone())],
        );

        let provider = registry.get("google").unwrap();

        assert_eq!(provider.name(), "google");
        assert_eq!(provider.binary_path(), binary_path.as_path());
        assert_eq!(
            provider.provider_dir(),
            providers_dir.join("google").as_path()
        );
    }

    #[test]
    fn registry_reports_missing_provider() {
        let registry = ProviderRegistry::from_installed(
            PathBuf::from("/tmp/caldir/providers"),
            Vec::<(String, PathBuf)>::new(),
        );

        assert!(matches!(
            registry.get("google"),
            Err(CalDirError::ProviderNotInstalled(name)) if name == "google"
        ));
    }
}
