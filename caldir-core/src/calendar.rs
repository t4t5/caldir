mod config;
mod error;
mod event;
mod state;

use crate::diff::{CalendarDiff, EventChange};
use crate::event::{EventInstanceId, expand_in_range};
use crate::utils::slugify;
use crate::{Event, RemoteConfig};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
pub use config::CalendarConfig;
pub use error::CalendarError;
pub use event::CalendarEvent;
pub(crate) use event::CalendarEventError;
pub use state::CalendarState;
pub(crate) use state::SyncedEventIds;

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
    state: CalendarState,
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

        // Create empty state file:
        let state_dir = calendar_state_dir(path);
        let state = CalendarState::new();
        state.write(&state_dir)?;

        Ok(Self {
            path: path.to_path_buf(),
            config,
            state,
        })
    }

    /// Load existing calendar
    pub fn load(path: &Path) -> Result<Self, CalendarError> {
        if !path.is_dir() {
            return Err(CalendarError::NotFound(path.to_path_buf()));
        }

        let config_path = calendar_config_path(path);
        let config = CalendarConfig::load_optional(&config_path)?;

        let state_dir = calendar_state_dir(path);
        let state = CalendarState::load(&state_dir)?;

        Ok(Self {
            path: path.to_path_buf(),
            config,
            state,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn config_path(&self) -> PathBuf {
        calendar_config_path(self.path())
    }

    pub fn state(&self) -> &CalendarState {
        &self.state
    }

    pub fn slug(&self) -> Option<&str> {
        self.path().file_name().and_then(|s| s.to_str())
    }

    /// Load all events in calendar
    pub fn events(&self) -> Result<Vec<CalendarEvent>, CalendarError> {
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
    pub fn event(&self, event_slug: &str) -> Result<CalendarEvent, CalendarError> {
        let event_path = self.path().join(format!("{}.ics", event_slug));
        let calendar_event = CalendarEvent::load(event_path)?;
        Ok(calendar_event)
    }

    /// List all events occurring within time range
    pub fn events_in_range(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<Event>, CalendarError> {
        let events = self.events()?.into_iter().map(|ce| ce.event().clone());
        Ok(expand_in_range(events, from, to))
    }

    /// Create new event in calendar
    pub fn create_event(&self, event: Event) -> Result<CalendarEvent, CalendarError> {
        let calendar_event = CalendarEvent::create(self, event)?;
        Ok(calendar_event)
    }

    /// Delete event from calendar
    pub fn delete_event(&self, event_slug: &str) -> Result<(), CalendarError> {
        let event = self.event(event_slug)?;
        event.delete()?;
        Ok(())
    }

    pub fn remote_config(&self) -> Option<&RemoteConfig> {
        self.config.as_ref().and_then(|c| c.remote_config())
    }

    pub fn has_remote(&self) -> bool {
        self.remote_config().is_some()
    }

    pub fn remote_email(&self) -> Option<&str> {
        self.remote_config()
            .and_then(|remote_config| remote_config.account_identifier())
            .filter(|id| id.contains('@'))
    }

    pub fn apply_diff(&mut self, diff: &CalendarDiff) -> Result<(), CalendarError> {
        // Create faster lookup for updates:
        let mut events_by_instance_id: HashMap<EventInstanceId, CalendarEvent> = self
            .events()?
            .into_iter()
            .map(|e| (e.event().event_instance_id(), e))
            .collect();

        for change in diff.incoming() {
            match change {
                EventChange::Create(event) => {
                    let cal_event = self.create_event(event.clone())?;
                    events_by_instance_id.insert(cal_event.event().event_instance_id(), cal_event);
                }
                EventChange::Update { to, .. } => {
                    let found = events_by_instance_id.get_mut(&to.event_instance_id());

                    if let Some(cal_event) = found {
                        cal_event.update(to.clone())?;
                    }
                }
                EventChange::Delete(event) => {
                    if let Some(cal_event) =
                        events_by_instance_id.remove(&event.event_instance_id())
                    {
                        cal_event.delete()?;
                    }
                }
            }
        }

        self.state
            .add_new_synced_ids(diff.new_synced_ids())
            .write(&calendar_state_dir(&self.path))?;

        Ok(())
    }

    pub fn base_slug_for(name: Option<&str>) -> String {
        name.map(slugify).unwrap_or_else(|| "calendar".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{
        incoming_create_diff, incoming_delete_diff, incoming_update_diff, test_caldir,
        test_calendar, test_calendar_config, test_calendar_path, test_event,
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

        assert!(matches!(
            result,
            Err(CalendarError::Event(CalendarEventError::NotFound(_)))
        ));
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

    #[test]
    fn apply_diff_creates_file_for_incoming_create() {
        let (_tmp, mut calendar) = test_calendar();
        let event = test_event();

        let diff = incoming_create_diff(event);
        calendar.apply_diff(&diff).unwrap();

        let expected_path = calendar.path().join("2026-01-01T1200__test-event.ics");
        assert!(expected_path.is_file());
    }

    #[test]
    fn apply_diff_updates_file_for_incoming_update() {
        let (_tmp, mut calendar) = test_calendar();
        let from = test_event();
        let cal_event = calendar.create_event(from.clone()).unwrap();
        let old_path = cal_event.path().to_path_buf();

        let mut to = from.clone();
        to.summary = Some("Updated Test Event".to_string());

        let diff = incoming_update_diff(from, to);
        calendar.apply_diff(&diff).unwrap();

        let new_path = calendar
            .path()
            .join("2026-01-01T1200__updated-test-event.ics");
        assert!(new_path.is_file());
        assert!(!old_path.exists());
    }

    #[test]
    fn apply_diff_deletes_file_for_incoming_delete() {
        let (_tmp, mut calendar) = test_calendar();
        let event = test_event();
        let cal_event = calendar.create_event(event.clone()).unwrap();
        let path = cal_event.path().to_path_buf();

        let diff = incoming_delete_diff(event);
        calendar.apply_diff(&diff).unwrap();

        assert!(!path.exists());
    }

    #[test]
    fn apply_diff_records_incoming_create_in_state() {
        let (_tmp, mut calendar) = test_calendar();
        let event = test_event();
        let id = event.event_instance_id();

        calendar.apply_diff(&incoming_create_diff(event)).unwrap();

        assert!(calendar.state().synced_event_ids().contains(&id));
    }

    #[test]
    fn apply_diff_records_outgoing_create_in_state() {
        let (_tmp, mut calendar) = test_calendar();
        let event = test_event();
        let id = event.event_instance_id();

        // Outgoing create: the remote applied it, now record it locally.
        calendar
            .apply_diff(&crate::test_utils::outgoing_create_diff(event))
            .unwrap();

        assert!(calendar.state().synced_event_ids().contains(&id));
    }

    #[test]
    fn apply_diff_persists_state_to_disk() {
        let (_tmp, mut calendar) = test_calendar();
        let event = test_event();
        let id = event.event_instance_id();

        calendar.apply_diff(&incoming_create_diff(event)).unwrap();

        // Reload from disk to confirm state was actually persisted.
        let reloaded = Calendar::load(calendar.path()).unwrap();
        assert!(reloaded.state().synced_event_ids().contains(&id));
    }

    #[test]
    fn apply_diff_does_not_record_deletes_in_state() {
        let (_tmp, mut calendar) = test_calendar();
        let event = test_event();
        let id = event.event_instance_id();
        calendar.create_event(event.clone()).unwrap();

        calendar.apply_diff(&incoming_delete_diff(event)).unwrap();

        // Synced IDs are append-only: the delete doesn't add anything new,
        // and we don't remove existing entries.
        assert!(!calendar.state().synced_event_ids().contains(&id));
    }
}
