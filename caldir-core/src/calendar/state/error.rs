use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum CalendarStateError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error(
        "sync state format {found} in {path} was written by a newer caldir (supported: {supported})"
    )]
    NewerFormat {
        path: PathBuf,
        found: u32,
        supported: u32,
    },

    #[error("invalid sync state format in {path}: {contents:?}")]
    InvalidFormat { path: PathBuf, contents: String },
}
