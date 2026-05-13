use std::collections::{HashMap, HashSet};

use super::event_change::EventChange;
use crate::event::EventInstanceId;
use crate::{CalendarEvent, RemoteEvent, calendar::SyncedEventIds};

pub struct CalendarDiff {
    outgoing: Vec<EventChange>,
    incoming: Vec<EventChange>,
}

impl CalendarDiff {
    pub(crate) fn compute(
        local_events: Vec<CalendarEvent>,
        remote_events: Vec<RemoteEvent>,
        synced_ids: &SyncedEventIds,
    ) -> Self {
        let local_event_ids: HashSet<_> = local_events
            .iter()
            .map(|e| e.event().event_instance_id())
            .collect();

        let remote_by_id: HashMap<_, &RemoteEvent> = remote_events
            .iter()
            .map(|e| (e.event().event_instance_id(), e))
            .collect();

        let mut outgoing = Vec::new();
        let mut incoming = Vec::new();

        for local_event in &local_events {
            let id = local_event.event().event_instance_id();

            // In both local and remote: skip if equal, otherwise update
            if let Some(remote_event) = remote_by_id.get(&id) {
                if local_event.event() == remote_event.event() {
                    continue;
                }

                if local_is_newer(local_event, remote_event) {
                    outgoing.push(EventChange::Update {
                        from: remote_event.event().clone(),
                        to: local_event.event().clone(),
                    });
                } else {
                    incoming.push(EventChange::Update {
                        from: local_event.event().clone(),
                        to: remote_event.event().clone(),
                    });
                }
                continue;
            }

            if synced_ids.contains(&id) {
                // Local event was in remote, gone now. Delete locally.
                incoming.push(EventChange::Delete(local_event.event().clone()));
            } else {
                // Not in remote, create it!
                outgoing.push(EventChange::Create(local_event.event().clone()));
            }
        }

        for remote_event in &remote_events {
            let id = remote_event.event().event_instance_id();

            // Already in local and remote, skip
            if local_event_ids.contains(&id) {
                continue;
            }

            if synced_ids.contains(&id) {
                // Remote event was in local, gone now. Delete remotely.
                outgoing.push(EventChange::Delete(remote_event.event().clone()));
            } else {
                // Not in local, create it!
                incoming.push(EventChange::Create(remote_event.event().clone()));
            }
        }

        CalendarDiff { outgoing, incoming }
    }

    pub fn incoming(&self) -> &[EventChange] {
        &self.incoming
    }

    pub fn outgoing(&self) -> &[EventChange] {
        &self.outgoing
    }

    pub fn is_empty(&self) -> bool {
        self.outgoing.is_empty() && self.incoming.is_empty()
    }

    /// IDs of events whose presence in the calendar needs to be recorded in
    /// sync state after applying this diff. `Delete`s are intentionally not
    /// removed — synced IDs are an append-only log of "ever seen" events, and
    /// the diff logic only consults them for events still present in either
    /// side, so stale entries are harmless.
    pub(crate) fn new_synced_ids(&self) -> Vec<EventInstanceId> {
        self.incoming
            .iter()
            .chain(self.outgoing.iter())
            .filter_map(|change| match change {
                EventChange::Create(event) | EventChange::Update { to: event, .. } => {
                    Some(event.event_instance_id())
                }
                EventChange::Delete(_) => None,
            })
            .collect()
    }
}

#[cfg(test)]
impl CalendarDiff {
    pub(crate) fn from_changes(outgoing: Vec<EventChange>, incoming: Vec<EventChange>) -> Self {
        Self { outgoing, incoming }
    }
}

