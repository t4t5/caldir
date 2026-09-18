use caldir_core::{
    CaldirConfig, CaldirConfigError, CaldirError, CalendarConfig, CalendarConfigError,
    CalendarError, CalendarEvent, CalendarEventError, CalendarStateError, ConnectionError,
    EventError, ProviderError, ProviderTransportError, RemoteError,
};
use std::error::Error;
use std::io::{self, ErrorKind};

#[test]
fn consumers_can_match_nested_provider_errors() {
    let error = ConnectionError::Remote(RemoteError::Provider(ProviderError::Transport(
        ProviderTransportError::Spawn(std::io::ErrorKind::NotFound.into()),
    )));
    assert!(matches!(
        error,
        ConnectionError::Remote(RemoteError::Provider(ProviderError::Transport(
            ProviderTransportError::Spawn(cause)
        ))) if cause.kind() == std::io::ErrorKind::NotFound
    ));
}

#[test]
fn consumers_can_match_nested_calendar_errors() {
    let error = CaldirError::Calendar(CalendarError::Event(CalendarEventError::NotFound(
        "meeting.ics".into(),
    )));
    assert!(matches!(
        error,
        CaldirError::Calendar(CalendarError::Event(CalendarEventError::NotFound(path)))
            if path.ends_with("meeting.ics")
    ));
    let error = CalendarError::State(CalendarStateError::InvalidEvent(EventError::MissingUid));
    assert!(matches!(
        error,
        CalendarError::State(CalendarStateError::InvalidEvent(EventError::MissingUid))
    ));
}

#[test]
fn consumers_can_match_configuration_causes() {
    let error = CaldirError::Config(CaldirConfigError::UnknownConfigDirectory);
    assert!(matches!(
        error,
        CaldirError::Config(CaldirConfigError::UnknownConfigDirectory)
    ));
    let error = CalendarError::Config(CalendarConfigError::Io(
        std::io::ErrorKind::PermissionDenied.into(),
    ));
    assert!(matches!(
        error,
        CalendarError::Config(CalendarConfigError::Io(cause))
            if cause.kind() == std::io::ErrorKind::PermissionDenied
    ));
}

#[test]
fn transparent_wrappers_preserve_provider_message() {
    let message = "Missing required field: google_calendar_id";
    let error = ConnectionError::from(RemoteError::from(ProviderError::Provider(message.into())));

    assert_eq!(error.to_string(), message);
    assert!(error.source().is_none());
    assert_eq!(format!("{:#}", anyhow::Error::new(error)), message);
}

#[test]
fn nested_spawn_error_exposes_io_cause_without_duplicate_context() {
    let error = ConnectionError::from(RemoteError::from(ProviderError::from(
        ProviderTransportError::Spawn(io::Error::new(
            ErrorKind::PermissionDenied,
            "permission denied",
        )),
    )));

    assert_eq!(error.to_string(), "failed to spawn provider");
    let source = error.source().unwrap();
    assert_eq!(
        source.downcast_ref::<io::Error>().unwrap().kind(),
        ErrorKind::PermissionDenied
    );
    assert_eq!(source.to_string(), "permission denied");
    assert!(source.source().is_none());
    assert_eq!(
        format!("{:#}", anyhow::Error::new(error)),
        "failed to spawn provider: permission denied"
    );
}

#[test]
fn exchange_error_exposes_io_cause() {
    let error = ProviderTransportError::Io(io::Error::new(ErrorKind::BrokenPipe, "broken pipe"));

    assert_eq!(error.to_string(), "failed to exchange with provider");
    assert_eq!(
        error
            .source()
            .unwrap()
            .downcast_ref::<io::Error>()
            .unwrap()
            .kind(),
        ErrorKind::BrokenPipe
    );
    assert_eq!(
        format!("{:#}", anyhow::Error::new(error)),
        "failed to exchange with provider: broken pipe"
    );
}

#[test]
fn invalid_configs_preserve_path_and_concrete_parse_cause() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("config.toml");
    std::fs::write(&path, "name = [").unwrap();
    let parse_error = toml::from_str::<toml::Value>("name = [").unwrap_err();

    let errors = [
        anyhow::Error::new(CaldirError::from(
            CaldirConfig::load_or_default(&path).unwrap_err(),
        )),
        anyhow::Error::new(CalendarError::from(
            CalendarConfig::load_optional(&path).unwrap_err(),
        )),
    ];

    for error in errors {
        let context = format!("invalid config in TOML file {}", path.display());
        assert_eq!(error.to_string(), context);
        let frames: Vec<_> = error.chain().collect();
        assert_eq!(frames.len(), 2);
        let cause = frames[1].downcast_ref::<toml::de::Error>().unwrap();
        assert_eq!(cause.message(), parse_error.message());
        assert_eq!(format!("{error:#}"), format!("{context}: {cause}"));
    }
}

