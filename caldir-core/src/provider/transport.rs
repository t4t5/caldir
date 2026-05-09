mod error;
mod subprocess_transport;

#[cfg(test)]
pub(crate) mod mock_transport;
pub(crate) use error::ProviderTransportError;

pub(crate) use subprocess_transport::SubprocessTransport;

use async_trait::async_trait;
use std::time::Duration;

/// Provider transports take JSON strings in and return JSON strings out
#[async_trait]
pub(crate) trait ProviderTransport: std::fmt::Debug + Send + Sync {
    async fn exchange(
        &self,
        request: &str,
        timeout_dur: Duration,
    ) -> Result<String, ProviderTransportError>;
}
