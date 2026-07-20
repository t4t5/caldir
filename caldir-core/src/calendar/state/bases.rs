use super::CalendarStateError;
use crate::{Event, event::EventInstanceId};
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub(crate) const BASES_DIR_NAME: &str = "bases";

#[derive(Debug)]
pub(crate) struct EventBases {
    events: HashMap<EventInstanceId, Event>,
    upserted: HashSet<EventInstanceId>,
    removed: HashSet<EventInstanceId>,
}

impl EventBases {
    pub fn new() -> Self {
        Self {
            events: HashMap::new(),
            upserted: HashSet::new(),
            removed: HashSet::new(),
        }
    }

    pub fn get(&self, id: &EventInstanceId) -> Option<&Event> {
        self.events.get(id)
    }

    pub fn ids(&self) -> impl Iterator<Item = &EventInstanceId> {
        self.events.keys()
    }

    pub fn upsert(&mut self, event: Event) {
        let id = event.event_instance_id();
        if self
            .events
            .get(&id)
            .is_some_and(|existing| existing.same_snapshot(&event))
        {
            return;
        }

        self.events.insert(id.clone(), event);
        self.removed.remove(&id);
        self.upserted.insert(id);
    }

    pub fn remove(&mut self, id: &EventInstanceId) {
        if self.events.remove(id).is_some() {
            self.upserted.remove(id);
            self.removed.insert(id.clone());
        }
    }

    pub fn load(path: &Path) -> Result<Self, CalendarStateError> {
        if !path.is_dir() {
            return Ok(Self::new());
        }

        let mut events = HashMap::new();
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            if !entry.file_type()?.is_file() || path.extension().is_none_or(|ext| ext != "ics") {
                continue;
            }

            // An unreadable base is *no base*: bases are regenerable, so one
            // corrupt file must not block the sync. Degrades to the no-base
            // path (possible resurrection), never to delete-propagation.
            if let Some(event) = load_base(&path) {
                events.insert(event.event_instance_id(), event);
            }
        }

        Ok(Self {
            events,
            upserted: HashSet::new(),
            removed: HashSet::new(),
        })
    }

    pub fn write(&mut self, path: &Path) -> Result<(), CalendarStateError> {
        if self.upserted.is_empty() && self.removed.is_empty() {
            return Ok(());
        }

        std::fs::create_dir_all(path)?;

        // Each id clears only after its write lands, so an error mid-loop
        // leaves the rest pending rather than silently dropping them.
        for id in self.upserted.iter().cloned().collect::<Vec<_>>() {
            let event = &self.events[&id];
            write_atomic(&base_path(path, &id), event.to_ics_string().as_bytes())?;
            self.upserted.remove(&id);
        }

        for id in self.removed.iter().cloned().collect::<Vec<_>>() {
            if let Err(err) = std::fs::remove_file(base_path(path, &id))
                && err.kind() != std::io::ErrorKind::NotFound
            {
                return Err(err.into());
            }
            self.removed.remove(&id);
        }

        Ok(())
    }
}

fn base_path(dir: &Path, id: &EventInstanceId) -> std::path::PathBuf {
    dir.join(format!("{}.ics", filename_for_id(id)))
}

/// `None` for anything we can't read back as exactly one event.
fn load_base(path: &Path) -> Option<Event> {
    let contents = std::fs::read_to_string(path).ok()?;
    let mut events = Event::from_ics_str(&contents).ok()?.into_iter();

    match (events.next(), events.next()) {
        (Some(Ok(event)), None) => Some(event),
        _ => None,
    }
}

/// Percent-encodes anything outside a safe filename set. Never decoded —
/// identity always comes from the file's contents, not its name.
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
            "event base path has no parent directory",
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
    fn writes_and_loads_bases() {
        let tmp = tempfile::TempDir::new().unwrap();
        let event = test_event();
        let mut bases = EventBases::new();
        bases.upsert(event.clone());

        bases.write(tmp.path()).unwrap();
        let loaded = EventBases::load(tmp.path()).unwrap();

        assert_eq!(loaded.get(&event.event_instance_id()), Some(&event));
    }

    #[test]
    fn removes_base() {
        let event = test_event();
        let id = event.event_instance_id();
        let mut bases = EventBases::new();
        bases.upsert(event);

        bases.remove(&id);

        assert!(bases.get(&id).is_none());
    }

    #[test]
    fn no_op_write_leaves_base_untouched() {
        let tmp = tempfile::TempDir::new().unwrap();
        let event = test_event();
        let id = event.event_instance_id();
        let mut bases = EventBases::new();
        bases.upsert(event);
        bases.write(tmp.path()).unwrap();

        let path = base_path(tmp.path(), &id);
        let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
        let mut loaded = EventBases::load(tmp.path()).unwrap();
        loaded.write(tmp.path()).unwrap();

        assert_eq!(
            std::fs::metadata(path).unwrap().modified().unwrap(),
            modified
        );
    }

    #[test]
    fn corrupt_base_is_skipped_rather_than_failing_the_load() {
        let tmp = tempfile::TempDir::new().unwrap();
        let event = test_event();
        let mut bases = EventBases::new();
        bases.upsert(event.clone());
        bases.write(tmp.path()).unwrap();
        std::fs::write(tmp.path().join("zero-byte.ics"), "").unwrap();
        std::fs::write(tmp.path().join("garbled.ics"), "not an ics file").unwrap();

        let loaded = EventBases::load(tmp.path()).unwrap();

        assert_eq!(loaded.get(&event.event_instance_id()), Some(&event));
        assert_eq!(loaded.ids().count(), 1);
    }
}
