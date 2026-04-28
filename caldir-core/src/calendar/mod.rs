//! Calendar directory management.

mod cache;
pub mod config;
mod event;
mod state;

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::caldir::Caldir;
use crate::calendar::config::CalendarConfig;
use crate::calendar::event::CalendarEvent;
use crate::calendar::state::CalendarState;
use crate::error::{CalDirError, CalDirResult};
use crate::event::{Event, EventStatus, EventTime, Recurrence};
use crate::recurrence::{expand_recurring_event, truncate_recurrence_before};
use crate::remote::Remote;
use crate::utils::slugify;

#[derive(Clone, Serialize, Deserialize)]
pub struct Calendar {
    pub slug: String,
    pub config: CalendarConfig,
}

impl Calendar {
    pub fn new(slug: &str) -> Self {
        Calendar {
            slug: slug.to_string(),
            config: CalendarConfig::default(),
        }
    }

    fn base_slug_for(name: Option<&str>) -> String {
        name.map(slugify).unwrap_or_else(|| "calendar".to_string())
    }

    /// Generate a unique slug that doesn't conflict with existing calendar directories.
    /// If the base slug exists, tries slug-2, slug-3, etc.
    pub fn unique_slug_for(name: Option<&str>) -> CalDirResult<String> {
        let base = Self::base_slug_for(name);
        let caldir = Caldir::load()?;
        let data_path = caldir.data_path();

        // Try base slug first
        if !data_path.join(&base).exists() {
            return Ok(base);
        }

        // Collision - try suffixes
        for n in 2..=100 {
            let suffixed = format!("{}-{}", base, n);
            if !data_path.join(&suffixed).exists() {
                return Ok(suffixed);
            }
        }

        Err(CalDirError::Config(format!(
            "Too many calendar name collisions for '{}'",
            base
        )))
    }

    pub fn load(slug: &str) -> CalDirResult<Self> {
        let calendar_dir = Self::path_for(slug)?;
        let config = CalendarConfig::load(&calendar_dir)?;

        Ok(Calendar {
            slug: slug.to_string(),
            config,
        })
    }

    pub fn path_for(slug: &str) -> CalDirResult<PathBuf> {
        let caldir = Caldir::load()?;
        Ok(caldir.data_path().join(slug))
    }

    pub fn path(&self) -> CalDirResult<PathBuf> {
        Self::path_for(&self.slug)
    }

    // STATE + CONFIG:

    pub fn state(&self) -> CalendarState {
        CalendarState::load(self.clone())
    }

    pub fn save_config(&self) -> CalDirResult<()> {
        self.config.save(&self.path()?)
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
        let data_path = self.path()?;
        cache::cached_events_for_dir(&data_path)
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
        let all_events = self.events()?;

        // Classify events into singles, masters, and overrides
        let mut singles: Vec<Event> = Vec::new();
        let mut masters: Vec<Event> = Vec::new();
        // uid → (recurrence_id ICS string → override Event)
        let mut overrides: HashMap<String, HashMap<String, Event>> = HashMap::new();

        for ce in all_events {
            let event = ce.event;
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
            if event.status == EventStatus::Cancelled {
                continue;
            }
            if let Some(start_utc) = event.start.to_utc()
                && start_utc >= from
                && start_utc <= to
            {
                result.push(event);
            }
        }

        // Expand each master into instances within range
        for master in &masters {
            let uid_overrides = overrides.remove(&master.uid).unwrap_or_default();
            let instances = expand_recurring_event(master, from, to, &uid_overrides)?;
            result.extend(instances);
        }

        // Include orphaned overrides (override whose master is missing) if in range
        for (_uid, orphans) in overrides {
            for (_rid, event) in orphans {
                if event.status == EventStatus::Cancelled {
                    continue;
                }
                if let Some(start_utc) = event.start.to_utc()
                    && start_utc >= from
                    && start_utc <= to
                {
                    result.push(event);
                }
            }
        }

        // Sort by start time
        result.sort_by(|a, b| a.start.to_utc().cmp(&b.start.to_utc()));

        Ok(result)
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
        let dir = self.path()?;
        std::fs::create_dir_all(&dir)?;

        let event_slug = CalendarEvent::unique_slug_for(event, self)?;
        let event_path = dir.join(format!("{}.ics", event_slug));
        let calendar_event = CalendarEvent::new(event_path.clone(), event);

        calendar_event.save()?;
        Ok(event_path)
    }

    /// Update a local event file by finding it via uid and replacing its content.
    /// For recurring event instances, also matches on recurrence_id.
    pub fn update_event(&self, uid: &str, event: &Event) -> CalDirResult<()> {
        self.delete_event(uid, event.recurrence_id.as_ref())?;
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
        self.update_event(&master.uid, &truncated_master)?;

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
