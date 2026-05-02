use std::fmt;
use std::path::Path;

pub struct TildePath<'a>(&'a Path);

impl fmt::Display for TildePath<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Ok(home) = std::env::var("HOME")
            && let Ok(relative) = self.0.strip_prefix(&home)
        {
            return write!(f, "~/{}", relative.display());
        }
        write!(f, "{}", self.0.display())
    }
}

pub trait PathExt {
    fn tilde(&self) -> TildePath<'_>;
}

impl PathExt for Path {
    fn tilde(&self) -> TildePath<'_> {
        TildePath(self)
    }
}
