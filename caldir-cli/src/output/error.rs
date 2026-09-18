pub(crate) fn format_error(error: impl Into<anyhow::Error>) -> String {
    let error = error.into();
    format!("{error:#}")
}

#[cfg(test)]
mod tests {
    use super::format_error;
    use caldir_core::{ConnectionError, ProviderError, ProviderTransportError, RemoteError};

    #[test]
    fn renders_context_and_cause_once_through_transparent_wrappers() {
        let error = ConnectionError::from(RemoteError::from(ProviderError::from(
            ProviderTransportError::Spawn(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "permission denied",
            )),
        )));

        assert_eq!(
            format_error(error),
            "failed to spawn provider: permission denied"
        );
    }

    #[test]
    fn renders_anyhow_context_chain() {
        assert_eq!(
            format_error(anyhow::anyhow!("inner").context("outer")),
            "outer: inner"
        );
    }

    #[test]
    fn preserves_opaque_provider_text() {
        let message = "  Error handling request: déjà vu?!\n  outer: outer\t ";
        assert_eq!(
            format_error(ProviderError::Provider(message.into())),
            message
        );
    }
}
