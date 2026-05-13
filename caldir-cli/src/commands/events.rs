// pub fn run(
//     caldir: &Caldir,
//     calendar: Option<String>,
//     from: Option<String>,
//     to: Option<String>,
// ) -> Result<()> {
//     require_calendars(&caldir)?;
//
//     let calendars = resolve_calendars(&caldir, calendar.as_deref())?;
//
//     // Only parse dates if explicitly provided; events command has its own defaults
//     let from_dt = from
//         .as_deref()
//         .map(parse_date_start)
//         .transpose()
//         .map_err(|e| anyhow::anyhow!(e))?;
//
//     let to_dt = to
//         .as_deref()
//         .map(parse_date_end)
//         .transpose()
//         .map_err(|e| anyhow::anyhow!(e))?;
//
//     render_events_in_range(&caldir, calendars, from_dt, to_dt)
// }
