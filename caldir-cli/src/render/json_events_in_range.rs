use anyhow::Result;
use caldir_core::Calendar;
use chrono::{DateTime, Utc};

use crate::render::events_in_range::{
    collect_visible_expanded_events, group_events_by_display_day,
};
use crate::render::json_event::JsonEvent;

pub fn render_json_events_in_range(
    calendars: Vec<Calendar>,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<()> {
    let range_start = from.with_timezone(&chrono::Local).date_naive();
    let range_end = to.with_timezone(&chrono::Local).date_naive();

    let events = collect_visible_expanded_events(&calendars, from, to)?;
    let entries = group_events_by_display_day(&events, range_start, range_end);
    let json_events: Vec<JsonEvent<'_>> = entries.iter().map(JsonEvent::from).collect();

    serde_json::to_writer(std::io::stdout(), &json_events)?;
    println!();

    Ok(())
}
