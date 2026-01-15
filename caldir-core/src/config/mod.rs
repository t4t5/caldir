//! Configuration types for caldir.

mod global;
mod local;

pub use global::GlobalConfig;
pub use local::{LocalConfig, RemoteConfig};
