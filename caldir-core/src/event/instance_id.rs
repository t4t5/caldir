mod event_uid;
mod recurrence_id;

pub use event_uid::EventUid;
pub use recurrence_id::RecurrenceId;

// UID + RecurrenceId = the actual unique ID per event
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EventInstanceId((EventUid, Option<RecurrenceId>));

impl EventInstanceId {
    pub fn new(uid: EventUid, recurrence_id: Option<RecurrenceId>) -> Self {
        EventInstanceId((uid, recurrence_id))
    }

    pub fn uid(&self) -> &EventUid {
        &self.0.0
    }

    pub fn recurrence_id(&self) -> Option<&RecurrenceId> {
        self.0.1.as_ref()
    }
}
