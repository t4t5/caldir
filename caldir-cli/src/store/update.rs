//! Update event files in a calendar directory.

use super::LocalEvent;
use crate::event::Event;
use crate::ics::CalendarMetadata;
use anyhow::Result;
use std::path::Path;

/// Update an existing event file.
///
/// Deletes the old file and creates a new one with the updated content.
/// The filename may change if the event's date/time or title changed.
///
/// Returns the updated LocalEvent with the new path.
pub fn update(
    dir: &Path,
    old: &LocalEvent,
    new_event: &Event,
    metadata: &CalendarMetadata,
) -> Result<LocalEvent> {
    // Delete the old file
    super::delete::delete(old)?;

    // Create the new file (generates ICS content internally)
    super::create::create(dir, new_event, metadata)
}
