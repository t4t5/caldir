mod config;

use crate::Provider;
pub use config::RemoteConfig;

pub struct Remote {
    provider: Provider,
    config: RemoteConfig,
}

impl Remote {
    // can list, create, delete events through RPC calls to the provider
}
