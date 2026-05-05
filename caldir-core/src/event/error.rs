#[derive(Debug, thiserror::Error)]
pub enum EventError {
    #[error("failed to parse ICS {0}: {1}")]
    IcsParse(String, String),

    #[error("no event found in ICS calendar {0}")]
    NoEventInIcs(icalendar::Calendar),

    #[error("event is missing a start time (DTSTART)")]
    MissingStart,
}
