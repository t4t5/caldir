mod error;
pub(crate) mod event;
mod path;

use crate::calendar::error::CalendarError;
use crate::calendar::path::CalendarPath;
use crate::{Caldir, Event};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct Calendar {
    calendar_path: CalendarPath,
}

impl Calendar {
    /// Create new calendar
    pub fn new(caldir: &Caldir, desired_slug: &str) -> Result<Self, CalendarError> {
        let unique_slug = caldir.unique_calendar_slug(desired_slug);
        let path = caldir.config().calendar_dir().join(unique_slug);
        Self::from_path(path)
    }

    /// Load existing calendar
    pub fn load(caldir: &Caldir, slug: &str) -> Result<Self, CalendarError> {
        let path = caldir.config().calendar_dir().join(slug);

        let calendar = Self::from_path(path)?;

        if !calendar.path().is_dir() {
            return Err(CalendarError::NotFound(calendar.path().to_path_buf()));
        }

        Ok(calendar)
    }

    pub fn save(&self) -> Result<(), CalendarError> {
        std::fs::create_dir_all(self.path())?;
        Ok(())
    }

    pub fn path(&self) -> &Path {
        self.calendar_path.path()
    }

    pub fn slug(&self) -> &str {
        self.calendar_path.slug()
    }

    /// Generate a unique filename that doesn't conflict with existing event files.
    /// If `{base_slug}.ics` exists, tries `{base_slug}-2.ics`, `{base_slug}-3.ics`, etc.
    pub(crate) fn unique_event_filename(&self, event: &Event) -> String {
        let calendar_dir = self.path();
        let base_slug = event.base_slug();

        let desired = format!("{base_slug}.ics");

        if !calendar_dir.join(&desired).exists() {
            return desired;
        }

        let mut suffix = 2;

        loop {
            let candidate = format!("{base_slug}-{suffix}.ics");
            if !calendar_dir.join(&candidate).exists() {
                return candidate;
            }
            suffix += 1;
        }
    }

    fn from_path(path: PathBuf) -> Result<Self, CalendarError> {
        let calendar_path = CalendarPath::new(path)?;

        Ok(Calendar { calendar_path })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_directory_with_desired_slug() {
        let (tmp, caldir) = Caldir::new_tmp();

        let calendar = Calendar::new(&caldir, "work").unwrap();
        calendar.save().unwrap();

        assert_eq!(calendar.path(), tmp.path().join("work"));
        assert_eq!(calendar.slug(), "work");
        assert!(calendar.path().is_dir());
    }

    #[test]
    fn appends_suffix_on_slug_collision() {
        let (_tmp, caldir) = Caldir::new_tmp();

        let first = Calendar::new(&caldir, "work").unwrap();
        first.save().unwrap();

        let second = Calendar::new(&caldir, "work").unwrap();
        second.save().unwrap();

        assert_eq!(first.slug(), "work");
        assert_eq!(second.slug(), "work-2");
        assert!(second.path().is_dir());
    }

    #[test]
    fn load_returns_existing_calendar() {
        let (_tmp, caldir) = Caldir::new_tmp();
        Calendar::new(&caldir, "personal").unwrap().save().unwrap();

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
