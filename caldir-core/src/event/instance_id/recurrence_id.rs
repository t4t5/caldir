use super::super::time::NormalizedEventTime;
use crate::EventTime;
use chrono::{NaiveDate, NaiveDateTime};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;

// The instance identifier in a recurring event
#[derive(Debug, Clone, Eq)]
pub struct RecurrenceId(EventTime);

impl RecurrenceId {
    pub fn as_event_time(&self) -> &EventTime {
        &self.0
    }

    pub fn from_event_time(event_time: EventTime) -> Self {
        RecurrenceId(event_time)
    }

    fn normalized(&self) -> NormalizedEventTime {
        self.0.normalized()
    }
}

// Instances that fall on the same start time are treated as same,
// even if their raw data has different time zones or formats:
impl PartialEq for RecurrenceId {
    fn eq(&self, other: &Self) -> bool {
        self.normalized() == other.normalized()
    }
}

impl Hash for RecurrenceId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.normalized().hash(state);
    }
}

// RFC 5545 RECURRENCE-ID value:
//   YYYYMMDD                       — all-day
//   YYYYMMDDTHHMMSSZ               — UTC
//   YYYYMMDDTHHMMSS                — floating
//   TZID={tzid}:YYYYMMDDTHHMMSS    — zoned
const TZID_PREFIX: &str = "TZID=";

impl fmt::Display for RecurrenceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            EventTime::Date(date) => write!(f, "{}", date.format("%Y%m%d")),
            EventTime::DateTimeUtc(datetime) => write!(f, "{}", datetime.format("%Y%m%dT%H%M%SZ")),
            EventTime::DateTimeFloating(datetime) => {
                write!(f, "{}", datetime.format("%Y%m%dT%H%M%S"))
            }
            EventTime::DateTimeZoned { datetime, tzid } => {
                write!(
                    f,
                    "{TZID_PREFIX}{tzid}:{}",
                    datetime.format("%Y%m%dT%H%M%S")
                )
            }
        }
    }
}

impl FromStr for RecurrenceId {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_event_time(s).map(RecurrenceId).ok_or(())
    }
}

fn parse_event_time(s: &str) -> Option<EventTime> {
    if let Some(rest) = s.strip_prefix(TZID_PREFIX) {
        let (tzid, dt_str) = rest.split_once(':')?;
        let datetime = NaiveDateTime::parse_from_str(dt_str, "%Y%m%dT%H%M%S").ok()?;

        return Some(EventTime::DateTimeZoned {
            datetime,
            tzid: tzid.to_string(),
        });
    }

    if s.ends_with('Z') {
        let datetime = NaiveDateTime::parse_from_str(s, "%Y%m%dT%H%M%SZ").ok()?;
        return Some(EventTime::DateTimeUtc(datetime.and_utc()));
    }

    if !s.contains('T') {
        let date = NaiveDate::parse_from_str(s, "%Y%m%d").ok()?;
        return Some(EventTime::Date(date));
    }

    let datetime = NaiveDateTime::parse_from_str(s, "%Y%m%dT%H%M%S").ok()?;
    Some(EventTime::DateTimeFloating(datetime))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn zoned(tzid: &str, hour: u32) -> RecurrenceId {
        // 2026-07-14
        RecurrenceId::from_event_time(EventTime::DateTimeZoned {
            datetime: NaiveDate::from_ymd_opt(2026, 7, 14)
                .unwrap()
                .and_hms_opt(hour, 0, 0)
                .unwrap(),
            tzid: tzid.to_string(),
        })
    }

    #[test]
    fn same_instant_in_different_zones_is_equal() {
        assert_eq!(zoned("Europe/Stockholm", 19), zoned("Europe/London", 18));
    }

    #[test]
    fn round_trips_every_rfc5545_form() {
        for (rid, expected) in [
            (
                RecurrenceId::from_event_time(EventTime::Date(
                    NaiveDate::from_ymd_opt(2026, 1, 9).unwrap(),
                )),
                "20260109",
            ),
            (
                RecurrenceId::from_event_time(EventTime::DateTimeUtc(
                    NaiveDate::from_ymd_opt(2026, 1, 1)
                        .unwrap()
                        .and_hms_opt(17, 0, 0)
                        .unwrap()
                        .and_utc(),
                )),
                "20260101T170000Z",
            ),
            (
                RecurrenceId::from_event_time(EventTime::DateTimeFloating(
                    NaiveDate::from_ymd_opt(2026, 1, 9)
                        .unwrap()
                        .and_hms_opt(10, 0, 0)
                        .unwrap(),
                )),
                "20260109T100000",
            ),
            (
                zoned("Europe/Stockholm", 10),
                "TZID=Europe/Stockholm:20260714T100000",
            ),
        ] {
            assert_eq!(rid.to_string(), expected);
            let parsed: RecurrenceId = expected.parse().unwrap();
            assert_eq!(parsed.as_event_time(), rid.as_event_time());
        }
    }

    #[test]
    fn rejects_non_rfc5545_values() {
        for value in [
            "",
            "not-a-date",
            "2026-01-09",
            "TZID=Europe/Stockholm",
            "20260109T10",
        ] {
            assert!(value.parse::<RecurrenceId>().is_err(), "accepted {value}");
        }
    }
}
