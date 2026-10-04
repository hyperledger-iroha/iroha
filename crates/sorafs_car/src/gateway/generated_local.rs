//! Exact owner-selected local intent joined to the existing native provider verifier.
//!
//! Bare certificate pins are not admission authority. Only `authenticate_current` can produce
//! the transport consumed by HTTP clients; the environment still owns finality freshness.

use std::{
    fmt,
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
};

use iroha_data_model::{
    NetworkId,
    account::AccountId,
    sorafs::{
        capacity::ProviderId,
        provider_admission::discovery::account_read::VerifiedAccountReadProviderV1,
    },
};
use sorafs_manifest::{
    provider_admission::{EndpointAttestationKind, ProviderAdmissionGenesisMaterialV1},
    provider_advert::{EndpointKind, account_read::RegisteredAccountReadV1},
};

use super::{GatewayBuildError, GatewayProviderInput, Url};
use rustls::client::danger::ServerCertVerifier;

const MATERIAL_MAX: usize = 64 * 1024;
const CERT_MAX: usize = 16 * 1024;
const MATERIAL_LIMITS: norito::DecodeLimits = norito::DecodeLimits::new(
    MATERIAL_MAX,
    MATERIAL_MAX,
    MATERIAL_MAX * 2,
    2 * 1024 * 1024,
    48,
);

/// Owner-selected original generated-local transport intent, without native authority.
///
/// The managed caller authenticates retained signed genesis before selecting this value. A
/// caller cannot obtain transport authority merely by providing matching-looking fields.
#[derive(Clone)]
pub struct GeneratedLocalProviderTransportV1(Arc<Original>);

struct Original {
    network: NetworkId,
    chain: String,
    provider: ProviderId,
    owner: AccountId,
    material: ProviderAdmissionGenesisMaterialV1,
    url: Url,
    socket: SocketAddr,
}

impl fmt::Debug for GeneratedLocalProviderTransportV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GeneratedLocalProviderTransportV1")
            .finish_non_exhaustive()
    }
}

/// Exact original local transport joined to an opaque current native account-read proof.
///
/// This contains no signer and grants no service activation or transaction authority. The
/// caller must repeat the current join for each new token/session, after fresh finality.
#[derive(Clone, Debug)]
pub struct AuthenticatedGeneratedLocalProviderTransportV1(GeneratedLocalProviderTransportV1);

/// Closed, redacted refusal of generated-local transport intent or native binding.
#[derive(Clone, Copy, Debug, thiserror::Error)]
#[error(
    "generated local provider transport does not match its selected original and current native authority"
)]
pub struct GeneratedLocalProviderTransportErrorV1;

