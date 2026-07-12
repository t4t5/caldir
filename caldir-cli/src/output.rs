use anyhow::Result;
use serde::Serialize;

/// Human text rendering for a command's result.
pub trait TextRender {
    fn to_text(&self) -> String;
}

/// A command result renderable as either human text or JSON.
pub trait Output: TextRender {
    fn to_json(&self) -> serde_json::Value;
}

impl<T: Serialize + TextRender> Output for T {
    fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("view serialization is infallible")
    }
}

#[derive(Clone, Copy)]
pub enum OutputFormat {
    Text,
    Json,
}

/// Render a command result to stdout in the chosen format.
pub fn emit(output: &dyn Output, format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Text => println!("{}", output.to_text()),
        OutputFormat::Json => {
            serde_json::to_writer_pretty(std::io::stdout(), &output.to_json())?;
            println!();
        }
    }

    Ok(())
}
