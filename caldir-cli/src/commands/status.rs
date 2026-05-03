use anyhow::Result;
use caldir_core::caldir::Caldir;
use caldir_core::calendar::Calendar;
use caldir_core::date_range::DateRange;
use caldir_core::diff::CalendarDiff;
use owo_colors::OwoColorize;

use crate::render::{CalendarDiffRender, Render};
use crate::utils::tui;

pub async fn run(
    caldir: &Caldir,
    calendars: Vec<Calendar>,
    range: DateRange,
    verbose: bool,
) -> Result<()> {
    let environment = caldir.environment();
    for (i, cal) in calendars.iter().enumerate() {
        if cal.remote().is_none() {
            println!("{}", cal.render(environment));
            println!("   {}", "(local only)".dimmed());
        } else {
            let spinner = tui::create_spinner(cal.render(environment));
            let result = CalendarDiff::from_calendar(caldir, cal, &range).await;
            spinner.finish_and_clear();

            println!("{}", cal.render(environment));

            match result {
                Ok(diff) => println!("{}", diff.render(verbose, environment)),
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
