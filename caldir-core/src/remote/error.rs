use super::ProviderError;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RemoteError {
    #[error("failed to create remote event {0}")]
    CreateEvent(String, #[source] ProviderError),

    #[error("failed to update remote event {0}")]
    UpdateEvent(String, #[source] ProviderError),

    #[error("failed to delete remote event {0}")]
    DeleteEvent(String, #[source] ProviderError),

    #[error(transparent)]
    Provider(#[from] ProviderError),
}
