use std::collections::HashSet;

use super::event_change::EventChange;
use crate::{CalendarEvent, RemoteEvent, calendar::KnownEventIds};

pub struct CalendarDiff {
    incoming: Vec<EventChange>,
    outgoing: Vec<EventChange>,
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

        let remote_event_ids: HashSet<_> = remote_events
            .iter()
            .map(|e| e.event().event_instance_id())
            .collect();

        let mut incoming = Vec::new();
        let mut outgoing = Vec::new();

        for remote_event in remote_events {
            if !local_event_ids.contains(&remote_event.event().event_instance_id()) {
                incoming.push(EventChange::Create(remote_event.event().clone()));
            }
        }

        for local_event in local_events {
            if !remote_event_ids.contains(&local_event.event().event_instance_id()) {
                outgoing.push(EventChange::Create(local_event.event().clone()));
            }
        }

        CalendarDiff { incoming, outgoing }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{test_calendar_event, test_event};
    use pretty_assertions::assert_eq;

    #[test]
    fn new_remote_event_becomes_incoming_create() {
        let new_event = test_event();

        let local_events = vec![];
        let remote_events = vec![RemoteEvent::new(new_event.clone())];
        let known_ids = KnownEventIds::new();

        let diff = CalendarDiff::compute(local_events, remote_events, &known_ids);

        assert_eq!(diff.outgoing, vec![]);
        assert_eq!(diff.incoming, vec![EventChange::Create(new_event)]);
    }

    #[test]
    fn new_local_event_becomes_outgoing_create() {
        let (_tmp, calendar_event) = test_calendar_event();
        let new_event = calendar_event.event().clone();

        let local_events = vec![calendar_event];
        let remote_events = vec![];
        let known_ids = KnownEventIds::new();

        let diff = CalendarDiff::compute(local_events, remote_events, &known_ids);

        assert_eq!(diff.outgoing, vec![EventChange::Create(new_event)]);
        assert_eq!(diff.incoming, vec![]);
    }
}
