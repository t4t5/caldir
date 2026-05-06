use crate::event::Event;
use chrono::{DateTime, Local, TimeZone};
use icalendar::CalendarDateTime;
use icalendar::{Component, DatePerhapsTime};

impl Event {
    /// Generate a slug for an event based on its start time and summary.
    /// The slug is used as the filename for the event's .ics file.
    pub fn base_slug(&self) -> String {
        format!("{}__{}", self.time_slug(), self.summary_slug())
    }

    fn summary(&self) -> Option<&str> {
        self.0.get_summary()
    }

    fn start(&self) -> DatePerhapsTime {
        self.0
            .get_start()
            .expect("Event without DTSTART should have been rejected by from_ical_event")
    }

    /// Always uses local time (it's the most intuitive when browsing files).
    /// If a co-worker on the other side of the world creates an event at 9am their time,
    /// my filename should show what time it is for me, not for them.
    fn time_slug(&self) -> String {
        match self.start() {
            DatePerhapsTime::Date(d) => d.format("%Y-%m-%d").to_string(),
            DatePerhapsTime::DateTime(cal_datetime) => {
                calendar_datetime_to_local(cal_datetime, &Local)
                    .format("%Y-%m-%dT%H%M")
                    .to_string()
            }
        }
    }

    fn summary_slug(&self) -> String {
        slug::slugify(self.summary().unwrap_or("untitled"))
    }
}

fn calendar_datetime_to_local<Tz: TimeZone>(
    cal_datetime: CalendarDateTime,
    tz: &Tz,
) -> DateTime<Tz> {
    match cal_datetime {
        CalendarDateTime::Floating(naive) => tz.from_local_datetime(&naive).unwrap(),
        CalendarDateTime::Utc(utc) => utc.with_timezone(tz),
        CalendarDateTime::WithTimezone {
            date_time,
            tzid: event_tzid,
        } => {
            let event_tz: chrono_tz::Tz = event_tzid.parse().unwrap_or(chrono_tz::UTC);
            event_tz
                .from_local_datetime(&date_time)
                .unwrap()
                .with_timezone(tz)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, Utc};
    use icalendar::EventLike;

    #[test]
    fn generates_correct_base_slug_for_all_day_event() {
        let event = Event::from_ical_event(
            &icalendar::Event::new()
                .summary("Test Event")
                .starts(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap())
                .done(),
        )
        .unwrap();

        assert_eq!(event.base_slug(), "2024-01-01__test-event");
    }

    #[test]
    fn generates_correct_base_slug_for_timed_event() {
        let event = Event::from_ical_event(
            &icalendar::Event::new()
                .summary("Test Event")
                .starts(
                    NaiveDate::from_ymd_opt(2024, 1, 1)
                        .unwrap()
                        .and_hms_opt(15, 30, 20)
                        .unwrap(),
                )
                .done(),
        )
        .unwrap();

        assert_eq!(event.base_slug(), "2024-01-01T1530__test-event");
    }

    #[test]
    fn generates_untitled_base_slug_for_event_without_summary() {
        let event = Event::from_ical_event(
            &icalendar::Event::new()
                .starts(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap())
                .done(),
        )
        .unwrap();

        assert_eq!(event.base_slug(), "2024-01-01__untitled");
    }

    #[test]
    fn converts_utc_event_to_target_timezone() {
        // 2024-01-01 12:00:00 UTC
        let utc = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();

        // User is in Stockholm
        let local =
            calendar_datetime_to_local(CalendarDateTime::Utc(utc), &chrono_tz::Europe::Stockholm);

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
        let local = calendar_datetime_to_local(
            CalendarDateTime::WithTimezone {
                date_time,
                tzid: "America/New_York".into(),
            },
            &chrono_tz::Europe::Stockholm,
        );

        // NY 12:00 (EST, UTC-5) is 18:00 in Stockholm (CET, UTC+1) in January
        assert_eq!(local.format("%Y-%m-%dT%H%M").to_string(), "2024-01-01T1800");
    }

    #[test]
    fn falls_back_to_utc_for_invalid_timezone() {
        // 2024-01-01 12:00:00 with invalid timezone
        let date_time = NaiveDate::from_ymd_opt(2024, 1, 1)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap();

        // User is in Stockholm
        let zoned = calendar_datetime_to_local(
            CalendarDateTime::WithTimezone {
                date_time,
                tzid: "Invalid/Zone".into(),
            },
            &chrono_tz::Europe::Stockholm,
        );

        // Invalid zone falls back to UTC
        // 12:00 UTC becomes 13:00 in Stockholm (CET, UTC+1).
        assert_eq!(zoned.format("%Y-%m-%dT%H%M").to_string(), "2024-01-01T1300");
    }
}
