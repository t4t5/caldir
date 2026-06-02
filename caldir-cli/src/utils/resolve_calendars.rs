use caldir_core::{Caldir, Calendar};

pub fn resolve_calendars(
    caldir: &Caldir,
    calendar_filter: Option<&str>,
) -> Result<Vec<Calendar>, anyhow::Error> {
    let all_calendars: Vec<Calendar> = caldir
        .calendars()
        .into_iter()
        .filter_map(Result::ok)
        .collect();

    match calendar_filter {
        Some(slug) => match all_calendars.into_iter().find(|c| c.slug() == Some(slug)) {
            Some(cal) => Ok(vec![cal]),
            None => {
                let available: Vec<String> = caldir
                    .calendars()
                    .into_iter()
                    .filter_map(Result::ok)
                    .filter_map(|c| c.slug().map(str::to_string))
                    .collect();
                anyhow::bail!(
                    "Calendar '{}' not found. Available: {}",
                    slug,
                    available.join(", ")
                );
            }
        },
        None => Ok(all_calendars),
    }
}
