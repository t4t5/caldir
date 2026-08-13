mod connect;
mod create_event;
mod delete_event;
mod ics;
mod list_calendars;
mod list_events;
mod update_event;

use crate::{CalendarConfig, Event};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::time::Duration;

// actions:
pub use connect::{
    Connect, ConnectResponse, ConnectStepKind, CredentialField, CredentialsData, FieldType,
    HostedOAuthData, OAuthData, SetupData,
};
pub use create_event::CreateEvent;
pub use delete_event::DeleteEvent;
pub use ics::Ics;
pub use list_calendars::ListCalendars;
pub use list_events::ListEvents;
pub use update_event::UpdateEvent;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);

pub(crate) trait WireValue: Sized {
    type Wire: Serialize + DeserializeOwned;

    fn into_wire(self) -> Self::Wire;
    fn from_wire(wire: Self::Wire) -> Self;
}

impl WireValue for Event {
    type Wire = Ics<Event>;

    fn into_wire(self) -> Self::Wire {
        self.into()
    }

    fn from_wire(wire: Self::Wire) -> Self {
        wire.into_inner()
    }
}

impl<T: WireValue> WireValue for Vec<T> {
    type Wire = Vec<T::Wire>;

    fn into_wire(self) -> Self::Wire {
        self.into_iter().map(WireValue::into_wire).collect()
    }

    fn from_wire(wire: Self::Wire) -> Self {
        wire.into_iter().map(T::from_wire).collect()
    }
}

macro_rules! json_wire {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl WireValue for $ty {
                type Wire = Self;

                fn into_wire(self) -> Self::Wire {
                    self
                }

                fn from_wire(wire: Self::Wire) -> Self {
                    wire
                }
            }
        )+
    };
}

json_wire!((), CalendarConfig, ConnectResponse);

pub(crate) type Wire<T> = <T as WireValue>::Wire;

// Handles serialization of command + deserialization of response
pub(crate) trait Rpc: Serialize {
    type Response: WireValue;
    const METHOD: Method;
    const TIMEOUT: Duration = DEFAULT_TIMEOUT;

    fn to_json(&self) -> Result<serde_json::Value, serde_json::Error>
    where
        Self: Sized,
    {
        serde_json::to_value(Request::from_rpc(self)?)
    }
}

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

#[derive(Debug, Serialize, Deserialize)]
pub struct Request {
    #[serde(rename = "command")]
    pub method: Method,
    #[serde(default)]
    pub params: serde_json::Value,
}

impl Request {
    pub(crate) fn from_rpc<C: Rpc>(cmd: &C) -> Result<Self, serde_json::Error> {
        Ok(Self {
            method: C::METHOD,
            params: serde_json::to_value(cmd)?,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Response<T> {
    Success { data: T },
    Error { error: String },
}

impl<T: Serialize> Response<T> {
    /// Serialize a success response to a JSON string for stdout.
    pub fn success(data: T) -> String {
        serde_json::to_string(&Response::Success { data })
            .expect("Response::Success serialization is infallible for Serialize types")
    }
}

impl Response<()> {
    /// Serialize an error response to a JSON string for stdout.
    pub fn error(msg: &str) -> String {
        serde_json::to_string(&Response::<()>::Error {
            error: msg.to_string(),
        })
        .expect("Response::Error serialization is infallible")
    }
}
