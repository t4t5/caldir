use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum CaldirConfigError {
    #[error("invalid config in TOML file {0}: {1}")]
    InvalidConfigFile(PathBuf, toml::de::Error),

    #[error("invalid calendar config: {0}")]
    InvalidConfig(toml::ser::Error),

    #[error("could not determine config directory")]
    UnknownConfigDirectory,

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
