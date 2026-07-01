mod doctor_warning;
mod event_warning;

use std::io::{self, Write};

use anyhow::Result;
use caldir_core::{Caldir, Calendar};
use owo_colors::OwoColorize;

use crate::render::diff::Render;
use crate::utils::require_calendars;
use doctor_warning::DoctorWarning;
use event_warning::event_warnings;

/// Checks local caldir for bad calendar data:
pub fn run(caldir: &Caldir) -> Result<()> {
    require_calendars(caldir)?;

    let reports = calendar_reports(caldir);
    let mut out = io::stdout().lock();

    render(&mut out, caldir, &reports)
}

fn render(out: &mut impl Write, caldir: &Caldir, reports: &[CalendarReport]) -> Result<()> {
    let warning_count: usize = reports.iter().map(|report| report.warnings.len()).sum();

    for report in reports.iter().filter(|report| !report.warnings.is_empty()) {
        writeln!(out, "{}", report.calendar.render(caldir))?;
        for warning in &report.warnings {
            warning.render(out)?;
        }
        writeln!(out)?;
    }

    if warning_count == 0 {
        writeln!(out, "{} No problems found.", "✓".green())?;
    }

    Ok(())
}

#[derive(Debug)]
struct CalendarReport {
    calendar: Calendar,
    warnings: Vec<DoctorWarning>,
}

fn calendar_reports(caldir: &Caldir) -> Vec<CalendarReport> {
    caldir
        .calendars()
        .into_iter()
        .filter_map(Result::ok)
        .map(calendar_report)
        .collect()
}

fn calendar_report(calendar: Calendar) -> CalendarReport {
    let warnings = match calendar.events() {
        Ok(events) => event_warnings(&events),
        Err(err) => vec![DoctorWarning::UnreadableEvents(err.to_string())],
    };

    CalendarReport { calendar, warnings }
}
