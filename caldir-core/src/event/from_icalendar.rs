use crate::event::{
    Attendee, Event, EventError, EventTime, EventUid, Organizer, Recurrence, RecurrenceId,
    Reminder, Status, Transparency, XProperty,
};
use icalendar::{Component, EventLike};

impl TryFrom<&icalendar::Event> for Event {
    type Error = EventError;

    fn try_from(value: &icalendar::Event) -> Result<Self, Self::Error> {
        let start: EventTime = value.get_start().ok_or(EventError::MissingStart)?.into();

        let end = value.get_end().map(EventTime::from);

        let recurrence = Recurrence::from_ical_event(value);

        let recurrence_id = value
            .get_recurrence_id()
            .map(EventTime::from)
            .map(RecurrenceId::from_event_time);

        let uid = value.get_uid().ok_or(EventError::MissingUid)?.to_string();

        let organizer = value.properties().get("ORGANIZER").map(Organizer::from);

        let attendees = value
            .multi_properties()
            .get("ATTENDEE")
            .map(|props| props.iter().map(Attendee::from).collect())
            .unwrap_or_default();

        // STATUS and TRANSP default to CONFIRMED / OPAQUE per RFC 5545, so a
        // missing line is treated as the default value rather than an
        // independent "unset" state.
        let status = value
            .property_value("STATUS")
            .and_then(Status::from_ics_str)
            .unwrap_or_default();

        let transparency = value
            .property_value("TRANSP")
            .and_then(Transparency::from_ics_str)
            .unwrap_or_default();

        let reminders = Reminder::from_ical_event(value);

        let x_properties = value
            .properties()
            .iter()
            .filter(|(name, _)| name.starts_with("X-"))
            .map(|(_, prop)| XProperty::from(prop))
            .collect();

        Ok(Event {
            uid: EventUid::new(uid),
            summary: value.get_summary().map(ToString::to_string),
            description: value.get_description().map(ToString::to_string),
            location: value.get_location().map(ToString::to_string),
            start,
            end,
            status,
            transparency,
            recurrence,
            recurrence_id,
            last_modified: value.get_last_modified(),
            // SEQUENCE defaults to 0 per RFC 5545; treat missing or
            // unparseable values the same as an explicit 0.
            sequence: value
                .property_value("SEQUENCE")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
            organizer,
            attendees,
            reminders,
            url: value.property_value("URL").map(ToString::to_string),
            x_properties,
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
        let ical_event = test_icalendar_event().uid("abc123@hooli.com").done();

        let event = Event::try_from(ical_event).unwrap();

        assert_eq!(event.uid.as_str(), "abc123@hooli.com");
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
    fn converts_status() {
        let ical_event = test_icalendar_event()
            .append_property(icalendar::Property::new("STATUS", "TENTATIVE"))
            .done();

        let event = Event::try_from(ical_event).unwrap();

        assert_eq!(event.status, Status::Tentative);
    }

    #[test]
    fn status_defaults_to_confirmed_when_missing() {
        let ical_event = test_icalendar_event().done();

        let event = Event::try_from(ical_event).unwrap();

        assert_eq!(event.status, Status::Confirmed);
    }

    #[test]
    fn converts_transparency() {
        let ical_event = test_icalendar_event()
            .append_property(icalendar::Property::new("TRANSP", "TRANSPARENT"))
            .done();

        let event = Event::try_from(ical_event).unwrap();

        assert_eq!(event.transparency, Transparency::Transparent);
    }

    #[test]
    fn transparency_defaults_to_opaque_when_missing() {
        let ical_event = test_icalendar_event().done();

        let event = Event::try_from(ical_event).unwrap();

        assert_eq!(event.transparency, Transparency::Opaque);
    }

    #[test]
    fn converts_recurrence() {
        let ical_event = test_icalendar_event()
            .append_property(icalendar::Property::new("RRULE", "FREQ=WEEKLY;BYDAY=MO"))
            .done();

        let event = Event::try_from(ical_event).unwrap();

        assert_eq!(
            event.recurrence,
            Some(crate::event::Recurrence::new("FREQ=WEEKLY;BYDAY=MO"))
        );
    }

    #[test]
    fn recurrence_is_none_when_missing() {
        let ical_event = test_icalendar_event().done();

        let event = Event::try_from(ical_event).unwrap();

        assert_eq!(event.recurrence, None);
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
            event.recurrence_id.unwrap().as_event_time(),
            &EventTime::Date(chrono::NaiveDate::from_ymd_opt(2026, 5, 15).unwrap())
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

        assert_eq!(event.sequence, 3);
    }

    #[test]
    fn converts_negative_sequence() {
        let ical_event = test_icalendar_event()
            .append_property(icalendar::Property::new("SEQUENCE", "-1"))
            .done();

        let event = Event::try_from(ical_event).unwrap();

        assert_eq!(event.sequence, -1);
    }

    #[test]
    fn sequence_defaults_to_zero_when_missing() {
        let ical_event = test_icalendar_event().done();

        let event = Event::try_from(ical_event).unwrap();

        assert_eq!(event.sequence, 0);
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
    fn converts_attendees() {
        let ical_event = test_icalendar_event()
            .append_multi_property(icalendar::Property::new(
                "ATTENDEE",
                "mailto:bob@example.com",
            ))
            .append_multi_property(icalendar::Property::new(
                "ATTENDEE",
                "mailto:carol@example.com",
            ))
            .done();

        let event = Event::try_from(ical_event).unwrap();

        assert_eq!(
            event.attendees,
            vec![
                crate::event::Attendee::new("bob@example.com"),
                crate::event::Attendee::new("carol@example.com"),
            ]
        );
    }

    #[test]
    fn attendees_is_empty_when_missing() {
        let ical_event = test_icalendar_event().done();

        let event = Event::try_from(ical_event).unwrap();

        assert!(event.attendees.is_empty());
    }

    #[test]
    fn converts_reminders() {
        let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:test@caldir\r\nDTSTART:20260101T120000Z\r\nBEGIN:VALARM\r\nACTION:DISPLAY\r\nDESCRIPTION:Reminder\r\nTRIGGER:-PT10M\r\nEND:VALARM\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let event = crate::event::Event::parse_single_ics(ics);

        assert_eq!(event.reminders.len(), 1);
        assert_eq!(event.reminders[0], crate::event::Reminder::from_minutes(10));
    }

    #[test]
    fn reminders_is_empty_when_missing() {
        let ical_event = test_icalendar_event().done();

        let event = Event::try_from(ical_event).unwrap();

        assert!(event.reminders.is_empty());
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

    #[test]
    fn converts_x_properties() {
        let ical_event = test_icalendar_event()
            .append_property(icalendar::Property::new(
                "X-HOOLI-EVENT-ID",
                "abc123@hooli.com",
            ))
            .done();

        let event = Event::try_from(ical_event).unwrap();

        assert_eq!(
            event.x_properties,
            vec![XProperty::new("X-HOOLI-EVENT-ID", "abc123@hooli.com")]
        );
    }

    #[test]
    fn ignores_non_x_properties() {
        let ical_event = test_icalendar_event().summary("Hello").done();

        let event = Event::try_from(ical_event).unwrap();

        assert!(event.x_properties.is_empty());
    }
}
