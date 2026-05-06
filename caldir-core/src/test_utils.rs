use crate::{Caldir, Calendar, Event, caldir::config::CaldirConfig};
use chrono::NaiveDate;
use icalendar::{Component, EventLike};

pub fn test_caldir() -> (tempfile::TempDir, Caldir) {
    let tmp = tempfile::TempDir::new().unwrap();
    let caldir = Caldir::new(CaldirConfig {
        calendar_dir: tmp.path().to_path_buf(),
    });
    (tmp, caldir)
}

pub fn test_calendar() -> (tempfile::TempDir, Caldir, Calendar) {
    let (tmp, caldir) = test_caldir();
    let calendar = Calendar::new(&caldir, "work").unwrap();
    calendar.save().unwrap();
    (tmp, caldir, calendar)
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
