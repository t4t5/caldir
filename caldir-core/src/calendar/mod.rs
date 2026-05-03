//! Calendar directory management.

mod cache;
pub mod config;
mod event;
mod state;

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::caldir::Caldir;
use crate::calendar::config::CalendarConfig;
use crate::calendar::event::CalendarEvent;
use crate::calendar::state::CalendarState;
use crate::error::{CalDirError, CalDirResult};
use crate::event::{Event, Recurrence};
use crate::event_time::EventTime;
use crate::recurrence::{expand_recurring_event, truncate_recurrence_before};
use crate::remote::Remote;
use crate::utils::slugify;

#[derive(Clone, Serialize, Deserialize)]
pub struct Calendar {
    pub slug: String,
    /// Absolute path to this calendar's directory (`{caldir_data_path}/{slug}`).
    /// This is the calendar's own data path; `Caldir::data_path()` is the
    /// parent that holds it.
    pub data_path: PathBuf,
    pub config: CalendarConfig,
}

impl Calendar {
    fn base_slug_for(name: Option<&str>) -> String {
        name.map(slugify).unwrap_or_else(|| "calendar".to_string())
    }

    /// Generate a unique slug that doesn't conflict with existing calendar directories.
    /// If the base slug exists, tries slug-2, slug-3, etc.
    pub fn unique_slug_in(name: Option<&str>, caldir_data_path: &Path) -> CalDirResult<String> {
        let base = Self::base_slug_for(name);

        if !caldir_data_path.join(&base).exists() {
            return Ok(base);
        }

        for n in 2..=100 {
            let suffixed = format!("{}-{}", base, n);
            if !caldir_data_path.join(&suffixed).exists() {
                return Ok(suffixed);
            }
        }

        Err(CalDirError::Config(format!(
            "Too many calendar name collisions for '{}'",
            base
        )))
    }

    /// Production constructor — resolves the caldir data path via the global
    /// `Caldir::load()` config. Use [`Calendar::load_in`] in tests or anywhere
    /// you need to point at a specific caldir.
    pub fn load(slug: &str) -> CalDirResult<Self> {
        let caldir = Caldir::load()?;
        Self::load_in(slug, caldir.data_path())
    }

    /// Load a calendar at `caldir_data_path/slug`. `caldir_data_path` is the
    /// root caldir data directory (`~/caldir` in production, a tempdir in tests).
    pub fn load_in(slug: &str, caldir_data_path: impl AsRef<Path>) -> CalDirResult<Self> {
        let data_path = caldir_data_path.as_ref().join(slug);
        let config = CalendarConfig::load(&data_path)?;
        Ok(Calendar {
            slug: slug.to_string(),
            data_path,
            config,
        })
    }

    /// Construct an in-memory calendar without touching disk. Used by the
    /// `connect` flow when materializing a new calendar from a remote config
    /// before saving it.
    pub fn new_in(slug: &str, caldir_data_path: impl AsRef<Path>, config: CalendarConfig) -> Self {
        Calendar {
            slug: slug.to_string(),
            data_path: caldir_data_path.as_ref().join(slug),
            config,
        }
    }

    pub fn data_path(&self) -> &Path {
        self.data_path.as_path()
    }

    // STATE + CONFIG:

    pub fn state(&self) -> CalendarState {
        CalendarState::load(self.clone())
    }

    pub fn save_config(&self) -> CalDirResult<()> {
        self.config.save(self.data_path())
    }

    // EVENTS OPERATIONS:

    /// Get the account email for this calendar (from remote config)
    pub fn account_email(&self) -> Option<&str> {
        self.config.remote.as_ref()?.account_identifier()
    }

    /// Where changes get pushed to / pulled from (None if no remote configured)
    pub fn remote(&self) -> Option<&Remote> {
        self.config.remote.as_ref()
    }

    /// Load events from local directory.
    ///
    /// Backed by a process-wide per-file cache (`calendar::cache`) for the
    /// benefit of long-running hosts (e.g. GUI desktop apps using caldir):
    /// the first call reads and parses every `.ics` file,
    /// subsequent calls only re-parse files whose mtime has changed.
    /// The one-shot CLI gets no benefit (fresh process per
    /// invocation) but pays no meaningful cost either.
    pub fn events(&self) -> CalDirResult<Vec<CalendarEvent>> {
        cache::cached_events_for_dir(self.data_path())
    }

