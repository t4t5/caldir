mod create_event;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::time::Duration;

// actions:
pub(crate) use create_event::CreateEvent;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);

pub(crate) trait Rpc: Serialize {
    type Response: DeserializeOwned;
    const OP: Op;
    const TIMEOUT: Duration = DEFAULT_TIMEOUT;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Op {
    Connect,
    ListCalendars,
    ListEvents,
    CreateEvent,
    UpdateEvent,
    DeleteEvent,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Request {
    pub(crate) op: Op,
    #[serde(default)]
    pub(crate) params: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(crate) enum Response<T> {
    Success { data: T },
    Error { error: String },
}
