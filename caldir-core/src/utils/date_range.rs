//! Date range for filtering events.

use chrono::{DateTime, Duration, Local, NaiveDate, TimeZone, Utc};

const UNBOUNDED_PAST: &str = "1970-01-01T00:00:00+00:00";
const UNBOUNDED_FUTURE: &str = "2100-01-01T00:00:00+00:00";
const DEFAULT_SYNC_PAST_DAYS: i64 = 365;
const DEFAULT_SYNC_FUTURE_DAYS: i64 = 365;

#[derive(Debug, Clone, Default)]
pub struct DateRange {
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
}

impl DateRange {
    // +/- 1 year
    pub fn default_sync_window() -> Self {
        Self::sync_window_at(Local::now().date_naive(), Local)
    }

    pub fn sync_window_at<Tz: TimeZone>(today: NaiveDate, tz: Tz) -> Self {
        Self {
            from: Some(start_of_day_utc(
                &tz,
                today - Duration::days(DEFAULT_SYNC_PAST_DAYS),
            )),
            to: Some(end_of_day_utc(
                &tz,
                today + Duration::days(DEFAULT_SYNC_FUTURE_DAYS),
            )),
        }
    }

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

fn start_of_day_utc<Tz: TimeZone>(tz: &Tz, date: NaiveDate) -> DateTime<Utc> {
    let local = date
        .and_hms_opt(0, 0, 0)
        .expect("midnight should be a valid NaiveDateTime");

    tz.from_local_datetime(&local)
        .earliest()
        .map(|dt| dt.with_timezone(&Utc))
        .expect("midnight should resolve in supported timezones")
}

fn end_of_day_utc<Tz: TimeZone>(tz: &Tz, date: NaiveDate) -> DateTime<Utc> {
    let local = date
        .and_hms_opt(23, 59, 59)
        .expect("end of day should be a valid NaiveDateTime");

    tz.from_local_datetime(&local)
        .latest()
        .map(|dt| dt.with_timezone(&Utc))
        .expect("end of day should resolve in supported timezones")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, NaiveTime, TimeZone};
    use chrono_tz::Europe::Stockholm;

    fn utc(y: i32, m: u32, d: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, 0, 0, 0).unwrap()
    }

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn range_in(
        tz: chrono_tz::Tz,
        range: &DateRange,
    ) -> (Option<(NaiveDate, NaiveTime)>, (NaiveDate, NaiveTime)) {
        let from = range.from.map(|dt| {
            let local = dt.with_timezone(&tz);
            (local.date_naive(), local.time())
        });
        let to_local = range.to.unwrap().with_timezone(&tz);
        (from, (to_local.date_naive(), to_local.time()))
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

    #[test]
    fn default_sync_window_is_one_year_back_and_forward_in_local_days() {
        let range = DateRange::sync_window_at(date(2026, 5, 14), Stockholm);

        let (from, to) = range_in(Stockholm, &range);
        assert_eq!(
            from,
            Some((date(2025, 5, 14), NaiveTime::from_hms_opt(0, 0, 0).unwrap())),
        );
        assert_eq!(
            to,
            (
                date(2027, 5, 14),
                NaiveTime::from_hms_opt(23, 59, 59).unwrap()
            ),
        );
    }
}
