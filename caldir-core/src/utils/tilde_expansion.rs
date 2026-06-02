use std::path::{Path, PathBuf};

pub fn expand_tilde(path: &Path) -> PathBuf {
    let Ok(rest) = path.strip_prefix("~") else {
        return path.to_path_buf();
    };
    match home::home_dir() {
        Some(home) => home.join(rest),
        None => path.to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_leading_tilde() {
        let home = home::home_dir().unwrap();

        assert_eq!(expand_tilde(Path::new("~/caldir")), home.join("caldir"));
        assert_eq!(expand_tilde(Path::new("~")), home);
    }

    #[test]
    fn leaves_absolute_paths_unchanged() {
        assert_eq!(
            expand_tilde(Path::new("/tmp/calendar")),
            PathBuf::from("/tmp/calendar"),
        );
    }

    #[test]
    fn leaves_relative_paths_unchanged() {
        assert_eq!(
            expand_tilde(Path::new("calendar/sub")),
            PathBuf::from("calendar/sub"),
        );
    }

    #[test]
    fn does_not_expand_tilde_mid_path() {
        // `~user/foo` — tilde as a username prefix is not supported.
        assert_eq!(
            expand_tilde(Path::new("~user/foo")),
            PathBuf::from("~user/foo"),
        );
    }
}
