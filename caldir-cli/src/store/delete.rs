//! Delete event files from a calendar directory.

use super::LocalEvent;
use anyhow::{Context, Result};

/// Delete an event file from the calendar directory.
pub fn delete(local_event: &LocalEvent) -> Result<()> {
    std::fs::remove_file(&local_event.path)
        .with_context(|| format!("Failed to delete {}", local_event.path.display()))?;
    Ok(())
}
