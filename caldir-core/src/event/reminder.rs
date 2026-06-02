use icalendar::{Component, Property};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::cmp::Reverse;
use std::fmt;
use std::time::Duration;

const DEFAULT_REMINDER_DESCRIPTION: &str = "Reminder";
const MINUTES_PER_HOUR: u64 = 60;
const MINUTES_PER_DAY: u64 = 24 * MINUTES_PER_HOUR;
const MINUTES_PER_WEEK: u64 = 7 * MINUTES_PER_DAY;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Reminder {
    /// Minutes before the event start.
    pub minutes_before_start: i64,
}

impl fmt::Display for Reminder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} before start", self.to_human())
    }
}

impl Serialize for Reminder {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_human())
    }
}

impl<'de> Deserialize<'de> for Reminder {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Reminder::from_human(&s).map_err(serde::de::Error::custom)
    }
}

impl Reminder {
    pub fn from_minutes(minutes: i64) -> Self {
        Reminder {
            minutes_before_start: minutes,
        }
    }

    pub fn from_human(input: &str) -> Result<Self, humantime::DurationError> {
        let dur = humantime::parse_duration(input)?;
        let minutes = (dur.as_secs() / 60) as i64;

        Ok(Reminder {
            minutes_before_start: minutes,
        })
    }

    pub fn to_human(&self) -> String {
        let seconds = self.minutes_before_start.unsigned_abs() * 60;
        humantime::format_duration(Duration::from_secs(seconds)).to_string()
    }

    /// We intentionally treat all alarms the same,
    /// and don't distinguish between ACTION:DISPLAY, ACTION:AUDIO, or ACTION:EMAIL.
    /// (How alarms are handled should be up to the app layer instead)
    pub(crate) fn from_ical_event(event: &icalendar::Event) -> Vec<Self> {
        let mut reminders: Vec<Self> = event
            .components()
            .iter()
            .filter(|c| c.component_kind() == "VALARM")
            .filter_map(|c| Reminder::from_valarm(c).ok())
            .collect();
        reminders.sort_by_key(|reminder| Reverse(reminder.minutes_before_start));
        reminders
    }

    fn from_valarm<C: Component + ?Sized>(value: &C) -> Result<Self, ()> {
        let trigger_prop = value.properties().get("TRIGGER").ok_or(())?;
        parse_trigger_minutes_before_start(trigger_prop).map(Reminder::from_minutes)
    }

    /// Format this reminder as a minimal DISPLAY `VALARM` block (RFC 5545).
    ///
    /// We emit the block ourselves rather than going through
    /// `icalendar::Alarm` + the icalendar event serializer, because
    /// `icalendar::Component::fmt_write` injects a random `UID:<uuid>` line
    /// into every sub-component that doesn't already have one. VALARM doesn't
    /// require a UID per RFC 5545, so letting that through would break
    /// byte-stable round-trips and surface as spurious sync diffs.
    pub(crate) fn ics_block(&self) -> String {
        let mut block = String::from("BEGIN:VALARM\r\n");

        for property in self.properties() {
            let line: String = property
                .try_into()
                .expect("formatting an icalendar Property to String never fails");
            block.push_str(&line);
        }

        block.push_str("END:VALARM\r\n");
        block
    }

    fn properties(&self) -> Vec<Property> {
        let mut trigger = Property::new(
            "TRIGGER",
            format_trigger_minutes_before_start(self.minutes_before_start),
        );
        trigger.add_parameter("RELATED", "START");

        vec![
            Property::new("ACTION", "DISPLAY").done(),
            Property::new("DESCRIPTION", DEFAULT_REMINDER_DESCRIPTION).done(),
            trigger.done(),
        ]
    }
}

fn parse_trigger_minutes_before_start(prop: &Property) -> Result<i64, ()> {
    match prop.params().get("VALUE").map(|p| p.value()) {
        None => {}
        Some(value) if value.eq_ignore_ascii_case("DURATION") => {}
        Some(_) => return Err(()),
    }

    match prop.params().get("RELATED").map(|p| p.value()) {
        None => {}
        Some(value) if value.eq_ignore_ascii_case("START") => {}
        Some(_) => return Err(()),
    }

    let raw = prop.value().strip_prefix('-').ok_or(())?;
    let minutes = parse_duration_minutes(raw)?;
    i64::try_from(minutes).map_err(|_| ())
}

