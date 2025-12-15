pub mod gcal;

use anyhow::Result;

/// A calendar event from a provider
#[derive(Debug, Clone)]
pub struct Event {
    /// Provider-specific event ID (used as UID in .ics)
    pub id: String,
    /// Event title/summary
    pub summary: String,
    /// Event description (optional)
    pub description: Option<String>,
    /// Event location (optional)
    pub location: Option<String>,
    /// Start time (None for all-day events, use start_date instead)
    pub start_time: Option<chrono::DateTime<chrono::Utc>>,
    /// End time (None for all-day events, use end_date instead)
    pub end_time: Option<chrono::DateTime<chrono::Utc>>,
    /// Start date (for all-day events)
    pub start_date: Option<chrono::NaiveDate>,
    /// End date (for all-day events)
    pub end_date: Option<chrono::NaiveDate>,
    /// Whether the event is an all-day event
    pub all_day: bool,
    /// Event status
    pub status: EventStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EventStatus {
    Confirmed,
    Tentative,
    Cancelled,
}

/// Result of a sync operation
pub struct SyncResult {
    /// Events that were fetched (new or updated)
    pub events: Vec<Event>,
    /// IDs of events that were deleted
    pub deleted_ids: Vec<String>,
}
