use super::CalendarStateError;
use crate::Event;
use crate::event::EventInstanceId;
use std::collections::HashMap;
use std::path::Path;

pub(crate) const SNAPSHOTS_DIR_NAME: &str = "snapshots";

#[derive(Debug)]
pub(crate) struct SyncedSnapshots(HashMap<EventInstanceId, Event>);

impl SyncedSnapshots {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn get(&self, id: &EventInstanceId) -> Option<&Event> {
        self.0.get(id)
    }

    pub fn upsert(&mut self, event: Event) {
        self.0.insert(event.event_instance_id(), event);
    }

    pub fn remove(&mut self, id: &EventInstanceId) {
        self.0.remove(id);
    }

    pub fn load(path: &Path) -> Result<Self, CalendarStateError> {
        if !path.is_dir() {
            return Ok(Self::new());
        }

        let mut snapshots = HashMap::new();
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            if !entry.file_type()?.is_file() || path.extension().is_none_or(|ext| ext != "ics") {
                continue;
            }

            let contents = std::fs::read_to_string(&path)?;
            let mut events = Event::from_ics_str(&contents)
                .map_err(|err| CalendarStateError::InvalidSnapshot(path.clone(), err))?;
            let event = match <[Result<Event, _>; 1]>::try_from(std::mem::take(&mut events)) {
                Ok([result]) => {
                    result.map_err(|err| CalendarStateError::InvalidSnapshot(path.clone(), err))?
                }
                Err(events) => {
                    return Err(CalendarStateError::InvalidSnapshotCount {
                        path,
                        found: events.len(),
                    });
                }
            };
            snapshots.insert(event.event_instance_id(), event);
        }

        Ok(Self(snapshots))
    }

    pub fn write(&self, path: &Path) -> Result<(), CalendarStateError> {
        std::fs::create_dir_all(path)?;

        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            if entry.file_type()?.is_file()
                && entry.path().extension().is_some_and(|ext| ext == "ics")
            {
                std::fs::remove_file(entry.path())?;
            }
        }

        for (id, event) in &self.0 {
            let snapshot_path = path.join(format!("{}.ics", filename_for_id(id)));
            write_atomic(&snapshot_path, event.to_ics_string().as_bytes())?;
        }

        Ok(())
    }
}

fn filename_for_id(id: &EventInstanceId) -> String {
    id.to_string()
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => {
                vec![byte as char]
            }
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}

fn write_atomic(path: &Path, contents: &[u8]) -> Result<(), CalendarStateError> {
    let parent = path.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "snapshot path has no parent directory",
        )
    })?;

    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    std::io::Write::write_all(&mut tmp, contents)?;
    tmp.persist(path).map_err(|e| e.error)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::test_event;

    #[test]
    fn writes_and_loads_snapshots() {
        let tmp = tempfile::TempDir::new().unwrap();
        let event = test_event();
        let mut snapshots = SyncedSnapshots::new();
        snapshots.upsert(event.clone());

        snapshots.write(tmp.path()).unwrap();
        let loaded = SyncedSnapshots::load(tmp.path()).unwrap();

        assert_eq!(loaded.get(&event.event_instance_id()), Some(&event));
    }

    #[test]
    fn removes_snapshot() {
        let event = test_event();
        let id = event.event_instance_id();
        let mut snapshots = SyncedSnapshots::new();
        snapshots.upsert(event);

        snapshots.remove(&id);

        assert!(snapshots.get(&id).is_none());
    }
}
