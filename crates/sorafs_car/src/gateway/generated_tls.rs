//! Shared generated-local TLS identity. This owner only narrows ordinary WebPKI validation.

use super::{GatewayBuildError, Url};
use rustls::client::danger::ServerCertVerifier;
use std::{fmt, net::SocketAddr, sync::Arc, time::Duration};

pub(super) struct OriginalTlsIdentity {
    root: Arc<[u8]>,
    leaf: Arc<[u8]>,
}
impl OriginalTlsIdentity {
    pub(super) fn new(root: &[u8], leaf: &[u8]) -> Self {
        // Both callers select the same bounded original material before this private copy.
        Self {
            root: Arc::from(root),
            leaf: Arc::from(leaf),
        }
    }
    fn validate_timeouts(connect: Duration, request: Duration) -> Result<(), GatewayBuildError> {
        if connect.is_zero() || request.is_zero() || connect > request {
            return Err(GatewayBuildError::InvalidTimeouts);
        }
        Ok(())
    }
    pub(super) fn blocking_http_client(
        &self,
        base: &Url,
        socket: SocketAddr,
        connect_timeout: Duration,
        request_timeout: Duration,
    ) -> Result<reqwest::blocking::Client, GatewayBuildError> {
        Self::validate_timeouts(connect_timeout, request_timeout)?;
        let host = base
            .host_str()
            .ok_or(GatewayBuildError::InvalidGeneratedLocalTransport)?;
        reqwest::blocking::Client::builder()
            .no_proxy()
            .no_gzip()
            .no_brotli()
            .no_deflate()
            .no_zstd()
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .connect_timeout(connect_timeout)
            .timeout(request_timeout)
            .use_preconfigured_tls(self.tls_config()?)
            .resolve_to_addrs(host, &[socket])
            .build()
            .map_err(GatewayBuildError::ClientBuild)
    }
    pub(super) fn async_http_client(
        &self,
        base: &Url,
        socket: SocketAddr,
        connect_timeout: Duration,
        request_timeout: Duration,
    ) -> Result<reqwest::Client, GatewayBuildError> {
        Self::validate_timeouts(connect_timeout, request_timeout)?;
        let host = base
            .host_str()
            .ok_or(GatewayBuildError::InvalidGeneratedLocalTransport)?;
        reqwest::Client::builder()
            .no_proxy()
            .no_gzip()
            .no_brotli()
            .no_deflate()
            .no_zstd()
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .connect_timeout(connect_timeout)
            .timeout(request_timeout)
            .use_preconfigured_tls(self.tls_config()?)
            .resolve_to_addrs(host, &[socket])
            .build()
            .map_err(GatewayBuildError::ClientBuild)
    }
    fn tls_config(&self) -> Result<rustls::ClientConfig, GatewayBuildError> {
        use rustls::{RootCertStore, client::WebPkiServerVerifier, pki_types::CertificateDer};
        let mut roots = RootCertStore::empty();
        roots
            .add(CertificateDer::from(self.root.to_vec()))
            .map_err(|_| GatewayBuildError::InvalidPinnedTlsRoots)?;
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let standard =
            WebPkiServerVerifier::builder_with_provider(Arc::new(roots), provider.clone())
                .build()
                .map_err(|_| GatewayBuildError::InvalidPinnedTlsRoots)?;
        // This Rustls customization only narrows normal WebPKI validation: the delegating
        // verifier additionally requires the exact original leaf before any HTTP is sent.
        let mut config = rustls::ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map_err(|_| GatewayBuildError::InvalidPinnedTlsRoots)?
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(OriginalLeafVerifier {
                standard,
                leaf: self.leaf.clone(),
            }))
            .with_no_client_auth();
        config.enable_early_data = false;
        Ok(config)
    }
}

struct OriginalLeafVerifier {
    standard: Arc<rustls::client::WebPkiServerVerifier>,
    leaf: Arc<[u8]>,
}
impl fmt::Debug for OriginalLeafVerifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OriginalLeafVerifier")
            .finish_non_exhaustive()
    }
}
impl ServerCertVerifier for OriginalLeafVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &rustls::pki_types::CertificateDer<'_>,
        intermediates: &[rustls::pki_types::CertificateDer<'_>],
        server_name: &rustls::pki_types::ServerName<'_>,
        ocsp_response: &[u8],
        now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        let verified = self.standard.verify_server_cert(
            end_entity,
            intermediates,
            server_name,
            ocsp_response,
            now,
        )?;
        if end_entity.as_ref() != self.leaf.as_ref() {
            return Err(rustls::Error::General(
                "original generated provider leaf differs".into(),
            ));
        }
        Ok(verified)
    }
    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        self.standard.verify_tls12_signature(message, cert, dss)
    }
    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        self.standard.verify_tls13_signature(message, cert, dss)
    }
    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.standard.supported_verify_schemes()
    }
}
