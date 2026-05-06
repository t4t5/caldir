pub mod config;

use config::CaldirConfig;

pub struct Caldir {
    config: CaldirConfig,
}

impl Caldir {
    pub fn new(config: CaldirConfig) -> Self {
        Caldir { config }
    }

    pub fn config(&self) -> &CaldirConfig {
        &self.config
    }

    /// Generate a unique slug that doesn't conflict with existing calendar directories.
    /// If the base slug exists, tries slug-2, slug-3, etc.
    pub fn unique_calendar_slug(&self, desired: &str) -> String {
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
        Self::new(CaldirConfig::default())
    }
}
