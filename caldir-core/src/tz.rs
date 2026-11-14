//! IANA timezones for chrono, backed by jiff's tz database.
//!
//! Reads the system zoneinfo (bundled copy as fallback), so tz rule changes
//! arrive with OS tzdata updates instead of caldir releases.

use chrono::{FixedOffset, LocalResult, NaiveDate, NaiveDateTime, NaiveTime, Offset, TimeZone};
use jiff::tz::{AmbiguousOffset, Dst, TimeZoneDatabase};
use jiff::{Timestamp, civil};
use std::{fmt, str::FromStr};

#[derive(Clone, PartialEq, Eq)]
pub struct Tz(jiff::tz::TimeZone);

impl Tz {
    pub fn get(name: &str) -> Option<Self> {
        db().get(name).ok().map(Tz)
    }

    /// Canonical IANA name, e.g. `Europe/Stockholm`.
    pub fn name(&self) -> &str {
        self.0.iana_name().unwrap_or("UTC")
    }

    /// Offset seconds, DST flag and abbreviation in effect at `utc`.
    pub(crate) fn info_at(&self, utc: &NaiveDateTime) -> (i32, bool, String) {
        let info = self.0.to_offset_info(timestamp(utc));
        (
            info.offset().seconds(),
            info.dst() == Dst::Yes,
            info.abbreviation().to_string(),
        )
    }

    fn offset(&self, offset: jiff::tz::Offset) -> TzOffset {
        TzOffset {
            tz: self.clone(),
            fixed: FixedOffset::east_opt(offset.seconds()).expect("tz offsets are under a day"),
        }
    }
}

fn db() -> &'static TimeZoneDatabase {
    // Tests pin the bundled tzdata so results don't depend on the host
    #[cfg(test)]
    {
        static BUNDLED: std::sync::LazyLock<TimeZoneDatabase> =
            std::sync::LazyLock::new(TimeZoneDatabase::bundled);
        &BUNDLED
    }
    #[cfg(not(test))]
    jiff::tz::db()
}

impl fmt::Debug for Tz {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl fmt::Display for Tz {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Debug, thiserror::Error)]
#[error("unknown timezone `{0}`")]
pub struct UnknownTz(String);

impl FromStr for Tz {
    type Err = UnknownTz;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        Tz::get(name).ok_or_else(|| UnknownTz(name.to_string()))
    }
}

#[derive(Clone, Debug)]
pub struct TzOffset {
    tz: Tz,
    fixed: FixedOffset,
}

impl Offset for TzOffset {
    fn fix(&self) -> FixedOffset {
        self.fixed
    }
}

impl fmt::Display for TzOffset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fixed.fmt(f)
    }
}

impl TimeZone for Tz {
    type Offset = TzOffset;

    fn from_offset(offset: &TzOffset) -> Self {
        offset.tz.clone()
    }

    fn offset_from_local_date(&self, local: &NaiveDate) -> LocalResult<TzOffset> {
        self.offset_from_local_datetime(&local.and_time(NaiveTime::MIN))
    }

    fn offset_from_local_datetime(&self, local: &NaiveDateTime) -> LocalResult<TzOffset> {
        let Some(local) = civil_datetime(local) else {
            return LocalResult::None;
        };
        match self.0.to_ambiguous_timestamp(local).offset() {
            AmbiguousOffset::Unambiguous { offset } => LocalResult::Single(self.offset(offset)),
            AmbiguousOffset::Gap { .. } => LocalResult::None,
            AmbiguousOffset::Fold { before, after } => {
                LocalResult::Ambiguous(self.offset(before), self.offset(after))
            }
        }
    }

    fn offset_from_utc_date(&self, utc: &NaiveDate) -> TzOffset {
        self.offset_from_utc_datetime(&utc.and_time(NaiveTime::MIN))
    }

    fn offset_from_utc_datetime(&self, utc: &NaiveDateTime) -> TzOffset {
        self.offset(self.0.to_offset(timestamp(utc)))
    }
}

fn timestamp(utc: &NaiveDateTime) -> Timestamp {
    let utc = utc.and_utc();
    let nanos = utc.timestamp_subsec_nanos().min(999_999_999) as i32;
    Timestamp::new(utc.timestamp(), nanos).unwrap_or(if utc.timestamp() < 0 {
        Timestamp::MIN
    } else {
        Timestamp::MAX
    })
}

fn civil_datetime(local: &NaiveDateTime) -> Option<civil::DateTime> {
    use chrono::{Datelike, Timelike};
    civil::DateTime::new(
        local.year().try_into().ok()?,
        local.month() as i8,
        local.day() as i8,
        local.hour() as i8,
        local.minute() as i8,
        local.second() as i8,
        local.nanosecond().min(999_999_999) as i32,
    )
    .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn naive(y: i32, m: u32, d: u32, h: u32, min: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(y, m, d)
            .unwrap()
            .and_hms_opt(h, min, 0)
            .unwrap()
    }

    #[test]
    fn resolves_names_case_insensitively_to_canonical() {
        assert_eq!(
            Tz::get("europe/stockholm").unwrap().name(),
            "Europe/Stockholm"
        );
        assert_eq!(Tz::get("US/Pacific").unwrap().name(), "US/Pacific");
        assert!(Tz::get("Bogus/Zone").is_none());
    }

    #[test]
    fn converts_utc_to_local() {
        let tz = Tz::get("Europe/Stockholm").unwrap();
        let local = Utc
            .from_utc_datetime(&naive(2026, 7, 1, 12, 0))
            .with_timezone(&tz);
        assert_eq!(local.naive_local(), naive(2026, 7, 1, 14, 0));
        assert_eq!(local.to_rfc3339(), "2026-07-01T14:00:00+02:00");
    }

    #[test]
    fn local_gap_has_no_offset() {
        let tz = Tz::get("Europe/Stockholm").unwrap();
        assert!(
            tz.from_local_datetime(&naive(2026, 3, 29, 2, 30))
                .single()
                .is_none()
        );
        assert!(
            tz.from_local_datetime(&naive(2026, 3, 29, 2, 30))
                .earliest()
                .is_none()
        );
    }

    #[test]
    fn local_fold_is_ambiguous_earliest_first() {
        let tz = Tz::get("Europe/Stockholm").unwrap();
        let LocalResult::Ambiguous(earliest, latest) =
            tz.from_local_datetime(&naive(2026, 10, 25, 2, 30))
        else {
            panic!("expected a fold");
        };
        assert_eq!(earliest.naive_utc(), naive(2026, 10, 25, 0, 30));
        assert_eq!(latest.naive_utc(), naive(2026, 10, 25, 1, 30));
    }
}
