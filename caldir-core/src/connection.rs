use crate::{Calendar, Remote};

/// A connection is a [local calendar] + [remote calendar] pair
pub struct Connection {
    calendar: Calendar,
    remote: Remote,
}

impl Connection {
    pub fn new(calendar: Calendar, remote: Remote) -> Self {
        Self { calendar, remote }
    }

    pub fn calendar(&self) -> &Calendar {
        &self.calendar
    }

    pub fn remote(&self) -> &Remote {
        &self.remote
    }
}
