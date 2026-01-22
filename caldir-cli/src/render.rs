//! TUI rendering traits for caldir types.
//!
//! This module provides extension traits that add colored terminal rendering
//! to caldir-core types using owo_colors.

use caldir_core::calendar::Calendar;
use caldir_core::diff::{CalendarDiff, DiffKind, EventDiff};
use owo_colors::OwoColorize;

/// Extension trait for TUI rendering with colors.
pub trait Render {
    fn render(&self) -> String;
}

impl Render for DiffKind {
    fn render(&self) -> String {
        let symbol = self.symbol();
        match self {
            DiffKind::Create => symbol.green().to_string(),
            DiffKind::Update => symbol.yellow().to_string(),
            DiffKind::Delete => symbol.red().to_string(),
        }
    }
}

/// Colorize text according to the diff kind
fn colorize_diff(kind: DiffKind, text: &str) -> String {
    match kind {
        DiffKind::Create => text.green().to_string(),
        DiffKind::Update => text.yellow().to_string(),
        DiffKind::Delete => text.red().to_string(),
    }
}

impl Render for EventDiff {
    fn render(&self) -> String {
        let event = self.event();
        let summary = colorize_diff(self.kind, &event.to_string());
        let time = event.render_event_time();

        format!("{} {} {}", self.kind.render(), summary, time.dimmed())
    }
}

impl Render for Calendar {
    fn render(&self) -> String {
        format!("📅 {}", self.slug)
    }
}

/// Extended rendering for CalendarDiff with directional output
pub trait CalendarDiffRender {
    fn render(&self) -> String;
    fn render_pull(&self) -> String;
    fn render_push(&self) -> String;
}

impl CalendarDiffRender for CalendarDiff {
    fn render(&self) -> String {
        if self.is_empty() {
            return "   No changes".dimmed().to_string();
        }

        let mut lines = Vec::new();

        if !self.to_push.is_empty() {
            lines.push("   Local changes (to push):".dimmed().to_string());
            for diff in &self.to_push {
                lines.push(format!("   {}", diff.render()));
                lines.extend(render_field_diffs(diff).into_iter().map(|l| format!("      {}", l)));
            }
        }

        if !self.to_pull.is_empty() {
            // Add spacing if we have both push and pull changes
            if !self.to_push.is_empty() {
                lines.push(String::new());
            }
            lines.push("   Remote changes (to pull):".dimmed().to_string());
            for diff in &self.to_pull {
                lines.push(format!("   {}", diff.render()));
                lines.extend(render_field_diffs(diff).into_iter().map(|l| format!("      {}", l)));
            }
        }

        lines.join("\n")
    }

    fn render_pull(&self) -> String {
        if self.to_pull.is_empty() {
            return "   No changes to pull".dimmed().to_string();
        }

        let mut lines = Vec::new();
        for diff in &self.to_pull {
            lines.push(format!("   {}", diff.render()));
        }
        lines.join("\n")
    }

    fn render_push(&self) -> String {
        if self.to_push.is_empty() {
            return "   No changes to push".dimmed().to_string();
        }

        let mut lines = Vec::new();
        for diff in &self.to_push {
            lines.push(format!("   {}", diff.render()));
        }
        lines.join("\n")
    }
}

/// Render field-by-field differences for an EventDiff (only for updates)
fn render_field_diffs(diff: &EventDiff) -> Vec<String> {
    let mut lines = Vec::new();

    // Only show field diffs for updates
    if let (Some(old), Some(new)) = (&diff.old, &diff.new) {
        if old.id != new.id {
            lines.push(format!("{}: {} → {}", "id".dimmed(), old.id.red(), new.id.green()));
        }
        if old.summary != new.summary {
            lines.push(format!("{}: {} → {}", "summary".dimmed(), old.summary.red(), new.summary.green()));
        }
        if old.description != new.description {
            lines.push(format!("{}: {:?} → {:?}", "description".dimmed(), old.description.as_ref().map(|s| s.red()), new.description.as_ref().map(|s| s.green())));
        }
        if old.location != new.location {
            lines.push(format!("{}: {:?} → {:?}", "location".dimmed(), old.location.as_ref().map(|s| s.red()), new.location.as_ref().map(|s| s.green())));
        }
        if old.start != new.start {
            lines.push(format!("{}: {} → {}", "start".dimmed(), old.start.to_string().red(), new.start.to_string().green()));
        }
        if old.end != new.end {
            lines.push(format!("{}: {} → {}", "end".dimmed(), old.end.to_string().red(), new.end.to_string().green()));
        }
        if old.status != new.status {
            lines.push(format!("{}: {:?} → {:?}", "status".dimmed(), old.status, new.status));
        }
        if old.recurrence != new.recurrence {
            lines.push(format!("{}: {:?} → {:?}", "recurrence".dimmed(), old.recurrence, new.recurrence));
        }
        if old.original_start != new.original_start {
            lines.push(format!("{}: {:?} → {:?}", "original_start".dimmed(), old.original_start, new.original_start));
        }
        if old.reminders != new.reminders {
            lines.push(format!("{}: {:?} → {:?}", "reminders".dimmed(), old.reminders, new.reminders));
        }
        if old.transparency != new.transparency {
            lines.push(format!("{}: {:?} → {:?}", "transparency".dimmed(), old.transparency, new.transparency));
        }
        if old.organizer != new.organizer {
            lines.push(format!("{}: {:?} → {:?}", "organizer".dimmed(), old.organizer, new.organizer));
        }
        if old.attendees != new.attendees {
            lines.push(format!("{}: {:?} → {:?}", "attendees".dimmed(), old.attendees, new.attendees));
        }
        if old.conference_url != new.conference_url {
            lines.push(format!("{}: {:?} → {:?}", "conference_url".dimmed(), old.conference_url, new.conference_url));
        }
    }

    lines
}
