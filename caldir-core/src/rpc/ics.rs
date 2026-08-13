//! ICS string codec for events on the RPC wire.
//! Fields use `#[serde(with = "crate::rpc::ics")]`; response values wrap in [`Ics`].

use crate::Event;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ics(pub Event);

impl Ics {
    pub fn into_inner(self) -> Event {
        self.0
    }
}

impl Serialize for Ics {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serialize(&self.0, serializer)
    }
}

impl<'de> Deserialize<'de> for Ics {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserialize(deserializer).map(Self)
    }
}

pub(crate) fn serialize<S: Serializer>(event: &Event, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&event.to_ics_string())
}

pub(crate) fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Event, D::Error> {
    let ics = String::deserialize(deserializer)?;
    Event::from_single_ics_str(&ics).map_err(serde::de::Error::custom)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EventTime;
    use chrono::NaiveDate;

    #[test]
    fn event_round_trips_through_ics_wire_format() {
        let event = Event::new(
            "Test",
            EventTime::Date(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
        );

        let json = serde_json::to_string(&Ics(event.clone())).unwrap();
        let decoded: Ics = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.into_inner(), event);
    }
}
