pub const PROVIDER_NAME: &str = "google";
pub(crate) const GOOGLE_EVENT_ID_PROPERTY: &str = "X-GOOGLE-EVENT-ID";
pub(crate) const GOOGLE_COLOR_ID_PROPERTY: &str = "X-GOOGLE-COLOR-ID";
pub(crate) const GOOGLE_EVENT_TYPE_PROPERTY: &str = "X-GOOGLE-EVENT-TYPE";
pub(crate) const GOOGLE_DEFAULT_REMINDERS_PROPERTY: &str = "X-GOOGLE-DEFAULT-REMINDERS";

/// Remote-config key holding a calendar's default popup reminders, in minutes
/// before start (e.g. `default_reminders = [10, 0]`).
pub(crate) const DEFAULT_REMINDERS_PARAM: &str = "default_reminders";
