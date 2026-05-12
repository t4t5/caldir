use anyhow::Result;
use caldir_core::{Caldir, CaldirConfig};
use std::{io::Write, path::Path};

pub fn run(caldir: &Caldir) -> Result<()> {
    let mut out = std::io::stdout().lock();

    let config = caldir.config();
    let config_path = CaldirConfig::default_system_config_path()?;

    render(&mut out, &config_path, config)
}

fn render(out: &mut impl Write, config_path: &Path, config: &CaldirConfig) -> Result<()> {
    writeln!(out, "Path: {}", config_path.display())?;
    writeln!(out)?;
    write!(out, "{}", config)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::capture;
    use caldir_core::{Reminder, TimeFormat};
    use indoc::indoc;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;

    #[test]
    fn render_writes_expected_output() {
        let config = CaldirConfig::new(
            PathBuf::from("/tmp/calendars"),
            TimeFormat::H24,
            Some("my_calendar".to_string()),
            Some(vec![
                Reminder::from_minutes(30),
                Reminder::from_minutes(120),
            ]),
        );

        let config_path = PathBuf::from("/tmp/caldir/config.toml");

        let output = capture(|out| render(out, &config_path, &config));

        let expected = indoc! {r#"
            Path: /tmp/caldir/config.toml

            calendar_dir = "/tmp/calendars"
            time_format = "24h"
            default_calendar = "my_calendar"
            default_reminders = ["30m", "2h"]
        "#};

        assert_eq!(output, expected);
    }
}
