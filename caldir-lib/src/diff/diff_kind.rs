use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum DiffKind {
    Create,
    Update,
    Delete,
}

impl fmt::Display for DiffKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiffKind::Create => write!(f, "+"),
            DiffKind::Update => write!(f, "~"),
            DiffKind::Delete => write!(f, "-"),
        }
    }
}
