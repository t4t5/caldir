#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReminderAction {
    Display,
    Audio,
    Email,
    /// IANA / vendor-specific actions (e.g. `X-MARTY`). Preserved for round-trip.
    Other(String),
}

impl ReminderAction {
    pub(super) fn as_ics_str(&self) -> &str {
        match self {
            ReminderAction::Display => "DISPLAY",
            ReminderAction::Audio => "AUDIO",
            ReminderAction::Email => "EMAIL",
            ReminderAction::Other(name) => name,
        }
    }
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
    use pretty_assertions::assert_eq;

    #[test]
    fn parses_known_actions() {
        assert_eq!(ReminderAction::from("DISPLAY"), ReminderAction::Display);
        assert_eq!(ReminderAction::from("AUDIO"), ReminderAction::Audio);
        assert_eq!(ReminderAction::from("EMAIL"), ReminderAction::Email);
    }

    #[test]
    fn parses_unknown_action_as_other() {
        assert_eq!(
            ReminderAction::from("X-CUSTOM"),
            ReminderAction::Other("X-CUSTOM".to_string())
        );
    }

    #[test]
    fn formats_known_actions() {
        assert_eq!(ReminderAction::Display.as_ics_str(), "DISPLAY");
        assert_eq!(ReminderAction::Audio.as_ics_str(), "AUDIO");
        assert_eq!(ReminderAction::Email.as_ics_str(), "EMAIL");
    }

    #[test]
    fn formats_other_action() {
        assert_eq!(
            ReminderAction::Other("X-CUSTOM".to_string()).as_ics_str(),
            "X-CUSTOM"
        );
    }
}