impl GeneratedLocalProviderTransportV1 {
    /// Select bounded original intent, without reading files, resolving DNS or sending requests.
    ///
    /// Only the fixed provider-hash `.localhost` name and its explicit original port are admitted.
    /// The sole destination is IPv4 loopback; arbitrary IP addresses and trust stores are absent.
    /// # Errors
    /// Refuses noncanonical scope, excessive material or a different generated endpoint shape.
    pub fn select(
        network_id: NetworkId,
        chain_id: &str,
        provider: ProviderId,
        owner: &AccountId,
        material: &ProviderAdmissionGenesisMaterialV1,
    ) -> Result<Self, GeneratedLocalProviderTransportErrorV1> {
        norito::core::with_decode_limits_scope(MATERIAL_LIMITS, || {
            let fail = || GeneratedLocalProviderTransportErrorV1;
            if chain_id.is_empty()
                || chain_id.len() > 256
                || chain_id.trim() != chain_id
                || chain_id.chars().any(char::is_control)
                || provider.as_bytes() == &[0; 32]
                || material.proposal.provider_id != *provider.as_bytes()
                || material.proposal.endpoints.len() != 1
                || material.retention_epoch == u64::MAX
            {
                return Err(fail());
            }
            // Bound the complete graph before the native validator or any retained graph copy.
            let _flags =
                norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
            if norito::core::encoded_frame_len(owner).map_err(|_| fail())? > 4096 {
                return Err(fail());
            }
            let length = norito::core::encoded_frame_len(material).map_err(|_| fail())?;
            if length > MATERIAL_MAX {
                return Err(fail());
            }
            material.validate().map_err(|_| fail())?;
            let endpoint = &material.proposal.endpoints[0];
            let tls = &endpoint.attestation;
            let hex = hex::encode(provider.as_bytes());
            let hostname = format!("{}.{}.localhost", &hex[..32], &hex[32..]);
            let policy =
                RegisteredAccountReadV1::from_capabilities(&material.proposal.capabilities)
                    .map_err(|_| fail())?
                    .ok_or_else(fail)?;
            let origin = policy.https_origin().map_err(|_| fail())?;
            let url = Url::parse(&format!("{origin}/")).map_err(|_| fail())?;
            let port = url
                .port_or_known_default()
                .filter(|p| *p != 0)
                .ok_or_else(fail)?;
            if endpoint.endpoint.kind != EndpointKind::Torii
                || endpoint.endpoint.host_pattern != hostname
                || tls.kind != EndpointAttestationKind::Tls
                || tls.attested_at != material.issued_at
                || tls.expires_at != material.retention_epoch
                || tls.leaf_certificate.is_empty()
                || tls.leaf_certificate.len() > CERT_MAX
                || tls.intermediate_certificates.len() != 1
                || tls.intermediate_certificates[0].is_empty()
                || tls.intermediate_certificates[0].len() > CERT_MAX
                || tls.alpn_ids != ["http/1.1"]
                || !tls.report.is_empty()
                || url.host_str() != Some(hostname.as_str())
                || url.scheme() != "https"
                || !url.username().is_empty()
                || url.password().is_some()
                || url.query().is_some()
                || url.fragment().is_some()
                || url.path() != "/"
                || url.as_str() != format!("{origin}/")
            {
                return Err(fail());
            }
            // Canonical bounded ownership uses the shared codec, with one aggregate decode scope.
            norito::core::reserve_decode_allocation(length).map_err(|_| fail())?;
            let bytes = norito::core::to_bytes_bounded(material, length).map_err(|_| fail())?;
            let material = norito::decode_canonical_with_limits(&bytes, MATERIAL_LIMITS)
                .map_err(|_| fail())?;
            Ok(Self(Arc::new(Original {
                network: network_id,
                chain: chain_id.to_owned(),
                provider,
                owner: owner.clone(),
                material,
                url,
                socket: SocketAddr::from((Ipv4Addr::LOCALHOST, port)),
            })))
        })
    }

    /// Original network choice; this accessor is not authenticated current state.
    #[must_use]
    pub fn network_id(&self) -> NetworkId {
        self.0.network
    }
    /// Original chain label; this accessor is not authenticated current state.
    #[must_use]
    pub fn chain_id(&self) -> &str {
        &self.0.chain
    }
    /// Exact original provider choice.
    #[must_use]
    pub fn provider_id(&self) -> ProviderId {
        self.0.provider
    }

    /// Join the exact retained original with the existing native provider proof owner.
    /// # Errors
    /// Refuses changed network, chain, owner, origin, endpoint material or admission lineage.
    pub fn authenticate_current(
        &self,
        current: &VerifiedAccountReadProviderV1,
    ) -> Result<
        AuthenticatedGeneratedLocalProviderTransportV1,
        GeneratedLocalProviderTransportErrorV1,
    > {
        let discovery = current.discovery();
        let admission = discovery.admission();
        let envelope = admission.envelope();
        let original = &self.0;
        if discovery.network_id() != original.network
            || current.chain_id() != original.chain
            || discovery.owner() != &original.owner
            || discovery.advert().body.provider_id != *original.provider.as_bytes()
            || !admission.is_genesis_material()
            || envelope.proposal != original.material.proposal
            || envelope.advert_body != original.material.advert_body
            || envelope.issued_at != original.material.issued_at
            || envelope.retention_epoch != original.material.retention_epoch
            || current.policy().https_origin().ok().as_deref()
                != Some(original.url.as_str().trim_end_matches('/'))
        {
            return Err(GeneratedLocalProviderTransportErrorV1);
        }
        Ok(AuthenticatedGeneratedLocalProviderTransportV1(self.clone()))
    }
}

