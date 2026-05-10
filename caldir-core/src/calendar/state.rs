mod error;
mod known_event_ids;

pub use error::CalendarStateError;
use known_event_ids::KnownEventIds;
use std::path::Path;

pub struct CalendarState {
    known_event_ids: KnownEventIds,
}

const KNOWN_IDS_FILE_NAME: &str = "known_event_ids";

impl CalendarState {
    pub fn load(state_dir: &Path) -> Result<Self, CalendarStateError> {
        let known_ids_path = state_dir.join(KNOWN_IDS_FILE_NAME);
        let known_event_ids = KnownEventIds::load(&known_ids_path)?;

        Ok(Self { known_event_ids })
    }

    pub fn write(&self, state_dir: &Path) -> Result<(), CalendarStateError> {
        std::fs::create_dir_all(state_dir)?;
        let known_ids_path = state_dir.join(KNOWN_IDS_FILE_NAME);
        self.known_event_ids.write(&known_ids_path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn loads_known_event_ids_from_state_dir() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join(KNOWN_IDS_FILE_NAME), "abc@hooli.com").unwrap();

        CalendarState::load(dir.path()).unwrap();
    }

    #[test]
    fn load_returns_empty_when_known_event_ids_file_missing() {
        let dir = tempfile::TempDir::new().unwrap();
        let state = CalendarState::load(dir.path()).unwrap();

        let dst = tempfile::TempDir::new().unwrap();
        state.write(dst.path()).unwrap();

        let written = std::fs::read_to_string(dst.path().join(KNOWN_IDS_FILE_NAME)).unwrap();
        assert!(written.is_empty());
    }

    #[test]
    fn writes_known_event_ids_to_state_dir() {
        let src = tempfile::TempDir::new().unwrap();
        std::fs::write(src.path().join(KNOWN_IDS_FILE_NAME), "abc@hooli.com").unwrap();
        let state = CalendarState::load(src.path()).unwrap();

        let dst = tempfile::TempDir::new().unwrap();
        state.write(dst.path()).unwrap();

        let written = std::fs::read_to_string(dst.path().join(KNOWN_IDS_FILE_NAME)).unwrap();
        assert_eq!(written, "abc@hooli.com");
    }

    #[test]
    fn write_creates_state_dir_if_missing() {
        let dir = tempfile::TempDir::new().unwrap();
        let state_dir = dir.path().join("does/not/exist");
        let state = CalendarState::load(dir.path()).unwrap();

        state.write(&state_dir).unwrap();

        assert!(state_dir.join(KNOWN_IDS_FILE_NAME).is_file());
    }
}
