use super::event_change::EventChange;
use crate::{CalendarEvent, RemoteEvent, VersionedEvent, event::EventInstanceId};
use std::collections::HashSet;

pub struct CalendarDiff {
    outgoing: Vec<EventChange>,
    incoming: Vec<EventChange>,
}

impl CalendarDiff {
    pub fn compute(
        local_events: Vec<CalendarEvent>,
        remote_events: Vec<RemoteEvent>,
        // known_ids: &HashSet<EventInstanceId>,
    ) -> Self {
        let local_events = local_events
            .into_iter()
            .map(CalendarEvent::into_versioned)
            .collect::<Vec<VersionedEvent>>();

        let remote_events = remote_events
            .into_iter()
            .map(RemoteEvent::into_versioned)
            .collect::<Vec<VersionedEvent>>();

        // FIXME:
        CalendarDiff {
            outgoing: vec![],
            incoming: vec![],
        }
    }
}
