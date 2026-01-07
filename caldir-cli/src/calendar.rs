use anyhow::Result;
use caldir_core::{Event, EventTime};
use std::collections::HashSet;
use std::fmt;
use std::path::Path;

use crate::caldir::Caldir;
use crate::config::CalendarConfig;
use crate::diff::CalendarDiff;
use crate::ics::{self, CalendarMetadata};
use crate::local::LocalState;
use crate::local_event::LocalEvent;
use crate::remote::Remote;

pub struct Calendar {
    pub name: String,
    pub config: CalendarConfig,
    pub caldir: Caldir,
}

impl Calendar {
    pub fn from(name: &str, caldir: &Caldir, config: &CalendarConfig) -> Self {
        Calendar {
            name: name.to_string(),
            caldir: caldir.clone(),
            config: config.clone(),
        }
    }

    /// Where the calendar's ics files are stored
    fn data_path(&self) -> std::path::PathBuf {
        self.caldir.data_path().join(&self.name)
    }

    /// Where changes get pushed to / pulled from
    pub fn remote(&self) -> Remote {
        Remote::from_calendar_config(&self.config)
    }

    /// Load events from local directory
    pub fn events(&self) -> Result<Vec<LocalEvent>> {
        let local_events = std::fs::read_dir(self.data_path())?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|e| e == "ics"))
            .filter_map(|path| LocalEvent::from_file(path).ok())
            .collect();

        Ok(local_events)
    }

    /// UIDs we've seen before (for detecting deletions)
    pub fn seen_event_uids(&self) -> Result<HashSet<String>> {
        Ok(LocalState::load(&self.data_path())?.synced_uids().clone())
    }

    pub async fn get_diff(&self) -> Result<CalendarDiff<'_>> {
        CalendarDiff::from_calendar(self).await
    }

    pub fn render(&self) -> String {
        format!("🗓️ {}", self.name)
    }

    // =========================================================================
    // Event operations (used by CalendarDiff::apply_pull)
    // =========================================================================

    pub fn create_event(&self, event: &Event) -> Result<()> {
        let dir = self.data_path();
        std::fs::create_dir_all(&dir)?;

        let content = ics::generate_ics(event, &self.metadata())?;
        let filename = filename_for(event, &dir)?;

        std::fs::write(dir.join(filename), content)?;
        Ok(())
    }

    pub fn update_event(&self, event_id: &str, event: &Event) -> Result<()> {
        self.delete_event(event_id)?;
        self.create_event(event)
    }

    pub fn delete_event(&self, event_id: &str) -> Result<()> {
        if let Some(local) = self.events()?.into_iter().find(|e| e.event.id == event_id) {
            std::fs::remove_file(&local.path)?;
        }
        Ok(())
    }

    pub fn update_sync_state(&self) -> Result<()> {
        let synced_uids: HashSet<String> = self.events()?.into_iter().map(|e| e.event.id).collect();
        LocalState::save(&self.data_path(), &synced_uids)
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
fn filename_for(event: &Event, dir: &Path) -> Result<String> {
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

    anyhow::bail!("Too many filename collisions for {}", base)
}

fn file_has_uid(dir: &Path, filename: &str, uid: &str) -> bool {
    std::fs::read_to_string(dir.join(filename))
        .ok()
        .and_then(|content| ics::parse_event(&content))
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

fn slugify(s: &str) -> String {
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
