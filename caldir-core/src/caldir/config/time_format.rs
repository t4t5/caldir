use serde::{Deserialize, Serialize};

/// Time display format: 24-hour ("15:00") or 12-hour ("3:00pm").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TimeFormat {
    #[default]
    #[serde(rename = "24h")]
    H24,
    #[serde(rename = "12h")]
    H12,
}
