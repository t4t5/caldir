use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("Provider file is not executable: {0}")]
    NotExecutable(PathBuf),

    #[error("Provider path has no filename: {0}")]
    MissingFilename(PathBuf),

    #[error("Provider filename is not valid UTF-8: {0}")]
    NonUtf8Filename(PathBuf),

    #[error("Provider filename does not match `caldir-provider-<name>`: {0}")]
    InvalidProviderFilename(PathBuf),
}
