use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};
use icalendar::{CalendarDateTime, DatePerhapsTime};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EventTime {
    Date(NaiveDate),
    DateTimeUtc(DateTime<Utc>),
    DateTimeFloating(NaiveDateTime),
    DateTimeZoned {
        datetime: NaiveDateTime,
        tzid: String,
    },
}

/// Comparable form of `EventTime`, with resolvable zones converted to UTC.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum NormalizedEventTime {
    Date(NaiveDate),
    Instant(DateTime<Utc>),
    Floating(NaiveDateTime),
}

impl EventTime {
    pub fn to_local_tz<Tz: TimeZone>(&self, tz: &Tz) -> DateTime<Tz> {
        match self {
            EventTime::Date(date) => resolve_local(
                date.and_hms_opt(0, 0, 0)
                    .expect("midnight should be a valid NaiveDateTime"),
                tz,
            ),
            EventTime::DateTimeFloating(datetime) => resolve_local(*datetime, tz),
            EventTime::DateTimeUtc(datetime) => datetime.with_timezone(tz),
            EventTime::DateTimeZoned { datetime, tzid } => match parse_tzid(tzid) {
                Some(event_tz) => resolve_local(*datetime, &event_tz).with_timezone(tz),
                None => resolve_local(*datetime, tz),
            },
        }
    }

    pub fn to_utc(&self) -> DateTime<Utc> {
        match self {
            EventTime::Date(date) => date
                .and_hms_opt(0, 0, 0)
                .expect("midnight should be a valid NaiveDateTime")
                .and_local_timezone(chrono::Local)
                .unwrap()
                .with_timezone(&Utc),
            EventTime::DateTimeFloating(datetime) => datetime
                .and_local_timezone(chrono::Local)
                .unwrap()
                .with_timezone(&Utc),
            EventTime::DateTimeUtc(datetime) => *datetime,
            EventTime::DateTimeZoned { datetime, tzid } => match parse_tzid(tzid) {
                Some(event_tz) => datetime
                    .and_local_timezone(event_tz)
                    .unwrap()
                    .with_timezone(&Utc),
                None => datetime
                    .and_local_timezone(chrono::Local)
                    .unwrap()
                    .with_timezone(&Utc),
            },
        }
    }

    /// Check if this is an all-day date (not a datetime)
    pub fn is_date(&self) -> bool {
        matches!(self, EventTime::Date(_))
    }

    /// See [`NormalizedEventTime`].
    pub(crate) fn normalized(&self) -> NormalizedEventTime {
        match self {
            EventTime::Date(date) => NormalizedEventTime::Date(*date),
            EventTime::DateTimeUtc(datetime) => NormalizedEventTime::Instant(*datetime),
            EventTime::DateTimeFloating(datetime) => NormalizedEventTime::Floating(*datetime),
            EventTime::DateTimeZoned { datetime, tzid } => match parse_tzid(tzid) {
                Some(tz) => {
                    NormalizedEventTime::Instant(resolve_local(*datetime, &tz).with_timezone(&Utc))
                }
                None => NormalizedEventTime::Floating(*datetime),
            },
        }
    }
}

fn parse_tzid(tzid: &str) -> Option<chrono_tz::Tz> {
    tzid.parse().ok()
}

/// Convert a naive (non-zoned) datetime into a zoned datetime.
/// Picks a deterministic answer when DST makes the wall clock ambiguous
/// (e.g. Stockholm 02:30 on 2026-10-25, which happens twice because clocks roll back)
fn resolve_local<Tz: TimeZone>(naive: NaiveDateTime, tz: &Tz) -> DateTime<Tz> {
    if let Some(dt) = tz.from_local_datetime(&naive).earliest() {
        return dt;
    }
    // DST gap (e.g. Stockholm 02:30 on 2026-03-29): skip forward past the jump.
    let bumped = naive + chrono::Duration::hours(1);
    tz.from_local_datetime(&bumped)
        .earliest()
        .unwrap_or_else(|| tz.from_utc_datetime(&naive))
}

impl From<&EventTime> for DatePerhapsTime {
    fn from(value: &EventTime) -> Self {
        match value {
            EventTime::Date(date) => (*date).into(),
            EventTime::DateTimeUtc(datetime) => (*datetime).into(),
            EventTime::DateTimeFloating(datetime) => (*datetime).into(),
            EventTime::DateTimeZoned { datetime, tzid } => CalendarDateTime::WithTimezone {
                date_time: *datetime,
                tzid: tzid.to_string(),
            }
            .into(),
        }
    }
}

impl From<EventTime> for DatePerhapsTime {
    fn from(value: EventTime) -> Self {
        DatePerhapsTime::from(&value)
    }
}

