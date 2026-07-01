use anyhow::Result;
use owo_colors::OwoColorize;
use std::io::Write;
use std::path::PathBuf;

#[derive(Debug)]
pub(crate) enum DoctorWarning {
    DuplicateFiles(Vec<PathBuf>),
    UnreadableEvents(String),
}

impl DoctorWarning {
    pub(crate) fn render(&self, out: &mut impl Write) -> Result<()> {
        match self {
            DoctorWarning::DuplicateFiles(paths) => {
                writeln!(
                    out,
                    "   {} same event saved as multiple files; delete all but one:",
                    "⚠".yellow()
                )?;
                for path in paths {
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                    writeln!(out, "       {}", name.dimmed())?;
                }
            }
            DoctorWarning::UnreadableEvents(error) => {
                writeln!(out, "   {}", error.red())?;
            }
        }

        Ok(())
    }
}
