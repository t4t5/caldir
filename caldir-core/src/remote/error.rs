use super::ProviderError;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RemoteError {
    #[error(transparent)]
    Provider(#[from] ProviderError),
}
