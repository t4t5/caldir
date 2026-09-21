use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CaldirConfigError {
    #[error("failed to read caldir config {0}")]
    Read(PathBuf, #[source] std::io::Error),

    #[error("failed to write caldir config {0}")]
    Write(PathBuf, #[source] std::io::Error),

    #[error("invalid config in TOML file {0}")]
    InvalidConfigFile(PathBuf, #[source] toml::de::Error),

    #[error("invalid TOML syntax in config file {0}")]
    InvalidConfigSyntax(PathBuf, #[source] toml_edit::TomlError),

    #[error("failed to serialize caldir config")]
    InvalidConfig(#[source] toml::ser::Error),

    #[error("could not determine config directory")]
    UnknownConfigDirectory,

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
