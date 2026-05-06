mod error;

use crate::{Calendar, Event};
use error::CalendarEventError;
use std::path::PathBuf;

#[derive(Debug)]
pub struct CalendarEvent {
    event: Event,
    path: PathBuf,
}

impl CalendarEvent {
    pub fn new(calendar: &Calendar, event: Event) -> Self {
        let filename = calendar.unique_event_filename(&event);
        let path = calendar.path().join(filename);
        CalendarEvent { event, path }
    }

    pub fn save(&self) -> Result<(), CalendarEventError> {
        let ical_event = self.event.ical_event();
        let ical_calendar = icalendar::Calendar::new().push(ical_event).done();

        std::fs::write(&self.path, ical_calendar.to_string())?;

        Ok(())
    }

    pub fn from_path(path: PathBuf) -> Result<Self, CalendarEventError> {
        let contents = std::fs::read_to_string(&path)?;

        let event = Event::from_contents(&contents)
            .map_err(|err| CalendarEventError::InvalidEvent(path.clone(), err))?;

        Ok(CalendarEvent { event, path })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::test_calendar;
    use crate::test_utils::test_event;
    use std::fs;

    #[test]
    fn errors_on_invalid_ics() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("test.ics");
        fs::write(&path, "BEGIN:VCALENDAR").unwrap(); // Missing END

        let err = CalendarEvent::from_path(path).unwrap_err();

        assert!(matches!(err, CalendarEventError::InvalidEvent(p, _) if p.ends_with("test.ics")));
    }

    #[test]
    fn parses_valid_ics() {
        let ics = "BEGIN:VCALENDAR\nVERSION:2.0\nBEGIN:VEVENT\nDTSTART:20240101T120000Z\nSUMMARY:Test Event\nEND:VEVENT\nEND:VCALENDAR";
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("test.ics");
        fs::write(&path, ics).unwrap();

        assert!(CalendarEvent::from_path(path).is_ok());
    }

    #[test]
    fn saves_event_to_file() {
        let (_tmp, _caldir, calendar) = test_calendar();
        let cal_event = CalendarEvent::new(&calendar, test_event());

        cal_event.save().unwrap();

        assert!(cal_event.path.ends_with("2026-01-01T1200__test-event.ics"));
    }

    #[test]
    fn generates_unique_filenames_within_calendar() {
        let (_tmp, _caldir, calendar) = test_calendar();

        let cal_event_1 = CalendarEvent::new(&calendar, test_event());
        cal_event_1.save().unwrap();

        assert!(
            cal_event_1
                .path
                .ends_with("2026-01-01T1200__test-event.ics")
        );

        let cal_event_2 = CalendarEvent::new(&calendar, test_event());
        cal_event_2.save().unwrap();

        assert!(
            cal_event_2
                .path
                .ends_with("2026-01-01T1200__test-event-2.ics")
        );

        let cal_event_3 = CalendarEvent::new(&calendar, test_event());
        cal_event_3.save().unwrap();

        assert!(
            cal_event_3
                .path
                .ends_with("2026-01-01T1200__test-event-3.ics")
        );
    }

    #[test]
    fn keeps_base_filenames_in_different_calendar() {
        let (_tmp, _caldir, calendar_1) = test_calendar();
        let cal_event_1 = CalendarEvent::new(&calendar_1, test_event());
        cal_event_1.save().unwrap();

        assert!(
            cal_event_1
                .path
                .ends_with("2026-01-01T1200__test-event.ics")
        );

        let (_tmp, _caldir, calendar_2) = test_calendar();
        let cal_event_2 = CalendarEvent::new(&calendar_2, test_event());
        cal_event_2.save().unwrap();

        assert!(
            cal_event_2
                .path
                .ends_with("2026-01-01T1200__test-event.ics")
        );
    }
}
