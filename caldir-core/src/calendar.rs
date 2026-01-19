//! Calendar directory management.

use serde::{Deserialize, Serialize};

use crate::caldir::Caldir;
use crate::config::calendar_config::CalendarConfig;
use crate::error::{CalDirError, CalDirResult};
use crate::event::{Event, EventTime};
use crate::ics::{CalendarMetadata, generate_ics, parse_event};
use crate::local::{LocalEvent, LocalState};
use crate::remote::remote::Remote;
use std::collections::HashSet;
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Clone, Serialize, Deserialize)]
pub struct Calendar {
    pub name: String,
    pub config: CalendarConfig,
}

impl Calendar {
    pub fn new(name: &str) -> Self {
        Calendar {
            name: name.to_string(),
            config: CalendarConfig::default(),
        }
    }

    pub fn load(name: &str) -> CalDirResult<Self> {
        let calendar_dir = Self::data_dir_path(name)?;
        let config = CalendarConfig::load(&calendar_dir)?;

        Ok(Calendar {
            name: name.to_string(),
            config,
        })
    }

    pub fn data_dir_path(calendar_name: &str) -> CalDirResult<PathBuf> {
        let caldir = Caldir::load()?;
        Ok(caldir.data_path().join(calendar_name))
    }

    pub fn data_dir(&self) -> CalDirResult<PathBuf> {
        Self::data_dir_path(&self.name)
    }

    /// Where changes get pushed to / pulled from (None if no remote configured)
    pub fn remote(&self) -> Option<Remote> {
        self.config.remote.clone()
    }

    /// Save the calendar config to .caldir/config.toml
    pub fn save_config(&self) -> CalDirResult<()> {
        self.config.save(&self.data_dir()?)
    }

    /// Load events from local directory
    pub fn events(&self) -> CalDirResult<Vec<LocalEvent>> {
        let data_path = self.data_dir()?;

        let entries = std::fs::read_dir(&data_path)?;

        let local_events = entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|e| e == "ics"))
            .filter_map(|path| LocalEvent::from_file(path).ok())
            .collect();

        Ok(local_events)
    }

    /// UIDs we've seen before (for detecting deletions)
    pub fn seen_event_uids(&self) -> CalDirResult<HashSet<String>> {
        let dir = self.data_dir()?;

        Ok(LocalState::load(&dir)?.synced_uids().clone())
    }

    // =========================================================================
    // Event operations
    // =========================================================================

    pub fn create_event(&self, event: &Event) -> CalDirResult<()> {
        let dir = self.data_dir()?;
        std::fs::create_dir_all(&dir)?;

        let content = generate_ics(event, &self.metadata())?;
        let filename = filename_for(event, &dir)?;

        std::fs::write(dir.join(filename), content)?;
        Ok(())
    }

    pub fn update_event(&self, event_id: &str, event: &Event) -> CalDirResult<()> {
        self.delete_event(event_id)?;
        self.create_event(event)
    }

    pub fn delete_event(&self, event_id: &str) -> CalDirResult<()> {
        if let Some(local) = self.events()?.into_iter().find(|e| e.event.id == event_id) {
            std::fs::remove_file(&local.path)?;
        }
        Ok(())
    }

    pub fn update_sync_state(&self) -> CalDirResult<()> {
        let dir = self.data_dir()?;

        let synced_uids: HashSet<String> = self.events()?.into_iter().map(|e| e.event.id).collect();
        LocalState::save(&dir, &synced_uids)
    }

    fn metadata(&self) -> CalendarMetadata {
        CalendarMetadata {
            calendar_id: self.name.clone(),
            calendar_name: self.name.clone(),
        }
    }
}

impl fmt::Display for Calendar {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

// =============================================================================
// Filename generation
// =============================================================================

/// Generate a unique filename for an event, handling collisions.
fn filename_for(event: &Event, dir: &Path) -> CalDirResult<String> {
    let base = base_filename(event);
    let stem = base.trim_end_matches(".ics");

    // Try base filename first
    if !dir.join(&base).exists() || file_has_uid(dir, &base, &event.id) {
        return Ok(base);
    }

    // Collision - try suffixes
    for n in 2..=100 {
        let suffixed = format!("{}-{}.ics", stem, n);
        if !dir.join(&suffixed).exists() || file_has_uid(dir, &suffixed, &event.id) {
            return Ok(suffixed);
        }
    }

    Err(CalDirError::Sync(format!(
        "Too many filename collisions for {}",
        base
    )))
}

fn file_has_uid(dir: &Path, filename: &str, uid: &str) -> bool {
    std::fs::read_to_string(dir.join(filename))
        .ok()
        .and_then(|content| parse_event(&content))
        .is_some_and(|e| e.id == uid)
}

fn base_filename(event: &Event) -> String {
    let slug = slugify(&event.summary);

    if event.recurrence.is_some() {
        return format!("_recurring__{}.ics", slug);
    }

    let date = match &event.start {
        EventTime::Date(d) => d.format("%Y-%m-%d").to_string(),
        EventTime::DateTimeUtc(dt) => dt.format("%Y-%m-%dT%H%M").to_string(),
        EventTime::DateTimeFloating(dt) => dt.format("%Y-%m-%dT%H%M").to_string(),
        EventTime::DateTimeZoned { datetime, .. } => datetime.format("%Y-%m-%dT%H%M").to_string(),
    };

    format!("{}__{}.ics", date, slug)
}

pub fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
        .chars()
        .take(50)
        .collect()
}
