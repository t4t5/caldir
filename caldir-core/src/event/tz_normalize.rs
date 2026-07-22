//! Normalize inbound TZIDs to values understood by IANA timezone consumers.

use chrono_tz::Tz;
use std::{
    collections::HashSet,
    sync::{Mutex, OnceLock},
};
use windows_timezones::WindowsTimezone;

pub(crate) enum Tzid {
    Iana(String),     // e.g. "Europe/London"
    FixedOffset(i32), // e.g. "GMT+0100"
    Unknown,
}

pub(crate) fn classify(tzid: &str) -> Tzid {
    if tzid.parse::<Tz>().is_ok() {
        return Tzid::Iana(tzid.to_string());
    }

    if let Ok(windows_timezone) = tzid.parse::<WindowsTimezone>() {
        return Tzid::Iana(Tz::from(windows_timezone).name().to_string());
    }

    if let Some(seconds) = parse_fixed_offset(tzid) {
        return offset_tzid(seconds);
    }

    // RFC 5545 §3.2.19 global-registry form, e.g.
    // `/mozilla.org/20070129_1/Europe/Berlin`: retry the ladder on each
    // path suffix.
    if let Some(rest) = tzid.strip_prefix('/') {
        let segments: Vec<&str> = rest.split('/').collect();
        for start in 0..segments.len() {
            match classify(&segments[start..].join("/")) {
                Tzid::Unknown => continue,
                resolved => return resolved,
            }
        }
    }

    // Windows display names, e.g. `(UTC+01:00) Amsterdam, Berlin, …`:
    // retry the ladder on the parenthesized prefix.
    if let Some((inner, _)) = tzid.strip_prefix('(').and_then(|rest| rest.split_once(')')) {
        match classify(inner) {
            Tzid::Unknown => {}
            resolved => return resolved,
        }
    }

    Tzid::Unknown
}

fn offset_tzid(seconds: i32) -> Tzid {
    if seconds % 3600 == 0 {
        let hours = seconds / 3600;
        // Etc/GMT zones have inverted signs: GMT+1 wall clock = Etc/GMT-1.
        let iana = if hours == 0 {
            "Etc/GMT".to_string()
        } else {
            format!("Etc/GMT{:+}", -hours)
        };
        // Etc/GMT±N only exists for -12..=+14; anything else falls
        // through to the fixed-offset path.
        if iana.parse::<Tz>().is_ok() {
            return Tzid::Iana(iana);
        }
    }

    Tzid::FixedOffset(seconds)
}

/// Normalize an IANA, Windows, whole-hour GMT/UTC offset,
/// global-registry (`/vendor/…`), or display-name (`(UTC+01:00) …`) TZID.
///
/// IANA inputs and unknown strings pass through unchanged.
/// Fractional fixed offsets also pass through,
/// because normalizing them requires the event's wall-clock value
/// [`super::EventTime`] parsing performs that conversion.
pub fn normalize(tzid: String) -> String {
    match classify(&tzid) {
        Tzid::Iana(iana) => iana,
        Tzid::FixedOffset(_) => tzid,
        Tzid::Unknown => {
            warn_unknown(&tzid);
            tzid
        }
    }
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
    use pretty_assertions::assert_eq;

    #[test]
    fn normalize_matrix() {
        for (input, expected) in [
            // IANA and tzdb literals pass through
            ("Europe/Stockholm", "Europe/Stockholm"),
            ("UTC", "UTC"),
            ("GMT", "GMT"),
            ("CET", "CET"),
            ("EST", "EST"),
            ("US/Pacific", "US/Pacific"),
            // Windows names map to IANA
            ("W. Europe Standard Time", "Europe/Berlin"),
            // whole-hour offsets map to Etc/GMT zones (inverted sign!)
            ("GMT+0100", "Etc/GMT-1"),
            ("GMT+01:00", "Etc/GMT-1"),
            ("UTC+0100", "Etc/GMT-1"),
            ("GMT+1", "Etc/GMT-1"),
            ("GMT-0500", "Etc/GMT+5"),
            ("GMT+0000", "Etc/GMT"),
            ("GMT+1300", "Etc/GMT-13"),
            // valid offsets with no Etc zone: resolved at EventTime parse
            ("GMT+0530", "GMT+0530"),
            ("GMT-1300", "GMT-1300"),
            // global-registry prefixes resolve to their IANA suffix
            ("/mozilla.org/20070129_1/Europe/Berlin", "Europe/Berlin"),
            (
                "/freeassociation.sourceforge.net/America/New_York",
                "America/New_York",
            ),
            ("/citadel.org/20211207_1/UTC", "UTC"),
            // Windows display names resolve via the parenthesized prefix
            (
                "(UTC+01:00) Amsterdam, Berlin, Bern, Rome, Stockholm, Vienna",
                "Etc/GMT-1",
            ),
            ("(UTC-05:00) Eastern Time (US & Canada)", "Etc/GMT+5"),
            ("(UTC) Coordinated Universal Time", "UTC"),
            // fractional display-name offset: resolved at EventTime parse
            (
                "(UTC+05:30) Chennai, Kolkata, Mumbai, New Delhi",
                "(UTC+05:30) Chennai, Kolkata, Mumbai, New Delhi",
            ),
            // malformed and unknown pass through
            ("GMT+9900", "GMT+9900"),
            ("GMT+", "GMT+"),
            ("GMT+01:0x", "GMT+01:0x"),
            ("/not/a/zone", "/not/a/zone"),
            ("/", "/"),
            ("(no offset here) Some Place", "(no offset here) Some Place"),
            ("PST", "PST"),
            ("AEST", "AEST"),
            ("tzone://Microsoft/Custom", "tzone://Microsoft/Custom"),
            ("", ""),
        ] {
            assert_eq!(normalize(input.to_string()), expected, "{input}");
        }
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
