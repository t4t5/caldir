mod config;
mod error;
mod event;
mod path;

use crate::{Caldir, Event};
use error::CalendarError;
use event::CalendarEventError;
use path::CalendarPath;
use std::path::{Path, PathBuf};

pub use event::CalendarEvent;

#[derive(Debug)]
pub struct Calendar {
    calendar_path: CalendarPath,
}

impl Calendar {
    /// Create new calendar
    pub fn create(caldir: &Caldir, desired_slug: &str) -> Result<Self, CalendarError> {
        let unique_slug = caldir.unique_calendar_slug(desired_slug);
        let path = caldir.config().calendar_dir().join(unique_slug);
        std::fs::create_dir_all(path.clone())?;
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

    /// Load all events in calendar
    pub fn events(&self) -> Result<Vec<CalendarEvent>, CalendarEventError> {
        let mut events: Vec<CalendarEvent> = Vec::new();

        for entry in std::fs::read_dir(self.path())? {
            let entry = entry?;
            let path = entry.path();

            if entry.file_type()?.is_file() && path.extension().is_some_and(|ext| ext == "ics") {
                events.push(CalendarEvent::load(path)?);
            }
        }

        Ok(events)
    }

    /// Load specific event in calendar
    pub fn event(&self, event_slug: &str) -> Result<CalendarEvent, CalendarEventError> {
        let event_path = self.path().join(format!("{}.ics", event_slug));

        if !event_path.is_file() {
            return Err(CalendarEventError::NotFound(event_path));
        }

        CalendarEvent::load(event_path)
    }

    /// Create new event in calendar
    pub fn create_event(&self, event: Event) -> Result<CalendarEvent, CalendarEventError> {
        event::CalendarEvent::create(self, event)
    }

    /// Delete event from calendar
    pub fn delete_event(&self, event_slug: &str) -> Result<(), CalendarEventError> {
        let event = self.event(event_slug)?;
        event.delete()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{test_caldir, test_calendar, test_event};

    #[test]
    fn creates_directory_with_desired_slug() {
        let (tmp, caldir) = test_caldir();

        let calendar = Calendar::create(&caldir, "work").unwrap();

        assert_eq!(calendar.path(), tmp.path().join("work"));
        assert_eq!(calendar.slug(), "work");
        assert!(calendar.path().is_dir());
    }

    #[test]
    fn appends_suffix_on_slug_collision() {
        let (_tmp, caldir) = test_caldir();

        let calendar_1 = Calendar::create(&caldir, "work").unwrap();
        assert_eq!(calendar_1.slug(), "work");

        let calendar_2 = Calendar::create(&caldir, "work").unwrap();
        assert_eq!(calendar_2.slug(), "work-2");

        let calendar_3 = Calendar::create(&caldir, "work").unwrap();
        assert_eq!(calendar_3.slug(), "work-3");
    }

    #[test]
    fn load_returns_existing_calendar() {
        let (_tmp, caldir) = test_caldir();
        Calendar::create(&caldir, "personal").unwrap();

        let calendar = Calendar::load(&caldir, "personal").unwrap();

        assert_eq!(calendar.slug(), "personal");
    }

    #[test]
    fn load_errors_when_directory_missing() {
        let (_tmp, caldir) = test_caldir();

        let result = Calendar::load(&caldir, "missing");

        assert!(matches!(result, Err(CalendarError::NotFound(_))));
    }

    #[test]
    fn load_errors_when_not_directory() {
        let (tmp, caldir) = test_caldir();

        let file_path = tmp.path().join("not_a_directory");
        std::fs::write(&file_path, "I am a file, not a directory").unwrap();

        let result = Calendar::load(&caldir, "not_a_directory");

        assert!(matches!(result, Err(CalendarError::NotFound(p)) if p == file_path));
    }

    #[test]
    fn events_only_returns_events_from_current_calendar() {
        let (_tmp, caldir) = test_caldir();

        let work = Calendar::create(&caldir, "work").unwrap();
        let personal = Calendar::create(&caldir, "personal").unwrap();

        work.create_event(test_event()).unwrap();
        work.create_event(test_event()).unwrap();
        personal.create_event(test_event()).unwrap();

        assert_eq!(work.events().unwrap().len(), 2);
        assert_eq!(personal.events().unwrap().len(), 1);
    }

    #[test]
    fn events_ignores_non_ics_files() {
        let (_tmp, calendar) = test_calendar();

        calendar.create_event(test_event()).unwrap();

        // Drop in stray files that other tools (e.g. vdirsyncer) might leave around.
        std::fs::write(calendar.path().join("color"), "#ff0000").unwrap();
        std::fs::write(calendar.path().join("displayname"), "Work").unwrap();
        std::fs::write(calendar.path().join("README.md"), "notes").unwrap();

        let events = calendar.events().unwrap();

        assert_eq!(events.len(), 1);
    }

    #[test]
    fn event_returns_event_by_slug() {
        let (_tmp, calendar) = test_calendar();
        let created = calendar.create_event(test_event()).unwrap();

        let found = calendar.event("2026-01-01T1200__test-event").unwrap();

        assert_eq!(found.path(), created.path());
    }

    #[test]
    fn event_errors_when_file_missing() {
        let (_tmp, calendar) = test_calendar();

        let result = calendar.event("does-not-exist");

        assert!(matches!(result, Err(CalendarEventError::NotFound(_))));
    }

    #[test]
    fn delete_event_removes_file() {
        let (_tmp, calendar) = test_calendar();
        let cal_event = calendar.create_event(test_event()).unwrap();
        let path = cal_event.path().to_path_buf();
        assert!(path.is_file());

        calendar
            .delete_event("2026-01-01T1200__test-event")
            .unwrap();

        assert!(!path.exists());
    }
}
