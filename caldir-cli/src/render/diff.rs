use crate::render::time::format_datetime;
use caldir_core::{
    Attendee, Caldir, Calendar, CalendarDiff, EventChange, Recurrence, Reminder, TimeFormat,
    XProperty,
};
use owo_colors::OwoColorize;
use std::collections::{HashMap, HashSet};
use std::fmt;

/// Extension trait for TUI rendering with colors. Implementations that format
/// event times take the caldir context explicitly, so rendering has no global
/// config dependency.
pub trait Render {
    fn render(&self, caldir: &Caldir) -> String;
}

fn event_change_symbol(kind: &EventChange) -> &str {
    match kind {
        EventChange::Create(_) => "+",
        EventChange::Update { .. } => "~",
        EventChange::Delete(_) => "-",
    }
}

/// Colorize text according to the diff kind
fn colorize_diff(kind: &EventChange, text: &str) -> String {
    match kind {
        EventChange::Create(_) => text.green().to_string(),
        EventChange::Update { .. } => text.yellow().to_string(),
        EventChange::Delete(_) => text.red().to_string(),
    }
}

impl Render for EventChange {
    fn render(&self, caldir: &Caldir) -> String {
        let event = match self {
            EventChange::Create(event) | EventChange::Delete(event) => event,
            EventChange::Update { to, .. } => to,
        };

        let summary = colorize_diff(self, event.summary().unwrap_or("(Untitled)"));

        let time = format_datetime(&event.start, caldir.config().time_format());

        let recurring = if event.recurrence.is_some() {
            " 🔁"
        } else {
            ""
        };

        format!(
            "{} {} {}{}",
            colorize_diff(self, event_change_symbol(self)),
            summary,
            time.dimmed(),
            recurring
        )
    }
}

impl Render for Calendar {
    fn render(&self, _caldir: &Caldir) -> String {
        format!("📅 {}", self.slug().unwrap_or(""))
    }
}

/// Threshold for compact view (show counts instead of individual events)
const COMPACT_THRESHOLD: usize = 5;

