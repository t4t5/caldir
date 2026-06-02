//! Map between Microsoft Windows time zone names and IANA names via CLDR.
//!
//! Outlook and Microsoft Graph emit TZIDs like `E. South America Standard
//! Time` that `chrono_tz` and `rrule` don't understand. Backed by the
//! `windows-timezones` crate.

use chrono_tz::Tz;
use windows_timezones::WindowsTimezone;

/// Map a Windows zone name to IANA. IANA inputs and unknown strings pass
/// through unchanged. The IANA passthrough avoids rewrites like `UTC` →
/// `Etc/UTC` that would create phantom diffs on disk.
pub fn normalize(tzid: String) -> String {
    if tzid.parse::<Tz>().is_ok() {
        return tzid;
    }
    match tzid.parse::<WindowsTimezone>() {
        Ok(wt) => Tz::from(wt).name().to_string(),
        Err(_) => tzid,
    }
}

/// Map an IANA zone name to its canonical Windows equivalent. `None` for
/// non-IANA inputs or IANA zones with no Windows counterpart.
pub fn from_iana(iana: &str) -> Option<&'static str> {
    let tz: Tz = iana.parse().ok()?;
    let wt = WindowsTimezone::try_from(tz).ok()?;
    Some(wt.name())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn normalize_maps_known_windows_zone_to_iana() {
        assert_eq!(
            normalize("E. South America Standard Time".to_string()),
            "America/Sao_Paulo"
        );
        assert_eq!(
            normalize("Pacific Standard Time".to_string()),
            "America/Los_Angeles"
        );
        assert_eq!(
            normalize("W. Europe Standard Time".to_string()),
            "Europe/Berlin"
        );
        assert_eq!(normalize("GMT Standard Time".to_string()), "Europe/London");
    }

    #[test]
    fn normalize_passes_iana_zones_through_unchanged() {
        assert_eq!(
            normalize("America/New_York".to_string()),
            "America/New_York"
        );
        assert_eq!(
            normalize("Europe/Stockholm".to_string()),
            "Europe/Stockholm"
        );
        // Don't rewrite "UTC" to "Etc/UTC" even though CLDR maps both.
        assert_eq!(normalize("UTC".to_string()), "UTC");
    }

    #[test]
    fn normalize_passes_unknown_strings_through_unchanged() {
        assert_eq!(normalize("Bogus/Zone".to_string()), "Bogus/Zone");
        assert_eq!(normalize(String::new()), String::new());
        assert_eq!(
            normalize("tzone://Microsoft/Custom".to_string()),
            "tzone://Microsoft/Custom"
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
    fn from_iana_returns_none_for_non_iana_input() {
        assert_eq!(from_iana("Pacific Standard Time"), None);
        assert_eq!(from_iana("Bogus/Zone"), None);
        assert_eq!(from_iana(""), None);
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
