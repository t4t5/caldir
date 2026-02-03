//! Date range for filtering events.

use chrono::{DateTime, Duration, NaiveDate, Utc};

use crate::constants::DEFAULT_SYNC_DAYS;

/// Date range for filtering events.
/// None values mean unbounded in that direction.
#[derive(Debug, Clone)]
pub struct DateRange {
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
}

impl Default for DateRange {
    /// Default range: ±DEFAULT_SYNC_DAYS from now
    fn default() -> Self {
        let now = Utc::now();
        DateRange {
            from: Some(now - Duration::days(DEFAULT_SYNC_DAYS)),
            to: Some(now + Duration::days(DEFAULT_SYNC_DAYS)),
        }
    }
}

impl DateRange {
    /// Parse a date string into a DateRange.
    /// - `from`: "start" for unbounded, or YYYY-MM-DD
    /// - `to`: YYYY-MM-DD, defaults to +DEFAULT_SYNC_DAYS if not specified
    pub fn from_args(from: Option<&str>, to: Option<&str>) -> Result<Self, String> {
        let now = Utc::now();

        let from_dt = match from {
            Some("start") => None, // Unbounded past
            Some(s) => Some(parse_date_start(s)?),
            None => Some(now - Duration::days(DEFAULT_SYNC_DAYS)),
        };

        let to_dt = match to {
            Some(s) => Some(parse_date_end(s)?),
            None => Some(now + Duration::days(DEFAULT_SYNC_DAYS)),
        };

        Ok(DateRange {
            from: from_dt,
            to: to_dt,
        })
    }

    /// Get `from` as RFC3339 string, using a very old date if unbounded.
    pub fn from_rfc3339(&self) -> String {
        self.from
            .unwrap_or_else(|| DateTime::parse_from_rfc3339("1970-01-01T00:00:00Z").unwrap().into())
            .to_rfc3339()
    }

    /// Get `to` as RFC3339 string, using a far future date if unbounded.
    pub fn to_rfc3339(&self) -> String {
        self.to
            .unwrap_or_else(|| DateTime::parse_from_rfc3339("2100-01-01T00:00:00Z").unwrap().into())
            .to_rfc3339()
    }
}

/// Parse YYYY-MM-DD as start of day in UTC
fn parse_date_start(s: &str) -> Result<DateTime<Utc>, String> {
    let date = NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|_| format!("Invalid date format '{}'. Expected YYYY-MM-DD", s))?;
    Ok(date.and_hms_opt(0, 0, 0).unwrap().and_utc())
}

/// Parse YYYY-MM-DD as end of day in UTC
fn parse_date_end(s: &str) -> Result<DateTime<Utc>, String> {
    let date = NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|_| format!("Invalid date format '{}'. Expected YYYY-MM-DD", s))?;
    Ok(date.and_hms_opt(23, 59, 59).unwrap().and_utc())
}
