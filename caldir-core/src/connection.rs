mod error;

use crate::{Calendar, CalendarDiff, Remote};
use error::ConnectionError;

use crate::calendar::KnownEventIds;

/// A connection is a [local calendar] + [remote calendar] pair
pub struct Connection {
    local: Calendar,
    remote: Remote,
}

impl Connection {
    pub fn new(local: Calendar, remote: Remote) -> Self {
        Self { local, remote }
    }

    pub fn local(&self) -> &Calendar {
        &self.local
    }

    pub fn remote(&self) -> &Remote {
        &self.remote
    }

    pub async fn diff(&self) -> Result<CalendarDiff, ConnectionError> {
        let local_events = self.local().events()?;
        let remote_events = self.remote().list_events().await?;

        let known_ids = self.known_event_ids();

        let diff = CalendarDiff::compute(local_events, remote_events, known_ids);

        Ok(diff)
    }

    fn known_event_ids(&self) -> &KnownEventIds {
        self.local().state().known_event_ids()
    }
}
