// Recurring events share the same UID (stupid design)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EventUid(String);

impl EventUid {
    pub fn from_str(uid: String) -> Self {
        EventUid(uid)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