impl From<DatePerhapsTime> for EventTime {
    fn from(value: DatePerhapsTime) -> Self {
        match value {
            DatePerhapsTime::Date(date) => EventTime::Date(date),
            DatePerhapsTime::DateTime(CalendarDateTime::Floating(datetime)) => {
                EventTime::DateTimeFloating(datetime)
            }
            DatePerhapsTime::DateTime(CalendarDateTime::Utc(datetime)) => {
                EventTime::DateTimeUtc(datetime)
            }
            DatePerhapsTime::DateTime(CalendarDateTime::WithTimezone { date_time, tzid }) => {
                EventTime::DateTimeZoned {
                    datetime: date_time,
                    // Single chokepoint for Windows → IANA normalization.
                    tzid: super::windows_tz::normalize(tzid),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_local_converts_utc_event_to_target_timezone() {
        // 2024-01-01 12:00:00 UTC
        let utc = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();

        let event_time = &EventTime::DateTimeUtc(utc);

        // User is in Stockholm:
        let local = event_time.to_local_tz(&chrono_tz::Europe::Stockholm);

        // Local time should be CET (UTC+1) in January = 13:00
        assert_eq!(local.format("%Y-%m-%dT%H%M").to_string(), "2024-01-01T1300");
    }

    #[test]
    fn to_local_converts_zoned_event_to_target_timezone() {
        let date_time = NaiveDate::from_ymd_opt(2024, 1, 1)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap();

        // Even creator is in NYC:
        let event_time = &EventTime::DateTimeZoned {
            datetime: date_time,
            tzid: "America/New_York".to_string(),
        };

        // User is in Stockholm:
        let local = event_time.to_local_tz(&chrono_tz::Europe::Stockholm);

        // 12:00 in NY (EST, UTC-5)
        // = 18:00 in Stockholm (CET, UTC+1) in January
        assert_eq!(local.format("%Y-%m-%dT%H%M").to_string(), "2024-01-01T1800");
    }

    #[test]
    fn to_local_treats_unknown_zoned_event_as_floating() {
        let date_time = NaiveDate::from_ymd_opt(2024, 1, 1)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap();
        let event_time = EventTime::DateTimeZoned {
            datetime: date_time,
            tzid: "Bogus/Zone".to_string(),
        };

        let local = event_time.to_local_tz(&chrono_tz::Europe::Stockholm);

        assert_eq!(local.format("%Y-%m-%dT%H%M").to_string(), "2024-01-01T1200");
    }

    #[test]
    fn to_local_handles_dst_spring_forward_gap() {
        // In Stockholm, 2024-03-31 02:30 doesn't exist — clocks jumped from
        // 02:00 CET to 03:00 CEST. We should still produce a slug.
        let naive = NaiveDate::from_ymd_opt(2024, 3, 31)
            .unwrap()
            .and_hms_opt(2, 30, 0)
            .unwrap();

        let event_time = EventTime::DateTimeFloating(naive);

        let local = event_time.to_local_tz(&chrono_tz::Europe::Stockholm);

        // Fallback skips forward past the jump, landing at 03:30 CEST.
        assert_eq!(local.format("%Y-%m-%dT%H%M").to_string(), "2024-03-31T0330");
    }

    #[test]
    fn to_local_handles_dst_fall_back_overlap() {
        // In Stockholm, 2024-10-27 02:30 happens twice — clocks rewound from
        // 03:00 CEST back to 02:00 CET. We pick the earliest occurrence.
        let naive = NaiveDate::from_ymd_opt(2024, 10, 27)
            .unwrap()
            .and_hms_opt(2, 30, 0)
            .unwrap();

        let event_time = EventTime::DateTimeFloating(naive);

        let local = event_time.to_local_tz(&chrono_tz::Europe::Stockholm);

        assert_eq!(local.format("%Y-%m-%dT%H%M").to_string(), "2024-10-27T0230");
    }

    #[test]
    fn to_utc_passes_through_utc_event() {
        let utc = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();

        let event_time = EventTime::DateTimeUtc(utc);

        assert_eq!(event_time.to_utc(), utc);
    }

    #[test]
    fn to_utc_converts_zoned_event_using_tzid() {
        // 12:00 in NYC (EST, UTC-5) in January = 17:00 UTC
        let datetime = NaiveDate::from_ymd_opt(2024, 1, 1)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap();
        let event_time = EventTime::DateTimeZoned {
            datetime,
            tzid: "America/New_York".to_string(),
        };

        let utc = event_time.to_utc();

        assert_eq!(utc.format("%Y-%m-%dT%H%M").to_string(), "2024-01-01T1700");
    }

    #[test]
    fn to_utc_respects_dst_for_zoned_event() {
        // 12:00 in NYC during DST (EDT, UTC-4) in July = 16:00 UTC
        let datetime = NaiveDate::from_ymd_opt(2024, 7, 1)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap();
        let event_time = EventTime::DateTimeZoned {
            datetime,
            tzid: "America/New_York".to_string(),
        };

        let utc = event_time.to_utc();

        assert_eq!(utc.format("%Y-%m-%dT%H%M").to_string(), "2024-07-01T1600");
    }
}
