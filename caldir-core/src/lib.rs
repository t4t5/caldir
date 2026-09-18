mod caldir;
mod calendar;
mod connection;
mod diff;
mod event;
pub mod provider;
mod remote;
pub mod rpc;
mod utils;

#[cfg(test)]
mod test_utils;

// Public API:
pub use caldir::{Caldir, CaldirConfig, CaldirConfigError, CaldirError, TimeFormat};
pub use calendar::{
    Calendar, CalendarConfig, CalendarConfigError, CalendarError, CalendarEvent,
    CalendarEventError, CalendarStateError,
};
pub use connection::{Connection, ConnectionError};
pub use diff::{CalendarDiff, EventChange};
pub use event::{
    Attachment, Attendee, Availability, Event, EventError, EventInstanceId, EventTime, EventUid,
    Organizer, ParticipationStatus, Recurrence, RecurrenceId, Reminder, Status, Visibility,
    XProperty, expand_in_range, tz_normalize,
};
pub use provider::{
    Provider, ProviderError, ProviderRegistry, ProviderSlug, ProviderTransportError,
};
pub use remote::{Remote, RemoteConfig, RemoteConfigParams, RemoteError, RemoteEvent};
pub use utils::{DateBounds, DateRange};
