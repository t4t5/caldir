use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupData {
    pub instructions: String,
    pub fields: Vec<CredentialField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialField {
    pub id: String,
    pub label: String,
    pub field_type: FieldType,
    #[serde(default)]
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    Text,
    Password,
    Url,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthData {
    pub authorization_url: String,
    pub state: String,
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostedOAuthData {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialsData {
    pub fields: Vec<CredentialField>,
}
