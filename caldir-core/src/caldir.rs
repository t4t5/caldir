//! Master struct. Everything else flows from here.

use std::path::{Path, PathBuf};

pub use crate::caldir_builder::CaldirBuilder;
use crate::caldir_config::CaldirConfig;
use crate::calendar::Calendar;
use crate::error::{CalDirError, CalDirResult};
use crate::remote::provider::Provider;

pub struct Caldir {
    config_path: PathBuf,
    config: CaldirConfig,
    dir: PathBuf,
    providers: Vec<Provider>,
}

impl Caldir {
    pub fn builder() -> CaldirBuilder {
        CaldirBuilder::new()
    }

    /// Load from the current process environment.
    pub fn load() -> CalDirResult<Self> {
        Self::builder().build()
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub fn config(&self) -> &CaldirConfig {
        &self.config
    }

    pub fn config_mut(&mut self) -> &mut CaldirConfig {
        &mut self.config
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub(crate) fn from_resolved(
        config_path: PathBuf,
        config: CaldirConfig,
        dir: PathBuf,
        providers: Vec<Provider>,
    ) -> Self {
        Self {
            config_path,
            config,
            dir,
            providers,
        }
    }

    /// Persist the current config back to the config file.
    pub fn save_config(&self) -> CalDirResult<()> {
        self.config.save_to(&self.config_path)
    }

    pub fn provider_names(&self) -> Vec<String> {
        self.providers
            .iter()
            .map(|provider| provider.name().to_string())
            .collect()
    }

    pub fn provider(&self, name: &str) -> CalDirResult<Provider> {
        self.providers
            .iter()
            .find(|provider| provider.name() == name)
            .cloned()
            .ok_or_else(|| CalDirError::ProviderNotInstalled(name.to_string()))
    }

    /// Load a single calendar by slug, anchored at this caldir's directory.
    pub fn calendar(&self, slug: &str) -> CalDirResult<Calendar> {
        Calendar::load(slug, &self.dir)
    }

    /// Construct an in-memory calendar (not yet on disk) anchored at this
    /// caldir's directory. Used by the `connect` flow.
    pub fn new_calendar(
        &self,
        slug: &str,
        config: crate::calendar::config::CalendarConfig,
    ) -> Calendar {
        Calendar::new(slug, &self.dir, config)
    }

    /// Generate a slug for a new calendar with the given display name that
    /// doesn't collide with any existing directory in this caldir.
    pub fn unique_slug_for(&self, name: Option<&str>) -> CalDirResult<String> {
        Calendar::unique_slug(name, &self.dir)
    }

    /// Discover calendars by scanning the caldir for subdirectories.
    /// Every non-hidden directory is a calendar; `.caldir/config.toml`
    /// is optional and only carries metadata + remote sync settings.
    pub fn calendars(&self) -> CalDirResult<Vec<Calendar>> {
        let dir = &self.dir;

        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error.into()),
        };

        let mut calendars = Vec::new();
        for entry in entries {
            let path = entry?.path();
            if !path.is_dir() {
                continue;
            }

            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if name.starts_with('.') {
                continue;
            }

            calendars.push(Calendar::load(name, dir)?);
        }

        calendars.sort_by(|a, b| a.slug.cmp(&b.slug));
        Ok(calendars)
    }

