mod error;
mod protocol;
mod registry;
mod slug;

use crate::{SubprocessTransport, Transport};
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

pub(crate) use error::ProviderError;
pub use registry::ProviderRegistry;
pub use slug::{ProviderSlug, provider_slug_from_filename};

use protocol::{ProviderCommand, Request, Response};

#[derive(Debug, Clone)]
pub struct Provider {
    slug: ProviderSlug,
    transport: Arc<dyn Transport>,
}

impl Provider {
    pub(crate) fn from_binary_path(binary_path: PathBuf) -> Result<Self, ProviderError> {
        if !is_executable(&binary_path) {
            return Err(ProviderError::NotExecutable(binary_path));
        }

        let slug = binary_path
            .file_name()
            .and_then(|filename| filename.to_str())
            .and_then(provider_slug_from_filename)
            .ok_or_else(|| ProviderError::InvalidProviderFilename(binary_path.clone()))?;

        let transport = SubprocessTransport::new(binary_path);

        Ok(Provider {
            slug,
            transport: Arc::new(transport),
        })
    }

    fn slug(&self) -> &ProviderSlug {
        &self.slug
    }

    pub(crate) async fn call<Command: ProviderCommand>(
        &self,
        cmd: Command,
    ) -> Result<Command::Response, ProviderError> {
        let params = serde_json::to_value(&cmd).map_err(ProviderError::Serialize)?;

        let request = Request {
            command: Command::NAME,
            params,
        };

        let request_json = serde_json::to_string(&request).map_err(ProviderError::Serialize)?;

        let response_json = self
            .transport
            .exchange(&request_json, Command::TIMEOUT)
            .await?;

        let response: Response<Command::Response> =
            serde_json::from_str(&response_json).map_err(ProviderError::Deserialize)?;

        match response {
            Response::Success { data } => Ok(data),
            Response::Error { error } => Err(ProviderError::Provider(error)),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_transport(slug: ProviderSlug, transport: Arc<dyn Transport>) -> Self {
        Provider { slug, transport }
    }

    #[cfg(test)]
    pub(crate) fn transport(&self) -> &dyn Transport {
        &*self.transport
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
    use std::time::Duration;

    use serde::{Deserialize, Serialize};

    use crate::test_utils::test_binary;

    use super::protocol::Command;
    use super::*;
    use crate::TransportError;
    use crate::mock::MockTransport;

    #[derive(Serialize)]
    struct EchoCommand {
        value: String,
    }

    #[derive(Deserialize, Debug, PartialEq)]
    struct EchoResponse {
        value: String,
    }

    impl ProviderCommand for EchoCommand {
        type Response = EchoResponse;
        const NAME: Command = Command::ListEvents;
        const TIMEOUT: Duration = Duration::from_secs(7);
    }

    fn provider_with_transport(transport: Arc<dyn Transport>) -> Provider {
        Provider::with_transport(ProviderSlug::from("test"), transport)
    }

    #[test]
    fn from_binary_path_succeeds_for_valid_provider_binary() {
        let (_tmp, bin) = test_binary("caldir-provider-hooli");

        let provider = Provider::from_binary_path(bin.clone()).unwrap();

        assert_eq!(provider.slug.as_str(), "hooli");
    }

    #[test]
    fn from_binary_path_errors_when_file_does_not_exist() {
        let tmp = tempfile::TempDir::new().unwrap();
        let bin = tmp.path().join("caldir-provider-nonexistant");

        let result = Provider::from_binary_path(bin.clone());

        assert!(matches!(result, Err(ProviderError::NotExecutable(p)) if p == bin));
    }

    #[cfg(unix)]
    #[test]
    fn from_binary_path_errors_when_file_not_executable() {
        let tmp = tempfile::TempDir::new().unwrap();
        let bin = tmp.path().join("caldir-provider-hooli");
        std::fs::write(&bin, b"").unwrap();

        let result = Provider::from_binary_path(bin.clone());

        assert!(matches!(result, Err(ProviderError::NotExecutable(p)) if p == bin));
    }

    #[test]
    fn from_binary_path_errors_when_filename_lacks_prefix() {
        let (_tmp, bin) = test_binary("hooli");

        let result = Provider::from_binary_path(bin.clone());

        assert!(matches!(result, Err(ProviderError::InvalidProviderFilename(p)) if p == bin));
    }

    #[test]
    fn from_binary_path_errors_when_slug_is_empty() {
        let (_tmp, bin) = test_binary("caldir-provider");

        let result = Provider::from_binary_path(bin.clone());

        assert!(matches!(result, Err(ProviderError::InvalidProviderFilename(p)) if p == bin));
    }

    #[tokio::test]
    async fn call_sends_typed_request_and_returns_typed_response() {
        let mock = Arc::new(MockTransport::with_response(
            r#"{"status":"success","data":{"value":"echoed"}}"#,
        ));
        let provider = provider_with_transport(mock.clone());

        let response = provider
            .call(EchoCommand {
                value: "hello".into(),
            })
            .await
            .unwrap();

        assert_eq!(
            response,
            EchoResponse {
                value: "echoed".into()
            }
        );

        let captured: serde_json::Value =
            serde_json::from_str(&mock.captured_request().unwrap()).unwrap();
        assert_eq!(captured["command"], "list_events");
        assert_eq!(captured["params"]["value"], "hello");
    }

    #[tokio::test]
    async fn call_uses_per_command_timeout() {
        let mock = Arc::new(MockTransport::with_response(
            r#"{"status":"success","data":{"value":"x"}}"#,
        ));
        let provider = provider_with_transport(mock.clone());

        provider
            .call(EchoCommand { value: "x".into() })
            .await
            .unwrap();

        assert_eq!(mock.captured_timeout(), Some(Duration::from_secs(7)));
    }

    #[tokio::test]
    async fn call_returns_provider_error_on_error_response() {
        let mock = Arc::new(MockTransport::with_response(
            r#"{"status":"error","error":"oh no"}"#,
        ));
        let provider = provider_with_transport(mock);

        let err = provider
            .call(EchoCommand { value: "x".into() })
            .await
            .unwrap_err();

        assert!(matches!(err, ProviderError::Provider(msg) if msg == "oh no"));
    }

    #[tokio::test]
    async fn call_returns_deserialize_error_on_garbage_response() {
        let mock = Arc::new(MockTransport::with_response("not json at all"));
        let provider = provider_with_transport(mock);

        let err = provider
            .call(EchoCommand { value: "x".into() })
            .await
            .unwrap_err();

        assert!(matches!(err, ProviderError::Deserialize(_)));
    }

    #[tokio::test]
    async fn call_propagates_transport_error() {
        let mock = Arc::new(MockTransport::with_error(TransportError::Timeout(
            Duration::from_secs(1),
        )));
        let provider = provider_with_transport(mock);

        let err = provider
            .call(EchoCommand { value: "x".into() })
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            ProviderError::Transport(TransportError::Timeout(_))
        ));
    }
}
