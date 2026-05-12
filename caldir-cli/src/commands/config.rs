use anyhow::Result;
use caldir_core::{Caldir, CaldirConfig};
use owo_colors::OwoColorize;

pub fn run(caldir: &Caldir) -> Result<()> {
    println!(
        "{} {}",
        "Path:".bold(),
        CaldirConfig::default_system_config_path()?.display()
    );
    println!();
    println!("{}", caldir.config());

    Ok(())
}
