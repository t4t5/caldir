mod caldir;
mod calendar;
mod connection;
mod diff;
mod event;
mod provider;
mod remote;
pub mod rpc;
mod utils;

#[cfg(test)]
mod test_utils;

// Public API:
pub use caldir::{Caldir, CaldirConfig, CaldirError, TimeFormat};
pub use calendar::{Calendar, CalendarConfig, CalendarEvent};
pub use connection::Connection;
pub use diff::{CalendarDiff, EventChange};
pub use event::{
    Attendee, Event, EventTime, ParticipationStatus, Recurrence, Reminder, expand_in_range,
};
pub use provider::{Provider, ProviderRegistry, ProviderSlug};
pub use remote::{Remote, RemoteConfig, RemoteConfigParams, RemoteEvent};
pub use utils::{DateBounds, DateRange};
