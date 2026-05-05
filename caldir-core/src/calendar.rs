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
    /// Create new calendar
    pub fn create(caldir: &Caldir, desired_slug: &str) -> Result<Self, CalendarError> {
        let calendar = Self::new(caldir, desired_slug)?;
        std::fs::create_dir_all(calendar.path())?;
        Ok(calendar)
    }

    // #[cfg(test)]
    // pub(crate) fn create_tmp(desired_slug: &str) -> (tempfile::TempDir, Self) {
    //     let (tmpdir, tmp_caldir) = Caldir::new_tmp();
    //     let tmp_calendar = Self::create(&tmp_caldir, desired_slug).unwrap();
    //     (tmpdir, tmp_calendar)
    // }

    /// Load existing calendar
    pub fn load(caldir: &Caldir, slug: &str) -> Result<Self, CalendarError> {
        let path = caldir.config().calendar_dir().join(slug);

        let calendar = Self::from_path(path)?;

        if !calendar.path().is_dir() {
            return Err(CalendarError::NotFound(calendar.path().to_path_buf()));
        }

        Ok(calendar)
    }

    fn new(caldir: &Caldir, desired_slug: &str) -> Result<Self, CalendarError> {
        let unique_slug = caldir.unique_calendar_slug(desired_slug);
        let path = caldir.config().calendar_dir().join(unique_slug);
        Self::from_path(path)
    }

    fn from_path(path: PathBuf) -> Result<Self, CalendarError> {
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

    #[test]
    fn creates_directory_with_desired_slug() {
        let (tmp, caldir) = Caldir::new_tmp();

        let calendar = Calendar::create(&caldir, "work").unwrap();

        assert_eq!(calendar.path(), tmp.path().join("work"));
        assert_eq!(calendar.slug(), "work");
        assert!(calendar.path().is_dir());
    }

    #[test]
    fn appends_suffix_on_slug_collision() {
        let (_tmp, caldir) = Caldir::new_tmp();

        let first = Calendar::create(&caldir, "work").unwrap();
        let second = Calendar::create(&caldir, "work").unwrap();

        assert_eq!(first.slug(), "work");
        assert_eq!(second.slug(), "work-2");
        assert!(second.path().is_dir());
    }

    #[test]
    fn load_returns_existing_calendar() {
        let (_tmp, caldir) = Caldir::new_tmp();
        Calendar::create(&caldir, "personal").unwrap();

        let calendar = Calendar::load(&caldir, "personal").unwrap();

        assert_eq!(calendar.slug(), "personal");
    }

    #[test]
    fn load_errors_when_directory_missing() {
        let (_tmp, caldir) = Caldir::new_tmp();

        let result = Calendar::load(&caldir, "missing");

        assert!(matches!(result, Err(CalendarError::NotFound(_))));
    }
}
