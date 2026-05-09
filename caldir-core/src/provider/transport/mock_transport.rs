use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;

use super::{ProviderTransport, ProviderTransportError};

/// Records the request and timeout, then returns a canned response. One-shot:
/// a second call to `exchange` will panic.
pub(crate) struct MockTransport {
    response: Mutex<Option<Result<String, ProviderTransportError>>>,
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
            response: Mutex::new(Some(Ok(response.into()))),
            captured_request: Mutex::new(None),
            captured_timeout: Mutex::new(None),
        }
    }

    pub(crate) fn with_error(error: ProviderTransportError) -> Self {
        Self {
            response: Mutex::new(Some(Err(error))),
            captured_request: Mutex::new(None),
            captured_timeout: Mutex::new(None),
        }
    }

    pub(crate) fn set_response(&self, response: impl Into<String>) {
        *self.response.lock().unwrap() = Some(Ok(response.into()));
    }

    pub(crate) fn captured_request(&self) -> Option<String> {
        self.captured_request.lock().unwrap().clone()
    }

    pub(crate) fn captured_timeout(&self) -> Option<Duration> {
        *self.captured_timeout.lock().unwrap()
    }
}

#[async_trait]
impl ProviderTransport for MockTransport {
    async fn exchange(
        &self,
        request: &str,
        timeout_dur: Duration,
    ) -> Result<String, ProviderTransportError> {
        *self.captured_request.lock().unwrap() = Some(request.to_string());
        *self.captured_timeout.lock().unwrap() = Some(timeout_dur);

        self.response
            .lock()
            .unwrap()
            .take()
            .expect("MockTransport::exchange called more than once")
    }
}
