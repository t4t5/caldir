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

use serde::Serialize;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command as TokioCommand;
use tokio::time::timeout;

use crate::error::{CalDirError, CalDirResult};
use crate::remote::protocol::{
    Command, Connect, ConnectResponse, ProviderCommand, Request, Response,
};
use crate::remote::provider_account::ProviderAccount;

const PROVIDER_TIMEOUT: Duration = Duration::from_secs(15);
/// No timeout for auth commands since they involve user interaction.
const AUTH_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Clone, Debug)]
pub struct Provider {
    slug: String,
    binary_path: PathBuf,
}

impl fmt::Display for Provider {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.slug)
    }
}

pub static PROVIDER_BINARY_PREFIX: &str = "caldir-provider-";

fn provider_slug_from_filename(filename: &str) -> Option<&str> {
    let slug = filename.strip_prefix(PROVIDER_BINARY_PREFIX)?;
    let slug = slug.strip_suffix(std::env::consts::EXE_SUFFIX)?;
    (!slug.is_empty()).then_some(slug)
}

impl Provider {
    pub fn from_binary_path(binary_path: PathBuf) -> Option<Self> {
        if !is_executable(&binary_path) {
            return None;
        }

        let filename = &binary_path.file_name()?.to_str()?;
        let slug = provider_slug_from_filename(filename)?;

        Some(Provider::new(slug, &binary_path))
    }

    fn new(slug: &str, binary_path: &PathBuf) -> Self {
        Provider {
            slug: slug.into(),
            binary_path: binary_path.into(),
        }
    }

    // pub fn name(&self) -> &str {
    //     &self.name
    // }
    //
    // pub fn binary_path(&self) -> &Path {
    //     &self.binary_path
    // }
    //
    // pub fn dir(&self) -> &Path {
    //     &self.dir
    // }

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
        let request = Request { command, params };
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

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    path.is_file()
        && path
            .metadata()
            .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
}

#[cfg(windows)]
fn is_executable(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| format!(".{extension}"))
            .is_some_and(|extension| extension.eq_ignore_ascii_case(std::env::consts::EXE_SUFFIX))
}

#[cfg(not(any(unix, windows)))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    fn make_executable(path: &Path) {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = std::fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).unwrap();
    }

    #[cfg(not(unix))]
    fn make_executable(_path: &Path) {}

    fn provider_binary_name(provider: &str) -> String {
        format!("caldir-provider-{provider}{}", std::env::consts::EXE_SUFFIX)
    }

    #[test]
    fn provider_carries_runtime_context() {
        let providers_dir = PathBuf::from("/tmp/caldir/providers");
        let binary_path = PathBuf::from("/tmp/bin/caldir-provider-google");
        let provider = Provider::new("google", binary_path.clone(), providers_dir.join("google"));

        assert_eq!(provider.name(), "google");
        assert_eq!(provider.binary_path(), binary_path.as_path());
        assert_eq!(provider.dir(), providers_dir.join("google").as_path());
    }

    #[cfg(unix)]
    #[test]
    fn discover_installed_ignores_non_executable_provider_files() {
        let tmp = tempfile::tempdir().unwrap();
        let bin_dir = tmp.path().join("bin");
        std::fs::create_dir_all(&bin_dir).unwrap();

        std::fs::write(bin_dir.join(provider_binary_name("google")), "").unwrap();

        let providers = Provider::discover_installed(tmp.path().join("providers"), [&bin_dir]);

        assert!(providers.is_empty());
    }

    #[test]
    fn discover_installed_finds_executable_provider_files() {
        let tmp = tempfile::tempdir().unwrap();
        let bin_dir = tmp.path().join("bin");
        std::fs::create_dir_all(&bin_dir).unwrap();

        let binary_path = bin_dir.join(provider_binary_name("google"));
        std::fs::write(&binary_path, "").unwrap();
        make_executable(&binary_path);

        let providers = Provider::discover_installed(tmp.path().join("providers"), [&bin_dir]);

        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].name(), "google");
        assert_eq!(providers[0].binary_path(), binary_path.as_path());
    }
}
