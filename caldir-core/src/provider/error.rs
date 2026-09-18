use std::path::PathBuf;

use super::transport::ProviderTransportError;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ProviderError {
    #[error("Provider file is not executable: {0}")]
    NotExecutable(PathBuf),

    #[error("Provider filename does not match `caldir-provider-<name>`: {0}")]
    InvalidProviderFilename(PathBuf),

    #[error("Provider {0} not found")]
    ProviderNotFound(String),

    #[error(transparent)]
    Transport(#[from] ProviderTransportError),

    #[error("failed to serialize provider request")]
    Serialize(#[source] serde_json::Error),

    #[error("failed to deserialize provider response")]
    Deserialize(#[source] serde_json::Error),

    #[error("{0}")]
    Provider(String),
}
