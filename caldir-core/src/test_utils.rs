use std::path::PathBuf;

use chrono::NaiveDate;
use icalendar::{Component, EventLike};

use crate::{
    Caldir, CaldirConfig, Calendar, CalendarConfig, CalendarEvent, Event, EventTime,
    ProviderRegistry,
};
use tempfile::TempDir;

pub fn test_caldir() -> (TempDir, Caldir) {
    let (tmp_data_dir, test_caldir_config) = test_caldir_config();

    let caldir = Caldir::new(test_caldir_config, ProviderRegistry::new());

    (tmp_data_dir, caldir)
}

pub fn test_caldir_config() -> (TempDir, CaldirConfig) {
    let tmp = TempDir::new().unwrap();
    let data_dir = tmp.path().join("test-caldir");

    let config = CaldirConfig::new(data_dir);

    (tmp, config)
}

pub fn test_calendar_path() -> (TempDir, PathBuf) {
    let tmp = TempDir::new().unwrap();
    let calendar_path = tmp.path().to_path_buf().join("test-calendar");
    (tmp, calendar_path)
}

pub fn test_calendar() -> (TempDir, Calendar) {
    let (tmp, caldir) = test_caldir();
    let calendar = caldir.create_calendar("test-calendar", None).unwrap();
    (tmp, calendar)
}

pub fn test_calendar_event() -> (TempDir, CalendarEvent) {
    let (tmp, calendar) = test_calendar();
    let event = test_event();

    let calendar_event = calendar.create_event(event.clone()).unwrap();
    (tmp, calendar_event)
}

pub fn test_event_time() -> EventTime {
    EventTime::DateTimeFloating(
        NaiveDate::from_ymd_opt(2026, 1, 1)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap(),
    )
}

pub fn test_event() -> Event {
    Event::new("Test Event", test_event_time())
}

pub fn test_icalendar_event() -> icalendar::Event {
    icalendar::Event::new()
        .uid("test-uid@caldir")
        .starts(icalendar::DatePerhapsTime::Date(
            chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
        ))
        .clone()
}

pub fn test_calendar_config() -> CalendarConfig {
    CalendarConfig::new(
        Some("Test Calendar".to_string()),
        Some("#ff0000".to_string()),
        Some(false),
        None,
    )
}

pub fn test_binary(filename: &str) -> (TempDir, PathBuf) {
    let tmp = TempDir::new().unwrap();

    let path = tmp
        .path()
        .join(format!("{}{}", filename, std::env::consts::EXE_SUFFIX));

    std::fs::write(&path, b"").unwrap();

    // Set executable permissions to executable:
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).unwrap();
    }

    (tmp, path)
}
