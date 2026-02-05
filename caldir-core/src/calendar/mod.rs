//! Calendar directory management.

pub mod config;
mod event;
mod state;

use serde::{Deserialize, Serialize};

use crate::caldir::Caldir;
use crate::calendar::config::CalendarConfig;
use crate::calendar::event::CalendarEvent;
use crate::calendar::state::CalendarState;
use crate::error::{CalDirError, CalDirResult};
use crate::event::{Event, EventTime};
use crate::remote::Remote;
use crate::utils::slugify;
use std::collections::HashSet;
use std::fmt;
use std::path::PathBuf;

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

    pub fn save_state(&self) -> CalDirResult<()> {
        // Track event IDs (uid + recurrence_id) for sync state
        let known_event_ids: HashSet<String> = self
            .events()?
            .into_iter()
            .map(|e| e.event.unique_id())
            .collect();
        self.state().save(&known_event_ids)
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
        self.delete_event_by_uid(uid, event.recurrence_id.as_ref())?;
        self.create_event(event)
    }

    /// Delete a local event file by uid.
    /// For recurring event instances, also matches on recurrence_id.
    pub fn delete_event_by_uid(
        &self,
        uid: &str,
        recurrence_id: Option<&EventTime>,
    ) -> CalDirResult<()> {
        if let Some(local) = self.events()?.into_iter().find(|e| {
            e.event.uid == uid && e.event.recurrence_id.as_ref() == recurrence_id
        }) {
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