    /// Load events in the given date range, expanding recurring events into instances.
    ///
    /// Returns individual event instances (not master recurring events). Instance overrides
    /// from disk replace their corresponding generated occurrences.
    pub fn events_in_range(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> CalDirResult<Vec<Event>> {
        let all_events = self.events()?.into_iter().map(|ce| ce.event);
        events_in_range_from_events(all_events, from, to)
    }

    /// Search events by summary (case-insensitive substring match).
    /// Returns raw CalendarEvent entries (not expanded recurring instances).
    pub fn search_events(&self, query: &str) -> CalDirResult<Vec<CalendarEvent>> {
        let query_lower = query.to_lowercase();
        let all = self.events()?;
        Ok(all
            .into_iter()
            .filter(|ce| ce.event.summary.to_lowercase().contains(&query_lower))
            .collect())
    }

    pub fn create_event(&self, event: &Event) -> CalDirResult<PathBuf> {
        let dir = self.data_path();
        std::fs::create_dir_all(dir)?;

        let event_slug = CalendarEvent::unique_slug_for(event, self)?;
        let event_path = dir.join(format!("{}.ics", event_slug));
        let calendar_event = CalendarEvent::new(event_path.clone(), event);

        calendar_event.save()?;
        Ok(event_path)
    }

    /// Update a local event file by finding it via uid and replacing its content.
    /// For recurring event instances, also matches on recurrence_id.
    pub fn update_event(
        &self,
        uid: &str,
        recurrence_id: Option<&EventTime>,
        event: &Event,
    ) -> CalDirResult<()> {
        // Delete first so that we don't end up with file with same name + suffix
        self.delete_event(uid, recurrence_id)?;
        self.create_event(event)?;
        Ok(())
    }

    /// Find the master recurring event for a given uid.
    pub fn master_event_for(&self, uid: &str) -> CalDirResult<Option<Event>> {
        let master = self
            .events()?
            .into_iter()
            .find(|ce| ce.event.uid == uid && ce.event.recurrence.is_some())
            .map(|ce| ce.event);
        Ok(master)
    }

    /// Resolves either:
    /// - on-disk event (non-recurring, master recurring, instance override)
    /// - synthetic event (generic instance of recurring event)
    pub fn event_by_unique_id(&self, unique_id: &str) -> CalDirResult<Option<Event>> {
        let events: Vec<Event> = self.events()?.into_iter().map(|ce| ce.event).collect();
        Ok(events
            .iter()
            .find(|e| e.unique_id() == unique_id)
            .cloned()
            .or_else(|| synthesize_recurring_instance(unique_id, &events)))
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
    ) -> CalDirResult<Event> {
        let all_events = self.events()?;

        // 1. Find the master.
        let master = all_events
            .iter()
            .find(|ce| ce.event.uid == master_uid && ce.event.recurrence_id.is_none())
            .map(|ce| ce.event.clone())
            .ok_or_else(|| {
                CalDirError::Config(format!("Master event not found: {}", master_uid))
            })?;
        let master_recurrence = master
            .recurrence
            .as_ref()
            .ok_or_else(|| CalDirError::Config(format!("Event {} is not recurring", master_uid)))?;

        // 2. Truncate the master's recurrence and write it back.
        let truncated_recurrence =
            truncate_recurrence_before(master_recurrence, &master.start, &split_start);
        let truncated_master = Event {
            recurrence: Some(truncated_recurrence),
            updated: Some(Utc::now()),
            sequence: master.sequence.map(|s| s + 1).or(Some(1)),
            ..master.clone()
        };
        self.update_event(
            &master.uid,
            master.recurrence_id.as_ref(),
            &truncated_master,
        )?;

        // 3. Create the new master, inheriting all metadata from the original.
        let new_master = Event {
            start: split_start.clone(),
            end: split_end,
            recurrence: new_recurrence,
            recurrence_id: None,
            updated: Some(Utc::now()),
            sequence: None,
            ..master.with_new_uid()
        };
        self.create_event(&new_master)?;

        // 4. Delete overrides at or after split_start. Includes the override
        //    at split_start itself (the new master replaces it) and orphaned
        //    overrides at later dates that no longer match an occurrence of
        //    the truncated master.
        let split_start_utc = split_start.to_utc();
        for ce in &all_events {
            if ce.event.uid != master_uid {
                continue;
            }
            let Some(rid) = &ce.event.recurrence_id else {
                continue;
            };
            if let (Some(rid_utc), Some(start_utc)) = (rid.to_utc(), split_start_utc)
                && rid_utc >= start_utc
            {
                self.delete_event(&ce.event.uid, Some(rid))?;
            }
        }

        Ok(new_master)
    }

    /// Delete a local event file by id
    /// For recurring event instances, also matches on recurrence_id.
    pub fn delete_event(&self, uid: &str, recurrence_id: Option<&EventTime>) -> CalDirResult<()> {
        if let Some(local) = self
            .events()?
            .into_iter()
            .find(|e| e.event.uid == uid && e.event.recurrence_id.as_ref() == recurrence_id)
        {
            std::fs::remove_file(&local.path)?;
        }
        Ok(())
    }
}

impl fmt::Display for Calendar {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.slug)
    }
}

