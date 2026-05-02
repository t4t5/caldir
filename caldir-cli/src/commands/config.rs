use anyhow::Result;
use caldir_core::caldir::Caldir;
use caldir_core::caldir_config::CaldirConfig;
use owo_colors::OwoColorize;

use crate::utils::path::PathExt;

pub fn run() -> Result<()> {
    let config_path = CaldirConfig::config_path().map_err(|e| anyhow::anyhow!(e))?;
    let caldir = Caldir::load().map_err(|e| anyhow::anyhow!(e))?;

    println!("{} {}", "Path:".bold(), config_path.tilde());
    println!();
    println!("{}", caldir.config());

    Ok(())
}
