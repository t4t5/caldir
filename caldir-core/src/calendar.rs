mod config;
mod error;
mod event;
mod state;

use crate::event::{EventInstanceId, EventTime, Recurrence, expand_in_range};
use crate::utils::slugify;
use crate::{Event, RemoteConfig};
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

    /// Split a recurring series at `split_start`.
    ///
    /// The original master's RRULE is truncated to end strictly before
    /// `split_start`, any EXDATEs at or after `split_start` are dropped, and
    /// any override files at or after `split_start` are deleted (they're
    /// either being replaced by the new series or are now orphaned).
    ///
    /// A new master is created starting at `split_start` (with `split_end`
    /// and `new_recurrence`), inheriting all other metadata (summary,
    /// description, location, reminders, attendees, etc.) from the original
    /// master. The new master gets a fresh UID and a reset SEQUENCE.
    ///
    /// Returns the new master event. Errors if no master with `master_uid`
    /// exists or if the master is not recurring.
    pub fn split_recurring_series_at(
        &self,
        master_uid: &str,
        split_start: EventTime,
        split_end: EventTime,
        new_recurrence: Option<Recurrence>,
    ) -> Result<Event, CalendarError> {
        let mut all_events = self.events()?;

        let master_idx = all_events
            .iter()
            .position(|ce| ce.event().uid.as_str() == master_uid && ce.event().recurrence.is_some())
            .ok_or_else(|| CalendarError::MasterNotFound(master_uid.to_string()))?;

        let mut master_ce = all_events.swap_remove(master_idx);
        let master_event = master_ce.event().clone();
        let master_recurrence = master_event
            .recurrence
            .as_ref()
            .ok_or_else(|| CalendarError::NotRecurring(master_uid.to_string()))?;

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
            if ce.event().uid.as_str() != master_uid {
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

    pub(crate) fn config(&self) -> Option<&CalendarConfig> {
        self.config.as_ref()
    }

    pub fn remote_email(&self) -> Option<&str> {
        self.remote_config()
            .and_then(|remote_config| remote_config.account_identifier())
            .filter(|id| id.contains('@'))
    }

    pub(crate) fn record_synced_ids(
        &mut self,
        ids: impl IntoIterator<Item = EventInstanceId>,
    ) -> Result<(), CalendarError> {
        self.state
            .add_new_synced_ids(ids)
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
    use chrono::{NaiveDate, TimeZone};

    fn make_master(uid: &str, start: DateTime<Utc>, rrule: &str) -> Event {
        let mut event = Event::new("Daily standup", EventTime::DateTimeUtc(start));
        event.uid = crate::event::EventUid::new(uid);
        event.description = Some("Notes".to_string());
        event.location = Some("Office".to_string());
        event.set_end(EventTime::DateTimeUtc(start + chrono::Duration::hours(1)));
        event.set_recurrence(Recurrence::new(rrule));
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
        cal.split_recurring_series_at(uid, split_start, split_end, None)
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
                uid,
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
            uid,
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
            uid,
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
            uid,
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
                uid,
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
                "nonexistent",
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
                "solo@test",
                EventTime::DateTimeUtc(t(2026, 4, 5, 10, 0)),
                EventTime::DateTimeUtc(t(2026, 4, 5, 11, 0)),
                None,
            )
            .unwrap_err();

        assert!(matches!(err, CalendarError::MasterNotFound(_)));
    }
}
