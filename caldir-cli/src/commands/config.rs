use anyhow::Result;
use caldir_core::caldir::Caldir;
use caldir_core::caldir_config::CaldirConfig;
use owo_colors::OwoColorize;

pub fn run() -> Result<()> {
    let config_path = CaldirConfig::config_path().map_err(|e| anyhow::anyhow!(e))?;
    let caldir = Caldir::load().map_err(|e| anyhow::anyhow!(e))?;

    println!("{}", "Paths".bold());
    println!("  Config:     {}", config_path.display());
    println!("  Calendars:  {}", caldir.data_path().display());
    println!(
        "  Providers:  {}",
        config_path
            .parent()
            .map(|p| p.join("providers"))
            .unwrap_or_default()
            .display()
    );

    Ok(())
}
