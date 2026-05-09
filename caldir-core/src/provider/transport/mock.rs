use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;

use super::{Transport, TransportError};

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
