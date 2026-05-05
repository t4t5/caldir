use crate::calendar::error::CalendarError;

use std::path::{Path, PathBuf};

/// CalendarPath represents a path to a calendar file.
/// It validates that the path has a UTF-8 file name so that it generates a valid slug.
#[derive(Debug)]
pub struct CalendarPath(PathBuf);

impl CalendarPath {
    pub fn new(path: PathBuf) -> Result<Self, CalendarError> {
        Self::slug_from(&path)?;
        Ok(Self(path))
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    pub fn slug(&self) -> &str {
        Self::slug_from(&self.0).expect("CalendarPath validates that path has a UTF-8 file name")
    }

    fn slug_from(path: &Path) -> Result<&str, CalendarError> {
        path.file_name()
            .and_then(|s| s.to_str())
            .ok_or(CalendarError::InvalidCalendarPath)
    }
}
