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

/// Threshold for compact view (show counts instead of individual events)
const COMPACT_THRESHOLD: usize = 5;

/// Render a list of diffs, using compact view if there are many events and verbose is false
fn render_diff_list(diffs: &[EventDiff], verbose: bool, lines: &mut Vec<String>) {
    if verbose || diffs.len() <= COMPACT_THRESHOLD {
        // Full view: show each event
        for diff in diffs {
            lines.push(format!("   {}", diff.render()));
            // Always show field diffs for updates when in full view
            if diff.kind == DiffKind::Update {
                lines.extend(
                    render_field_diffs(diff)
                        .into_iter()
                        .map(|l| format!("     {}", l)),
                );
            }
        }
    } else {
        // Compact view: show counts by diff kind
        let creates = diffs.iter().filter(|d| d.kind == DiffKind::Create).count();
        let updates = diffs.iter().filter(|d| d.kind == DiffKind::Update).count();
        let deletes = diffs.iter().filter(|d| d.kind == DiffKind::Delete).count();

        if creates > 0 {
            let label = format!("({} new {})", creates, pluralize("event", creates));
            lines.push(format!("   {} {}", "+".green(), label.green()));
        }
        if updates > 0 {
            let label = format!("({} changed {})", updates, pluralize("event", updates));
            lines.push(format!("   {} {}", "~".yellow(), label.yellow()));
        }
        if deletes > 0 {
            let label = format!("({} deleted {})", deletes, pluralize("event", deletes));
            lines.push(format!("   {} {}", "-".red(), label.red()));
        }
    }
}

/// Simple pluralization helper
fn pluralize(word: &str, count: usize) -> &str {
    if count == 1 {
        word
    } else {
        match word {
            "event" => "events",
            _ => word,
        }
    }
}

/// Extended rendering for CalendarDiff with directional output
pub trait CalendarDiffRender {
    fn render(&self, verbose: bool) -> String;
    fn render_sync(&self, verbose: bool) -> String;
    fn render_pull(&self, verbose: bool) -> String;
    fn render_push(&self, verbose: bool) -> String;
    fn render_discard(&self, verbose: bool) -> String;
}

fn render_bidirectional(
    diff: &CalendarDiff,
    verbose: bool,
    push_label: &str,
    pull_label: &str,
) -> String {
    if diff.is_empty() {
        return "   No changes".dimmed().to_string();
    }

    let mut lines = Vec::new();

    if !diff.to_push.is_empty() {
        lines.push(format!("   {}:", push_label).dimmed().to_string());
        render_diff_list(&diff.to_push, verbose, &mut lines);
    }

    if !diff.to_pull.is_empty() {
        if !diff.to_push.is_empty() {
            lines.push(String::new());
        }
        lines.push(format!("   {}:", pull_label).dimmed().to_string());
        render_diff_list(&diff.to_pull, verbose, &mut lines);
    }

    lines.join("\n")
}

impl CalendarDiffRender for CalendarDiff {
    fn render(&self, verbose: bool) -> String {
        render_bidirectional(
            self,
            verbose,
            "Local changes (to push)",
            "Remote changes (to pull)",
        )
    }

    fn render_sync(&self, verbose: bool) -> String {
        render_bidirectional(
            self,
            verbose,
            "Local changes (pushed)",
            "Remote changes (pulled)",
        )
    }

    fn render_pull(&self, verbose: bool) -> String {
        if self.to_pull.is_empty() {
            return "   No changes to pull".dimmed().to_string();
        }

        let mut lines = Vec::new();
        render_diff_list(&self.to_pull, verbose, &mut lines);
        lines.join("\n")
    }

    fn render_push(&self, verbose: bool) -> String {
        if self.to_push.is_empty() {
            return "   No changes to push".dimmed().to_string();
        }

        let mut lines = Vec::new();
        render_diff_list(&self.to_push, verbose, &mut lines);
        lines.join("\n")
    }

    fn render_discard(&self, verbose: bool) -> String {
        if self.to_push.is_empty() {
            return "   Nothing to discard".dimmed().to_string();
        }

        let mut lines = Vec::new();
        lines.push(format!("   {}:", "Local changes (to discard)").dimmed().to_string());
        render_diff_list(&self.to_push, verbose, &mut lines);
        lines.join("\n")
    }
}

