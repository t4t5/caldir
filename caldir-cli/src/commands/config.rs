use std::path::PathBuf;

use anyhow::Result;
use caldir_core::caldir::Caldir;
use caldir_core::caldir_config::CaldirConfig;
use owo_colors::OwoColorize;

pub fn run() -> Result<()> {
    let config_path = CaldirConfig::config_path().map_err(|e| anyhow::anyhow!(e))?;
    let caldir = Caldir::load().map_err(|e| anyhow::anyhow!(e))?;

    let config_display = std::env::var("HOME")
        .ok()
        .and_then(|home| {
            config_path
                .strip_prefix(&home)
                .ok()
                .map(|p| PathBuf::from("~").join(p))
        })
        .unwrap_or_else(|| config_path.clone());

    println!("{}", "Paths".bold());
    println!("  Config:     {}", config_display.display());
    println!("  Calendars:  {}", caldir.display_path().display());

    Ok(())
}
