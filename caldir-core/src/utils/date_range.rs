//! Date range for filtering events.

use chrono::{NaiveDate, NaiveDateTime};

pub trait DateBounds {
    fn start_of_date(self) -> NaiveDateTime;
    fn end_of_date(self) -> NaiveDateTime;
}

impl DateBounds for NaiveDate {
    fn start_of_date(self) -> NaiveDateTime {
        self.and_hms_opt(0, 0, 0).expect("0:0:0 is always valid")
    }

    fn end_of_date(self) -> NaiveDateTime {
        self.and_hms_opt(23, 59, 59)
            .expect("23:59:59 is always valid")
    }
}

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
}
