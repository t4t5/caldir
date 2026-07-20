use std::{collections::HashMap, path::Path};

use sha2::{Digest, Sha256};

use super::CalendarStateError;
use crate::{Event, EventInstanceId};

pub(crate) const EVENT_BASES_DIR_NAME: &str = "bases";

pub(crate) struct EventBases(HashMap<EventInstanceId, Event>);

impl IntoIterator for EventBases {
    type Item = (EventInstanceId, Event);
    type IntoIter = std::collections::hash_map::IntoIter<EventInstanceId, Event>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl EventBases {
    pub(crate) fn write_from<'a>(
        events: impl IntoIterator<Item = &'a Event>,
        path: &Path,
    ) -> Result<(), CalendarStateError> {
        std::fs::create_dir_all(path)?;

        for event in events {
            let filename = hash_filename(&event.event_instance_id().to_string());
            let event_path = path.join(format!("{filename}.ics"));

            // Skip unchanged bases to avoid churning every file on every sync.
            // Parse-compare, not byte-compare: to_ics_string stamps a fresh DTSTAMP.
            if Event::load_single(&event_path).is_ok_and(|existing| &existing == event) {
                continue;
            }

            let mut tmp = tempfile::NamedTempFile::new_in(path)?;
            std::io::Write::write_all(&mut tmp, event.to_ics_string().as_bytes())?;
            tmp.persist(event_path).map_err(|err| err.error)?;
        }

        Ok(())
    }

    pub(crate) fn load(path: &Path) -> Result<Self, CalendarStateError> {
        let mut event_bases = HashMap::new();

        if path.is_dir() {
            for entry in std::fs::read_dir(path)? {
                let entry = entry?;
                let file_path = entry.path();
                let Some(filename) = file_path.file_name().and_then(|name| name.to_str()) else {
                    continue;
                };

                if file_path.is_file() && is_hashed_filename(filename) {
                    // An unreadable base is no base — degrade to the legacy
                    // known-id entry rather than failing the whole load.
                    let Ok(event) = Event::load_single(&file_path) else {
                        continue;
                    };
                    let event_instance_id = event.event_instance_id();
                    let expected_filename =
                        format!("{}.ics", hash_filename(&event_instance_id.to_string()));

                    if filename == expected_filename {
                        event_bases.insert(event_instance_id, event);
                    }
                }
            }
        }

        Ok(Self(event_bases))
    }
}

fn hash_filename(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn is_hashed_filename(filename: &str) -> bool {
    filename.strip_suffix(".ics").is_some_and(|stem| {
        stem.len() == 64
            && stem
                .bytes()
                .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
    })
}

#[cfg(test)]
mod tests {
    use super::{EventBases, hash_filename, is_hashed_filename};
    use crate::test_utils::test_event;

    #[test]
    fn filename_hash_is_fixed_length_and_filesystem_safe() {
        let filename = hash_filename("event@google.com__TZID=Europe/Stockholm:20260630T190000");

        assert_eq!(
            filename,
            "5dc4f5ac8fb72c87d9333e218f4b36d44f5fae5ad3cb888b788bbad2cbaf2148"
        );
        assert_eq!(filename.len(), 64);
    }

    #[test]
    fn write_skips_unchanged_bases_despite_dtstamp_drift() {
        let dir = tempfile::TempDir::new().unwrap();
        let event = test_event();
        EventBases::write_from([&event], dir.path()).unwrap();

        let path = dir.path().join(format!(
            "{}.ics",
            hash_filename(&event.event_instance_id().to_string())
        ));

        // Simulate an earlier sync: same event, older DTSTAMP.
        let restamped: String = std::fs::read_to_string(&path)
            .unwrap()
            .lines()
            .map(|line| {
                if line.starts_with("DTSTAMP:") {
                    "DTSTAMP:20000101T000000Z\r\n".to_string()
                } else {
                    format!("{line}\r\n")
                }
            })
            .collect();
        std::fs::write(&path, &restamped).unwrap();

        EventBases::write_from([&event], dir.path()).unwrap();

        // Untouched: the old DTSTAMP survives.
        assert_eq!(std::fs::read_to_string(&path).unwrap(), restamped);
    }

    #[test]
    fn load_ignores_non_hashed_files() {
        let dir = tempfile::TempDir::new().unwrap();
        let event = test_event();
        std::fs::write(dir.path().join("legacy.ics"), event.to_ics_string()).unwrap();

        let loaded = EventBases::load(dir.path()).unwrap();

        assert_eq!(loaded.into_iter().count(), 0);
    }

    #[test]
    fn corrupt_base_is_skipped_rather_than_failing_the_load() {
        let dir = tempfile::TempDir::new().unwrap();
        let event = test_event();
        EventBases::write_from([&event], dir.path()).unwrap();

        let garbled = format!("{}.ics", hash_filename("garbled"));
        let empty = format!("{}.ics", hash_filename("empty"));
        std::fs::write(dir.path().join(garbled), "not an ics file").unwrap();
        std::fs::write(dir.path().join(empty), "").unwrap();

        let loaded: Vec<_> = EventBases::load(dir.path()).unwrap().into_iter().collect();

        assert_eq!(loaded, vec![(event.event_instance_id(), event)]);
    }

    #[test]
    fn recognizes_only_lowercase_sha256_filenames() {
        assert!(is_hashed_filename(
            "5dc4f5ac8fb72c87d9333e218f4b36d44f5fae5ad3cb888b788bbad2cbaf2148.ics"
        ));
        assert!(!is_hashed_filename("legacy.ics"));
        assert!(!is_hashed_filename(
            "5DC4F5AC8FB72C87D9333E218F4B36D44F5FAE5AD3CB888B788BBAD2CBAF2148.ics"
        ));
    }
}
