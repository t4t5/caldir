use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Transparency {
    // OPAQUE is the RFC 5545 default — see `Status` for the rationale.
    #[default]
    Opaque,
    Transparent,
}

impl fmt::Display for Transparency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Opaque => "busy",
            Self::Transparent => "free",
        };
        write!(f, "{}", s)
    }
}

impl Transparency {
    pub fn as_ics_str(&self) -> &'static str {
        match self {
            Self::Opaque => "OPAQUE",
            Self::Transparent => "TRANSPARENT",
        }
    }

    pub fn from_ics_str(s: &str) -> Option<Self> {
        match s {
            "OPAQUE" => Some(Self::Opaque),
            "TRANSPARENT" => Some(Self::Transparent),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_to_rfc5545_strings() {
        assert_eq!(Transparency::Opaque.as_ics_str(), "OPAQUE");
        assert_eq!(Transparency::Transparent.as_ics_str(), "TRANSPARENT");
    }

    #[test]
    fn parses_each_rfc5545_string() {
        assert_eq!(
            Transparency::from_ics_str("OPAQUE"),
            Some(Transparency::Opaque)
        );
        assert_eq!(
            Transparency::from_ics_str("TRANSPARENT"),
            Some(Transparency::Transparent)
        );
    }

    #[test]
    fn unknown_strings_return_none() {
        assert_eq!(Transparency::from_ics_str("BUSY"), None);
        assert_eq!(Transparency::from_ics_str("opaque"), None);
        assert_eq!(Transparency::from_ics_str(""), None);
    }

    #[test]
    fn display_uses_user_facing_label() {
        assert_eq!(Transparency::Opaque.to_string(), "busy");
        assert_eq!(Transparency::Transparent.to_string(), "free");
    }
}
