use crate::event::{EventTime, EventTimeError};
use icalendar::{Component, DatePerhapsTime, Property};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recurrence {
    /// RRULE value (after "RRULE:"), e.g. "FREQ=WEEKLY;BYDAY=MO".
    ///
    /// Stored as a string rather than a typed `rrule::RRule` for two reasons:
    /// 1. Robustness — providers (especially CalDAV servers) emit RRULEs that
    ///    the rrule crate can't always parse. Typing the field would turn every
    ///    such case into a load error and break sync for that event.
    /// 2. Round-tripping — `rrule::RRule`'s Display re-serializes from typed
    ///    fields and drops redundant defaults (e.g. INTERVAL=1), causing
    ///    spurious "modified" diffs on subsequent syncs.
    ///
    /// Parse with `rrule::RRule::from_str` at the call site when typed access
    /// is needed (e.g. expanding occurrences).
    pub rrule: String,
    pub exdates: Vec<EventTime>,
    /// RDATE values — explicit dates added to the recurrence set.
    /// Sometimes emited by CalDAV servers like iCloud
    ///
    /// Note: RFC 5545 also allows PERIOD values (`DTSTART/DURATION` pairs)
    /// in RDATE, but those are not modeled here — only date and date-time
    /// values are parsed. PERIOD-valued RDATEs will be silently dropped.
    pub rdates: Vec<EventTime>,
}

impl Recurrence {
    pub fn new(rrule: impl Into<String>) -> Self {
        Recurrence {
            rrule: rrule.into(),
            exdates: Vec::new(),
            rdates: Vec::new(),
        }
    }

    pub(crate) fn from_ical_event(
        event: &icalendar::Event,
    ) -> Result<Option<Self>, EventTimeError> {
        let Some(rrule) = event.property_value("RRULE") else {
            return Ok(None);
        };

        let exdates = parse_event_time_list(event, "EXDATE")?;
        let rdates = parse_event_time_list(event, "RDATE")?;

        Ok(Some(Recurrence {
            rrule: rrule.to_string(),
            exdates,
            rdates,
        }))
    }

    pub(crate) fn apply_to(&self, event: &mut icalendar::Event) {
        event.append_property(Property::new("RRULE", &self.rrule));
        for exdate in &self.exdates {
            event.append_multi_property(DatePerhapsTime::from(exdate).to_property("EXDATE"));
        }
        for rdate in &self.rdates {
            event.append_multi_property(DatePerhapsTime::from(rdate).to_property("RDATE"));
        }
    }
}

