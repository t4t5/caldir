use crate::event::{Event, EventError, EventTime, Organizer};
use icalendar::{Component, EventLike};

impl TryFrom<&icalendar::Event> for Event {
    type Error = EventError;

    fn try_from(value: &icalendar::Event) -> Result<Self, Self::Error> {
        let start: EventTime = value
            .get_start()
            .ok_or(EventError::MissingStart)?
            .try_into()?;

        let end = value.get_end().map(EventTime::try_from).transpose()?;

        let recurrence_id = value
            .get_recurrence_id()
            .map(EventTime::try_from)
            .transpose()?;

        let uid = value.get_uid().ok_or(EventError::MissingUid)?.to_string();

        let organizer = value.properties().get("ORGANIZER").map(Organizer::from);

        Ok(Event {
            uid,
            summary: value.get_summary().map(ToString::to_string),
            description: value.get_description().map(ToString::to_string),
            location: value.get_location().map(ToString::to_string),
            start,
            end,
            recurrence_id,
            last_modified: value.get_last_modified(),
            sequence: value
                .property_value("SEQUENCE")
                .and_then(|s| s.parse().ok()),
            organizer,
            url: value.property_value("URL").map(ToString::to_string),
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
    fn succeeds_when_ical_event_has_start_and_uid() {
        let ical_event = test_icalendar_event().done();

        assert!(Event::try_from(ical_event).is_ok());
    }

    #[test]
    fn errors_when_ical_event_missing_uid() {
        let ical_event = icalendar::Event::new()
            .starts(icalendar::DatePerhapsTime::Date(
                chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            ))
            .summary("Test Event")
            .done();

        let result = Event::try_from(ical_event);

        assert!(matches!(result, Err(EventError::MissingUid)));
    }

    #[test]
    fn errors_when_ical_event_missing_start() {
        let ical_event = icalendar::Event::new()
            .uid("test-uid@caldir")
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
    fn converts_uid() {
        let ical_event = test_icalendar_event().uid("abc123@google.com").done();

        let event = Event::try_from(ical_event).unwrap();

        assert_eq!(event.uid, "abc123@google.com");
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

    #[test]
    fn converts_recurrence_id() {
        let ical_event = test_icalendar_event()
            .recurrence_id(icalendar::DatePerhapsTime::Date(
                chrono::NaiveDate::from_ymd_opt(2026, 5, 15).unwrap(),
            ))
            .done();

        let event = Event::try_from(ical_event).unwrap();

        assert_eq!(
            event.recurrence_id,
            Some(EventTime::Date(
                chrono::NaiveDate::from_ymd_opt(2026, 5, 15).unwrap()
            ))
        );
    }

    #[test]
    fn recurrence_id_is_none_when_missing() {
        let ical_event = test_icalendar_event().done();

        let event = Event::try_from(ical_event).unwrap();

        assert_eq!(event.recurrence_id, None);
    }

    #[test]
    fn converts_last_modified() {
        let last_modified = chrono::NaiveDate::from_ymd_opt(2026, 5, 2)
            .unwrap()
            .and_hms_opt(17, 39, 14)
            .unwrap()
            .and_utc();

        let ical_event = test_icalendar_event().last_modified(last_modified).done();

        let event = Event::try_from(ical_event).unwrap();

        assert_eq!(event.last_modified, Some(last_modified));
    }

    #[test]
    fn last_modified_is_none_when_missing() {
        let ical_event = test_icalendar_event().done();

        let event = Event::try_from(ical_event).unwrap();

        assert_eq!(event.last_modified, None);
    }

    #[test]
    fn converts_sequence() {
        let ical_event = test_icalendar_event().sequence(3).done();

        let event = Event::try_from(ical_event).unwrap();

        assert_eq!(event.sequence, Some(3));
    }

    #[test]
    fn converts_negative_sequence() {
        let ical_event = test_icalendar_event()
            .append_property(icalendar::Property::new("SEQUENCE", "-1"))
            .done();

        let event = Event::try_from(ical_event).unwrap();

        assert_eq!(event.sequence, Some(-1));
    }

    #[test]
    fn sequence_is_none_when_missing() {
        let ical_event = test_icalendar_event().done();

        let event = Event::try_from(ical_event).unwrap();

        assert_eq!(event.sequence, None);
    }

    #[test]
    fn converts_organizer() {
        let ical_event = test_icalendar_event()
            .append_property(icalendar::Property::new(
                "ORGANIZER",
                "mailto:alice@example.com",
            ))
            .done();

        let event = Event::try_from(ical_event).unwrap();

        assert_eq!(event.organizer, Some(Organizer::new("alice@example.com")));
    }

    #[test]
    fn organizer_is_none_when_missing() {
        let ical_event = test_icalendar_event().done();

        let event = Event::try_from(ical_event).unwrap();

        assert_eq!(event.organizer, None);
    }

    #[test]
    fn converts_url() {
        let ical_event = test_icalendar_event()
            .append_property(icalendar::Property::new(
                "URL",
                "https://meet.example.com/abc-defg-hij",
            ))
            .done();

        let event = Event::try_from(ical_event).unwrap();

        assert_eq!(
            event.url.as_deref(),
            Some("https://meet.example.com/abc-defg-hij")
        );
    }

    #[test]
    fn url_is_none_when_missing() {
        let ical_event = test_icalendar_event().done();

        let event = Event::try_from(ical_event).unwrap();

        assert_eq!(event.url, None);
    }
}
