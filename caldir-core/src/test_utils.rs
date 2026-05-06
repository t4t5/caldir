use crate::{Caldir, Calendar, caldir::config::CaldirConfig};

pub fn test_caldir() -> (tempfile::TempDir, Caldir) {
    let tmp = tempfile::TempDir::new().unwrap();
    let caldir = Caldir::new(CaldirConfig {
        calendar_dir: tmp.path().to_path_buf(),
    });
    (tmp, caldir)
}

pub fn test_calendar() -> (tempfile::TempDir, Caldir, Calendar) {
    let (tmp, caldir) = test_caldir();
    let calendar = Calendar::new(&caldir, "work").unwrap();
    calendar.save().unwrap();
    (tmp, caldir, calendar)
}
