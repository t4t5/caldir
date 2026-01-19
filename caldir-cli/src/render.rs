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
        format!("📅 {}", self.name)
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
            }
        }

        if !self.to_pull.is_empty() {
            lines.push("   Remote changes (to pull):".dimmed().to_string());
            for diff in &self.to_pull {
                lines.push(format!("   {}", diff.render()));
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
