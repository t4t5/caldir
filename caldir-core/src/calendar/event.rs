mod error;

use crate::{Calendar, Event, VersionedEvent};
use std::fs::OpenOptions;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
pub use error::CalendarEventError;

#[derive(Debug)]
pub struct CalendarEvent {
    event: Event,
    path: PathBuf,
}

impl CalendarEvent {
    pub fn create(calendar: &Calendar, event: Event) -> Result<Self, CalendarEventError> {
        let base_slug = event.base_slug();
        let contents = event.to_ics_string();

        let path = write_best_event_file(calendar.path(), &base_slug, None, contents.as_bytes())?;

        Ok(CalendarEvent { event, path })
    }

    pub fn load(path: impl Into<PathBuf>) -> Result<Self, CalendarEventError> {
        let path = path.into();

        if !path.is_file() {
            return Err(CalendarEventError::NotFound(path));
        }

        let contents = std::fs::read_to_string(&path)?;

        let event = Event::from_ics_str(&contents)
            .map_err(|err| CalendarEventError::InvalidEvent(path.clone(), err))?;

        Ok(CalendarEvent { event, path })
    }

    pub fn update(&mut self, event: Event) -> Result<(), CalendarEventError> {
        let base_slug = event.base_slug();
        let contents = event.to_ics_string();
        let dir = self.path.parent().unwrap_or_else(|| Path::new("."));

        let new_path =
            write_best_event_file(dir, &base_slug, Some(&self.path), contents.as_bytes())?;

        if new_path == self.path {
            self.event = event;
            return Ok(());
        }

        if let Err(err) = std::fs::remove_file(&self.path) {
            let _ = std::fs::remove_file(&new_path);
            return Err(err.into());
        }

        self.event = event;
        self.path = new_path;

        Ok(())
    }

    pub fn delete(self) -> Result<(), CalendarEventError> {
        std::fs::remove_file(self.path).map_err(Into::into)
    }

    pub fn event(&self) -> &Event {
        &self.event
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn filename(&self) -> Option<&str> {
        self.path.file_name().and_then(|name| name.to_str())
    }

    // Used for sync direction detection
    fn file_modified_time(&self) -> Option<DateTime<Utc>> {
        std::fs::metadata(self.path())
            .ok()
            .and_then(|m| m.modified().ok())
            .map(DateTime::<Utc>::from)
    }

    pub(crate) fn into_versioned(self) -> VersionedEvent {
        let modified_at = self.file_modified_time();

        VersionedEvent {
            event: self.event,
            modified_at,
        }
    }
}

fn write_best_event_file(
    calendar_dir: &Path,
    base_slug: &str,
    current_path: Option<&Path>,
    contents: &[u8],
) -> Result<PathBuf, CalendarEventError> {
    let mut suffix = 1;

    loop {
        let filename = if suffix == 1 {
            format!("{base_slug}.ics")
        } else {
            format!("{base_slug}-{suffix}.ics")
        };
        let path = calendar_dir.join(filename);

        if current_path == Some(path.as_path()) {
            std::fs::write(&path, contents)?;
            return Ok(path);
        }

        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                if let Err(err) = file.write_all(contents) {
                    let _ = std::fs::remove_file(&path);
                    return Err(err.into());
                }

                return Ok(path);
            }
            Err(err) if err.kind() == ErrorKind::AlreadyExists => {
                suffix += 1;
            }
            Err(err) => return Err(err.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::test_calendar;
    use crate::test_utils::test_calendar_event;
    use crate::test_utils::test_event;
    use std::fs;

    #[test]
    fn create_saves_event_to_file() {
        let (_tmp, calendar) = test_calendar();
        let cal_event = CalendarEvent::create(&calendar, test_event()).unwrap();

        assert!(cal_event.path().is_file());
        assert_eq!(
            cal_event.filename(),
            Some("2026-01-01T1200__test-event.ics")
        );
    }

    #[test]
    fn create_generates_unique_filenames_within_calendar() {
        let (_tmp, calendar) = test_calendar();

        let cal_event_1 = CalendarEvent::create(&calendar, test_event()).unwrap();

        assert_eq!(
            cal_event_1.filename(),
            Some("2026-01-01T1200__test-event.ics")
        );

        let cal_event_2 = CalendarEvent::create(&calendar, test_event()).unwrap();

        assert_eq!(
            cal_event_2.filename(),
            Some("2026-01-01T1200__test-event-2.ics")
        );

        let cal_event_3 = CalendarEvent::create(&calendar, test_event()).unwrap();

        assert_eq!(
            cal_event_3.filename(),
            Some("2026-01-01T1200__test-event-3.ics")
        );
    }

    #[test]
    fn create_keeps_base_filenames_in_different_calendars() {
        let (_tmp, calendar_1) = test_calendar();
        let cal_event_1 = CalendarEvent::create(&calendar_1, test_event()).unwrap();

        let (_tmp, calendar_2) = test_calendar();
        let cal_event_2 = CalendarEvent::create(&calendar_2, test_event()).unwrap();

        assert_eq!(
            cal_event_1.filename().unwrap(),
            cal_event_2.filename().unwrap()
        );
    }

    #[test]
    fn load_errors_on_missing_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("missing.ics");

        let err = CalendarEvent::load(path).unwrap_err();

        assert!(matches!(err, CalendarEventError::NotFound(p) if p.ends_with("missing.ics")));
    }

    #[test]
    fn load_errors_on_invalid_ics() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("test.ics");
        fs::write(&path, "BEGIN:VCALENDAR").unwrap(); // Missing END

        let err = CalendarEvent::load(path).unwrap_err();

        assert!(matches!(err, CalendarEventError::InvalidEvent(p, _) if p.ends_with("test.ics")));
    }

