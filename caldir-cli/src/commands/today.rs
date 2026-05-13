pub fn run_today(caldir: &Caldir, calendar: Option<String>) -> Result<()> {
    require_calendars(&caldir)?;

    let calendars = resolve_calendars(&caldir, calendar.as_deref())?;

    let today = Local::now().date_naive();

    let end_of_today = today
        .and_hms_opt(23, 59, 59)
        .unwrap()
        .and_local_timezone(Local)
        .unwrap()
        .with_timezone(&Utc);

    run_parsed(&caldir, calendars, from_dt, to_dt)
}
