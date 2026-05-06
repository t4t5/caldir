use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use icalendar::Property;

const SECONDS_PER_MINUTE: u64 = 60;
const SECONDS_PER_HOUR: u64 = 60 * SECONDS_PER_MINUTE;
const SECONDS_PER_DAY: u64 = 24 * SECONDS_PER_HOUR;
const SECONDS_PER_WEEK: u64 = 7 * SECONDS_PER_DAY;

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

impl Related {
    fn from_param(value: &str) -> Result<Self, ()> {
        match value {
            value if value.eq_ignore_ascii_case("START") => Ok(Related::Start),
            value if value.eq_ignore_ascii_case("END") => Ok(Related::End),
            _ => Err(()),
        }
    }

    fn as_param(self) -> &'static str {
        match self {
            Related::Start => "START",
            Related::End => "END",
        }
    }
}

/// Parse a TRIGGER property into a [`ReminderTrigger`].
///
/// Hand-rolled rather than going through `icalendar::Trigger::try_from(&Property)`
/// because that path rejects negative durations (`-PT10M`), which is the most
/// common form for "N minutes before the event".
pub(super) fn parse_trigger(prop: &Property) -> Result<ReminderTrigger, ()> {
    match prop.params().get("VALUE").map(|p| p.value()) {
        Some(value) if value.eq_ignore_ascii_case("DATE-TIME") => {
            parse_absolute_trigger(prop.value()).map(ReminderTrigger::Absolute)
        }
        Some(value) if value.eq_ignore_ascii_case("DURATION") => parse_relative_trigger(prop),
        None => parse_relative_trigger(prop),
        Some(_) => Err(()),
    }
}

fn parse_absolute_trigger(raw: &str) -> Result<DateTime<Utc>, ()> {
    NaiveDateTime::parse_from_str(raw, "%Y%m%dT%H%M%SZ")
        .map(|dt| dt.and_utc())
        .map_err(|_| ())
}

fn parse_related(prop: &Property) -> Result<Related, ()> {
    prop.params()
        .get("RELATED")
        .map(|p| Related::from_param(p.value()))
        .unwrap_or(Ok(Related::Start))
}

pub(super) fn format_trigger_property(trigger: &ReminderTrigger) -> Property {
    match trigger {
        ReminderTrigger::Relative { offset, related } => {
            let mut prop = Property::new("TRIGGER", format_duration(*offset));
            prop.add_parameter("RELATED", related.as_param());
            prop.done()
        }
        ReminderTrigger::Absolute(dt) => {
            let mut prop = Property::new("TRIGGER", dt.format("%Y%m%dT%H%M%SZ").to_string());
            prop.add_parameter("VALUE", "DATE-TIME");
            prop.done()
        }
    }
}

fn parse_relative_trigger(prop: &Property) -> Result<ReminderTrigger, ()> {
    Ok(ReminderTrigger::Relative {
        offset: parse_alarm_offset(prop.value())?,
        related: parse_related(prop)?,
    })
}

/// Parse the duration forms commonly used by VALARM TRIGGER.
///
/// This is deliberately scoped to event alarms, not a reusable RFC 5545
/// duration abstraction. It accepts signed day/week/time offsets like
/// `-PT10M`, `-PT1H`, `-P1D`, `PT5M`, and mixed day-time values.
fn parse_alarm_offset(raw: &str) -> Result<Duration, ()> {
    let (negative, raw) = match raw.as_bytes().first() {
        Some(b'-') => (true, &raw[1..]),
        Some(b'+') => (false, &raw[1..]),
        _ => (false, raw),
    };

    let body = raw.strip_prefix('P').ok_or(())?;
    if body.is_empty() {
        return Err(());
    }

    let seconds = if let Some(weeks) = body.strip_suffix('W') {
        parse_u64(weeks)?.checked_mul(SECONDS_PER_WEEK).ok_or(())?
    } else {
        parse_day_time_offset(body)?
    };

    let seconds = i64::try_from(seconds).map_err(|_| ())?;
    let seconds = if negative { -seconds } else { seconds };
    Duration::try_seconds(seconds).ok_or(())
}

fn parse_day_time_offset(body: &str) -> Result<u64, ()> {
    let (date_part, time_part) = match body.split_once('T') {
        Some((_, "")) => return Err(()),
        Some((date_part, time_part)) => (date_part, Some(time_part)),
        None => (body, None),
    };

    let days = match date_part {
        "" => 0,
        value => parse_u64(value.strip_suffix('D').ok_or(())?)?,
    };

    let time_seconds = time_part.map(parse_time_offset).unwrap_or(Ok(0))?;

    days.checked_mul(SECONDS_PER_DAY)
        .and_then(|seconds| seconds.checked_add(time_seconds))
        .ok_or(())
}

