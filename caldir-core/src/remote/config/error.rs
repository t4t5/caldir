use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum RemoteConfigError {
    #[error("invalid config in TOML file {0}: {1}")]
    InvalidConfigFile(PathBuf, toml::de::Error),

    #[error("invalid remote config: {0}")]
    InvalidConfig(toml::ser::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
