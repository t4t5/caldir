use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A calendar paired with the config to save in .caldir/config.toml
#[derive(Debug, Serialize, Deserialize)]
pub struct CalendarConfig {
    pub name: String,
    /// Provider-specific config (e.g., google_account, google_calendar_id)
    pub config: HashMap<String, serde_json::Value>,
}