#[test]
fn invalid_event_preserves_path_and_concrete_event_cause() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("meeting.ics");
    std::fs::write(
        &path,
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nDTSTART:20260101T090000Z\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
    ).unwrap();
    let error = ConnectionError::from(CalendarError::from(CalendarEvent::load(&path).unwrap_err()));
    let context = format!("invalid event in ICS file {}", path.display());

    assert_eq!(error.to_string(), context);
    assert!(matches!(
        error.source().unwrap().downcast_ref::<EventError>(),
        Some(EventError::MissingUid)
    ));
    assert_eq!(
        format!("{:#}", anyhow::Error::new(error)),
        format!("{context}: event is missing a UID")
    );
}

#[test]
fn event_read_error_preserves_path_and_io_cause() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("missing.ics");
    let error = EventError::Io(path.clone(), std::fs::read_to_string(&path).unwrap_err());
    let context = format!("failed to read event from {}", path.display());

    assert_eq!(error.to_string(), context);
    let source = error.source().unwrap().downcast_ref::<io::Error>().unwrap();
    assert_eq!(source.kind(), ErrorKind::NotFound);
    let expected = format!("{context}: {source}");
    assert_eq!(format!("{:#}", anyhow::Error::new(error)), expected);
}

#[test]
fn config_io_failures_identify_operation_and_path() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("config.toml");
    std::fs::write(&path, [0xff]).unwrap();

    for (kind, error) in [
        (
            "caldir",
            anyhow::Error::new(CaldirConfig::load_or_default(&path).unwrap_err()),
        ),
        (
            "calendar",
            anyhow::Error::new(CalendarConfig::load_optional(&path).unwrap_err()),
        ),
    ] {
        assert_eq!(
            error.to_string(),
            format!("failed to read {kind} config {}", path.display())
        );
        assert_eq!(error.chain().count(), 2);
        assert_eq!(
            error
                .root_cause()
                .downcast_ref::<io::Error>()
                .unwrap()
                .kind(),
            ErrorKind::InvalidData
        );
    }

    for (kind, error) in [
        (
            "caldir",
            anyhow::Error::new(CaldirConfig::default().write(tmp.path()).unwrap_err()),
        ),
        (
            "calendar",
            anyhow::Error::new(CalendarConfig::default().write(tmp.path()).unwrap_err()),
        ),
    ] {
        assert_eq!(
            error.to_string(),
            format!("failed to write {kind} config {}", tmp.path().display())
        );
        assert_eq!(error.chain().count(), 2);
        assert!(error.root_cause().is::<io::Error>());
    }
}

#[test]
fn event_file_failures_identify_operation_and_path() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("meeting.ics");
    std::fs::write(&path, [0xff]).unwrap();
    let error = anyhow::Error::new(CalendarEvent::load(&path).unwrap_err());
    assert_eq!(
        error.to_string(),
        format!("failed to read event file {}", path.display())
    );
    assert_eq!(
        error
            .root_cause()
            .downcast_ref::<io::Error>()
            .unwrap()
            .kind(),
        ErrorKind::InvalidData
    );

    std::fs::write(&path, "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:meeting\r\nDTSTART:20260101T090000Z\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n").unwrap();
    let event = CalendarEvent::load(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    let error = anyhow::Error::new(ConnectionError::from(CalendarError::from(
        event.delete().unwrap_err(),
    )));
    let context = format!("failed to delete event file {}", path.display());
    assert_eq!(error.to_string(), context);
    assert_eq!(error.chain().count(), 2);
    let cause = error.root_cause().downcast_ref::<io::Error>().unwrap();
    assert_eq!(cause.kind(), ErrorKind::NotFound);
    assert_eq!(format!("{error:#}"), format!("{context}: {cause}"));
}

#[test]
fn serialization_errors_expose_concrete_causes() {
    let json_cause = <serde_json::Error as serde::ser::Error>::custom("unsupported request");
    let json_error = ProviderError::Serialize(json_cause);
    assert!(json_error.source().unwrap().is::<serde_json::Error>());
    assert_eq!(
        format!("{:#}", anyhow::Error::new(json_error)),
        "failed to serialize provider request: unsupported request"
    );

    let cause = || <toml::ser::Error as serde::ser::Error>::custom("unsupported config");
    let errors = [
        (
            anyhow::Error::new(CalendarConfigError::InvalidConfig(cause())),
            "calendar",
        ),
        (
            anyhow::Error::new(CaldirConfigError::InvalidConfig(cause())),
            "caldir",
        ),
    ];
    for (error, kind) in errors {
        let frames: Vec<_> = error.chain().collect();
        assert_eq!(frames.len(), 2);
        assert!(frames[1].is::<toml::ser::Error>());
        assert_eq!(
            format!("{error:#}"),
            format!("failed to serialize {kind} config: unsupported config")
        );
    }
}
