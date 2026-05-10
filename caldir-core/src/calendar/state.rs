mod error;
mod known_event_ids;

pub use error::CalendarStateError;
use known_event_ids::KnownEventIds;
use std::path::Path;

struct CalendarState {
    known_event_ids: KnownEventIds,
}

const KNOWN_IDS_FILE_NAME: &str = "known_event_ids";

impl CalendarState {
    pub(crate) fn load(state_dir: &Path) -> Result<Self, CalendarStateError> {
        let known_ids_path = state_dir.join(KNOWN_IDS_FILE_NAME);
        let known_event_ids = KnownEventIds::load(&known_ids_path)?;

        Ok(Self { known_event_ids })
    }
}
