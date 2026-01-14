use serde::{Deserialize, Serialize};

/// A calendar returned by the provider's list_calendars command
#[derive(Debug, Serialize, Deserialize)]
pub struct ProviderCalendar {
    pub id: String,
    pub name: String,
    pub primary: bool,
}
