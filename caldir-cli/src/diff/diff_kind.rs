use owo_colors::OwoColorize;
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

impl DiffKind {
    /// Colorize text according to this diff kind
    pub fn colorize(&self, text: &str) -> String {
        match self {
            DiffKind::Create => text.green().to_string(),
            DiffKind::Update => text.yellow().to_string(),
            DiffKind::Delete => text.red().to_string(),
        }
    }

    /// Render the symbol with appropriate color
    pub fn render(&self) -> String {
        self.colorize(&self.to_string())
    }
}
