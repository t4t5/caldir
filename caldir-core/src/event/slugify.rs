use crate::event::{Event, EventTime};
use chrono::{DateTime, Local, NaiveDateTime, TimeZone};

impl Event {
    /// Generate a slug for an event based on its start time and summary.
    /// The slug is used as the filename for the event's .ics file.
    pub fn base_slug(&self) -> String {
        format!("{}__{}", self.time_slug(), self.summary_slug())
    }

    fn summary_slug(&self) -> String {
        // Strip non-alphanumeric chars (e.g. emoji) before slugifying.
        // Otherwise `slug` transliterates symbols via `deunicode` (☕ → "coffee").
        let cleaned: String = self
            .summary
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect();

        let slug = slug::slugify(cleaned);

        if slug.is_empty() {
            "untitled".to_string()
        } else {
            slug
        }
    }

    /// Always uses local time (it's the most intuitive when browsing files).
    /// If a co-worker on the other side of the world creates an event at 9am their time,
    /// my filename should show what time it is for me, not for them.
    fn time_slug(&self) -> String {
        match &self.start {
            EventTime::Date(date) => date.format("%Y-%m-%d").to_string(),
            EventTime::DateTimeUtc(_)
            | EventTime::DateTimeFloating(_)
            | EventTime::DateTimeZoned { .. } => event_time_to_local(&self.start, &Local)
                .format("%Y-%m-%dT%H%M")
                .to_string(),
        }
    }
}

fn event_time_to_local<Tz: TimeZone>(event_time: &EventTime, tz: &Tz) -> DateTime<Tz> {
    match event_time {
        EventTime::Date(date) => resolve_local(
            date.and_hms_opt(0, 0, 0)
                .expect("midnight should be a valid NaiveDateTime"),
            tz,
        ),
        EventTime::DateTimeFloating(datetime) => resolve_local(*datetime, tz),
        EventTime::DateTimeUtc(datetime) => datetime.with_timezone(tz),
        EventTime::DateTimeZoned { datetime, tzid } => {
            resolve_local(*datetime, tzid).with_timezone(tz)
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, Utc};

    #[test]
    fn generates_expected_slug_for_emoji_summary() {
        let event = Event::new(
            "Café ☕️ meeting",
            EventTime::Date(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
        );

        assert_eq!(event.summary_slug(), "cafe-meeting");
    }

    #[test]
    fn generates_expected_slug_for_empty_summary() {
        let event = Event::new(
            "",
            EventTime::Date(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
        );

        assert_eq!(event.summary_slug(), "untitled");
    }

    #[test]
    fn generates_correct_base_slug_for_all_day_event() {
        let event = Event::new(
            "Test Event",
            EventTime::Date(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
        );

        assert_eq!(event.base_slug(), "2024-01-01__test-event");
    }

    #[test]
    fn generates_correct_base_slug_for_timed_event() {
        let event = Event::new(
            "Test Event",
            EventTime::DateTimeFloating(
                NaiveDate::from_ymd_opt(2024, 1, 1)
                    .unwrap()
                    .and_hms_opt(15, 30, 20)
                    .unwrap(),
            ),
        );

        assert_eq!(event.base_slug(), "2024-01-01T1530__test-event");
    }

    #[test]
    fn generates_untitled_base_slug_for_event_without_summary() {
        let event = Event::new(
            "",
            EventTime::Date(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
        );

        assert_eq!(event.base_slug(), "2024-01-01__untitled");
    }

    #[test]
    fn converts_utc_event_to_target_timezone() {
        // 2024-01-01 12:00:00 UTC
        let utc = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();

        // User is in Stockholm
        let local =
            event_time_to_local(&EventTime::DateTimeUtc(utc), &chrono_tz::Europe::Stockholm);

        // Local time should be CET (UTC+1) in January = 13:00
        assert_eq!(local.format("%Y-%m-%dT%H%M").to_string(), "2024-01-01T1300");
    }

    #[test]
    fn converts_zoned_event_to_target_timezone() {
        // 2024-01-01 12:00:00 in New York time (EST, UTC-5)
        let date_time = NaiveDate::from_ymd_opt(2024, 1, 1)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap();

        // User is in Stockholm
        let local = event_time_to_local(
            &EventTime::DateTimeZoned {
                datetime: date_time,
                tzid: chrono_tz::America::New_York,
            },
            &chrono_tz::Europe::Stockholm,
        );

        // NY 12:00 (EST, UTC-5) is 18:00 in Stockholm (CET, UTC+1) in January
        assert_eq!(local.format("%Y-%m-%dT%H%M").to_string(), "2024-01-01T1800");
    }

    #[test]
    fn handles_dst_spring_forward_gap() {
        // In Stockholm, 2024-03-31 02:30 doesn't exist — clocks jumped from
        // 02:00 CET to 03:00 CEST. We should still produce a slug.
        let naive = NaiveDate::from_ymd_opt(2024, 3, 31)
            .unwrap()
            .and_hms_opt(2, 30, 0)
            .unwrap();

        let local = event_time_to_local(
            &EventTime::DateTimeFloating(naive),
            &chrono_tz::Europe::Stockholm,
        );

        // Fallback skips forward past the jump, landing at 03:30 CEST.
        assert_eq!(local.format("%Y-%m-%dT%H%M").to_string(), "2024-03-31T0330");
    }

    #[test]
    fn handles_dst_fall_back_overlap() {
        // In Stockholm, 2024-10-27 02:30 happens twice — clocks rewound from
        // 03:00 CEST back to 02:00 CET. We pick the earliest occurrence.
        let naive = NaiveDate::from_ymd_opt(2024, 10, 27)
            .unwrap()
            .and_hms_opt(2, 30, 0)
            .unwrap();

        let local = event_time_to_local(
            &EventTime::DateTimeFloating(naive),
            &chrono_tz::Europe::Stockholm,
        );

        assert_eq!(local.format("%Y-%m-%dT%H%M").to_string(), "2024-10-27T0230");
    }
}
