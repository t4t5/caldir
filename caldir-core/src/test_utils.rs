use std::path::PathBuf;

use chrono::NaiveDate;
use icalendar::{Component, EventLike};

use crate::caldir::TimeFormat;
use crate::diff::{CalendarDiff, EventChange};
use crate::provider::mock_provider::MockProvider;
use crate::{
    Caldir, CaldirConfig, Calendar, CalendarConfig, CalendarEvent, Event, EventTime, Provider,
    ProviderRegistry, ProviderSlug, Remote, RemoteConfig, RemoteConfigParams,
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

    let config = CaldirConfig::new(data_dir, TimeFormat::default(), None, None);

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

pub fn test_remote_config(provider_slug: &str) -> RemoteConfig {
    let params = RemoteConfigParams::new();
    RemoteConfig::new(ProviderSlug::from(provider_slug), params)
}

pub fn test_mock_provider() -> MockProvider {
    MockProvider::new("test-provider")
}

pub fn test_remote_params() -> RemoteConfigParams {
    let mut params = RemoteConfigParams::new();
    params.insert(
        "test_account".to_string(),
        toml::Value::String("user@example.com".to_string()),
    );
    params
}

pub fn test_remote() -> (MockProvider, Remote) {
    let mock = test_mock_provider();
    let remote = Remote::new(mock.provider(), test_remote_params());
    (mock, remote)
}

pub fn outgoing_create_diff(event: Event) -> CalendarDiff {
    CalendarDiff::from_changes(vec![EventChange::Create(event)], vec![])
}

pub fn outgoing_update_diff(from: Event, to: Event) -> CalendarDiff {
    CalendarDiff::from_changes(vec![EventChange::Update { from, to }], vec![])
}

pub fn outgoing_delete_diff(event: Event) -> CalendarDiff {
    CalendarDiff::from_changes(vec![EventChange::Delete(event)], vec![])
}

pub fn incoming_create_diff(event: Event) -> CalendarDiff {
    CalendarDiff::from_changes(vec![], vec![EventChange::Create(event)])
}

pub fn incoming_update_diff(from: Event, to: Event) -> CalendarDiff {
    CalendarDiff::from_changes(vec![], vec![EventChange::Update { from, to }])
}

pub fn incoming_delete_diff(event: Event) -> CalendarDiff {
    CalendarDiff::from_changes(vec![], vec![EventChange::Delete(event)])
}

pub fn test_provider(slug: &str) -> (TempDir, Provider) {
    let (tmp, bin_path) = test_binary(&format!("caldir-provider-{slug}"));
    let provider = Provider::from_binary_path(bin_path).unwrap();
    (tmp, provider)
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
