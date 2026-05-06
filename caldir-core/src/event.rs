mod error;
mod slugify;

pub use error::EventError;
use icalendar::{CalendarDateTime, Component, DatePerhapsTime};

#[derive(Debug, Clone)]
pub struct Event(icalendar::Event);

impl Event {
    pub(crate) fn from_contents(contents: &str) -> Result<Self, EventError> {
        let calendar: icalendar::Calendar = contents
            .parse()
            .map_err(|err| EventError::InvalidIcs(contents.to_string(), err))?;

        Self::from_ical_calendar(&calendar)
    }

    pub(crate) fn from_ical_calendar(icalendar: &icalendar::Calendar) -> Result<Self, EventError> {
        let ical_event = icalendar
            .events()
            .next()
            .ok_or_else(|| EventError::NoEventInIcs(icalendar.clone()))?;

        Self::from_ical_event(ical_event)
    }

    pub(crate) fn from_ical_event(inner: &icalendar::Event) -> Result<Self, EventError> {
        let start = inner.get_start().ok_or(EventError::MissingStart)?;

        if let DatePerhapsTime::DateTime(CalendarDateTime::WithTimezone { tzid, .. }) = start
            && tzid.parse::<chrono_tz::Tz>().is_err()
        {
            return Err(EventError::InvalidTimezone(tzid));
        }

        Ok(Event(inner.clone()))
    }

    pub fn ical_event(&self) -> &icalendar::Event {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_ics() {
        // Missing "END:VCALENDAR"
        let ics = "BEGIN:VCALENDAR\nVERSION:2.0\nBEGIN:VEVENT\nDTSTART:20240101T120000Z\nSUMMARY:Test Event\nEND:VEVENT";

        let result = Event::from_contents(ics);
        assert!(matches!(result, Err(EventError::InvalidIcs(_, _))));
    }

    #[test]
    fn rejects_ics_without_events() {
        let ics = "BEGIN:VCALENDAR\nVERSION:2.0\nEND:VCALENDAR";
        let result = Event::from_contents(ics);
        assert!(matches!(result, Err(EventError::NoEventInIcs(_))));
    }

    #[test]
    fn rejects_event_without_start() {
        let result = Event::from_ical_event(&icalendar::Event::new().done());

        assert!(matches!(result, Err(EventError::MissingStart)));
    }

    #[test]
    fn rejects_event_with_unparseable_tzid() {
        let ics = "BEGIN:VCALENDAR\nVERSION:2.0\nBEGIN:VEVENT\nDTSTART;TZID=Pacific Standard Time:20240101T120000\nSUMMARY:Test\nEND:VEVENT\nEND:VCALENDAR";

        let result = Event::from_contents(ics);

        assert!(
            matches!(result, Err(EventError::InvalidTimezone(tzid)) if tzid == "Pacific Standard Time")
        );
    }
}
