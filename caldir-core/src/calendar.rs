use std::path::{Path, PathBuf};

mod error;
mod path;

use crate::Caldir;
use crate::calendar::error::CalendarError;
use crate::calendar::path::CalendarPath;

#[derive(Debug)]
pub struct Calendar {
    calendar_path: CalendarPath,
}

impl Calendar {
    pub fn new(caldir: &Caldir, slug: &str) -> Result<Self, CalendarError> {
        let path = caldir.config().calendar_dir().join(slug);
        Self::from_path(path)
    }

    pub fn from_path(path: PathBuf) -> Result<Self, CalendarError> {
        let calendar_path = CalendarPath::new(path)?;

        Ok(Calendar { calendar_path })
    }

    pub fn path(&self) -> &Path {
        self.calendar_path.path()
    }

    pub fn slug(&self) -> &str {
        self.calendar_path.slug()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CaldirConfig;

    #[test]
    fn new_joins_calendar_dir_with_slug() {
        let caldir = Caldir::new(CaldirConfig {
            calendar_dir: PathBuf::from("/tmp/caldir"),
        });

        let calendar = Calendar::new(&caldir, "work").unwrap();

        assert_eq!(calendar.path(), Path::new("/tmp/caldir/work"));
        assert_eq!(calendar.slug(), "work");
    }

    #[test]
    fn from_path_exposes_slug() {
        let calendar = Calendar::from_path(PathBuf::from("/tmp/caldir/personal")).unwrap();

        assert_eq!(calendar.slug(), "personal");
    }

    #[test]
    fn from_path_rejects_path_without_file_name() {
        let result = Calendar::from_path(PathBuf::from("/"));

        assert!(matches!(result, Err(CalendarError::InvalidCalendarPath)));
    }
}
