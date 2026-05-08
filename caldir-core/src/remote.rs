mod config;

use crate::Provider;
pub use config::{RemoteConfig, RemoteConfigParams};

/// remote = a resolved provider with a config
pub struct Remote {
    provider: Provider,
    config: RemoteConfig,
}

impl Remote {
    pub fn new(provider: Provider, config: RemoteConfig) -> Self {
        Self { provider, config }
    }

    // can list, create, delete events through RPC calls to the provider
}
