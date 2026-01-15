//! Authentication endpoints

use axum::{
    Router,
    extract::{Path, State},
    routing::post,
    Json,
};
use serde::Serialize;

use caldir_core::local::config::{LocalConfig, RemoteConfig};
use caldir_core::provider::Provider;

use crate::routes::AppError;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/auth/{provider}", post(authenticate))
}

/// Response from authentication
#[derive(Serialize)]
pub struct AuthResponse {
    pub account: String,
    pub calendars_created: Vec<String>,
    pub calendars_existing: Vec<String>,
}

/// POST /auth/:provider - Authenticate with a provider
async fn authenticate(
    State(state): State<AppState>,
    Path(provider_name): Path<String>,
) -> Result<Json<AuthResponse>, AppError> {
    let provider = Provider::from_name(&provider_name);
    let caldir = state.caldir()?;

    // Provider handles the full OAuth flow
    let account = provider.authenticate().await?;

    // List all calendars for this account
    let calendars = provider.list_calendars(&account).await?;

    let mut created = Vec::new();
    let mut existing = Vec::new();

    // Create local directories for each calendar
    for entry in calendars {
        let dir_name = slugify(&entry.name);
        let cal_path = caldir.data_path().join(&dir_name);

        // Skip if already exists
        if cal_path.join(".caldir/config.toml").exists() {
            existing.push(dir_name);
            continue;
        }

        // Create directory structure
        std::fs::create_dir_all(cal_path.join(".caldir/state"))?;

        // Convert JSON config from provider to TOML values
        let params = entry
            .config
            .into_iter()
            .map(|(k, v)| Ok((k, serde_json::from_value(v)?)))
            .collect::<anyhow::Result<_>>()?;

        // Save config
        let config = LocalConfig {
            remote: Some(RemoteConfig {
                provider: provider_name.clone(),
                params,
            }),
        };
        config.save(&cal_path)?;

        created.push(dir_name);
    }

    Ok(Json(AuthResponse {
        account,
        calendars_created: created,
        calendars_existing: existing,
    }))
}

/// Convert a calendar name to a directory-safe slug
fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
