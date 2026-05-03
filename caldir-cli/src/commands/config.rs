use anyhow::Result;
use caldir_core::caldir::Caldir;
use owo_colors::OwoColorize;

use crate::utils::path::PathExt;

pub fn run(caldir: &Caldir) -> Result<()> {
    match caldir.config_path() {
        Some(path) => println!("{} {}", "Path:".bold(), path.tilde()),
        None => println!("{} (memory)", "Path:".bold()),
    }
    println!();
    println!("{}", caldir.config());

    Ok(())
}
