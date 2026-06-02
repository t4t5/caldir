use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Availability {
    // Busy (OPAQUE) is the RFC 5545 default — see `Status` for the rationale.
    #[default]
    Busy,
    Free,
}

impl fmt::Display for Availability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Busy => "busy",
            Self::Free => "free",
        };
        write!(f, "{}", s)
    }
}

impl Availability {
    pub fn as_ics_str(&self) -> &'static str {
        match self {
            Self::Busy => "OPAQUE",
            Self::Free => "TRANSPARENT",
        }
    }

    pub fn from_ics_str(s: &str) -> Option<Self> {
        match s {
            "OPAQUE" => Some(Self::Busy),
            "TRANSPARENT" => Some(Self::Free),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_to_rfc5545_strings() {
        assert_eq!(Availability::Busy.as_ics_str(), "OPAQUE");
        assert_eq!(Availability::Free.as_ics_str(), "TRANSPARENT");
    }

    #[test]
    fn parses_each_rfc5545_string() {
        assert_eq!(
            Availability::from_ics_str("OPAQUE"),
            Some(Availability::Busy)
        );
        assert_eq!(
            Availability::from_ics_str("TRANSPARENT"),
            Some(Availability::Free)
        );
    }

    #[test]
    fn unknown_strings_return_none() {
        assert_eq!(Availability::from_ics_str("BUSY"), None);
        assert_eq!(Availability::from_ics_str("opaque"), None);
        assert_eq!(Availability::from_ics_str(""), None);
    }

    #[test]
    fn display_uses_user_facing_label() {
        assert_eq!(Availability::Busy.to_string(), "busy");
        assert_eq!(Availability::Free.to_string(), "free");
    }
}
