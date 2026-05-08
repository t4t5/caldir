mod config;
mod error;

use crate::{Calendar, CalendarConfig, Provider, ProviderRegistry, ProviderSlug};
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

    fn calendars(&self) -> Vec<Calendar> {
        let mut calendars = Vec::new();

        if let Ok(entries) = std::fs::read_dir(self.data_dir()) {
            for entry in entries.flatten() {
                if entry.path().is_dir()
                    && !entry.file_name().to_string_lossy().starts_with('.')
                    && let Ok(calendar) = Calendar::load(&entry.path())
                {
                    calendars.push(calendar);
                }
            }
        }

        calendars
    }

    fn provider(&self, provider_slug: ProviderSlug) -> Result<&Provider, CaldirError> {
        self.providers
            .get(&provider_slug)
            .map_err(CaldirError::Provider)
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
    use super::*;
    use crate::provider::ProviderError;
    use crate::test_utils::{test_binary, test_caldir, test_caldir_config};

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

    #[test]
    fn calendars_returns_empty_if_no_calendars() {
        let (_tmp, caldir) = test_caldir();

        assert!(caldir.calendars().is_empty());
    }

    #[test]
    fn calendars_returns_each_calendar_subdirectory() {
        let (_tmp, caldir) = test_caldir();

        caldir.create_calendar("personal", None).unwrap();
        caldir.create_calendar("work", None).unwrap();

        let calendars = caldir.calendars();

        let mut slugs: Vec<String> = calendars
            .iter()
            .map(|c| c.slug().unwrap().to_string())
            .collect();

        slugs.sort();

        assert_eq!(slugs, vec!["personal", "work"]);
    }

    #[test]
    fn calendars_ignores_hidden_directories() {
        let (_tmp, caldir) = test_caldir();

        caldir.create_calendar("work", None).unwrap();
        std::fs::create_dir_all(caldir.data_dir().join(".hidden")).unwrap();
        std::fs::create_dir_all(caldir.data_dir().join(".git")).unwrap();

        let calendars = caldir.calendars();

        let mut slugs: Vec<String> = calendars
            .iter()
            .map(|c| c.slug().unwrap().to_string())
            .collect();

        assert_eq!(slugs, vec!["work"]);
    }

    #[test]
    fn provider_returns_provider_when_present_in_registry() {
        let (_tmp_bin, bin_path) = test_binary("caldir-provider-hooli");
        let mut registry = ProviderRegistry::new();
        registry.add(Provider::from_binary_path(bin_path).unwrap());

        let (_tmp, config) = test_caldir_config();
        let caldir = Caldir::new(config, registry);

        assert!(caldir.provider(ProviderSlug::from("hooli")).is_ok());
    }

    #[test]
    fn provider_errors_when_not_present_in_registry() {
        let (_tmp, caldir) = test_caldir();

        let result = caldir.provider(ProviderSlug::from("hooli"));

        assert!(matches!(
            result,
            Err(CaldirError::Provider(ProviderError::ProviderNotFound(_)))
        ));
    }
}
