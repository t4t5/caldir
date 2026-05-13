//! Date range for filtering events.

use crate::utils::DateBounds;
use chrono::{NaiveDate, NaiveDateTime};

const UNBOUNDED_PAST: &str = "1970-01-01T00:00:00+00:00";
const UNBOUNDED_FUTURE: &str = "2100-01-01T00:00:00+00:00";

#[derive(Debug, Clone)]
pub struct DateRange {
    pub from: Option<NaiveDateTime>,
    pub to: Option<NaiveDateTime>,
}

impl DateRange {
    pub fn from_dates(from: Option<NaiveDate>, to: Option<NaiveDate>) -> Self {
        DateRange {
            from: from.map(|d| d.start_of_date()),
            to: to.map(|d| d.end_of_date()),
        }
    }

    /// RFC3339 `from`, or a sentinel deep-past timestamp when unbounded.
    /// Naive timestamps are interpreted as UTC.
    pub fn from_rfc3339(&self) -> String {
        match self.from {
            Some(dt) => format!("{}+00:00", dt.format("%Y-%m-%dT%H:%M:%S")),
            None => UNBOUNDED_PAST.to_string(),
        }
    }

    /// RFC3339 `to`, or a sentinel deep-future timestamp when unbounded.
    /// Naive timestamps are interpreted as UTC.
    pub fn to_rfc3339(&self) -> String {
        match self.to {
            Some(dt) => format!("{}+00:00", dt.format("%Y-%m-%dT%H:%M:%S")),
            None => UNBOUNDED_FUTURE.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_dates_expands_to_full_day_bounds() {
        let from = NaiveDate::from_ymd_opt(2026, 3, 20).unwrap();
        let to = NaiveDate::from_ymd_opt(2026, 3, 25).unwrap();

        let range = DateRange::from_dates(Some(from), Some(to));

        assert_eq!(range.from, Some(from.and_hms_opt(0, 0, 0).unwrap()));
        assert_eq!(range.to, Some(to.and_hms_opt(23, 59, 59).unwrap()));
    }

    #[test]
    fn from_dates_preserves_none() {
        let range = DateRange::from_dates(None, None);

        assert!(range.from.is_none());
        assert!(range.to.is_none());
    }

    #[test]
    fn rfc3339_uses_bounded_dates_when_set() {
        let range = DateRange::from_dates(
            Some(NaiveDate::from_ymd_opt(2026, 3, 20).unwrap()),
            Some(NaiveDate::from_ymd_opt(2026, 3, 25).unwrap()),
        );

        assert_eq!(range.from_rfc3339(), "2026-03-20T00:00:00+00:00");
        assert_eq!(range.to_rfc3339(), "2026-03-25T23:59:59+00:00");
    }

    #[test]
    fn rfc3339_uses_sentinels_when_unbounded() {
        let range = DateRange::from_dates(None, None);

        assert_eq!(range.from_rfc3339(), "1970-01-01T00:00:00+00:00");
        assert_eq!(range.to_rfc3339(), "2100-01-01T00:00:00+00:00");
    }
}
