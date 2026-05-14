use crate::utils::parse_date;
use anyhow::{Context, Result};
use caldir_core::{DateBounds, DateRange};
use chrono::{Duration, Local, TimeZone};

const DEFAULT_PAST_DAYS: i64 = 365;
const DEFAULT_FUTURE_DAYS: i64 = 365;

pub fn resolve_sync_range(from: Option<String>, to: Option<String>) -> Result<DateRange> {
    let tz = Local;
    let today = Local::now().date_naive();

    let from_utc = match from.as_deref() {
        Some("start") => None,
        Some(s) => {
            let date = parse_date(s).with_context(|| format!("invalid --from date: {s}"))?;
            Some(
                tz.from_local_datetime(&date.start_of_date())
                    .earliest()
                    .with_context(|| format!("ambiguous local time for {date}"))?
                    .to_utc(),
            )
        }
        None => {
            let date = today - Duration::days(DEFAULT_PAST_DAYS);
            Some(
                tz.from_local_datetime(&date.start_of_date())
                    .earliest()
                    .with_context(|| format!("ambiguous local time for {date}"))?
                    .to_utc(),
            )
        }
    };

    let to_date = match to {
        Some(s) => parse_date(&s).with_context(|| format!("invalid --to date: {s}"))?,
        None => today + Duration::days(DEFAULT_FUTURE_DAYS),
    };
    let to_utc = tz
        .from_local_datetime(&to_date.end_of_date())
        .latest()
        .with_context(|| format!("ambiguous local time for {to_date}"))?
        .to_utc();

    Ok(DateRange {
        from: from_utc,
        to: Some(to_utc),
    })
}
