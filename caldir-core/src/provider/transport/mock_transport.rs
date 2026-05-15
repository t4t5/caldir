use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;

use super::{ProviderTransport, ProviderTransportError};

/// Records each request/timeout and replays canned responses from a FIFO
/// queue. Stub one or more responses via `set_response` / `set_error`;
/// `exchange` panics if it runs out, so a missing stub surfaces loudly.
pub(crate) struct MockTransport {
    responses: Mutex<VecDeque<Result<String, ProviderTransportError>>>,
    captured_requests: Mutex<Vec<String>>,
    captured_timeouts: Mutex<Vec<Duration>>,
}

impl std::fmt::Debug for MockTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MockTransport").finish()
    }
}

impl MockTransport {
    pub(crate) fn empty() -> Self {
        Self {
            responses: Mutex::new(VecDeque::new()),
            captured_requests: Mutex::new(Vec::new()),
            captured_timeouts: Mutex::new(Vec::new()),
        }
    }

    pub(crate) fn with_response(response: impl Into<String>) -> Self {
        let this = Self::empty();
        this.set_response(response);
        this
    }

    pub(crate) fn with_error(error: ProviderTransportError) -> Self {
        let this = Self::empty();
        this.set_error(error);
        this
    }

    pub(crate) fn set_response(&self, response: impl Into<String>) {
        self.responses
            .lock()
            .unwrap()
            .push_back(Ok(response.into()));
    }

    pub(crate) fn set_error(&self, error: ProviderTransportError) {
        self.responses.lock().unwrap().push_back(Err(error));
    }

    pub(crate) fn captured_request(&self) -> Option<String> {
        self.captured_requests.lock().unwrap().last().cloned()
    }

    pub(crate) fn captured_timeout(&self) -> Option<Duration> {
        self.captured_timeouts.lock().unwrap().last().copied()
    }
}

#[async_trait]
impl ProviderTransport for MockTransport {
    async fn exchange(
        &self,
        request: &str,
        timeout_dur: Duration,
    ) -> Result<String, ProviderTransportError> {
        self.captured_requests
            .lock()
            .unwrap()
            .push(request.to_string());
        self.captured_timeouts.lock().unwrap().push(timeout_dur);

        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .expect("MockTransport::exchange called with no queued response")
    }
}
