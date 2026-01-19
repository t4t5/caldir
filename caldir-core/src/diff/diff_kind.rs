//! Diff kind enumeration.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffKind {
    Create,
    Update,
    Delete,
}

impl DiffKind {
    /// Get the symbol for this diff kind
    pub fn symbol(&self) -> &'static str {
        match self {
            Self::Create => "+",
            Self::Update => "~",
            Self::Delete => "-",
        }
    }
}

impl fmt::Display for DiffKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.symbol())
    }
}
