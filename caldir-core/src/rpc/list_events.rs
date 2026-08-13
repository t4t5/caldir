use super::{Method, Rpc};
use crate::{Event, RemoteConfigParams};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct ListEvents {
    #[serde(flatten)]
    pub remote: RemoteConfigParams,
    pub from: String,
    pub to: String,
}

impl Rpc for ListEvents {
    const METHOD: Method = Method::ListEvents;
    type Response = Vec<Event>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_events_serializes_json() {
        let mut params = RemoteConfigParams::new();
        params.insert(
            "hooli_account".to_string(),
            toml::Value::String("user@hmail.com".to_string()),
        );

        let cmd = ListEvents {
            remote: params,
            from: "2026-01-01T00:00:00+00:00".into(),
            to: "2026-12-31T23:59:59+00:00".into(),
        };

        let json = cmd.to_json().unwrap();

        assert_eq!(json["command"], "list_events");
        assert_eq!(json["params"]["hooli_account"], "user@hmail.com");
        assert_eq!(json["params"]["from"], "2026-01-01T00:00:00+00:00");
        assert_eq!(json["params"]["to"], "2026-12-31T23:59:59+00:00");
    }
}
