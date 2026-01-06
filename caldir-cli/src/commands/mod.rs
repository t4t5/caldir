use indicatif::{ProgressBar, ProgressStyle};

// pub mod auth;
// pub mod new;
pub mod pull;
// pub mod push;
pub mod status;

/// Number of days to sync in each direction (past and future)
pub const SYNC_DAYS: i64 = 365;

/// Create and start a spinner with the given message.
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
