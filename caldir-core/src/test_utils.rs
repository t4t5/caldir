use crate::{Caldir, Calendar, CalendarEvent, Event, caldir::config::CaldirConfig};
use chrono::NaiveDate;
use icalendar::{Component, EventLike};

pub fn test_caldir() -> (tempfile::TempDir, Caldir) {
    let tmp = tempfile::TempDir::new().unwrap();
    let caldir = Caldir::new(CaldirConfig {
        calendar_dir: tmp.path().to_path_buf(),
    });
    (tmp, caldir)
}

pub fn test_calendar() -> (tempfile::TempDir, Calendar) {
    let (tmp, caldir) = test_caldir();
    let calendar = Calendar::create(&caldir, "test").unwrap();
    (tmp, calendar)
}

pub fn test_calendar_event() -> (tempfile::TempDir, CalendarEvent) {
    let (tmp, calendar) = test_calendar();
    let event = test_event();

    let calendar_event = calendar.create_event(event.clone()).unwrap();
    (tmp, calendar_event)
}

pub fn test_event() -> Event {
    Event::from_ical_event(
        &icalendar::Event::new()
            .summary("Test Event")
            .starts(
                NaiveDate::from_ymd_opt(2026, 1, 1)
                    .unwrap()
                    .and_hms_opt(12, 0, 0)
                    .unwrap(),
            )
            .done(),
    )
    .unwrap()
}
