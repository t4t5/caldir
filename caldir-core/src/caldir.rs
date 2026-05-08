mod config;
mod error;

use crate::{Calendar, CalendarConfig, ProviderRegistry};
use std::path::PathBuf;

pub use config::CaldirConfig;
pub use error::CaldirError;

pub struct Caldir {
    config: CaldirConfig,
    providers: ProviderRegistry,
}

impl Caldir {
    pub fn new(config: CaldirConfig, providers: ProviderRegistry) -> Self {
        Caldir { config, providers }
    }

    pub fn data_dir(&self) -> PathBuf {
        self.config.data_dir()
    }

    pub fn create_calendar(
        &self,
        desired_slug: &str,
        config: Option<CalendarConfig>,
    ) -> Result<Calendar, CaldirError> {
        let unique_slug = self.find_best_available_calendar_slug(desired_slug);
        let calendar_path = self.data_dir().join(unique_slug);

        Ok(Calendar::create(&calendar_path, config)?)
    }

    /// Generate a unique slug that doesn't conflict with existing calendar directories.
    /// If the base slug exists, tries slug-2, slug-3, etc.
    fn find_best_available_calendar_slug(&self, desired: &str) -> String {
        let calendar_dir = self.config.data_dir();

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

#[cfg(test)]
mod tests {
    use crate::test_utils::test_caldir;

    #[test]
    fn create_calendar_creates_directory_with_desired_slug() {
        let (_tmp, caldir) = test_caldir();

        let calendar = caldir.create_calendar("work", None).unwrap();

        assert_eq!(calendar.path(), caldir.data_dir().join("work"));
        assert_eq!(calendar.slug().unwrap(), "work");
        assert!(calendar.path().is_dir());
    }

    #[test]
    fn create_appends_suffix_on_slug_collision() {
        let (_tmp, caldir) = test_caldir();

        let calendar_1 = caldir.create_calendar("work", None).unwrap();
        assert_eq!(calendar_1.slug().unwrap(), "work");

        let calendar_2 = caldir.create_calendar("work", None).unwrap();
        assert_eq!(calendar_2.slug().unwrap(), "work-2");

        let calendar_3 = caldir.create_calendar("work", None).unwrap();
        assert_eq!(calendar_3.slug().unwrap(), "work-3");
    }
}
