use crate::{Event, VersionedEvent};

pub struct RemoteEvent(Event);

impl RemoteEvent {
    pub fn new(event: Event) -> Self {
        Self(event)
    }

    pub fn into_versioned(self) -> VersionedEvent {
        let modified_at = self.0.last_modified;

        VersionedEvent {
            event: self.0,
            modified_at,
        }
    }
}
