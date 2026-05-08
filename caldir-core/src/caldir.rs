mod config;

use crate::ProviderRegistry;
pub use config::CaldirConfig;
use std::path::PathBuf;

pub struct Caldir {
    config: CaldirConfig,
    providers: ProviderRegistry,
}

impl Caldir {
    pub fn new(config: CaldirConfig, providers: ProviderRegistry) -> Self {
        Caldir { config, providers }
    }

    pub fn dir(&self) -> PathBuf {
        self.config.calendar_dir()
    }

    /// Generate a unique slug that doesn't conflict with existing calendar directories.
    /// If the base slug exists, tries slug-2, slug-3, etc.
    pub(crate) fn unique_calendar_slug(&self, desired: &str) -> String {
        let calendar_dir = self.config.calendar_dir();

        if !calendar_dir.join(desired).exists() {
            return desired.to_string();
        }

        let mut suffix = 2;

        loop {
            let candidate = format!("{desired}-{suffix}");
            if !calendar_dir.join(&candidate).exists() {
                return candidate;
            }
            suffix += 1;
        }
    }
}

impl Default for Caldir {
    fn default() -> Self {
        Self::new(
            CaldirConfig::default(),
            ProviderRegistry::from_system_path(),
        )
    }
}
