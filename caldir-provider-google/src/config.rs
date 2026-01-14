//! Configuration and token storage for the Google provider.
//!
//! Credentials and tokens are stored in:
//!   ~/.config/caldir/providers/google/credentials.json
//!   ~/.config/caldir/providers/google/tokens/{account}.json

use crate::types::{GoogleAccountTokens, GoogleCredentials};
use anyhow::{Context, Result};
use std::path::PathBuf;

fn base_dir() -> Result<PathBuf> {
    Ok(dirs::config_dir()
        .context("Could not determine config directory")?
        .join("caldir")
        .join("providers")
        .join("google"))
}

pub struct GoogleAppConfig {
    pub creds: GoogleCredentials,
}

impl GoogleAppConfig {
    pub fn load() -> Result<Self> {
        let path = base_dir()?.join("credentials.json");

        if !path.exists() {
            anyhow::bail!(
                "Google credentials not found.\n\n\
                Create {} with:\n\n\
                {{\n  \
                  \"client_id\": \"your-client-id.apps.googleusercontent.com\",\n  \
                  \"client_secret\": \"your-client-secret\"\n\
                }}\n\n\
                See https://console.cloud.google.com/apis/credentials for setup.",
                path.display()
            );
        }

        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read credentials from {}", path.display()))?;

        let creds: GoogleCredentials = serde_json::from_str(&contents)
            .with_context(|| format!("Failed to parse credentials from {}", path.display()))?;

        Ok(Self { creds })
    }

    pub fn account(&self, account_email: &str) -> GoogleAccountConfig {
        GoogleAccountConfig {
            account: account_email.to_string(),
            creds: self.creds.clone(),
        }
    }
}

pub struct GoogleAccountConfig {
    pub account: String,
    pub creds: GoogleCredentials,
}

impl GoogleAccountConfig {
    pub fn load_tokens(&self) -> Result<GoogleAccountTokens> {
        let path = self.token_path()?;

        if !path.exists() {
            anyhow::bail!(
                "No tokens for account: {}\n\
                Run `caldir-cli auth google` first.",
                self.account
            );
        }

        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read tokens from {}", path.display()))?;

        let tokens: GoogleAccountTokens = serde_json::from_str(&contents)
            .with_context(|| format!("Failed to parse tokens from {}", path.display()))?;

        Ok(tokens)
    }

    pub fn save_tokens(&self, tokens: &GoogleAccountTokens) -> Result<()> {
        let path = self.token_path()?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create tokens directory at {}", parent.display())
            })?;
        }

        let contents =
            serde_json::to_string_pretty(tokens).context("Failed to serialize tokens")?;

        std::fs::write(&path, contents)
            .with_context(|| format!("Failed to write tokens to {}", path.display()))?;

        Ok(())
    }

    fn token_path(&self) -> Result<PathBuf> {
        let safe_account = self.account.replace(['/', '\\', ':'], "_");
        Ok(base_dir()?
            .join("tokens")
            .join(format!("{}.json", safe_account)))
    }
}
