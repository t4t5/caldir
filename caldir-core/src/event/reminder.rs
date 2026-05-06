mod action;
mod trigger;

pub use action::ReminderAction;
pub use trigger::{Related, ReminderTrigger};

use icalendar::{Component, Property};
use trigger::{format_trigger_property, parse_trigger};

const DEFAULT_REMINDER_DESCRIPTION: &str = "Reminder";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Reminder {
    pub trigger: ReminderTrigger,
    pub action: ReminderAction,
    pub description: Option<String>,
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

    /// Format this reminder as a `VALARM` block (RFC 5545).
    ///
    /// We emit the block ourselves rather than going through
    /// `icalendar::Alarm` + the icalendar event serializer, because
    /// `icalendar::Component::fmt_write` injects a random `UID:<uuid>` line
    /// into every sub-component that doesn't already have one. VALARM doesn't
    /// require a UID per RFC 5545 and we don't model it on `Reminder`, so
    /// letting that through would break byte-stable round-trips and surface as
    /// spurious sync diffs. We still use `icalendar::Property` for individual
    /// lines so we get correct line folding and text escaping.
    pub(crate) fn to_ics_block(&self) -> String {
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
        let mut props = vec![Property::new("ACTION", self.action.as_ics_str()).done()];
        if let Some(desc) = self.effective_description() {
            props.push(Property::new("DESCRIPTION", desc).done());
        }
        props.push(format_trigger_property(&self.trigger));
        props
    }

    /// DISPLAY alarms require a DESCRIPTION per RFC 5545; fill in a placeholder
    /// when the user didn't supply one. Other action types may legitimately
    /// omit DESCRIPTION.
    fn effective_description(&self) -> Option<&str> {
        match (&self.description, &self.action) {
            (Some(desc), _) => Some(desc.as_str()),
            (None, ReminderAction::Display) => Some(DEFAULT_REMINDER_DESCRIPTION),
            (None, _) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone, Utc};
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

    #[test]
    fn parses_full_valarm() {
        let reminder = parse_reminder(
            "BEGIN:VALARM\r\nACTION:DISPLAY\r\nDESCRIPTION:Wake up\r\nTRIGGER:-PT10M\r\nEND:VALARM",
        );

        assert_eq!(reminder.action, ReminderAction::Display);
        assert_eq!(reminder.description.as_deref(), Some("Wake up"));
        assert_eq!(
            reminder.trigger,
            ReminderTrigger::Relative {
                offset: Duration::minutes(-10),
                related: Related::Start,
            }
        );
    }

    #[test]
    fn defaults_action_to_display_when_missing() {
        let reminder = parse_reminder("BEGIN:VALARM\r\nTRIGGER:-PT10M\r\nEND:VALARM");

        assert_eq!(reminder.action, ReminderAction::Display);
    }

    #[test]
    fn skips_alarm_when_trigger_missing() {
        let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:test@caldir\r\nDTSTART:20260101T120000Z\r\nBEGIN:VALARM\r\nACTION:DISPLAY\r\nDESCRIPTION:no trigger\r\nEND:VALARM\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let cal: icalendar::Calendar = ics.parse().unwrap();
        let event = cal
            .components
            .into_iter()
            .find_map(|c| match c {
                icalendar::CalendarComponent::Event(e) => Some(e),
                _ => None,
            })
            .unwrap();

        assert!(Reminder::from_ical_event(&event).is_empty());
    }

    #[test]
    fn writes_full_valarm() {
        let reminder = Reminder {
            trigger: ReminderTrigger::Relative {
                offset: Duration::minutes(-10),
                related: Related::Start,
            },
            action: ReminderAction::Display,
            description: Some("Wake up".to_string()),
        };

        let block = reminder.to_ics_block();

        assert_eq!(
            block,
            "BEGIN:VALARM\r\nACTION:DISPLAY\r\nDESCRIPTION:Wake up\r\nTRIGGER;RELATED=START:-PT10M\r\nEND:VALARM\r\n"
        );
    }

    #[test]
    fn fills_in_placeholder_description_for_display_when_missing() {
        let reminder = Reminder {
            trigger: ReminderTrigger::Relative {
                offset: Duration::minutes(-10),
                related: Related::Start,
            },
            action: ReminderAction::Display,
            description: None,
        };

        let block = reminder.to_ics_block();

        assert!(block.contains(&format!("DESCRIPTION:{DEFAULT_REMINDER_DESCRIPTION}\r\n")));
    }

    #[test]
    fn omits_description_for_audio_when_missing() {
        let reminder = Reminder {
            trigger: ReminderTrigger::Relative {
                offset: Duration::minutes(-10),
                related: Related::Start,
            },
            action: ReminderAction::Audio,
            description: None,
        };

        let block = reminder.to_ics_block();

        assert!(!block.contains("DESCRIPTION"));
    }

    #[test]
    fn does_not_emit_uid_inside_valarm() {
        // icalendar's `Component::fmt_write` auto-injects a random UID into
        // every sub-component. We sidestep that by formatting VALARM ourselves;
        // this test guards against accidentally regressing back through it.
        let reminder = Reminder {
            trigger: ReminderTrigger::Relative {
                offset: Duration::minutes(-10),
                related: Related::Start,
            },
            action: ReminderAction::Display,
            description: None,
        };

        let block = reminder.to_ics_block();

        assert!(
            !block.contains("UID"),
            "VALARM block contained a UID:\n{block}"
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
