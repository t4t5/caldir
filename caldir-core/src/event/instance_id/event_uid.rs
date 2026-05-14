// Recurring events share the same UID (stupid design)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EventUid(String);

impl EventUid {
    pub fn new(uid: impl Into<String>) -> Self {
        EventUid(uid.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
