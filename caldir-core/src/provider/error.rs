use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub(crate) enum ProviderError {
    #[error("Provider file is not executable: {0}")]
    NotExecutable(PathBuf),

    #[error("Provider filename does not match `caldir-provider-<name>`: {0}")]
    InvalidProviderFilename(PathBuf),

    #[error("Provider {0} not found")]
    ProviderNotFound(String),
}
