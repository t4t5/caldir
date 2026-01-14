use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A calendar returned by the provider's list_calendars command
#[derive(Debug, Serialize, Deserialize)]
pub struct ProviderCalendar {
    pub id: String,
    pub name: String,
    pub primary: bool,
}

/// A calendar paired with the config to save in .caldir/config.toml
#[derive(Debug, Serialize, Deserialize)]
pub struct CalendarWithConfig {
    pub calendar: ProviderCalendar,
    /// Provider-specific config (e.g., google_account, google_calendar_id)
    pub config: HashMap<String, serde_json::Value>,
}
