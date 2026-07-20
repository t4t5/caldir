use std::collections::{HashMap, HashSet};

use super::event_change::EventChange;
use crate::event::Status;
use crate::{CalendarEvent, DateRange, EventInstanceId, RemoteEvent};

pub struct CalendarDiff {
    outgoing: Vec<EventChange>,
    incoming: Vec<EventChange>,
}

impl CalendarDiff {
    pub(crate) fn compute(
        local_events: Vec<CalendarEvent>,
        remote_events: Vec<RemoteEvent>,
        synced_ids: &HashSet<EventInstanceId>,
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

                // Both cancelled: treat as in-sync regardless of field drift.
                // Cancelled events are historical; we don't churn syncing them.
                if event.status == Status::Cancelled
                    && remote_event.event().status == Status::Cancelled
                {
                    continue;
                }

                // Never push an unspecified (None) visibility — inherit the remote's.
                let mut to_push = event.clone();
                if to_push.visibility.is_none() {
                    to_push.visibility = remote_event.event().visibility;
                }

                if &to_push != remote_event.event() && local_is_newer(local_event, remote_event) {
                    outgoing.push(EventChange::Update {
                        from: remote_event.event().clone(),
                        to: to_push,
                    });
                } else {
                    incoming.push(EventChange::Update {
                        from: event.clone(),
                        to: remote_event.event().clone(),
                    });
                }
                continue;
            }

            // Out-of-window events aren't in the remote response, so we
            // can't tell if they're deleted or just out of range. Skip.
            if let (Some(from), Some(to)) = (range.from, range.to)
                && !event.has_occurrence_in_range(from, to)
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

