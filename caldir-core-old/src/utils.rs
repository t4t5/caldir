use std::path::{Path, PathBuf};

pub fn slugify(s: &str) -> String {
    let slug = slug::slugify(s);
    slug.chars().take(50).collect()
}

pub fn expand_tilde(path: &Path) -> PathBuf {
    let path = path.to_string_lossy();
    PathBuf::from(shellexpand::tilde(path.as_ref()).into_owned())
}
