//! Calendar diff computation and application.

use std::collections::{HashMap, HashSet};

use crate::calendar::Calendar;
use crate::date_range::DateRange;
use crate::diff::{DiffKind, EventDiff};
use crate::error::{CalDirError, CalDirResult};
use crate::event::Event;

/// Represents the differences between local and remote calendar state.
pub struct CalendarDiff {
    pub calendar: Calendar,
    pub to_push: Vec<EventDiff>,
    pub to_pull: Vec<EventDiff>,
}

impl CalendarDiff {
    pub fn is_empty(&self) -> bool {
        self.to_push.is_empty() && self.to_pull.is_empty()
    }

    pub async fn apply_push(&self) -> CalDirResult<()> {
        let remote = self
            .calendar
            .remote()
            .ok_or_else(|| CalDirError::RemoteNotFound(self.calendar.slug.to_string()))?;

        let mut known_ids: HashSet<String> =
            self.calendar.state().read().known_event_ids.into_iter().collect();

        for diff in &self.to_push {
            match diff.kind {
                DiffKind::Create => {
                    let event = diff.new.as_ref().expect("Create must have new event");
                    let created = remote.create_event(event).await?;
                    // Update local file with remote-assigned ID and fields (find by uid)
                    self.calendar.update_event(&event.uid, &created)?;
                    known_ids.insert(created.unique_id());
                }
                DiffKind::Update => {
                    let event = diff.new.as_ref().expect("Update must have new event");
                    let updated = remote.update_event(event).await?;
                    // Update local file with any remote changes (find by uid)
                    self.calendar.update_event(&event.uid, &updated)?;
                }
                DiffKind::Delete => {
                    let event = diff.old.as_ref().expect("Delete must have old event");
                    // Get provider-specific event ID for deletion
                    let provider_event_id = get_provider_event_id(event);
                    remote.delete_event(&provider_event_id).await?;
                    known_ids.remove(&event.unique_id());
                }
            }
        }

        self.calendar.state().save(&known_ids)?;

        Ok(())
    }

    /// Discard local changes by reversing `to_push` diffs.
    /// Create → delete the local file, Update → rewrite with remote version, Delete → recreate from remote.
    pub fn apply_discard(&self) -> CalDirResult<()> {
        for diff in &self.to_push {
            match diff.kind {
                DiffKind::Create => {
                    let event = diff.new.as_ref().expect("Create must have new event");
                    self.calendar
                        .delete_event(&event.uid, event.recurrence_id.as_ref())?;
                }
                DiffKind::Update => {
                    let old = diff.old.as_ref().expect("Update must have old event");
                    let new = diff.new.as_ref().expect("Update must have new event");
                    self.calendar.update_event(&new.uid, old)?;
                }
                DiffKind::Delete => {
                    let event = diff.old.as_ref().expect("Delete must have old event");
                    self.calendar.create_event(event)?;
                }
            }
        }

        Ok(())
    }

    pub fn apply_pull(&self) -> CalDirResult<()> {
        let mut known_ids: HashSet<String> =
            self.calendar.state().read().known_event_ids.into_iter().collect();

        for diff in &self.to_pull {
            match diff.kind {
                DiffKind::Create => {
                    let event = diff.new.as_ref().expect("Create must have new event");
                    self.calendar.create_event(event)?;
                    known_ids.insert(event.unique_id());
                }
                DiffKind::Update => {
                    let event = diff.new.as_ref().expect("Update must have new event");
                    self.calendar.update_event(&event.uid, event)?;
                }
                DiffKind::Delete => {
                    let event = diff.old.as_ref().expect("Delete must have old event");
                    self.calendar
                        .delete_event(&event.uid, event.recurrence_id.as_ref())?;
                    known_ids.remove(&event.unique_id());
                }
            }
        }

        self.calendar.state().save(&known_ids)?;

        Ok(())
    }

