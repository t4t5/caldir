use crate::calendar::CalendarError;
use crate::remote::RemoteError;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ConnectionError {
    #[error(transparent)]
    Remote(#[from] RemoteError),

    #[error(transparent)]
    Calendar(#[from] CalendarError),

    /// Some outgoing changes failed; the rest were still pushed.
    #[error("{}", format_push_failures(.0))]
    PushFailures(Vec<RemoteError>),
}

fn format_push_failures(errors: &[RemoteError]) -> String {
    errors
        .iter()
        .map(|error| error_chain(error))
        .collect::<Vec<_>>()
        .join("\n")
}

fn error_chain(error: &dyn std::error::Error) -> String {
    let mut chain = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        chain.push_str(": ");
        chain.push_str(&cause.to_string());
        source = cause.source();
    }
    chain
}
