//! Shared platform-aware path resolution.
//!
//! Both `CaldirConfig` (CLI config dir) and `ProviderStorage` (per-provider
//! storage dir) anchor their defaults on the same platform-native config
//! directory; this module is the single source of truth for that resolution.

use std::path::PathBuf;

/// The platform-native config-root directory:
/// - Linux/BSD: `$XDG_CONFIG_HOME` or `~/.config`
/// - macOS:     `~/.config` (override; `dirs::config_dir` returns `~/Library/Application Support`)
/// - Windows:   `%APPDATA%`
///
/// Returns `None` if the platform doesn't expose enough state to resolve a
/// directory (e.g. on macOS if `$HOME` is unset).
#[cfg(target_os = "macos")]
pub(crate) fn platform_config_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".config"))
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn platform_config_dir() -> Option<PathBuf> {
    dirs::config_dir()
}
