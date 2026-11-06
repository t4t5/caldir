#![cfg(unix)]

use caldir_core::{CaldirConfig, Calendar, CalendarEvent};
use std::fs;
use std::process::Command;

#[test]
fn new_with_description_writes_a_parseable_event() {
    let tmp = tempfile::tempdir().unwrap();
    // macOS resolves config under $HOME/.config, Linux under $XDG_CONFIG_HOME.
    let config_dir = tmp.path().join(".config");
    let data_dir = tmp.path().join("calendars");
    fs::create_dir_all(config_dir.join("caldir")).unwrap();

    let mut config = CaldirConfig::default();
    config.set_data_dir(data_dir.clone());
    config
        .write(&config_dir.join("caldir/config.toml"))
        .unwrap();

    let calendar = Calendar::create(&data_dir.join("personal"), None).unwrap();

    let description = "Bring the card, and the form.\nSecond line; with: punctuation";
    let output = Command::new(env!("CARGO_BIN_EXE_caldir"))
        .args([
            "new",
            "Dentist",
            "--start",
            "2026-03-21",
            "--calendar",
            "personal",
            "--no-reminders",
            "--description",
            description,
        ])
        .env("HOME", tmp.path())
        .env("XDG_CONFIG_HOME", &config_dir)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");

    let events = calendar.events().unwrap();
    assert_eq!(events.len(), 1);
    let reloaded = CalendarEvent::load(events[0].path()).unwrap();
    assert_eq!(reloaded.event().description.as_deref(), Some(description));
}
