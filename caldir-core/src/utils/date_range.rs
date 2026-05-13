//! Date range for filtering events.

use chrono::{DateTime, Utc};

const UNBOUNDED_PAST: &str = "1970-01-01T00:00:00+00:00";
const UNBOUNDED_FUTURE: &str = "2100-01-01T00:00:00+00:00";

#[derive(Debug, Clone, Default)]
pub struct DateRange {
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
}

impl DateRange {
    /// RFC3339 `(from, to)`, substituting sentinel deep-past/deep-future timestamps when unbounded.
    pub fn to_rfc3339(&self) -> (String, String) {
        let from = match self.from {
            Some(dt) => dt.to_rfc3339(),
            None => UNBOUNDED_PAST.to_string(),
        };
        let to = match self.to {
            Some(dt) => dt.to_rfc3339(),
            None => UNBOUNDED_FUTURE.to_string(),
        };
        (from, to)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn utc(y: i32, m: u32, d: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, 0, 0, 0).unwrap()
    }

    #[test]
    fn rfc3339_uses_bounded_dates_when_set() {
        let range = DateRange {
            from: Some(utc(2026, 3, 20)),
            to: Some(utc(2026, 3, 25)),
        };

        assert_eq!(
            range.to_rfc3339(),
            (
                "2026-03-20T00:00:00+00:00".to_string(),
                "2026-03-25T00:00:00+00:00".to_string(),
            )
        );
    }

    #[test]
    fn rfc3339_uses_sentinels_when_unbounded() {
        let range = DateRange {
            from: None,
            to: None,
        };

        assert_eq!(
            range.to_rfc3339(),
            (
                "1970-01-01T00:00:00+00:00".to_string(),
                "2100-01-01T00:00:00+00:00".to_string(),
            )
        );
    }
}
