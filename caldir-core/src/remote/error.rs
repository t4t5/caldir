use super::ProviderError;

#[derive(Debug, thiserror::Error)]
pub enum RemoteError {
    #[error("Provider error: {0}")]
    Provider(#[from] ProviderError),
}
