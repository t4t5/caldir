use chrono::{DateTime, Local, TimeZone};
use icalendar::{CalendarDateTime, Component, DatePerhapsTime};

mod error;

pub use error::EventError;

pub struct Event(icalendar::Event);

impl Event {
    pub(crate) fn from_ical(inner: icalendar::Event) -> Result<Self, EventError> {
        if inner.get_start().is_none() {
            return Err(EventError::MissingStart);
        }
        Ok(Event(inner))
    }

    fn summary(&self) -> Option<&str> {
        self.0.get_summary()
    }

    fn start(&self) -> DatePerhapsTime {
        self.0
            .get_start()
            .expect("Event without DTSTART should have been rejected by from_ical")
    }

    pub fn time_slug(&self) -> String {
        match self.start() {
            DatePerhapsTime::Date(d) => d.format("%Y-%m-%d").to_string(),
            DatePerhapsTime::DateTime(cdt) => cdt_to_local(cdt).format("%Y-%m-%dT%H%M").to_string(),
        }
    }

    pub fn summary_slug(&self) -> String {
        slug::slugify(self.summary().unwrap_or("untitled"))
    }

    pub fn base_slug(&self) -> String {
        format!("{}__{}", self.time_slug(), self.summary_slug())
    }
}

fn cdt_to_local(cdt: CalendarDateTime) -> DateTime<Local> {
    match cdt {
        CalendarDateTime::Floating(naive) => Local.from_local_datetime(&naive).unwrap(),
        CalendarDateTime::Utc(utc) => utc.with_timezone(&Local),
        CalendarDateTime::WithTimezone { date_time, tzid } => {
            let tz: chrono_tz::Tz = tzid.parse().unwrap_or(chrono_tz::UTC);
            tz.from_local_datetime(&date_time)
                .unwrap()
                .with_timezone(&Local)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use icalendar::EventLike;

    #[test]
    fn generates_correct_base_slug_for_all_day_event() {
        let event = Event::from_ical(
            icalendar::Event::new()
                .summary("Test Event")
                .starts(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap())
                .done(),
        )
        .unwrap();

        assert_eq!(event.base_slug(), "2024-01-01__test-event");
    }

    #[test]
    fn generates_correct_base_slug_for_timed_event() {
        let event = Event::from_ical(
            icalendar::Event::new()
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
        let event = Event::from_ical(
            icalendar::Event::new()
                .starts(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap())
                .done(),
        )
        .unwrap();

        assert_eq!(event.base_slug(), "2024-01-01__untitled");
    }

    #[test]
    fn rejects_event_without_start() {
        let result = Event::from_ical(icalendar::Event::new().done());

        assert!(matches!(result, Err(EventError::MissingStart)));
    }
}
