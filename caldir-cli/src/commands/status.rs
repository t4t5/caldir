use anyhow::Result;
use caldir_core::Caldir;
use caldir_core::Calendar;
use caldir_core::CalendarDiff;
use caldir_core::DateRange;
use owo_colors::OwoColorize;

use crate::render::{CalendarDiffRender, Render};
use crate::utils::tui;

pub async fn run(
    caldir: &Caldir,
    calendar: Option<String>,
    from: Option<String>,
    to: Option<String>,
    verbose: bool,
) -> Result<()> {
    require_calendars(&caldir)?;

    let calendars = match calendar {
        Some(cal) => vec![caldir.calendar(&cal)],
        None => caldir.calendars(),
    }?;

    let range =
        DateRange::from_args(from.as_deref(), to.as_deref()).map_err(|e| anyhow::anyhow!(e))?;

    run_parsed(&caldir, calendars, range, verbose).await
}

async fn run_parsed(
    caldir: &Caldir,
    calendars: Vec<Calendar>,
    range: DateRange,
    verbose: bool,
) -> Result<()> {
    for (i, cal) in calendars.iter().enumerate() {
        if cal.remote().is_none() {
            println!("{}", cal.render(caldir));
            println!("   {}", "(local only)".dimmed());
        } else {
            let spinner = tui::create_spinner(cal.render(caldir));
            let result = CalendarDiff::from_calendar(caldir, cal, &range).await;
            spinner.finish_and_clear();

            println!("{}", cal.render(caldir));

            match result {
                Ok(diff) => println!("{}", diff.render(verbose, caldir)),
                Err(e) => println!("   {}", e.to_string().red()),
            }
        }

        // Add spacing between calendars (but not after the last one)
        if i < calendars.len() - 1 {
            println!();
        }
    }

    Ok(())
}
