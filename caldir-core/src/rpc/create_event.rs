use super::{Method, Rpc};
use crate::{Event, RemoteConfigParams};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub(crate) struct CreateEvent {
    #[serde(flatten)]
    pub remote: RemoteConfigParams,
    pub event: Event,
}

impl Rpc for CreateEvent {
    const METHOD: Method = Method::CreateEvent;
    type Response = Event;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RemoteConfigParams, event::EventTime};

    #[test]
    fn create_event_serializes_json() {
        let mut params = RemoteConfigParams::new();
        params.insert(
            "hooli_account".to_string(),
            toml::Value::String("user@hmail.com".to_string()),
        );

        let event = Event::new(
            "Test",
            EventTime::Date(chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
        );

        let cmd = CreateEvent {
            remote: params,
            event: event.clone(),
        };

        let json = cmd.to_wire_value().unwrap();

        assert_eq!(json["command"], "create_event");
        assert_eq!(json["params"]["hooli_account"], "user@hmail.com");

        let ics = json["params"]["event"]
            .as_str()
            .expect("event should be a string");
        assert!(ics.starts_with("BEGIN:VCALENDAR"));
        assert!(ics.contains(&format!("UID:{}", event.uid.as_str())));
    }
}