impl AuthenticatedGeneratedLocalProviderTransportV1 {
    /// Exact original HTTPS root URL, including its selected nonzero port.
    #[must_use]
    pub fn base_url(&self) -> &Url {
        &self.0.0.url
    }
    /// Sole original local socket; the original DNS hostname remains the TLS server name.
    #[must_use]
    pub fn socket_addr(&self) -> SocketAddr {
        self.0.0.socket
    }
    /// Exact original CA replacing platform roots. Ordinary TLS name/time checks still apply.
    #[must_use]
    pub fn ca_certificate_der(&self) -> &[u8] {
        &self.0.0.material.proposal.endpoints[0]
            .attestation
            .intermediate_certificates[0]
    }
    /// Construct the bounded blocking control client using this exact native-joined selection.
    ///
    /// The account adapter supplies paths beneath `base_url`; no unrelated default headers,
    /// proxy, platform roots, redirect, automatic retry or DNS fallback are installed.
    /// # Errors
    /// Refuses invalid timeout or certificate material and HTTP client construction failure.
    pub fn blocking_http_client(
        &self,
        connect_timeout: std::time::Duration,
        request_timeout: std::time::Duration,
    ) -> Result<reqwest::blocking::Client, GatewayBuildError> {
        self.validate_timeouts(connect_timeout, request_timeout)?;
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
            .resolve_to_addrs(self.host()?, &[self.socket_addr()])
            .build()
            .map_err(GatewayBuildError::ClientBuild)
    }
    pub(super) fn async_http_client(
        &self,
        connect_timeout: std::time::Duration,
        request_timeout: std::time::Duration,
    ) -> Result<reqwest::Client, GatewayBuildError> {
        self.validate_timeouts(connect_timeout, request_timeout)?;
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
            .resolve_to_addrs(self.host()?, &[self.socket_addr()])
            .build()
            .map_err(GatewayBuildError::ClientBuild)
    }
    fn validate_timeouts(
        &self,
        connect: std::time::Duration,
        request: std::time::Duration,
    ) -> Result<(), GatewayBuildError> {
        if connect.is_zero() || request.is_zero() || connect > request {
            return Err(GatewayBuildError::InvalidTimeouts);
        }
        Ok(())
    }
    fn tls_config(&self) -> Result<rustls::ClientConfig, GatewayBuildError> {
        use rustls::{RootCertStore, client::WebPkiServerVerifier, pki_types::CertificateDer};
        let mut roots = RootCertStore::empty();
        roots
            .add(CertificateDer::from(self.ca_certificate_der().to_vec()))
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
                original: self.0.0.clone(),
            }))
            .with_no_client_auth();
        config.enable_early_data = false;
        Ok(config)
    }
    fn host(&self) -> Result<&str, GatewayBuildError> {
        self.base_url()
            .host_str()
            .ok_or(GatewayBuildError::InvalidGeneratedLocalTransport)
    }
    pub(super) fn validate_input(
        &self,
        input: &GatewayProviderInput,
    ) -> Result<Url, GatewayBuildError> {
        if input.provider_id_hex != hex::encode(self.0.0.provider.as_bytes())
            || input.base_url != self.base_url().as_str()
            || input.privacy_events_url.is_some()
        {
            return Err(GatewayBuildError::InvalidGeneratedLocalTransport);
        }
        Ok(self.base_url().clone())
    }
}

struct OriginalLeafVerifier {
    standard: Arc<rustls::client::WebPkiServerVerifier>,
    original: Arc<Original>,
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
        if end_entity.as_ref()
            != self.original.material.proposal.endpoints[0]
                .attestation
                .leaf_certificate
        {
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

#[cfg(test)]
mod tests;
