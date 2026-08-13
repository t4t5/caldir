mod request;
mod response;

use super::{Method, Rpc};
use serde::{Deserialize, Serialize};

pub use request::{
    CredentialField, CredentialsData, FieldType, HostedOAuthData, OAuthData, SetupData,
};
pub use response::{ConnectResponse, ConnectStepKind};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Connect {
    #[serde(default)]
    pub options: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    pub data: serde_json::Map<String, serde_json::Value>,
}

impl Rpc for Connect {
    const METHOD: Method = Method::Connect;
    type Response = ConnectResponse;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn connect_serializes_json() {
        let redirect_uri = "http://localhost:8080/callback";

        let mut options = serde_json::Map::new();
        options.insert("redirect_uri".to_string(), json!(redirect_uri));
        options.insert("hosted".to_string(), json!(false));

        let mut data = serde_json::Map::new();
        data.insert("code".to_string(), json!("4/0AeaYSHB..."));
        data.insert("state".to_string(), json!("xyz789"));
        data.insert("redirect_uri".to_string(), json!(redirect_uri));

        let cmd = Connect { options, data };

        let json = cmd.to_json().unwrap();

        assert_eq!(json["command"], "connect");
        assert_eq!(json["params"]["options"]["redirect_uri"], redirect_uri);
        assert_eq!(json["params"]["options"]["hosted"], false);
        assert_eq!(json["params"]["data"]["code"], "4/0AeaYSHB...");
        assert_eq!(json["params"]["data"]["state"], "xyz789");
        assert_eq!(json["params"]["data"]["redirect_uri"], redirect_uri);
    }
}
