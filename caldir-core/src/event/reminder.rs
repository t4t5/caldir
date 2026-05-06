use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use icalendar::{Alarm, Component, Property, Related as IcalRelated, Trigger as IcalTrigger};

const DEFAULT_REMINDER_DESCRIPTION: &str = "Reminder";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Reminder {
    pub trigger: ReminderTrigger,
    pub action: ReminderAction,
    pub description: Option<String>,
}

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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReminderAction {
    Display,
    Audio,
    Email,
    /// IANA / vendor-specific actions (e.g. `X-MARTY`). Preserved for round-trip.
    Other(String),
}

impl Reminder {
    /// Parse all VALARM children, sort, and return.
    ///
    /// Reminder order has no semantic meaning, so we canonicalize on the way in
    /// to keep emitted ICS deterministic and to avoid spurious sync diffs when
    /// providers return alarms in different orders.
    pub(crate) fn from_ical_event(event: &icalendar::Event) -> Vec<Self> {
        let mut reminders: Vec<Self> = event
            .components()
            .iter()
            .filter(|c| c.component_kind() == "VALARM")
            .filter_map(|c| Reminder::from_valarm(c).ok())
            .collect();
        reminders.sort();
        reminders
    }

    /// Build a [`Reminder`] from any `VALARM`-shaped component.
    ///
    /// Generic over [`Component`] because `icalendar::Other` (the type returned
    /// by `Event::components()` for child VALARMs) is not part of the crate's
    /// public API.
    fn from_valarm<C: Component + ?Sized>(value: &C) -> Result<Self, ()> {
        let trigger_prop = value.properties().get("TRIGGER").ok_or(())?;
        let trigger = parse_trigger(trigger_prop)?;

        let action = value
            .property_value("ACTION")
            .map(ReminderAction::from)
            .unwrap_or(ReminderAction::Display);

        let description = value.property_value("DESCRIPTION").map(str::to_string);

        Ok(Reminder {
            trigger,
            action,
            description,
        })
    }
}

