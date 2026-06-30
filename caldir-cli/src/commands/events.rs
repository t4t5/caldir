use anyhow::{Context, Result};
use caldir_core::Caldir;
use caldir_core::DateBounds;
use chrono::{DateTime, Duration, TimeZone, Utc};

use crate::render::events_in_range::render_text_events_in_range;
use crate::utils::parse_date;
use crate::utils::{require_calendars, resolve_calendars};

pub fn run(
    caldir: &Caldir,
    calendar: Option<String>,
    from: Option<String>,
    to: Option<String>,
) -> Result<()> {
    require_calendars(caldir)?;

    let calendars = resolve_calendars(caldir, calendar.as_deref())?;

    let tz: chrono_tz::Tz = iana_time_zone::get_timezone()?.parse()?;

    let (from, to) = resolve_range(
        Utc::now().with_timezone(&tz),
        from.as_deref(),
        to.as_deref(),
    )?;

    render_text_events_in_range(caldir, calendars, from, to)
}

fn resolve_range<Tz: TimeZone>(
    now: DateTime<Tz>,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<(DateTime<Utc>, DateTime<Utc>)> {
    let tz = now.timezone();

    let today = now.date_naive();

    let from_date = match from {
        Some(s) => parse_date(s).with_context(|| format!("invalid --from date: {s}"))?,
        None => today,
    };
    let to_date = match to {
        Some(s) => parse_date(s).with_context(|| format!("invalid --to date: {s}"))?,
        None => today + Duration::days(2),
    };

    let start = from_date
        .start_of_date()
        .and_local_timezone(tz.clone())
        .earliest()
        .unwrap()
        .with_timezone(&Utc);

    let end = to_date
        .end_of_date()
        .and_local_timezone(tz)
        .latest()
        .unwrap()
        .with_timezone(&Utc);

    Ok((start, end))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use chrono_tz::Europe::Stockholm;

    fn stockholm_date(d: DateTime<Utc>) -> NaiveDate {
        d.with_timezone(&Stockholm).date_naive()
    }

    fn stockholm_time(d: DateTime<Utc>) -> chrono::NaiveTime {
        d.with_timezone(&Stockholm).time()
    }

    #[test]
    fn defaults_to_three_day_window_starting_today() {
        let now = Stockholm.with_ymd_and_hms(2026, 5, 13, 12, 0, 0).unwrap();
        let (from, to) = resolve_range(now, None, None).unwrap();

        assert_eq!(
            stockholm_date(from),
            NaiveDate::from_ymd_opt(2026, 5, 13).unwrap(),
        );
        assert_eq!(
            stockholm_date(to),
            NaiveDate::from_ymd_opt(2026, 5, 15).unwrap(),
        );
        assert_eq!(
            stockholm_time(from),
            chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
        );
        assert_eq!(
            stockholm_time(to),
            chrono::NaiveTime::from_hms_opt(23, 59, 59).unwrap(),
        );
    }

    #[test]
    fn uses_provided_from_and_to() {
        let now = Stockholm.with_ymd_and_hms(2026, 5, 13, 12, 0, 0).unwrap();
        let (from, to) = resolve_range(now, Some("2026-06-01"), Some("2026-06-10")).unwrap();

        assert_eq!(
            stockholm_date(from),
            NaiveDate::from_ymd_opt(2026, 6, 1).unwrap(),
        );
        assert_eq!(
            stockholm_date(to),
            NaiveDate::from_ymd_opt(2026, 6, 10).unwrap(),
        );
    }

    #[test]
    fn provided_from_with_default_to() {
        let now = Stockholm.with_ymd_and_hms(2026, 5, 13, 12, 0, 0).unwrap();
        let (from, to) = resolve_range(now, Some("2026-04-01"), None).unwrap();

        assert_eq!(
            stockholm_date(from),
            NaiveDate::from_ymd_opt(2026, 4, 1).unwrap(),
        );
        assert_eq!(
            stockholm_date(to),
            NaiveDate::from_ymd_opt(2026, 5, 15).unwrap(),
            "to should still default to today + 2 days",
        );
    }

    #[test]
    fn default_from_with_provided_to() {
        let now = Stockholm.with_ymd_and_hms(2026, 5, 13, 12, 0, 0).unwrap();
        let (from, to) = resolve_range(now, None, Some("2026-07-01")).unwrap();

        assert_eq!(
            stockholm_date(from),
            NaiveDate::from_ymd_opt(2026, 5, 13).unwrap(),
            "from should still default to today",
        );
        assert_eq!(
            stockholm_date(to),
            NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
        );
    }

    #[test]
    fn invalid_from_returns_error() {
        let now = Stockholm.with_ymd_and_hms(2026, 5, 13, 12, 0, 0).unwrap();
        let err = resolve_range(now, Some("not-a-date"), None).unwrap_err();
        assert!(err.to_string().contains("--from"));
    }

    #[test]
    fn invalid_to_returns_error() {
        let now = Stockholm.with_ymd_and_hms(2026, 5, 13, 12, 0, 0).unwrap();
        let err = resolve_range(now, None, Some("2026/07/01")).unwrap_err();
        assert!(err.to_string().contains("--to"));
    }
}
