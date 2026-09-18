use std::{path::PathBuf, time::Duration};

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ProviderTransportError {
    #[error("failed to spawn provider executable {0}")]
    SpawnBinary(PathBuf, #[source] std::io::Error),

    #[error("failed to spawn provider")]
    Spawn(#[source] std::io::Error),

    #[error("failed to exchange with provider")]
    Io(#[source] std::io::Error),

    #[error("Provider response was not valid UTF-8")]
    BadUtf8,

    #[error("Provider returned no response")]
    EmptyResponse,

    #[error("Provider exited with status {code:?}")]
    NonZeroExit { code: Option<i32> },

    #[error("Provider timed out after {0:?}")]
    Timeout(Duration),
}
