//! Handle the connect flow for webcal subscriptions.
//!
//! Single credential field: the ICS feed URL.
//! On submit: fetches the URL and verifies it's valid ICS.

use anyhow::Result;
use caldir_core::remote::protocol::{
    Connect, ConnectResponse, ConnectStepKind, CredentialField, CredentialsData, FieldType,
};

pub async fn handle(cmd: Connect) -> Result<ConnectResponse> {
    // If data contains the URL, this is the submit step.
    if cmd.data.contains_key("url") {
        let raw_url = cmd
            .data
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'url' in credentials"))?;

        // Normalize webcal:// to https://
        let url = raw_url.replacen("webcal://", "https://", 1);

        // Fetch the ICS feed to validate it
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("caldir-provider-webcal")
            .build()?;

        let response = client.get(&url).send().await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to fetch calendar URL: HTTP {}", response.status());
        }

        let body = response.text().await?;

        if !body.contains("BEGIN:VCALENDAR") {
            anyhow::bail!(
                "The URL does not appear to be a valid ICS calendar feed (no BEGIN:VCALENDAR found)"
            );
        }

        return Ok(ConnectResponse::Done {
            account_identifier: url,
        });
    }

    // Init step: return credential field requirements
    let fields = vec![CredentialField {
        id: "url".to_string(),
        label: "Calendar URL".to_string(),
        field_type: FieldType::Url,
        required: true,
        help: Some("URL to an .ics calendar feed (webcal:// or https://)".to_string()),
    }];

    let creds_data = CredentialsData { fields };

    Ok(ConnectResponse::NeedsInput {
        step: ConnectStepKind::Credentials,
        data: serde_json::to_value(creds_data)?,
    })
}
