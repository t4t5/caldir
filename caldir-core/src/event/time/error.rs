#[derive(Debug, thiserror::Error)]
pub enum EventTimeError {
    #[error(
        "event has unparseable timezone {0:?} (expected an IANA zone like \"Europe/Stockholm\")"
    )]
    InvalidTimezone(String),
}
