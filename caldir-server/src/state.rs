use anyhow::Result;
use caldir_lib::Caldir;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    // We reload Caldir on each request to pick up filesystem changes
    // In the future, could add caching with file watching
}

impl AppState {
    pub fn new() -> Result<Self> {
        // Verify caldir can be loaded at startup
        let _ = Caldir::load()?;
        Ok(AppState {})
    }

    pub fn caldir(&self) -> Result<Caldir> {
        Caldir::load()
    }
}
