mod config;
mod error;
mod event;
mod state;

use crate::{Event, RemoteConfig};
pub(crate) use event::CalendarEventError;
use std::path::{Path, PathBuf};

pub use config::CalendarConfig;
pub use error::CalendarError;
pub use event::CalendarEvent;
pub use state::CalendarState;

const DOTDIR_NAME: &str = ".caldir";

// ~/caldir/my_calendar/.caldir/config.toml
const CONFIG_FILE_NAME: &str = "config.toml";

// ~/caldir/my_calendar/.caldir/state/known_event_ids
const STATE_DIR_NAME: &str = "state";

fn calendar_dotdir(calendar_path: &Path) -> PathBuf {
    calendar_path.join(DOTDIR_NAME)
}

fn calendar_config_path(calendar_path: &Path) -> PathBuf {
    calendar_dotdir(calendar_path).join(CONFIG_FILE_NAME)
}

fn calendar_state_dir(calendar_path: &Path) -> PathBuf {
    calendar_dotdir(calendar_path).join(STATE_DIR_NAME)
}

#[derive(Debug)]
pub struct Calendar {
    path: PathBuf,
    config: Option<CalendarConfig>,
}

impl Calendar {
    /// Create new calendar
    pub fn create(path: &Path, config: Option<CalendarConfig>) -> Result<Self, CalendarError> {
        // Error if path already exists:
        if path.exists() {
            return Err(CalendarError::AlreadyExists(path.to_path_buf()));
        }

        // create calendar directory and its .caldir/ subdirectory:
        std::fs::create_dir_all(calendar_dotdir(path))?;

        // Create calendar config file (if config is provided):
        if let Some(ref config) = config {
            let config_path = calendar_config_path(path);
            CalendarConfig::write(config, &config_path)?;
        }

        Ok(Self {
            path: path.to_path_buf(),
            config,
        })
    }

