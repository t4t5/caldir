mod config;
mod error;
mod event;
mod state;

use crate::event::{EventInstanceId, EventTime, EventUid, Recurrence, expand_in_range};
use crate::utils::slugify;
use crate::{Event, RemoteConfig};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
pub use config::CalendarConfig;
pub use error::CalendarError;
pub use event::CalendarEvent;
pub(crate) use event::CalendarEventError;
pub use state::CalendarState;
pub(crate) use state::SyncBases;

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

        Ok(Self {
            path: path.to_path_buf(),
            config,
            state: CalendarState::new(),
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

    pub fn event_by_instance_id(
        &self,
        id: &EventInstanceId,
    ) -> Result<Option<CalendarEvent>, CalendarError> {
        let found = self
            .events()?
            .into_iter()
            .find(|ce| ce.event().event_instance_id() == *id);

        Ok(found)
    }

    /// Find the master event of a recurring series given its uid.
    /// i.e. a master event is one whose `recurrence` field is set.
    /// Instance overrides (with `recurrence_id` set) are not considered masters.
    pub fn master_event_for(&self, uid: &str) -> Result<Option<Event>, CalendarError> {
        let master = self
            .events()?
            .into_iter()
            .find(|ce| ce.event().uid.as_str() == uid && ce.event().recurrence.is_some())
            .map(|ce| ce.event().clone());
        Ok(master)
    }

    pub fn is_read_only(&self) -> bool {
        self.config
            .as_ref()
            .and_then(|c| c.read_only())
            .unwrap_or(false)
    }

    pub fn name(&self) -> Option<&str> {
        self.config.as_ref().and_then(|c| c.name())
    }

    pub fn color(&self) -> Option<&str> {
        self.config.as_ref().and_then(|c| c.color())
    }

    pub fn read_only_setting(&self) -> Option<bool> {
        self.config.as_ref().and_then(|c| c.read_only())
    }

    /// Split a recurring series at `split_start`:
    ///
    /// Original master:
    /// 1. RRULE is truncated to end strictly before `split_start`
    /// 2. Any EXDATEs at or after `split_start` are dropped
    /// 3. Any override files at or after `split_start` are deleted
    ///
    /// New master:
    /// 1. Starts at `split_start`
    /// 2. Inherit all other metadata (summary, description, location etc) from original
    /// 3. Gets a fresh UID and a reset SEQUENCE
    pub fn split_recurring_series_at(
        &self,
        master_uid: &EventUid,
        split_start: EventTime,
        split_end: EventTime,
        new_recurrence: Option<Recurrence>,
    ) -> Result<Event, CalendarError> {
        let mut all_events = self.events()?;

        let master_idx = all_events
            .iter()
            .position(|ce| ce.event().uid == *master_uid && ce.event().recurrence.is_some())
            .ok_or_else(|| CalendarError::MasterNotFound(master_uid.as_str().to_string()))?;

        let mut master_ce = all_events.swap_remove(master_idx);
        let master_event = master_ce.event().clone();
        let master_recurrence = master_event
            .recurrence
            .as_ref()
            .ok_or_else(|| CalendarError::NotRecurring(master_uid.as_str().to_string()))?;

        let truncated_recurrence =
            master_recurrence.truncate_before(&master_event.start, &split_start);

        let truncated_master = Event {
            recurrence: Some(truncated_recurrence),
            last_modified: Some(Utc::now()),
            sequence: master_event.sequence + 1,
            ..master_event.clone()
        };
        master_ce.update(truncated_master)?;

        let new_master = Event {
            start: split_start.clone(),
            end: Some(split_end),
            recurrence: new_recurrence,
            recurrence_id: None,
            last_modified: Some(Utc::now()),
            sequence: 0,
            ..master_event.with_new_uid()
        };
        let new_master_ce = self.create_event(new_master)?;
        let new_master_event = new_master_ce.event().clone();

        let split_start_utc = split_start.to_utc();
        for ce in all_events.into_iter() {
            if ce.event().uid != *master_uid {
                continue;
            }
            let Some(rid) = ce.event().recurrence_id.as_ref() else {
                continue;
            };
            if rid.as_event_time().to_utc() >= split_start_utc {
                ce.delete()?;
            }
        }

        Ok(new_master_event)
    }

    /// Exclude a single occurrence from a recurring series
    /// i.e. "delete this instance only"
    pub fn delete_recurring_instance(&self, id: &EventInstanceId) -> Result<(), CalendarError> {
        let uid = id.uid();

        let Some(recurrence_id) = id.recurrence_id() else {
            return Err(CalendarError::NotRecurring(uid.as_str().to_string()));
        };

        let mut master = None;
        let mut override_event = None;

        // Find master event (+ potential override event)
        for ce in self.events()? {
            if ce.event().uid != *uid {
                continue;
            }

            let is_override_for_occurrence = ce.event().event_instance_id() == *id;

            if ce.event().recurrence.is_some() {
                master = Some(ce);
            } else if is_override_for_occurrence {
                override_event = Some(ce);
            }
        }

        if master.is_none() && override_event.is_none() {
            return Err(CalendarError::MasterNotFound(uid.as_str().to_string()));
        }

        // If override exists, delete it:
        if let Some(ce) = override_event {
            ce.delete()?;
        }

        // If a master exists, record the exclusion as an EXDATE.
        if let Some(mut ce) = master {
            ce.add_exdate(recurrence_id.as_event_time().clone())?;
        }

        Ok(())
    }

    /// Override a single occurrence of a recurring series
    pub fn update_recurring_instance(
        &self,
        id: &EventInstanceId,
        apply: impl FnOnce(&mut Event),
    ) -> Result<CalendarEvent, CalendarError> {
        let Some(recurrence_id) = id.recurrence_id() else {
            return Err(CalendarError::NotRecurring(id.uid().as_str().to_string()));
        };

        let existing = self.event_by_instance_id(id)?;

        match existing {
            // An override file already exists -> edit it in place
            Some(mut ce) => {
                let mut event = ce.event().clone();
                apply(&mut event);

                event.last_modified = Some(Utc::now());
                event.sequence += 1;
                ce.update(event)?;

                Ok(ce)
            }
            // No override exists yet -> synthesize one from the master
            None => {
                let master = self
                    .master_event_for(id.uid().as_str())?
                    .ok_or_else(|| CalendarError::MasterNotFound(id.uid().as_str().to_string()))?;

                let mut event = master.occurrence_at(recurrence_id.as_event_time().clone());

                event.sequence = 0;
                event.last_modified = Some(Utc::now());
                apply(&mut event);

                self.create_event(event)
            }
        }
    }

    /// List all events occurring within time range
    pub fn expanded_events_in_range(
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

    pub fn config(&self) -> Option<&CalendarConfig> {
        self.config.as_ref()
    }

    pub fn remote_email(&self) -> Option<&str> {
        self.remote_config()
            .and_then(|remote_config| remote_config.account_identifier())
            .filter(|id| id.contains('@'))
    }

    pub(crate) fn record_sync_bases(
        &mut self,
        events: impl IntoIterator<Item = Event>,
    ) -> Result<(), CalendarError> {
        self.state
            .record_sync_bases(events, &calendar_state_dir(&self.path))?;
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

    use crate::event::RecurrenceId;
    use chrono::TimeZone;

    fn make_master(uid: &str, start: DateTime<Utc>, rrule: &str) -> Event {
        let mut event = Event::new("Daily standup", EventTime::DateTimeUtc(start));
        event.uid = crate::event::EventUid::new(uid);
        event.description = Some("Notes".to_string());
        event.location = Some("Office".to_string());
        event.end = Some(EventTime::DateTimeUtc(start + chrono::Duration::hours(1)));
        event.recurrence = Some(Recurrence::new(rrule));
        event
    }

    fn make_override(uid: &str, instance: DateTime<Utc>, summary: &str) -> Event {
        let mut event = Event::new(summary, EventTime::DateTimeUtc(instance));
        event.uid = crate::event::EventUid::new(uid);
        event.recurrence_id = Some(RecurrenceId::from_event_time(EventTime::DateTimeUtc(
            instance,
        )));
        event
    }

    fn loaded_master(cal: &Calendar, uid: &str) -> Event {
        cal.master_event_for(uid)
            .unwrap()
            .unwrap_or_else(|| panic!("master {uid} should exist"))
    }

    fn loaded_overrides(cal: &Calendar, uid: &str) -> Vec<Event> {
        cal.events()
            .unwrap()
            .into_iter()
            .map(|ce| ce.event().clone())
            .filter(|e| e.uid.as_str() == uid && e.recurrence_id.is_some())
            .collect()
    }

    fn t(year: i32, month: u32, day: u32, hour: u32, min: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, day, hour, min, 0)
            .unwrap()
    }

    fn instance_id(uid: &str, occurrence: EventTime) -> EventInstanceId {
        EventInstanceId::new(
            EventUid::new(uid),
            Some(RecurrenceId::from_event_time(occurrence)),
        )
    }

    #[test]
    fn master_event_for_returns_master_when_present() {
        let (_tmp, calendar) = test_calendar();
        calendar
            .create_event(make_master(
                "series@caldir",
                t(2026, 4, 1, 10, 0),
                "FREQ=DAILY",
            ))
            .unwrap();

        let master = calendar.master_event_for("series@caldir").unwrap();

        assert!(master.is_some());
        assert_eq!(master.unwrap().uid.as_str(), "series@caldir");
    }

    #[test]
    fn master_event_for_returns_none_when_missing() {
        let (_tmp, calendar) = test_calendar();

        let master = calendar.master_event_for("nonexistent").unwrap();

        assert!(master.is_none());
    }

    #[test]
    fn master_event_for_ignores_non_recurring_events_with_same_uid() {
        let (_tmp, calendar) = test_calendar();

        let mut event = test_event();
        event.uid = crate::event::EventUid::new("series@caldir");
        calendar.create_event(event).unwrap();

        // Same uid, but no recurrence → not a master.
        let master = calendar.master_event_for("series@caldir").unwrap();

        assert!(master.is_none());
    }

    #[test]
    fn event_by_instance_id_finds_non_recurring_event() {
        let (_tmp, calendar) = test_calendar();
        let cal_event = calendar.create_event(test_event()).unwrap();
        let id = cal_event.event().event_instance_id();

        let found = calendar.event_by_instance_id(&id).unwrap();

        assert!(found.is_some());
        assert_eq!(found.unwrap().path(), cal_event.path());
    }

    #[test]
    fn event_by_instance_id_finds_recurring_instance_override() {
        let (_tmp, calendar) = test_calendar();
        calendar
            .create_event(make_master(
                "series@caldir",
                t(2026, 4, 1, 10, 0),
                "FREQ=DAILY",
            ))
            .unwrap();
        let override_event =
            make_override("series@caldir", t(2026, 4, 3, 10, 0), "Special standup");
        let override_id = override_event.event_instance_id();
        calendar.create_event(override_event).unwrap();

        let found = calendar.event_by_instance_id(&override_id).unwrap();

        assert!(found.is_some());
        assert_eq!(
            found.unwrap().event().summary.as_deref(),
            Some("Special standup")
        );
    }

    #[test]
    fn event_by_instance_id_returns_none_when_missing() {
        let (_tmp, calendar) = test_calendar();
        let id = test_event().event_instance_id();

        let found = calendar.event_by_instance_id(&id).unwrap();

        assert!(found.is_none());
    }

    #[test]
    fn is_read_only_false_when_no_config() {
        let (_tmp, calendar) = test_calendar();

        assert!(!calendar.is_read_only());
    }

    #[test]
    fn is_read_only_false_when_config_has_no_read_only_field() {
        let (_tmp, caldir) = test_caldir();
        let config = CalendarConfig::new(Some("Test".to_string()), None, None, None);
        let calendar = caldir.create_calendar("test", Some(config)).unwrap();

        assert!(!calendar.is_read_only());
    }

    #[test]
    fn is_read_only_true_when_config_says_true() {
        let (_tmp, caldir) = test_caldir();
        let config = CalendarConfig::new(Some("Test".to_string()), None, Some(true), None);
        let calendar = caldir.create_calendar("test", Some(config)).unwrap();

        assert!(calendar.is_read_only());
    }

    #[test]
    fn is_read_only_false_when_config_says_false() {
        let (_tmp, caldir) = test_caldir();
        let config = CalendarConfig::new(Some("Test".to_string()), None, Some(false), None);
        let calendar = caldir.create_calendar("test", Some(config)).unwrap();

        assert!(!calendar.is_read_only());
    }

    #[test]
    fn split_truncates_master_rrule_before_split() {
        let (_tmp, cal) = test_calendar();
        let uid = "master@test";
        cal.create_event(make_master(uid, t(2026, 4, 1, 10, 0), "FREQ=DAILY"))
            .unwrap();

        let split_start = EventTime::DateTimeUtc(t(2026, 4, 5, 10, 0));
        let split_end = EventTime::DateTimeUtc(t(2026, 4, 5, 11, 0));
        cal.split_recurring_series_at(&EventUid::new(uid), split_start, split_end, None)
            .unwrap();

        let master = loaded_master(&cal, uid);
        let rrule = &master.recurrence.as_ref().unwrap().rrule;
        // UNTIL is one second before split_start, in UTC form.
        assert_eq!(rrule, "FREQ=DAILY;UNTIL=20260405T095959Z");
    }

    #[test]
    fn split_creates_new_master_with_fresh_uid_and_inherited_metadata() {
        let (_tmp, cal) = test_calendar();
        let uid = "master@test";
        cal.create_event(make_master(uid, t(2026, 4, 1, 10, 0), "FREQ=DAILY"))
            .unwrap();

        let new_recurrence = Some(Recurrence::new("FREQ=WEEKLY"));
        let new_master = cal
            .split_recurring_series_at(
                &EventUid::new(uid),
                EventTime::DateTimeUtc(t(2026, 4, 5, 10, 0)),
                EventTime::DateTimeUtc(t(2026, 4, 5, 11, 30)),
                new_recurrence,
            )
            .unwrap();

        // Fresh UID, not the master's.
        assert_ne!(new_master.uid.as_str(), uid);
        // Inherits metadata.
        assert_eq!(new_master.summary.as_deref(), Some("Daily standup"));
        assert_eq!(new_master.description.as_deref(), Some("Notes"));
        assert_eq!(new_master.location.as_deref(), Some("Office"));
        // Uses the new start/end and recurrence.
        assert_eq!(
            new_master.start,
            EventTime::DateTimeUtc(t(2026, 4, 5, 10, 0))
        );
        assert_eq!(
            new_master.end,
            Some(EventTime::DateTimeUtc(t(2026, 4, 5, 11, 30)))
        );
        assert_eq!(new_master.recurrence.as_ref().unwrap().rrule, "FREQ=WEEKLY");
        assert!(new_master.recurrence_id.is_none());
    }

    #[test]
    fn split_bumps_master_sequence() {
        let (_tmp, cal) = test_calendar();
        let uid = "master@test";
        let mut master = make_master(uid, t(2026, 4, 1, 10, 0), "FREQ=DAILY");
        master.sequence = 3;
        cal.create_event(master).unwrap();

        cal.split_recurring_series_at(
            &EventUid::new(uid),
            EventTime::DateTimeUtc(t(2026, 4, 5, 10, 0)),
            EventTime::DateTimeUtc(t(2026, 4, 5, 11, 0)),
            None,
        )
        .unwrap();

        assert_eq!(loaded_master(&cal, uid).sequence, 4);
    }

    #[test]
    fn split_drops_overrides_at_or_after_split_keeps_earlier_ones() {
        let (_tmp, cal) = test_calendar();
        let uid = "master@test";
        cal.create_event(make_master(uid, t(2026, 4, 1, 10, 0), "FREQ=DAILY"))
            .unwrap();

        cal.create_event(make_override(uid, t(2026, 4, 3, 10, 0), "before"))
            .unwrap();
        cal.create_event(make_override(uid, t(2026, 4, 5, 10, 0), "at-split"))
            .unwrap();
        cal.create_event(make_override(uid, t(2026, 4, 7, 10, 0), "after"))
            .unwrap();

        cal.split_recurring_series_at(
            &EventUid::new(uid),
            EventTime::DateTimeUtc(t(2026, 4, 5, 10, 0)),
            EventTime::DateTimeUtc(t(2026, 4, 5, 11, 0)),
            None,
        )
        .unwrap();

        let overrides = loaded_overrides(&cal, uid);
        assert_eq!(
            overrides.len(),
            1,
            "only the pre-split override should remain"
        );
        assert_eq!(overrides[0].summary.as_deref(), Some("before"));
    }

    #[test]
    fn split_drops_exdates_at_or_after_split() {
        let (_tmp, cal) = test_calendar();
        let uid = "master@test";
        let mut master = make_master(uid, t(2026, 4, 1, 10, 0), "FREQ=DAILY");
        let kept = EventTime::DateTimeUtc(t(2026, 4, 3, 10, 0));
        let dropped_at = EventTime::DateTimeUtc(t(2026, 4, 5, 10, 0));
        let dropped_after = EventTime::DateTimeUtc(t(2026, 4, 6, 10, 0));
        master.recurrence = Some(Recurrence {
            rrule: "FREQ=DAILY".into(),
            exdates: vec![kept.clone(), dropped_at, dropped_after],
            rdates: vec![],
        });
        cal.create_event(master).unwrap();

        cal.split_recurring_series_at(
            &EventUid::new(uid),
            EventTime::DateTimeUtc(t(2026, 4, 5, 10, 0)),
            EventTime::DateTimeUtc(t(2026, 4, 5, 11, 0)),
            None,
        )
        .unwrap();

        assert_eq!(
            loaded_master(&cal, uid).recurrence.unwrap().exdates,
            vec![kept]
        );
    }

    #[test]
    fn split_with_no_new_recurrence_creates_single_event() {
        let (_tmp, cal) = test_calendar();
        let uid = "master@test";
        cal.create_event(make_master(uid, t(2026, 4, 1, 10, 0), "FREQ=DAILY"))
            .unwrap();

        let new_master = cal
            .split_recurring_series_at(
                &EventUid::new(uid),
                EventTime::DateTimeUtc(t(2026, 4, 5, 10, 0)),
                EventTime::DateTimeUtc(t(2026, 4, 5, 11, 0)),
                None,
            )
            .unwrap();

        assert!(new_master.recurrence.is_none());
    }

    #[test]
    fn split_errors_when_master_not_found() {
        let (_tmp, cal) = test_calendar();

        let err = cal
            .split_recurring_series_at(
                &EventUid::new("nonexistent"),
                EventTime::DateTimeUtc(t(2026, 4, 5, 10, 0)),
                EventTime::DateTimeUtc(t(2026, 4, 5, 11, 0)),
                None,
            )
            .unwrap_err();

        assert!(matches!(err, CalendarError::MasterNotFound(_)));
    }

    #[test]
    fn split_errors_when_event_is_not_recurring() {
        let (_tmp, cal) = test_calendar();
        // Non-recurring event with the target uid. Since master_event_for filters
        // by recurrence.is_some(), this looks like the master "isn't there" —
        // surface as MasterNotFound, which is the right user-facing message.
        let mut single = test_event();
        single.uid = crate::event::EventUid::new("solo@test");
        cal.create_event(single).unwrap();

        let err = cal
            .split_recurring_series_at(
                &EventUid::new("solo@test"),
                EventTime::DateTimeUtc(t(2026, 4, 5, 10, 0)),
                EventTime::DateTimeUtc(t(2026, 4, 5, 11, 0)),
                None,
            )
            .unwrap_err();

        assert!(matches!(err, CalendarError::MasterNotFound(_)));
    }

    #[test]
    fn delete_recurring_instance_adds_exdate_to_master() {
        let (_tmp, cal) = test_calendar();
        let uid = "series@caldir";
        cal.create_event(make_master(uid, t(2026, 4, 1, 10, 0), "FREQ=DAILY"))
            .unwrap();

        let occurrence = EventTime::DateTimeUtc(t(2026, 4, 3, 10, 0));
        cal.delete_recurring_instance(&instance_id(uid, occurrence.clone()))
            .unwrap();

        assert_eq!(
            loaded_master(&cal, uid).recurrence.unwrap().exdates,
            vec![occurrence]
        );
    }

    #[test]
    fn delete_recurring_instance_removes_it_from_expansion() {
        let (_tmp, cal) = test_calendar();
        let uid = "series@caldir";
        cal.create_event(make_master(uid, t(2026, 4, 1, 10, 0), "FREQ=DAILY"))
            .unwrap();

        let occurrence = EventTime::DateTimeUtc(t(2026, 4, 3, 10, 0));
        cal.delete_recurring_instance(&instance_id(uid, occurrence.clone()))
            .unwrap();

        let starts: Vec<_> = cal
            .expanded_events_in_range(t(2026, 4, 1, 0, 0), t(2026, 4, 5, 0, 0))
            .unwrap()
            .into_iter()
            .map(|e| e.start.to_utc())
            .collect();
        assert_eq!(
            starts,
            vec![
                t(2026, 4, 1, 10, 0),
                t(2026, 4, 2, 10, 0),
                t(2026, 4, 4, 10, 0)
            ]
        );
    }

    #[test]
    fn delete_recurring_instance_deletes_materialized_override() {
        let (_tmp, cal) = test_calendar();
        let uid = "series@caldir";
        cal.create_event(make_master(uid, t(2026, 4, 1, 10, 0), "FREQ=DAILY"))
            .unwrap();
        // A previously moved/edited instance for 04-03 sits on disk as an override.
        cal.create_event(make_override(uid, t(2026, 4, 3, 10, 0), "moved standup"))
            .unwrap();

        cal.delete_recurring_instance(&instance_id(
            uid,
            EventTime::DateTimeUtc(t(2026, 4, 3, 10, 0)),
        ))
        .unwrap();

        // Override file is gone, and the base occurrence is excluded from the master.
        assert!(loaded_overrides(&cal, uid).is_empty());
        assert_eq!(
            loaded_master(&cal, uid).recurrence.unwrap().exdates,
            vec![EventTime::DateTimeUtc(t(2026, 4, 3, 10, 0))]
        );
    }

    #[test]
    fn delete_recurring_instance_bumps_sequence() {
        let (_tmp, cal) = test_calendar();
        let uid = "series@caldir";
        let mut master = make_master(uid, t(2026, 4, 1, 10, 0), "FREQ=DAILY");
        master.sequence = 3;
        cal.create_event(master).unwrap();

        cal.delete_recurring_instance(&instance_id(
            uid,
            EventTime::DateTimeUtc(t(2026, 4, 3, 10, 0)),
        ))
        .unwrap();

        assert_eq!(loaded_master(&cal, uid).sequence, 4);
    }

    #[test]
    fn delete_recurring_instance_is_idempotent() {
        let (_tmp, cal) = test_calendar();
        let uid = "series@caldir";
        cal.create_event(make_master(uid, t(2026, 4, 1, 10, 0), "FREQ=DAILY"))
            .unwrap();

        let occurrence = EventTime::DateTimeUtc(t(2026, 4, 3, 10, 0));
        cal.delete_recurring_instance(&instance_id(uid, occurrence.clone()))
            .unwrap();
        cal.delete_recurring_instance(&instance_id(uid, occurrence.clone()))
            .unwrap();

        let master = loaded_master(&cal, uid);
        // No duplicate EXDATE, and the second call didn't bump SEQUENCE again.
        assert_eq!(master.recurrence.unwrap().exdates, vec![occurrence]);
        assert_eq!(master.sequence, 1);
    }

    #[test]
    fn delete_recurring_instance_succeeds_for_orphan_override_without_master() {
        let (_tmp, cal) = test_calendar();
        let uid = "series@caldir";
        // Override with no master in the calendar.
        cal.create_event(make_override(uid, t(2026, 4, 3, 10, 0), "orphan"))
            .unwrap();

        cal.delete_recurring_instance(&instance_id(
            uid,
            EventTime::DateTimeUtc(t(2026, 4, 3, 10, 0)),
        ))
        .unwrap();

        assert!(loaded_overrides(&cal, uid).is_empty());
    }

    #[test]
    fn delete_recurring_instance_errors_when_uid_missing() {
        let (_tmp, cal) = test_calendar();

        let err = cal
            .delete_recurring_instance(&instance_id(
                "nonexistent",
                EventTime::DateTimeUtc(t(2026, 4, 3, 10, 0)),
            ))
            .unwrap_err();

        assert!(matches!(err, CalendarError::MasterNotFound(_)));
    }

    #[test]
    fn update_recurring_instance_creates_override_from_master() {
        let (_tmp, cal) = test_calendar();
        let uid = "series@caldir";
        cal.create_event(make_master(uid, t(2026, 4, 1, 10, 0), "FREQ=DAILY"))
            .unwrap();

        let occurrence = EventTime::DateTimeUtc(t(2026, 4, 3, 10, 0));
        cal.update_recurring_instance(&instance_id(uid, occurrence.clone()), |event| {
            event.summary = Some("Moved standup".to_string());
            event.start = EventTime::DateTimeUtc(t(2026, 4, 3, 14, 0));
            event.end = Some(EventTime::DateTimeUtc(t(2026, 4, 3, 15, 0)));
        })
        .unwrap();

        let overrides = loaded_overrides(&cal, uid);
        assert_eq!(overrides.len(), 1);
        let override_event = &overrides[0];
        // Carries the occurrence's recurrence id, not its own RRULE.
        assert!(override_event.recurrence.is_none());
        assert_eq!(
            override_event
                .recurrence_id
                .as_ref()
                .unwrap()
                .as_event_time(),
            &occurrence
        );
        // Edited fields applied.
        assert_eq!(override_event.summary.as_deref(), Some("Moved standup"));
        assert_eq!(
            override_event.start,
            EventTime::DateTimeUtc(t(2026, 4, 3, 14, 0))
        );
        // Metadata inherited from the master.
        assert_eq!(override_event.location.as_deref(), Some("Office"));
        // Master is untouched.
        assert!(
            loaded_master(&cal, uid)
                .recurrence
                .unwrap()
                .exdates
                .is_empty()
        );
    }

    #[test]
    fn update_recurring_instance_overrides_only_that_occurrence_in_expansion() {
        let (_tmp, cal) = test_calendar();
        let uid = "series@caldir";
        cal.create_event(make_master(uid, t(2026, 4, 1, 10, 0), "FREQ=DAILY"))
            .unwrap();

        cal.update_recurring_instance(
            &instance_id(uid, EventTime::DateTimeUtc(t(2026, 4, 3, 10, 0))),
            |event| event.summary = Some("Special".to_string()),
        )
        .unwrap();

        let summaries: Vec<_> = cal
            .expanded_events_in_range(t(2026, 4, 1, 0, 0), t(2026, 4, 5, 0, 0))
            .unwrap()
            .into_iter()
            .map(|e| e.summary.unwrap_or_default())
            .collect();
        assert_eq!(
            summaries,
            vec!["Daily standup", "Daily standup", "Special", "Daily standup"]
        );
    }

    #[test]
    fn update_recurring_instance_edits_existing_override_in_place() {
        let (_tmp, cal) = test_calendar();
        let uid = "series@caldir";
        cal.create_event(make_master(uid, t(2026, 4, 1, 10, 0), "FREQ=DAILY"))
            .unwrap();
        cal.create_event(make_override(uid, t(2026, 4, 3, 10, 0), "first edit"))
            .unwrap();

        cal.update_recurring_instance(
            &instance_id(uid, EventTime::DateTimeUtc(t(2026, 4, 3, 10, 0))),
            |event| event.summary = Some("second edit".to_string()),
        )
        .unwrap();

        // Still a single override (updated, not duplicated).
        let overrides = loaded_overrides(&cal, uid);
        assert_eq!(overrides.len(), 1);
        assert_eq!(overrides[0].summary.as_deref(), Some("second edit"));
    }

    #[test]
    fn update_recurring_instance_bumps_existing_override_sequence() {
        let (_tmp, cal) = test_calendar();
        let uid = "series@caldir";
        cal.create_event(make_master(uid, t(2026, 4, 1, 10, 0), "FREQ=DAILY"))
            .unwrap();
        let mut override_event = make_override(uid, t(2026, 4, 3, 10, 0), "edit");
        override_event.sequence = 2;
        cal.create_event(override_event).unwrap();

        cal.update_recurring_instance(
            &instance_id(uid, EventTime::DateTimeUtc(t(2026, 4, 3, 10, 0))),
            |event| event.summary = Some("re-edit".to_string()),
        )
        .unwrap();

        assert_eq!(loaded_overrides(&cal, uid)[0].sequence, 3);
    }

    #[test]
    fn update_recurring_instance_errors_when_not_recurring() {
        let (_tmp, cal) = test_calendar();

        let err = cal
            .update_recurring_instance(
                &EventInstanceId::new(EventUid::new("solo@test"), None),
                |_| {},
            )
            .unwrap_err();

        assert!(matches!(err, CalendarError::NotRecurring(_)));
    }

    #[test]
    fn update_recurring_instance_errors_when_master_missing() {
        let (_tmp, cal) = test_calendar();

        let err = cal
            .update_recurring_instance(
                &instance_id("nonexistent", EventTime::DateTimeUtc(t(2026, 4, 3, 10, 0))),
                |_| {},
            )
            .unwrap_err();

        assert!(matches!(err, CalendarError::MasterNotFound(_)));
    }
}
