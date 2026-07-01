use crate::commands::doctor::warning::DoctorWarning;
use caldir_core::{CalendarEvent, EventInstanceId};
use std::collections::HashMap;
use std::path::PathBuf;

pub(crate) fn duplicate_file_warnings(events: &[CalendarEvent]) -> Vec<DoctorWarning> {
    duplicate_file_sets(events)
        .into_iter()
        .map(DoctorWarning::DuplicateFiles)
        .collect()
}

fn duplicate_file_sets(events: &[CalendarEvent]) -> Vec<Vec<PathBuf>> {
    let mut by_id: HashMap<EventInstanceId, Vec<PathBuf>> = HashMap::new();
    for ce in events {
        by_id
            .entry(ce.event().event_instance_id())
            .or_default()
            .push(ce.path().to_path_buf());
    }

    let mut sets: Vec<Vec<PathBuf>> = by_id
        .into_values()
        .filter(|paths| paths.len() > 1)
        .collect();

    for paths in &mut sets {
        paths.sort();
    }
    sets.sort();
    sets
}

#[cfg(test)]
mod tests {
    use super::duplicate_file_warnings;
    use crate::commands::doctor::warning::DoctorWarning;
    use caldir_core::{Calendar, Event, EventTime};
    use chrono::NaiveDate;
    use pretty_assertions::assert_eq;

    fn test_calendar() -> (tempfile::TempDir, Calendar) {
        let tmp = tempfile::tempdir().unwrap();
        let calendar = Calendar::create(&tmp.path().join("work"), None).unwrap();
        (tmp, calendar)
    }

    fn test_event(summary: &str) -> Event {
        Event::new(
            summary,
            EventTime::DateTimeFloating(
                NaiveDate::from_ymd_opt(2026, 1, 1)
                    .unwrap()
                    .and_hms_opt(12, 0, 0)
                    .unwrap(),
            ),
        )
    }

    #[test]
    fn warns_about_multiple_files_with_the_same_event_identity() {
        let (_tmp, calendar) = test_calendar();
        let event = test_event("Standup");

        calendar.create_event(event.clone()).unwrap();
        calendar.create_event(event).unwrap();

        let warnings = duplicate_file_warnings(&calendar.events().unwrap());

        assert_eq!(warnings.len(), 1);
        let DoctorWarning::DuplicateFiles(paths) = &warnings[0] else {
            panic!("expected duplicate file warning");
        };
        assert_eq!(paths.len(), 2);
    }

    #[test]
    fn ignores_distinct_events_with_the_same_filename_slug() {
        let (_tmp, calendar) = test_calendar();

        calendar.create_event(test_event("Standup")).unwrap();
        calendar.create_event(test_event("Standup")).unwrap();

        let warnings = duplicate_file_warnings(&calendar.events().unwrap());

        assert!(warnings.is_empty());
    }
}
