use chrono::{DateTime, Utc};

use crate::Event;

pub struct RemoteEvent(Event);

impl RemoteEvent {
    pub fn new(event: Event) -> Self {
        Self(event)
    }

    pub fn modified_at(&self) -> Option<DateTime<Utc>> {
        self.0.last_modified
    }

    pub fn event(&self) -> &Event {
        &self.0
    }
}
