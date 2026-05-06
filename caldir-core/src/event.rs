mod error;
mod from_icalendar;
mod slugify;
mod time;
mod to_icalendar;

pub use error::EventError;
pub use time::{EventTime, EventTimeError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub summary: Option<String>,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start: EventTime,
    pub end: Option<EventTime>,
}

impl Event {
    pub fn new(summary: impl Into<String>, start: EventTime) -> Self {
        Event {
            summary: Some(summary.into()),
            description: None,
            location: None,
            start,
            end: None,
        }
    }

    pub(crate) fn from_ics_str(contents: &str) -> Result<Self, EventError> {
        let icalendar: icalendar::Calendar = contents
            .parse()
            .map_err(|err| EventError::InvalidIcs(contents.to_string(), err))?;

        let ical_event = icalendar
            .events()
            .next()
            .ok_or_else(|| EventError::NoEventInIcs(icalendar.clone()))?;

        ical_event.try_into()
    }

    pub(crate) fn to_ics_string(&self) -> String {
        let ical_event: icalendar::Event = self.into();
        let calendar = icalendar::Calendar::new().push(ical_event).done();
        calendar.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_ics() {
        // Missing "END:VCALENDAR"
        let ics = "BEGIN:VCALENDAR\nVERSION:2.0\nBEGIN:VEVENT\nDTSTART:20240101T120000Z\nSUMMARY:Test Event\nEND:VEVENT";

        let result = Event::from_ics_str(ics);
        assert!(matches!(result, Err(EventError::InvalidIcs(_, _))));
    }

    #[test]
    fn rejects_ics_without_events() {
        let ics = "BEGIN:VCALENDAR\nVERSION:2.0\nEND:VCALENDAR";
        let result = Event::from_ics_str(ics);
        assert!(matches!(result, Err(EventError::NoEventInIcs(_))));
    }

    #[test]
    fn rejects_event_without_start() {
        let result = Event::try_from(&icalendar::Event::new().done());

        assert!(matches!(result, Err(EventError::MissingStart)));
    }

    #[test]
    fn rejects_event_with_unparseable_tzid() {
        let ics = "BEGIN:VCALENDAR\nVERSION:2.0\nBEGIN:VEVENT\nDTSTART;TZID=Pacific Standard Time:20240101T120000\nSUMMARY:Test\nEND:VEVENT\nEND:VCALENDAR";

        let result = Event::from_ics_str(ics);

        assert!(matches!(
            result,
            Err(EventError::InvalidTime(EventTimeError::InvalidTimezone(tzid)))
                if tzid == "Pacific Standard Time"
        ));
    }

    #[test]
    fn parses_minimal_event_fields_from_ics() {
        let ics = "BEGIN:VCALENDAR\nVERSION:2.0\nBEGIN:VEVENT\nDTSTART:20240101T120000Z\nSUMMARY:Test Event\nLOCATION:Conference Room\nEND:VEVENT\nEND:VCALENDAR";

        let event = Event::from_ics_str(ics).unwrap();

        assert_eq!(event.summary.unwrap(), "Test Event");
        assert_eq!(event.location.as_deref(), Some("Conference Room"));
        assert!(matches!(event.start, EventTime::DateTimeUtc(_)));
    }
}
