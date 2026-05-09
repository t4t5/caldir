use super::{Method, Rpc};
use crate::{Event, RemoteConfig};
use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct CreateEvent {
    #[serde(flatten)]
    pub remote_config: RemoteConfig,
    pub event: Event,
}

impl Rpc for CreateEvent {
    const METHOD: Method = Method::CreateEvent;
    type Response = Event;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ProviderSlug, RemoteConfigParams, event::EventTime};

    #[test]
    fn create_event_serializes_event_as_ics_string() {
        let mut params = RemoteConfigParams::new();
        params.insert(
            "hooli_account".to_string(),
            toml::Value::String("user@hmail.com".to_string()),
        );
        let remote_config = RemoteConfig::new(ProviderSlug::from("hooli"), params);
        let event = Event::new(
            "Test",
            EventTime::Date(chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
        );

        let json: serde_json::Value = serde_json::to_value(CreateEvent {
            remote_config,
            event: event.clone(),
        })
        .unwrap();

        assert_eq!(json["provider"], "hooli");
        assert_eq!(json["hooli_account"], "user@hmail.com");
        let ics = json["event"].as_str().expect("event should be a string");
        assert!(ics.starts_with("BEGIN:VCALENDAR"));
        assert!(ics.contains(&format!("UID:{}", event.uid.as_str())));
    }
}
