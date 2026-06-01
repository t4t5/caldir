use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    // RFC 5545 CLASS values. Absent CLASS maps to `None`, so no Default here.
    Public,
    Private,
    Confidential,
}

impl fmt::Display for Visibility {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Public => "public",
            Self::Private => "private",
            Self::Confidential => "confidential",
        };
        write!(f, "{}", s)
    }
}

impl Visibility {
    pub fn as_ics_str(&self) -> &'static str {
        match self {
            Self::Public => "PUBLIC",
            Self::Private => "PRIVATE",
            Self::Confidential => "CONFIDENTIAL",
        }
    }

    pub fn from_ics_str(s: &str) -> Option<Self> {
        match s {
            "PUBLIC" => Some(Self::Public),
            "PRIVATE" => Some(Self::Private),
            "CONFIDENTIAL" => Some(Self::Confidential),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_to_rfc5545_strings() {
        assert_eq!(Visibility::Public.as_ics_str(), "PUBLIC");
        assert_eq!(Visibility::Private.as_ics_str(), "PRIVATE");
        assert_eq!(Visibility::Confidential.as_ics_str(), "CONFIDENTIAL");
    }

    #[test]
    fn parses_each_rfc5545_string() {
        assert_eq!(Visibility::from_ics_str("PUBLIC"), Some(Visibility::Public));
        assert_eq!(
            Visibility::from_ics_str("PRIVATE"),
            Some(Visibility::Private)
        );
        assert_eq!(
            Visibility::from_ics_str("CONFIDENTIAL"),
            Some(Visibility::Confidential)
        );
    }

    #[test]
    fn unknown_strings_return_none() {
        // RFC 5545 permits implementation-defined CLASS values; we treat any
        // value we don't recognize as the default (PUBLIC) at the call site,
        // so `from_ics_str` reports unknown explicitly via `None`.
        assert_eq!(Visibility::from_ics_str("SECRET"), None);
        assert_eq!(Visibility::from_ics_str("private"), None);
        assert_eq!(Visibility::from_ics_str(""), None);
    }

    #[test]
    fn display_uses_lowercase_label() {
        assert_eq!(Visibility::Public.to_string(), "public");
        assert_eq!(Visibility::Private.to_string(), "private");
        assert_eq!(Visibility::Confidential.to_string(), "confidential");
    }
}
