use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};
use icalendar::{CalendarDateTime, DatePerhapsTime};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventTime {
    Date(NaiveDate),
    DateTimeUtc(DateTime<Utc>),
    DateTimeFloating(NaiveDateTime),
    DateTimeZoned {
        datetime: NaiveDateTime,
        tzid: String,
    },
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
                    tzid,
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
            tzid: "Pacific Standard Time".to_string(),
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
}
