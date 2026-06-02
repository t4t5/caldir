use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Status {
    // CONFIRMED is the RFC 5545 default — an event with no STATUS line is
    // semantically confirmed, so absence and "confirmed" must round-trip the
    // same way.
    #[default]
    Confirmed,
    Tentative,
    Cancelled,
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Confirmed => "confirmed",
            Self::Tentative => "tentative",
            Self::Cancelled => "cancelled",
        };
        write!(f, "{}", s)
    }
}

impl Status {
    pub fn as_ics_str(&self) -> &'static str {
        match self {
            Self::Confirmed => "CONFIRMED",
            Self::Tentative => "TENTATIVE",
            Self::Cancelled => "CANCELLED",
        }
    }

    pub fn from_ics_str(s: &str) -> Option<Self> {
        match s {
            "CONFIRMED" => Some(Self::Confirmed),
            "TENTATIVE" => Some(Self::Tentative),
            "CANCELLED" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_to_rfc5545_strings() {
        assert_eq!(Status::Confirmed.as_ics_str(), "CONFIRMED");
        assert_eq!(Status::Tentative.as_ics_str(), "TENTATIVE");
        assert_eq!(Status::Cancelled.as_ics_str(), "CANCELLED");
    }

    #[test]
    fn parses_each_rfc5545_string() {
        assert_eq!(Status::from_ics_str("CONFIRMED"), Some(Status::Confirmed));
        assert_eq!(Status::from_ics_str("TENTATIVE"), Some(Status::Tentative));
        assert_eq!(Status::from_ics_str("CANCELLED"), Some(Status::Cancelled));
    }

    #[test]
    fn unknown_strings_return_none() {
        assert_eq!(Status::from_ics_str("DRAFT"), None);
        assert_eq!(Status::from_ics_str("confirmed"), None);
        assert_eq!(Status::from_ics_str(""), None);
    }

    #[test]
    fn display_uses_lowercase_label() {
        assert_eq!(Status::Confirmed.to_string(), "confirmed");
        assert_eq!(Status::Tentative.to_string(), "tentative");
        assert_eq!(Status::Cancelled.to_string(), "cancelled");
    }
}
