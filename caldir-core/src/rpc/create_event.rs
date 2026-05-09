use super::{Method, Rpc};
use crate::{Event, RemoteConfigParams};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
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
    fn create_event_serializes_event_as_ics_string() {
        let mut params = RemoteConfigParams::new();
        params.insert(
            "hooli_account".to_string(),
            toml::Value::String("user@hmail.com".to_string()),
        );

        let event = Event::new(
            "Test",
            EventTime::Date(chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
        );

        let json: serde_json::Value = serde_json::to_value(CreateEvent {
            remote: params,
            event: event.clone(),
        })
        .unwrap();

        assert_eq!(json["hooli_account"], "user@hmail.com");

        let ics = json["event"].as_str().expect("event should be a string");
        assert!(ics.starts_with("BEGIN:VCALENDAR"));
        assert!(ics.contains(&format!("UID:{}", event.uid.as_str())));
    }
}
