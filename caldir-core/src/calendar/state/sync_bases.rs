use crate::{Event, event::EventInstanceId};
use std::collections::HashMap;
use std::path::Path;

use super::error::CalendarStateError;
use super::event_bases::{EVENT_BASES_DIR_NAME, EventBases};
use super::known_event_ids::{KNOWN_IDS_FILE_NAME, KnownEventIds};

// If event base file exists -> <EventInstanceId, Some<Event>>
// If no event base file, but known event ID exists -> <EventInstanceId, None>
#[derive(Debug)]
pub(crate) struct SyncBases(HashMap<EventInstanceId, Option<Box<Event>>>);

impl SyncBases {
    pub(crate) fn new() -> Self {
        Self(HashMap::new())
    }

    pub(crate) fn get(&self, id: &EventInstanceId) -> Option<&Option<Box<Event>>> {
        self.0.get(id)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&EventInstanceId, &Option<Box<Event>>)> {
        self.0.iter()
    }

    pub(crate) fn load_from_state_dir(state_dir: &Path) -> Result<Self, CalendarStateError> {
        let known_event_ids = Self::load_known_event_ids(state_dir)?;
        let event_bases = Self::load_event_bases(state_dir)?;

        let sync_bases = Self::from_event_bases_and_known_ids(event_bases, known_event_ids);

        Ok(sync_bases)
    }

    // Legacy method:
    pub(crate) fn insert_known_event_id(&mut self, id: EventInstanceId) {
        self.0.entry(id).or_insert(None);
    }

    pub(crate) fn insert_event_base(&mut self, id: EventInstanceId, event: Event) {
        self.0.insert(id, Some(Box::new(event)));
    }

    pub(crate) fn save(&self, state_dir: &Path) -> Result<(), CalendarStateError> {
        let known_ids_path = state_dir.join(KNOWN_IDS_FILE_NAME);
        let event_bases_dir = state_dir.join(EVENT_BASES_DIR_NAME);

        // Keep writing the legacy format for clients using an older caldir-core.
        KnownEventIds::write_from(self.0.keys(), &known_ids_path)?;

        // New format with event bases:
        EventBases::write_from(
            self.0.values().filter_map(Option::as_deref),
            &event_bases_dir,
        )?;

        Ok(())
    }

    // Legacy file:
    fn load_known_event_ids(state_dir: &Path) -> Result<KnownEventIds, CalendarStateError> {
        let known_ids_path = state_dir.join(KNOWN_IDS_FILE_NAME);
        KnownEventIds::load(&known_ids_path)
    }

    // New file format with event bases:
    fn load_event_bases(state_dir: &Path) -> Result<EventBases, CalendarStateError> {
        let event_bases_dir = state_dir.join(EVENT_BASES_DIR_NAME);
        let event_bases = EventBases::load(&event_bases_dir)?;
        Ok(event_bases)
    }

    // Use event bases if they exist,
    // Fallback to known event IDs if no event base exists:
    fn from_event_bases_and_known_ids(
        event_bases: EventBases,
        known_event_ids: KnownEventIds,
    ) -> Self {
        let mut sync_bases = HashMap::new();

        for id in known_event_ids.iter() {
            sync_bases.insert(id.clone(), None);
        }

        for (id, event) in event_bases.into_iter() {
            sync_bases.insert(id, Some(Box::new(event)));
        }

        Self(sync_bases)
    }
}
