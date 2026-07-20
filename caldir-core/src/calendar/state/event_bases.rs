use std::{collections::HashMap, path::Path};

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
            let event_path = path.join(format!("{}.ics", event.event_instance_id()));
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
                if file_path.is_file() {
                    let event = Event::load_single(&file_path)?;
                    let event_instance_id = event.event_instance_id();
                    event_bases.insert(event_instance_id, event);
                }
            }
        }

        Ok(Self(event_bases))
    }
}