fn parse_time_offset(raw: &str) -> Result<u64, ()> {
    let original = raw;
    let (hours, raw) = consume_unit(raw, 'H')?.unwrap_or((0, raw));
    let (minutes, raw) = consume_unit(raw, 'M')?.unwrap_or((0, raw));
    let (seconds, raw) = consume_unit(raw, 'S')?.unwrap_or((0, raw));

    if !raw.is_empty() || raw == original {
        return Err(());
    }

    hours
        .checked_mul(SECONDS_PER_HOUR)
        .and_then(|total| {
            minutes
                .checked_mul(SECONDS_PER_MINUTE)
                .and_then(|minutes| total.checked_add(minutes))
        })
        .and_then(|total| total.checked_add(seconds))
        .ok_or(())
}

fn consume_unit(raw: &str, unit: char) -> Result<Option<(u64, &str)>, ()> {
    let Some(unit_index) = raw.find(unit) else {
        return Ok(None);
    };
    let (digits, rest) = raw.split_at(unit_index);
    let rest = rest.strip_prefix(unit).ok_or(())?;
    Ok(Some((parse_u64(digits)?, rest)))
}

fn parse_u64(raw: &str) -> Result<u64, ()> {
    if raw.is_empty() || !raw.chars().all(|c| c.is_ascii_digit()) {
        return Err(());
    }
    raw.parse().map_err(|_| ())
}

/// Format a VALARM offset as a compact duration string.
fn format_duration(d: Duration) -> String {
    let total_seconds = d.num_seconds();
    if total_seconds == 0 {
        return "PT0S".to_string();
    }
    let abs = total_seconds.unsigned_abs();
    let sign = if total_seconds < 0 { "-" } else { "" };

    if abs.is_multiple_of(SECONDS_PER_DAY) {
        let days = abs / SECONDS_PER_DAY;
        if days.is_multiple_of(7) {
            return format!("{sign}P{}W", days / 7);
        }
        return format!("{sign}P{days}D");
    }

    let days = abs / SECONDS_PER_DAY;
    let remainder = abs % SECONDS_PER_DAY;
    let hours = remainder / SECONDS_PER_HOUR;
    let minutes = (remainder % SECONDS_PER_HOUR) / SECONDS_PER_MINUTE;
    let seconds = remainder % SECONDS_PER_MINUTE;

    let mut s = if days > 0 {
        format!("{sign}P{days}DT")
    } else {
        format!("{sign}PT")
    };
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
    fn parses_related_case_insensitively() {
        let mut prop = Property::new("TRIGGER", "-PT5M");
        prop.add_parameter("RELATED", "end");

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
    fn rejects_unknown_related() {
        let mut prop = Property::new("TRIGGER", "PT10M");
        prop.add_parameter("RELATED", "MIDDLE");

        assert!(parse_trigger(&prop.done()).is_err());
    }

    #[test]
    fn rejects_unsupported_value_kind() {
        let mut prop = Property::new("TRIGGER", "PT10M");
        prop.add_parameter("VALUE", "TEXT");

        assert!(parse_trigger(&prop.done()).is_err());
    }

    #[test]
    fn rejects_iso_year_and_month_durations() {
        assert!(parse_alarm_offset("P1Y").is_err());
        assert!(parse_alarm_offset("P1M").is_err());
    }

    #[test]
    fn parses_mixed_date_and_time_duration() {
        let mut prop = Property::new("TRIGGER", "+P1DT2H30M");
        prop.add_parameter("VALUE", "DURATION");

        let trigger = parse_trigger(&prop.done()).unwrap();

        assert_eq!(
            trigger,
            ReminderTrigger::Relative {
                offset: Duration::days(1) + Duration::hours(2) + Duration::minutes(30),
                related: Related::Start,
            }
        );
    }

    #[test]
    fn parses_zero_duration() {
        let prop = Property::new("TRIGGER", "PT0S").done();

        let trigger = parse_trigger(&prop).unwrap();

        assert_eq!(
            trigger,
            ReminderTrigger::Relative {
                offset: Duration::zero(),
                related: Related::Start,
            }
        );
    }

    #[test]
    fn formats_relative_trigger_with_related_start() {
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
    fn formats_relative_trigger_with_related_end() {
        let prop = format_trigger_property(&ReminderTrigger::Relative {
            offset: Duration::minutes(-10),
            related: Related::End,
        });

        assert_eq!(prop.value(), "-PT10M");
        assert_eq!(prop.params().get("RELATED").map(|p| p.value()), Some("END"));
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

    #[test]
    fn formats_mixed_days_and_hours() {
        assert_eq!(format_duration(Duration::hours(25)), "P1DT1H");
    }
}
