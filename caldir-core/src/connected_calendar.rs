use crate::{Calendar, Remote};

pub struct ConnectedCalendar {
    calendar: Calendar,
    remote: Remote,
}

impl ConnectedCalendar {
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
