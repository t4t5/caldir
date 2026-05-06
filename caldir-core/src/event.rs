mod error;
mod from_icalendar;
mod slugify;
mod time;
mod to_icalendar;

pub use error::EventError;
pub use time::{EventTime, EventTimeError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub uid: String,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start: EventTime,
    pub end: Option<EventTime>,
}

impl Event {
    pub fn new(summary: impl Into<String>, start: EventTime) -> Self {
        Event {
            uid: new_uid(),
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

fn new_uid() -> String {
    format!("{}@caldir", uuid::Uuid::new_v4())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_generates_uid_with_caldir_domain() {
        let event = Event::new(
            "Test",
            time::EventTime::Date(chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
        );

        assert!(event.uid.ends_with("@caldir"));
        let prefix = event.uid.trim_end_matches("@caldir");
        assert!(uuid::Uuid::parse_str(prefix).is_ok());
    }

    #[test]
    fn new_generates_unique_uids() {
        let start = time::EventTime::Date(chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
        let a = Event::new("Test", start.clone());
        let b = Event::new("Test", start);

        assert_ne!(a.uid, b.uid);
    }

    #[test]
    fn rejects_invalid_ics() {
        // Missing "END:VCALENDAR"
        let ics = "BEGIN:VCALENDAR\nVERSION:2.0\nBEGIN:VEVENT\nUID:test-uid@caldir\nDTSTART:20240101T120000Z\nSUMMARY:Test Event\nEND:VEVENT";

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
        let ics = "BEGIN:VCALENDAR\nVERSION:2.0\nBEGIN:VEVENT\nUID:test-uid@caldir\nDTSTART;TZID=Pacific Standard Time:20240101T120000\nSUMMARY:Test\nEND:VEVENT\nEND:VCALENDAR";

        let result = Event::from_ics_str(ics);

        assert!(matches!(
            result,
            Err(EventError::InvalidTime(EventTimeError::InvalidTimezone(tzid)))
                if tzid == "Pacific Standard Time"
        ));
    }

    #[test]
    fn parses_minimal_event_fields_from_ics() {
        let ics = "BEGIN:VCALENDAR\nVERSION:2.0\nBEGIN:VEVENT\nUID:test-uid@caldir\nDTSTART:20240101T120000Z\nSUMMARY:Test Event\nLOCATION:Conference Room\nEND:VEVENT\nEND:VCALENDAR";

        let event = Event::from_ics_str(ics).unwrap();

        assert_eq!(event.summary.unwrap(), "Test Event");
        assert_eq!(event.location.as_deref(), Some("Conference Room"));
        assert!(matches!(event.start, EventTime::DateTimeUtc(_)));
    }
}
