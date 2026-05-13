use caldir_core::{Caldir, Event, ParticipationStatus};
use owo_colors::OwoColorize;

use crate::render::time::format_time_only;

/// Format a standard event line: "  {time} {summary} [{cal_slug}]{status}"
pub fn format_event_line(event: &Event, cal_slug: &str, status: &str, caldir: &Caldir) -> String {
    let time = format_time_only(&event.start, caldir.config().time_format());
    let cal_tag = format!("[{}]", cal_slug);

    format!(
        "  {} {} {}{}",
        time,
        event.summary().unwrap_or("(Untitled)"),
        cal_tag.dimmed(),
        status
    )
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
