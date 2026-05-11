use super::{Method, Rpc};
use crate::{Event, RemoteConfigParams};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub(crate) struct DeleteEvent {
    #[serde(flatten)]
    pub remote: RemoteConfigParams,
    pub event: Event,
}

impl Rpc for DeleteEvent {
    const METHOD: Method = Method::DeleteEvent;
    type Response = Event;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RemoteConfigParams, event::EventTime};

    #[test]
    fn delete_event_serializes_json() {
        let mut params = RemoteConfigParams::new();
        params.insert(
            "hooli_account".to_string(),
            toml::Value::String("user@hmail.com".to_string()),
        );

        let event = Event::new(
            "Test",
            EventTime::Date(chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
        )
        .add_x_property("X-HOOLI-EVENT-ID", "abc123@hooli.com");

        let uid = event.uid.as_str().to_string();

        let cmd = DeleteEvent {
            remote: params,
            event,
        };

        let json = cmd.to_json().unwrap();

        assert_eq!(json["command"], "delete_event");
        assert_eq!(json["params"]["hooli_account"], "user@hmail.com");

        let ics = json["params"]["event"]
            .as_str()
            .expect("event should be a string");

        assert!(ics.starts_with("BEGIN:VCALENDAR"));
        assert!(ics.contains(&format!("UID:{}", uid)));
        assert!(ics.contains("X-HOOLI-EVENT-ID:abc123@hooli.com"));
    }
}
