use crate::utils::parse_date;
use anyhow::{Context, Result};
use caldir_core::{DateBounds, DateRange};
use chrono::{DateTime, Duration, Local, NaiveDate, TimeZone};

const DEFAULT_PAST_DAYS: i64 = 365;
const DEFAULT_FUTURE_DAYS: i64 = 365;

pub fn resolve_sync_range(from: Option<String>, to: Option<String>) -> Result<DateRange> {
    resolve_sync_range_at(from, to, Local::now().date_naive(), Local)
}

/// Inner implementation so tests can pin `today` (and timezone) instead of
/// reading the wall clock.
fn resolve_sync_range_at<Tz: TimeZone>(
    from: Option<String>,
    to: Option<String>,
    today: NaiveDate,
    tz: Tz,
) -> Result<DateRange> {
    if from.is_none() && to.is_none() {
        return Ok(DateRange::sync_window_at(today, tz));
    }

    let from_utc = match from.as_deref() {
        Some("start") => None,
        Some(s) => Some(start_of_day_utc(
            &tz,
            parse_date(s).with_context(|| format!("invalid --from date: {s}"))?,
        )?),
        None => Some(start_of_day_utc(
            &tz,
            today - Duration::days(DEFAULT_PAST_DAYS),
        )?),
    };

    let to_date = match to {
        Some(s) => parse_date(&s).with_context(|| format!("invalid --to date: {s}"))?,
        None => today + Duration::days(DEFAULT_FUTURE_DAYS),
    };
    let to_utc = end_of_day_utc(&tz, to_date)?;

    Ok(DateRange {
        from: from_utc,
        to: Some(to_utc),
    })
}

fn start_of_day_utc<Tz: TimeZone>(tz: &Tz, date: NaiveDate) -> Result<DateTime<chrono::Utc>> {
    tz.from_local_datetime(&date.start_of_date())
        .earliest()
        .map(|dt| dt.to_utc())
        .with_context(|| format!("ambiguous local time for {date}"))
}

fn end_of_day_utc<Tz: TimeZone>(tz: &Tz, date: NaiveDate) -> Result<DateTime<chrono::Utc>> {
    tz.from_local_datetime(&date.end_of_date())
        .latest()
        .map(|dt| dt.to_utc())
        .with_context(|| format!("ambiguous local time for {date}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveTime;
    use chrono_tz::Europe::Stockholm;

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
    fn defaults_to_plus_minus_365_days_from_today() {
        let today = date(2026, 5, 14);
        let range = resolve_sync_range_at(None, None, today, Stockholm).unwrap();

        let (from, to) = range_in(Stockholm, &range);
        assert_eq!(
            from,
            Some((date(2025, 5, 14), NaiveTime::from_hms_opt(0, 0, 0).unwrap())),
            "from should be midnight 365 days before today, local time",
        );
        assert_eq!(
            to,
            (
                date(2027, 5, 14),
                NaiveTime::from_hms_opt(23, 59, 59).unwrap()
            ),
            "to should be end-of-day 365 days after today, local time",
        );
    }

    #[test]
    fn from_start_means_unbounded_past() {
        let today = date(2026, 5, 14);
        let range = resolve_sync_range_at(Some("start".into()), None, today, Stockholm).unwrap();

        assert!(
            range.from.is_none(),
            "from=start should yield unbounded past"
        );
        assert!(range.to.is_some(), "to should still default to today+365");
    }

    #[test]
    fn explicit_dates_are_passed_through() {
        let today = date(2026, 5, 14);
        let range = resolve_sync_range_at(
            Some("2026-01-01".into()),
            Some("2026-01-31".into()),
            today,
            Stockholm,
        )
        .unwrap();

        let (from, to) = range_in(Stockholm, &range);
        assert_eq!(
            from,
            Some((date(2026, 1, 1), NaiveTime::from_hms_opt(0, 0, 0).unwrap())),
        );
        assert_eq!(
            to,
            (
                date(2026, 1, 31),
                NaiveTime::from_hms_opt(23, 59, 59).unwrap()
            ),
        );
    }

    #[test]
    fn mixing_explicit_from_with_default_to_keeps_default_future() {
        let today = date(2026, 5, 14);
        let range =
            resolve_sync_range_at(Some("2024-06-01".into()), None, today, Stockholm).unwrap();

        let (from, to) = range_in(Stockholm, &range);
        assert_eq!(from.unwrap().0, date(2024, 6, 1));
        assert_eq!(
            to.0,
            date(2027, 5, 14),
            "to should still default to today+365"
        );
    }

    #[test]
    fn errors_on_invalid_from_date() {
        let err = resolve_sync_range_at(
            Some("not-a-date".into()),
            None,
            date(2026, 5, 14),
            Stockholm,
        )
        .unwrap_err();
        assert!(err.to_string().contains("invalid --from date"));
    }

    #[test]
    fn errors_on_invalid_to_date() {
        let err = resolve_sync_range_at(
            None,
            Some("not-a-date".into()),
            date(2026, 5, 14),
            Stockholm,
        )
        .unwrap_err();
        assert!(err.to_string().contains("invalid --to date"));
    }
}
