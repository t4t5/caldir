use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CalendarConfigError {
    #[error("failed to read calendar config {0}")]
    Read(PathBuf, #[source] std::io::Error),

    #[error("failed to write calendar config {0}")]
    Write(PathBuf, #[source] std::io::Error),

    #[error("invalid config in TOML file {0}")]
    InvalidConfigFile(PathBuf, #[source] toml::de::Error),

    #[error("failed to serialize calendar config")]
    InvalidConfig(#[source] toml::ser::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