/// Build an in-memory override skeleton for a synthetic recurring-instance id
/// (`{uid}__{rid_ics}`) by inheriting the master's metadata. Returns `None`
/// when `unique_id` isn't synthetic, no master matches, or `rid_ics` won't
/// parse against the master's start.
fn synthesize_recurring_instance(unique_id: &str, events: &[Event]) -> Option<Event> {
    let (uid, rid_ics) = unique_id.split_once("__")?;

    let master = events
        .iter()
        .find(|e| e.uid == uid && e.recurrence.is_some())?;

    let recurrence_id = EventTime::from_ics_string_like(rid_ics, &master.start).ok()?;

    Some(Event {
        recurrence: None,
        recurrence_id: Some(recurrence_id),
        sequence: None,
        updated: None,
        ..master.clone()
    })
}

/// Filter and expand an in-memory event set for the given UTC range.
///
/// This is the pure core behind [`Calendar::events_in_range`]. Filesystem-backed
/// calendars load events from disk first, then delegate here.
pub fn events_in_range_from_events<I>(
    all_events: I,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> CalDirResult<Vec<Event>>
where
    I: IntoIterator<Item = Event>,
{
    events_in_range_from_events_in_zone(all_events, from, to, rrule::Tz::LOCAL)
}

fn events_in_range_from_events_in_zone<I>(
    all_events: I,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    host_tz: rrule::Tz,
) -> CalDirResult<Vec<Event>>
where
    I: IntoIterator<Item = Event>,
{
    // Classify events into singles, masters, and overrides
    let mut singles: Vec<Event> = Vec::new();
    let mut masters: Vec<Event> = Vec::new();
    // uid -> (recurrence_id ICS string -> override Event)
    let mut overrides: HashMap<String, HashMap<String, Event>> = HashMap::new();

    for event in all_events {
        if event.recurrence.is_some() {
            masters.push(event);
        } else if let Some(ref rid) = event.recurrence_id {
            overrides
                .entry(event.uid.clone())
                .or_default()
                .insert(rid.to_ics_string(), event);
        } else {
            singles.push(event);
        }
    }

    let mut result: Vec<Event> = Vec::new();

    // Include singles that fall in range
    for event in singles {
        if event.starts_in_range(from, to, &host_tz) {
            result.push(event);
        }
    }

    // Expand each master into instances within range
    for master in &masters {
        let uid_overrides = overrides.remove(&master.uid).unwrap_or_default();
        let instances = expand_recurring_event(master, from, to, &uid_overrides, host_tz)?;
        result.extend(instances);
    }

    // Include orphaned overrides (override whose master is missing) if in range
    for (_uid, orphans) in overrides {
        for (_rid, event) in orphans {
            if event.starts_in_range(from, to, &host_tz) {
                result.push(event);
            }
        }
    }

    // Sort by resolved start instant in the same host timezone used for filtering.
    result.sort_by_key(|event| {
        event
            .start
            .resolve_instant_in_zone(&host_tz)
            .or_else(|| event.start.to_utc())
    });

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, NaiveDateTime, TimeZone};

    fn t(year: i32, month: u32, day: u32, hour: u32, min: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, day, hour, min, 0)
            .unwrap()
    }

    fn naive(year: i32, month: u32, day: u32, hour: u32, min: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(year, month, day)
            .unwrap()
            .and_hms_opt(hour, min, 0)
            .unwrap()
    }

    #[test]
    fn events_in_range_preserves_recurring_floating_wall_time() {
        let pacific = rrule::Tz::US__Pacific;
        let start = EventTime::DateTimeFloating(naive(2026, 1, 15, 9, 0));
        let end = EventTime::DateTimeFloating(naive(2026, 1, 15, 10, 0));
        let event = Event::new(
            "Daily standup".into(),
            start.clone(),
            end.clone(),
            None,
            None,
            Some(Recurrence {
                rrule: "FREQ=DAILY;COUNT=1".into(),
                exdates: vec![],
            }),
            vec![],
        );

        // 09:00 Pacific on this date is 17:00 UTC.
        let events = events_in_range_from_events_in_zone(
            vec![event],
            t(2026, 1, 15, 16, 30),
            t(2026, 1, 15, 17, 30),
            pacific,
        )
        .unwrap();

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].start, start);
        assert_eq!(events[0].end, end);
        assert!(events[0].starts_in_range(
            t(2026, 1, 15, 16, 30),
            t(2026, 1, 15, 17, 30),
            &pacific
        ));
    }

    // --- split_recurring_series_at -------------------------------------------------

    /// Build a recurring master event with a fixed UID so tests can find it
    /// without depending on the random uuid generator.
    fn make_master(uid: &str, start_utc: DateTime<Utc>, rrule: &str) -> Event {
        let start = EventTime::DateTimeUtc(start_utc);
        let end = EventTime::DateTimeUtc(start_utc + chrono::Duration::hours(1));
        let mut event = Event::new(
            "Daily standup".into(),
            start,
            end,
            Some("Notes".into()),
            Some("Office".into()),
            Some(Recurrence {
                rrule: rrule.into(),
                exdates: vec![],
            }),
            vec![],
        );
        event.uid = uid.into();
        event
    }

    /// Build an instance override sharing `master_uid` at `rid_utc`.
    fn make_override(master_uid: &str, rid_utc: DateTime<Utc>, summary: &str) -> Event {
        let start = EventTime::DateTimeUtc(rid_utc);
        let end = EventTime::DateTimeUtc(rid_utc + chrono::Duration::hours(1));
        let mut event = Event::new(summary.into(), start, end, None, None, None, vec![]);
        event.uid = master_uid.into();
        event.recurrence_id = Some(EventTime::DateTimeUtc(rid_utc));
        event
    }

    /// Make a Calendar pointing at a fresh tempdir. The TempDir is returned
    /// alongside so it stays alive for the test's lifetime.
    fn make_calendar() -> (tempfile::TempDir, Calendar) {
        let tmp = tempfile::tempdir().unwrap();
        let cal = Calendar::load_in("test", tmp.path()).unwrap();
        std::fs::create_dir_all(cal.data_path()).unwrap();
        (tmp, cal)
    }

    /// Find the master (recurring) event for `uid` in the calendar's on-disk events.
    fn loaded_master(cal: &Calendar, uid: &str) -> Event {
        cal.events()
            .unwrap()
            .into_iter()
            .map(|ce| ce.event)
            .find(|e| e.uid == uid && e.recurrence.is_some())
            .expect("master not found on disk")
    }

    fn loaded_overrides(cal: &Calendar, uid: &str) -> Vec<Event> {
        cal.events()
            .unwrap()
            .into_iter()
            .map(|ce| ce.event)
            .filter(|e| e.uid == uid && e.recurrence_id.is_some())
            .collect()
    }

    #[test]
    fn split_truncates_master_rrule_before_split() {
        let (_tmp, cal) = make_calendar();
        let uid = "master@test";
        let master_start = t(2026, 4, 1, 10, 0);
        cal.create_event(&make_master(uid, master_start, "FREQ=DAILY"))
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
        let (_tmp, cal) = make_calendar();
        let uid = "master@test";
        cal.create_event(&make_master(uid, t(2026, 4, 1, 10, 0), "FREQ=DAILY"))
            .unwrap();

        let new_recurrence = Some(Recurrence {
            rrule: "FREQ=WEEKLY".into(),
            exdates: vec![],
        });
        let new_master = cal
            .split_recurring_series_at(
                uid,
                EventTime::DateTimeUtc(t(2026, 4, 5, 10, 0)),
                EventTime::DateTimeUtc(t(2026, 4, 5, 11, 30)),
                new_recurrence,
            )
            .unwrap();

        // Fresh UID, not the master's.
        assert_ne!(new_master.uid, uid);
        // Inherits metadata.
        assert_eq!(new_master.summary, "Daily standup");
        assert_eq!(new_master.description.as_deref(), Some("Notes"));
        assert_eq!(new_master.location.as_deref(), Some("Office"));
        // Uses the new start/end and recurrence.
        assert_eq!(
            new_master.start,
            EventTime::DateTimeUtc(t(2026, 4, 5, 10, 0))
        );
        assert_eq!(
            new_master.end,
            EventTime::DateTimeUtc(t(2026, 4, 5, 11, 30))
        );
        assert_eq!(new_master.recurrence.as_ref().unwrap().rrule, "FREQ=WEEKLY");
        assert!(new_master.recurrence_id.is_none());

        // And it landed on disk.
        let on_disk = loaded_master(&cal, &new_master.uid);
        assert_eq!(on_disk.uid, new_master.uid);
    }

    #[test]
    fn split_bumps_master_sequence() {
        let (_tmp, cal) = make_calendar();
        let uid = "master@test";
        let mut master = make_master(uid, t(2026, 4, 1, 10, 0), "FREQ=DAILY");
        master.sequence = Some(3);
        cal.create_event(&master).unwrap();

        cal.split_recurring_series_at(
            uid,
            EventTime::DateTimeUtc(t(2026, 4, 5, 10, 0)),
            EventTime::DateTimeUtc(t(2026, 4, 5, 11, 0)),
            None,
        )
        .unwrap();

        assert_eq!(loaded_master(&cal, uid).sequence, Some(4));
    }

    #[test]
    fn split_drops_overrides_at_or_after_split_keeps_earlier_ones() {
        let (_tmp, cal) = make_calendar();
        let uid = "master@test";
        cal.create_event(&make_master(uid, t(2026, 4, 1, 10, 0), "FREQ=DAILY"))
            .unwrap();

        // Three overrides: before, exactly at, and after the split.
        cal.create_event(&make_override(uid, t(2026, 4, 3, 10, 0), "before"))
            .unwrap();
        cal.create_event(&make_override(uid, t(2026, 4, 5, 10, 0), "at-split"))
            .unwrap();
        cal.create_event(&make_override(uid, t(2026, 4, 7, 10, 0), "after"))
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
        assert_eq!(overrides[0].summary, "before");
    }

    #[test]
    fn split_drops_exdates_at_or_after_split() {
        let (_tmp, cal) = make_calendar();
        let uid = "master@test";
        let mut master = make_master(uid, t(2026, 4, 1, 10, 0), "FREQ=DAILY");
        let kept = EventTime::DateTimeUtc(t(2026, 4, 3, 10, 0));
        let dropped_at = EventTime::DateTimeUtc(t(2026, 4, 5, 10, 0));
        let dropped_after = EventTime::DateTimeUtc(t(2026, 4, 6, 10, 0));
        master.recurrence = Some(Recurrence {
            rrule: "FREQ=DAILY".into(),
            exdates: vec![kept.clone(), dropped_at, dropped_after],
        });
        cal.create_event(&master).unwrap();

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
        let (_tmp, cal) = make_calendar();
        let uid = "master@test";
        cal.create_event(&make_master(uid, t(2026, 4, 1, 10, 0), "FREQ=DAILY"))
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
        let (_tmp, cal) = make_calendar();
        let err = cal
            .split_recurring_series_at(
                "nonexistent",
                EventTime::DateTimeUtc(t(2026, 4, 5, 10, 0)),
                EventTime::DateTimeUtc(t(2026, 4, 5, 11, 0)),
                None,
            )
            .unwrap_err();
        assert!(format!("{err}").contains("not found"));
    }

    #[test]
    fn split_errors_when_master_is_not_recurring() {
        let (_tmp, cal) = make_calendar();
        let uid = "single@test";
        let mut single = Event::new(
            "Single".into(),
            EventTime::DateTimeUtc(t(2026, 4, 1, 10, 0)),
            EventTime::DateTimeUtc(t(2026, 4, 1, 11, 0)),
            None,
            None,
            None,
            vec![],
        );
        single.uid = uid.into();
        cal.create_event(&single).unwrap();

        let err = cal
            .split_recurring_series_at(
                uid,
                EventTime::DateTimeUtc(t(2026, 4, 5, 10, 0)),
                EventTime::DateTimeUtc(t(2026, 4, 5, 11, 0)),
                None,
            )
            .unwrap_err();
        assert!(format!("{err}").contains("not recurring"));
    }
}
