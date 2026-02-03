use anyhow::Result;
use caldir_core::calendar::Calendar;
use caldir_core::date_range::DateRange;
use caldir_core::diff::CalendarDiff;
use owo_colors::OwoColorize;

use crate::render::{CalendarDiffRender, Render};
use crate::utils::tui;

pub async fn run(calendars: Vec<Calendar>, range: DateRange, verbose: bool) -> Result<()> {

    for (i, cal) in calendars.iter().enumerate() {
        let spinner = tui::create_spinner(cal.render());
        let result = CalendarDiff::from_calendar(cal, &range).await;
        spinner.finish_and_clear();

        println!("{}", cal.render());

        match result {
            Ok(diff) => println!("{}", diff.render(verbose)),
            Err(e) => println!("   {}", e.to_string().red()),
        }

        // Add spacing between calendars (but not after the last one)
        if i < calendars.len() - 1 {
            println!();
        }
    }

    Ok(())
}
