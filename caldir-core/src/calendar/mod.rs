//! Calendar directory management.

mod cache;
mod calendar_event;
pub mod config;
mod split;
mod state;
#[cfg(test)]
mod test_support;

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::caldir::Caldir;
use crate::calendar::calendar_event::CalendarEvent;
use crate::calendar::config::CalendarConfig;
use crate::calendar::state::CalendarState;
use crate::error::{CalDirError, CalDirResult};
use crate::event::Event;
use crate::event_time::EventTime;
use crate::recurrence::expand_recurring_event;
use crate::remote::Remote;
use crate::utils::slugify;

#[derive(Clone, Serialize, Deserialize)]
pub struct Calendar {
    pub slug: String,
    pub data_path: PathBuf, // ~/caldir/{slug}
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

    pub fn load(slug: &str) -> CalDirResult<Self> {
        let caldir = Caldir::load()?;
        Self::load_in(slug, caldir.data_path())
    }

    /// Load a calendar at `caldir_data_path/slug`
    /// (caldir_data_path` is `~/caldir` in production, a tempdir in tests)
    pub fn load_in(slug: &str, caldir_data_path: impl AsRef<Path>) -> CalDirResult<Self> {
        let data_path = caldir_data_path.as_ref().join(slug);
        let config = CalendarConfig::load(&data_path)?;

        Ok(Calendar {
            slug: slug.to_string(),
            data_path,
            config,
        })
    }

    /// Construct an in-memory calendar without touching disk.
    /// Used by the `connect` flow when materializing a new calendar
    /// from a remote config before saving it.
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

    pub fn state(&self) -> CalendarState {
        CalendarState::load(self.clone())
    }

    pub fn save_config(&self) -> CalDirResult<()> {
        self.config.save(self.data_path())
    }

    /// Get the account email for this calendar (from remote config)
    pub fn account_email(&self) -> Option<&str> {
        self.config.remote.as_ref()?.account_identifier()
    }

    /// Where changes get pushed to / pulled from (None if no remote configured)
    pub fn remote(&self) -> Option<&Remote> {
        self.config.remote.as_ref()
    }

    /// Load events from local directory.
    pub fn events(&self) -> CalDirResult<Vec<CalendarEvent>> {
        cache::cached_events_for_dir(self.data_path())
    }

    /// Load events in the given date range, expanding recurring events into instances.
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
    use super::test_support::t;
    use super::*;
    use crate::event::Recurrence;
    use chrono::{NaiveDate, NaiveDateTime};

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
}
