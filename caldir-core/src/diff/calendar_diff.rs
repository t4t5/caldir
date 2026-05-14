use std::collections::{HashMap, HashSet};

use super::event_change::EventChange;
use crate::{CalendarEvent, DateRange, RemoteEvent, calendar::SyncedEventIds};

pub struct CalendarDiff {
    outgoing: Vec<EventChange>,
    incoming: Vec<EventChange>,
}

impl CalendarDiff {
    pub(crate) fn compute(
        local_events: Vec<CalendarEvent>,
        remote_events: Vec<RemoteEvent>,
        synced_ids: &SyncedEventIds,
        range: &DateRange,
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
            let event = local_event.event();
            let id = event.event_instance_id();

            // In both local and remote: skip if equal, otherwise update
            if let Some(remote_event) = remote_by_id.get(&id) {
                if event == remote_event.event() {
                    continue;
                }

                if local_is_newer(local_event, remote_event) {
                    outgoing.push(EventChange::Update {
                        from: remote_event.event().clone(),
                        to: event.clone(),
                    });
                } else {
                    incoming.push(EventChange::Update {
                        from: event.clone(),
                        to: remote_event.event().clone(),
                    });
                }
                continue;
            }

            // Out-of-window non-recurring events: remote didn't fetch them,
            // so we can't classify them. Leave untouched.
            if event.recurrence.is_none()
                && let (Some(from), Some(to)) = (range.from, range.to)
                && !event.occurs_in_range(from, to)
            {
                continue;
            }

            if synced_ids.contains(&id) {
                incoming.push(EventChange::Delete(event.clone()));
            } else {
                outgoing.push(EventChange::Create(event.clone()));
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

    /// Drop outgoing changes. Used for read-only calendars where outgoing
    /// could never be applied — surfacing them as pending pushes is misleading.
    pub fn discard_outgoing(&mut self) {
        self.outgoing.clear();
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
    use crate::event::EventTime;
    use crate::test_utils::{test_calendar, test_calendar_event, test_event};
    use chrono::{TimeZone, Utc};
    use pretty_assertions::assert_eq;

    #[test]
    fn new_local_event_becomes_outgoing_create() {
        let (_tmp, calendar_event) = test_calendar_event();
        let new_event = calendar_event.event().clone();

        let local_events = vec![calendar_event];
        let remote_events = vec![];
        let synced_ids = SyncedEventIds::new();

        let diff = CalendarDiff::compute(
            local_events,
            remote_events,
            &synced_ids,
            &DateRange::default(),
        );

        assert_eq!(diff.outgoing, vec![EventChange::Create(new_event)]);
        assert_eq!(diff.incoming, vec![]);
    }

    #[test]
    fn new_remote_event_becomes_incoming_create() {
        let new_event = test_event();

        let local_events = vec![];
        let remote_events = vec![RemoteEvent::new(new_event.clone())];
        let synced_ids = SyncedEventIds::new();

        let diff = CalendarDiff::compute(
            local_events,
            remote_events,
            &synced_ids,
            &DateRange::default(),
        );

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

        let diff = CalendarDiff::compute(
            local_events,
            remote_events,
            &synced_ids,
            &DateRange::default(),
        );

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

        let diff = CalendarDiff::compute(
            local_events,
            remote_events,
            &synced_ids,
            &DateRange::default(),
        );

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

        let diff = CalendarDiff::compute(
            local_events,
            remote_events,
            &synced_ids,
            &DateRange::default(),
        );

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
    fn sync_metadata_only_differences_produce_no_diff() {
        // Differences confined to LAST-MODIFIED, SEQUENCE, and X-properties
        // are sync noise — they shouldn't surface as pending updates.
        let (_tmp, calendar) = test_calendar();
        let local_event = test_event();

        let mut remote_event = local_event.clone();
        remote_event.last_modified = Some(Utc::now() + chrono::Duration::days(1));
        remote_event.sequence += 1;
        remote_event
            .x_properties
            .push(crate::event::XProperty::new("X-CUSTOM-FIELD", "value"));

        let calendar_event = calendar.create_event(local_event.clone()).unwrap();

        let mut synced_ids = SyncedEventIds::new();
        synced_ids.insert(local_event.event_instance_id());

        let diff = CalendarDiff::compute(
            vec![calendar_event],
            vec![RemoteEvent::new(remote_event)],
            &synced_ids,
            &DateRange::default(),
        );

        assert_eq!(diff.outgoing, vec![]);
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

        let diff = CalendarDiff::compute(
            local_events,
            remote_events,
            &synced_ids,
            &DateRange::default(),
        );

        assert_eq!(diff.outgoing, vec![]);
        assert_eq!(
            diff.incoming,
            vec![EventChange::Update {
                from: local_event,
                to: remote_event,
            }]
        );
    }

    #[test]
    fn local_event_outside_window_is_not_flagged_for_delete() {
        // Spec: events outside the queried window are left untouched even
        // when their IDs sit in synced_ids and the remote response is empty.
        let (_tmp, calendar) = test_calendar();

        let mut old_event = test_event();
        old_event.start =
            EventTime::DateTimeUtc(Utc.with_ymd_and_hms(2020, 1, 1, 9, 0, 0).unwrap());
        old_event.end = Some(EventTime::DateTimeUtc(
            Utc.with_ymd_and_hms(2020, 1, 1, 10, 0, 0).unwrap(),
        ));
        let calendar_event = calendar.create_event(old_event.clone()).unwrap();

        let mut synced_ids = SyncedEventIds::new();
        synced_ids.insert(old_event.event_instance_id());

        let range = DateRange {
            from: Some(Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()),
            to: Some(Utc.with_ymd_and_hms(2027, 1, 1, 0, 0, 0).unwrap()),
        };

        let diff = CalendarDiff::compute(vec![calendar_event], vec![], &synced_ids, &range);

        assert_eq!(diff.outgoing, vec![]);
        assert_eq!(diff.incoming, vec![]);
    }

    #[test]
    fn local_event_inside_window_is_flagged_for_delete_when_remote_drops_it() {
        let (_tmp, calendar) = test_calendar();

        let mut event = test_event();
        event.start = EventTime::DateTimeUtc(Utc.with_ymd_and_hms(2026, 6, 15, 9, 0, 0).unwrap());
        event.end = Some(EventTime::DateTimeUtc(
            Utc.with_ymd_and_hms(2026, 6, 15, 10, 0, 0).unwrap(),
        ));
        let calendar_event = calendar.create_event(event.clone()).unwrap();

        let mut synced_ids = SyncedEventIds::new();
        synced_ids.insert(event.event_instance_id());

        let range = DateRange {
            from: Some(Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()),
            to: Some(Utc.with_ymd_and_hms(2027, 1, 1, 0, 0, 0).unwrap()),
        };

        let diff = CalendarDiff::compute(vec![calendar_event], vec![], &synced_ids, &range);

        assert_eq!(diff.outgoing, vec![]);
        assert_eq!(diff.incoming, vec![EventChange::Delete(event)]);
    }
}
