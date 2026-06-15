mod bases;
mod error;
mod synced_event_ids;

use crate::Event;
use crate::event::EventInstanceId;
use bases::BASES_DIR_NAME;
pub use error::CalendarStateError;
use std::path::Path;
use synced_event_ids::SYNCED_IDS_FILE_NAME;

pub(crate) use bases::EventBases;
pub(crate) use synced_event_ids::SyncedEventIds;

#[derive(Debug)]
pub struct CalendarState {
    synced_event_ids: SyncedEventIds,
    event_bases: EventBases,
}

impl CalendarState {
    pub fn new() -> Self {
        Self {
            synced_event_ids: SyncedEventIds::new(),
            event_bases: EventBases::new(),
        }
    }

    pub fn load(state_dir: &Path) -> Result<Self, CalendarStateError> {
        let synced_ids_path = state_dir.join(SYNCED_IDS_FILE_NAME);
        let synced_event_ids = SyncedEventIds::load(&synced_ids_path)?;
        let event_bases = EventBases::load(&state_dir.join(BASES_DIR_NAME))?;

        Ok(Self {
            synced_event_ids,
            event_bases,
        })
    }

    pub fn write(&self, state_dir: &Path) -> Result<(), CalendarStateError> {
        std::fs::create_dir_all(state_dir)?;
        let synced_ids_path = state_dir.join(SYNCED_IDS_FILE_NAME);
        self.synced_event_ids.write(&synced_ids_path)?;
        self.event_bases.write(&state_dir.join(BASES_DIR_NAME))?;
        Ok(())
    }

    pub(crate) fn synced_event_ids(&self) -> &SyncedEventIds {
        &self.synced_event_ids
    }

    pub(crate) fn event_bases(&self) -> &EventBases {
        &self.event_bases
    }

    pub(crate) fn add_new_synced_ids(
        &mut self,
        ids: impl IntoIterator<Item = EventInstanceId>,
    ) -> &mut Self {
        for id in ids {
            self.synced_event_ids.insert(id);
        }
        self
    }

    pub(crate) fn upsert_event_bases(
        &mut self,
        events: impl IntoIterator<Item = Event>,
    ) -> &mut Self {
        for event in events {
            self.event_bases.upsert(event);
        }
        self
    }

    pub(crate) fn remove_event_bases(
        &mut self,
        ids: impl IntoIterator<Item = EventInstanceId>,
    ) -> &mut Self {
        for id in ids {
            self.event_bases.remove(&id);
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn loads_synced_event_ids_from_state_dir() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join(SYNCED_IDS_FILE_NAME), "abc@hooli.com").unwrap();

        CalendarState::load(dir.path()).unwrap();
    }

    #[test]
    fn load_returns_empty_when_synced_event_ids_file_missing() {
        let dir = tempfile::TempDir::new().unwrap();
        let state = CalendarState::load(dir.path()).unwrap();

        let dst = tempfile::TempDir::new().unwrap();
        state.write(dst.path()).unwrap();

        let written = std::fs::read_to_string(dst.path().join(SYNCED_IDS_FILE_NAME)).unwrap();
        assert!(written.is_empty());
    }

    #[test]
    fn writes_synced_event_ids_to_state_dir() {
        let src = tempfile::TempDir::new().unwrap();
        std::fs::write(src.path().join(SYNCED_IDS_FILE_NAME), "abc@hooli.com").unwrap();
        let state = CalendarState::load(src.path()).unwrap();

        let dst = tempfile::TempDir::new().unwrap();
        state.write(dst.path()).unwrap();

        let written = std::fs::read_to_string(dst.path().join(SYNCED_IDS_FILE_NAME)).unwrap();
        assert_eq!(written, "abc@hooli.com");
    }

    #[test]
    fn write_creates_state_dir_if_missing() {
        let dir = tempfile::TempDir::new().unwrap();
        let state_dir = dir.path().join("does/not/exist");
        let state = CalendarState::load(dir.path()).unwrap();

        state.write(&state_dir).unwrap();

        assert!(state_dir.join(SYNCED_IDS_FILE_NAME).is_file());
    }
}
