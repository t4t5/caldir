use crate::EventTime;
use chrono::{DateTime, Utc};

// The instance identifier in a recurring event
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RecurrenceId(EventTime);

impl RecurrenceId {
    pub fn as_event_time(&self) -> &EventTime {
        &self.0
    }

    pub fn from_event_time(event_time: EventTime) -> Self {
        RecurrenceId(event_time)
    }

    pub fn to_instant(&self) -> DateTime<Utc> {
        self.0.to_utc()
    }
}
