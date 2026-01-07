use indicatif::{ProgressBar, ProgressStyle};

pub fn create_spinner(message: String) -> ProgressBar {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&["-", "\\", "|", "/"])
            .template("{msg} {spinner}")
            .unwrap(),
    );
    spinner.set_message(message);
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));
    spinner
}
