//! Initialize authentication - returns credential field requirements.
//!
//! For iCloud, we use credential-based auth (Apple ID + app-specific password)
//! rather than OAuth.

use anyhow::Result;
use caldir_core::remote::protocol::{
    AuthInit, AuthInitResponse, AuthType, CredentialField, CredentialsData, FieldType,
};

pub async fn handle(_cmd: AuthInit) -> Result<AuthInitResponse> {
    let fields = vec![
        CredentialField {
            id: "apple_id".to_string(),
            label: "Apple ID".to_string(),
            field_type: FieldType::Text,
            required: true,
            help: Some("Your Apple ID email address".to_string()),
        },
        CredentialField {
            id: "app_password".to_string(),
            label: "App-Specific Password".to_string(),
            field_type: FieldType::Password,
            required: true,
            help: Some(
                "Create at https://account.apple.com/sign-in -> Sign-In and Security -> App-Specific Passwords".to_string(),
            ),
        },
    ];

    let creds_data = CredentialsData { fields };

    Ok(AuthInitResponse {
        auth_type: AuthType::Credentials,
        data: serde_json::to_value(creds_data)?,
    })
}
