mod error;
mod event_bases;
mod known_event_ids;
mod sync_bases;

pub use error::CalendarStateError;
use std::path::Path;

#[cfg(test)]
use std::collections::HashSet;

pub(crate) use sync_bases::SyncBases;

use crate::EventInstanceId;

#[derive(Debug)]
pub struct CalendarState {
    sync_bases: SyncBases,
}

impl CalendarState {
    pub(crate) fn new() -> Self {
        Self {
            sync_bases: SyncBases::new(),
        }
    }

    pub(crate) fn load(state_dir: &Path) -> Result<Self, CalendarStateError> {
        let sync_bases = SyncBases::load_from_state_dir(state_dir)?;

        Ok(Self { sync_bases })
    }

    // FIXME (legacy) replace with "add_sync_base"
    pub(crate) fn add_new_synced_ids(
        &mut self,
        new_ids: impl IntoIterator<Item = EventInstanceId>,
    ) {
        for id in new_ids {
            self.sync_bases.insert_known_event_id(id);
        }
    }

    pub(crate) fn save(&self, state_dir: &Path) -> Result<(), CalendarStateError> {
        self.sync_bases.save(state_dir)
    }

    pub(crate) fn sync_bases(&self) -> &SyncBases {
        &self.sync_bases
    }

    #[cfg(test)]
    pub(crate) fn synced_event_ids(&self) -> HashSet<EventInstanceId> {
        self.sync_bases.iter().map(|(id, _)| id.clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::known_event_ids::KNOWN_IDS_FILE_NAME;
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn loads_synced_event_ids_from_state_dir() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join(KNOWN_IDS_FILE_NAME), "abc@hooli.com").unwrap();

        CalendarState::load(dir.path()).unwrap();
    }

    #[test]
    fn load_returns_empty_when_synced_event_ids_file_missing() {
        let dir = tempfile::TempDir::new().unwrap();
        let state = CalendarState::load(dir.path()).unwrap();

        let dst = tempfile::TempDir::new().unwrap();
        state.save(dst.path()).unwrap();

        let written = std::fs::read_to_string(dst.path().join(KNOWN_IDS_FILE_NAME)).unwrap();
        assert!(written.is_empty());
    }

    #[test]
    fn writes_synced_event_ids_to_state_dir() {
        let src = tempfile::TempDir::new().unwrap();
        std::fs::write(src.path().join(KNOWN_IDS_FILE_NAME), "abc@hooli.com").unwrap();
        let state = CalendarState::load(src.path()).unwrap();

        let dst = tempfile::TempDir::new().unwrap();
        state.save(dst.path()).unwrap();

        let written = std::fs::read_to_string(dst.path().join(KNOWN_IDS_FILE_NAME)).unwrap();
        assert_eq!(written, "abc@hooli.com");
    }

    #[test]
    fn write_creates_state_dir_if_missing() {
        let dir = tempfile::TempDir::new().unwrap();
        let state_dir = dir.path().join("does/not/exist");
        let state = CalendarState::load(dir.path()).unwrap();

        state.save(&state_dir).unwrap();

        assert!(state_dir.join(KNOWN_IDS_FILE_NAME).is_file());
    }
}
