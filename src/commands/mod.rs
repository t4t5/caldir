pub mod auth;
pub mod new;
pub mod pull;
pub mod push;
pub mod status;

use std::collections::HashMap;

/// Number of days to sync in each direction (past and future)
pub const SYNC_DAYS: i64 = 365;

/// Extract calendar_id from provider params, looking for {provider}_calendar_id
pub fn get_calendar_id(params: &HashMap<String, toml::Value>, fallback: &str) -> String {
    for (key, value) in params {
        if key.ends_with("_calendar_id")
            && let toml::Value::String(s) = value
        {
            return s.clone();
        }
    }
    fallback.to_string()
}
