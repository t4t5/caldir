use caldir_core::CaldirConfig;
use serde::Serialize;
use std::path::PathBuf;

use crate::output::TextRender;

#[derive(Serialize)]
pub struct ConfigView {
    pub path: PathBuf,
    pub config: CaldirConfig,
}

impl TextRender for ConfigView {
    fn to_text(&self) -> String {
        format!("Path: {}\n\n{}", self.path.display(), self.config)
            .trim_end()
            .to_string()
    }
}
