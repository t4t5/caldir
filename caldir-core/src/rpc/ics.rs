use crate::Event;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ics<T>(pub T);

impl<T> Ics<T> {
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl From<Event> for Ics<Event> {
    fn from(event: Event) -> Self {
        Self(event)
    }
}

impl Serialize for Ics<Event> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_ics_string())
    }
}

impl<'de> Deserialize<'de> for Ics<Event> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let ics = String::deserialize(deserializer)?;
        Event::from_single_ics_str(&ics)
            .map(Self)
            .map_err(serde::de::Error::custom)
    }
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

        let json = serde_json::to_string(&Ics::from(event.clone())).unwrap();
        let decoded: Ics<Event> = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.into_inner(), event);
    }
}
