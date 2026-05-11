mod create_event;
mod delete_event;
mod list_events;
mod update_event;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::time::Duration;

// actions:
pub(crate) use create_event::CreateEvent;
pub(crate) use delete_event::DeleteEvent;
pub(crate) use list_events::ListEvents;
pub(crate) use update_event::UpdateEvent;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);

pub(crate) trait Rpc: Serialize {
    type Response: Serialize + DeserializeOwned;
    const METHOD: Method;
    const TIMEOUT: Duration = DEFAULT_TIMEOUT;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Method {
    Connect,
    ListCalendars,
    ListEvents,
    CreateEvent,
    UpdateEvent,
    DeleteEvent,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Request {
    #[serde(rename = "command")] // TODO: update providers so we can remove this
    pub(crate) method: Method,
    #[serde(default)]
    pub(crate) params: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(crate) enum Response<T> {
    Success { data: T },
    Error { error: String },
}
