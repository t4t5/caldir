use caldir_core::{
    CaldirConfigError, CaldirError, CalendarConfigError, CalendarError, CalendarEventError,
    CalendarStateError, ConnectionError, EventError, ProviderError, ProviderTransportError,
    RemoteError,
};

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
