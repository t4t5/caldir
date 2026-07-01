use serde::Serialize;

/// Human-readable terminal rendering for a command's result.
pub trait TextRender {
    fn to_text(&self) -> String;
}

/// A command result renderable as either human text or JSON.
/// Blanket-implemented for any `Serialize + TextRender`.
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
pub fn emit(output: &dyn Output, format: OutputFormat) {
    match format {
        OutputFormat::Text => println!("{}", output.to_text()),
        OutputFormat::Json => println!(
            "{}",
            serde_json::to_string_pretty(&output.to_json()).unwrap()
        ),
    }
}
