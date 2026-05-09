mod error;
mod subprocess;

#[cfg(test)]
pub(crate) mod mock;

use std::time::Duration;

use async_trait::async_trait;

pub(crate) use error::TransportError;
pub(crate) use subprocess::SubprocessTransport;

#[async_trait]
pub(crate) trait Transport: std::fmt::Debug + Send + Sync {
    async fn exchange(
        &self,
        request: &str,
        timeout_dur: Duration,
    ) -> Result<String, TransportError>;
}
