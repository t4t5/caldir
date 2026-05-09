use std::path::PathBuf;

use super::transport::TransportError;

#[derive(Debug, thiserror::Error)]
pub(crate) enum ProviderError {
    #[error("Provider file is not executable: {0}")]
    NotExecutable(PathBuf),

    #[error("Provider filename does not match `caldir-provider-<name>`: {0}")]
    InvalidProviderFilename(PathBuf),

    #[error("Provider {0} not found")]
    ProviderNotFound(String),

    #[error("{0}")]
    Transport(#[from] TransportError),

    #[error("Failed to serialize provider request: {0}")]
    Serialize(serde_json::Error),

    #[error("Failed to deserialize provider response: {0}")]
    Deserialize(serde_json::Error),

    #[error("Provider returned error: {0}")]
    Provider(String),
}