    pub fn default_calendar_slug(&self) -> Option<&str> {
        self.config.default_calendar.as_deref()
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use crate::caldir_config::CaldirConfig;
    use std::ops::{Deref, DerefMut};
    use tempfile::TempDir;

    pub struct TestCaldir {
        _tmp: TempDir,
        caldir: Caldir,
    }

    impl TestCaldir {
        pub fn new() -> Self {
            let tmp = tempfile::tempdir().unwrap();
            let calendar_dir = tmp.path().join("caldir");
            let config_path = tmp.path().join("config.toml");
            std::fs::create_dir_all(&calendar_dir).unwrap();
            let caldir = Caldir::builder()
                .config_path(&config_path)
                .config(CaldirConfig {
                    calendar_dir,
                    ..CaldirConfig::new()
                })
                .without_providers()
                .build()
                .unwrap();

            Self { _tmp: tmp, caldir }
        }
    }

    impl Deref for TestCaldir {
        type Target = Caldir;

        fn deref(&self) -> &Self::Target {
            &self.caldir
        }
    }

    impl DerefMut for TestCaldir {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.caldir
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::caldir_config::CaldirConfig;
    use crate::error::CalDirError;
    use test_support::TestCaldir;

    #[cfg(unix)]
    fn make_executable(path: &std::path::Path) {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = std::fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).unwrap();
    }

    #[cfg(not(unix))]
    fn make_executable(_path: &std::path::Path) {}

    fn provider_binary_name(provider: &str) -> String {
        format!("caldir-provider-{provider}{}", std::env::consts::EXE_SUFFIX)
    }

    #[test]
    fn calendars_returns_error_for_invalid_calendar_config() {
        let caldir = TestCaldir::new();
        let config_dir = caldir.dir().join("work/.caldir");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(config_dir.join("config.toml"), "remote =").unwrap();

        let Err(error) = caldir.calendars() else {
            panic!("expected invalid calendar config to fail");
        };

        assert!(matches!(error, CalDirError::Config(_)));
    }

    #[test]
    fn new_calendar_appears_in_calendars_after_save() {
        use crate::calendar::config::CalendarConfig as PerCalendarConfig;

        let caldir = TestCaldir::new();
        assert!(caldir.calendars().unwrap().is_empty());

        let cal = caldir.new_calendar(
            "work",
            PerCalendarConfig {
                name: Some("Work".into()),
                color: Some("#0b8043".into()),
                ..PerCalendarConfig::default()
            },
        );
        cal.save_config().unwrap();

        let calendars = caldir.calendars().unwrap();
        assert_eq!(calendars.len(), 1);
        assert_eq!(calendars[0].slug, "work");
        assert_eq!(calendars[0].config.name.as_deref(), Some("Work"));
        assert_eq!(calendars[0].config.color.as_deref(), Some("#0b8043"));
        assert_eq!(calendars[0].dir(), caldir.dir().join("work"));
    }

    #[test]
    fn calendars_skips_hidden_dirs_files_and_dotcaldir() {
        let caldir = TestCaldir::new();
        let root = caldir.dir();

        std::fs::create_dir_all(root.join("personal")).unwrap();
        std::fs::create_dir_all(root.join("work")).unwrap();
        std::fs::create_dir_all(root.join(".caldir")).unwrap();
        std::fs::create_dir_all(root.join(".hidden")).unwrap();
        std::fs::write(root.join("notes.txt"), "").unwrap();

        let slugs: Vec<_> = caldir
            .calendars()
            .unwrap()
            .into_iter()
            .map(|c| c.slug)
            .collect();
        assert_eq!(slugs, vec!["personal", "work"]);
    }

    #[test]
    fn calendars_returns_empty_when_data_path_does_not_exist() {
        let tmp = tempfile::tempdir().unwrap();
        let caldir = Caldir::builder()
            .config_path(tmp.path().join("config.toml"))
            .config(CaldirConfig {
                calendar_dir: tmp.path().join("missing"),
                ..CaldirConfig::new()
            })
            .without_providers()
            .build()
            .unwrap();

        assert!(caldir.calendars().unwrap().is_empty());
    }

    #[test]
    fn save_config_writes_to_the_configured_path() {
        let mut caldir = TestCaldir::new();
        let config_path = caldir.config_path().to_path_buf();

        assert!(caldir.config_mut().set_default_calendar_if_unset("work"));
        caldir.save_config().unwrap();

        let contents = std::fs::read_to_string(config_path).unwrap();
        assert!(contents.contains("default_calendar = \"work\""));
    }

    #[test]
    fn paths_are_resolved_when_caldir_is_built() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join("config.toml");

        let caldir = Caldir::builder()
            .config_path(&config_path)
            .config(CaldirConfig {
                calendar_dir: "~/calendars".into(),
                providers_data_dir: Some("~/provider-data".into()),
                ..CaldirConfig::new()
            })
            .without_providers()
            .build()
            .unwrap();

        assert_eq!(caldir.config().calendar_dir, PathBuf::from("~/calendars"));
        assert_eq!(
            caldir.config().providers_data_dir,
            Some(PathBuf::from("~/provider-data"))
        );
        let expected = crate::utils::expand_tilde(std::path::Path::new("~/calendars"));
        assert_eq!(caldir.dir(), expected.as_path());
    }

    #[test]
    fn provider_data_is_stored_next_to_config_by_default() {
        let home = tempfile::tempdir().unwrap();

        let bin_dir = home.path().join("bin");
        std::fs::create_dir_all(&bin_dir).unwrap();
        let provider_binary = bin_dir.join(provider_binary_name("google"));
        std::fs::write(&provider_binary, "").unwrap();
        make_executable(&provider_binary);

        let random_path = home.path().join("random_path");
        let config_path = random_path.join("config.toml");

        let caldir = Caldir::builder()
            .config_path(&config_path)
            .provider_search_dirs([&bin_dir])
            .build()
            .unwrap();

        assert_eq!(
            caldir.provider("google").unwrap().dir(),
            random_path.join("providers/google")
        );
    }

    #[test]
    fn provider_data_can_be_stored_elsewhere() {
        let home = tempfile::tempdir().unwrap();

        let bin_dir = home.path().join("bin");
        std::fs::create_dir_all(&bin_dir).unwrap();
        let provider_binary = bin_dir.join(provider_binary_name("google"));
        std::fs::write(&provider_binary, "").unwrap();
        make_executable(&provider_binary);

        let random_path = home.path().join("random_path");
        let config_path = random_path.join("config.toml");

        let custom_data_path = home.path().join("elsewhere");
        CaldirConfig {
            calendar_dir: home.path().join("calendars"),
            providers_data_dir: Some(custom_data_path.clone()),
            ..CaldirConfig::new()
        }
        .save_to(&config_path)
        .unwrap();

        let caldir = Caldir::builder()
            .config_path(&config_path)
            .provider_search_dirs([&bin_dir])
            .build()
            .unwrap();

        assert_eq!(
            caldir.provider("google").unwrap().dir(),
            custom_data_path.join("google")
        );
    }
}
