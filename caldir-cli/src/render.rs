//! CLI-specific rendering with colors and formatting
//!
//! This module provides a `Render` trait that extends core types with
//! terminal rendering capabilities (colors, formatting).

use caldir_lib::diff::{DiffKind, EventDiff};
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

/// Trait for rendering types to the terminal with colors
pub trait Render {
    fn render(&self) -> String;
}

impl Render for DiffKind {
    fn render(&self) -> String {
        match self {
            DiffKind::Create => "+".green().to_string(),
            DiffKind::Update => "~".yellow().to_string(),
            DiffKind::Delete => "-".red().to_string(),
        }
    }
}

impl Render for EventDiff {
    fn render(&self) -> String {
        let event = self.event();
        let summary = match self.kind {
            DiffKind::Create => event.summary.green().to_string(),
            DiffKind::Update => event.summary.yellow().to_string(),
            DiffKind::Delete => event.summary.red().to_string(),
        };
        let time = event.start.to_string();

        format!("{} {} {}", self.kind.render(), summary, time.dimmed())
    }
}

impl Render for Vec<EventDiff> {
    fn render(&self) -> String {
        if self.is_empty() {
            return format!("   {}", "No changes".dimmed());
        }

        self.iter()
            .map(|e| format!("   {}", e.render()))
            .collect::<Vec<_>>()
            .join("\n")
    }
}
