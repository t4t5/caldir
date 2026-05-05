use std::path::PathBuf;

mod error;

use crate::{Calendar, Event};

use error::CalendarEventError;

pub struct CalendarEvent {
    event: Event,
    path: PathBuf,
}

impl CalendarEvent {
    pub fn new(calendar: &Calendar, event: Event) -> Self {
        let path = calendar
            .path()
            .join(event.base_slug())
            .with_extension("ics");

        // TODO: collision handling

        CalendarEvent { event, path }
    }

    pub fn save(&self) -> Result<(), CalendarEventError> {
        let ical_event = &self.event.0;
        let ical_calendar = icalendar::Calendar::new().push(ical_event).done();

        std::fs::write(&self.path, ical_calendar.to_string())?;

        Ok(())
    }

    // fn base_path(&self, calendar: &Calendar) -> PathBuf {
    //     let calendar_path = calendar.path();
    //     let slug = &self.event.base_slug();
    //
    //     calendar_path.join(slug)
    // }

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
}