    #[test]
    fn load_parses_valid_ics() {
        let ics = "BEGIN:VCALENDAR\nVERSION:2.0\nBEGIN:VEVENT\nUID:test-uid@caldir\nDTSTART:20240101T120000Z\nSUMMARY:Test Event\nEND:VEVENT\nEND:VCALENDAR";
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("test.ics");
        fs::write(&path, ics).unwrap();

        assert!(CalendarEvent::load(path).is_ok());
    }

    #[test]
    fn update_renames_file_when_summary_changes() {
        let (_tmp, mut cal_event) = test_calendar_event();

        assert_eq!(
            cal_event.filename(),
            Some("2026-01-01T1200__test-event.ics")
        );

        let mut event = cal_event.event().clone();
        event.summary = Some("Planning Session".to_string());
        cal_event.update(event).unwrap();

        assert_eq!(
            cal_event.filename(),
            Some("2026-01-01T1200__planning-session.ics")
        );
    }

    #[test]
    fn update_keeps_filename_when_other_properties_change() {
        let (_tmp, mut cal_event) = test_calendar_event();

        assert_eq!(
            cal_event.filename(),
            Some("2026-01-01T1200__test-event.ics")
        );

        let mut event = cal_event.event().clone();
        event.location = Some("Conference Room".to_string());
        cal_event.update(event).unwrap();

        assert_eq!(
            cal_event.filename(),
            Some("2026-01-01T1200__test-event.ics")
        );

        let contents = fs::read_to_string(cal_event.path()).unwrap();
        assert!(contents.contains("LOCATION:Conference Room"));
    }

    #[test]
    fn update_updates_filename_to_base_when_base_is_available() {
        let (_tmp, calendar) = test_calendar();

        let cal_event_1 = CalendarEvent::create(&calendar, test_event()).unwrap();
        let mut cal_event_2 = CalendarEvent::create(&calendar, test_event()).unwrap();

        assert_eq!(
            cal_event_2.filename(),
            Some("2026-01-01T1200__test-event-2.ics")
        );

        // Delete original event that had "test-event" slug
        cal_event_1
            .delete()
            .expect("Failed to delete calendar event");

        // "test-event" slug is now available for cal_event_2 to use:
        cal_event_2.update(cal_event_2.event().clone()).unwrap();

        assert_eq!(
            cal_event_2.filename(),
            Some("2026-01-01T1200__test-event.ics")
        );
    }
}
