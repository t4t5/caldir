mod bases;
mod error;
mod synced_event_ids;
mod update;

use bases::BASES_DIR_NAME;
pub use error::CalendarStateError;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use synced_event_ids::SYNCED_IDS_FILE_NAME;

pub(crate) use bases::EventBases;
pub(crate) use synced_event_ids::SyncedEventIds;
pub(crate) use update::SyncStateUpdate;

const FORMAT_FILE_NAME: &str = "format";
const SUPPORTED_FORMAT: u32 = 1;

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

    pub fn write(&mut self, state_dir: &Path) -> Result<(), CalendarStateError> {
        std::fs::create_dir_all(state_dir)?;
        write_format_if_missing(&state_dir.join(FORMAT_FILE_NAME))?;
        let synced_ids_path = state_dir.join(SYNCED_IDS_FILE_NAME);
        self.synced_event_ids.write(&synced_ids_path)?;
        self.event_bases.write(&state_dir.join(BASES_DIR_NAME))?;
        Ok(())
    }

    /// Refuse state a newer caldir wrote, and backfill the format of state
    /// written before the guard existed. Sync entry points only — listing and
    /// editing ICS files must keep working whatever the state says.
    pub(crate) fn check_format(state_dir: &Path) -> Result<(), CalendarStateError> {
        std::fs::create_dir_all(state_dir)?;
        let path = state_dir.join(FORMAT_FILE_NAME);

        match std::fs::read_to_string(&path) {
            Ok(contents) => validate_format(path, contents),
            Err(err) if err.kind() == ErrorKind::NotFound => {
                write_format_if_missing(&path)?;
                let contents = std::fs::read_to_string(&path)?;
                validate_format(path, contents)
            }
            Err(err) => Err(err.into()),
        }
    }

    pub(crate) fn synced_event_ids(&self) -> &SyncedEventIds {
        &self.synced_event_ids
    }

    pub(crate) fn event_bases(&self) -> &EventBases {
        &self.event_bases
    }

    pub(crate) fn apply(&mut self, update: SyncStateUpdate) -> &mut Self {
        for id in update.synced_ids {
            self.synced_event_ids.insert(id);
        }
        for event in update.bases {
            self.event_bases.upsert(event);
        }
        for id in update.removed_bases {
            self.event_bases.remove(&id);
        }
        self
    }
}

fn validate_format(path: PathBuf, contents: String) -> Result<(), CalendarStateError> {
    let format = contents
        .trim()
        .parse::<u32>()
        .map_err(|_| CalendarStateError::InvalidFormat {
            path: path.clone(),
            contents: contents.clone(),
        })?;

    if format > SUPPORTED_FORMAT {
        return Err(CalendarStateError::NewerFormat {
            path,
            found: format,
            supported: SUPPORTED_FORMAT,
        });
    }

    Ok(())
}

fn write_format_if_missing(path: &Path) -> Result<(), CalendarStateError> {
    let parent = path.parent().ok_or_else(|| {
        std::io::Error::new(
            ErrorKind::InvalidInput,
            "sync state format path has no parent directory",
        )
    })?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    std::io::Write::write_all(&mut tmp, SUPPORTED_FORMAT.to_string().as_bytes())?;

    match tmp.persist_noclobber(path) {
        Ok(_) => Ok(()),
        Err(err) if err.error.kind() == ErrorKind::AlreadyExists => Ok(()),
        Err(err) => Err(err.error.into()),
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
        let mut state = CalendarState::load(dir.path()).unwrap();

        let dst = tempfile::TempDir::new().unwrap();
        state.write(dst.path()).unwrap();

        let written = std::fs::read_to_string(dst.path().join(SYNCED_IDS_FILE_NAME)).unwrap();
        assert!(written.is_empty());
    }

    #[test]
    fn writes_synced_event_ids_to_state_dir() {
        let src = tempfile::TempDir::new().unwrap();
        std::fs::write(src.path().join(SYNCED_IDS_FILE_NAME), "abc@hooli.com").unwrap();
        let mut state = CalendarState::load(src.path()).unwrap();

        let dst = tempfile::TempDir::new().unwrap();
        state.write(dst.path()).unwrap();

        let written = std::fs::read_to_string(dst.path().join(SYNCED_IDS_FILE_NAME)).unwrap();
        assert_eq!(written, "abc@hooli.com");
    }

    #[test]
    fn write_creates_state_dir_if_missing() {
        let dir = tempfile::TempDir::new().unwrap();
        let state_dir = dir.path().join("does/not/exist");
        let mut state = CalendarState::load(dir.path()).unwrap();

        state.write(&state_dir).unwrap();

        assert!(state_dir.join(SYNCED_IDS_FILE_NAME).is_file());
    }

    #[test]
    fn write_creates_format_one() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut state = CalendarState::new();

        state.write(dir.path()).unwrap();

        assert_eq!(
            std::fs::read_to_string(dir.path().join(FORMAT_FILE_NAME)).unwrap(),
            "1"
        );
    }

    #[test]
    fn check_format_backfills_missing_format() {
        let dir = tempfile::TempDir::new().unwrap();

        CalendarState::check_format(dir.path()).unwrap();

        assert_eq!(
            std::fs::read_to_string(dir.path().join(FORMAT_FILE_NAME)).unwrap(),
            "1"
        );
    }

    #[test]
    fn check_format_refuses_newer_format() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join(FORMAT_FILE_NAME);
        std::fs::write(&path, "2").unwrap();

        let err = CalendarState::check_format(dir.path()).unwrap_err();

        assert!(matches!(
            err,
            CalendarStateError::NewerFormat {
                path: error_path,
                found: 2,
                supported: 1
            } if error_path == path
        ));
    }

    #[test]
    fn check_format_refuses_unparseable_format() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join(FORMAT_FILE_NAME);
        std::fs::write(&path, "garbage").unwrap();

        let err = CalendarState::check_format(dir.path()).unwrap_err();

        assert!(matches!(
            err,
            CalendarStateError::InvalidFormat {
                path: error_path,
                ..
            } if error_path == path
        ));
    }
}
