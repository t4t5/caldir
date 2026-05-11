mod error;
mod synced_event_ids;

pub use error::CalendarStateError;
use std::path::Path;
use synced_event_ids::SYNCED_IDS_FILE_NAME;

pub(crate) use synced_event_ids::SyncedEventIds;

#[derive(Debug)]
pub struct CalendarState {
    synced_event_ids: SyncedEventIds,
}

impl CalendarState {
    pub fn new() -> Self {
        Self {
            synced_event_ids: SyncedEventIds::new(),
        }
    }

    pub fn load(state_dir: &Path) -> Result<Self, CalendarStateError> {
        let synced_ids_path = state_dir.join(SYNCED_IDS_FILE_NAME);
        let synced_event_ids = SyncedEventIds::load(&synced_ids_path)?;

        Ok(Self { synced_event_ids })
    }

    pub fn write(&self, state_dir: &Path) -> Result<(), CalendarStateError> {
        std::fs::create_dir_all(state_dir)?;
        let synced_ids_path = state_dir.join(SYNCED_IDS_FILE_NAME);
        self.synced_event_ids.write(&synced_ids_path)?;
        Ok(())
    }

    pub fn synced_event_ids(&self) -> &SyncedEventIds {
        &self.synced_event_ids
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
