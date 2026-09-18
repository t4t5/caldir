use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CalendarConfigError {
    #[error("invalid config in TOML file {0}")]
    InvalidConfigFile(PathBuf, #[source] toml::de::Error),

    #[error("failed to serialize calendar config")]
    InvalidConfig(#[source] toml::ser::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
