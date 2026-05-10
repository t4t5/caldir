mod error;

use crate::{Calendar, Remote};
use error::ConnectionError;

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

    async fn diff(&self) -> Result<(), ConnectionError> {
        let local_events = self.local().events();
        let remote_events = self.remote().list_events().await?;

        Ok(())
    }
}
