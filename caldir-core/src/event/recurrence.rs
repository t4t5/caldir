use crate::event::EventTime;
use icalendar::{Component, DatePerhapsTime, Property};

#[derive(Debug, Clone, Eq)]
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

// EXDATE/RDATE are sets per RFC 5545, so equality must ignore order.
// Otherwise a provider re-emitting the same dates in a different order
// produces a spurious "modified" diff and a no-op push/pull.
impl PartialEq for Recurrence {
    fn eq(&self, other: &Self) -> bool {
        self.rrule == other.rrule
            && sorted_by_utc(&self.exdates) == sorted_by_utc(&other.exdates)
            && sorted_by_utc(&self.rdates) == sorted_by_utc(&other.rdates)
    }
}

fn sorted_by_utc(times: &[EventTime]) -> Vec<EventTime> {
    let mut v = times.to_vec();
    v.sort_by_key(|t| t.to_utc());
    v
}

impl Recurrence {
    pub fn new(rrule: impl Into<String>) -> Self {
        Recurrence {
            rrule: rrule.into(),
            exdates: Vec::new(),
            rdates: Vec::new(),
        }
    }

    pub(crate) fn from_ical_event(event: &icalendar::Event) -> Option<Self> {
        let rrule = event.property_value("RRULE")?;

        let exdates = parse_event_time_list(event, "EXDATE");
        let rdates = parse_event_time_list(event, "RDATE");

        Some(Recurrence {
            rrule: rrule.to_string(),
            exdates,
            rdates,
        })
    }