fn parse_event_time_list(
    event: &icalendar::Event,
    name: &str,
) -> Result<Vec<EventTime>, EventTimeError> {
    Ok(event
        .multi_properties()
        .get(name)
        .map(|props| {
            props
                .iter()
                .filter_map(DatePerhapsTime::from_property)
                .map(EventTime::try_from)
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::test_icalendar_event;
    use chrono::NaiveDate;
    use pretty_assertions::assert_eq;

    #[test]
    fn from_ical_event_returns_none_when_no_rrule() {
        let event = test_icalendar_event().done();

        let recurrence = Recurrence::from_ical_event(&event).unwrap();

        assert_eq!(recurrence, None);
    }

    #[test]
    fn from_ical_event_parses_rrule_value() {
        let event = test_icalendar_event()
            .append_property(Property::new("RRULE", "FREQ=WEEKLY;BYDAY=MO"))
            .done();

        let recurrence = Recurrence::from_ical_event(&event).unwrap().unwrap();

        assert_eq!(recurrence.rrule, "FREQ=WEEKLY;BYDAY=MO");
        assert!(recurrence.exdates.is_empty());
        assert!(recurrence.rdates.is_empty());
    }

    #[test]
    fn from_ical_event_parses_multiple_exdates() {
        let event = test_icalendar_event()
            .append_property(Property::new("RRULE", "FREQ=DAILY"))
            .append_multi_property(Property::new("EXDATE", "20260105").done())
            .append_multi_property(Property::new("EXDATE", "20260108").done())
            .done();

        let recurrence = Recurrence::from_ical_event(&event).unwrap().unwrap();

        assert_eq!(
            recurrence.exdates,
            vec![
                EventTime::Date(NaiveDate::from_ymd_opt(2026, 1, 5).unwrap()),
                EventTime::Date(NaiveDate::from_ymd_opt(2026, 1, 8).unwrap()),
            ]
        );
    }

    #[test]
    fn from_ical_event_parses_zoned_exdate() {
        let mut exdate = Property::new("EXDATE", "20260105T100000");
        exdate.add_parameter("TZID", "Europe/Stockholm");

        let event = test_icalendar_event()
            .append_property(Property::new("RRULE", "FREQ=WEEKLY"))
            .append_multi_property(exdate.done())
            .done();

        let recurrence = Recurrence::from_ical_event(&event).unwrap().unwrap();

        assert_eq!(
            recurrence.exdates,
            vec![EventTime::DateTimeZoned {
                datetime: NaiveDate::from_ymd_opt(2026, 1, 5)
                    .unwrap()
                    .and_hms_opt(10, 0, 0)
                    .unwrap(),
                tzid: "Europe/Stockholm".to_string(),
            }]
        );
    }

    #[test]
    fn from_ical_event_preserves_arbitrary_exdate_timezone() {
        let mut exdate = Property::new("EXDATE", "20260105T100000");
        exdate.add_parameter("TZID", "Pacific Standard Time");

        let event = test_icalendar_event()
            .append_property(Property::new("RRULE", "FREQ=WEEKLY"))
            .append_multi_property(exdate.done())
            .done();

        let recurrence = Recurrence::from_ical_event(&event).unwrap().unwrap();

        assert_eq!(
            recurrence.exdates,
            vec![EventTime::DateTimeZoned {
                datetime: NaiveDate::from_ymd_opt(2026, 1, 5)
                    .unwrap()
                    .and_hms_opt(10, 0, 0)
                    .unwrap(),
                tzid: "Pacific Standard Time".to_string(),
            }]
        );
    }

    #[test]
    fn from_ical_event_parses_multiple_rdates() {
        let event = test_icalendar_event()
            .append_property(Property::new("RRULE", "FREQ=DAILY"))
            .append_multi_property(Property::new("RDATE", "20260201").done())
            .append_multi_property(Property::new("RDATE", "20260215").done())
            .done();

        let recurrence = Recurrence::from_ical_event(&event).unwrap().unwrap();

        assert_eq!(
            recurrence.rdates,
            vec![
                EventTime::Date(NaiveDate::from_ymd_opt(2026, 2, 1).unwrap()),
                EventTime::Date(NaiveDate::from_ymd_opt(2026, 2, 15).unwrap()),
            ]
        );
    }

    #[test]
    fn from_ical_event_parses_zoned_rdate() {
        let mut rdate = Property::new("RDATE", "20260201T100000");
        rdate.add_parameter("TZID", "Europe/Stockholm");

        let event = test_icalendar_event()
            .append_property(Property::new("RRULE", "FREQ=WEEKLY"))
            .append_multi_property(rdate.done())
            .done();

        let recurrence = Recurrence::from_ical_event(&event).unwrap().unwrap();

        assert_eq!(
            recurrence.rdates,
            vec![EventTime::DateTimeZoned {
                datetime: NaiveDate::from_ymd_opt(2026, 2, 1)
                    .unwrap()
                    .and_hms_opt(10, 0, 0)
                    .unwrap(),
                tzid: "Europe/Stockholm".to_string(),
            }]
        );
    }

    #[test]
    fn from_ical_event_preserves_arbitrary_rdate_timezone() {
        let mut rdate = Property::new("RDATE", "20260201T100000");
        rdate.add_parameter("TZID", "Pacific Standard Time");

        let event = test_icalendar_event()
            .append_property(Property::new("RRULE", "FREQ=WEEKLY"))
            .append_multi_property(rdate.done())
            .done();

        let recurrence = Recurrence::from_ical_event(&event).unwrap().unwrap();

        assert_eq!(
            recurrence.rdates,
            vec![EventTime::DateTimeZoned {
                datetime: NaiveDate::from_ymd_opt(2026, 2, 1)
                    .unwrap()
                    .and_hms_opt(10, 0, 0)
                    .unwrap(),
                tzid: "Pacific Standard Time".to_string(),
            }]
        );
    }

    #[test]
    fn from_ical_event_parses_exdates_and_rdates_together() {
        let event = test_icalendar_event()
            .append_property(Property::new("RRULE", "FREQ=WEEKLY"))
            .append_multi_property(Property::new("EXDATE", "20260105").done())
            .append_multi_property(Property::new("RDATE", "20260201").done())
            .done();

        let recurrence = Recurrence::from_ical_event(&event).unwrap().unwrap();

        assert_eq!(
            recurrence.exdates,
            vec![EventTime::Date(
                NaiveDate::from_ymd_opt(2026, 1, 5).unwrap()
            )]
        );
        assert_eq!(
            recurrence.rdates,
            vec![EventTime::Date(
                NaiveDate::from_ymd_opt(2026, 2, 1).unwrap()
            )]
        );
    }

    #[test]
    fn apply_to_writes_rrule_exdates_and_rdates() {
        let recurrence = Recurrence {
            rrule: "FREQ=WEEKLY;BYDAY=MO".to_string(),
            exdates: vec![
                EventTime::Date(NaiveDate::from_ymd_opt(2026, 1, 5).unwrap()),
                EventTime::Date(NaiveDate::from_ymd_opt(2026, 1, 12).unwrap()),
            ],
            rdates: vec![
                EventTime::Date(NaiveDate::from_ymd_opt(2026, 2, 1).unwrap()),
                EventTime::Date(NaiveDate::from_ymd_opt(2026, 2, 15).unwrap()),
            ],
        };

        let mut event = icalendar::Event::new();
        recurrence.apply_to(&mut event);

        assert_eq!(event.property_value("RRULE"), Some("FREQ=WEEKLY;BYDAY=MO"));
        let exdates = event.multi_properties().get("EXDATE").unwrap();
        assert_eq!(exdates.len(), 2);
        assert_eq!(exdates[0].value(), "20260105");
        assert_eq!(exdates[1].value(), "20260112");
        let rdates = event.multi_properties().get("RDATE").unwrap();
        assert_eq!(rdates.len(), 2);
        assert_eq!(rdates[0].value(), "20260201");
        assert_eq!(rdates[1].value(), "20260215");
    }

    #[test]
    fn apply_to_omits_exdate_and_rdate_when_empty() {
        let recurrence = Recurrence::new("FREQ=DAILY");

        let mut event = icalendar::Event::new();
        recurrence.apply_to(&mut event);

        assert!(event.multi_properties().get("EXDATE").is_none());
        assert!(event.multi_properties().get("RDATE").is_none());
    }
}
