//! Local event file storage.
//!
//! Manages storing and retrieving calendar events as .ics files on disk.

mod create;
mod delete;
mod list;
mod update;

pub use create::{create, expected_filename};
pub use delete::delete;
pub use list::list;
pub use update::update;

use crate::event::Event;
use chrono::{DateTime, Utc};
use std::path::PathBuf;

/// A calendar event stored as a local .ics file.
pub struct LocalEvent {
    /// Path to the .ics file
    pub path: PathBuf,
    /// The event data
    pub event: Event,
    /// File modification time (used for sync direction detection)
    pub modified: Option<DateTime<Utc>>,
}