    /// Return a copy truncated to end strictly before `before`.
    ///
    /// The returned recurrence:
    /// - Has any existing `UNTIL=` and `COUNT=` fragments stripped from its RRULE
    ///   and replaced with a fresh `UNTIL=<just before `before`>`.
    /// - Has any EXDATEs at or after `before` removed (they are no longer
    ///   meaningful once the series ends earlier).
    ///
    /// `dtstart` is the master event's start time; it determines the format of
    /// the emitted UNTIL value (per RFC 5545 / the rrule crate's matching rule).
    ///
    /// Used to implement "split a recurring series at this instance" — a primitive
    /// that any caldir client can use to translate a "this and future" edit into
    /// two separate events on disk.
    pub fn truncate_before(&self, dtstart: &EventTime, before: &EventTime) -> Recurrence {
        let until_value = format_until_value(dtstart, before);
        let truncated_rrule = with_until(&self.rrule, &until_value);

        let before_utc = before.to_utc();
        let kept_exdates: Vec<EventTime> = self
            .exdates
            .iter()
            .filter(|ex| ex.to_utc() < before_utc)
            .cloned()
            .collect();

        Recurrence {
            rrule: truncated_rrule,
            exdates: kept_exdates,
            rdates: self.rdates.clone(),
        }
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

/// Format an RRULE UNTIL value for a series with `dtstart`, set "just before"
/// `before`. Format matches the rrule crate's expectations for the dtstart
/// variant (see RFC 5545).
fn format_until_value(dtstart: &EventTime, before: &EventTime) -> String {
    use chrono::Duration;

    match dtstart {
        EventTime::Date(_) => {
            let before_date = match before {
                EventTime::Date(d) => *d,
                _ => before.to_utc().date_naive(),
            };
            (before_date - Duration::days(1))
                .format("%Y%m%d")
                .to_string()
        }
        EventTime::DateTimeUtc(_) | EventTime::DateTimeZoned { .. } => {
            let before_utc = before.to_utc();
            (before_utc - Duration::seconds(1))
                .format("%Y%m%dT%H%M%SZ")
                .to_string()
        }
        EventTime::DateTimeFloating(_) => {
            let before_naive = before.to_utc().naive_utc();
            (before_naive - Duration::seconds(1))
                .format("%Y%m%dT%H%M%S")
                .to_string()
        }
    }
}

/// Return a copy of `rrule` with any existing `UNTIL=` and `COUNT=` fragments
/// removed and a new `UNTIL=<until_value>` appended. Order of other fragments
/// is preserved.
fn with_until(rrule: &str, until_value: &str) -> String {
    let mut parts: Vec<String> = rrule
        .split(';')
        .filter(|p| !p.is_empty())
        .filter(|p| !p.starts_with("UNTIL=") && !p.starts_with("COUNT="))
        .map(|p| p.to_string())
        .collect();
    parts.push(format!("UNTIL={}", until_value));
    parts.join(";")
}

fn parse_event_time_list(event: &icalendar::Event, name: &str) -> Vec<EventTime> {
    event
        .multi_properties()
        .get(name)
        .map(|props| {
            props
                .iter()
                .flat_map(split_property_values)
                .filter_map(|p| DatePerhapsTime::from_property(&p))
                .map(EventTime::from)
                .collect()
        })
        .unwrap_or_default()
}

// RFC 5545 allows EXDATE/RDATE to carry multiple comma-separated values on a
// single line. `DatePerhapsTime::from_property` parses the whole value as one
// date(-time), so it returns None for comma-packed properties and every entry
// is silently dropped. Split the value here and rebuild a property per entry,
// preserving parameters (TZID, VALUE) so downstream parsing still sees them.
fn split_property_values(prop: &Property) -> Vec<Property> {
    let key = prop.key();
    prop.value()
        .split(',')
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| {
            let mut split = Property::new(key, v);
            for (param_key, param) in prop.params() {
                split.add_parameter(param_key, param.value());
            }
            split
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::test_icalendar_event;
    use chrono::{NaiveDate, TimeZone};
    use pretty_assertions::assert_eq;

    #[test]
    fn from_ical_event_returns_none_when_no_rrule() {
        let event = test_icalendar_event().done();

        let recurrence = Recurrence::from_ical_event(&event);

        assert_eq!(recurrence, None);
    }

    #[test]
    fn from_ical_event_parses_rrule_value() {
        let event = test_icalendar_event()
            .append_property(Property::new("RRULE", "FREQ=WEEKLY;BYDAY=MO"))
            .done();

        let recurrence = Recurrence::from_ical_event(&event).unwrap();

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

        let recurrence = Recurrence::from_ical_event(&event).unwrap();

        assert_eq!(
            recurrence.exdates,
            vec![
                EventTime::Date(NaiveDate::from_ymd_opt(2026, 1, 5).unwrap()),
                EventTime::Date(NaiveDate::from_ymd_opt(2026, 1, 8).unwrap()),
            ]
        );
    }

    #[test]
    fn from_ical_event_parses_comma_separated_exdates() {
        let event = test_icalendar_event()
            .append_property(Property::new("RRULE", "FREQ=DAILY"))
            .append_multi_property(Property::new("EXDATE", "20260105,20260108").done())
            .done();

        let recurrence = Recurrence::from_ical_event(&event).unwrap();

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

        let recurrence = Recurrence::from_ical_event(&event).unwrap();

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
    fn from_ical_event_normalizes_windows_exdate_timezone_to_iana() {
        let mut exdate = Property::new("EXDATE", "20260105T100000");
        exdate.add_parameter("TZID", "Pacific Standard Time");

        let event = test_icalendar_event()
            .append_property(Property::new("RRULE", "FREQ=WEEKLY"))
            .append_multi_property(exdate.done())
            .done();

        let recurrence = Recurrence::from_ical_event(&event).unwrap();

        assert_eq!(
            recurrence.exdates,
            vec![EventTime::DateTimeZoned {
                datetime: NaiveDate::from_ymd_opt(2026, 1, 5)
                    .unwrap()
                    .and_hms_opt(10, 0, 0)
                    .unwrap(),
                tzid: "America/Los_Angeles".to_string(),
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

        let recurrence = Recurrence::from_ical_event(&event).unwrap();

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

        let recurrence = Recurrence::from_ical_event(&event).unwrap();

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
    fn from_ical_event_normalizes_windows_rdate_timezone_to_iana() {
        let mut rdate = Property::new("RDATE", "20260201T100000");
        rdate.add_parameter("TZID", "Pacific Standard Time");

        let event = test_icalendar_event()
            .append_property(Property::new("RRULE", "FREQ=WEEKLY"))
            .append_multi_property(rdate.done())
            .done();

        let recurrence = Recurrence::from_ical_event(&event).unwrap();

        assert_eq!(
            recurrence.rdates,
            vec![EventTime::DateTimeZoned {
                datetime: NaiveDate::from_ymd_opt(2026, 2, 1)
                    .unwrap()
                    .and_hms_opt(10, 0, 0)
                    .unwrap(),
                tzid: "America/Los_Angeles".to_string(),
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

        let recurrence = Recurrence::from_ical_event(&event).unwrap();

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
    fn equality_ignores_exdate_and_rdate_order() {
        let d1 = EventTime::Date(NaiveDate::from_ymd_opt(2026, 1, 5).unwrap());
        let d2 = EventTime::Date(NaiveDate::from_ymd_opt(2026, 1, 12).unwrap());
        let d3 = EventTime::Date(NaiveDate::from_ymd_opt(2026, 2, 1).unwrap());
        let d4 = EventTime::Date(NaiveDate::from_ymd_opt(2026, 2, 15).unwrap());

        let a = Recurrence {
            rrule: "FREQ=WEEKLY".to_string(),
            exdates: vec![d1.clone(), d2.clone()],
            rdates: vec![d3.clone(), d4.clone()],
        };
        let b = Recurrence {
            rrule: "FREQ=WEEKLY".to_string(),
            exdates: vec![d2, d1],
            rdates: vec![d4, d3],
        };

        assert_eq!(a, b);
    }

    #[test]
    fn equality_detects_different_exdates() {
        let a = Recurrence {
            rrule: "FREQ=WEEKLY".to_string(),
            exdates: vec![EventTime::Date(
                NaiveDate::from_ymd_opt(2026, 1, 5).unwrap(),
            )],
            rdates: vec![],
        };
        let b = Recurrence {
            rrule: "FREQ=WEEKLY".to_string(),
            exdates: vec![EventTime::Date(
                NaiveDate::from_ymd_opt(2026, 1, 6).unwrap(),
            )],
            rdates: vec![],
        };

        assert_ne!(a, b);
    }

    #[test]
    fn truncate_before_adds_until_for_utc_dtstart() {
        let rec = Recurrence::new("FREQ=DAILY");
        let dtstart =
            EventTime::DateTimeUtc(chrono::Utc.with_ymd_and_hms(2026, 4, 1, 10, 0, 0).unwrap());
        let before =
            EventTime::DateTimeUtc(chrono::Utc.with_ymd_and_hms(2026, 4, 5, 10, 0, 0).unwrap());

        let truncated = rec.truncate_before(&dtstart, &before);

        assert_eq!(truncated.rrule, "FREQ=DAILY;UNTIL=20260405T095959Z");
    }

    #[test]
    fn truncate_before_adds_until_for_date_dtstart() {
        let rec = Recurrence::new("FREQ=DAILY");
        let dtstart = EventTime::Date(NaiveDate::from_ymd_opt(2026, 4, 1).unwrap());
        let before = EventTime::Date(NaiveDate::from_ymd_opt(2026, 4, 5).unwrap());

        let truncated = rec.truncate_before(&dtstart, &before);

        assert_eq!(truncated.rrule, "FREQ=DAILY;UNTIL=20260404");
    }

    #[test]
    fn truncate_before_replaces_existing_until() {
        let rec = Recurrence::new("FREQ=DAILY;UNTIL=20271231T235959Z");
        let dtstart =
            EventTime::DateTimeUtc(chrono::Utc.with_ymd_and_hms(2026, 4, 1, 10, 0, 0).unwrap());
        let before =
            EventTime::DateTimeUtc(chrono::Utc.with_ymd_and_hms(2026, 4, 5, 10, 0, 0).unwrap());

        let truncated = rec.truncate_before(&dtstart, &before);

        assert_eq!(truncated.rrule, "FREQ=DAILY;UNTIL=20260405T095959Z");
    }

    #[test]
    fn truncate_before_replaces_existing_count() {
        let rec = Recurrence::new("FREQ=DAILY;COUNT=100");
        let dtstart =
            EventTime::DateTimeUtc(chrono::Utc.with_ymd_and_hms(2026, 4, 1, 10, 0, 0).unwrap());
        let before =
            EventTime::DateTimeUtc(chrono::Utc.with_ymd_and_hms(2026, 4, 5, 10, 0, 0).unwrap());

        let truncated = rec.truncate_before(&dtstart, &before);

        assert_eq!(truncated.rrule, "FREQ=DAILY;UNTIL=20260405T095959Z");
        assert!(!truncated.rrule.contains("COUNT="));
    }

    #[test]
    fn truncate_before_drops_exdates_at_or_after_before() {
        let kept =
            EventTime::DateTimeUtc(chrono::Utc.with_ymd_and_hms(2026, 4, 3, 10, 0, 0).unwrap());
        let dropped_at =
            EventTime::DateTimeUtc(chrono::Utc.with_ymd_and_hms(2026, 4, 5, 10, 0, 0).unwrap());
        let dropped_after =
            EventTime::DateTimeUtc(chrono::Utc.with_ymd_and_hms(2026, 4, 6, 10, 0, 0).unwrap());

        let rec = Recurrence {
            rrule: "FREQ=DAILY".to_string(),
            exdates: vec![kept.clone(), dropped_at, dropped_after],
            rdates: vec![],
        };

        let truncated = rec.truncate_before(
            &EventTime::DateTimeUtc(chrono::Utc.with_ymd_and_hms(2026, 4, 1, 10, 0, 0).unwrap()),
            &EventTime::DateTimeUtc(chrono::Utc.with_ymd_and_hms(2026, 4, 5, 10, 0, 0).unwrap()),
        );

        assert_eq!(truncated.exdates, vec![kept]);
    }

    #[test]
    fn truncate_before_keeps_rdates() {
        // RDATEs aren't pruned by truncation — they're explicit additions.
        let rdate =
            EventTime::DateTimeUtc(chrono::Utc.with_ymd_and_hms(2026, 4, 10, 10, 0, 0).unwrap());
        let rec = Recurrence {
            rrule: "FREQ=DAILY".to_string(),
            exdates: vec![],
            rdates: vec![rdate.clone()],
        };

        let truncated = rec.truncate_before(
            &EventTime::DateTimeUtc(chrono::Utc.with_ymd_and_hms(2026, 4, 1, 10, 0, 0).unwrap()),
            &EventTime::DateTimeUtc(chrono::Utc.with_ymd_and_hms(2026, 4, 5, 10, 0, 0).unwrap()),
        );

        assert_eq!(truncated.rdates, vec![rdate]);
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
