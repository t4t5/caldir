use super::{ConnectResponse, Ics};
use crate::{CalendarConfig, Event};
use serde::Serialize;
use serde::de::DeserializeOwned;

pub(crate) trait WireValue: Sized {
    type Wire: Serialize + DeserializeOwned;

    fn into_wire(self) -> Self::Wire;
    fn from_wire(wire: Self::Wire) -> Self;
}

impl WireValue for Event {
    type Wire = Ics;

    fn into_wire(self) -> Self::Wire {
        Ics(self)
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

/// Response types that pass through as plain JSON; ICS-encoded types impl [`WireValue`] directly.
pub(crate) trait JsonWireValue: Serialize + DeserializeOwned {}

impl JsonWireValue for () {}
impl JsonWireValue for CalendarConfig {}
impl JsonWireValue for ConnectResponse {}

impl<T: JsonWireValue> WireValue for T {
    type Wire = Self;

    fn into_wire(self) -> Self::Wire {
        self
    }

    fn from_wire(wire: Self::Wire) -> Self {
        wire
    }
}

pub(crate) type Wire<T> = <T as WireValue>::Wire;
