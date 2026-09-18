use super::ProviderError;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RemoteError {
    #[error("Provider error: {0}")]
    Provider(#[from] ProviderError),
}
