//! CLI-specific rendering with colors and formatting

use caldir_lib::diff::{CalendarDiff, DiffKind, EventDiff};
use caldir_lib::Calendar;
use indicatif::{ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;

pub fn create_spinner(msg: String) -> ProgressBar {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .unwrap(),
    );
    spinner.set_message(msg);
    spinner.enable_steady_tick(std::time::Duration::from_millis(100));
    spinner
}

pub fn render_calendar(calendar: &Calendar) -> String {
    format!("🗓️ {}", calendar.name)
}

pub fn render_diff_kind(kind: &DiffKind) -> String {
    match kind {
        DiffKind::Create => "+".green().to_string(),
        DiffKind::Update => "~".yellow().to_string(),
        DiffKind::Delete => "-".red().to_string(),
    }
}

fn colorize_by_kind(kind: &DiffKind, text: &str) -> String {
    match kind {
        DiffKind::Create => text.green().to_string(),
        DiffKind::Update => text.yellow().to_string(),
        DiffKind::Delete => text.red().to_string(),
    }
}

pub fn render_event_diff(diff: &EventDiff) -> String {
    let event = diff.event();
    let summary = colorize_by_kind(&diff.kind, &event.to_string());
    let time = event.render_event_time();

    format!("{} {} {}", render_diff_kind(&diff.kind), summary, time.dimmed())
}

pub fn render_calendar_diff(diff: &CalendarDiff) -> String {
    if diff.is_empty() {
        return "   No changes".dimmed().to_string();
    }

    let mut lines = Vec::new();

    if !diff.to_push.is_empty() {
        lines.push("   Local changes (to push):".dimmed().to_string());
        for d in &diff.to_push {
            lines.push(format!("   {}", render_event_diff(d)));
        }
    }

    if !diff.to_pull.is_empty() {
        lines.push("   Remote changes (to pull):".dimmed().to_string());
        for d in &diff.to_pull {
            lines.push(format!("   {}", render_event_diff(d)));
        }
    }

    lines.join("\n")
}

pub fn render_pull_diff(diff: &CalendarDiff) -> String {
    if diff.to_pull.is_empty() {
        return "   No changes to pull".dimmed().to_string();
    }

    let mut lines = Vec::new();
    for d in &diff.to_pull {
        lines.push(format!("   {}", render_event_diff(d)));
    }
    lines.join("\n")
}

pub fn render_push_diff(diff: &CalendarDiff) -> String {
    if diff.to_push.is_empty() {
        return "   No changes to push".dimmed().to_string();
    }

    let mut lines = Vec::new();
    for d in &diff.to_push {
        lines.push(format!("   {}", render_event_diff(d)));
    }
    lines.join("\n")
}
