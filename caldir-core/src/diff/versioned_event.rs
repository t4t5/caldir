use chrono::{DateTime, Utc};

use crate::Event;

pub struct VersionedEvent {
    pub event: Event,
    pub modified_at: Option<DateTime<Utc>>,
}
