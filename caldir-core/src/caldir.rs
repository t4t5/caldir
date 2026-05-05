pub mod config;

use config::CaldirConfig;

pub struct Caldir {
    config: CaldirConfig,
}

impl Caldir {
    pub fn new(config: CaldirConfig) -> Self {
        Caldir { config }
    }
}

impl Default for Caldir {
    fn default() -> Self {
        Self::new(CaldirConfig::default())
    }
}
