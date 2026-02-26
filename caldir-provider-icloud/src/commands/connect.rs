//! Handle the connect flow for iCloud Calendar.
//!
//! iCloud uses credential-based auth (Apple ID + app-specific password).
//! The flow is two steps:
//! 1. Return credential field requirements (NeedsInput with Credentials)
//! 2. Validate credentials, discover CalDAV endpoints, return Done

use anyhow::{Context, Result};
use caldir_core::remote::protocol::{
    Connect, ConnectResponse, ConnectStepKind, CredentialField, CredentialsData, FieldType,
};
use libdav::caldav::FindCalendarHomeSet;

use crate::caldav::create_caldav_client;
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

        // Discover CalDAV endpoints using libdav
        let (principal_url, calendar_home_url) =
            discover_caldav_endpoints(apple_id, app_password).await?;

        // Save session
        let session = Session::new(apple_id, app_password, &principal_url, &calendar_home_url);
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

/// Discover CalDAV principal and calendar-home URLs using libdav.
async fn discover_caldav_endpoints(apple_id: &str, app_password: &str) -> Result<(String, String)> {
    let caldav = create_caldav_client(CALDAV_ENDPOINT, apple_id, app_password)?;

    let principal = caldav
        .find_current_user_principal()
        .await
        .context("Failed to find current user principal")?
        .ok_or_else(|| anyhow::anyhow!(
            "iCloud authentication failed. Check your Apple ID and app password."
        ))?;

    let principal_url = format!(
        "{}://{}{}",
        caldav.base_url().scheme_str().unwrap_or("https"),
        caldav
            .base_url()
            .authority()
            .map(|a| a.as_str())
            .unwrap_or("caldav.icloud.com"),
        principal.path()
    );

    let home_set_response = caldav
        .request(FindCalendarHomeSet::new(&principal))
        .await
        .context("Failed to find calendar home set")?;

    let calendar_home = home_set_response
        .home_sets
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("No calendar home set found for this account"))?;

    let calendar_home_url = format!(
        "{}://{}{}",
        caldav.base_url().scheme_str().unwrap_or("https"),
        caldav
            .base_url()
            .authority()
            .map(|a| a.as_str())
            .unwrap_or("caldav.icloud.com"),
        calendar_home.path()
    );

    Ok((principal_url, calendar_home_url))
}
