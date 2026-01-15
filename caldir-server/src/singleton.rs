//! Singleton pattern to ensure only one caldir-server instance runs.

use anyhow::{Context, Result};
use fs2::FileExt;
use std::fs::{self, File};
use std::path::PathBuf;

/// A lock guard that releases the lock when dropped
pub struct LockGuard {
    _file: File,
}

fn lock_path() -> Result<PathBuf> {
    let runtime_dir = dirs::runtime_dir()
        .or_else(|| dirs::cache_dir())
        .ok_or_else(|| anyhow::anyhow!("Could not determine runtime directory"))?;

    let dir = runtime_dir.join("caldir");
    fs::create_dir_all(&dir)?;

    Ok(dir.join("server.lock"))
}

/// Acquire an exclusive lock, failing if another instance is running
pub fn acquire_lock() -> Result<LockGuard> {
    let path = lock_path()?;
    let file = File::create(&path).context("Failed to create lock file")?;

    file.try_lock_exclusive().map_err(|_| {
        anyhow::anyhow!(
            "Another caldir-server instance is already running.\n\
            If you believe this is an error, remove: {}",
            path.display()
        )
    })?;

    Ok(LockGuard { _file: file })
}
