mod error;
mod event_bases;
mod known_event_ids;

use crate::{Event, calendar::state::event_bases::EventBases, event::EventInstanceId};
pub use error::CalendarStateError;
use known_event_ids::KNOWN_IDS_FILE_NAME;
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

pub(crate) use known_event_ids::KnownEventIds;

#[derive(Debug)]
pub struct CalendarState {
    sync_bases: HashMap<EventInstanceId, SyncBase>,
}

#[derive(Debug)]
enum SyncBase {
    KnownEventId,          // legacy "known_event_ids" file
    EventBase(Box<Event>), // new format for 3-way sync
}

impl CalendarState {
    pub fn new() -> Self {
        Self {
            sync_bases: HashMap::new(),
        }
    }

    pub fn load(state_dir: &Path) -> Result<Self, CalendarStateError> {
        let known_event_ids = Self::load_known_event_ids(state_dir)?;
        let event_bases = Self::load_event_bases(state_dir)?;

        let mut sync_bases = known_event_ids
            .iter()
            .map(|id| (id.clone(), SyncBase::KnownEventId))
            .collect::<HashMap<_, _>>();

        for (id, event) in event_bases {
            sync_bases.insert(id, SyncBase::EventBase(Box::new(event)));
        }

        Ok(Self { sync_bases })
    }

    fn load_known_event_ids(state_dir: &Path) -> Result<KnownEventIds, CalendarStateError> {
        let known_ids_path = state_dir.join(KNOWN_IDS_FILE_NAME);
        KnownEventIds::load(&known_ids_path)
    }

    fn load_event_bases(state_dir: &Path) -> Result<EventBases, CalendarStateError> {
        let event_bases_dir = state_dir.join(event_bases::EVENT_BASES_DIR_NAME);
        let event_bases = event_bases::EventBases::load(&event_bases_dir)?;
        Ok(event_bases)
    }

    pub(crate) fn synced_event_ids(&self) -> HashSet<EventInstanceId> {
        self.sync_bases
            .iter()
            .map(|(id, sync_base)| match sync_base {
                SyncBase::KnownEventId => id.clone(),
                SyncBase::EventBase(event) => event.event_instance_id(),
            })
            .collect()
    }

    pub(crate) fn known_event_ids(&self) -> KnownEventIds {
        let ids = self
            .sync_bases
            .iter()
            .filter(|&(_id, sync_base)| matches!(sync_base, SyncBase::KnownEventId))
            .map(|(id, _sync_base)| id.clone())
            .collect();

        KnownEventIds::from(ids)
    }

    // Todo: change to "save"? and store event bases too?
    pub fn write(&self, state_dir: &Path) -> Result<(), CalendarStateError> {
        std::fs::create_dir_all(state_dir)?;

        let known_ids_path = state_dir.join(KNOWN_IDS_FILE_NAME);

        self.known_event_ids().write(&known_ids_path)?;

        Ok(())
    }

    pub(crate) fn add_new_synced_ids(
        &mut self,
        ids: impl IntoIterator<Item = EventInstanceId>,
    ) -> &mut Self {
        for id in ids {
            self.sync_bases.entry(id).or_insert(SyncBase::KnownEventId);
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn loads_synced_event_ids_from_state_dir() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join(KNOWN_IDS_FILE_NAME), "abc@hooli.com").unwrap();

        CalendarState::load(dir.path()).unwrap();
    }

    #[test]
    fn event_bases_take_priority_over_known_event_ids() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(
            dir.path().join(KNOWN_IDS_FILE_NAME),
            "base@caldir\nfallback@caldir",
        )
        .unwrap();
        let bases_dir = dir.path().join(event_bases::EVENT_BASES_DIR_NAME);
        std::fs::create_dir(&bases_dir).unwrap();
        std::fs::write(
            bases_dir.join("base.ics"),
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:base@caldir\r\nDTSTART:20240101T120000Z\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
        )
        .unwrap();

        let state = CalendarState::load(dir.path()).unwrap();

        assert!(matches!(
            state.sync_bases.get(&EventInstanceId::from("base@caldir")),
            Some(SyncBase::EventBase(_))
        ));
        assert!(matches!(
            state
                .sync_bases
                .get(&EventInstanceId::from("fallback@caldir")),
            Some(SyncBase::KnownEventId)
        ));
        assert_eq!(
            state
                .known_event_ids()
                .iter()
                .cloned()
                .collect::<HashSet<_>>(),
            HashSet::from([EventInstanceId::from("fallback@caldir")])
        );
    }

    #[test]
    fn load_returns_empty_when_synced_event_ids_file_missing() {
        let dir = tempfile::TempDir::new().unwrap();
        let state = CalendarState::load(dir.path()).unwrap();

        let dst = tempfile::TempDir::new().unwrap();
        state.write(dst.path()).unwrap();

        let written = std::fs::read_to_string(dst.path().join(KNOWN_IDS_FILE_NAME)).unwrap();
        assert!(written.is_empty());
    }

    #[test]
    fn writes_synced_event_ids_to_state_dir() {
        let src = tempfile::TempDir::new().unwrap();
        std::fs::write(src.path().join(KNOWN_IDS_FILE_NAME), "abc@hooli.com").unwrap();
        let state = CalendarState::load(src.path()).unwrap();

        let dst = tempfile::TempDir::new().unwrap();
        state.write(dst.path()).unwrap();

        let written = std::fs::read_to_string(dst.path().join(KNOWN_IDS_FILE_NAME)).unwrap();
        assert_eq!(written, "abc@hooli.com");
    }

    #[test]
    fn write_creates_state_dir_if_missing() {
        let dir = tempfile::TempDir::new().unwrap();
        let state_dir = dir.path().join("does/not/exist");
        let state = CalendarState::load(dir.path()).unwrap();

        state.write(&state_dir).unwrap();

        assert!(state_dir.join(KNOWN_IDS_FILE_NAME).is_file());
    }
}
