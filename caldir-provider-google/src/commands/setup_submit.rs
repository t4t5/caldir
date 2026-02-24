//! Handle one-time setup — save OAuth credentials to app_config.toml.

use anyhow::{Context, Result};
use caldir_core::remote::protocol::SetupSubmit;

use crate::app_config::AppConfig;

pub async fn handle(cmd: SetupSubmit) -> Result<()> {
    let client_id = cmd
        .fields
        .get("client_id")
        .and_then(|v| v.as_str())
        .context("Missing client_id")?
        .to_string();

    let client_secret = cmd
        .fields
        .get("client_secret")
        .and_then(|v| v.as_str())
        .context("Missing client_secret")?
        .to_string();

    let app_config = AppConfig {
        client_id,
        client_secret,
    };

    app_config.save()?;

    Ok(())
}
