use std::sync::Arc;
use std::time::SystemTime;

use aws_smithy_http_client::hyper_014::HyperClientBuilder;
use aws_smithy_runtime_api::client::http::SharedHttpClient;
use rustls::client::{ServerCertVerified, ServerCertVerifier};
use rustls::{Certificate, Error, ServerName};

/// Returns true when the endpoint URL uses HTTPS.
pub fn endpoint_uses_https(endpoint: Option<&str>) -> bool {
    endpoint
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some_and(|value| value.starts_with("https://"))
}

#[derive(Debug)]
struct SkipTlsVerifier;

impl ServerCertVerifier for SkipTlsVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &Certificate,
        _intermediates: &[Certificate],
        _server_name: &ServerName,
        _scts: &mut dyn Iterator<Item = &[u8]>,
        _ocsp: &[u8],
        _now: SystemTime,
    ) -> Result<ServerCertVerified, Error> {
        Ok(ServerCertVerified::assertion())
    }
}

/// Build an HTTP client that skips TLS certificate verification.
pub fn build_insecure_http_client() -> SharedHttpClient {
    let mut tls_config = rustls::ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(rustls::RootCertStore::empty())
        .with_no_client_auth();
    tls_config
        .dangerous()
        .set_certificate_verifier(Arc::new(SkipTlsVerifier));

    let connector = hyper_rustls::HttpsConnectorBuilder::new()
        .with_tls_config(tls_config)
        .https_or_http()
        .enable_http1()
        .build();

    HyperClientBuilder::new().build(connector)
}

#[cfg(test)]
mod tests {
    use super::endpoint_uses_https;

    #[test]
    fn endpoint_uses_https_detects_https_scheme() {
        assert!(endpoint_uses_https(Some("https://minio.local:9000")));
        assert!(endpoint_uses_https(Some("  https://example.com  ")));
    }

    #[test]
    fn endpoint_uses_https_rejects_http_empty_and_missing() {
        assert!(!endpoint_uses_https(Some("http://127.0.0.1:9000")));
        assert!(!endpoint_uses_https(Some("")));
        assert!(!endpoint_uses_https(Some("   ")));
        assert!(!endpoint_uses_https(None));
    }
}
