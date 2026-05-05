use crate::remote::provider::{PROVIDER_BINARY_PREFIX, Provider};

pub struct ProviderRegistry(Vec<Provider>);

impl ProviderRegistry {
    pub fn default() -> Self {
        Self::from_path_env()
    }

    // Check binaries starting with "caldir-provider-" in $PATH
    fn from_path_env() -> Self {
        let path = std::env::var_os("PATH").unwrap_or_default();

        let mut providers = Vec::new();

        for dir in std::env::split_paths(&path) {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };

            // Only get files that start with "caldir-provider-"
            for entry in entries.flatten() {
                if let Some(file_name) = entry.file_name().to_str() {
                    if !file_name.starts_with(PROVIDER_BINARY_PREFIX) {
                        continue;
                    }

                    let path = entry.path();

                    if let Some(provider) = Provider::from_binary_path(path) {
                        providers.push(provider);
                    }
                }
            }
        }

        Self(providers)
    }
}
