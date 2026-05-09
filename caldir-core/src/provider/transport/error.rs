use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum ProviderTransportError {
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
