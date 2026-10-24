use caldir_core::{Caldir, Calendar};

/// Calendars matching `slugs`, or every calendar when `slugs` is empty.
pub fn resolve_calendars(
    caldir: &Caldir,
    slugs: &[String],
) -> Result<Vec<Calendar>, anyhow::Error> {
    let all_calendars: Vec<Calendar> = caldir
        .calendars()
        .into_iter()
        .filter_map(Result::ok)
        .collect();

    select_calendars(all_calendars, slugs)
}

fn select_calendars(
    all_calendars: Vec<Calendar>,
    slugs: &[String],
) -> Result<Vec<Calendar>, anyhow::Error> {
    if slugs.is_empty() {
        return Ok(all_calendars);
    }

    let missing: Vec<&str> = slugs
        .iter()
        .map(String::as_str)
        .filter(|slug| !all_calendars.iter().any(|c| c.slug() == Some(*slug)))
        .collect();

    if !missing.is_empty() {
        let available: Vec<&str> = all_calendars.iter().filter_map(Calendar::slug).collect();
        anyhow::bail!(
            "{} '{}' not found. Available: {}",
            if missing.len() == 1 {
                "Calendar"
            } else {
                "Calendars"
            },
            missing.join("', '"),
            available.join(", ")
        );
    }

    Ok(all_calendars
        .into_iter()
        .filter(|c| c.slug().is_some_and(|slug| slugs.iter().any(|s| s == slug)))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn calendars(dir: &std::path::Path) -> Vec<Calendar> {
        ["home", "work", "rooms"]
            .iter()
            .map(|slug| Calendar::create(&dir.join(slug), None).unwrap())
            .collect()
    }

    fn slugs(calendars: &[Calendar]) -> Vec<&str> {
        calendars.iter().filter_map(Calendar::slug).collect()
    }

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn no_slugs_selects_every_calendar() {
        let tmp = tempfile::tempdir().unwrap();

        let selected = select_calendars(calendars(tmp.path()), &[]).unwrap();

        assert_eq!(slugs(&selected), vec!["home", "work", "rooms"]);
    }

    #[test]
    fn selects_each_named_calendar() {
        let tmp = tempfile::tempdir().unwrap();

        let selected =
            select_calendars(calendars(tmp.path()), &strings(&["work", "home", "work"])).unwrap();

        assert_eq!(slugs(&selected), vec!["home", "work"]);
    }

    #[test]
    fn errors_with_every_unknown_slug() {
        let tmp = tempfile::tempdir().unwrap();

        let err = select_calendars(calendars(tmp.path()), &strings(&["home", "nope", "typo"]))
            .unwrap_err();

        assert_eq!(
            err.to_string(),
            "Calendars 'nope', 'typo' not found. Available: home, work, rooms"
        );
    }
}
