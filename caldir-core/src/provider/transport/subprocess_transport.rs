use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::timeout;

use super::{ProviderTransport, ProviderTransportError};

#[derive(Debug)]
pub(crate) struct SubprocessTransport {
    bin_path: PathBuf,
}

impl SubprocessTransport {
    pub(crate) fn new(bin_path: PathBuf) -> Self {
        Self { bin_path }
    }
}

/// The subprocess transport runs a provider binary as a subprocess.
/// It then sends JSON strings to it via stdin, and reads JSON strings from its stdout
#[async_trait]
impl ProviderTransport for SubprocessTransport {
    async fn exchange(
        &self,
        request: &str,
        timeout_dur: Duration,
    ) -> Result<String, ProviderTransportError> {
        let exchange = async {
            let mut child = Command::new(&self.bin_path)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn()
                .map_err(ProviderTransportError::Spawn)?;

            let mut stdin = child.stdin.take().expect("stdin was piped above");

            stdin
                .write_all(format!("{request}\n").as_bytes())
                .await
                .map_err(ProviderTransportError::Io)?;

            drop(stdin);

            let output = child
                .wait_with_output()
                .await
                .map_err(ProviderTransportError::Io)?;

            if !output.status.success() {
                return Err(ProviderTransportError::NonZeroExit {
                    code: output.status.code(),
                });
            }

            let response =
                String::from_utf8(output.stdout).map_err(|_| ProviderTransportError::BadUtf8)?;

            if response.is_empty() {
                return Err(ProviderTransportError::EmptyResponse);
            }

            Ok(response)
        };

        timeout(timeout_dur, exchange)
            .await
            .map_err(|_| ProviderTransportError::Timeout(timeout_dur))?
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    use super::*;

    fn make_executable(path: &Path) {
        let mut perms = std::fs::metadata(path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(path, perms).unwrap();
    }

    fn echo_script(tmp: &tempfile::TempDir, body: &str) -> PathBuf {
        let path = tmp.path().join("echo-bin");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        make_executable(&path);
        path
    }

    #[tokio::test]
    async fn subprocess_exchange_returns_stdout_of_provider_binary() {
        let tmp = tempfile::TempDir::new().unwrap();
        // Drain stdin first so we don't race the parent's write to a closed pipe.
        let bin = echo_script(
            &tmp,
            r#"cat > /dev/null; echo '{"status":"success","data":42}'"#,
        );
        let transport = SubprocessTransport::new(bin);

        let response = transport
            .exchange("ignored-request", Duration::from_secs(5))
            .await
            .unwrap();

        assert_eq!(response.trim(), r#"{"status":"success","data":42}"#);
    }

    #[tokio::test]
    async fn subprocess_exchange_errors_on_non_zero_exit() {
        let tmp = tempfile::TempDir::new().unwrap();
        let bin = echo_script(&tmp, "exit 7");
        let transport = SubprocessTransport::new(bin);

        let err = transport
            .exchange("req", Duration::from_secs(5))
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            ProviderTransportError::NonZeroExit { code: Some(7) }
        ));
    }

    #[tokio::test]
    async fn subprocess_exchange_errors_on_empty_stdout() {
        let tmp = tempfile::TempDir::new().unwrap();
        // Drain stdin so we don't race the subprocess to a broken-pipe write,
        // then exit 0 with empty stdout.
        let bin = echo_script(&tmp, "cat > /dev/null");
        let transport = SubprocessTransport::new(bin);

        let err = transport
            .exchange("req", Duration::from_secs(5))
            .await
            .unwrap_err();

        assert!(matches!(err, ProviderTransportError::EmptyResponse));
    }

    #[tokio::test]
    async fn subprocess_exchange_errors_on_timeout() {
        let tmp = tempfile::TempDir::new().unwrap();
        let bin = echo_script(&tmp, "sleep 5; echo done");
        let transport = SubprocessTransport::new(bin);

        let err = transport
            .exchange("req", Duration::from_millis(50))
            .await
            .unwrap_err();

        assert!(matches!(err, ProviderTransportError::Timeout(_)));
    }
}
