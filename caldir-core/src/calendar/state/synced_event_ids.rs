/// We need this to distinguish between:
///
/// Scenario A.
///   1. Local event was created + pushed
///   2. Local is deleted
///   3. Sync should now delete remote!
///
/// Scenario B:
///   1. Remote event was created.
///   2. Sync should now create local!
///
/// In both cases, we only see that remote has events that local doesn't.
/// The `known_event_ids` file helps us see if it's a brand new event, or a previously known one.
use super::CalendarStateError;
use crate::event::EventInstanceId;
use std::{collections::HashSet, path::Path};

// Filename kept as `known_event_ids` for backwards compatibility with existing
// on-disk state, even though the in-code name is `SyncedEventIds`.
pub(crate) const SYNCED_IDS_FILE_NAME: &str = "known_event_ids";

#[derive(Debug)]
pub(crate) struct SyncedEventIds(HashSet<EventInstanceId>);

/// Event instance IDs are stored in plaintext, one per line:
/// e.g.
///   t5slp0vorqgoasogqkvadjt9jj@hooli.com__20240625T170000Z
///   t5slp0vorqgoasogqkvadjt9jj@hooli.com__20240625T180000
///   t81pd0rkq8ujaughbrjhh87svo@hooli.com
impl SyncedEventIds {
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
                .map(EventInstanceId::from)
                .collect::<HashSet<_>>();

            Ok(Self(ids))
        } else {
            Ok(Self::new())
        }
    }

    /// Writes atomically (tempfile + rename) so a kill mid-write can't truncate
    /// the file. Truncation would make the next sync treat every known event
    /// as new and re-create them remotely.
    pub fn write(&self, path: &Path) -> Result<(), CalendarStateError> {
        let contents = self
            .0
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");

        let parent = path.parent().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "synced ids path has no parent directory",
            )
        })?;

        let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
        std::io::Write::write_all(&mut tmp, contents.as_bytes())?;
        tmp.persist(path).map_err(|e| e.error)?;

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
                EventUid::new("t5slp0vorqgoasogqkvadjt9jj@hooli.com".to_string()),
                Some(RecurrenceId::from_event_time(EventTime::DateTimeUtc(
                    Utc.with_ymd_and_hms(2024, 6, 25, 17, 0, 0).unwrap(),
                ))),
            ),
            EventInstanceId::new(
                EventUid::new("t5slp0vorqgoasogqkvadjt9jj@hooli.com".to_string()),
                Some(RecurrenceId::from_event_time(EventTime::DateTimeFloating(
                    NaiveDate::from_ymd_opt(2024, 6, 25)
                        .unwrap()
                        .and_hms_opt(18, 0, 0)
                        .unwrap(),
                ))),
            ),
            EventInstanceId::new(
                EventUid::new("t81pd0rkq8ujaughbrjhh87svo@hooli.com".to_string()),
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

        let loaded = SyncedEventIds::load(&path).unwrap();

        assert_eq!(loaded.0, sample_ids());
    }

    fn zoned_instance(tzid: &str, hour: u32) -> EventInstanceId {
        EventInstanceId::new(
            EventUid::new("podd@google.com".to_string()),
            Some(RecurrenceId::from_event_time(EventTime::DateTimeZoned {
                datetime: NaiveDate::from_ymd_opt(2026, 7, 14)
                    .unwrap()
                    .and_hms_opt(hour, 0, 0)
                    .unwrap(),
                tzid: tzid.to_string(),
            })),
        )
    }

    #[test]
    fn contains_matches_same_instant_across_timezones() {
        // Synced state recorded the instance in one zone; a local override in a
        // different zone for the same instant must still count as "known".
        let path = tempfile::TempDir::new().unwrap();
        let file = path.path().join("known_event_ids");
        std::fs::write(&file, "podd@google.com__TZID=Europe/London:20260714T180000").unwrap();

        let loaded = SyncedEventIds::load(&file).unwrap();

        // 19:00 Stockholm == 18:00 London == 17:00Z — all the same occurrence.
        assert!(loaded.contains(&zoned_instance("Europe/Stockholm", 19)));
        assert!(loaded.contains(&zoned_instance("Europe/London", 18)));
    }

    #[test]
    fn write_replaces_existing_file_contents() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("known_event_ids");
        std::fs::write(&path, "stale-id@example.com").unwrap();

        SyncedEventIds(sample_ids()).write(&path).unwrap();

        let got = std::fs::read_to_string(&path).unwrap();
        assert!(!got.contains("stale-id@example.com"));
        assert!(got.contains("t81pd0rkq8ujaughbrjhh87svo@hooli.com"));
    }

    #[test]
    fn writes_ids_to_plaintext_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("known_event_ids");
        let ids = SyncedEventIds(sample_ids());

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
