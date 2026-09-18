#![cfg(target_os = "linux")]

use caldir_core::{CaldirConfig, Calendar, CalendarConfig, RemoteConfig, RemoteConfigParams};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

#[test]
fn handled_calendar_failure_keeps_heading_cause_and_subsequent_calendars() {
    for command in ["status", "sync"] {
        let tmp = tempfile::tempdir().unwrap();
        let config_dir = tmp.path().join("config");
        let data_dir = tmp.path().join("calendars");
        let provider_dir = tmp.path().join("providers");
        fs::create_dir_all(config_dir.join("caldir")).unwrap();
        fs::create_dir(&provider_dir).unwrap();

        let mut config = CaldirConfig::default();
        config.set_data_dir(data_dir.clone());
        config
            .write(&config_dir.join("caldir/config.toml"))
            .unwrap();

        let provider_path = provider_dir.join("caldir-provider-fixture");
        fs::write(
            &provider_path,
            "#!/bin/sh\nread -r request\nprintf '%s\\n' '{\"status\":\"success\",\"data\":[]}'\n",
        )
        .unwrap();
        fs::set_permissions(&provider_path, fs::Permissions::from_mode(0o755)).unwrap();

        for slug in ["work", "personal"] {
            let remote = RemoteConfig::new("fixture".into(), RemoteConfigParams::new());
            let calendar_config = CalendarConfig::new(None, None, None, Some(remote));
            Calendar::create(&data_dir.join(slug), Some(calendar_config)).unwrap();
        }

        // Discovery follows filesystem order; make its first calendar fail.
        let calendars: Vec<_> = fs::read_dir(&data_dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        let bad_event = calendars[0].join("missing-uid.ics");
        fs::write(
            &bad_event,
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nDTSTART:20260101T090000Z\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
        ).unwrap();

        let output = Command::new(env!("CARGO_BIN_EXE_caldir"))
            .args([command, "--from", "2026-01-01", "--to", "2026-01-31"])
            .env("XDG_CONFIG_HOME", &config_dir)
            .env("PATH", &provider_dir)
            .output()
            .unwrap();
        assert!(output.status.success(), "{command}: {output:?}");
        assert!(output.stderr.is_empty(), "{command}: {output:?}");

        let stdout = String::from_utf8(output.stdout).unwrap();
        let first_header = format!("📅 {}", calendars[0].file_name().unwrap().to_str().unwrap());
        let second_header = format!("📅 {}", calendars[1].file_name().unwrap().to_str().unwrap());
        let diagnostic = format!(
            "invalid event in ICS file {}: event is missing a UID",
            bad_event.display()
        );

        assert!(stdout.contains(&diagnostic), "{command}: {stdout}");
        assert_eq!(
            stdout.matches(&diagnostic).count(),
            1,
            "{command}: {stdout}"
        );
        assert!(stdout.find(&first_header).unwrap() < stdout.find(&diagnostic).unwrap());
        assert!(stdout.find(&diagnostic).unwrap() < stdout.find(&second_header).unwrap());
        assert!(stdout.find(&second_header).unwrap() < stdout.find("No changes").unwrap());
    }
}
