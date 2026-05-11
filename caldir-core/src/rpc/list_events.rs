use super::{Method, Rpc};
use crate::{Event, RemoteConfigParams};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub(crate) struct ListEvents {
    #[serde(flatten)]
    pub remote: RemoteConfigParams,
}

impl Rpc for ListEvents {
    const METHOD: Method = Method::ListEvents;
    type Response = Vec<Event>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rpc::Request;

    #[test]
    fn list_events_serializes_json() {
        let mut params = RemoteConfigParams::new();
        params.insert(
            "hooli_account".to_string(),
            toml::Value::String("user@hmail.com".to_string()),
        );

        let cmd = ListEvents { remote: params };

        let json = serde_json::to_value(Request::from_rpc(&cmd).unwrap()).unwrap();

        assert_eq!(json["command"], "list_events");
        assert_eq!(json["params"]["hooli_account"], "user@hmail.com");
    }
}