fn parse_duration_minutes(raw: &str) -> Result<u64, ()> {
    let body = raw.strip_prefix('P').ok_or(())?;
    if body.is_empty() {
        return Err(());
    }

    if let Some(weeks) = body.strip_suffix('W') {
        return parse_u64(weeks)?.checked_mul(MINUTES_PER_WEEK).ok_or(());
    }

    let (date_part, time_part) = match body.split_once('T') {
        Some((_, "")) => return Err(()),
        Some((date_part, time_part)) => (date_part, Some(time_part)),
        None => (body, None),
    };

    let days = match date_part {
        "" => 0,
        value => parse_u64(value.strip_suffix('D').ok_or(())?)?,
    };
    let time_minutes = time_part.map(parse_time_minutes).unwrap_or(Ok(0))?;

    days.checked_mul(MINUTES_PER_DAY)
        .and_then(|minutes| minutes.checked_add(time_minutes))
        .ok_or(())
}

fn parse_time_minutes(raw: &str) -> Result<u64, ()> {
    let original = raw;
    let (hours, raw) = consume_unit(raw, 'H')?.unwrap_or((0, raw));
    let (minutes, raw) = consume_unit(raw, 'M')?.unwrap_or((0, raw));
    let (seconds, raw) = consume_unit(raw, 'S')?.unwrap_or((0, raw));

    if !raw.is_empty() || raw == original || seconds % 60 != 0 {
        return Err(());
    }

    hours
        .checked_mul(MINUTES_PER_HOUR)
        .and_then(|total| total.checked_add(minutes))
        .and_then(|total| total.checked_add(seconds / 60))
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

fn format_trigger_minutes_before_start(minutes: i64) -> String {
    let minutes = minutes.unsigned_abs();
    if minutes == 0 {
        return "PT0S".to_string();
    }

    if minutes.is_multiple_of(MINUTES_PER_WEEK) {
        return format!("-P{}W", minutes / MINUTES_PER_WEEK);
    }
    if minutes.is_multiple_of(MINUTES_PER_DAY) {
        return format!("-P{}D", minutes / MINUTES_PER_DAY);
    }

    let days = minutes / MINUTES_PER_DAY;
    let remainder = minutes % MINUTES_PER_DAY;
    let hours = remainder / MINUTES_PER_HOUR;
    let minutes = remainder % MINUTES_PER_HOUR;

    let mut s = if days > 0 {
        format!("-P{days}DT")
    } else {
        "-PT".to_string()
    };
    if hours > 0 {
        s.push_str(&format!("{hours}H"));
    }
    if minutes > 0 {
        s.push_str(&format!("{minutes}M"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn parse_reminders(ics_alarm_blocks: &str) -> Vec<Reminder> {
        let ics = format!(
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:test@caldir\r\nDTSTART:20260101T120000Z\r\n{ics_alarm_blocks}\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n"
        );
        let cal: icalendar::Calendar = ics.parse().unwrap();
        let event = cal
            .components
            .into_iter()
            .find_map(|c| match c {
                icalendar::CalendarComponent::Event(e) => Some(e),
                _ => None,
            })
            .expect("VEVENT should be present");
        Reminder::from_ical_event(&event)
    }

    fn parse_reminder(ics_alarm_block: &str) -> Reminder {
        parse_reminders(ics_alarm_block)
            .into_iter()
            .next()
            .expect("VALARM should produce a Reminder")
    }

    #[test]
    fn parses_display_alarm_minutes_before_start() {
        let reminder = parse_reminder(
            "BEGIN:VALARM\r\nACTION:DISPLAY\r\nDESCRIPTION:Wake up\r\nTRIGGER:-PT10M\r\nEND:VALARM",
        );

        assert_eq!(reminder, Reminder::from_minutes(10));
    }

    #[test]
    fn defaults_action_to_display_when_missing() {
        let reminder = parse_reminder("BEGIN:VALARM\r\nTRIGGER:-PT10M\r\nEND:VALARM");

        assert_eq!(reminder, Reminder::from_minutes(10));
    }

    #[test]
    fn parses_hours_days_and_weeks() {
        assert_eq!(parse_duration_minutes("PT1H"), Ok(60));
        assert_eq!(parse_duration_minutes("P1D"), Ok(1_440));
        assert_eq!(parse_duration_minutes("P1W"), Ok(10_080));
    }

    #[test]
    fn parses_mixed_day_time_duration() {
        let reminder = parse_reminder(
            "BEGIN:VALARM\r\nACTION:DISPLAY\r\nTRIGGER;VALUE=DURATION:-P1DT2H30M\r\nEND:VALARM",
        );

        assert_eq!(reminder, Reminder::from_minutes(1_590));
    }

    #[test]
    fn parses_zero_duration() {
        let reminder = parse_reminder("BEGIN:VALARM\r\nTRIGGER:-PT0S\r\nEND:VALARM");

        assert_eq!(reminder, Reminder::from_minutes(0));
    }

    #[test]
    fn skips_alarm_when_trigger_missing() {
        let reminders = parse_reminders(
            "BEGIN:VALARM\r\nACTION:DISPLAY\r\nDESCRIPTION:no trigger\r\nEND:VALARM",
        );

        assert!(reminders.is_empty());
    }

    #[test]
    fn accepts_audio_alarm() {
        // iCloud emits AUDIO alarms by default
        let reminder = parse_reminder(
            "BEGIN:VALARM\r\nACTION:AUDIO\r\nATTACH:Basso\r\nTRIGGER:-PT10M\r\nEND:VALARM",
        );

        assert_eq!(reminder, Reminder::from_minutes(10));
    }

    #[test]
    fn accepts_email_alarm() {
        let reminder = parse_reminder(
            "BEGIN:VALARM\r\nACTION:EMAIL\r\nATTENDEE:mailto:a@b.com\r\nTRIGGER:-PT10M\r\nEND:VALARM",
        );

        assert_eq!(reminder, Reminder::from_minutes(10));
    }

    #[test]
    fn skips_positive_offset_alarm() {
        let reminders =
            parse_reminders("BEGIN:VALARM\r\nACTION:DISPLAY\r\nTRIGGER:PT10M\r\nEND:VALARM");

        assert!(reminders.is_empty());
    }

    #[test]
    fn skips_end_relative_alarm() {
        let reminders = parse_reminders(
            "BEGIN:VALARM\r\nACTION:DISPLAY\r\nTRIGGER;RELATED=END:-PT10M\r\nEND:VALARM",
        );

        assert!(reminders.is_empty());
    }

    #[test]
    fn skips_absolute_alarm() {
        let reminders = parse_reminders(
            "BEGIN:VALARM\r\nACTION:DISPLAY\r\nTRIGGER;VALUE=DATE-TIME:20260101T120000Z\r\nEND:VALARM",
        );

        assert!(reminders.is_empty());
    }

    #[test]
    fn skips_sub_minute_alarm() {
        let reminders =
            parse_reminders("BEGIN:VALARM\r\nACTION:DISPLAY\r\nTRIGGER:-PT30S\r\nEND:VALARM");

        assert!(reminders.is_empty());
    }

    #[test]
    fn skips_iso_year_and_month_durations() {
        assert!(parse_duration_minutes("P1Y").is_err());
        assert!(parse_duration_minutes("P1M").is_err());
    }

    #[test]
    fn writes_full_valarm() {
        let reminder = Reminder::from_minutes(10);

        let block = reminder.ics_block();

        assert_eq!(
            block,
            "BEGIN:VALARM\r\nACTION:DISPLAY\r\nDESCRIPTION:Reminder\r\nTRIGGER;RELATED=START:-PT10M\r\nEND:VALARM\r\n"
        );
    }

    #[test]
    fn does_not_emit_uid_inside_valarm() {
        // icalendar's `Component::fmt_write` auto-injects a random UID into
        // every sub-component. We sidestep that by formatting VALARM ourselves;
        // this test guards against accidentally regressing back through it.
        let block = Reminder::from_minutes(10).ics_block();

        assert!(
            !block.contains("UID"),
            "VALARM block contained a UID:\n{block}"
        );
    }

    #[test]
    fn from_ical_event_sorts_by_alarm_time() {
        let reminders = parse_reminders(
            "BEGIN:VALARM\r\nACTION:DISPLAY\r\nDESCRIPTION:Reminder\r\nTRIGGER:-PT5M\r\nEND:VALARM\r\nBEGIN:VALARM\r\nACTION:DISPLAY\r\nDESCRIPTION:Reminder\r\nTRIGGER:-PT30M\r\nEND:VALARM\r\nBEGIN:VALARM\r\nACTION:DISPLAY\r\nDESCRIPTION:Reminder\r\nTRIGGER:-PT10M\r\nEND:VALARM",
        );

        let minutes: Vec<_> = reminders.iter().map(|r| r.minutes_before_start).collect();
        assert_eq!(minutes, vec![30, 10, 5]);
    }

    #[test]
    fn display_shows_humantime_with_before_start_suffix() {
        assert_eq!(Reminder::from_minutes(10).to_string(), "10m before start");
        assert_eq!(
            Reminder::from_minutes(90).to_string(),
            "1h 30m before start"
        );
    }

    #[test]
    fn formats_compact_trigger_durations() {
        assert_eq!(format_trigger_minutes_before_start(0), "PT0S");
        assert_eq!(format_trigger_minutes_before_start(10), "-PT10M");
        assert_eq!(format_trigger_minutes_before_start(60), "-PT1H");
        assert_eq!(format_trigger_minutes_before_start(90), "-PT1H30M");
        assert_eq!(format_trigger_minutes_before_start(1_440), "-P1D");
        assert_eq!(format_trigger_minutes_before_start(1_500), "-P1DT1H");
        assert_eq!(format_trigger_minutes_before_start(20_160), "-P2W");
    }
}
