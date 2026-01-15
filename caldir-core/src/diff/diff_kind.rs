use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
