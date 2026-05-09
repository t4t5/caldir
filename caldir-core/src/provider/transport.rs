use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::timeout;

#[derive(Debug, thiserror::Error)]
pub(crate) enum TransportError {
    #[error("Failed to spawn provider: {0}")]
    Spawn(std::io::Error),

    #[error("I/O error during provider exchange: {0}")]
    Io(std::io::Error),

    #[error("Provider response was not valid UTF-8")]
    BadUtf8,

    #[error("Provider returned no response")]
    EmptyResponse,

    #[error("Provider exited with status {code:?}")]
    NonZeroExit { code: Option<i32> },

    #[error("Provider timed out after {0:?}")]
    Timeout(Duration),
}

#[async_trait]
pub(crate) trait Transport: std::fmt::Debug + Send + Sync {
    async fn exchange(
        &self,
        request: &str,
        timeout_dur: Duration,
    ) -> Result<String, TransportError>;
}

#[derive(Debug)]
pub(crate) struct SubprocessTransport {
    bin_path: PathBuf,
}

impl SubprocessTransport {
    pub(crate) fn new(bin_path: PathBuf) -> Self {
        Self { bin_path }
    }

    #[cfg(test)]
    pub(crate) fn bin_path(&self) -> &Path {
        &self.bin_path
    }
}

#[async_trait]
impl Transport for SubprocessTransport {
    async fn exchange(
        &self,
        request: &str,
        timeout_dur: Duration,
    ) -> Result<String, TransportError> {
        let exchange = async {
            let mut child = Command::new(&self.bin_path)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn()
                .map_err(TransportError::Spawn)?;

            let mut stdin = child.stdin.take().expect("stdin was piped above");
            stdin
                .write_all(format!("{request}\n").as_bytes())
                .await
                .map_err(TransportError::Io)?;
            drop(stdin);

            let output = child.wait_with_output().await.map_err(TransportError::Io)?;

            if !output.status.success() {
                return Err(TransportError::NonZeroExit {
                    code: output.status.code(),
                });
            }

            let response = String::from_utf8(output.stdout).map_err(|_| TransportError::BadUtf8)?;

            if response.is_empty() {
                return Err(TransportError::EmptyResponse);
            }

            Ok(response)
        };

        timeout(timeout_dur, exchange)
            .await
            .map_err(|_| TransportError::Timeout(timeout_dur))?
    }
}

#[cfg(test)]
pub(crate) mod mock {
    use std::sync::Mutex;

    use super::*;

    /// Records the request and timeout, then returns a canned response.
    pub(crate) struct MockTransport {
        response: Result<String, TransportError>,
        captured_request: Mutex<Option<String>>,
        captured_timeout: Mutex<Option<Duration>>,
    }

    impl std::fmt::Debug for MockTransport {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("MockTransport").finish()
        }
    }

    impl MockTransport {
        pub(crate) fn with_response(response: impl Into<String>) -> Self {
            Self {
                response: Ok(response.into()),
                captured_request: Mutex::new(None),
                captured_timeout: Mutex::new(None),
            }
        }

        pub(crate) fn with_error(error: TransportError) -> Self {
            Self {
                response: Err(error),
                captured_request: Mutex::new(None),
                captured_timeout: Mutex::new(None),
            }
        }

        pub(crate) fn captured_request(&self) -> Option<String> {
            self.captured_request.lock().unwrap().clone()
        }

        pub(crate) fn captured_timeout(&self) -> Option<Duration> {
            *self.captured_timeout.lock().unwrap()
        }
    }

    #[async_trait]
    impl Transport for MockTransport {
        async fn exchange(
            &self,
            request: &str,
            timeout_dur: Duration,
        ) -> Result<String, TransportError> {
            *self.captured_request.lock().unwrap() = Some(request.to_string());
            *self.captured_timeout.lock().unwrap() = Some(timeout_dur);
            match &self.response {
                Ok(resp) => Ok(resp.clone()),
                Err(e) => Err(clone_transport_error(e)),
            }
        }
    }

    fn clone_transport_error(e: &TransportError) -> TransportError {
        match e {
            TransportError::Spawn(io) => {
                TransportError::Spawn(std::io::Error::new(io.kind(), io.to_string()))
            }
            TransportError::Io(io) => {
                TransportError::Io(std::io::Error::new(io.kind(), io.to_string()))
            }
            TransportError::BadUtf8 => TransportError::BadUtf8,
            TransportError::EmptyResponse => TransportError::EmptyResponse,
            TransportError::NonZeroExit { code } => TransportError::NonZeroExit { code: *code },
            TransportError::Timeout(d) => TransportError::Timeout(*d),
        }
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
        let bin = echo_script(&tmp, r#"echo '{"status":"success","data":42}'"#);
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

        assert!(matches!(err, TransportError::NonZeroExit { code: Some(7) }));
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

        assert!(matches!(err, TransportError::EmptyResponse));
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

        assert!(matches!(err, TransportError::Timeout(_)));
    }
}
