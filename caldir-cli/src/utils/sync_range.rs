use crate::utils::parse_date;
use anyhow::{Context, Result};
use caldir_core::{DateBounds, DateRange};
use chrono::{Duration, Local, TimeZone};

const DEFAULT_PAST_DAYS: i64 = 365;
const DEFAULT_FUTURE_DAYS: i64 = 365;

pub fn resolve_sync_range(from: Option<String>, to: Option<String>) -> Result<DateRange> {
    let tz = Local;
    let today = Local::now().date_naive();

    let from_date = match from {
        Some(s) => parse_date(&s).with_context(|| format!("invalid --from date: {s}"))?,
        None => today - Duration::days(DEFAULT_PAST_DAYS),
    };

    let to_date = match to {
        Some(s) => parse_date(&s).with_context(|| format!("invalid --to date: {s}"))?,
        None => today + Duration::days(DEFAULT_FUTURE_DAYS),
    };

    let from_utc = tz
        .from_local_datetime(&from_date.start_of_date())
        .earliest()
        .with_context(|| format!("ambiguous local time for {from_date}"))?
        .to_utc();
    let to_utc = tz
        .from_local_datetime(&to_date.end_of_date())
        .latest()
        .with_context(|| format!("ambiguous local time for {to_date}"))?
        .to_utc();

    Ok(DateRange {
        from: Some(from_utc),
        to: Some(to_utc),
    })
}
