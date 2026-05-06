use crate::event::Event;
use icalendar::{Component, EventLike};

impl From<&Event> for icalendar::Event {
    fn from(value: &Event) -> Self {
        let mut event = icalendar::Event::new();
        event.starts(icalendar::DatePerhapsTime::from(&value.start));

        if let Some(end) = &value.end {
            event.ends(icalendar::DatePerhapsTime::from(end));
        }

        if let Some(summary) = &value.summary {
            event.summary(summary);
        }

        if let Some(description) = &value.description {
            event.description(description);
        }

        if let Some(location) = &value.location {
            event.location(location);
        }

        event.done()
    }
}

impl From<Event> for icalendar::Event {
    fn from(value: Event) -> Self {
        icalendar::Event::from(&value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EventTime;
    use crate::test_utils::test_event;

    #[test]
    fn converts_summary() {
        let mut event = test_event();
        event.summary = Some("Hello world".to_string());

        let ical_event: icalendar::Event = event.into();

        assert_eq!(ical_event.get_summary(), Some("Hello world"));
    }

    #[test]
    fn converts_location() {
        let mut event = test_event();
        event.location = Some("New York".to_string());

        let ical_event: icalendar::Event = event.into();

        assert_eq!(ical_event.get_location(), Some("New York"));
    }

    #[test]
    fn converts_description() {
        let mut event = test_event();
        event.description = Some("Multi-line\nnotes".to_string());

        let ical_event: icalendar::Event = event.into();

        assert_eq!(ical_event.get_description(), Some("Multi-line\nnotes"));
    }

    #[test]
    fn converts_start() {
        let mut event = test_event();
        event.start = EventTime::Date(chrono::NaiveDate::from_ymd_opt(2026, 10, 10).unwrap());

        let ical_event: icalendar::Event = event.into();

        assert_eq!(
            ical_event.get_start(),
            Some(icalendar::DatePerhapsTime::Date(
                chrono::NaiveDate::from_ymd_opt(2026, 10, 10).unwrap()
            ))
        );
    }

    #[test]
    fn converts_end() {
        let mut event = test_event();
        event.end = Some(EventTime::Date(
            chrono::NaiveDate::from_ymd_opt(2026, 10, 11).unwrap(),
        ));

        let ical_event: icalendar::Event = event.into();

        assert_eq!(
            ical_event.get_end(),
            Some(icalendar::DatePerhapsTime::Date(
                chrono::NaiveDate::from_ymd_opt(2026, 10, 11).unwrap()
            ))
        );
    }

    #[test]
    fn omits_end_when_none() {
        let mut event = test_event();
        event.end = None;

        let ical_event: icalendar::Event = event.into();

        assert_eq!(ical_event.get_end(), None);
    }
}
