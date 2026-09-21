use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CalendarConfigError {
    #[error("failed to read calendar config {0}")]
    Read(PathBuf, #[source] std::io::Error),

    #[error("failed to write calendar config {0}")]
    Write(PathBuf, #[source] std::io::Error),

    #[error("invalid config in TOML file {0}")]
    Deserialize(PathBuf, #[source] toml::de::Error),

    #[error("invalid TOML syntax in config file {0}")]
    Parse(PathBuf, #[source] toml_edit::TomlError),

    #[error("failed to serialize calendar config")]
    Serialize(#[source] toml::ser::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
