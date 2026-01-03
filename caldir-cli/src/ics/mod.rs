//! ICS file generation and parsing.
//!
//! This module handles reading and writing .ics files according to RFC 5545.

mod generate;
mod parse;

pub use generate::{generate_filename, generate_ics};
pub use parse::parse_event;

/// Metadata about the calendar source (embedded in .ics files)
#[derive(Debug, Clone)]
pub struct CalendarMetadata {
    /// Calendar ID (e.g., "user@gmail.com")
    pub calendar_id: String,
    /// Human-readable calendar name (e.g., "Personal Calendar")
    pub calendar_name: String,
}
