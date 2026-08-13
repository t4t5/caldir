//! One module per RPC action, each mapping to a [`Method`] variant.

mod connect;
mod create_event;
mod delete_event;
mod list_calendars;
mod list_events;
mod update_event;

use serde::{Deserialize, Serialize};

pub use connect::{
    Connect, ConnectResponse, ConnectStepKind, CredentialField, CredentialsData, FieldType,
    HostedOAuthData, OAuthData, SetupData,
};
pub use create_event::CreateEvent;
pub use delete_event::DeleteEvent;
pub use list_calendars::ListCalendars;
pub use list_events::ListEvents;
pub use update_event::UpdateEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Method {
    Connect,
    ListCalendars,
    ListEvents,
    CreateEvent,
    UpdateEvent,
    DeleteEvent,
}
