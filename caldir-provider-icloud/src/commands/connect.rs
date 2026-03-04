//! Handle the connect flow for iCloud Calendar.
//!
//! iCloud uses credential-based auth (Apple ID + app-specific password).
//! The flow is two steps:
//! 1. Return credential field requirements (NeedsInput with Credentials)
//! 2. Validate credentials, discover CalDAV endpoints, return Done

use anyhow::Result;
use caldir_core::remote::protocol::{
    Connect, ConnectResponse, ConnectStepKind, CredentialField, CredentialsData, FieldType,
};
use caldir_provider_caldav::ops;

use crate::constants::CALDAV_ENDPOINT;
use crate::session::Session;

pub async fn handle(cmd: Connect) -> Result<ConnectResponse> {
    // If data contains credentials, this is the submit step.
    if cmd.data.contains_key("apple_id") {
        let apple_id = cmd
            .data
            .get("apple_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'apple_id' in credentials"))?;

        let app_password = cmd
            .data
            .get("app_password")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'app_password' in credentials"))?;

        let endpoints =
            ops::discover_endpoints(CALDAV_ENDPOINT, apple_id, app_password).await?;

        let session = Session::new(
            apple_id,
            app_password,
            &endpoints.principal_url,
            &endpoints.calendar_home_url,
        );
        session.save()?;

        return Ok(ConnectResponse::Done {
            account_identifier: apple_id.to_string(),
        });
    }

    // Init step: return credential field requirements
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

    Ok(ConnectResponse::NeedsInput {
        step: ConnectStepKind::Credentials,
        data: serde_json::to_value(creds_data)?,
    })
}
