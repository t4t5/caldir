//! Session storage for webcal subscriptions.
//!
//! Stores the ICS feed URL and extracted calendar metadata at:
//!   ~/.config/caldir/providers/webcal/session/{slug}.toml

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use crate::constants::PROVIDER_NAME;

pub fn base_dir() -> Result<PathBuf> {
    Ok(dirs::config_dir()
        .context("Could not determine config directory")?
        .join("caldir")
        .join("providers")
        .join(PROVIDER_NAME))
}

/// Webcal session (URL + extracted calendar metadata).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub url: String,
    pub display_name: Option<String>,
    pub color: Option<String>,
}

impl Session {
    /// Derive a slug from the URL host and a path hash for use as filename.
    pub fn slug(url: &str) -> String {
        let parsed = url::Url::parse(url).ok();
        let host = parsed
            .as_ref()
            .and_then(|u| u.host_str().map(|h| h.to_string()))
            .unwrap_or_else(|| "unknown".to_string());

        let mut hasher = DefaultHasher::new();
        url.hash(&mut hasher);
        let hash = hasher.finish();

        let raw = format!("{}_{:x}", host, hash);
        raw.replace(['/', '\\', ':', '@', '.'], "_")
    }

    /// Use the URL host as the account identifier.
    pub fn account_identifier(url: &str) -> String {
        let slug = Self::slug(url);
        format!("webcal_{}", slug)
    }

    fn path_for(url: &str) -> Result<PathBuf> {
        let slug = Self::slug(url);
        Ok(base_dir()?.join("session").join(format!("{}.toml", slug)))
    }

    fn path(&self) -> Result<PathBuf> {
        Self::path_for(&self.url)
    }

    pub fn new(
        url: impl Into<String>,
        display_name: Option<String>,
        color: Option<String>,
    ) -> Self {
        Session {
            url: url.into(),
            display_name,
            color,
        }
    }

    pub fn load(account_identifier: &str) -> Result<Self> {
        let session_dir = base_dir()?.join("session");
        if !session_dir.exists() {
            anyhow::bail!("Webcal session for {} not found!", account_identifier);
        }

        for entry in std::fs::read_dir(&session_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("toml") {
                let contents = std::fs::read_to_string(&path)?;
                if let Ok(session) = toml::from_str::<Session>(&contents) {
                    let id = Self::account_identifier(&session.url);
                    if id == account_identifier {
                        return Ok(session);
                    }
                }
            }
        }

        anyhow::bail!("Webcal session for {} not found!", account_identifier);
    }

    pub fn save(&self) -> Result<()> {
        let path = self.path()?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create session directory: {}", parent.display())
            })?;
        }

        let contents = toml::to_string_pretty(&self).context("Failed to serialize session")?;

        std::fs::write(&path, contents)
            .with_context(|| format!("Failed to write session to {}", path.display()))?;

        Ok(())
    }
}
