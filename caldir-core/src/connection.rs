mod error;

use crate::{Calendar, CalendarDiff, DateRange, Remote};
use error::ConnectionError;

use crate::calendar::SyncedEventIds;

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

    pub async fn diff(&self, range: &DateRange) -> Result<CalendarDiff, ConnectionError> {
        let local_events = self.local().events()?;
        let remote_events = self.remote().list_events(range).await?;

        let synced_ids = self.synced_event_ids();

        let diff = CalendarDiff::compute(local_events, remote_events, synced_ids, range);

        Ok(diff)
    }

    fn synced_event_ids(&self) -> &SyncedEventIds {
        self.local().state().synced_event_ids()
    }
}
