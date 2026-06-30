use anyhow::Result;
use caldir_core::Caldir;
use caldir_core::DateBounds;
use chrono::{DateTime, Datelike, Duration, TimeZone, Utc};

use crate::render::events_in_range::render_text_events_in_range;
use crate::utils::{require_calendars, resolve_calendars};

pub fn run(caldir: &Caldir, calendar: Option<String>) -> Result<()> {
    require_calendars(caldir)?;

    let calendars = resolve_calendars(caldir, calendar.as_deref())?;

    let tz: chrono_tz::Tz = iana_time_zone::get_timezone()?.parse()?;
    let (from, to) = week_range(Utc::now().with_timezone(&tz));

    render_text_events_in_range(caldir, calendars, from, to)
}

fn week_range<Tz: TimeZone>(now: DateTime<Tz>) -> (DateTime<Utc>, DateTime<Utc>) {
    let tz = now.timezone();
    let today = now.date_naive();
    let weekday = today.weekday().num_days_from_monday();

    // If today is Sunday, jump to the upcoming Mon–Sun week rather than showing a single day.
    let (start_date, end_date) = if weekday == 6 {
        (today + Duration::days(1), today + Duration::days(7))
    } else {
        let days_until_sunday = 6 - weekday;
        (today, today + Duration::days(days_until_sunday as i64))
    };

    let start = start_date
        .start_of_date()
        .and_local_timezone(tz.clone())
        .earliest()
        .unwrap()
        .with_timezone(&Utc);

    let end = end_date
        .end_of_date()
        .and_local_timezone(tz)
        .latest()
        .unwrap()
        .with_timezone(&Utc);

    (start, end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use chrono_tz::Europe::Stockholm;

    fn stockholm_date(d: DateTime<Utc>) -> NaiveDate {
        d.with_timezone(&Stockholm).date_naive()
    }

    #[test]
    fn tuesday_shows_tuesday_through_sunday() {
        let now = Stockholm.with_ymd_and_hms(2026, 5, 12, 12, 0, 0).unwrap();
        let (from, to) = week_range(now);

        assert_eq!(
            stockholm_date(from),
            NaiveDate::from_ymd_opt(2026, 5, 12).unwrap(),
            "start should be Tuesday May 12 in local time",
        );
        assert_eq!(
            stockholm_date(to),
            NaiveDate::from_ymd_opt(2026, 5, 17).unwrap(),
            "end should be Sunday May 17 in local time",
        );
        assert_eq!(
            from.with_timezone(&Stockholm).time(),
            chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
            "start should be midnight local time",
        );
    }

    #[test]
    fn sunday_shows_following_monday_through_sunday() {
        let now = Stockholm.with_ymd_and_hms(2026, 5, 17, 12, 0, 0).unwrap();
        let (from, to) = week_range(now);

        assert_eq!(
            stockholm_date(from),
            NaiveDate::from_ymd_opt(2026, 5, 18).unwrap(),
            "on Sunday, start should jump to next Monday May 18",
        );
        assert_eq!(
            stockholm_date(to),
            NaiveDate::from_ymd_opt(2026, 5, 24).unwrap(),
            "end should be the following Sunday May 24",
        );
        assert_eq!(
            from.with_timezone(&Stockholm).time(),
            chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
            "start should be midnight local time",
        );
    }
}