            // Missing locally + cancelled on remote: treat as already in sync.
            // A missing file is semantically equivalent to STATUS:CANCELLED —
            // both mean "not active". Avoids resurrecting tombstones on pull
            // and avoids spurious push-deletes for events Google has already
            // cancelled.
            if remote_event.event().status == Status::Cancelled {
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

// After a pull, the local file's mtime equals the remote LAST-MODIFIED (see sync_file_mtime)
// We only treat a diff as an outgoing change if mtime > LAST-MODIFIED
// (Never if mtime == LAST-MODIFIED!)
// This helps us augment old events with new potential properties
// that previously might not have been parsed.
fn local_is_newer(local: &CalendarEvent, remote: &RemoteEvent) -> bool {
    match (local.modified_at(), remote.modified_at()) {
        (Some(l), Some(r)) => l > r,
        (Some(_), None) => true,
        (None, Some(_)) => false,
        (None, None) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Event;
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
        let synced_ids = HashSet::new();

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
        let synced_ids = HashSet::new();

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

        let mut synced_ids = HashSet::new();
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
    fn invitee_seeing_cancellation_gets_status_flip_update() {
        // Active local + cancelled remote (same UID) → pull the status flip
        // so the user sees the cancellation in their calendar.
        let (_tmp, calendar) = test_calendar();
        let event = test_event();
        let calendar_event = calendar.create_event(event.clone()).unwrap();

        let mut cancelled = event.clone();
        cancelled.status = Status::Cancelled;
        // Cancellation happens on the provider side, so the remote LAST-MODIFIED
        // is fresher than the local file's mtime — pull direction. Far-future
        // timestamp keeps the test deterministic vs. the just-written tempfile.
        cancelled.last_modified = Some(Utc.with_ymd_and_hms(3000, 1, 1, 0, 0, 0).unwrap());

        let diff = CalendarDiff::compute(
            vec![calendar_event],
            vec![RemoteEvent::new(cancelled.clone())],
            &HashSet::new(),
            &DateRange::default(),
        );

        assert_eq!(diff.outgoing, vec![]);
        assert_eq!(
            diff.incoming,
            vec![EventChange::Update {
                from: event,
                to: cancelled,
            }]
        );
    }

    #[test]
    fn missing_local_cancelled_remote_is_skipped() {
        // No local file + remote cancelled tombstone → already in sync.
        // Prevents spurious "outgoing delete" churn and stops cancelled
        // tombstones from being re-created on pull.
        let mut cancelled = test_event();
        cancelled.status = Status::Cancelled;

        let mut synced_ids = HashSet::new();
        synced_ids.insert(cancelled.event_instance_id());

        let diff = CalendarDiff::compute(
            vec![],
            vec![RemoteEvent::new(cancelled)],
            &synced_ids,
            &DateRange::default(),
        );

        assert_eq!(diff.outgoing, vec![]);
        assert_eq!(diff.incoming, vec![]);
    }

    #[test]
    fn both_cancelled_with_field_drift_is_skipped() {
        // Same UID, both cancelled, but local has stripped data and remote
        // has full data. Don't surface this as churn — cancelled events are
        // historical record, the field-shape drift doesn't matter.
        let (_tmp, calendar) = test_calendar();
        let mut local = test_event();
        local.status = Status::Cancelled;
        let calendar_event = calendar.create_event(local.clone()).unwrap();

        let mut remote = local.clone();
        remote.summary = Some("Now with extra data".into());
        remote.location = Some("Somewhere".into());

        let diff = CalendarDiff::compute(
            vec![calendar_event],
            vec![RemoteEvent::new(remote)],
            &HashSet::new(),
            &DateRange::default(),
        );

        assert_eq!(diff.outgoing, vec![]);
        assert_eq!(diff.incoming, vec![]);
    }

    #[test]
    fn expired_recurring_event_not_flagged_as_delete() {
        // Expired recurring event (UNTIL in the past) has no occurrences in
        // the sync window, but it's not deleted — don't flag it as one.
        let (_tmp, calendar) = test_calendar();

        let mut event = Event::new(
            "Old recurring",
            EventTime::Date(chrono::NaiveDate::from_ymd_opt(1968, 5, 8).unwrap()),
        );
        event.recurrence = Some(crate::event::Recurrence::new(
            "FREQ=YEARLY;UNTIL=20080507T120000Z",
        ));
        let calendar_event = calendar.create_event(event.clone()).unwrap();

        let mut synced_ids = HashSet::new();
        synced_ids.insert(event.event_instance_id());

        let range = DateRange {
            from: Some(Utc.with_ymd_and_hms(2025, 5, 14, 0, 0, 0).unwrap()),
            to: Some(Utc.with_ymd_and_hms(2027, 5, 14, 0, 0, 0).unwrap()),
        };

        let diff = CalendarDiff::compute(vec![calendar_event], vec![], &synced_ids, &range);

        assert_eq!(diff.outgoing, vec![]);
        assert_eq!(diff.incoming, vec![]);
    }

    #[test]
    fn deleted_remote_event_becomes_incoming_delete() {
        let (_tmp, calendar_event) = test_calendar_event();
        let local_event = calendar_event.event().clone();

        let mut synced_ids = HashSet::new();
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

        let mut synced_ids = HashSet::new();
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
        // Differences confined to LAST-MODIFIED and SEQUENCE are sync noise —
        // they shouldn't surface as pending updates.
        let (_tmp, calendar) = test_calendar();
        let local_event = test_event();

        let mut remote_event = local_event.clone();
        remote_event.last_modified = Some(Utc::now() + chrono::Duration::days(1));
        remote_event.sequence += 1;

        let calendar_event = calendar.create_event(local_event.clone()).unwrap();

        let mut synced_ids = HashSet::new();
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
    fn equal_timestamps_with_field_drift_resolve_to_pull() {
        // Regression: when local mtime equals remote LAST-MODIFIED but the
        // events still differ on some field, the difference reflects a
        // schema/parser change (the local file pre-dates our support for
        // some property), not a local edit. Direction must be pull so the
        // remote authoritative value refreshes the field we now understand.
        let (_tmp, calendar) = test_calendar();
        let when = Utc.with_ymd_and_hms(2026, 5, 15, 13, 12, 52).unwrap();

        let mut local_event = test_event();
        local_event.last_modified = Some(when);
        let calendar_event = calendar.create_event(local_event.clone()).unwrap();

        let mut remote_event = local_event.clone();
        remote_event.visibility = Some(crate::event::Visibility::Private);

        let mut synced_ids = HashSet::new();
        synced_ids.insert(local_event.event_instance_id());

        let diff = CalendarDiff::compute(
            vec![calendar_event],
            vec![RemoteEvent::new(remote_event.clone())],
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
    fn unspecified_local_visibility_resolves_to_pull_even_when_local_mtime_is_newer() {
        // Regression: a local file predating CLASS support has visibility: None.
        // Even when its mtime is newer than the remote LAST-MODIFIED (e.g. after
        // a bulk re-serialization bumped every file's mtime), an unspecified
        // visibility must not originate a push that overwrites the remote's real
        // value — it resolves to a pull instead.
        let (_tmp, calendar) = test_calendar();

        let mut local_event = test_event();
        local_event.last_modified = Some(Utc.with_ymd_and_hms(2026, 3, 5, 8, 45, 16).unwrap());
        assert_eq!(local_event.visibility, None);
        let calendar_event = calendar.create_event(local_event.clone()).unwrap();

        let mut remote_event = local_event.clone();
        remote_event.visibility = Some(crate::event::Visibility::Private);
        remote_event.last_modified = Some(Utc.with_ymd_and_hms(2025, 6, 9, 10, 42, 20).unwrap());

        let mut synced_ids = HashSet::new();
        synced_ids.insert(local_event.event_instance_id());

        let diff = CalendarDiff::compute(
            vec![calendar_event],
            vec![RemoteEvent::new(remote_event.clone())],
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
    fn outgoing_update_keeps_remote_visibility_when_local_is_unspecified() {
        // A genuine local edit (summary) still pushes, but it must not drag an
        // unspecified visibility along: the pushed event keeps the remote's
        // visibility rather than dropping it to the default.
        let (_tmp, calendar) = test_calendar();

        let mut local_event = test_event();
        local_event.summary = Some("Edited locally".to_string());
        local_event.last_modified = Some(Utc.with_ymd_and_hms(2026, 3, 5, 8, 45, 16).unwrap());
        let calendar_event = calendar.create_event(local_event.clone()).unwrap();

        let mut remote_event = local_event.clone();
        remote_event.summary = Some("Test Event".to_string());
        remote_event.visibility = Some(crate::event::Visibility::Private);
        remote_event.last_modified = Some(Utc.with_ymd_and_hms(2025, 6, 9, 10, 42, 20).unwrap());

        let mut synced_ids = HashSet::new();
        synced_ids.insert(local_event.event_instance_id());

        let diff = CalendarDiff::compute(
            vec![calendar_event],
            vec![RemoteEvent::new(remote_event)],
            &synced_ids,
            &DateRange::default(),
        );

        assert_eq!(diff.incoming, vec![]);
        match diff.outgoing.as_slice() {
            [EventChange::Update { to, .. }] => {
                assert_eq!(to.summary.as_deref(), Some("Edited locally"));
                assert_eq!(to.visibility, Some(crate::event::Visibility::Private));
            }
            other => panic!("expected one outgoing Update, got {other:?}"),
        }
    }

    #[test]
    fn x_property_change_surfaces_as_diff() {
        let (_tmp, calendar) = test_calendar();
        let mut local_event = test_event();
        local_event
            .x_properties
            .push(crate::event::XProperty::new("X-GOOGLE-COLOR-ID", "5"));

        let mut remote_event = local_event.clone();
        remote_event.x_properties = vec![crate::event::XProperty::new("X-GOOGLE-COLOR-ID", "9")];
        remote_event.last_modified = Some(Utc::now() + chrono::Duration::days(1));

        let calendar_event = calendar.create_event(local_event.clone()).unwrap();

        let mut synced_ids = HashSet::new();
        synced_ids.insert(local_event.event_instance_id());

        let diff = CalendarDiff::compute(
            vec![calendar_event],
            vec![RemoteEvent::new(remote_event.clone())],
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
    fn x_properties_compare_equal_regardless_of_order() {
        // Local files come from BTreeMap-backed ICS parsing (alphabetical),
        // but providers like Google build x_properties in insertion order.
        // Same content in different order must compare equal.
        let mut event_a = test_event();
        event_a.x_properties = vec![
            crate::event::XProperty::new("X-GOOGLE-EVENT-ID", "abc"),
            crate::event::XProperty::new("X-GOOGLE-COLOR-ID", "5"),
        ];

        let mut event_b = event_a.clone();
        event_b.x_properties = vec![
            crate::event::XProperty::new("X-GOOGLE-COLOR-ID", "5"),
            crate::event::XProperty::new("X-GOOGLE-EVENT-ID", "abc"),
        ];

        assert_eq!(event_a, event_b);
    }

    #[test]
    fn updated_remote_event_becomes_incoming_update() {
        let (_tmp, calendar) = test_calendar();
        let local_event = test_event();

        let mut remote_event = local_event.clone();
        remote_event.summary = Some("Updated Test Event".to_string());
        remote_event.last_modified = Some(Utc::now() + chrono::Duration::days(1));

        let calendar_event = calendar.create_event(local_event.clone()).unwrap();

        let mut synced_ids = HashSet::new();
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

        let mut synced_ids = HashSet::new();
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

        let mut synced_ids = HashSet::new();
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