/// Render a list of diffs, using compact view if there are many events and verbose is false
fn render_diff_list(
    diffs: &[EventChange],
    verbose: bool,
    caldir: &Caldir,
    lines: &mut Vec<String>,
) {
    if verbose || diffs.len() <= COMPACT_THRESHOLD {
        // Full view: show each event
        for diff in diffs {
            lines.push(format!("   {}", diff.render(caldir)));
            // Always show field diffs for updates when in full view
            if let EventChange::Update { .. } = diff {
                lines.extend(
                    render_field_diffs(diff, caldir)
                        .into_iter()
                        .map(|l| format!("     {}", l)),
                );
            }
        }
    } else {
        // Compact view: show counts by diff kind
        let creates = diffs
            .iter()
            .filter(|d| matches!(d, EventChange::Create(_)))
            .count();

        let updates = diffs
            .iter()
            .filter(|d| matches!(d, EventChange::Update { .. }))
            .count();

        let deletes = diffs
            .iter()
            .filter(|d| matches!(d, EventChange::Delete(_)))
            .count();

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
    fn render(&self, verbose: bool, caldir: &Caldir) -> String;
    fn render_pull(&self, verbose: bool, caldir: &Caldir) -> String;
    fn render_push(&self, verbose: bool, caldir: &Caldir) -> String;
    fn render_discard(&self, verbose: bool, caldir: &Caldir) -> String;
}

fn render_bidirectional(
    diff: &CalendarDiff,
    verbose: bool,
    caldir: &Caldir,
    push_label: &str,
    pull_label: &str,
) -> String {
    if diff.is_empty() {
        return "   No changes".dimmed().to_string();
    }

    let mut lines = Vec::new();

    if !diff.outgoing().is_empty() {
        lines.push(format!("   {}:", push_label).dimmed().to_string());
        render_diff_list(diff.outgoing(), verbose, caldir, &mut lines);
    }

    if !diff.incoming().is_empty() {
        if !diff.outgoing().is_empty() {
            lines.push(String::new());
        }
        lines.push(format!("   {}:", pull_label).dimmed().to_string());
        render_diff_list(diff.incoming(), verbose, caldir, &mut lines);
    }

    lines.join("\n")
}

impl CalendarDiffRender for CalendarDiff {
    fn render(&self, verbose: bool, caldir: &Caldir) -> String {
        render_bidirectional(
            self,
            verbose,
            caldir,
            "Local changes (to push)",
            "Remote changes (to pull)",
        )
    }

    fn render_pull(&self, verbose: bool, caldir: &Caldir) -> String {
        if self.incoming().is_empty() {
            return "   No changes to pull".dimmed().to_string();
        }

        let mut lines = Vec::new();
        render_diff_list(self.incoming(), verbose, caldir, &mut lines);
        lines.join("\n")
    }

    fn render_push(&self, verbose: bool, caldir: &Caldir) -> String {
        if self.outgoing().is_empty() {
            return "   No changes to push".dimmed().to_string();
        }

        let mut lines = Vec::new();
        render_diff_list(self.outgoing(), verbose, caldir, &mut lines);
        lines.join("\n")
    }

    fn render_discard(&self, verbose: bool, caldir: &Caldir) -> String {
        if self.outgoing().is_empty() {
            return "   Nothing to discard".dimmed().to_string();
        }

        let mut lines = Vec::new();
        lines.push(
            format!("   {}:", "Local changes (to discard)")
                .dimmed()
                .to_string(),
        );
        render_diff_list(self.outgoing(), verbose, caldir, &mut lines);
        lines.join("\n")
    }
}

/// Render field-by-field differences for an EventDiff (only for updates)
fn render_field_diffs(diff: &EventChange, caldir: &Caldir) -> Vec<String> {
    let mut lines = Vec::new();
    let time_format = caldir.config().time_format();

    // Only show field diffs for updates
    if let EventChange::Update { from: old, to: new } = diff {
        if old.summary != new.summary {
            lines.push(format!(
                "{}: {} → {}",
                "summary".dimmed(),
                old.summary.as_deref().unwrap_or("(Untitled)").red(),
                new.summary.as_deref().unwrap_or("(Untitled)").green()
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
                format_datetime(&old.start, time_format).red(),
                format_datetime(&new.start, time_format).green()
            ));
        }
        if old.end != new.end {
            lines.push(format!(
                "{}: {} → {}",
                "end".dimmed(),
                old.end
                    .as_ref()
                    .map_or("(none)".into(), |e| format_datetime(e, time_format))
                    .red(),
                new.end
                    .as_ref()
                    .map_or("(none)".into(), |e| format_datetime(e, time_format))
                    .green()
            ));
        }
        if old.status != new.status {
            lines.push(render_display("status", &old.status, &new.status));
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
            let reminder_lines = render_reminder_diffs(&old.reminders, &new.reminders);
            if !reminder_lines.is_empty() {
                lines.push(format!("{}:", "reminders".dimmed()));
                lines.extend(reminder_lines.into_iter().map(|l| format!("  {}", l)));
            }
        }
        if old.transparency != new.transparency {
            lines.push(render_display(
                "transparency",
                &old.transparency,
                &new.transparency,
            ));
        }
        if old.organizer != new.organizer {
            lines.push(render_optional_display(
                "organizer",
                &old.organizer,
                &new.organizer,
            ));
        }
        if old.attendees != new.attendees {
            let attendee_lines = render_attendee_diffs(&old.attendees, &new.attendees);
            if !attendee_lines.is_empty() {
                lines.push(format!("{}:", "attendees".dimmed()));
                lines.extend(attendee_lines.into_iter().map(|l| format!("  {}", l)));
            }
        }
        if old.url != new.url {
            lines.push(render_optional_diff("url", &old.url, &new.url));
        }
        let xprop_lines = render_x_property_diffs(&old.x_properties, &new.x_properties);
        if !xprop_lines.is_empty() {
            lines.push(format!("{}:", "x-properties".dimmed()));
            lines.extend(xprop_lines.into_iter().map(|l| format!("  {}", l)));
        }
    }

    lines
}

// Includes attributes on the property, not just the value
fn render_x_property_diffs(old: &[XProperty], new: &[XProperty]) -> Vec<String> {
    let mut lines = Vec::new();

    let old_by_name: HashMap<&str, &XProperty> = old.iter().map(|p| (p.name.as_str(), p)).collect();
    let new_by_name: HashMap<&str, &XProperty> = new.iter().map(|p| (p.name.as_str(), p)).collect();

    for (name, old_p) in &old_by_name {
        if let Some(new_p) = new_by_name.get(name)
            && old_p != new_p
        {
            lines.extend(render_x_property_change(name, old_p, new_p));
        }
    }

    for (name, new_p) in &new_by_name {
        if !old_by_name.contains_key(name) {
            lines.push(format!(
                "{} {} {}",
                "+".green(),
                name.green(),
                new_p.value.dimmed()
            ));
        }
    }

    for (name, old_p) in &old_by_name {
        if !new_by_name.contains_key(name) {
            lines.push(format!(
                "{} {} {}",
                "-".red(),
                name.red(),
                old_p.value.dimmed()
            ));
        }
    }

    lines
}

fn render_x_property_change(name: &str, old: &XProperty, new: &XProperty) -> Vec<String> {
    let mut lines = Vec::new();
    let param_lines = render_x_property_param_diffs(&old.params, &new.params);

    if old.value != new.value {
        lines.push(format!(
            "{}: {} → {}",
            name.dimmed(),
            old.value.red(),
            new.value.green()
        ));
    } else if !param_lines.is_empty() {
        lines.push(format!("{}:", name.dimmed()));
    }

    lines.extend(param_lines.into_iter().map(|l| format!("  {}", l)));
    lines
}

fn render_x_property_param_diffs(
    old: &[(String, String)],
    new: &[(String, String)],
) -> Vec<String> {
    let mut lines = Vec::new();

    let old_params: HashMap<&str, &str> =
        old.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let new_params: HashMap<&str, &str> =
        new.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();

    for (k, old_v) in &old_params {
        if let Some(new_v) = new_params.get(k)
            && new_v != old_v
        {
            lines.push(format!(
                "{}: {} → {}",
                k.dimmed(),
                old_v.red(),
                new_v.green()
            ));
        }
    }

    for (k, new_v) in &new_params {
        if !old_params.contains_key(k) {
            lines.push(format!("{} {}={}", "+".green(), k.green(), new_v.dimmed()));
        }
    }

    for (k, old_v) in &old_params {
        if !new_params.contains_key(k) {
            lines.push(format!("{} {}={}", "-".red(), k.red(), old_v.dimmed()));
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

/// Render an Option<T: Display> field diff with red/green colors and a
/// `(none)` fallback. Used for structured fields with Display (organizer).
fn render_optional_display<T: fmt::Display>(
    field: &str,
    old: &Option<T>,
    new: &Option<T>,
) -> String {
    let old_str = old.as_ref().map_or("(none)".to_string(), |v| v.to_string());
    let new_str = new.as_ref().map_or("(none)".to_string(), |v| v.to_string());
    format!(
        "{}: {} → {}",
        field.dimmed(),
        old_str.red(),
        new_str.green()
    )
}

/// Render a `T: Display` field diff with red/green colors. Used for enum-like
/// fields with defaults (status, transparency).
fn render_display<T: fmt::Display>(field: &str, old: &T, new: &T) -> String {
    format!(
        "{}: {} → {}",
        field.dimmed(),
        old.to_string().red(),
        new.to_string().green()
    )
}

/// Render reminder changes as add/remove lines (similar to attendee diffs).
fn render_reminder_diffs(old: &[Reminder], new: &[Reminder]) -> Vec<String> {
    let mut lines = Vec::new();

    let old_set: HashSet<i64> = old.iter().map(|r| r.minutes_before_start).collect();
    let new_set: HashSet<i64> = new.iter().map(|r| r.minutes_before_start).collect();

    for added in new
        .iter()
        .filter(|r| !old_set.contains(&r.minutes_before_start))
    {
        lines.push(format!("{} {}", "+".green(), added.to_string().green()));
    }

    for removed in old
        .iter()
        .filter(|r| !new_set.contains(&r.minutes_before_start))
    {
        lines.push(format!("{} {}", "-".red(), removed.to_string().red()));
    }

    lines
}

/// Render attendee changes, showing only what actually changed per attendee
fn render_attendee_diffs(old: &[Attendee], new: &[Attendee]) -> Vec<String> {
    let mut lines = Vec::new();

    let old_by_email: HashMap<String, &Attendee> =
        old.iter().map(|a| (a.email.to_lowercase(), a)).collect();

    let new_by_email: HashMap<String, &Attendee> =
        new.iter().map(|a| (a.email.to_lowercase(), a)).collect();

    // Attendees in both old and new — check for status changes
    for (email, old_att) in &old_by_email {
        if let Some(new_att) = new_by_email.get(email)
            && old_att.status != new_att.status
        {
            let label = attendee_label(new_att);
            let old_status = old_att
                .status
                .map_or("(none)".to_string(), |s| s.to_string());

            let new_status = new_att
                .status
                .map_or("(none)".to_string(), |s| s.to_string());

            lines.push(format!(
                "{}: {} → {}",
                label.dimmed(),
                old_status.red(),
                new_status.green()
            ));
        }
    }

    // Added attendees
    for (email, att) in &new_by_email {
        if !old_by_email.contains_key(email) {
            lines.push(format!("{} {}", "+".green(), attendee_label(att).green()));
        }
    }

    // Removed attendees
    for (email, att) in &old_by_email {
        if !new_by_email.contains_key(email) {
            lines.push(format!("{} {}", "-".red(), attendee_label(att).red()));
        }
    }

    lines
}

/// Format an attendee as "Name (email)" or just "email"
fn attendee_label(att: &Attendee) -> String {
    match &att.name {
        Some(name) if !name.is_empty() => format!("{} ({})", name, att.email),
        _ => att.email.clone(),
    }
}

/// Render recurrence diff showing RRULE and EXDATE changes
fn render_recurrence_diff(old: &Option<Recurrence>, new: &Option<Recurrence>) -> Vec<String> {
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
            let old_set: HashSet<_> = old_rec
                .exdates
                .iter()
                .map(|e| format_datetime(e, TimeFormat::H24))
                .collect();

            let new_set: HashSet<_> = new_rec
                .exdates
                .iter()
                .map(|e| format_datetime(e, TimeFormat::H24))
                .collect();

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
                lines.push(format!(
                    "{} exdate {}",
                    "+".green(),
                    format_datetime(ex, TimeFormat::H24).green()
                ));
            }
        }
        (Some(old_rec), None) => {
            lines.push(format!("{} rrule {}", "-".red(), old_rec.rrule.red()));
            for ex in &old_rec.exdates {
                lines.push(format!(
                    "{} exdate {}",
                    "-".red(),
                    format_datetime(ex, TimeFormat::H24).red()
                ));
            }
        }
        (None, None) => {}
    }

    lines
}
