use caldir_core::Calendar;
use serde::Serialize;

use crate::output::TextRender;

#[derive(Serialize)]
pub struct CalendarView {
    pub slug: String,
    pub name: Option<String>,
    pub color: Option<String>,
    pub read_only: bool,
    pub provider: Option<String>,
}

impl From<Calendar> for CalendarView {
    fn from(calendar: Calendar) -> Self {
        Self {
            slug: calendar.slug().unwrap_or_default().to_string(),
            name: calendar.name().map(str::to_string),
            color: calendar.color().map(str::to_string),
            read_only: calendar.is_read_only(),
            provider: calendar
                .remote_config()
                .map(|remote| remote.provider_slug().to_string()),
        }
    }
}

#[derive(Serialize)]
#[serde(transparent)]
pub struct CalendarsView(pub Vec<CalendarView>);

impl TextRender for CalendarsView {
    fn to_text(&self) -> String {
        if self.0.is_empty() {
            return "No calendars found.".to_string();
        }

        self.0
            .iter()
            .map(|calendar| {
                format!(
                    "{}\n  name: {}\n  color: {}\n  read only: {}\n  provider: {}",
                    calendar.slug,
                    calendar.name.as_deref().unwrap_or("-"),
                    calendar.color.as_deref().unwrap_or("-"),
                    if calendar.read_only { "yes" } else { "no" },
                    calendar.provider.as_deref().unwrap_or("-")
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::{Output, TextRender};
    use pretty_assertions::assert_eq;

    #[test]
    fn empty_view_renders_text_and_json() {
        let view = CalendarsView(vec![]);

        assert_eq!(view.to_text(), "No calendars found.");
        assert_eq!(view.to_json(), serde_json::json!([]));
    }
}
