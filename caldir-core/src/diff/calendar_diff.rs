use std::collections::HashSet;

use super::event_change::EventChange;
use crate::{CalendarEvent, RemoteEvent, calendar::KnownEventIds};

pub struct CalendarDiff {
    outgoing: Vec<EventChange>,
    incoming: Vec<EventChange>,
}

impl CalendarDiff {
    pub fn compute(
        local_events: Vec<CalendarEvent>,
        remote_events: Vec<RemoteEvent>,
        _known_ids: &KnownEventIds,
    ) -> Self {
        let local_event_ids: HashSet<_> = local_events
            .iter()
            .map(|e| e.event().event_instance_id())
            .collect();

        let mut incoming = Vec::new();

        for remote_event in remote_events {
            if !local_event_ids.contains(&remote_event.event().event_instance_id()) {
                incoming.push(EventChange::Create(remote_event.event().clone()));
            }
        }

        CalendarDiff {
            outgoing: vec![],
            incoming,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::test_event;
    use pretty_assertions::assert_eq;

    #[test]
    fn remote_only_event_becomes_incoming_create() {
        let local_events = vec![];
        let remote_events = vec![RemoteEvent::new(test_event())];
        let known_ids = KnownEventIds::new();

        let diff = CalendarDiff::compute(local_events, remote_events, &known_ids);

        assert_eq!(diff.outgoing.len(), 0);
        assert_eq!(diff.incoming.len(), 1);
        assert!(matches!(diff.incoming[0], EventChange::Create(_)));
    }
}
