use crate::event::Event;
use icalendar::{Component, EventLike};

impl From<&Event> for icalendar::Event {
    fn from(value: &Event) -> Self {
        let mut event = icalendar::Event::new();
        event.starts(icalendar::DatePerhapsTime::from(&value.start));
        event.uid(&value.uid);

        if let Some(end) = &value.end {
            event.ends(icalendar::DatePerhapsTime::from(end));
        }

        if let Some(recurrence) = &value.recurrence {
            recurrence.apply_to(&mut event);
        }

        if let Some(recurrence_id) = &value.recurrence_id {
            event.recurrence_id(icalendar::DatePerhapsTime::from(recurrence_id));
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

        if let Some(sequence) = value.sequence {
            event.append_property(icalendar::Property::new("SEQUENCE", sequence.to_string()));
        }

        if let Some(organizer) = &value.organizer {
            event.append_property(icalendar::Property::from(organizer));
        }

        for attendee in &value.attendees {
            event.append_multi_property(icalendar::Property::from(attendee));
        }

        for reminder in &value.reminders {
            event.alarm(icalendar::Alarm::from(reminder));
        }

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
        event.uid = "abc123@google.com".to_string();

        let ical_event: icalendar::Event = event.into();

        assert_eq!(ical_event.get_uid(), Some("abc123@google.com"));
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
        event.recurrence_id = Some(EventTime::Date(
            chrono::NaiveDate::from_ymd_opt(2026, 5, 15).unwrap(),
        ));

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
        event.sequence = Some(3);

        let ical_event: icalendar::Event = event.into();

        assert_eq!(ical_event.property_value("SEQUENCE"), Some("3"));
    }

    #[test]
    fn converts_negative_sequence() {
        let mut event = test_event();
        event.sequence = Some(-1);

        let ical_event: icalendar::Event = event.into();

        assert_eq!(ical_event.property_value("SEQUENCE"), Some("-1"));
    }

    #[test]
    fn omits_sequence_when_none() {
        let mut event = test_event();
        event.sequence = None;

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

    #[test]
    fn converts_reminders() {
        use chrono::Duration;
        let mut event = test_event();
        event.reminders = vec![crate::event::Reminder {
            trigger: crate::event::ReminderTrigger::Relative {
                offset: Duration::minutes(-10),
                related: crate::event::Related::Start,
            },
            action: crate::event::ReminderAction::Display,
            description: Some("Reminder".to_string()),
        }];

        let ical_event: icalendar::Event = event.into();

        let alarms: Vec<_> = ical_event
            .components()
            .iter()
            .filter(|c| c.component_kind() == "VALARM")
            .collect();
        assert_eq!(alarms.len(), 1);
        assert_eq!(alarms[0].property_value("ACTION"), Some("DISPLAY"));
        assert_eq!(alarms[0].property_value("TRIGGER"), Some("-PT10M"));
    }

    #[test]
    fn omits_reminders_when_empty() {
        let mut event = test_event();
        event.reminders = vec![];

        let ical_event: icalendar::Event = event.into();

        let alarms: Vec<_> = ical_event
            .components()
            .iter()
            .filter(|c| c.component_kind() == "VALARM")
            .collect();
        assert!(alarms.is_empty());
    }

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
            "X-GOOGLE-EVENT-ID",
            "abc123@google.com",
        )];

        let ical_event: icalendar::Event = event.into();

        assert_eq!(
            ical_event.property_value("X-GOOGLE-EVENT-ID"),
            Some("abc123@google.com")
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