/// Parse a TRIGGER property into a [`ReminderTrigger`].
///
/// Hand-rolled rather than going through `icalendar::Trigger::try_from(&Property)`
/// because that path rejects negative durations (`-PT10M`), which is the most
/// common form for "N minutes before the event".
fn parse_trigger(prop: &Property) -> Result<ReminderTrigger, ()> {
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

impl From<&Reminder> for Alarm {
    fn from(value: &Reminder) -> Self {
        // We can't construct an `Alarm` directly (its `default()` is private),
        // so we seed one via `Alarm::display(...)` and then overwrite the
        // properties we care about. This also lets us emit the TRIGGER in
        // canonical RFC 5545 form (`-PT10M`) rather than chrono's seconds-only
        // `Duration::Display` (`-PT600S`), which keeps round-trips stable.
        let placeholder = IcalTrigger::Duration(Duration::zero(), Some(IcalRelated::Start));
        let description = value
            .description
            .as_deref()
            .unwrap_or(DEFAULT_REMINDER_DESCRIPTION);
        let mut alarm = Alarm::display(description, placeholder);

        alarm.append_property(format_trigger_property(&value.trigger));

        match &value.action {
            ReminderAction::Display => return alarm,
            ReminderAction::Audio => {
                alarm.append_property(Property::new("ACTION", "AUDIO"));
            }
            ReminderAction::Email => {
                alarm.append_property(Property::new("ACTION", "EMAIL"));
            }
            ReminderAction::Other(name) => {
                alarm.append_property(Property::new("ACTION", name));
            }
        }

        if value.description.is_none() {
            alarm.remove_property("DESCRIPTION");
        }

        alarm
    }
}

fn format_trigger_property(trigger: &ReminderTrigger) -> Property {
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

impl From<&str> for ReminderAction {
    fn from(value: &str) -> Self {
        match value {
            "DISPLAY" => ReminderAction::Display,
            "AUDIO" => ReminderAction::Audio,
            "EMAIL" => ReminderAction::Email,
            other => ReminderAction::Other(other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use icalendar::Component;
    use pretty_assertions::assert_eq;

    fn parse_reminder(ics_alarm_block: &str) -> Reminder {
        let ics = format!(
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:test@caldir\r\nDTSTART:20260101T120000Z\r\n{ics_alarm_block}\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n"
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
            .into_iter()
            .next()
            .expect("VALARM should produce a Reminder")
    }

    fn try_parse_reminder(ics_alarm_block: &str) -> Option<Reminder> {
        let ics = format!(
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:test@caldir\r\nDTSTART:20260101T120000Z\r\n{ics_alarm_block}\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n"
        );
        let cal: icalendar::Calendar = ics.parse().unwrap();
        let event = cal.components.into_iter().find_map(|c| match c {
            icalendar::CalendarComponent::Event(e) => Some(e),
            _ => None,
        })?;
        Reminder::from_ical_event(&event).into_iter().next()
    }

    #[test]
    fn parses_relative_display_alarm() {
        let reminder = parse_reminder(
            "BEGIN:VALARM\r\nACTION:DISPLAY\r\nDESCRIPTION:Reminder\r\nTRIGGER:-PT10M\r\nEND:VALARM",
        );

        assert_eq!(reminder.action, ReminderAction::Display);
        assert_eq!(
            reminder.description.as_deref(),
            Some(DEFAULT_REMINDER_DESCRIPTION)
        );
        assert_eq!(
            reminder.trigger,
            ReminderTrigger::Relative {
                offset: Duration::minutes(-10),
                related: Related::Start,
            }
        );
    }

    #[test]
    fn parses_relative_alarm_defaults_related_to_start() {
        let reminder = parse_reminder(
            "BEGIN:VALARM\r\nACTION:DISPLAY\r\nDESCRIPTION:Reminder\r\nTRIGGER:-PT10M\r\nEND:VALARM",
        );

        assert!(matches!(
            reminder.trigger,
            ReminderTrigger::Relative {
                related: Related::Start,
                ..
            }
        ));
    }

    #[test]
    fn parses_relative_alarm_with_related_end() {
        let reminder = parse_reminder(
            "BEGIN:VALARM\r\nACTION:DISPLAY\r\nDESCRIPTION:Reminder\r\nTRIGGER;RELATED=END:-PT5M\r\nEND:VALARM",
        );

        assert_eq!(
            reminder.trigger,
            ReminderTrigger::Relative {
                offset: Duration::minutes(-5),
                related: Related::End,
            }
        );
    }

    #[test]
    fn parses_audio_action() {
        let reminder =
            parse_reminder("BEGIN:VALARM\r\nACTION:AUDIO\r\nTRIGGER:-PT10M\r\nEND:VALARM");

        assert_eq!(reminder.action, ReminderAction::Audio);
        assert_eq!(reminder.description, None);
    }

    #[test]
    fn parses_email_action() {
        let reminder = parse_reminder(
            "BEGIN:VALARM\r\nACTION:EMAIL\r\nDESCRIPTION:Body\r\nSUMMARY:Subj\r\nTRIGGER:-PT1H\r\nEND:VALARM",
        );

        assert_eq!(reminder.action, ReminderAction::Email);
        assert_eq!(reminder.description.as_deref(), Some("Body"));
    }

    #[test]
    fn parses_unknown_action_as_other() {
        let reminder =
            parse_reminder("BEGIN:VALARM\r\nACTION:X-CUSTOM\r\nTRIGGER:-PT5M\r\nEND:VALARM");

        assert_eq!(
            reminder.action,
            ReminderAction::Other("X-CUSTOM".to_string())
        );
    }

    #[test]
    fn defaults_action_to_display_when_missing() {
        let reminder = parse_reminder("BEGIN:VALARM\r\nTRIGGER:-PT10M\r\nEND:VALARM");

        assert_eq!(reminder.action, ReminderAction::Display);
    }

    #[test]
    fn parses_absolute_utc_trigger() {
        let reminder = parse_reminder(
            "BEGIN:VALARM\r\nACTION:DISPLAY\r\nDESCRIPTION:Reminder\r\nTRIGGER;VALUE=DATE-TIME:20260101T120000Z\r\nEND:VALARM",
        );

        assert_eq!(
            reminder.trigger,
            ReminderTrigger::Absolute(Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap())
        );
    }

    #[test]
    fn skips_alarm_when_trigger_missing() {
        let result = try_parse_reminder(
            "BEGIN:VALARM\r\nACTION:DISPLAY\r\nDESCRIPTION:Reminder\r\nEND:VALARM",
        );

        assert!(result.is_none());
    }

    #[test]
    fn writes_relative_trigger_with_explicit_related() {
        let reminder = Reminder {
            trigger: ReminderTrigger::Relative {
                offset: Duration::minutes(-10),
                related: Related::Start,
            },
            action: ReminderAction::Display,
            description: Some("Wake up".to_string()),
        };

        let alarm = Alarm::from(&reminder);

        let trigger_prop = alarm.properties().get("TRIGGER").unwrap();
        assert_eq!(trigger_prop.value(), "-PT10M");
        assert_eq!(
            trigger_prop.params().get("RELATED").map(|p| p.value()),
            Some("START")
        );
    }

    #[test]
    fn writes_absolute_trigger() {
        let reminder = Reminder {
            trigger: ReminderTrigger::Absolute(Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap()),
            action: ReminderAction::Display,
            description: Some(DEFAULT_REMINDER_DESCRIPTION.to_string()),
        };

        let alarm = Alarm::from(&reminder);

        let trigger_prop = alarm.properties().get("TRIGGER").unwrap();
        assert_eq!(trigger_prop.value(), "20260101T120000Z");
    }

    #[test]
    fn writes_display_action_with_description() {
        let reminder = Reminder {
            trigger: ReminderTrigger::Relative {
                offset: Duration::minutes(-10),
                related: Related::Start,
            },
            action: ReminderAction::Display,
            description: Some("Wake up".to_string()),
        };

        let alarm = Alarm::from(&reminder);

        assert_eq!(alarm.property_value("ACTION"), Some("DISPLAY"));
        assert_eq!(alarm.property_value("DESCRIPTION"), Some("Wake up"));
    }

    #[test]
    fn writes_audio_action_without_description() {
        let reminder = Reminder {
            trigger: ReminderTrigger::Relative {
                offset: Duration::minutes(-10),
                related: Related::Start,
            },
            action: ReminderAction::Audio,
            description: None,
        };

        let alarm = Alarm::from(&reminder);

        assert_eq!(alarm.property_value("ACTION"), Some("AUDIO"));
        assert!(alarm.properties().get("DESCRIPTION").is_none());
    }

    #[test]
    fn writes_email_action_preserves_description() {
        let reminder = Reminder {
            trigger: ReminderTrigger::Relative {
                offset: Duration::hours(-1),
                related: Related::Start,
            },
            action: ReminderAction::Email,
            description: Some("Body".to_string()),
        };

        let alarm = Alarm::from(&reminder);

        assert_eq!(alarm.property_value("ACTION"), Some("EMAIL"));
        assert_eq!(alarm.property_value("DESCRIPTION"), Some("Body"));
    }

    #[test]
    fn writes_other_action() {
        let reminder = Reminder {
            trigger: ReminderTrigger::Relative {
                offset: Duration::minutes(-5),
                related: Related::Start,
            },
            action: ReminderAction::Other("X-CUSTOM".to_string()),
            description: None,
        };

        let alarm = Alarm::from(&reminder);

        assert_eq!(alarm.property_value("ACTION"), Some("X-CUSTOM"));
        assert!(alarm.properties().get("DESCRIPTION").is_none());
    }

    #[test]
    fn writes_display_with_placeholder_description_when_missing() {
        let reminder = Reminder {
            trigger: ReminderTrigger::Relative {
                offset: Duration::minutes(-10),
                related: Related::Start,
            },
            action: ReminderAction::Display,
            description: None,
        };

        let alarm = Alarm::from(&reminder);

        assert_eq!(
            alarm.property_value("DESCRIPTION"),
            Some(DEFAULT_REMINDER_DESCRIPTION)
        );
    }

    #[test]
    fn from_ical_event_sorts_by_trigger_offset() {
        let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:test@caldir\r\nDTSTART:20260101T120000Z\r\nBEGIN:VALARM\r\nACTION:DISPLAY\r\nDESCRIPTION:Reminder\r\nTRIGGER:-PT5M\r\nEND:VALARM\r\nBEGIN:VALARM\r\nACTION:DISPLAY\r\nDESCRIPTION:Reminder\r\nTRIGGER:-PT30M\r\nEND:VALARM\r\nBEGIN:VALARM\r\nACTION:DISPLAY\r\nDESCRIPTION:Reminder\r\nTRIGGER:-PT10M\r\nEND:VALARM\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let cal: icalendar::Calendar = ics.parse().unwrap();
        let event = cal
            .components
            .iter()
            .find_map(|c| match c {
                icalendar::CalendarComponent::Event(e) => Some(e.clone()),
                _ => None,
            })
            .unwrap();

        let reminders = Reminder::from_ical_event(&event);

        let offsets: Vec<_> = reminders
            .iter()
            .map(|r| match &r.trigger {
                ReminderTrigger::Relative { offset, .. } => offset.num_minutes(),
                ReminderTrigger::Absolute(_) => unreachable!(),
            })
            .collect();
        assert_eq!(offsets, vec![-30, -10, -5]);
    }

    #[test]
    fn from_ical_event_skips_unparseable_alarm() {
        let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:test@caldir\r\nDTSTART:20260101T120000Z\r\nBEGIN:VALARM\r\nACTION:DISPLAY\r\nDESCRIPTION:no trigger\r\nEND:VALARM\r\nBEGIN:VALARM\r\nACTION:DISPLAY\r\nDESCRIPTION:Reminder\r\nTRIGGER:-PT10M\r\nEND:VALARM\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let cal: icalendar::Calendar = ics.parse().unwrap();
        let event = cal
            .components
            .iter()
            .find_map(|c| match c {
                icalendar::CalendarComponent::Event(e) => Some(e.clone()),
                _ => None,
            })
            .unwrap();

        let reminders = Reminder::from_ical_event(&event);

        assert_eq!(reminders.len(), 1);
    }

    #[test]
    fn relative_sorts_before_absolute() {
        let mut reminders = [
            Reminder {
                trigger: ReminderTrigger::Absolute(
                    Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap(),
                ),
                action: ReminderAction::Display,
                description: None,
            },
            Reminder {
                trigger: ReminderTrigger::Relative {
                    offset: Duration::minutes(-5),
                    related: Related::Start,
                },
                action: ReminderAction::Display,
                description: None,
            },
        ];
        reminders.sort();

        assert!(matches!(
            reminders[0].trigger,
            ReminderTrigger::Relative { .. }
        ));
        assert!(matches!(reminders[1].trigger, ReminderTrigger::Absolute(_)));
    }
}