    pub async fn from_calendar(calendar: &Calendar, range: &DateRange) -> CalDirResult<Self> {
        let remote = calendar
            .remote()
            .ok_or_else(|| CalDirError::RemoteNotFound(calendar.slug.to_string()))?;

        let remote_events = remote.events(range).await?;
        let local_events = calendar.events()?;
        let known_event_ids = calendar.state().read().known_event_ids;

        // Build lookup maps by event key (uid, recurrence_id)
        let local_by_key: HashMap<_, _> = local_events
            .into_iter()
            .map(|e| (event_key(&e.event), e))
            .collect();

        let remote_by_key: HashMap<_, _> = remote_events
            .into_iter()
            .map(|e| (event_key(&e), e))
            .collect();

        let mut to_push = Vec::new();
        let mut to_pull = Vec::new();

        // Local events not on remote
        for (key, local) in &local_by_key {
            if remote_by_key.contains_key(key) {
                continue; // Will handle in shared_events below
            }

            if known_event_ids.contains(&local.event.unique_id()) {
                // Was synced before, now gone from remote → delete locally
                // But only if in sync range (old events weren't fetched, so we can't know)
                #[allow(clippy::collapsible_if)]
                if let Some(diff) = EventDiff::get_diff(Some(local.event.clone()), None) {
                    if local.is_in_sync_range() {
                        to_pull.push(diff);
                    }
                }
            } else {
                // Never synced -> new local event, push to create
                if let Some(diff) = EventDiff::get_diff(None, Some(local.event.clone())) {
                    to_push.push(diff);
                }
            }
        }

        // Remote events not in local
        for (key, remote) in &remote_by_key {
            if local_by_key.contains_key(key) {
                continue; // Will handle in shared_events below
            }

            if known_event_ids.contains(&remote.unique_id()) {
                // Was synced before, now gone locally -> delete on remote
                if let Some(diff) = EventDiff::get_diff(Some(remote.clone()), None) {
                    to_push.push(diff);
                }
            } else {
                // Never synced -> create locally
                if let Some(diff) = EventDiff::get_diff(None, Some(remote.clone())) {
                    to_pull.push(diff);
                }
            }
        }

        // Events that exist both locally and remotely
        for (key, local) in &local_by_key {
            if let Some(remote) = remote_by_key.get(key) {
                if local.event == *remote {
                    continue;
                }

                // Content differs - use timestamps to determine direction
                if local.is_newer_than(remote) {
                    // Local was modified more recently -> push
                    if let Some(diff) =
                        EventDiff::get_diff(Some(remote.clone()), Some(local.event.clone()))
                    {
                        to_push.push(diff);
                    }
                } else {
                    // Remote was modified more recently -> pull
                    if let Some(diff) =
                        EventDiff::get_diff(Some(local.event.clone()), Some(remote.clone()))
                    {
                        to_pull.push(diff);
                    }
                }
            }
        }

        // Sort by event start time (ascending)
        let sort_by_start =
            |a: &EventDiff, b: &EventDiff| a.event().start.to_utc().cmp(&b.event().start.to_utc());
        to_push.sort_by(sort_by_start);
        to_pull.sort_by(sort_by_start);

        Ok(CalendarDiff {
            calendar: calendar.clone(),
            to_push,
            to_pull,
        })
    }
}

/// Create a key for event lookup: (uid, formatted recurrence_id)
fn event_key(event: &Event) -> (String, Option<String>) {
    (
        event.uid.clone(),
        event.recurrence_id.as_ref().map(|t| t.to_ics_string()),
    )
}

/// Get provider-specific event ID for API calls (deletion, updates).
/// For Google: X-GOOGLE-EVENT-ID from custom_properties
/// For CalDAV (iCloud): uses the UID directly
fn get_provider_event_id(event: &Event) -> String {
    // Check for Google-specific event ID first
    if let Some((_, google_id)) = event
        .custom_properties
        .iter()
        .find(|(k, _)| k == "X-GOOGLE-EVENT-ID")
    {
        return google_id.clone();
    }
    // Fall back to UID (used by CalDAV providers like iCloud)
    event.uid.clone()
}
