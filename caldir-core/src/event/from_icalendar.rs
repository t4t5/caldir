use crate::event::{Event, EventError, EventTime};
use icalendar::{Component, EventLike};

impl TryFrom<&icalendar::Event> for Event {
    type Error = EventError;

    fn try_from(value: &icalendar::Event) -> Result<Self, Self::Error> {
        let start: EventTime = value
            .get_start()
            .ok_or(EventError::MissingStart)?
            .try_into()?;

        let end = value.get_end().map(EventTime::try_from).transpose()?;

        Ok(Event {
            summary: value.get_summary().map(ToString::to_string),
            description: value.get_description().map(ToString::to_string),
            location: value.get_location().map(ToString::to_string),
            start,
            end,
        })
    }
}

impl TryFrom<icalendar::Event> for Event {
    type Error = EventError;

    fn try_from(value: icalendar::Event) -> Result<Self, Self::Error> {
        Event::try_from(&value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::test_icalendar_event;

    #[test]
    fn succeeds_when_ical_event_has_start() {
        let ical_event = icalendar::Event::new()
            .starts(icalendar::DatePerhapsTime::Date(
                chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            ))
            .done();

        assert!(Event::try_from(ical_event).is_ok());
    }

    #[test]
    fn errors_when_ical_event_missing_start() {
        let ical_event = icalendar::Event::new()
            .summary("Test Event")
            .location("Test Location")
            .done();

        let result = Event::try_from(ical_event);

        assert!(matches!(result, Err(EventError::MissingStart)));
    }

    #[test]
    fn converts_summary() {
        let ical_event = test_icalendar_event().summary("Hello world").done();

        let event = Event::try_from(ical_event).unwrap();

        assert_eq!(event.summary.as_deref(), Some("Hello world"));
    }

    #[test]
    fn converts_location() {
        let ical_event = test_icalendar_event().location("London").done();

        let event = Event::try_from(ical_event).unwrap();

        assert_eq!(event.location.as_deref(), Some("London"));
    }

    #[test]
    fn converts_description() {
        let ical_event = test_icalendar_event()
            .description("Multi-line\nnotes")
            .done();

        let event = Event::try_from(ical_event).unwrap();

        assert_eq!(event.description.as_deref(), Some("Multi-line\nnotes"));
    }

    #[test]
    fn converts_start_date() {
        let ical_event = test_icalendar_event()
            .starts(icalendar::DatePerhapsTime::Date(
                chrono::NaiveDate::from_ymd_opt(2026, 2, 10).unwrap(),
            ))
            .done();

        let event = Event::try_from(ical_event).unwrap();

        assert_eq!(
            event.start,
            EventTime::Date(chrono::NaiveDate::from_ymd_opt(2026, 2, 10).unwrap())
        );
    }

    #[test]
    fn converts_end() {
        let ical_event = test_icalendar_event()
            .ends(icalendar::DatePerhapsTime::Date(
                chrono::NaiveDate::from_ymd_opt(2026, 2, 11).unwrap(),
            ))
            .done();

        let event = Event::try_from(ical_event).unwrap();

        assert_eq!(
            event.end,
            Some(EventTime::Date(
                chrono::NaiveDate::from_ymd_opt(2026, 2, 11).unwrap()
            ))
        );
    }

    #[test]
    fn end_is_none_when_missing() {
        let ical_event = test_icalendar_event().done();

        let event = Event::try_from(ical_event).unwrap();

        assert_eq!(event.end, None);
    }
}