    /// Load existing calendar
    pub fn load(path: &Path) -> Result<Self, CalendarError> {
        if !path.is_dir() {
            return Err(CalendarError::NotFound(path.to_path_buf()));
        }

        let config_path = calendar_config_path(path);
        let config = CalendarConfig::load_optional(&config_path)?;

        Ok(Self {
            path: path.to_path_buf(),
            config,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn config_path(&self) -> PathBuf {
        calendar_config_path(self.path())
    }

    pub fn state(&self) -> Result<CalendarState, CalendarStateError> {
        let state_dir = calendar_state_dir(self.path());
        let state = CalendarState::load(&state_dir)?;
        Ok(state)
    }

    pub fn slug(&self) -> Option<&str> {
        self.path().file_name().and_then(|s| s.to_str())
    }

    /// Load all events in calendar
    pub fn events(&self) -> Result<Vec<CalendarEvent>, CalendarEventError> {
        let mut events: Vec<CalendarEvent> = Vec::new();

        for entry in std::fs::read_dir(self.path())? {
            let entry = entry?;
            let path = entry.path();

            if entry.file_type()?.is_file() && path.extension().is_some_and(|ext| ext == "ics") {
                events.push(CalendarEvent::load(path)?);
            }
        }

        Ok(events)
    }

    /// Load specific event in calendar
    pub fn event(&self, event_slug: &str) -> Result<CalendarEvent, CalendarEventError> {
        let event_path = self.path().join(format!("{}.ics", event_slug));

        if !event_path.is_file() {
            return Err(CalendarEventError::NotFound(event_path));
        }

        CalendarEvent::load(event_path)
    }

    /// Create new event in calendar
    pub fn create_event(&self, event: Event) -> Result<CalendarEvent, CalendarEventError> {
        event::CalendarEvent::create(self, event)
    }

    /// Delete event from calendar
    pub fn delete_event(&self, event_slug: &str) -> Result<(), CalendarEventError> {
        let event = self.event(event_slug)?;
        event.delete()
    }

    pub fn remote_config(&self) -> Option<&RemoteConfig> {
        self.config.as_ref().and_then(|c| c.remote_config())
    }

    pub fn has_remote(&self) -> bool {
        self.remote_config().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{
        test_caldir, test_calendar, test_calendar_config, test_calendar_path, test_event,
    };

    #[test]
    fn create_creates_caldir_subdirectory() {
        let (_, path) = test_calendar_path();

        let calendar = Calendar::create(&path, None).unwrap();

        assert!(calendar.path().join(".caldir").is_dir());
    }

    #[test]
    fn create_without_config_does_not_create_config_file() {
        let (_, path) = test_calendar_path();

        let calendar = Calendar::create(&path, None).unwrap();

        assert!(!calendar.config_path().is_file());
    }

    #[test]
    fn create_with_config_writes_config_file() {
        let (_, path) = test_calendar_path();
        let config = test_calendar_config();

        let calendar = Calendar::create(&path, Some(config.clone())).unwrap();

        let expected_config_path = &path.join(".caldir").join("config.toml");

        // Config file is located in the right place:
        assert!(calendar.config_path().is_file());
        assert_eq!(&calendar.config_path(), expected_config_path);

        // with the right content:
        let loaded_config = CalendarConfig::load(&calendar.config_path()).unwrap();
        assert_eq!(loaded_config, config);
    }

    #[test]
    fn load_returns_existing_calendar() {
        let (_, path) = test_calendar_path();
        let result = Calendar::create(&path, None);

        assert!(result.is_ok());
        assert_eq!(result.unwrap().path(), path);
    }

    #[test]
    fn load_errors_when_directory_missing() {
        let (tmp, _caldir) = test_caldir();

        let result = Calendar::load(&tmp.path().join("missing"));

        assert!(matches!(result, Err(CalendarError::NotFound(_))));
    }

    #[test]
    fn load_errors_when_not_directory() {
        let (tmp, _caldir) = test_caldir();

        let file_path = tmp.path().join("not_a_directory");
        std::fs::write(&file_path, "I am a file, not a directory").unwrap();

        let result = Calendar::load(&file_path);

        assert!(matches!(result, Err(CalendarError::NotFound(p)) if p == file_path));
    }

    #[test]
    fn events_only_returns_events_from_current_calendar() {
        let (_tmp, caldir) = test_caldir();

        // 2 calendars in the same caldir data directory:
        let work = caldir.create_calendar("work", None).unwrap();
        let personal = caldir.create_calendar("personal", None).unwrap();

        work.create_event(test_event()).unwrap();
        work.create_event(test_event()).unwrap();
        personal.create_event(test_event()).unwrap();

        assert_eq!(work.events().unwrap().len(), 2);
        assert_eq!(personal.events().unwrap().len(), 1);
    }

    #[test]
    fn events_ignores_non_ics_files() {
        let (_tmp, calendar) = test_calendar();

        calendar.create_event(test_event()).unwrap();

        // Drop in stray files that other tools (e.g. vdirsyncer) might leave around.
        std::fs::write(calendar.path().join("color"), "#ff0000").unwrap();
        std::fs::write(calendar.path().join("displayname"), "Work").unwrap();
        std::fs::write(calendar.path().join("README.md"), "notes").unwrap();

        let events = calendar.events().unwrap();

        assert_eq!(events.len(), 1);
    }

    #[test]
    fn event_returns_event_by_slug() {
        let (_tmp, calendar) = test_calendar();
        let created = calendar.create_event(test_event()).unwrap();

        let found = calendar.event("2026-01-01T1200__test-event").unwrap();

        assert_eq!(found.path(), created.path());
    }

    #[test]
    fn event_errors_when_file_missing() {
        let (_tmp, calendar) = test_calendar();

        let result = calendar.event("does-not-exist");

        assert!(matches!(result, Err(CalendarEventError::NotFound(_))));
    }

    #[test]
    fn delete_event_removes_file() {
        let (_tmp, calendar) = test_calendar();
        let cal_event = calendar.create_event(test_event()).unwrap();
        let path = cal_event.path().to_path_buf();
        assert!(path.is_file());

        calendar
            .delete_event("2026-01-01T1200__test-event")
            .unwrap();

        assert!(!path.exists());
    }
}
