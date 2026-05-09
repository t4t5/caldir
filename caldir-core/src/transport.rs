mod error;
mod subprocess_transport;

#[cfg(test)]
pub(crate) mod mock_transport;

use async_trait::async_trait;
pub(crate) use error::TransportError;
use std::time::Duration;
pub(crate) use subprocess_transport::SubprocessTransport;

#[async_trait]
pub(crate) trait Transport: std::fmt::Debug + Send + Sync {
    async fn exchange(
        &self,
        request: &str,
        timeout_dur: Duration,
    ) -> Result<String, TransportError>;
}