/// Render field-by-field differences for an EventDiff (only for updates)
fn render_field_diffs(diff: &EventDiff) -> Vec<String> {
    let mut lines = Vec::new();

    // Only show field diffs for updates
    if let (Some(old), Some(new)) = (&diff.old, &diff.new) {
        if old.summary != new.summary {
            lines.push(format!(
                "{}: {} → {}",
                "summary".dimmed(),
                old.summary.red(),
                new.summary.green()
            ));
        }
        if old.description != new.description {
            lines.push(render_optional_diff(
                "description",
                &old.description,
                &new.description,
            ));
        }
        if old.location != new.location {
            lines.push(render_optional_diff(
                "location",
                &old.location,
                &new.location,
            ));
        }
        if old.start != new.start {
            lines.push(format!(
                "{}: {} → {}",
                "start".dimmed(),
                old.start.to_string().red(),
                new.start.to_string().green()
            ));
        }
        if old.end != new.end {
            lines.push(format!(
                "{}: {} → {}",
                "end".dimmed(),
                old.end.to_string().red(),
                new.end.to_string().green()
            ));
        }
        if old.status != new.status {
            lines.push(format!(
                "{}: {:?} → {:?}",
                "status".dimmed(),
                old.status,
                new.status
            ));
        }
        if old.recurrence != new.recurrence {
            lines.extend(render_recurrence_diff(&old.recurrence, &new.recurrence));
        }
        if old.recurrence_id != new.recurrence_id {
            lines.push(format!(
                "{}: {:?} → {:?}",
                "recurrence_id".dimmed(),
                old.recurrence_id,
                new.recurrence_id
            ));
        }
        if old.reminders != new.reminders {
            lines.push(format!(
                "{}: {:?} → {:?}",
                "reminders".dimmed(),
                old.reminders,
                new.reminders
            ));
        }
        if old.transparency != new.transparency {
            lines.push(format!(
                "{}: {:?} → {:?}",
                "transparency".dimmed(),
                old.transparency,
                new.transparency
            ));
        }
        if old.organizer != new.organizer {
            lines.push(format!(
                "{}: {:?} → {:?}",
                "organizer".dimmed(),
                old.organizer,
                new.organizer
            ));
        }
        if old.attendees != new.attendees {
            lines.push(format!(
                "{}: {:?} → {:?}",
                "attendees".dimmed(),
                old.attendees,
                new.attendees
            ));
        }
        if old.conference_url != new.conference_url {
            lines.push(render_optional_diff(
                "conference_url",
                &old.conference_url,
                &new.conference_url,
            ));
        }
    }

    lines
}

/// Render an optional string field diff
fn render_optional_diff(field: &str, old: &Option<String>, new: &Option<String>) -> String {
    let old_str = old.as_deref().unwrap_or("(none)");
    let new_str = new.as_deref().unwrap_or("(none)");
    format!(
        "{}: {} → {}",
        field.dimmed(),
        old_str.red(),
        new_str.green()
    )
}

/// Render recurrence diff showing RRULE and EXDATE changes
fn render_recurrence_diff(
    old: &Option<caldir_core::event::Recurrence>,
    new: &Option<caldir_core::event::Recurrence>,
) -> Vec<String> {
    let mut lines = Vec::new();

    match (old, new) {
        (Some(old_rec), Some(new_rec)) => {
            if old_rec.rrule != new_rec.rrule {
                lines.push(format!(
                    "{}: {} → {}",
                    "rrule".dimmed(),
                    old_rec.rrule.red(),
                    new_rec.rrule.green()
                ));
            }
            // Show exdate changes
            use std::collections::HashSet;
            let old_set: HashSet<_> =
                old_rec.exdates.iter().map(|e| format!("{}", e)).collect();
            let new_set: HashSet<_> =
                new_rec.exdates.iter().map(|e| format!("{}", e)).collect();
            for ex in old_set.difference(&new_set) {
                lines.push(format!("{} exdate {}", "-".red(), ex.red()));
            }
            for ex in new_set.difference(&old_set) {
                lines.push(format!("{} exdate {}", "+".green(), ex.green()));
            }
        }
        (None, Some(new_rec)) => {
            lines.push(format!("{} rrule {}", "+".green(), new_rec.rrule.green()));
            for ex in &new_rec.exdates {
                lines.push(format!("{} exdate {}", "+".green(), ex.to_string().green()));
            }
        }
        (Some(old_rec), None) => {
            lines.push(format!("{} rrule {}", "-".red(), old_rec.rrule.red()));
            for ex in &old_rec.exdates {
                lines.push(format!("{} exdate {}", "-".red(), ex.to_string().red()));
            }
        }
        (None, None) => {}
    }

    lines
}
