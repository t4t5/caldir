use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);

/// A typed provider command. Each implementor binds itself to a
/// [`Command`] name and a response type, so callers of `Provider::call`
/// get back the right `Response` without naming it explicitly.
pub(crate) trait ProviderCommand: Serialize {
    type Response: DeserializeOwned;
    const NAME: Command;
    const TIMEOUT: Duration = DEFAULT_TIMEOUT;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Command {
    Connect,
    ListCalendars,
    ListEvents,
    CreateEvent,
    UpdateEvent,
    DeleteEvent,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Request {
    pub(crate) command: Command,
    #[serde(default)]
    pub(crate) params: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(crate) enum Response<T> {
    Success { data: T },
    Error { error: String },
}
