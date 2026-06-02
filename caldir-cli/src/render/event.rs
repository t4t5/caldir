use caldir_core::{Caldir, Event, ParticipationStatus, Status};
use owo_colors::OwoColorize;

use crate::render::time::format_time_only;

pub fn is_visible(event: &Event) -> bool {
    event.status != Status::Cancelled
}

/// Format a standard event line: "  {time} {summary} [{cal_slug}]{status}"
pub fn format_event_line(event: &Event, cal_slug: &str, status: &str, caldir: &Caldir) -> String {
    let time = format_time_only(&event.start, caldir.config().time_format());
    let cal_tag = format!("[{}]", cal_slug);

    let summary_text = &event.summary.clone().unwrap_or("(Untitled)".to_string());

    format!("  {} {} {}{}", time, summary_text, cal_tag.dimmed(), status)
}

/// Render a participation status as colored text (e.g. "accepted" in green, "pending" in yellow)
pub fn render_participation_status(status: ParticipationStatus) -> String {
    let label = status.to_string();

    match status {
        ParticipationStatus::Accepted => label.green().to_string(),
        ParticipationStatus::Declined => label.red().to_string(),
        ParticipationStatus::Tentative | ParticipationStatus::NeedsAction => {
            label.yellow().to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use caldir_core::EventTime;
    use chrono::NaiveDate;

    #[test]
    fn confirmed_events_are_visible_cancelled_are_not() {
        let start = EventTime::Date(NaiveDate::from_ymd_opt(2026, 5, 27).unwrap());
        let confirmed = Event::new("Standup", start.clone());
        let mut cancelled = Event::new("yolo", start);
        cancelled.status = Status::Cancelled;

        assert!(is_visible(&confirmed));
        assert!(!is_visible(&cancelled));
    }
}
