use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CaldirConfigError {
    #[error("invalid config in TOML file {0}")]
    InvalidConfigFile(PathBuf, #[source] toml::de::Error),

    #[error("failed to serialize caldir config")]
    InvalidConfig(#[source] toml::ser::Error),

    #[error("could not determine config directory")]
    UnknownConfigDirectory,

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
