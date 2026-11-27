//! TLS of the connections the agent opens itself (mail, Home Assistant): the certificate is checked by Windows.

use std::sync::Arc;

use rustls_platform_verifier::BuilderVerifierExt;
use tokio_rustls::rustls::{self, ClientConfig};

/// The crypto provider is named here. The application holds two of them (aws-lc-rs of the HTTP client, ring of the
/// updater), and then rustls does not choose one by itself: `ClientConfig::builder()` panics.
pub fn client_config() -> Result<ClientConfig, rustls::Error> {
    let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
    Ok(ClientConfig::builder_with_provider(provider).with_safe_default_protocol_versions()?.with_platform_verifier()?.with_no_client_auth())
}

#[cfg(test)]
mod tests {
    /// Fails in a build of the whole workspace when the provider is left to rustls; a build of this crate alone has one only.
    #[test]
    fn a_client_configuration_is_made_whatever_providers_the_build_holds() {
        assert!(super::client_config().is_ok());
    }
}