fn local_is_newer(local: &CalendarEvent, remote: &RemoteEvent) -> bool {
    match (local.modified_at(), remote.modified_at()) {
        (Some(l), Some(r)) => l >= r,
        (Some(_), None) => true,
        (None, Some(_)) => false,
        (None, None) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{test_calendar, test_calendar_event, test_event};
    use chrono::Utc;
    use pretty_assertions::assert_eq;

    #[test]
    fn new_local_event_becomes_outgoing_create() {
        let (_tmp, calendar_event) = test_calendar_event();
        let new_event = calendar_event.event().clone();

        let local_events = vec![calendar_event];
        let remote_events = vec![];
        let synced_ids = SyncedEventIds::new();

        let diff = CalendarDiff::compute(local_events, remote_events, &synced_ids);

        assert_eq!(diff.outgoing, vec![EventChange::Create(new_event)]);
        assert_eq!(diff.incoming, vec![]);
    }

    #[test]
    fn new_remote_event_becomes_incoming_create() {
        let new_event = test_event();

        let local_events = vec![];
        let remote_events = vec![RemoteEvent::new(new_event.clone())];
        let synced_ids = SyncedEventIds::new();

        let diff = CalendarDiff::compute(local_events, remote_events, &synced_ids);

        assert_eq!(diff.outgoing, vec![]);
        assert_eq!(diff.incoming, vec![EventChange::Create(new_event)]);
    }

    #[test]
    fn deleted_local_event_becomes_outgoing_delete() {
        let remote_event = test_event();

        let mut synced_ids = SyncedEventIds::new();
        synced_ids.insert(remote_event.event_instance_id());

        let local_events = vec![];
        let remote_events = vec![RemoteEvent::new(remote_event.clone())];

        let diff = CalendarDiff::compute(local_events, remote_events, &synced_ids);

        assert_eq!(diff.outgoing, vec![EventChange::Delete(remote_event)]);
        assert_eq!(diff.incoming, vec![]);
    }

    #[test]
    fn deleted_remote_event_becomes_incoming_delete() {
        let (_tmp, calendar_event) = test_calendar_event();
        let local_event = calendar_event.event().clone();

        let mut synced_ids = SyncedEventIds::new();
        synced_ids.insert(local_event.event_instance_id());

        let local_events = vec![calendar_event];
        let remote_events = vec![];

        let diff = CalendarDiff::compute(local_events, remote_events, &synced_ids);

        assert_eq!(diff.outgoing, vec![]);
        assert_eq!(diff.incoming, vec![EventChange::Delete(local_event)]);
    }

    #[test]
    fn updated_local_event_becomes_outgoing_update() {
        let (_tmp, calendar) = test_calendar();
        let remote_event = test_event();

        let mut local_event = remote_event.clone();
        local_event.summary = Some("Updated Test Event".to_string());
        let calendar_event = calendar.create_event(local_event.clone()).unwrap();

        let mut synced_ids = SyncedEventIds::new();
        synced_ids.insert(remote_event.event_instance_id());

        let local_events = vec![calendar_event];
        let remote_events = vec![RemoteEvent::new(remote_event.clone())];

        let diff = CalendarDiff::compute(local_events, remote_events, &synced_ids);

        assert_eq!(
            diff.outgoing,
            vec![EventChange::Update {
                from: remote_event,
                to: local_event,
            }]
        );
        assert_eq!(diff.incoming, vec![]);
    }

    #[test]
    fn updated_remote_event_becomes_incoming_update() {
        let (_tmp, calendar) = test_calendar();
        let local_event = test_event();

        let mut remote_event = local_event.clone();
        remote_event.summary = Some("Updated Test Event".to_string());
        remote_event.last_modified = Some(Utc::now() + chrono::Duration::days(1));

        let calendar_event = calendar.create_event(local_event.clone()).unwrap();

        let mut synced_ids = SyncedEventIds::new();
        synced_ids.insert(local_event.event_instance_id());

        let local_events = vec![calendar_event];
        let remote_events = vec![RemoteEvent::new(remote_event.clone())];

        let diff = CalendarDiff::compute(local_events, remote_events, &synced_ids);

        assert_eq!(diff.outgoing, vec![]);
        assert_eq!(
            diff.incoming,
            vec![EventChange::Update {
                from: local_event,
                to: remote_event,
            }]
        );
    }
}
