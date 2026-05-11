use super::CalendarStateError;
use crate::event::EventInstanceId;
use std::{collections::HashSet, path::Path};

// ~/caldir/my_calendar/.caldir/state/known_event_ids
pub(crate) const KNOWN_IDS_FILE_NAME: &str = "known_event_ids";

#[derive(Debug)]
pub(crate) struct KnownEventIds(HashSet<EventInstanceId>);

/// Event instance IDs are stored in plaintext, one per line:
/// e.g.
///   t5slp0vorqgoasogqkvadjt9jj@hooli.com__20240625T170000Z
///   t5slp0vorqgoasogqkvadjt9jj@hooli.com__20240625T180000
///   t81pd0rkq8ujaughbrjhh87svo@hooli.com
impl KnownEventIds {
    pub fn new() -> Self {
        Self(HashSet::new())
    }

    pub fn insert(&mut self, id: EventInstanceId) {
        self.0.insert(id);
    }

    pub fn contains(&self, id: &EventInstanceId) -> bool {
        self.0.contains(id)
    }

    pub fn load(path: &Path) -> Result<Self, CalendarStateError> {
        if path.is_file() {
            let contents = std::fs::read_to_string(path)?;

            let ids = contents
                .lines()
                .map(|line| line.trim())
                .filter(|line| !line.is_empty())
                .map(EventInstanceId::from_str)
                .collect::<Result<HashSet<_>, _>>()?;

            Ok(Self(ids))
        } else {
            Ok(Self::new())
        }
    }

    pub fn write(&self, path: &Path) -> Result<(), CalendarStateError> {
        let contents = self
            .0
            .iter()
            .map(|id| id.to_str())
            .collect::<Vec<_>>()
            .join("\n");

        std::fs::write(path, contents)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EventTime;
    use crate::event::{EventUid, RecurrenceId};
    use chrono::{NaiveDate, TimeZone, Utc};
    use pretty_assertions::assert_eq;

    fn sample_ids() -> HashSet<EventInstanceId> {
        HashSet::from([
            EventInstanceId::new(
                EventUid::from_str("t5slp0vorqgoasogqkvadjt9jj@hooli.com".to_string()),
                Some(RecurrenceId::from_event_time(EventTime::DateTimeUtc(
                    Utc.with_ymd_and_hms(2024, 6, 25, 17, 0, 0).unwrap(),
                ))),
            ),
            EventInstanceId::new(
                EventUid::from_str("t5slp0vorqgoasogqkvadjt9jj@hooli.com".to_string()),
                Some(RecurrenceId::from_event_time(EventTime::DateTimeFloating(
                    NaiveDate::from_ymd_opt(2024, 6, 25)
                        .unwrap()
                        .and_hms_opt(18, 0, 0)
                        .unwrap(),
                ))),
            ),
            EventInstanceId::new(
                EventUid::from_str("t81pd0rkq8ujaughbrjhh87svo@hooli.com".to_string()),
                None,
            ),
        ])
    }

    fn sample_lines() -> Vec<&'static str> {
        vec![
            "t5slp0vorqgoasogqkvadjt9jj@hooli.com__20240625T170000Z",
            "t5slp0vorqgoasogqkvadjt9jj@hooli.com__20240625T180000",
            "t81pd0rkq8ujaughbrjhh87svo@hooli.com",
        ]
    }

    #[test]
    fn loads_ids_from_plaintext_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("known_event_ids");
        std::fs::write(&path, sample_lines().join("\n")).unwrap();

        let loaded = KnownEventIds::load(&path).unwrap();

        assert_eq!(loaded.0, sample_ids());
    }

    #[test]
    fn writes_ids_to_plaintext_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("known_event_ids");
        let ids = KnownEventIds(sample_ids());

        ids.write(&path).unwrap();

        let mut got: Vec<String> = std::fs::read_to_string(&path)
            .unwrap()
            .lines()
            .map(str::to_string)
            .collect();

        got.sort();

        let mut expected: Vec<String> = sample_lines().into_iter().map(str::to_string).collect();

        expected.sort();

        assert_eq!(got, expected);
    }
}
