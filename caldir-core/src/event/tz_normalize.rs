//! Normalize inbound TZIDs to values understood by IANA timezone consumers.

use super::EventTime;
use chrono::{FixedOffset, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;
use std::{
    collections::HashSet,
    sync::{Mutex, OnceLock},
};
use windows_timezones::WindowsTimezone;

enum Normalized {
    Zoned(String),
    FixedOffset { tzid: String, seconds: i32 },
    Unknown(String),
}

/// Normalize an IANA, Windows, or whole-hour GMT/UTC offset TZID.
///
/// IANA inputs and unknown strings pass through unchanged. Fractional fixed
/// offsets also pass through because normalizing them requires the event's
/// wall-clock value; [`EventTime`] parsing performs that conversion.
pub fn normalize(tzid: String) -> String {
    match classify(tzid) {
        Normalized::Zoned(tzid) | Normalized::FixedOffset { tzid, .. } => tzid,
        Normalized::Unknown(tzid) => {
            warn_unknown(&tzid);
            tzid
        }
    }
}

pub(crate) fn normalize_event_time(datetime: NaiveDateTime, tzid: String) -> EventTime {
    match classify(tzid) {
        Normalized::Zoned(tzid) => EventTime::DateTimeZoned { datetime, tzid },
        Normalized::FixedOffset { seconds, .. } => {
            let offset = FixedOffset::east_opt(seconds).expect("validated fixed offset");
            let datetime = offset
                .from_local_datetime(&datetime)
                .single()
                .expect("fixed offsets have no ambiguous local times")
                .with_timezone(&Utc);
            EventTime::DateTimeUtc(datetime)
        }
        Normalized::Unknown(tzid) => {
            warn_unknown(&tzid);
            EventTime::DateTimeZoned { datetime, tzid }
        }
    }
}

fn classify(tzid: String) -> Normalized {
    if tzid.parse::<Tz>().is_ok() {
        return Normalized::Zoned(tzid);
    }

    if let Ok(windows_timezone) = tzid.parse::<WindowsTimezone>() {
        return Normalized::Zoned(Tz::from(windows_timezone).name().to_string());
    }

    if let Some(seconds) = parse_fixed_offset(&tzid) {
        if seconds % 3600 == 0 {
            let hours = seconds / 3600;
            let iana = match hours.cmp(&0) {
                std::cmp::Ordering::Greater => format!("Etc/GMT-{hours}"),
                std::cmp::Ordering::Less => format!("Etc/GMT+{}", -hours),
                std::cmp::Ordering::Equal => "Etc/GMT".to_string(),
            };
            if iana.parse::<Tz>().is_ok() {
                return Normalized::Zoned(iana);
            }
        }

        return Normalized::FixedOffset { tzid, seconds };
    }

    Normalized::Unknown(tzid)
}

fn parse_fixed_offset(tzid: &str) -> Option<i32> {
    let rest = tzid
        .strip_prefix("GMT")
        .or_else(|| tzid.strip_prefix("UTC"))?;
    let (sign, digits) = match rest.as_bytes().first()? {
        b'+' => (1, &rest[1..]),
        b'-' => (-1, &rest[1..]),
        _ => return None,
    };

    let (hours, minutes) = if let Some((hours, minutes)) = digits.split_once(':') {
        if !(1..=2).contains(&hours.len()) || minutes.len() != 2 {
            return None;
        }
        (parse_digits(hours)?, parse_digits(minutes)?)
    } else {
        match digits.len() {
            1 | 2 => (parse_digits(digits)?, 0),
            4 => (parse_digits(&digits[..2])?, parse_digits(&digits[2..])?),
            _ => return None,
        }
    };

    if hours > 23 || minutes > 59 {
        return None;
    }

    Some(sign * (hours * 3600 + minutes * 60))
}

fn parse_digits(value: &str) -> Option<i32> {
    value
        .bytes()
        .all(|byte| byte.is_ascii_digit())
        .then(|| value.parse().ok())?
}

pub(crate) fn warn_unknown(tzid: &str) {
    static WARNED: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    let mut warned = WARNED
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    if warned.insert(tzid.to_string()) {
        eprintln!("warning: unknown TZID `{tzid}`; treating it as floating local time");
    }
}

/// Map an IANA zone name to its canonical Windows equivalent. `None` for
/// non-IANA inputs or IANA zones with no Windows counterpart.
pub fn from_iana(iana: &str) -> Option<&'static str> {
    let tz: Tz = iana.parse().ok()?;
    let windows_timezone = WindowsTimezone::try_from(tz).ok()?;
    Some(windows_timezone.name())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, NaiveDate};
    use pretty_assertions::assert_eq;

    #[test]
    fn normalize_maps_supported_tzids() {
        for unchanged in ["Europe/Stockholm", "UTC", "GMT", "CET", "EST", "US/Pacific"] {
            assert_eq!(normalize(unchanged.to_string()), unchanged);
        }

        assert_eq!(
            normalize("W. Europe Standard Time".to_string()),
            "Europe/Berlin"
        );

        for input in ["GMT+0100", "GMT+01:00", "UTC+0100", "GMT+1"] {
            assert_eq!(normalize(input.to_string()), "Etc/GMT-1", "{input}");
        }
        assert_eq!(normalize("GMT-0500".to_string()), "Etc/GMT+5");
        assert_eq!(normalize("GMT+0000".to_string()), "Etc/GMT");
    }

    #[test]
    fn normalize_passes_fractional_malformed_and_unknown_tzids_through() {
        for unchanged in [
            "GMT+0530",
            "GMT+9900",
            "GMT+",
            "GMT+01:0x",
            "/mozilla.org/20070129_1/Europe/Berlin",
            "PST",
            "AEST",
            "tzone://Microsoft/Custom",
            "",
        ] {
            assert_eq!(normalize(unchanged.to_string()), unchanged);
        }
    }

    #[test]
    fn fractional_offset_event_becomes_utc() {
        let datetime = NaiveDate::from_ymd_opt(2026, 7, 24)
            .unwrap()
            .and_hms_opt(19, 2, 0)
            .unwrap();

        assert_eq!(
            normalize_event_time(datetime, "GMT+0530".to_string()),
            EventTime::DateTimeUtc(
                DateTime::parse_from_rfc3339("2026-07-24T13:32:00Z")
                    .unwrap()
                    .with_timezone(&Utc)
            )
        );
    }

    #[test]
    fn from_iana_maps_known_iana_to_windows() {
        assert_eq!(
            from_iana("America/Sao_Paulo"),
            Some("E. South America Standard Time")
        );
        assert_eq!(
            from_iana("America/Los_Angeles"),
            Some("Pacific Standard Time")
        );
        assert_eq!(from_iana("Europe/Berlin"), Some("W. Europe Standard Time"));
        assert_eq!(from_iana("Europe/London"), Some("GMT Standard Time"));
    }

    #[test]
    fn from_iana_maps_fixed_offset_to_a_sane_windows_zone() {
        assert_eq!(
            from_iana("Etc/GMT-1"),
            Some("W. Central Africa Standard Time")
        );
    }

    #[test]
    fn windows_then_iana_round_trips_for_canonical_pairs() {
        for original in [
            "E. South America Standard Time",
            "Pacific Standard Time",
            "W. Europe Standard Time",
            "GMT Standard Time",
            "Tokyo Standard Time",
            "AUS Eastern Standard Time",
        ] {
            let iana = normalize(original.to_string());
            assert_eq!(
                from_iana(&iana),
                Some(original),
                "round-trip lost identity for {original}"
            );
        }
    }
}
