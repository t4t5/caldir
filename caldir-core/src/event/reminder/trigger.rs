use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use icalendar::Property;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReminderTrigger {
    /// Offset from the event start or end. Negative = before the reference time.
    Relative { offset: Duration, related: Related },
    /// Absolute UTC time.
    Absolute(DateTime<Utc>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Related {
    #[default]
    Start,
    End,
}

/// Parse a TRIGGER property into a [`ReminderTrigger`].
///
/// Hand-rolled rather than going through `icalendar::Trigger::try_from(&Property)`
/// because that path rejects negative durations (`-PT10M`), which is the most
/// common form for "N minutes before the event".
pub(super) fn parse_trigger(prop: &Property) -> Result<ReminderTrigger, ()> {
    let value_kind = prop.params().get("VALUE").map(|p| p.value());
    let raw = prop.value();

    if value_kind == Some("DATE-TIME") {
        let dt = NaiveDateTime::parse_from_str(raw, "%Y%m%dT%H%M%SZ").map_err(|_| ())?;
        return Ok(ReminderTrigger::Absolute(dt.and_utc()));
    }

    let (is_negative, duration_str) = match raw.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, raw.strip_prefix('+').unwrap_or(raw)),
    };
    let parsed = iso8601::duration(duration_str).map_err(|_| ())?;
    let std_duration: std::time::Duration = parsed.into();
    let mut offset = Duration::from_std(std_duration).map_err(|_| ())?;
    if is_negative {
        offset = -offset;
    }

    let related = prop
        .params()
        .get("RELATED")
        .map(|p| match p.value() {
            "END" => Related::End,
            _ => Related::Start,
        })
        .unwrap_or_default();

    Ok(ReminderTrigger::Relative { offset, related })
}

pub(super) fn format_trigger_property(trigger: &ReminderTrigger) -> Property {
    match trigger {
        ReminderTrigger::Relative { offset, related } => {
            let mut prop = Property::new("TRIGGER", format_duration(*offset));
            prop.add_parameter(
                "RELATED",
                match related {
                    Related::Start => "START",
                    Related::End => "END",
                },
            );
            prop.done()
        }
        ReminderTrigger::Absolute(dt) => {
            let mut prop = Property::new("TRIGGER", dt.format("%Y%m%dT%H%M%SZ").to_string());
            prop.add_parameter("VALUE", "DATE-TIME");
            prop.done()
        }
    }
}

/// Format a duration as an RFC 5545 / ISO 8601 duration string.
///
/// Chrono's `Duration::Display` always emits seconds (`PT600S` for ten
/// minutes); we prefer the canonical mixed form (`PT10M`, `P1D`, `P1W`) so
/// that ICS round-trips don't gratuitously rewrite values pulled from
/// providers.
fn format_duration(d: Duration) -> String {
    let total_seconds = d.num_seconds();
    if total_seconds == 0 {
        return "PT0S".to_string();
    }
    let abs = total_seconds.unsigned_abs();
    let sign = if total_seconds < 0 { "-" } else { "" };

    if abs.is_multiple_of(86_400) {
        let days = abs / 86_400;
        if days.is_multiple_of(7) {
            return format!("{sign}P{}W", days / 7);
        }
        return format!("{sign}P{days}D");
    }

    let hours = abs / 3_600;
    let minutes = (abs % 3_600) / 60;
    let seconds = abs % 60;

    let mut s = format!("{sign}PT");
    if hours > 0 {
        s.push_str(&format!("{hours}H"));
    }
    if minutes > 0 {
        s.push_str(&format!("{minutes}M"));
    }
    if seconds > 0 {
        s.push_str(&format!("{seconds}S"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use pretty_assertions::assert_eq;

    #[test]
    fn parses_negative_relative_duration_with_default_related() {
        let prop = Property::new("TRIGGER", "-PT10M").done();

        let trigger = parse_trigger(&prop).unwrap();

        assert_eq!(
            trigger,
            ReminderTrigger::Relative {
                offset: Duration::minutes(-10),
                related: Related::Start,
            }
        );
    }

    #[test]
    fn parses_relative_duration_with_related_end() {
        let mut prop = Property::new("TRIGGER", "-PT5M");
        prop.add_parameter("RELATED", "END");

        let trigger = parse_trigger(&prop.done()).unwrap();

        assert_eq!(
            trigger,
            ReminderTrigger::Relative {
                offset: Duration::minutes(-5),
                related: Related::End,
            }
        );
    }

    #[test]
    fn parses_absolute_utc() {
        let mut prop = Property::new("TRIGGER", "20260101T120000Z");
        prop.add_parameter("VALUE", "DATE-TIME");

        let trigger = parse_trigger(&prop.done()).unwrap();

        assert_eq!(
            trigger,
            ReminderTrigger::Absolute(Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap())
        );
    }

    #[test]
    fn rejects_malformed_duration() {
        let prop = Property::new("TRIGGER", "not-a-duration").done();

        assert!(parse_trigger(&prop).is_err());
    }

    #[test]
    fn formats_relative_trigger_with_related() {
        let prop = format_trigger_property(&ReminderTrigger::Relative {
            offset: Duration::minutes(-10),
            related: Related::Start,
        });

        assert_eq!(prop.value(), "-PT10M");
        assert_eq!(
            prop.params().get("RELATED").map(|p| p.value()),
            Some("START")
        );
    }

    #[test]
    fn formats_absolute_trigger() {
        let prop = format_trigger_property(&ReminderTrigger::Absolute(
            Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap(),
        ));

        assert_eq!(prop.value(), "20260101T120000Z");
        assert_eq!(
            prop.params().get("VALUE").map(|p| p.value()),
            Some("DATE-TIME")
        );
    }

    #[test]
    fn formats_minutes_as_pt_minutes() {
        assert_eq!(format_duration(Duration::minutes(-10)), "-PT10M");
        assert_eq!(format_duration(Duration::minutes(15)), "PT15M");
    }

    #[test]
    fn formats_hours_as_pt_hours() {
        assert_eq!(format_duration(Duration::hours(-1)), "-PT1H");
    }

    #[test]
    fn formats_whole_days_as_p_days() {
        assert_eq!(format_duration(Duration::days(2)), "P2D");
    }

    #[test]
    fn formats_whole_weeks_as_p_weeks() {
        assert_eq!(format_duration(Duration::days(14)), "P2W");
    }

    #[test]
    fn formats_zero_duration() {
        assert_eq!(format_duration(Duration::zero()), "PT0S");
    }

    #[test]
    fn formats_mixed_hours_and_minutes() {
        assert_eq!(format_duration(Duration::minutes(90)), "PT1H30M");
    }
}
