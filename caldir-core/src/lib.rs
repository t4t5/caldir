mod caldir;
mod calendar;
mod connection;
mod event;
mod provider;
mod remote;
mod transport;
mod utils;

#[cfg(test)]
mod test_utils;
#[cfg(test)]
pub(crate) use transport::mock_transport::MockTransport;

pub(crate) use transport::{SubprocessTransport, Transport, TransportError};

// Public API:
pub use caldir::{Caldir, CaldirConfig};
pub use calendar::{Calendar, CalendarConfig, CalendarEvent};
pub use connection::Connection;
pub use event::{Event, EventError, EventTime, Reminder};
pub use provider::{Provider, ProviderRegistry, ProviderSlug};
pub use remote::{Remote, RemoteConfig, RemoteConfigParams};
