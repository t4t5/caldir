//! Handle the connect flow for generic CalDAV servers.
//!
//! Three credential fields: server_url, username, password.
//! On submit: discovers endpoints, saves session.

use anyhow::Result;
use caldir_core::remote::protocol::{
    Connect, ConnectResponse, ConnectStepKind, CredentialField, CredentialsData, FieldType,
};
use caldir_provider_caldav::ops;

use crate::session::Session;

pub async fn handle(cmd: Connect) -> Result<ConnectResponse> {
    // If data contains credentials, this is the submit step.
    if cmd.data.contains_key("server_url") {
        let server_url = cmd
            .data
            .get("server_url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'server_url' in credentials"))?;

        let username = cmd
            .data
            .get("username")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'username' in credentials"))?;

        let password = cmd
            .data
            .get("password")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'password' in credentials"))?;

        let endpoints = ops::discover_endpoints(server_url, username, password).await?;

        let account_identifier = Session::account_identifier(username, server_url);

        let session = Session::new(
            server_url,
            username,
            password,
            &endpoints.principal_url,
            &endpoints.calendar_home_url,
        );
        session.save()?;

        return Ok(ConnectResponse::Done {
            account_identifier,
        });
    }

    // Init step: return credential field requirements
    let fields = vec![
        CredentialField {
            id: "server_url".to_string(),
            label: "CalDAV Server URL".to_string(),
            field_type: FieldType::Url,
            required: true,
            help: Some("e.g. https://mail.runbox.com/caldav".to_string()),
        },
        CredentialField {
            id: "username".to_string(),
            label: "Username".to_string(),
            field_type: FieldType::Text,
            required: true,
            help: None,
        },
        CredentialField {
            id: "password".to_string(),
            label: "Password".to_string(),
            field_type: FieldType::Password,
            required: true,
            help: None,
        },
    ];

    let creds_data = CredentialsData { fields };

    Ok(ConnectResponse::NeedsInput {
        step: ConnectStepKind::Credentials,
        data: serde_json::to_value(creds_data)?,
    })
}
