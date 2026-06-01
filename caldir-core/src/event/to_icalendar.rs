use crate::event::{Availability, Event, Status};
use icalendar::{Component, EventLike};

impl From<&Event> for icalendar::Event {
    fn from(value: &Event) -> Self {
        let mut event = icalendar::Event::new();
        event.starts(icalendar::DatePerhapsTime::from(&value.start));
        event.uid(value.uid.as_str());

        if let Some(end) = &value.end {
            event.ends(icalendar::DatePerhapsTime::from(end));
        }

        // Omit STATUS / TRANSP when they hold their RFC 5545 defaults so files
        // round-trip cleanly (a parsed-then-written event yields the same bytes
        // it came from, even if the original had no STATUS line). CLASS is
        // handled separately — it's optional, not defaulted.
        if value.status != Status::default() {
            event.append_property(icalendar::Property::new(
                "STATUS",
                value.status.as_ics_str(),
            ));
        }

        if value.availability != Availability::default() {
            event.append_property(icalendar::Property::new(
                "TRANSP",
                value.availability.as_ics_str(),
            ));
        }

        // Write CLASS whenever set (incl. PUBLIC); omit only when unspecified.
        if let Some(visibility) = value.visibility {
            event.append_property(icalendar::Property::new("CLASS", visibility.as_ics_str()));
        }

        if let Some(recurrence) = &value.recurrence {
            recurrence.apply_to(&mut event);
        }

        if let Some(recurrence_id) = &value.recurrence_id {
            event.recurrence_id(icalendar::DatePerhapsTime::from(
                recurrence_id.as_event_time(),
            ));
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

        if let Some(last_modified) = value.last_modified {
            event.last_modified(last_modified);
        }

        // Omit SEQUENCE when it holds its RFC 5545 default (0) for the same
        // round-trip-cleanliness reason as STATUS / TRANSP.
        if value.sequence != 0 {
            event.append_property(icalendar::Property::new(
                "SEQUENCE",
                value.sequence.to_string(),
            ));
        }

        if let Some(organizer) = &value.organizer {
            event.append_property(icalendar::Property::from(organizer));
        }

        for attendee in &value.attendees {
            event.append_multi_property(icalendar::Property::from(attendee));
        }

        // Reminders are emitted directly into the ICS string by
        // `Event::to_ics_string`, not via `event.alarm(...)`, so we sidestep
        // icalendar's auto-UID injection on VALARM sub-components. See
        // `Reminder::ics_block`.

        if let Some(url) = &value.url {
            event.append_property(icalendar::Property::new("URL", url));
        }

        for x in &value.x_properties {
            event.append_property(icalendar::Property::from(x));
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
    use crate::event::{EventUid, RecurrenceId, Visibility};
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
    fn converts_uid() {
        let mut event = test_event();
        event.uid = EventUid::new("abc123@hooli.com".to_string());

        let ical_event: icalendar::Event = event.into();

        assert_eq!(ical_event.get_uid(), Some("abc123@hooli.com"));
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

    #[test]
    fn converts_status() {
        let mut event = test_event();
        event.status = Status::Cancelled;

        let ical_event: icalendar::Event = event.into();

        assert_eq!(ical_event.property_value("STATUS"), Some("CANCELLED"));
    }

    #[test]
    fn omits_status_when_default() {
        let mut event = test_event();
        event.status = Status::Confirmed;

        let ical_event: icalendar::Event = event.into();

        assert_eq!(ical_event.property_value("STATUS"), None);
    }

    #[test]
    fn converts_availability() {
        let mut event = test_event();
        event.availability = Availability::Free;

        let ical_event: icalendar::Event = event.into();

        assert_eq!(ical_event.property_value("TRANSP"), Some("TRANSPARENT"));
    }

    #[test]
    fn omits_availability_when_default() {
        let mut event = test_event();
        event.availability = Availability::Busy;

        let ical_event: icalendar::Event = event.into();

        assert_eq!(ical_event.property_value("TRANSP"), None);
    }

    #[test]
    fn converts_visibility() {
        let mut event = test_event();
        event.visibility = Some(Visibility::Private);

        let ical_event: icalendar::Event = event.into();

        assert_eq!(ical_event.property_value("CLASS"), Some("PRIVATE"));
    }

    #[test]
    fn omits_visibility_when_unspecified() {
        let mut event = test_event();
        event.visibility = None;

        let ical_event: icalendar::Event = event.into();

        assert_eq!(ical_event.property_value("CLASS"), None);
    }

    #[test]
    fn writes_explicit_public_visibility() {
        // An explicit Some(Public) is distinct from unspecified and must be
        // written, so the "default vs public" distinction round-trips.
        let mut event = test_event();
        event.visibility = Some(Visibility::Public);

        let ical_event: icalendar::Event = event.into();

        assert_eq!(ical_event.property_value("CLASS"), Some("PUBLIC"));
    }

    #[test]
    fn converts_recurrence() {
        let mut event = test_event();
        event.recurrence = Some(crate::event::Recurrence::new("FREQ=WEEKLY;BYDAY=MO"));

        let ical_event: icalendar::Event = event.into();

        assert_eq!(
            ical_event.property_value("RRULE"),
            Some("FREQ=WEEKLY;BYDAY=MO")
        );
    }

    #[test]
    fn omits_recurrence_when_none() {
        let mut event = test_event();
        event.recurrence = None;

        let ical_event: icalendar::Event = event.into();

        assert_eq!(ical_event.property_value("RRULE"), None);
    }

    #[test]
    fn converts_recurrence_id() {
        let mut event = test_event();
        event.recurrence_id = Some(RecurrenceId::from_event_time(EventTime::Date(
            chrono::NaiveDate::from_ymd_opt(2026, 5, 15).unwrap(),
        )));

        let ical_event: icalendar::Event = event.into();

        assert_eq!(
            ical_event.get_recurrence_id(),
            Some(icalendar::DatePerhapsTime::Date(
                chrono::NaiveDate::from_ymd_opt(2026, 5, 15).unwrap()
            ))
        );
    }

    #[test]
    fn omits_recurrence_id_when_none() {
        let mut event = test_event();
        event.recurrence_id = None;

        let ical_event: icalendar::Event = event.into();

        assert_eq!(ical_event.get_recurrence_id(), None);
    }

    #[test]
    fn converts_last_modified() {
        let last_modified = chrono::NaiveDate::from_ymd_opt(2026, 5, 2)
            .unwrap()
            .and_hms_opt(17, 39, 14)
            .unwrap()
            .and_utc();

        let mut event = test_event();
        event.last_modified = Some(last_modified);

        let ical_event: icalendar::Event = event.into();

        assert_eq!(
            ical_event.property_value("LAST-MODIFIED"),
            Some("20260502T173914Z")
        );
    }

    #[test]
    fn omits_last_modified_when_none() {
        let mut event = test_event();
        event.last_modified = None;

        let ical_event: icalendar::Event = event.into();

        assert_eq!(ical_event.property_value("LAST-MODIFIED"), None);
    }

    #[test]
    fn converts_sequence() {
        let mut event = test_event();
        event.sequence = 3;

        let ical_event: icalendar::Event = event.into();

        assert_eq!(ical_event.property_value("SEQUENCE"), Some("3"));
    }

    #[test]
    fn converts_negative_sequence() {
        let mut event = test_event();
        event.sequence = -1;

        let ical_event: icalendar::Event = event.into();

        assert_eq!(ical_event.property_value("SEQUENCE"), Some("-1"));
    }

    #[test]
    fn omits_sequence_when_default() {
        let mut event = test_event();
        event.sequence = 0;

        let ical_event: icalendar::Event = event.into();

        assert_eq!(ical_event.property_value("SEQUENCE"), None);
    }

    #[test]
    fn converts_organizer() {
        let mut event = test_event();
        event.organizer = Some(crate::event::Organizer::new("alice@example.com"));

        let ical_event: icalendar::Event = event.into();

        assert_eq!(
            ical_event.property_value("ORGANIZER"),
            Some("mailto:alice@example.com")
        );
    }

    #[test]
    fn omits_organizer_when_none() {
        let mut event = test_event();
        event.organizer = None;

        let ical_event: icalendar::Event = event.into();

        assert!(ical_event.properties().get("ORGANIZER").is_none());
    }

    #[test]
    fn converts_attendees() {
        let mut event = test_event();
        event.attendees = vec![
            crate::event::Attendee::new("bob@example.com"),
            crate::event::Attendee::new("carol@example.com"),
        ];

        let ical_event: icalendar::Event = event.into();

        let attendees = ical_event
            .multi_properties()
            .get("ATTENDEE")
            .expect("ATTENDEE multi-property should be present");
        assert_eq!(
            attendees.iter().map(|p| p.value()).collect::<Vec<_>>(),
            vec!["mailto:bob@example.com", "mailto:carol@example.com"]
        );
    }

    #[test]
    fn omits_attendees_when_empty() {
        let mut event = test_event();
        event.attendees = vec![];

        let ical_event: icalendar::Event = event.into();

        assert!(ical_event.multi_properties().get("ATTENDEE").is_none());
    }

    // Reminders are plumbed via `Event::to_ics_string` rather than
    // `From<&Event> for icalendar::Event`, so the wire-through test for them
    // lives in `event.rs` instead.

    #[test]
    fn converts_url() {
        let mut event = test_event();
        event.url = Some("https://meet.example.com/abc-defg-hij".to_string());

        let ical_event: icalendar::Event = event.into();

        assert_eq!(
            ical_event.property_value("URL"),
            Some("https://meet.example.com/abc-defg-hij")
        );
    }

    #[test]
    fn omits_url_when_none() {
        let mut event = test_event();
        event.url = None;

        let ical_event: icalendar::Event = event.into();

        assert_eq!(ical_event.property_value("URL"), None);
    }

    #[test]
    fn converts_x_properties() {
        let mut event = test_event();
        event.x_properties = vec![crate::event::XProperty::new(
            "X-HOOLI-EVENT-ID",
            "abc123@hooli.com",
        )];

        let ical_event: icalendar::Event = event.into();

        assert_eq!(
            ical_event.property_value("X-HOOLI-EVENT-ID"),
            Some("abc123@hooli.com")
        );
    }

    #[test]
    fn omits_x_properties_when_empty() {
        let mut event = test_event();
        event.x_properties = vec![];

        let ical_event: icalendar::Event = event.into();

        assert!(ical_event.properties().keys().all(|k| !k.starts_with("X-")));
    }
}
