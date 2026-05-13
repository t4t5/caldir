use anyhow::Result;
use caldir_core::Caldir;
use caldir_core::DateBounds;
use chrono::{DateTime, TimeZone, Utc};

use crate::render::events_in_range::render_events_in_range;
use crate::utils::{require_calendars, resolve_calendars};

pub fn run(caldir: &Caldir, calendar: Option<String>) -> Result<()> {
    require_calendars(caldir)?;

    let calendars = resolve_calendars(caldir, calendar.as_deref())?;

    let tz: chrono_tz::Tz = iana_time_zone::get_timezone()?.parse()?;

    let (from, to) = day_range(Utc::now().with_timezone(&tz));

    render_events_in_range(caldir, calendars, from, to)
}

fn day_range<Tz: TimeZone>(now: DateTime<Tz>) -> (DateTime<Utc>, DateTime<Utc>) {
    let tz = now.timezone();
    let today = now.date_naive();

    let start = today
        .start_of_date()
        .and_local_timezone(tz.clone())
        .earliest()
        .unwrap()
        .with_timezone(&Utc);

    let end = today
        .end_of_date()
        .and_local_timezone(tz)
        .latest()
        .unwrap()
        .with_timezone(&Utc);

    (start, end)
}
