mod error;

use crate::{Calendar, Event};
use error::CalendarEventError;
use std::path::PathBuf;

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
    use std::fs;

    use super::*;

    #[test]
    fn errors_on_invalid_ics() {
        let ics = "BEGIN:VCALENDAR"; // Missing END

        let tmp = tempfile::TempDir::new().unwrap();
        let tmp_ics_path = tmp.path().join("test.ics");
        fs::write(&tmp_ics_path, ics).unwrap();

        let calendar_event = CalendarEvent::from_path(tmp_ics_path);

        assert!(calendar_event.is_err());

        if let Err(CalendarEventError::InvalidEvent(path, _)) = calendar_event {
            assert!(path.ends_with("test.ics"));
        } else {
            panic!("Expected InvalidEvent error");
        }
    }

    #[test]
    fn parses_valid_ics() {
        let ics = "BEGIN:VCALENDAR\nVERSION:2.0\nBEGIN:VEVENT\nDTSTART:20240101T120000Z\nSUMMARY:Test Event\nEND:VEVENT\nEND:VCALENDAR";

        let tmp = tempfile::TempDir::new().unwrap();
        let tmp_ics_path = tmp.path().join("test.ics");
        fs::write(&tmp_ics_path, ics).unwrap();

        let calendar_event = CalendarEvent::from_path(tmp_ics_path);

        assert!(calendar_event.is_ok());
    }

    #[test]
    fn saves_event_to_file() {
        let (_tmp, caldir) = crate::Caldir::new_tmp();

        let calendar = Calendar::new(&caldir, "work").unwrap();
        calendar.save().unwrap();

        let ics = "BEGIN:VCALENDAR\nVERSION:2.0\nBEGIN:VEVENT\nDTSTART:20240101T120000Z\nSUMMARY:Test Event\nEND:VEVENT\nEND:VCALENDAR";

        let event = Event::from_contents(ics).unwrap();

        let calendar_event = CalendarEvent::new(&calendar, event);

        calendar_event
            .save()
            .expect("Failed to save calendar event");

        assert!(calendar_event.path.exists());

        // Check file name:
        assert_eq!(
            calendar_event
                .path
                .file_name()
                .expect("Event file should have a name")
                .to_str()
                .unwrap(),
            "2024-01-01T1200__test-event.ics"
        );
    }

    #[test]
    fn generates_unique_filenames() {
        let (_tmp, caldir) = crate::Caldir::new_tmp();

        let calendar = Calendar::new(&caldir, "work").unwrap();

        calendar.save().unwrap();

        let ics = "BEGIN:VCALENDAR\nVERSION:2.0\nBEGIN:VEVENT\nDTSTART:20240101T120000Z\nSUMMARY:Test Event\nEND:VEVENT\nEND:VCALENDAR";

        let event = Event::from_contents(ics).unwrap();

        let calendar_event1 = CalendarEvent::new(&calendar, event.clone());

        calendar_event1.save().unwrap();

        let calendar_event2 = CalendarEvent::new(&calendar, event);

        calendar_event2.save().unwrap();

        assert!(calendar_event1.path.exists());
        assert!(calendar_event2.path.exists());

        assert_ne!(calendar_event1.path, calendar_event2.path);

        assert_eq!(
            calendar_event1
                .path
                .file_name()
                .expect("Event file should have a name")
                .to_str()
                .unwrap(),
            "2024-01-01T1200__test-event.ics"
        );

        assert_eq!(
            calendar_event2
                .path
                .file_name()
                .expect("Event file should have a name")
                .to_str()
                .unwrap(),
            "2024-01-01T1200__test-event-2.ics"
        );
    }
}
