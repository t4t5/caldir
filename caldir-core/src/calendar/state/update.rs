use crate::Event;
use crate::diff::CalendarDiff;
use crate::event::EventInstanceId;

/// The sync-state changes one pull or push produced, accumulated as changes
/// land so a mid-loop failure still records what already made it to disk.
#[derive(Debug, Default)]
pub(crate) struct SyncStateUpdate {
    pub(super) synced_ids: Vec<EventInstanceId>,
    pub(super) bases: Vec<Event>,
    pub(super) removed_bases: Vec<EventInstanceId>,
}

impl SyncStateUpdate {
    /// Seeded with the bases the diff already settled — events that agreed on
    /// both sides, and bases with nothing left to anchor.
    pub(crate) fn from_diff(diff: &CalendarDiff) -> Self {
        Self {
            synced_ids: Vec::new(),
            bases: diff.event_bases().to_vec(),
            removed_bases: diff.removed_event_bases().to_vec(),
        }
    }

    /// Both sides now hold `event`: it is synced, and it is the new base.
    pub(crate) fn record_synced(&mut self, event: &Event) {
        self.synced_ids.push(event.event_instance_id());
        self.bases.push(event.clone());
    }

    pub(crate) fn record_removed(&mut self, id: EventInstanceId) {
        self.removed_bases.push(id);
    }
}
