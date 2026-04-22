//! Filesystem paths used by caldir across all platforms.

use std::path::PathBuf;

use crate::error::{CalDirError, CalDirResult};

/// Returns the caldir config directory for the current platform.
///
/// - Linux/BSD: `$XDG_CONFIG_HOME/caldir` or `~/.config/caldir`
/// - macOS: `~/.config/caldir`
/// - Windows: `%APPDATA%\caldir`
///
/// Deliberately diverges from `dirs::config_dir()` on macOS (which returns
/// `~/Library/Application Support`). caldir is a CLI whose configs users are
/// expected to read and edit — `~/.config/` matches where git, ssh, fish,
/// helix, etc. put theirs and makes dotfile-repo syncing easy.
///
/// On macOS, transparently migrates data from the legacy
/// `~/Library/Application Support/caldir` location on first call.
pub fn caldir_config_dir() -> CalDirResult<PathBuf> {
    let new_path = platform_config_dir()?.join("caldir");

    #[cfg(target_os = "macos")]
    migrate_legacy_macos_path(&new_path)?;

    Ok(new_path)
}

#[cfg(target_os = "macos")]
fn platform_config_dir() -> CalDirResult<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| CalDirError::Config("Could not determine home directory".into()))?;
    Ok(home.join(".config"))
}

#[cfg(not(target_os = "macos"))]
fn platform_config_dir() -> CalDirResult<PathBuf> {
    dirs::config_dir()
        .ok_or_else(|| CalDirError::Config("Could not determine config directory".into()))
}

#[cfg(target_os = "macos")]
fn migrate_legacy_macos_path(new_path: &std::path::Path) -> CalDirResult<()> {
    let home = dirs::home_dir()
        .ok_or_else(|| CalDirError::Config("Could not determine home directory".into()))?;
    let old_path = home
        .join("Library")
        .join("Application Support")
        .join("caldir");

    if !old_path.exists() {
        return Ok(());
    }

    if new_path.exists() {
        eprintln!(
            "warning: caldir config exists at both {} and {}. Using the new \
             location; remove the old one manually when ready.",
            new_path.display(),
            old_path.display(),
        );
        return Ok(());
    }

    if let Some(parent) = new_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    eprintln!(
        "Migrating caldir config: {} → {}",
        old_path.display(),
        new_path.display()
    );

    std::fs::rename(&old_path, new_path)?;

    Ok(())
}
