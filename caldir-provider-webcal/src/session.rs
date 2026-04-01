//! Session storage for webcal subscriptions.
//!
//! Stores the ICS feed URL and extracted calendar metadata at:
//!   ~/.config/caldir/providers/webcal/session/{slug}.toml

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
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
    /// Derive a filename-safe slug from the URL.
    fn slug(url: &str) -> String {
        url.replace(['/', '\\', ':', '@', '.', '?', '&', '=', '%', '#'], "_")
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

    /// Load a session by URL (the URL is the account identifier).
    pub fn load(url: &str) -> Result<Self> {
        let path = Self::path_for(url)?;
        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("Webcal session not found for URL: {}", url))?;
        toml::from_str(&contents).context("Failed to parse webcal session")
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
