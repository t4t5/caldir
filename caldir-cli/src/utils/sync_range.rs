use crate::utils::parse_date;
use anyhow::{Context, Result};
use caldir_core::{DateBounds, DateRange};
use chrono::{Duration, Local};

const DEFAULT_PAST_DAYS: i64 = 365;
const DEFAULT_FUTURE_DAYS: i64 = 365;

pub fn resolve_sync_range(from: Option<String>, to: Option<String>) -> Result<DateRange> {
    let today = Local::now().date_naive();

    let from_date = match from {
        Some(s) => parse_date(&s).with_context(|| format!("invalid --from date: {s}"))?,
        None => today - Duration::days(DEFAULT_PAST_DAYS),
    };

    let to_date = match to {
        Some(s) => parse_date(&s).with_context(|| format!("invalid --to date: {s}"))?,
        None => today + Duration::days(DEFAULT_FUTURE_DAYS),
    };

    Ok(DateRange {
        from: Some(from_date.start_of_date()),
        to: Some(to_date.end_of_date()),
    })
}
