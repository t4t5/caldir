//! Error types for the caldir ecosystem.

use thiserror::Error;

/// Errors that can occur in caldir operations.
#[derive(Error, Debug)]
pub enum CalDirError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Calendar not found: {0}")]
    CalendarNotFound(String),

    #[error("Provider error: {0}")]
    Provider(String),

    #[error("Provider '{0}' not found in PATH")]
    ProviderNotInstalled(String),

    #[error("Provider request timed out after {0}s")]
    ProviderTimeout(u64),

    #[error("ICS parse error: {0}")]
    IcsParse(String),

    #[error("ICS generation error: {0}")]
    IcsGenerate(String),

    #[error("Sync error: {0}")]
    Sync(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("No remote configured for calendar '{0}'")]
    NoRemoteConfigured(String),
}

/// Result type alias for caldir operations.
pub type CalDirResult<T> = Result<T, CalDirError>;
