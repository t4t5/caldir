use super::{Method, Rpc};
use crate::{Event, RemoteConfigParams};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub(crate) struct UpdateEvent {
    #[serde(flatten)]
    pub remote: RemoteConfigParams,
    pub event: Event,
}

impl Rpc for UpdateEvent {
    const METHOD: Method = Method::UpdateEvent;
    type Response = Event;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rpc::Request;
    use crate::{RemoteConfigParams, event::EventTime};

    #[test]
    fn update_event_serializes_json() {
        let mut params = RemoteConfigParams::new();
        params.insert(
            "hooli_account".to_string(),
            toml::Value::String("user@hmail.com".to_string()),
        );

        let event = Event::new(
            "Test",
            EventTime::Date(chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
        );

        let cmd = UpdateEvent {
            remote: params,
            event: event.clone(),
        };

        let json = serde_json::to_value(Request::from_rpc(&cmd).unwrap()).unwrap();

        assert_eq!(json["command"], "update_event");
        assert_eq!(json["params"]["hooli_account"], "user@hmail.com");

        let ics = json["params"]["event"]
            .as_str()
            .expect("event should be a string");
        assert!(ics.starts_with("BEGIN:VCALENDAR"));
        assert!(ics.contains(&format!("UID:{}", event.uid.as_str())));
    }
}
