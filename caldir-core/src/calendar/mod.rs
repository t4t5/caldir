//! Calendar directory management.

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
use crate::event::{Event, EventTime};
use crate::recurrence::expand_recurring_event;
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

    /// Where changes get pushed to / pulled from (None if no remote configured)
    pub fn remote(&self) -> Option<&Remote> {
        self.config.remote.as_ref()
    }

    /// Load events from local directory
    pub fn events(&self) -> CalDirResult<Vec<CalendarEvent>> {
        let data_path = self.path()?;

        let entries = std::fs::read_dir(&data_path)?;

        let local_events = entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|e| e == "ics"))
            .filter_map(|path| CalendarEvent::from_file(path).ok())
            .collect();

        Ok(local_events)
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
            if let Some(start_utc) = event.start.to_utc() {
                if start_utc >= from && start_utc <= to {
                    result.push(event);
                }
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
                if let Some(start_utc) = event.start.to_utc() {
                    if start_utc >= from && start_utc <= to {
                        result.push(event);
                    }
                }
            }
        }

        // Sort by start time
        result.sort_by(|a, b| a.start.to_utc().cmp(&b.start.to_utc()));

        Ok(result)
    }

    pub fn create_event(&self, event: &Event) -> CalDirResult<()> {
        let dir = self.path()?;
        std::fs::create_dir_all(&dir)?;

        let event_slug = CalendarEvent::unique_slug_for(event, self)?;
        let event_path = dir.join(format!("{}.ics", event_slug));
        let calendar_event = CalendarEvent::new(event_path, event);

        calendar_event.save()
    }

    /// Update a local event file by finding it via uid and replacing its content.
    /// For recurring event instances, also matches on recurrence_id.
    pub fn update_event(&self, uid: &str, event: &Event) -> CalDirResult<()> {
        self.delete_event(uid, event.recurrence_id.as_ref())?;
        self.create_event(event)
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
