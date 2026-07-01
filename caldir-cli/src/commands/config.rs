use anyhow::Result;
use caldir_core::{Caldir, CaldirConfig};
use serde::Serialize;
use std::path::PathBuf;

use crate::output::TextRender;

#[derive(Serialize)]
pub struct ConfigView {
    pub path: PathBuf,
    pub config: CaldirConfig,
}

impl TextRender for ConfigView {
    fn to_text(&self) -> String {
        format!("Path: {}\n\n{}", self.path.display(), self.config)
            .trim_end()
            .to_string()
    }
}

pub fn run(caldir: &Caldir) -> Result<ConfigView> {
    Ok(ConfigView {
        path: CaldirConfig::default_system_config_path()?,
        config: caldir.config().clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::Output;
    use caldir_core::{Reminder, TimeFormat};
    use indoc::indoc;
    use pretty_assertions::assert_eq;

    fn sample_config() -> CaldirConfig {
        CaldirConfig::new(
            PathBuf::from("/tmp/calendars"),
            TimeFormat::H24,
            Some("my_calendar".to_string()),
            Some(vec![
                Reminder::from_minutes(30),
                Reminder::from_minutes(120),
            ]),
        )
    }

    fn sample_config_view() -> ConfigView {
        ConfigView {
            path: PathBuf::from("/tmp/caldir/config.toml"),
            config: sample_config(),
        }
    }

    #[test]
    fn renders_text() {
        let expected = indoc! {r#"
            Path: /tmp/caldir/config.toml

            calendar_dir = "/tmp/calendars"
            time_format = "24h"
            default_calendar = "my_calendar"
            default_reminders = ["30m", "2h"]"#};

        assert_eq!(sample_config_view().to_text(), expected);
    }

    #[test]
    fn renders_json() {
        let json = sample_config_view().to_json();

        assert_eq!(json["path"], "/tmp/caldir/config.toml");
        assert_eq!(json["config"]["calendar_dir"], "/tmp/calendars");
        assert_eq!(json["config"]["time_format"], "24h");
        assert_eq!(json["config"]["default_calendar"], "my_calendar");
        assert_eq!(json["config"]["default_reminders"][0], "30m");
        assert_eq!(json["config"]["default_reminders"][1], "2h");
    }
}
