#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParticipationStatus {
    Accepted,
    Declined,
    Tentative,
    NeedsAction,
}

impl ParticipationStatus {
    pub fn as_ics_str(&self) -> &'static str {
        match self {
            Self::Accepted => "ACCEPTED",
            Self::Declined => "DECLINED",
            Self::Tentative => "TENTATIVE",
            Self::NeedsAction => "NEEDS-ACTION",
        }
    }

    pub fn from_ics_str(s: &str) -> Option<Self> {
        match s {
            "ACCEPTED" => Some(Self::Accepted),
            "DECLINED" => Some(Self::Declined),
            "TENTATIVE" => Some(Self::Tentative),
            "NEEDS-ACTION" => Some(Self::NeedsAction),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_to_rfc5545_strings() {
        assert_eq!(ParticipationStatus::Accepted.as_ics_str(), "ACCEPTED");
        assert_eq!(ParticipationStatus::Declined.as_ics_str(), "DECLINED");
        assert_eq!(ParticipationStatus::Tentative.as_ics_str(), "TENTATIVE");
        assert_eq!(
            ParticipationStatus::NeedsAction.as_ics_str(),
            "NEEDS-ACTION"
        );
    }

    #[test]
    fn parses_each_rfc5545_string() {
        assert_eq!(
            ParticipationStatus::from_ics_str("ACCEPTED"),
            Some(ParticipationStatus::Accepted)
        );
        assert_eq!(
            ParticipationStatus::from_ics_str("DECLINED"),
            Some(ParticipationStatus::Declined)
        );
        assert_eq!(
            ParticipationStatus::from_ics_str("TENTATIVE"),
            Some(ParticipationStatus::Tentative)
        );
        assert_eq!(
            ParticipationStatus::from_ics_str("NEEDS-ACTION"),
            Some(ParticipationStatus::NeedsAction)
        );
    }

    #[test]
    fn unknown_strings_return_none() {
        assert_eq!(ParticipationStatus::from_ics_str("DELEGATED"), None);
        assert_eq!(ParticipationStatus::from_ics_str("accepted"), None);
        assert_eq!(ParticipationStatus::from_ics_str(""), None);
    }
}
