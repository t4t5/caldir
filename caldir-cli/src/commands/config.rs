use anyhow::Result;
use caldir_core::caldir::Caldir;
use owo_colors::OwoColorize;

use crate::utils::path::PathExt;

pub fn run(caldir: &Caldir) -> Result<()> {
    println!("{} {}", "Path:".bold(), caldir.config_path().tilde());
    println!();
    println!("{}", caldir.settings().config());

    Ok(())
}
