use std::path::PathBuf;

mod error;

use crate::{Calendar, Event};

use error::CalendarEventError;

pub struct CalendarEvent {
    pub event: Event,
    pub path: PathBuf,
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

    // fn base_path(&self, calendar: &Calendar) -> PathBuf {
    //     let calendar_path = calendar.path();
    //     let slug = &self.event.base_slug();
    //
    //     calendar_path.join(slug)
    // }

    pub fn from_path(path: PathBuf) -> Result<Self, CalendarEventError> {
        let contents = std::fs::read_to_string(&path)?;

        let calendar: icalendar::Calendar = contents
            .parse()
            .map_err(|err| CalendarEventError::IcsParse(path.clone(), err))?;

        let event = calendar
            .events()
            .next()
            .ok_or_else(|| CalendarEventError::NoEventInIcs(path.clone()))?
            .clone();

        let event = Event::from_ical(event)
            .map_err(|err| CalendarEventError::InvalidEvent(path.clone(), err))?;

        Ok(CalendarEvent { event, path })
    }
}
