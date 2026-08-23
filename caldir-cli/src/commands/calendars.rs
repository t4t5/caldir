use anyhow::Result;
use caldir_core::{Caldir, CaldirError, Calendar};

use crate::output::calendars::{CalendarView, CalendarsView};

pub fn run(caldir: &Caldir) -> Result<CalendarsView> {
    build_view(caldir.calendars())
}

fn build_view(calendars: Vec<Result<Calendar, CaldirError>>) -> Result<CalendarsView> {
    let mut calendars = calendars.into_iter().collect::<Result<Vec<_>, _>>()?;
    calendars.sort_by(|a, b| a.slug().cmp(&b.slug()));

    Ok(CalendarsView(
        calendars.into_iter().map(CalendarView::from).collect(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::{Output, TextRender};
    use caldir_core::{CalendarConfig, ProviderSlug, RemoteConfig, RemoteConfigParams};
    use indoc::indoc;
    use pretty_assertions::assert_eq;

    #[test]
    fn renders_connected_and_local_calendars_ordered_by_slug() {
        let tmp = tempfile::tempdir().unwrap();
        let remote = RemoteConfig::new(ProviderSlug::from("google"), RemoteConfigParams::new());
        let config = CalendarConfig::new(
            Some("Personal".to_string()),
            Some("#4285f4".to_string()),
            Some(true),
            Some(remote),
        );
        let connected = Calendar::create(&tmp.path().join("personal"), Some(config)).unwrap();
        let local = Calendar::create(&tmp.path().join("local-notes"), None).unwrap();

        let view = build_view(vec![Ok(connected), Ok(local)]).unwrap();

        assert_eq!(
            view.to_text(),
            indoc! {r#"
                local-notes
                  name: -
                  color: -
                  read only: no
                  provider: -

                personal
                  name: Personal
                  color: #4285f4
                  read only: yes
                  provider: google"#}
        );

        let json = view.to_json();
        assert_eq!(json[0]["slug"], "local-notes");
        assert_eq!(json[0]["name"], serde_json::Value::Null);
        assert_eq!(json[0]["color"], serde_json::Value::Null);
        assert_eq!(json[0]["read_only"], false);
        assert_eq!(json[0]["provider"], serde_json::Value::Null);
        assert_eq!(json[1]["slug"], "personal");
        assert_eq!(json[1]["name"], "Personal");
        assert_eq!(json[1]["color"], "#4285f4");
        assert_eq!(json[1]["read_only"], true);
        assert_eq!(json[1]["provider"], "google");
    }

    #[test]
    fn invalid_calendar_config_returns_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let calendar_path = tmp.path().join("broken");
        std::fs::create_dir_all(calendar_path.join(".caldir")).unwrap();
        std::fs::write(calendar_path.join(".caldir/config.toml"), "not valid toml").unwrap();

        let calendar = Calendar::load(&calendar_path).map_err(CaldirError::from);

        assert!(build_view(vec![calendar]).is_err());
    }
}
