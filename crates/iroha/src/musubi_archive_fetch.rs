//! Production authenticated `SoraFS` transport used by Musubi archive fetching.
//!
//! Provider identities, origins, exact network identity, and operator-key files come from the
//! explicit platform client configuration. The transport validates the canonical `SoraFS`
//! manifest and every page of the storage plan against the immutable Musubi
//! archive commitment, mints a short-lived provider-bound stream token, and
//! regenerates the canonical CAR through a bounded reader.
use std::{
    collections::{BTreeMap, HashSet},
    fmt, fs,
    io::{self, Cursor, Read, Write},
    net::{IpAddr, ToSocketAddrs},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
mod bounded_stream;
mod json_preflight;
use crate::{
    client::Client,
    config::{MusubiFetchConfig, MusubiFetchProviderGatewayConfig},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use iroha_crypto::{ExposedPrivateKey, KeyPair, PrivateKey, PublicKey, Signature};
use iroha_data_model::{
    NetworkId,
    musubi::{
        ArchiveId, MUSUBI_MAX_CAR_BYTES_V1, MUSUBI_MAX_CHUNKS_V1, MUSUBI_MAX_FILES_V1,
        MusubiArchiveCommitmentV1,
    },
    sorafs::{capacity::ProviderId, pin_registry::ManifestDigest},
};
use json_preflight::{JsonDomEnvelopeV1, preflight_json_dom};
use rand::{TryRngCore, rngs::OsRng};
use reqwest::{
    StatusCode,
    blocking::{Client as HttpClient, Response as HttpResponse},
    header::{CONTENT_ENCODING, CONTENT_TYPE, HeaderMap},
    redirect::Policy as RedirectPolicy,
};
use sorafs_car::{
    CarBuildPlan, CarChunk, CarStreamingWriter, ChunkFetchSpec, FilePlan,
    compute_chunk_plan_digest_sha3,
    gateway::{GatewayFetchConfig, GatewayFetchContext, GatewayFetchError, GatewayProviderInput},
    multi_fetch::{FetchProvider, FetchRequest},
};
use sorafs_manifest::{
    ManifestV1, PinPolicyConstraints, STREAM_TOKEN_MAX_BASE64_BYTES_V1,
    STREAM_TOKEN_MAX_WIRE_BYTES_V1, StreamTokenV1, decode_manifest_v1_base64_canonical,
    validate_manifest, validate_registered_chunker_profile,
};
use url::{Host, Url};
const CLIENT_HEADER: &str = "x-sorafs-client";
const NONCE_HEADER: &str = "x-sorafs-nonce";
const VERIFYING_KEY_HEADER: &str = "x-sorafs-verifying-key";
const OPERATOR_PUBLIC_KEY_HEADER: &str = "x-iroha-operator-public-key";
const OPERATOR_TIMESTAMP_MS_HEADER: &str = "x-iroha-operator-timestamp-ms";
const OPERATOR_NONCE_HEADER: &str = "x-iroha-operator-nonce";
const OPERATOR_SIGNATURE_HEADER: &str = "x-iroha-operator-signature";
const APPLICATION_JSON: &str = "application/json";
const PLAN_PAGE_LIMIT: usize = 500;
const MAX_CONFIGURED_PROVIDERS: usize = 64;
const MAX_DNS_ADDRESSES_PER_HOST: usize = 16;
const MAX_REQUEST_TIMEOUT_MS: u64 = 120_000;
const DEFAULT_REQUEST_TIMEOUT_MS: u64 = 30_000;
const MAX_OPERATOR_PRIVATE_KEY_BYTES: u64 = 4 * 1024;
const MAX_CLIENT_CONFIG_BYTES: u64 = 1024 * 1024;
const MAX_MANIFEST_RESPONSE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_PLAN_PAGE_BYTES: u64 = 8 * 1024 * 1024;
const MAX_TOKEN_RESPONSE_BYTES: u64 = 64 * 1024;
const MAX_RETAINED_PLAN_HEAP_BYTES: usize = 16 * 1024 * 1024;
const MAX_PREPARED_PLANS: usize = 128;
const MAX_STREAM_CHUNK_BYTES: usize = 16 * 1024 * 1024;
const TOKEN_BYTE_WINDOW_GUARD: Duration = Duration::from_millis(1_100);
const BUNDLE_METADATA_FILE_COUNT: usize = 3;
const MANIFEST_JSON_ENVELOPE: JsonDomEnvelopeV1 = JsonDomEnvelopeV1 {
    tokens: 4_096,
    depth: 16,
    single_string_bytes: 700 * 1024,
    total_string_bytes: 1024 * 1024,
    atom_bytes: 64,
};
const PLAN_JSON_ENVELOPE: JsonDomEnvelopeV1 = JsonDomEnvelopeV1 {
    // A full page may contain 500 files with 64 path components each, plus
    // chunk records, digest strings, object keys, and scalar fields.
    tokens: 65_536,
    depth: 16,
    single_string_bytes: 4 * 1024,
    total_string_bytes: 4 * 1024 * 1024,
    atom_bytes: 64,
};
const TOKEN_JSON_ENVELOPE: JsonDomEnvelopeV1 = JsonDomEnvelopeV1 {
    tokens: 128,
    depth: 8,
    single_string_bytes: STREAM_TOKEN_MAX_BASE64_BYTES_V1,
    total_string_bytes: 16 * 1024,
    atom_bytes: 64,
};
// TODO: Qualify the complete HTTP/TLS + JSON DOM + CAR/cache fetch process against the 64 MiB
// peak-RSS gate in an isolated deployment-equivalent child. These allocation-free envelopes stop
// hostile JSON structure and oversized scalar literals before DOM allocation, but they are
// intentionally not presented as an allocator or process-RSS measurement.
// TODO: Replace the platform-configured provider-origin map with a finalized provider-advert
// projection that binds each DNS answer and stream-token verifying key to its enacted advert.
// The current client pins one exclusively-public DNS answer set for its lifetime and the existing
// gateway client independently repeats that protection, but the deployment-signed advert/IP
// binding and server-side adversarial DNS-rebinding qualification are not exposed by Torii yet.
/// Stable failure class exposed to the Musubi adapter without secret-bearing detail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MusubiArchiveRuntimeFailureClassV1 {
    /// Repeating the exact bounded request may succeed.
    Retryable,
    /// Provider evidence or returned bytes were inconsistent.
    Integrity,
    /// The exact provider/archive is not currently available.
    Unavailable,
    /// Configuration or authoritative state must change.
    Permanent,
}
/// Closed integrity surface carried to the authoritative consumer-failover boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MusubiArchiveRuntimeIntegritySurfaceV1 {
    /// Provider manifest, plan, CAR, or chunk evidence violated the exact archive commitment.
    ArchiveCommitment,
    /// Authenticated control evidence failed outside the immutable archive commitment.
    Other,
}
/// Secret-redacted production archive transport failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MusubiArchiveRuntimeErrorV1 {
    class: MusubiArchiveRuntimeFailureClassV1,
    code: &'static str,
    integrity_surface: Option<MusubiArchiveRuntimeIntegritySurfaceV1>,
}
impl MusubiArchiveRuntimeErrorV1 {
    const fn new(
        class: MusubiArchiveRuntimeFailureClassV1,
        code: &'static str,
        integrity_surface: Option<MusubiArchiveRuntimeIntegritySurfaceV1>,
    ) -> Self {
        Self {
            class,
            code,
            integrity_surface,
        }
    }
    /// Return the stable retry/integrity classification.
    #[must_use]
    pub const fn class(self) -> MusubiArchiveRuntimeFailureClassV1 {
        self.class
    }
    /// Return the stable public code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        self.code
    }
    /// Return the typed integrity surface without interpreting the public error code.
    #[must_use]
    pub const fn integrity_surface(self) -> Option<MusubiArchiveRuntimeIntegritySurfaceV1> {
        self.integrity_surface
    }
}
impl fmt::Display for MusubiArchiveRuntimeErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code)
    }
}
impl std::error::Error for MusubiArchiveRuntimeErrorV1 {}
#[derive(Clone)]
struct ProviderRuntimeV1 {
    provider: ProviderId,
    base_url: Url,
    operator_key_pair: KeyPair,
    http: HttpClient,
}
#[derive(Clone)]
struct PreparedProviderRuntimeV1 {
    provider: ProviderId,
    base_url: Url,
    operator_public_key: PublicKey,
    operator_private_key_path: PathBuf,
}
/// Parsed, secret-free production archive-fetch configuration.
///
/// Provider identities, canonical origins, and operator-key paths are validated and retained, but
/// private keys are not opened, DNS is not resolved, and HTTP clients are not built until
/// [`Self::build_client`] is called after a cache miss. Debug output deliberately omits network
/// identities, public keys, origins, client labels, and paths.
#[derive(Clone)]
pub struct PreparedMusubiArchiveFetchConfigV1 {
    providers: Vec<PreparedProviderRuntimeV1>,
    network_id: NetworkId,
    client_id: String,
    request_timeout: Duration,
}
impl fmt::Debug for PreparedMusubiArchiveFetchConfigV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedMusubiArchiveFetchConfigV1")
            .field("provider_count", &self.providers.len())
            .field("network_id_configured", &true)
            .field("request_timeout", &self.request_timeout)
            .finish_non_exhaustive()
    }
}
impl fmt::Debug for ProviderRuntimeV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderRuntimeV1")
            .field("provider", &self.provider)
            .field("origin_configured", &true)
            .field("operator_private_key_configured", &true)
            .finish_non_exhaustive()
    }
}
#[derive(Clone, Debug)]
struct PreparedPlanV1 {
    plan_binding: [u8; 32],
    root_cid_hex: String,
    chunker_handle: String,
}
#[derive(Clone)]
struct GatewaySessionFactoryV1 {
    runtime: ProviderRuntimeV1,
    network_id: NetworkId,
    pin_manifest: ManifestDigest,
    client_id: String,
    root_cid_hex: String,
    chunker_handle: String,
    max_chunk_bytes: u64,
    request_timeout: Duration,
}
struct GatewaySessionV1 {
    fetcher: sorafs_car::gateway::GatewayFetcher,
    provider: Arc<FetchProvider>,
    requests_remaining: u32,
    ttl_epoch: u64,
    byte_rate_limit: u64,
    byte_window_started: Option<Instant>,
    byte_window_used: u64,
}
/// Authenticated production `SoraFS` transport with bounded, pinned provider clients.
pub struct AuthenticatedMusubiArchiveFetchClientV1 {
    providers: BTreeMap<ProviderId, ProviderRuntimeV1>,
    network_id: NetworkId,
    client_id: String,
    request_timeout: Duration,
    prepared: BTreeMap<(ManifestDigest, ProviderId, ArchiveId), PreparedPlanV1>,
    stream_failure: Option<Arc<Mutex<Option<MusubiArchiveRuntimeErrorV1>>>>,
}
impl fmt::Debug for AuthenticatedMusubiArchiveFetchClientV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthenticatedMusubiArchiveFetchClientV1")
            .field("provider_count", &self.providers.len())
            .field("network_id_configured", &true)
            .field("prepared_plan_count", &self.prepared.len())
            .field("request_timeout", &self.request_timeout)
            .finish_non_exhaustive()
    }
}
impl PreparedMusubiArchiveFetchConfigV1 {
    /// Parse and validate the fetch subtree from one caller-owned bounded `client.toml` image.
    ///
    /// Relative operator-key paths are resolved against `config_path`, but neither those files nor
    /// any network service is accessed by this function.
    ///
    /// # Errors
    /// Returns a stable redacted error when the byte image or fetch subtree is invalid.
    pub fn from_platform_config_bytes(
        config_path: &Path,
        bytes: &[u8],
    ) -> Result<Self, MusubiArchiveRuntimeErrorV1> {
        let config_path = anchor_config_path(config_path)?;
        let length = u64::try_from(bytes.len())
            .map_err(|_| permanent("MUSUBI_ARCHIVE_FETCH_CONFIG_INVALID"))?;
        if bytes.is_empty() || length > MAX_CLIENT_CONFIG_BYTES {
            return Err(permanent("MUSUBI_ARCHIVE_FETCH_CONFIG_INVALID"));
        }
        let text = std::str::from_utf8(bytes)
            .map_err(|_| permanent("MUSUBI_ARCHIVE_FETCH_CONFIG_INVALID"))?;
        let document = parse_config_document(text)?;
        let config = parse_fetch_subtree(&document)?;
        Self::from_platform_config(&config_path, &config)
    }
    /// Validate one already-parsed fetch subtree without loading credentials or network state.
    ///
    /// # Errors
    /// Returns a stable redacted error when provider identities, origins, paths, or bounds are
    /// invalid.
    pub fn from_platform_config(
        config_path: &Path,
        config: &MusubiFetchConfig,
    ) -> Result<Self, MusubiArchiveRuntimeErrorV1> {
        let config_path = anchor_config_path(config_path)?;
        let network_id = config
            .network_id
            .ok_or_else(|| permanent("MUSUBI_ARCHIVE_FETCH_NETWORK_ID_MISSING"))?;
        if config.provider_gateways.is_empty()
            || config.provider_gateways.len() > MAX_CONFIGURED_PROVIDERS
        {
            return Err(permanent("MUSUBI_ARCHIVE_FETCH_CONFIG_INVALID"));
        }
        let client_id = config.client_id.as_deref().unwrap_or("musubi-v1");
        if !valid_visible_ascii(client_id, 1, 128) {
            return Err(permanent("MUSUBI_ARCHIVE_FETCH_CLIENT_ID_INVALID"));
        }
        let timeout_ms = config
            .request_timeout_ms
            .unwrap_or(DEFAULT_REQUEST_TIMEOUT_MS);
        if timeout_ms == 0 || timeout_ms > MAX_REQUEST_TIMEOUT_MS {
            return Err(permanent("MUSUBI_ARCHIVE_FETCH_TIMEOUT_INVALID"));
        }
        let request_timeout = Duration::from_millis(timeout_ms);
        let mut providers = Vec::<PreparedProviderRuntimeV1>::new();
        providers
            .try_reserve_exact(config.provider_gateways.len())
            .map_err(|_| permanent("MUSUBI_ARCHIVE_FETCH_CONFIG_INVALID"))?;
        let mut origins = HashSet::new();
        for configured in &config.provider_gateways {
            let provider = parse_provider_id(&configured.provider_id)?;
            let base_url = parse_gateway_base_url(&configured.url)?;
            let operator_public_key = configured
                .operator_public_key
                .parse::<PublicKey>()
                .map_err(|_| permanent("MUSUBI_ARCHIVE_FETCH_OPERATOR_KEY_INVALID"))?;
            if operator_public_key.to_string() != configured.operator_public_key {
                return Err(permanent("MUSUBI_ARCHIVE_FETCH_OPERATOR_KEY_INVALID"));
            }
            let origin = gateway_origin(&base_url)
                .ok_or_else(|| permanent("MUSUBI_ARCHIVE_FETCH_GATEWAY_URL_INVALID"))?;
            if providers
                .iter()
                .any(|prepared| prepared.provider == provider)
                || !origins.insert(origin)
            {
                return Err(permanent("MUSUBI_ARCHIVE_FETCH_PROVIDER_DUPLICATE"));
            }
            providers.push(PreparedProviderRuntimeV1 {
                provider,
                base_url,
                operator_public_key,
                operator_private_key_path: resolve_config_path(
                    &config_path,
                    &configured.operator_private_key_file,
                )?,
            });
        }
        Ok(Self {
            providers,
            network_id,
            client_id: client_id.to_owned(),
            request_timeout,
        })
    }
    /// Load and cross-check every runtime-only operator key before pinning DNS answers or
    /// constructing the authenticated client.
    ///
    /// # Errors
    /// Returns a stable redacted error when an operator key, DNS answer, or HTTP client is invalid.
    pub fn build_client(
        &self,
    ) -> Result<AuthenticatedMusubiArchiveFetchClientV1, MusubiArchiveRuntimeErrorV1> {
        let operator_key_pairs = self
            .providers
            .iter()
            .map(|prepared| {
                read_operator_key_pair(
                    &prepared.operator_private_key_path,
                    &prepared.operator_public_key,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut providers = BTreeMap::new();
        for (prepared, operator_key_pair) in
            self.providers.iter().zip(operator_key_pairs.into_iter())
        {
            let http = pinned_http_client(&prepared.base_url, self.request_timeout)?;
            providers.insert(
                prepared.provider,
                ProviderRuntimeV1 {
                    provider: prepared.provider,
                    base_url: prepared.base_url.clone(),
                    operator_key_pair,
                    http,
                },
            );
        }
        Ok(AuthenticatedMusubiArchiveFetchClientV1 {
            providers,
            network_id: self.network_id,
            client_id: self.client_id.clone(),
            request_timeout: self.request_timeout,
            prepared: BTreeMap::new(),
            stream_failure: None,
        })
    }
}
impl AuthenticatedMusubiArchiveFetchClientV1 {
    /// Load only `[musubi.fetch]` from one required platform `client.toml`.
    ///
    /// Account identity, account keys, mutation credentials, basic auth, and environment
    /// variables are deliberately not interpreted. Only the exact fetch-network identity and
    /// provider-specific operator key files are admitted.
    ///
    /// # Errors
    /// Returns a stable error for an unsafe file or malformed fetch subtree.
    pub fn load_platform_file(path: &Path) -> Result<Self, MusubiArchiveRuntimeErrorV1> {
        let path = anchor_config_path(path)?;
        let (bytes, _) = read_bounded_regular(&path, MAX_CLIENT_CONFIG_BYTES)
            .map_err(|_| permanent("MUSUBI_ARCHIVE_FETCH_CONFIG_INVALID"))?;
        PreparedMusubiArchiveFetchConfigV1::from_platform_config_bytes(&path, &bytes)?
            .build_client()
    }
    /// Build the production boundary from the typed platform-client fetch subtree.
    ///
    /// Relative operator-key paths are resolved beside `client.toml`. Every gateway is
    /// HTTPS-only, canonical, credential-free, standard-port, and pinned to an
    /// exclusively public bounded DNS answer set before this function returns.
    ///
    /// # Errors
    /// Returns a stable configuration error without exposing URLs, paths, or secrets.
    pub fn from_platform_config(
        config_path: &Path,
        config: &MusubiFetchConfig,
    ) -> Result<Self, MusubiArchiveRuntimeErrorV1> {
        PreparedMusubiArchiveFetchConfigV1::from_platform_config(config_path, config)?
            .build_client()
    }
    /// Fetch and validate the canonical manifest plus every storage-plan page.
    ///
    /// # Errors
    /// Returns a stable provider, network, or integrity failure.
    pub fn storage_plan(
        &mut self,
        pin_manifest: &ManifestDigest,
        provider: ProviderId,
        commitment: &MusubiArchiveCommitmentV1,
    ) -> Result<CarBuildPlan, MusubiArchiveRuntimeErrorV1> {
        commitment
            .validate()
            .map_err(|_| integrity("MUSUBI_ARCHIVE_COMMITMENT_INVALID"))?;
        let runtime = self
            .providers
            .get(&provider)
            .ok_or_else(|| unavailable("MUSUBI_ARCHIVE_PROVIDER_NOT_CONFIGURED"))?;
        let manifest = fetch_and_validate_manifest(runtime, pin_manifest, commitment)?;
        let plan = fetch_and_validate_plan(runtime, pin_manifest, commitment, &manifest)?;
        let prepared_key = (*pin_manifest, provider, commitment.archive_id());
        if self.prepared.len() >= MAX_PREPARED_PLANS && !self.prepared.contains_key(&prepared_key) {
            self.prepared.pop_first();
        }
        self.prepared.insert(
            prepared_key,
            PreparedPlanV1 {
                plan_binding: exact_plan_binding(&plan)?,
                root_cid_hex: hex::encode(&manifest.manifest.root_cid),
                chunker_handle: manifest.chunker_handle,
            },
        );
        Ok(plan)
    }
    /// Mint an exact stream token and open a bounded canonical CAR reader.
    ///
    /// # Errors
    /// Returns a stable error if no exact prepared plan exists, token issuance
    /// fails, or the authenticated gateway context cannot be constructed.
    pub fn open_authenticated_car(
        &mut self,
        pin_manifest: &ManifestDigest,
        provider: ProviderId,
        commitment: &MusubiArchiveCommitmentV1,
        plan: &CarBuildPlan,
    ) -> Result<Box<dyn Read + Send + 'static>, MusubiArchiveRuntimeErrorV1> {
        self.stream_failure = None;
        // Account the caller-retained allocation before cloning it into the worker. Exact plan
        // binding intentionally ignores Vec capacities, so this check must precede the clone.
        enforce_plan_memory_bound(plan)?;
        let key = (*pin_manifest, provider, commitment.archive_id());
        let prepared = self
            .prepared
            .remove(&key)
            .ok_or_else(|| permanent("MUSUBI_ARCHIVE_PLAN_NOT_PREPARED"))?;
        if prepared.plan_binding != exact_plan_binding(plan)? {
            return Err(control_integrity("MUSUBI_ARCHIVE_PLAN_SUBSTITUTED"));
        }
        let runtime = self
            .providers
            .get(&provider)
            .ok_or_else(|| unavailable("MUSUBI_ARCHIVE_PROVIDER_NOT_CONFIGURED"))?;
        let max_chunk_bytes = plan
            .chunks
            .iter()
            .map(|chunk| u64::from(chunk.length))
            .max()
            .filter(|length| *length != 0)
            .ok_or_else(|| integrity("MUSUBI_ARCHIVE_PLAN_RESPONSE_INVALID"))?;
        let sessions = GatewaySessionFactoryV1 {
            runtime: runtime.clone(),
            network_id: self.network_id,
            pin_manifest: *pin_manifest,
            client_id: self.client_id.clone(),
            root_cid_hex: prepared.root_cid_hex,
            chunker_handle: prepared.chunker_handle,
            max_chunk_bytes,
            request_timeout: self.request_timeout,
        };
        let initial_session = sessions.open()?;
        let stream_failure = Arc::new(Mutex::new(None));
        self.stream_failure = Some(Arc::clone(&stream_failure));
        canonical_car_reader(
            sessions,
            initial_session,
            plan.clone(),
            commitment.clone(),
            stream_failure,
        )
    }
    /// Consume the last failure reported while an authenticated CAR reader was running.
    ///
    /// The returned value is a stable, redacted classification. It lets the cache adapter
    /// distinguish a provider transport failure from a byte-integrity rejection after the
    /// reader has been consumed.
    #[must_use]
    pub fn take_stream_failure(&mut self) -> Option<MusubiArchiveRuntimeErrorV1> {
        let stream_failure = self.stream_failure.take()?;
        stream_failure.lock().map_or_else(
            |_| Some(permanent("MUSUBI_ARCHIVE_STREAM_STATE_UNAVAILABLE")),
            |mut failure| failure.take(),
        )
    }
}
fn parse_config_document(text: &str) -> Result<toml::Value, MusubiArchiveRuntimeErrorV1> {
    text.parse::<toml::Table>()
        .map(toml::Value::Table)
        .map_err(|_| permanent("MUSUBI_ARCHIVE_FETCH_CONFIG_INVALID"))
}
impl GatewaySessionFactoryV1 {
    fn open(&self) -> Result<GatewaySessionV1, MusubiArchiveRuntimeErrorV1> {
        let token = mint_stream_token(
            &self.runtime,
            &self.network_id,
            &self.pin_manifest,
            &self.client_id,
            self.max_chunk_bytes,
        )?;
        let StreamTokenEvidenceV1 {
            encoded,
            verifying_key_hex,
            requests_per_minute,
            ttl_epoch,
            rate_limit_bytes,
        } = token;
        let context = GatewayFetchContext::new_with_timeouts(
            GatewayFetchConfig {
                manifest_id_hex: hex::encode(self.pin_manifest.as_bytes()),
                chunker_handle: self.chunker_handle.clone(),
                manifest_envelope_b64: None,
                client_id: Some(self.client_id.clone()),
                expected_manifest_cid_hex: Some(self.root_cid_hex.clone()),
                blinded_cid_b64: None,
                salt_epoch: None,
                expected_cache_version: None,
            },
            [GatewayProviderInput {
                name: hex::encode(self.runtime.provider.as_bytes()),
                provider_id_hex: hex::encode(self.runtime.provider.as_bytes()),
                gateway_public_key_hex: verifying_key_hex,
                base_url: self.runtime.base_url.as_str().to_owned(),
                stream_token_b64: encoded,
                privacy_events_url: None,
            }],
            self.request_timeout.min(Duration::from_secs(10)),
            self.request_timeout,
        )
        .map_err(|_| permanent("MUSUBI_ARCHIVE_GATEWAY_CONTEXT_INVALID"))?;
        let provider = context
            .providers()
            .into_iter()
            .next()
            .ok_or_else(|| permanent("MUSUBI_ARCHIVE_GATEWAY_CONTEXT_INVALID"))?;
        Ok(GatewaySessionV1 {
            fetcher: context.fetcher(),
            provider: Arc::new(provider),
            requests_remaining: requests_per_minute,
            ttl_epoch,
            byte_rate_limit: rate_limit_bytes,
            byte_window_started: None,
            byte_window_used: 0,
        })
    }
}
#[derive(Debug)]
struct VerifiedManifestV1 {
    manifest: ManifestV1,
    payload_digest: [u8; 32],
    chunker_handle: String,
    chunk_count: usize,
    file_count: usize,
}
fn fetch_and_validate_manifest(
    runtime: &ProviderRuntimeV1,
    pin_manifest: &ManifestDigest,
    commitment: &MusubiArchiveCommitmentV1,
) -> Result<VerifiedManifestV1, MusubiArchiveRuntimeErrorV1> {
    let manifest_id_hex = hex::encode(pin_manifest.as_bytes());
    let mut url = runtime
        .base_url
        .join(&format!("v1/sorafs/storage/manifest/{manifest_id_hex}"))
        .map_err(|_| permanent("MUSUBI_ARCHIVE_FETCH_GATEWAY_URL_INVALID"))?;
    url.query_pairs_mut()
        .append_pair("limit", "1")
        .append_pair("offset", "0");
    let response = runtime
        .http
        .get(url)
        .header("accept", APPLICATION_JSON)
        .header("accept-encoding", "identity")
        .send()
        .map_err(|_| retryable("MUSUBI_ARCHIVE_MANIFEST_REQUEST_FAILED"))?;
    let body = read_json_response(
        response,
        MAX_MANIFEST_RESPONSE_BYTES,
        "MUSUBI_ARCHIVE_MANIFEST",
        MusubiArchiveRuntimeIntegritySurfaceV1::ArchiveCommitment,
    )?;
    parse_and_validate_manifest(&manifest_id_hex, &body, commitment)
}
fn parse_and_validate_manifest(
    expected_manifest_id_hex: &str,
    body: &[u8],
    commitment: &MusubiArchiveCommitmentV1,
) -> Result<VerifiedManifestV1, MusubiArchiveRuntimeErrorV1> {
    preflight_json_dom(body, MANIFEST_JSON_ENVELOPE)
        .map_err(|_| integrity("MUSUBI_ARCHIVE_MANIFEST_RESPONSE_INVALID"))?;
    let value: norito::json::Value = norito::json::from_slice(body)
        .map_err(|_| integrity("MUSUBI_ARCHIVE_MANIFEST_RESPONSE_INVALID"))?;
    let root = value
        .as_object()
        .ok_or_else(|| integrity("MUSUBI_ARCHIVE_MANIFEST_RESPONSE_INVALID"))?;
    let manifest_id_hex = required_string(root, "manifest_id_hex")?;
    let manifest_digest_hex = required_string(root, "manifest_digest_hex")?;
    if manifest_id_hex != expected_manifest_id_hex
        || manifest_digest_hex != expected_manifest_id_hex
    {
        return Err(integrity("MUSUBI_ARCHIVE_MANIFEST_ID_MISMATCH"));
    }
    let manifest_b64 = required_string(root, "manifest_b64")?;
    let manifest = decode_manifest_v1_base64_canonical(manifest_b64)
        .map_err(|_| integrity("MUSUBI_ARCHIVE_MANIFEST_RESPONSE_INVALID"))?;
    validate_manifest(&manifest, &PinPolicyConstraints::default())
        .map_err(|_| integrity("MUSUBI_ARCHIVE_MANIFEST_RESPONSE_INVALID"))?;
    let digest = manifest
        .digest()
        .map_err(|_| integrity("MUSUBI_ARCHIVE_MANIFEST_RESPONSE_INVALID"))?;
    if hex::encode(digest.as_bytes()) != expected_manifest_id_hex {
        return Err(integrity("MUSUBI_ARCHIVE_MANIFEST_ID_MISMATCH"));
    }
    let payload_digest = parse_lower_hex_32(required_string(root, "payload_digest_hex")?)?;
    let content_length = required_u64(root, "content_length")?;
    let chunk_count = required_usize(root, "chunk_count")?;
    let file_count = required_usize(root, "file_count")?;
    let returned_file_count = required_usize(root, "returned_file_count")?;
    let truncated_files = required_bool(root, "truncated_files")?;
    let chunker_handle = required_string(root, "chunk_profile_handle")?.to_owned();
    if returned_file_count != 1 || !truncated_files || chunk_count == 0 || file_count == 0 {
        return Err(integrity("MUSUBI_ARCHIVE_MANIFEST_RESPONSE_INVALID"));
    }
    let expected_files = usize::try_from(commitment.file_count)
        .ok()
        .and_then(|count| count.checked_add(BUNDLE_METADATA_FILE_COUNT))
        .ok_or_else(|| integrity("MUSUBI_ARCHIVE_MANIFEST_RESPONSE_INVALID"))?;
    let expected_chunks = usize::try_from(commitment.chunk_count)
        .map_err(|_| integrity("MUSUBI_ARCHIVE_MANIFEST_RESPONSE_INVALID"))?;
    let descriptor = validate_registered_chunker_profile(&manifest.chunking)
        .map_err(|_| integrity("MUSUBI_ARCHIVE_MANIFEST_RESPONSE_INVALID"))?;
    let expected_handle = format!(
        "{}.{}@{}",
        manifest.chunking.namespace, manifest.chunking.name, manifest.chunking.semver
    );
    if manifest.root_cid.as_slice() != commitment.root_cid.as_bytes()
        || manifest.chunk_digest_sha3_256 != *commitment.chunk_plan_digest.as_bytes()
        || manifest.por_root != *commitment.por_root.as_bytes()
        || manifest.content_length != commitment.content_length
        || manifest.car_digest != *commitment.car_digest.as_bytes()
        || manifest.car_size != commitment.car_size
        || content_length != commitment.content_length
        || chunk_count != expected_chunks
        || file_count != expected_files
        || chunker_handle != expected_handle
        || manifest.chunking.profile_id.0 != commitment.chunker.profile_id
        || manifest.chunking.namespace != commitment.chunker.namespace
        || manifest.chunking.name != commitment.chunker.name
        || manifest.chunking.semver != commitment.chunker.semver
        || manifest.chunking.multihash_code != commitment.chunker.multihash_code
        || descriptor.id.0 != commitment.chunker.profile_id
    {
        return Err(integrity("MUSUBI_ARCHIVE_MANIFEST_COMMITMENT_MISMATCH"));
    }
    Ok(VerifiedManifestV1 {
        manifest,
        payload_digest,
        chunker_handle,
        chunk_count,
        file_count,
    })
}
fn fetch_and_validate_plan(
    runtime: &ProviderRuntimeV1,
    pin_manifest: &ManifestDigest,
    commitment: &MusubiArchiveCommitmentV1,
    manifest: &VerifiedManifestV1,
) -> Result<CarBuildPlan, MusubiArchiveRuntimeErrorV1> {
    let descriptor = validate_registered_chunker_profile(&manifest.manifest.chunking)
        .map_err(|_| integrity("MUSUBI_ARCHIVE_PLAN_RESPONSE_INVALID"))?;
    let manifest_id_hex = hex::encode(pin_manifest.as_bytes());
    let mut offset = 0_usize;
    let mut file_payload_offset = 0_u64;
    let mut chunks = Vec::new();
    let mut files = Vec::new();
    chunks
        .try_reserve_exact(manifest.chunk_count)
        .map_err(|_| permanent("MUSUBI_ARCHIVE_PLAN_ALLOCATION_FAILED"))?;
    files
        .try_reserve_exact(manifest.file_count)
        .map_err(|_| permanent("MUSUBI_ARCHIVE_PLAN_ALLOCATION_FAILED"))?;
    loop {
        let mut url = runtime
            .base_url
            .join(&format!("v1/sorafs/storage/plan/{manifest_id_hex}"))
            .map_err(|_| permanent("MUSUBI_ARCHIVE_FETCH_GATEWAY_URL_INVALID"))?;
        url.query_pairs_mut()
            .append_pair("limit", &PLAN_PAGE_LIMIT.to_string())
            .append_pair("offset", &offset.to_string());
        let response = runtime
            .http
            .get(url)
            .header("accept", APPLICATION_JSON)
            .header("accept-encoding", "identity")
            .send()
            .map_err(|_| retryable("MUSUBI_ARCHIVE_PLAN_REQUEST_FAILED"))?;
        let body = read_json_response(
            response,
            MAX_PLAN_PAGE_BYTES,
            "MUSUBI_ARCHIVE_PLAN",
            MusubiArchiveRuntimeIntegritySurfaceV1::ArchiveCommitment,
        )?;
        let page = parse_plan_page(
            &manifest_id_hex,
            &body,
            offset,
            manifest,
            &mut file_payload_offset,
        )?;
        chunks.extend(page.chunks);
        files.extend(page.files);
        if !page.truncated_chunks && !page.truncated_files {
            break;
        }
        offset = offset
            .checked_add(PLAN_PAGE_LIMIT)
            .ok_or_else(|| integrity("MUSUBI_ARCHIVE_PLAN_PAGINATION_INVALID"))?;
        if offset >= manifest.chunk_count.max(manifest.file_count)
            && (page.truncated_chunks || page.truncated_files)
        {
            return Err(integrity("MUSUBI_ARCHIVE_PLAN_PAGINATION_INVALID"));
        }
    }
    if chunks.len() != manifest.chunk_count
        || files.len() != manifest.file_count
        || file_payload_offset != manifest.manifest.content_length
    {
        return Err(integrity("MUSUBI_ARCHIVE_PLAN_INCOMPLETE"));
    }
    let plan = CarBuildPlan {
        chunk_profile: descriptor.profile,
        payload_digest: blake3::Hash::from(manifest.payload_digest),
        content_length: manifest.manifest.content_length,
        chunks,
        files,
    };
    plan.validate()
        .map_err(|_| integrity("MUSUBI_ARCHIVE_PLAN_RESPONSE_INVALID"))?;
    if compute_chunk_plan_digest_sha3(&plan.chunks) != *commitment.chunk_plan_digest.as_bytes()
        || plan.content_length != commitment.content_length
        || plan.payload_digest.as_bytes() != &manifest.payload_digest
        || plan.chunks.len() != usize::try_from(commitment.chunk_count).unwrap_or(usize::MAX)
    {
        return Err(integrity("MUSUBI_ARCHIVE_PLAN_COMMITMENT_MISMATCH"));
    }
    enforce_plan_memory_bound(&plan)?;
    Ok(plan)
}
struct PlanPageV1 {
    truncated_chunks: bool,
    truncated_files: bool,
    chunks: Vec<CarChunk>,
    files: Vec<FilePlan>,
}
#[expect(
    clippy::too_many_lines,
    reason = "the provider-owned plan is validated in one ordered fail-closed audit surface"
)]
fn parse_plan_page(
    expected_manifest_id_hex: &str,
    body: &[u8],
    expected_offset: usize,
    manifest: &VerifiedManifestV1,
    file_payload_offset: &mut u64,
) -> Result<PlanPageV1, MusubiArchiveRuntimeErrorV1> {
    preflight_json_dom(body, PLAN_JSON_ENVELOPE)
        .map_err(|_| integrity("MUSUBI_ARCHIVE_PLAN_RESPONSE_INVALID"))?;
    let value: norito::json::Value = norito::json::from_slice(body)
        .map_err(|_| integrity("MUSUBI_ARCHIVE_PLAN_RESPONSE_INVALID"))?;
    if value
        .get("manifest_id_hex")
        .and_then(norito::json::Value::as_str)
        != Some(expected_manifest_id_hex)
    {
        return Err(integrity("MUSUBI_ARCHIVE_PLAN_MANIFEST_MISMATCH"));
    }
    let plan = value
        .get("plan")
        .and_then(norito::json::Value::as_object)
        .ok_or_else(|| integrity("MUSUBI_ARCHIVE_PLAN_RESPONSE_INVALID"))?;
    let chunk_count = required_usize(plan, "chunk_count")?;
    let file_count = required_usize(plan, "file_count")?;
    let chunk_digest_count = required_usize(plan, "chunk_digest_count")?;
    let returned_chunk_count = required_usize(plan, "returned_chunk_count")?;
    let returned_chunk_digest_count = required_usize(plan, "returned_chunk_digest_count")?;
    let returned_file_count = required_usize(plan, "returned_file_count")?;
    let offset = required_usize(plan, "offset")?;
    let limit = required_usize(plan, "limit")?;
    let content_length = required_u64(plan, "content_length")?;
    let payload_digest = parse_lower_hex_32(required_string(plan, "payload_digest_blake3")?)?;
    let chunker_handle = required_string(plan, "chunk_profile_handle")?;
    let truncated_chunks = required_bool(plan, "truncated_chunks")?;
    let truncated_files = required_bool(plan, "truncated_files")?;
    let truncated_chunk_digests = required_bool(plan, "truncated_chunk_digests")?;
    if offset != expected_offset
        || limit != PLAN_PAGE_LIMIT
        || chunk_count != manifest.chunk_count
        || file_count != manifest.file_count
        || chunk_digest_count != manifest.chunk_count
        || content_length != manifest.manifest.content_length
        || payload_digest != manifest.payload_digest
        || chunker_handle != manifest.chunker_handle
        || chunk_count == 0
        || chunk_count > usize::try_from(MUSUBI_MAX_CHUNKS_V1).unwrap_or(usize::MAX)
        || file_count == 0
        || file_count
            > usize::try_from(MUSUBI_MAX_FILES_V1)
                .unwrap_or(usize::MAX)
                .saturating_add(BUNDLE_METADATA_FILE_COUNT)
        || offset > chunk_count.max(file_count)
    {
        return Err(integrity("MUSUBI_ARCHIVE_PLAN_RESPONSE_INVALID"));
    }
    let chunks_value = plan
        .get("chunks")
        .and_then(norito::json::Value::as_array)
        .ok_or_else(|| integrity("MUSUBI_ARCHIVE_PLAN_RESPONSE_INVALID"))?;
    let chunk_digests = plan
        .get("chunk_digests_blake3")
        .and_then(norito::json::Value::as_array)
        .ok_or_else(|| integrity("MUSUBI_ARCHIVE_PLAN_RESPONSE_INVALID"))?;
    let files_value = plan
        .get("files")
        .and_then(norito::json::Value::as_array)
        .ok_or_else(|| integrity("MUSUBI_ARCHIVE_PLAN_RESPONSE_INVALID"))?;
    if chunks_value.len() != returned_chunk_count
        || chunk_digests.len() != returned_chunk_digest_count
        || chunks_value.len() != chunk_digests.len()
        || files_value.len() != returned_file_count
        || chunks_value.len() > limit
        || files_value.len() > limit
    {
        return Err(integrity("MUSUBI_ARCHIVE_PLAN_RESPONSE_INVALID"));
    }
    let expected_returned_chunks = chunk_count
        .saturating_sub(offset.min(chunk_count))
        .min(limit);
    let expected_returned_files = file_count.saturating_sub(offset.min(file_count)).min(limit);
    let expected_truncated_chunks =
        offset.min(chunk_count).saturating_add(returned_chunk_count) < chunk_count;
    let expected_truncated_files =
        offset.min(file_count).saturating_add(returned_file_count) < file_count;
    if returned_chunk_count != expected_returned_chunks
        || returned_chunk_digest_count != expected_returned_chunks
        || returned_file_count != expected_returned_files
        || truncated_chunks != expected_truncated_chunks
        || truncated_chunk_digests != truncated_chunks
        || truncated_files != expected_truncated_files
    {
        return Err(integrity("MUSUBI_ARCHIVE_PLAN_PAGINATION_INVALID"));
    }
    let mut chunks = Vec::new();
    chunks
        .try_reserve_exact(chunks_value.len())
        .map_err(|_| permanent("MUSUBI_ARCHIVE_PLAN_ALLOCATION_FAILED"))?;
    for (index, chunk) in chunks_value.iter().enumerate() {
        let chunk = chunk
            .as_object()
            .ok_or_else(|| integrity("MUSUBI_ARCHIVE_PLAN_RESPONSE_INVALID"))?;
        if chunk.contains_key("taikai_segment_hint") {
            return Err(integrity("MUSUBI_ARCHIVE_PLAN_RESPONSE_INVALID"));
        }
        let chunk_index = required_usize(chunk, "chunk_index")?;
        let expected_index = offset
            .checked_add(index)
            .ok_or_else(|| integrity("MUSUBI_ARCHIVE_PLAN_RESPONSE_INVALID"))?;
        let chunk_offset = required_u64(chunk, "offset")?;
        let length_u64 = required_u64(chunk, "length")?;
        let length = u32::try_from(length_u64)
            .map_err(|_| integrity("MUSUBI_ARCHIVE_PLAN_RESPONSE_INVALID"))?;
        let digest = parse_lower_hex_32(required_string(chunk, "digest_blake3")?)?;
        let listed_digest = chunk_digests
            .get(index)
            .and_then(norito::json::Value::as_str)
            .ok_or_else(|| integrity("MUSUBI_ARCHIVE_PLAN_RESPONSE_INVALID"))?;
        if chunk_index != expected_index
            || length == 0
            || usize::try_from(length).unwrap_or(usize::MAX) > MAX_STREAM_CHUNK_BYTES
            || parse_lower_hex_32(listed_digest)? != digest
        {
            return Err(integrity("MUSUBI_ARCHIVE_PLAN_RESPONSE_INVALID"));
        }
        chunks.push(CarChunk {
            offset: chunk_offset,
            length,
            digest,
            taikai_segment_hint: None,
        });
    }
    let mut files = Vec::new();
    files
        .try_reserve_exact(files_value.len())
        .map_err(|_| permanent("MUSUBI_ARCHIVE_PLAN_ALLOCATION_FAILED"))?;
    for (index, file) in files_value.iter().enumerate() {
        let file = file
            .as_object()
            .ok_or_else(|| integrity("MUSUBI_ARCHIVE_PLAN_RESPONSE_INVALID"))?;
        let path_values = file
            .get("path")
            .and_then(norito::json::Value::as_array)
            .ok_or_else(|| integrity("MUSUBI_ARCHIVE_PLAN_RESPONSE_INVALID"))?;
        if path_values.is_empty() || path_values.len() > 64 {
            return Err(integrity("MUSUBI_ARCHIVE_PLAN_RESPONSE_INVALID"));
        }
        let mut path = Vec::new();
        path.try_reserve_exact(path_values.len())
            .map_err(|_| permanent("MUSUBI_ARCHIVE_PLAN_ALLOCATION_FAILED"))?;
        let mut path_bytes = 0_usize;
        for component in path_values {
            let component = component
                .as_str()
                .ok_or_else(|| integrity("MUSUBI_ARCHIVE_PLAN_RESPONSE_INVALID"))?;
            path_bytes = path_bytes
                .checked_add(component.len())
                .and_then(|length| length.checked_add(1))
                .ok_or_else(|| integrity("MUSUBI_ARCHIVE_PLAN_RESPONSE_INVALID"))?;
            if component.is_empty()
                || component.len() > 255
                || component == "."
                || component == ".."
                || component.contains(['/', '\\', '\0'])
                || path_bytes > 4_096
            {
                return Err(integrity("MUSUBI_ARCHIVE_PLAN_RESPONSE_INVALID"));
            }
            path.push(component.to_owned());
        }
        let declared_offset = required_u64(file, "offset")?;
        let size = required_u64(file, "size")?;
        let first_chunk = required_usize(file, "first_chunk")?;
        let chunk_count = required_usize(file, "chunk_count")?;
        let expected_index = expected_offset
            .checked_add(index)
            .ok_or_else(|| integrity("MUSUBI_ARCHIVE_PLAN_RESPONSE_INVALID"))?;
        if expected_index >= manifest.file_count || declared_offset != *file_payload_offset {
            return Err(integrity("MUSUBI_ARCHIVE_PLAN_RESPONSE_INVALID"));
        }
        *file_payload_offset = file_payload_offset
            .checked_add(size)
            .ok_or_else(|| integrity("MUSUBI_ARCHIVE_PLAN_RESPONSE_INVALID"))?;
        files.push(FilePlan {
            path,
            first_chunk,
            chunk_count,
            size,
        });
    }
    Ok(PlanPageV1 {
        truncated_chunks,
        truncated_files,
        chunks,
        files,
    })
}
struct StreamTokenEvidenceV1 {
    encoded: String,
    verifying_key_hex: String,
    requests_per_minute: u32,
    ttl_epoch: u64,
    rate_limit_bytes: u64,
}
fn mint_stream_token(
    runtime: &ProviderRuntimeV1,
    network_id: &NetworkId,
    pin_manifest: &ManifestDigest,
    client_id: &str,
    max_chunk_bytes: u64,
) -> Result<StreamTokenEvidenceV1, MusubiArchiveRuntimeErrorV1> {
    if max_chunk_bytes == 0 || max_chunk_bytes > u64::try_from(MAX_STREAM_CHUNK_BYTES).unwrap_or(0)
    {
        return Err(integrity("MUSUBI_ARCHIVE_PLAN_RESPONSE_INVALID"));
    }
    let nonce = random_nonce()?;
    let url = runtime
        .base_url
        .join("v1/sorafs/storage/token")
        .map_err(|_| permanent("MUSUBI_ARCHIVE_FETCH_GATEWAY_URL_INVALID"))?;
    let mut body = norito::json::Map::new();
    body.insert(
        "manifest_id_hex".into(),
        norito::json::Value::from(hex::encode(pin_manifest.as_bytes())),
    );
    body.insert(
        "provider_id_hex".into(),
        norito::json::Value::from(hex::encode(runtime.provider.as_bytes())),
    );
    body.insert("ttl_secs".into(), norito::json::Value::Null);
    body.insert("max_streams".into(), norito::json::Value::from(1_u64));
    body.insert("rate_limit_bytes".into(), norito::json::Value::Null);
    body.insert("requests_per_minute".into(), norito::json::Value::Null);
    let body = norito::json::to_vec(&norito::json::Value::Object(body))
        .map_err(|_| permanent("MUSUBI_ARCHIVE_TOKEN_REQUEST_INVALID"))?;
    let operator_headers = operator_request_headers(runtime, network_id, &url, &body)?;
    let response = runtime
        .http
        .post(url)
        .header(OPERATOR_PUBLIC_KEY_HEADER, &operator_headers.public_key)
        .header(OPERATOR_TIMESTAMP_MS_HEADER, &operator_headers.timestamp_ms)
        .header(OPERATOR_NONCE_HEADER, &operator_headers.nonce)
        .header(OPERATOR_SIGNATURE_HEADER, &operator_headers.signature_b64)
        .header(CLIENT_HEADER, client_id)
        .header(NONCE_HEADER, &nonce)
        .header(CONTENT_TYPE, APPLICATION_JSON)
        .header("accept", APPLICATION_JSON)
        .header("accept-encoding", "identity")
        .body(body)
        .send()
        .map_err(|_| retryable("MUSUBI_ARCHIVE_TOKEN_REQUEST_FAILED"))?;
    let response_headers = response.headers().clone();
    let body = read_json_response(
        response,
        MAX_TOKEN_RESPONSE_BYTES,
        "MUSUBI_ARCHIVE_TOKEN",
        MusubiArchiveRuntimeIntegritySurfaceV1::Other,
    )?;
    if header_text(&response_headers, CLIENT_HEADER) != Some(client_id)
        || header_text(&response_headers, NONCE_HEADER) != Some(nonce.as_str())
    {
        return Err(control_integrity("MUSUBI_ARCHIVE_TOKEN_RESPONSE_INVALID"));
    }
    let verifying_key_hex = header_text(&response_headers, VERIFYING_KEY_HEADER)
        .filter(|value| is_lower_hex(value, 64))
        .ok_or_else(|| control_integrity("MUSUBI_ARCHIVE_TOKEN_RESPONSE_INVALID"))?
        .to_owned();
    preflight_json_dom(&body, TOKEN_JSON_ENVELOPE)
        .map_err(|_| control_integrity("MUSUBI_ARCHIVE_TOKEN_RESPONSE_INVALID"))?;
    let value: norito::json::Value = norito::json::from_slice(&body)
        .map_err(|_| control_integrity("MUSUBI_ARCHIVE_TOKEN_RESPONSE_INVALID"))?;
    let encoded = value
        .get("token_base64")
        .and_then(norito::json::Value::as_str)
        .filter(|value| {
            !value.is_empty()
                && value.len() <= STREAM_TOKEN_MAX_BASE64_BYTES_V1
                && value.trim() == *value
        })
        .ok_or_else(|| control_integrity("MUSUBI_ARCHIVE_TOKEN_RESPONSE_INVALID"))?
        .to_owned();
    let token = decode_stream_token_exact(&encoded)?;
    if token.body.max_streams != 1 || token.body.rate_limit_bytes < max_chunk_bytes {
        return Err(control_integrity("MUSUBI_ARCHIVE_TOKEN_RESPONSE_INVALID"));
    }
    Ok(StreamTokenEvidenceV1 {
        encoded,
        verifying_key_hex,
        requests_per_minute: token.body.requests_per_minute,
        ttl_epoch: token.body.ttl_epoch,
        rate_limit_bytes: token.body.rate_limit_bytes,
    })
}
struct OperatorRequestHeadersV1 {
    public_key: String,
    timestamp_ms: String,
    nonce: String,
    signature_b64: String,
}
fn operator_request_headers(
    runtime: &ProviderRuntimeV1,
    network_id: &NetworkId,
    url: &Url,
    body: &[u8],
) -> Result<OperatorRequestHeadersV1, MusubiArchiveRuntimeErrorV1> {
    let timestamp_ms: u64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| permanent("MUSUBI_ARCHIVE_OPERATOR_CLOCK_INVALID"))?
        .as_millis()
        .try_into()
        .map_err(|_| permanent("MUSUBI_ARCHIVE_OPERATOR_CLOCK_INVALID"))?;
    let nonce = random_nonce()?;
    let message = Client::operator_network_request_message(
        network_id,
        &crate::http::Method::POST,
        url,
        body,
        timestamp_ms,
        &nonce,
    )
    .map_err(|_| permanent("MUSUBI_ARCHIVE_OPERATOR_SIGNING_FAILED"))?;
    let signature = Signature::try_new(runtime.operator_key_pair.private_key(), &message)
        .map_err(|_| permanent("MUSUBI_ARCHIVE_OPERATOR_SIGNING_FAILED"))?;
    let public_key = runtime
        .operator_key_pair
        .public_key()
        .try_to_multihash_string()
        .map_err(|_| permanent("MUSUBI_ARCHIVE_OPERATOR_SIGNING_FAILED"))?;
    let timestamp_ms = crate::client::canonical_request_timestamp_header_value(timestamp_ms)
        .map_err(|_| permanent("MUSUBI_ARCHIVE_OPERATOR_SIGNING_FAILED"))?;
    let signature_b64 = crate::client::canonical_request_signature_header_value(&signature)
        .map_err(|_| permanent("MUSUBI_ARCHIVE_OPERATOR_SIGNING_FAILED"))?;
    Ok(OperatorRequestHeadersV1 {
        public_key,
        timestamp_ms,
        nonce,
        signature_b64,
    })
}
fn decode_stream_token_exact(encoded: &str) -> Result<StreamTokenV1, MusubiArchiveRuntimeErrorV1> {
    if encoded.len() > STREAM_TOKEN_MAX_BASE64_BYTES_V1 || encoded.trim() != encoded {
        return Err(control_integrity("MUSUBI_ARCHIVE_TOKEN_RESPONSE_INVALID"));
    }
    let bytes = STANDARD
        .decode(encoded.as_bytes())
        .map_err(|_| control_integrity("MUSUBI_ARCHIVE_TOKEN_RESPONSE_INVALID"))?;
    if bytes.len() > STREAM_TOKEN_MAX_WIRE_BYTES_V1 || STANDARD.encode(&bytes) != encoded {
        return Err(control_integrity("MUSUBI_ARCHIVE_TOKEN_RESPONSE_INVALID"));
    }
    let limits = norito::DecodeLimits::new(
        STREAM_TOKEN_MAX_WIRE_BYTES_V1,
        STREAM_TOKEN_MAX_WIRE_BYTES_V1,
        STREAM_TOKEN_MAX_WIRE_BYTES_V1.saturating_mul(2),
        STREAM_TOKEN_MAX_WIRE_BYTES_V1.saturating_mul(4),
        32,
    );
    let token: StreamTokenV1 = norito::decode_from_bytes_with_limits(&bytes, limits)
        .map_err(|_| control_integrity("MUSUBI_ARCHIVE_TOKEN_RESPONSE_INVALID"))?;
    let canonical = norito::encode_canonical(&token)
        .map_err(|_| control_integrity("MUSUBI_ARCHIVE_TOKEN_RESPONSE_INVALID"))?;
    if canonical != bytes
        || token.body.requests_per_minute == 0
        || token.body.ttl_epoch <= token.body.issued_at
    {
        return Err(control_integrity("MUSUBI_ARCHIVE_TOKEN_RESPONSE_INVALID"));
    }
    Ok(token)
}
fn canonical_car_reader(
    sessions: GatewaySessionFactoryV1,
    initial_session: GatewaySessionV1,
    plan: CarBuildPlan,
    commitment: MusubiArchiveCommitmentV1,
    stream_failure: Arc<Mutex<Option<MusubiArchiveRuntimeErrorV1>>>,
) -> Result<Box<dyn Read + Send + 'static>, MusubiArchiveRuntimeErrorV1> {
    enforce_plan_memory_bound(&plan)?;
    let expected_car_size = commitment.car_size;
    bounded_stream::bounded_car_reader(expected_car_size, move |output| {
        let result = stream_canonical_car(&plan, &commitment, sessions, initial_session, output);
        match result {
            Ok(()) => Ok(()),
            Err(error) => {
                if let Ok(mut failure) = stream_failure.lock() {
                    *failure = Some(error);
                }
                Err(error.code())
            }
        }
    })
    .map_err(|_| permanent("MUSUBI_ARCHIVE_STREAM_THREAD_FAILED"))
}
fn stream_canonical_car(
    plan: &CarBuildPlan,
    commitment: &MusubiArchiveCommitmentV1,
    sessions: GatewaySessionFactoryV1,
    initial_session: GatewaySessionV1,
    output: &mut dyn Write,
) -> Result<(), MusubiArchiveRuntimeErrorV1> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| permanent("MUSUBI_ARCHIVE_STREAM_RUNTIME_FAILED"))?;
    let mut payload = GatewayPayloadReaderV1 {
        runtime,
        sessions,
        session: initial_session,
        chunks: plan
            .try_chunk_fetch_specs()
            .map_err(|_| integrity("MUSUBI_ARCHIVE_PLAN_RESPONSE_INVALID"))?,
        next_chunk: 0,
        current: Cursor::new(Vec::new()),
        failure: None,
    };
    let writer = CarStreamingWriter::with_expected_roots(
        plan,
        vec![commitment.root_cid.as_bytes().to_vec()],
    );
    let stats = match writer.write_from_reader(&mut payload, output) {
        Ok(stats) => stats,
        Err(_) => {
            return Err(payload
                .failure
                .unwrap_or_else(|| integrity("MUSUBI_ARCHIVE_PROVIDER_STREAM_INVALID")));
        }
    };
    let actual_car_digest = stats.car_archive_digest.as_bytes();
    let expected_car_digest = commitment.car_digest.as_bytes();
    if stats.car_size != commitment.car_size
        || stats.car_size > MUSUBI_MAX_CAR_BYTES_V1
        || actual_car_digest != expected_car_digest
        || stats.root_cids.as_slice() != [commitment.root_cid.as_bytes().to_vec()]
        || stats.chunk_count != usize::try_from(commitment.chunk_count).unwrap_or(usize::MAX)
        || stats.payload_bytes != commitment.content_length
    {
        return Err(integrity("MUSUBI_ARCHIVE_PROVIDER_STREAM_INVALID"));
    }
    Ok(())
}
struct GatewayPayloadReaderV1 {
    runtime: tokio::runtime::Runtime,
    sessions: GatewaySessionFactoryV1,
    session: GatewaySessionV1,
    chunks: Vec<ChunkFetchSpec>,
    next_chunk: usize,
    current: Cursor<Vec<u8>>,
    failure: Option<MusubiArchiveRuntimeErrorV1>,
}
impl Read for GatewayPayloadReaderV1 {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        loop {
            let read = self.current.read(output)?;
            if read != 0 {
                return Ok(read);
            }
            let Some(spec) = self.chunks.get(self.next_chunk).cloned() else {
                return Ok(0);
            };
            self.ensure_session()?;
            self.reserve_byte_budget(u64::from(spec.length))?;
            if self.ensure_session()? {
                self.reserve_byte_budget(u64::from(spec.length))?;
            }
            let fetched = self
                .runtime
                .block_on(self.session.fetcher.fetch(FetchRequest {
                    provider: Arc::clone(&self.session.provider),
                    spec: spec.clone(),
                    attempt: 1,
                }));
            let response = match fetched {
                Ok(response) => response,
                Err(error) => {
                    return Err(self.fail(classify_gateway_fetch_error(&error)));
                }
            };
            self.session.requests_remaining = self.session.requests_remaining.saturating_sub(1);
            if response.bytes.len() != usize::try_from(spec.length).unwrap_or(usize::MAX)
                || blake3::hash(&response.bytes).as_bytes() != &spec.digest
            {
                return Err(self.fail(integrity("MUSUBI_ARCHIVE_CHUNK_INTEGRITY_FAILED")));
            }
            self.current = Cursor::new(response.bytes);
            self.next_chunk += 1;
        }
    }
}
fn classify_gateway_fetch_error(error: &GatewayFetchError) -> MusubiArchiveRuntimeErrorV1 {
    match error {
        GatewayFetchError::Request { .. } | GatewayFetchError::RequestBody { .. } => {
            retryable("MUSUBI_ARCHIVE_CHUNK_REQUEST_FAILED")
        }
        GatewayFetchError::ExpiredStreamToken { .. } => {
            retryable("MUSUBI_ARCHIVE_STREAM_TOKEN_EXPIRED")
        }
        GatewayFetchError::ResponseTooLarge { .. }
        | GatewayFetchError::CacheVersionMismatch { .. } => {
            integrity("MUSUBI_ARCHIVE_CHUNK_RESPONSE_INVALID")
        }
        GatewayFetchError::PolicyBlocked { .. } => {
            unavailable("MUSUBI_ARCHIVE_CHUNK_GOVERNED_UNAVAILABLE")
        }
        GatewayFetchError::UnexpectedStatus {
            status: StatusCode::NOT_FOUND | StatusCode::GONE,
            ..
        } => unavailable("MUSUBI_ARCHIVE_CHUNK_UNAVAILABLE"),
        GatewayFetchError::UnexpectedStatus {
            status:
                StatusCode::REQUEST_TIMEOUT
                | StatusCode::TOO_EARLY
                | StatusCode::TOO_MANY_REQUESTS
                | StatusCode::INTERNAL_SERVER_ERROR
                | StatusCode::BAD_GATEWAY
                | StatusCode::SERVICE_UNAVAILABLE
                | StatusCode::GATEWAY_TIMEOUT,
            ..
        } => retryable("MUSUBI_ARCHIVE_CHUNK_RETRYABLE"),
        GatewayFetchError::UnexpectedStatus {
            status: StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS,
            ..
        } => unavailable("MUSUBI_ARCHIVE_CHUNK_GOVERNED_UNAVAILABLE"),
        GatewayFetchError::UnknownProvider { .. }
        | GatewayFetchError::SystemClockBeforeUnixEpoch
        | GatewayFetchError::NonceExhausted { .. }
        | GatewayFetchError::InvalidRequestHeader { .. }
        | GatewayFetchError::UrlJoin { .. }
        | GatewayFetchError::UnexpectedStatus { .. } => {
            permanent("MUSUBI_ARCHIVE_CHUNK_REQUEST_REJECTED")
        }
    }
}
impl GatewayPayloadReaderV1 {
    fn ensure_session(&mut self) -> io::Result<bool> {
        let now = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(elapsed) => elapsed.as_secs(),
            Err(_) => return Err(self.fail(permanent("MUSUBI_ARCHIVE_STREAM_CLOCK_INVALID"))),
        };
        if self.session.requests_remaining != 0 && now.saturating_add(1) < self.session.ttl_epoch {
            return Ok(false);
        }
        self.session = match self.sessions.open() {
            Ok(session) => session,
            Err(error) => return Err(self.fail(error)),
        };
        Ok(true)
    }
    fn reserve_byte_budget(&mut self, requested: u64) -> io::Result<()> {
        if requested == 0 || requested > self.session.byte_rate_limit {
            return Err(self.fail(permanent("MUSUBI_ARCHIVE_TOKEN_BUDGET_INSUFFICIENT")));
        }
        let now = Instant::now();
        let started = self.session.byte_window_started.get_or_insert(now);
        let elapsed = now.saturating_duration_since(*started);
        if elapsed >= TOKEN_BYTE_WINDOW_GUARD {
            *started = now;
            self.session.byte_window_used = 0;
        }
        if self
            .session
            .byte_window_used
            .checked_add(requested)
            .is_none_or(|total| total > self.session.byte_rate_limit)
        {
            let delay = TOKEN_BYTE_WINDOW_GUARD.saturating_sub(elapsed);
            thread::sleep(delay);
            self.session.byte_window_started = Some(Instant::now());
            self.session.byte_window_used = 0;
        }
        self.session.byte_window_used = match self.session.byte_window_used.checked_add(requested) {
            Some(total) => total,
            None => return Err(self.fail(permanent("MUSUBI_ARCHIVE_TOKEN_BUDGET_INVALID"))),
        };
        Ok(())
    }
    fn fail(&mut self, error: MusubiArchiveRuntimeErrorV1) -> io::Error {
        self.failure = Some(error);
        let kind = if error.class() == MusubiArchiveRuntimeFailureClassV1::Integrity {
            io::ErrorKind::InvalidData
        } else {
            io::ErrorKind::Other
        };
        io::Error::new(kind, error.code())
    }
}
fn parse_fetch_subtree(
    document: &toml::Value,
) -> Result<MusubiFetchConfig, MusubiArchiveRuntimeErrorV1> {
    let musubi = document
        .get("musubi")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| permanent("MUSUBI_ARCHIVE_FETCH_CONFIG_MISSING"))?;
    let fetch = musubi
        .get("fetch")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| permanent("MUSUBI_ARCHIVE_FETCH_CONFIG_MISSING"))?;
    if fetch.keys().any(|key| {
        !matches!(
            key.as_str(),
            "network_id" | "client_id" | "request_timeout_ms" | "provider_gateways"
        )
    }) {
        return Err(permanent("MUSUBI_ARCHIVE_FETCH_CONFIG_INVALID"));
    }
    let network_id_text = fetch
        .get("network_id")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| permanent("MUSUBI_ARCHIVE_FETCH_NETWORK_ID_MISSING"))?;
    let network_id = network_id_text
        .parse::<NetworkId>()
        .map_err(|_| permanent("MUSUBI_ARCHIVE_FETCH_NETWORK_ID_INVALID"))?;
    if network_id.to_string() != network_id_text {
        return Err(permanent("MUSUBI_ARCHIVE_FETCH_NETWORK_ID_INVALID"));
    }
    let client_id = fetch
        .get("client_id")
        .map(|value| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| permanent("MUSUBI_ARCHIVE_FETCH_CONFIG_INVALID"))
        })
        .transpose()?;
    let request_timeout_ms = fetch
        .get("request_timeout_ms")
        .map(|value| {
            value
                .as_integer()
                .and_then(|value| u64::try_from(value).ok())
                .ok_or_else(|| permanent("MUSUBI_ARCHIVE_FETCH_CONFIG_INVALID"))
        })
        .transpose()?;
    let configured = fetch
        .get("provider_gateways")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| permanent("MUSUBI_ARCHIVE_FETCH_CONFIG_MISSING"))?;
    if configured.is_empty() || configured.len() > MAX_CONFIGURED_PROVIDERS {
        return Err(permanent("MUSUBI_ARCHIVE_FETCH_CONFIG_INVALID"));
    }
    let mut provider_gateways = Vec::new();
    provider_gateways
        .try_reserve_exact(configured.len())
        .map_err(|_| permanent("MUSUBI_ARCHIVE_FETCH_CONFIG_INVALID"))?;
    for value in configured {
        let provider = value
            .as_table()
            .ok_or_else(|| permanent("MUSUBI_ARCHIVE_FETCH_CONFIG_INVALID"))?;
        if provider.len() != 4
            || provider.keys().any(|key| {
                !matches!(
                    key.as_str(),
                    "provider_id" | "url" | "operator_public_key" | "operator_private_key_file"
                )
            })
        {
            return Err(permanent("MUSUBI_ARCHIVE_FETCH_CONFIG_INVALID"));
        }
        let field = |name: &str| {
            provider
                .get(name)
                .and_then(toml::Value::as_str)
                .filter(|value| !value.is_empty() && value.trim() == *value)
                .map(ToOwned::to_owned)
                .ok_or_else(|| permanent("MUSUBI_ARCHIVE_FETCH_CONFIG_INVALID"))
        };
        provider_gateways.push(MusubiFetchProviderGatewayConfig {
            provider_id: field("provider_id")?,
            url: field("url")?,
            operator_public_key: field("operator_public_key")?,
            operator_private_key_file: field("operator_private_key_file")?,
        });
    }
    Ok(MusubiFetchConfig {
        network_id: Some(network_id),
        client_id,
        request_timeout_ms,
        provider_gateways,
    })
}
fn parse_gateway_base_url(raw: &str) -> Result<Url, MusubiArchiveRuntimeErrorV1> {
    if raw.is_empty() || raw.len() > 2_048 || raw.trim() != raw {
        return Err(permanent("MUSUBI_ARCHIVE_FETCH_GATEWAY_URL_INVALID"));
    }
    let url = Url::parse(raw).map_err(|_| permanent("MUSUBI_ARCHIVE_FETCH_GATEWAY_URL_INVALID"))?;
    let canonical = url.as_str();
    let omitted_root_slash = canonical
        .strip_suffix('/')
        .is_some_and(|without_slash| raw == without_slash && url.path() == "/");
    if (raw != canonical && !omitted_root_slash)
        || url.scheme() != "https"
        || url.port_or_known_default() != Some(443)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
    {
        return Err(permanent("MUSUBI_ARCHIVE_FETCH_GATEWAY_URL_INVALID"));
    }
    let host = url
        .host()
        .ok_or_else(|| permanent("MUSUBI_ARCHIVE_FETCH_GATEWAY_URL_INVALID"))?;
    if matches!(host, Host::Ipv4(address) if !is_public_ip(IpAddr::V4(address)))
        || matches!(host, Host::Ipv6(address) if !is_public_ip(IpAddr::V6(address)))
    {
        return Err(permanent("MUSUBI_ARCHIVE_FETCH_GATEWAY_URL_INVALID"));
    }
    Ok(url)
}
fn pinned_http_client(
    base_url: &Url,
    request_timeout: Duration,
) -> Result<HttpClient, MusubiArchiveRuntimeErrorV1> {
    let host = base_url
        .host_str()
        .ok_or_else(|| permanent("MUSUBI_ARCHIVE_FETCH_GATEWAY_URL_INVALID"))?;
    let mut builder = HttpClient::builder()
        .no_proxy()
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .no_zstd()
        .https_only(true)
        .redirect(RedirectPolicy::none())
        .retry(reqwest::retry::never())
        .connect_timeout(request_timeout.min(Duration::from_secs(10)))
        .timeout(request_timeout);
    if !matches!(base_url.host(), Some(Host::Ipv4(_) | Host::Ipv6(_))) {
        let mut addresses = (host, 443)
            .to_socket_addrs()
            .map_err(|_| permanent("MUSUBI_ARCHIVE_FETCH_DNS_INVALID"))?
            .collect::<Vec<_>>();
        addresses.sort_unstable();
        addresses.dedup();
        if addresses.is_empty()
            || addresses.len() > MAX_DNS_ADDRESSES_PER_HOST
            || addresses.iter().any(|address| !is_public_ip(address.ip()))
        {
            return Err(permanent("MUSUBI_ARCHIVE_FETCH_DNS_INVALID"));
        }
        builder = builder.resolve_to_addrs(host, &addresses);
    }
    builder
        .build()
        .map_err(|_| permanent("MUSUBI_ARCHIVE_FETCH_HTTP_CLIENT_INVALID"))
}
fn is_public_ip(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => {
            let [first, second, third, _] = address.octets();
            !address.is_private()
                && !address.is_loopback()
                && !address.is_link_local()
                && !address.is_broadcast()
                && !address.is_documentation()
                && !address.is_unspecified()
                && !address.is_multicast()
                && first != 0
                && !(first == 100 && (64..=127).contains(&second))
                && !(first == 192 && second == 0 && third == 0)
                && !(first == 192 && second == 88 && third == 99)
                && !(first == 198 && (18..=19).contains(&second))
                && first < 240
        }
        IpAddr::V6(address) => {
            let segments = address.segments();
            let global_unicast = segments[0] & 0xe000 == 0x2000;
            let documentation = (segments[0] == 0x2001 && segments[1] == 0x0db8)
                || (segments[0] == 0x3fff && segments[1] & 0xf000 == 0);
            let special_purpose = segments[0] == 0x2001 && segments[1] <= 0x01ff;
            let six_to_four = segments[0] == 0x2002;
            global_unicast
                && !documentation
                && !special_purpose
                && !six_to_four
                && !address.is_loopback()
                && !address.is_unspecified()
                && !address.is_multicast()
        }
    }
}
fn gateway_origin(url: &Url) -> Option<(String, String, u16)> {
    Some((
        url.scheme().to_owned(),
        url.host_str()?.to_ascii_lowercase(),
        url.port_or_known_default()?,
    ))
}
fn anchor_config_path(path: &Path) -> Result<PathBuf, MusubiArchiveRuntimeErrorV1> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    std::env::current_dir()
        .map(|current| current.join(path))
        .map_err(|_| permanent("MUSUBI_ARCHIVE_FETCH_CONFIG_INVALID"))
}
fn resolve_config_path(
    config_path: &Path,
    configured: &str,
) -> Result<PathBuf, MusubiArchiveRuntimeErrorV1> {
    if configured.is_empty()
        || configured.len() > 4_096
        || configured.trim() != configured
        || configured.contains('\0')
    {
        return Err(permanent("MUSUBI_ARCHIVE_FETCH_OPERATOR_KEY_FILE_INVALID"));
    }
    let path = Path::new(configured);
    Ok(if path.is_absolute() {
        path.to_path_buf()
    } else {
        let anchored_config = if config_path.is_absolute() {
            config_path.to_path_buf()
        } else {
            std::env::current_dir()
                .map_err(|_| permanent("MUSUBI_ARCHIVE_FETCH_OPERATOR_KEY_FILE_INVALID"))?
                .join(config_path)
        };
        anchored_config
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(path)
    })
}
fn read_operator_key_pair(
    path: &Path,
    expected_public_key: &PublicKey,
) -> Result<KeyPair, MusubiArchiveRuntimeErrorV1> {
    let (bytes, metadata) = read_bounded_regular(path, MAX_OPERATOR_PRIVATE_KEY_BYTES)
        .map_err(|_| permanent("MUSUBI_ARCHIVE_FETCH_OPERATOR_KEY_FILE_INVALID"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        if metadata.permissions().mode() & 0o7777 != 0o600 {
            return Err(permanent(
                "MUSUBI_ARCHIVE_FETCH_OPERATOR_KEY_FILE_PERMISSIONS",
            ));
        }
    }
    let encoded = std::str::from_utf8(&bytes)
        .map_err(|_| permanent("MUSUBI_ARCHIVE_FETCH_OPERATOR_KEY_FILE_INVALID"))?;
    let encoded = encoded.strip_suffix('\n').unwrap_or(encoded);
    if !valid_visible_ascii(
        encoded,
        16,
        usize::try_from(MAX_OPERATOR_PRIVATE_KEY_BYTES).unwrap_or(usize::MAX),
    ) || encoded.contains(['\r', '\n'])
    {
        return Err(permanent("MUSUBI_ARCHIVE_FETCH_OPERATOR_KEY_FILE_INVALID"));
    }
    let private_key = encoded
        .parse::<PrivateKey>()
        .map_err(|_| permanent("MUSUBI_ARCHIVE_FETCH_OPERATOR_KEY_FILE_INVALID"))?;
    if ExposedPrivateKey(private_key.clone()).to_string() != encoded {
        return Err(permanent("MUSUBI_ARCHIVE_FETCH_OPERATOR_KEY_FILE_INVALID"));
    }
    KeyPair::new(expected_public_key.clone(), private_key)
        .map_err(|_| permanent("MUSUBI_ARCHIVE_FETCH_OPERATOR_KEY_MISMATCH"))
}
fn read_bounded_regular(path: &Path, maximum: u64) -> io::Result<(Vec<u8>, fs::Metadata)> {
    #[cfg(not(unix))]
    {
        let _ = (path, maximum);
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "secure no-follow file reads are unavailable on this platform",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _};
        let mut options = fs::OpenOptions::new();
        options
            .read(true)
            .custom_flags(platform_no_follow_flag() | platform_nonblocking_flag());
        let mut file = options.open(path)?;
        let before = file.metadata()?;
        if !before.is_file() || before.len() == 0 || before.len() > maximum || before.nlink() != 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "input is not a bounded singly-linked regular file",
            ));
        }
        let capacity = usize::try_from(before.len())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "input exceeds host width"))?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(capacity)
            .map_err(|_| io::Error::new(io::ErrorKind::OutOfMemory, "input allocation failed"))?;
        Read::by_ref(&mut file)
            .take(maximum.saturating_add(1))
            .read_to_end(&mut bytes)?;
        let after = file.metadata()?;
        if !after.is_file()
            || after.nlink() != 1
            || after.dev() != before.dev()
            || after.ino() != before.ino()
            || after.len() != before.len()
            || after.mtime() != before.mtime()
            || after.mtime_nsec() != before.mtime_nsec()
            || after.ctime() != before.ctime()
            || after.ctime_nsec() != before.ctime_nsec()
            || u64::try_from(bytes.len()).ok() != Some(before.len())
            || after.len() > maximum
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "input changed while being read",
            ));
        }
        Ok((bytes, after))
    }
}
#[cfg(all(
    target_os = "android",
    not(any(
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "riscv64",
        target_arch = "x86",
        target_arch = "x86_64"
    ))
))]
compile_error!("Musubi secure fetch-file reads are not qualified for this Android architecture");
#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))
))]
compile_error!("Musubi secure fetch-file reads are not qualified for this Unix target");
#[cfg(all(target_os = "android", target_arch = "riscv64"))]
const fn platform_no_follow_flag() -> i32 {
    0x400000
}
#[cfg(all(
    target_os = "android",
    any(target_arch = "aarch64", target_arch = "arm")
))]
const fn platform_no_follow_flag() -> i32 {
    0x8000
}
#[cfg(all(
    target_os = "android",
    any(target_arch = "x86", target_arch = "x86_64")
))]
const fn platform_no_follow_flag() -> i32 {
    0x20000
}
#[cfg(all(
    target_os = "linux",
    any(
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "m68k",
        target_arch = "powerpc",
        target_arch = "powerpc64"
    )
))]
const fn platform_no_follow_flag() -> i32 {
    0x8000
}
#[cfg(all(
    target_os = "linux",
    not(any(
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "m68k",
        target_arch = "powerpc",
        target_arch = "powerpc64"
    ))
))]
const fn platform_no_follow_flag() -> i32 {
    0x20000
}
#[cfg(all(
    unix,
    not(any(target_os = "linux", target_os = "android", target_os = "macos")),
    any(
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    )
))]
const fn platform_no_follow_flag() -> i32 {
    0x100
}
#[cfg(target_os = "macos")]
const fn platform_no_follow_flag() -> i32 {
    // O_NOFOLLOW rejects a substituted final component; the opened descriptor then remains the
    // sole read authority even if an ancestor or the directory entry is replaced concurrently.
    0x100
}
#[cfg(all(
    target_os = "linux",
    any(
        target_arch = "mips",
        target_arch = "mips32r6",
        target_arch = "mips64",
        target_arch = "mips64r6"
    )
))]
const fn platform_nonblocking_flag() -> i32 {
    0x80
}
#[cfg(all(
    target_os = "linux",
    any(target_arch = "sparc", target_arch = "sparc64")
))]
const fn platform_nonblocking_flag() -> i32 {
    0x4000
}
#[cfg(any(
    target_os = "android",
    all(
        target_os = "linux",
        not(any(
            target_arch = "mips",
            target_arch = "mips32r6",
            target_arch = "mips64",
            target_arch = "mips64r6",
            target_arch = "sparc",
            target_arch = "sparc64"
        ))
    )
))]
const fn platform_nonblocking_flag() -> i32 {
    0x800
}
#[cfg(all(
    unix,
    not(any(target_os = "linux", target_os = "android")),
    any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    )
))]
const fn platform_nonblocking_flag() -> i32 {
    0x4
}
#[cfg(unix)]
/// Return the qualified flags for a nonblocking, final-component no-follow open.
pub(crate) const fn secure_no_follow_nonblocking_flags() -> i32 {
    platform_no_follow_flag() | platform_nonblocking_flag()
}
#[cfg(all(target_os = "android", target_arch = "riscv64"))]
const fn platform_directory_only_flag() -> i32 {
    0x200000
}
#[cfg(all(
    target_os = "android",
    any(target_arch = "aarch64", target_arch = "arm")
))]
const fn platform_directory_only_flag() -> i32 {
    0x4000
}
#[cfg(all(
    target_os = "android",
    any(target_arch = "x86", target_arch = "x86_64")
))]
const fn platform_directory_only_flag() -> i32 {
    0x10000
}
#[cfg(all(
    target_os = "linux",
    any(
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "m68k",
        target_arch = "powerpc",
        target_arch = "powerpc64"
    )
))]
const fn platform_directory_only_flag() -> i32 {
    0x4000
}
#[cfg(all(
    target_os = "linux",
    not(any(
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "m68k",
        target_arch = "powerpc",
        target_arch = "powerpc64"
    ))
))]
const fn platform_directory_only_flag() -> i32 {
    0x10000
}
#[cfg(any(target_os = "macos", target_os = "ios"))]
const fn platform_directory_only_flag() -> i32 {
    0x0010_0000
}
#[cfg(target_os = "freebsd")]
const fn platform_directory_only_flag() -> i32 {
    0x0002_0000
}
#[cfg(target_os = "dragonfly")]
const fn platform_directory_only_flag() -> i32 {
    0x0800_0000
}
#[cfg(target_os = "openbsd")]
const fn platform_directory_only_flag() -> i32 {
    0x0002_0000
}
#[cfg(target_os = "netbsd")]
const fn platform_directory_only_flag() -> i32 {
    0x0020_0000
}
/// Return the qualified flags for a nonblocking, no-follow directory open.
#[cfg(unix)]
pub(crate) const fn secure_directory_open_flags() -> i32 {
    secure_no_follow_nonblocking_flags() | platform_directory_only_flag()
}
fn read_json_response(
    response: HttpResponse,
    maximum: u64,
    code_prefix: &'static str,
    integrity_surface: MusubiArchiveRuntimeIntegritySurfaceV1,
) -> Result<Vec<u8>, MusubiArchiveRuntimeErrorV1> {
    let status = response.status();
    if !status.is_success() {
        return Err(http_status_error(status, code_prefix));
    }
    if response.url().scheme() != "https" {
        return Err(permanent("MUSUBI_ARCHIVE_FETCH_REDIRECT_REJECTED"));
    }
    if response
        .headers()
        .get(CONTENT_ENCODING)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| !value.eq_ignore_ascii_case("identity"))
    {
        return Err(surfaced_integrity(
            "MUSUBI_ARCHIVE_RESPONSE_ENCODING_INVALID",
            integrity_surface,
        ));
    }
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(str::trim);
    if content_type != Some(APPLICATION_JSON) {
        return Err(surfaced_integrity(
            "MUSUBI_ARCHIVE_RESPONSE_CONTENT_TYPE_INVALID",
            integrity_surface,
        ));
    }
    if response
        .content_length()
        .is_some_and(|length| length > maximum)
    {
        return Err(surfaced_integrity(
            "MUSUBI_ARCHIVE_RESPONSE_TOO_LARGE",
            integrity_surface,
        ));
    }
    let maximum_usize =
        usize::try_from(maximum).map_err(|_| permanent("MUSUBI_ARCHIVE_RESPONSE_LIMIT_INVALID"))?;
    let mut body = Vec::new();
    body.try_reserve_exact(16 * 1024)
        .map_err(|_| permanent("MUSUBI_ARCHIVE_RESPONSE_ALLOCATION_FAILED"))?;
    response
        .take(maximum.saturating_add(1))
        .read_to_end(&mut body)
        .map_err(|_| retryable("MUSUBI_ARCHIVE_RESPONSE_READ_FAILED"))?;
    if body.is_empty() || body.len() > maximum_usize {
        return Err(surfaced_integrity(
            "MUSUBI_ARCHIVE_RESPONSE_TOO_LARGE",
            integrity_surface,
        ));
    }
    Ok(body)
}
fn http_status_error(status: StatusCode, prefix: &'static str) -> MusubiArchiveRuntimeErrorV1 {
    if matches!(status, StatusCode::NOT_FOUND | StatusCode::GONE) {
        return unavailable(match prefix {
            "MUSUBI_ARCHIVE_MANIFEST" => "MUSUBI_ARCHIVE_MANIFEST_UNAVAILABLE",
            "MUSUBI_ARCHIVE_PLAN" => "MUSUBI_ARCHIVE_PLAN_UNAVAILABLE",
            "MUSUBI_ARCHIVE_TOKEN" => "MUSUBI_ARCHIVE_TOKEN_UNAVAILABLE",
            _ => "MUSUBI_ARCHIVE_PROVIDER_UNAVAILABLE",
        });
    }
    if matches!(
        status,
        StatusCode::REQUEST_TIMEOUT
            | StatusCode::TOO_EARLY
            | StatusCode::TOO_MANY_REQUESTS
            | StatusCode::INTERNAL_SERVER_ERROR
            | StatusCode::BAD_GATEWAY
            | StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::GATEWAY_TIMEOUT
    ) {
        return retryable(match prefix {
            "MUSUBI_ARCHIVE_MANIFEST" => "MUSUBI_ARCHIVE_MANIFEST_RETRYABLE",
            "MUSUBI_ARCHIVE_PLAN" => "MUSUBI_ARCHIVE_PLAN_RETRYABLE",
            "MUSUBI_ARCHIVE_TOKEN" => "MUSUBI_ARCHIVE_TOKEN_RETRYABLE",
            _ => "MUSUBI_ARCHIVE_PROVIDER_RETRYABLE",
        });
    }
    permanent(match prefix {
        "MUSUBI_ARCHIVE_MANIFEST" => "MUSUBI_ARCHIVE_MANIFEST_REJECTED",
        "MUSUBI_ARCHIVE_PLAN" => "MUSUBI_ARCHIVE_PLAN_REJECTED",
        "MUSUBI_ARCHIVE_TOKEN" => "MUSUBI_ARCHIVE_TOKEN_REJECTED",
        _ => "MUSUBI_ARCHIVE_PROVIDER_REJECTED",
    })
}
fn enforce_plan_memory_bound(plan: &CarBuildPlan) -> Result<(), MusubiArchiveRuntimeErrorV1> {
    let chunk_bytes = plan
        .chunks
        .capacity()
        .checked_mul(std::mem::size_of::<CarChunk>())
        .ok_or_else(|| permanent("MUSUBI_ARCHIVE_PLAN_MEMORY_LIMIT"))?;
    let file_bytes = plan
        .files
        .capacity()
        .checked_mul(std::mem::size_of::<FilePlan>())
        .ok_or_else(|| permanent("MUSUBI_ARCHIVE_PLAN_MEMORY_LIMIT"))?;
    let path_bytes = plan.files.iter().try_fold(0_usize, |total, file| {
        let total = total.checked_add(
            file.path
                .capacity()
                .checked_mul(std::mem::size_of::<String>())?,
        )?;
        file.path.iter().try_fold(total, |total, component| {
            total.checked_add(component.capacity())
        })
    });
    let estimated = chunk_bytes
        .checked_add(file_bytes)
        .and_then(|value| value.checked_add(path_bytes.unwrap_or(usize::MAX)))
        .ok_or_else(|| permanent("MUSUBI_ARCHIVE_PLAN_MEMORY_LIMIT"))?;
    let max_chunk = plan
        .chunks
        .iter()
        .map(|chunk| usize::try_from(chunk.length).unwrap_or(usize::MAX))
        .max()
        .unwrap_or(0);
    let fetch_specs = plan
        .chunks
        .capacity()
        .checked_mul(std::mem::size_of::<ChunkFetchSpec>())
        .ok_or_else(|| permanent("MUSUBI_ARCHIVE_PLAN_MEMORY_LIMIT"))?;
    if estimated > MAX_RETAINED_PLAN_HEAP_BYTES
        || max_chunk == 0
        || max_chunk > MAX_STREAM_CHUNK_BYTES
        || estimated
            // One plan remains with the cache caller while one is owned by the stream worker.
            .checked_mul(2)
            // The worker derives one exact authenticated chunk-fetch inventory.
            .and_then(|value| value.checked_add(fetch_specs))
            // Cover the authenticated response, CAR writer section, and current reader chunk.
            .and_then(|value| {
                max_chunk
                    .checked_mul(3)
                    .and_then(|chunks| value.checked_add(chunks))
            })
            .and_then(|value| value.checked_add(bounded_stream::STREAM_MAX_OWNED_FRAME_BYTES))
            .is_none_or(|working| working > 64 * 1024 * 1024)
    {
        return Err(permanent("MUSUBI_ARCHIVE_PLAN_MEMORY_LIMIT"));
    }
    Ok(())
}
fn exact_plan_binding(plan: &CarBuildPlan) -> Result<[u8; 32], MusubiArchiveRuntimeErrorV1> {
    let mut transcript =
        blake3::Hasher::new_derive_key("iroha.musubi.sorafs.exact-car-build-plan-binding.v1");
    update_plan_usize(&mut transcript, plan.chunk_profile.min_size)?;
    update_plan_usize(&mut transcript, plan.chunk_profile.target_size)?;
    update_plan_usize(&mut transcript, plan.chunk_profile.max_size)?;
    transcript.update(&plan.chunk_profile.break_mask.to_le_bytes());
    transcript.update(plan.payload_digest.as_bytes());
    transcript.update(&plan.content_length.to_le_bytes());
    update_plan_usize(&mut transcript, plan.chunks.len())?;
    for chunk in &plan.chunks {
        if chunk.taikai_segment_hint.is_some() {
            return Err(integrity("MUSUBI_ARCHIVE_PLAN_BINDING_INVALID"));
        }
        transcript.update(&chunk.offset.to_le_bytes());
        transcript.update(&chunk.length.to_le_bytes());
        transcript.update(&chunk.digest);
    }
    update_plan_usize(&mut transcript, plan.files.len())?;
    for file in &plan.files {
        update_plan_usize(&mut transcript, file.path.len())?;
        for component in &file.path {
            update_plan_usize(&mut transcript, component.len())?;
            transcript.update(component.as_bytes());
        }
        update_plan_usize(&mut transcript, file.first_chunk)?;
        update_plan_usize(&mut transcript, file.chunk_count)?;
        transcript.update(&file.size.to_le_bytes());
    }
    Ok(*transcript.finalize().as_bytes())
}
fn update_plan_usize(
    transcript: &mut blake3::Hasher,
    value: usize,
) -> Result<(), MusubiArchiveRuntimeErrorV1> {
    let value =
        u64::try_from(value).map_err(|_| permanent("MUSUBI_ARCHIVE_PLAN_BINDING_INVALID"))?;
    transcript.update(&value.to_le_bytes());
    Ok(())
}
fn parse_provider_id(raw: &str) -> Result<ProviderId, MusubiArchiveRuntimeErrorV1> {
    if !is_lower_hex(raw, 64) {
        return Err(permanent("MUSUBI_ARCHIVE_FETCH_PROVIDER_ID_INVALID"));
    }
    let decoded =
        hex::decode(raw).map_err(|_| permanent("MUSUBI_ARCHIVE_FETCH_PROVIDER_ID_INVALID"))?;
    let bytes = <[u8; 32]>::try_from(decoded)
        .map_err(|_| permanent("MUSUBI_ARCHIVE_FETCH_PROVIDER_ID_INVALID"))?;
    if bytes.iter().all(|byte| *byte == 0) {
        return Err(permanent("MUSUBI_ARCHIVE_FETCH_PROVIDER_ID_INVALID"));
    }
    Ok(ProviderId::new(bytes))
}
fn parse_lower_hex_32(raw: &str) -> Result<[u8; 32], MusubiArchiveRuntimeErrorV1> {
    if !is_lower_hex(raw, 64) {
        return Err(integrity("MUSUBI_ARCHIVE_RESPONSE_HEX_INVALID"));
    }
    let decoded = hex::decode(raw).map_err(|_| integrity("MUSUBI_ARCHIVE_RESPONSE_HEX_INVALID"))?;
    <[u8; 32]>::try_from(decoded).map_err(|_| integrity("MUSUBI_ARCHIVE_RESPONSE_HEX_INVALID"))
}
fn is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
fn valid_visible_ascii(value: &str, minimum: usize, maximum: usize) -> bool {
    (minimum..=maximum).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_graphic())
}
fn random_nonce() -> Result<String, MusubiArchiveRuntimeErrorV1> {
    let mut bytes = [0_u8; 16];
    OsRng
        .try_fill_bytes(&mut bytes)
        .map_err(|_| permanent("MUSUBI_ARCHIVE_NONCE_UNAVAILABLE"))?;
    if bytes.iter().all(|byte| *byte == 0) {
        return Err(permanent("MUSUBI_ARCHIVE_NONCE_UNAVAILABLE"));
    }
    let encoded_bytes = bytes
        .len()
        .checked_mul(2)
        .ok_or_else(|| permanent("MUSUBI_ARCHIVE_NONCE_UNAVAILABLE"))?;
    let mut encoded = Vec::new();
    encoded
        .try_reserve_exact(encoded_bytes)
        .map_err(|_| permanent("MUSUBI_ARCHIVE_NONCE_UNAVAILABLE"))?;
    encoded.resize(encoded_bytes, 0);
    hex::encode_to_slice(bytes, &mut encoded)
        .map_err(|_| permanent("MUSUBI_ARCHIVE_NONCE_UNAVAILABLE"))?;
    String::from_utf8(encoded).map_err(|_| permanent("MUSUBI_ARCHIVE_NONCE_UNAVAILABLE"))
}
fn header_text<'headers>(headers: &'headers HeaderMap, name: &str) -> Option<&'headers str> {
    let mut values = headers.get_all(name).iter();
    let value = values.next()?.to_str().ok()?;
    if values.next().is_some() {
        return None;
    }
    Some(value)
}
fn required_string<'value>(
    map: &'value norito::json::Map,
    field: &str,
) -> Result<&'value str, MusubiArchiveRuntimeErrorV1> {
    map.get(field)
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| integrity("MUSUBI_ARCHIVE_RESPONSE_INVALID"))
}
fn required_u64(map: &norito::json::Map, field: &str) -> Result<u64, MusubiArchiveRuntimeErrorV1> {
    map.get(field)
        .and_then(norito::json::Value::as_u64)
        .ok_or_else(|| integrity("MUSUBI_ARCHIVE_RESPONSE_INVALID"))
}
fn required_usize(
    map: &norito::json::Map,
    field: &str,
) -> Result<usize, MusubiArchiveRuntimeErrorV1> {
    let value = required_u64(map, field)?;
    usize::try_from(value).map_err(|_| integrity("MUSUBI_ARCHIVE_RESPONSE_INVALID"))
}
fn required_bool(
    map: &norito::json::Map,
    field: &str,
) -> Result<bool, MusubiArchiveRuntimeErrorV1> {
    map.get(field)
        .and_then(norito::json::Value::as_bool)
        .ok_or_else(|| integrity("MUSUBI_ARCHIVE_RESPONSE_INVALID"))
}
const fn retryable(code: &'static str) -> MusubiArchiveRuntimeErrorV1 {
    MusubiArchiveRuntimeErrorV1::new(MusubiArchiveRuntimeFailureClassV1::Retryable, code, None)
}
const fn integrity(code: &'static str) -> MusubiArchiveRuntimeErrorV1 {
    surfaced_integrity(
        code,
        MusubiArchiveRuntimeIntegritySurfaceV1::ArchiveCommitment,
    )
}
const fn surfaced_integrity(
    code: &'static str,
    surface: MusubiArchiveRuntimeIntegritySurfaceV1,
) -> MusubiArchiveRuntimeErrorV1 {
    MusubiArchiveRuntimeErrorV1::new(
        MusubiArchiveRuntimeFailureClassV1::Integrity,
        code,
        Some(surface),
    )
}
const fn control_integrity(code: &'static str) -> MusubiArchiveRuntimeErrorV1 {
    surfaced_integrity(code, MusubiArchiveRuntimeIntegritySurfaceV1::Other)
}
const fn unavailable(code: &'static str) -> MusubiArchiveRuntimeErrorV1 {
    MusubiArchiveRuntimeErrorV1::new(MusubiArchiveRuntimeFailureClassV1::Unavailable, code, None)
}
const fn permanent(code: &'static str) -> MusubiArchiveRuntimeErrorV1 {
    MusubiArchiveRuntimeErrorV1::new(MusubiArchiveRuntimeFailureClassV1::Permanent, code, None)
}
#[cfg(test)]
mod tests {
    use super::*;
    const TEST_NETWORK_ID: &str =
        "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0";
    const OTHER_NETWORK_ID: &str =
        "hash:0E5751C026E543B2E8AB2EB06099DAA1D1E5DF47778F7787FAAB45CDF12FE3A9#6A22";
    const TEST_OPERATOR_PUBLIC_KEY: &str =
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03";
    #[test]
    fn exact_stream_token_decode_ignores_ambient_layout_flags() {
        let token = StreamTokenV1 {
            body: sorafs_manifest::StreamTokenBodyV1 {
                token_id: "01J3E4ZCMQ3GP2H3R5PSNF6Z7X".to_owned(),
                manifest_cid: vec![0x01, 0x55, 0x01],
                provider_id: [0xAA; 32],
                profile_handle: "sorafs.sf1@1.0.0".to_owned(),
                max_streams: 1,
                ttl_epoch: 2,
                rate_limit_bytes: 1,
                issued_at: 1,
                requests_per_minute: 1,
                token_pk_version: 1,
            },
            signature: vec![0x5A; 64],
        };
        let encoded = STANDARD.encode(
            norito::encode_canonical(&token).expect("encode canonical stream-token fixture"),
        );
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        assert_eq!(
            decode_stream_token_exact(&encoded).expect("decode under alternate ambient flags"),
            token
        );
    }
    #[test]
    fn operator_headers_bind_network_path_body_and_single_freshness_tuple() {
        let operator_key_pair = KeyPair::try_random().expect("operator key");
        let runtime = ProviderRuntimeV1 {
            provider: ProviderId::new([0x11; 32]),
            base_url: Url::parse("https://8.8.8.8/").expect("fixed provider URL"),
            operator_key_pair: operator_key_pair.clone(),
            http: HttpClient::builder()
                .redirect(RedirectPolicy::none())
                .build()
                .expect("local HTTP client"),
        };
        let network_id = TEST_NETWORK_ID
            .parse::<NetworkId>()
            .expect("fixed network identity");
        let url = runtime
            .base_url
            .join("v1/sorafs/storage/token")
            .expect("fixed token URL");
        let body = br#"{"manifest_id_hex":"11"}"#;
        let headers = operator_request_headers(&runtime, &network_id, &url, body)
            .expect("operator request headers");
        assert_eq!(
            headers.public_key,
            operator_key_pair
                .public_key()
                .try_to_multihash_string()
                .expect("canonical operator public key")
        );
        assert!(is_lower_hex(&headers.nonce, 32));
        assert!(
            headers
                .timestamp_ms
                .bytes()
                .all(|byte| byte.is_ascii_digit())
        );
        assert!(!headers.timestamp_ms.starts_with('0') || headers.timestamp_ms == "0");
        let timestamp_ms = headers
            .timestamp_ms
            .parse::<u64>()
            .expect("operator timestamp");
        let signature_bytes = STANDARD
            .decode(headers.signature_b64.as_bytes())
            .expect("operator signature base64");
        let signature = Signature::try_from_bytes(&signature_bytes)
            .expect("checked operator signature payload");
        let exact_message = Client::operator_network_request_message(
            &network_id,
            &crate::http::Method::POST,
            &url,
            body,
            timestamp_ms,
            &headers.nonce,
        )
        .expect("bounded exact operator message");
        signature
            .verify(operator_key_pair.public_key(), &exact_message)
            .expect("signature must bind the exact request");
        let other_network = OTHER_NETWORK_ID
            .parse::<NetworkId>()
            .expect("fixed foreign network identity");
        let other_path = runtime
            .base_url
            .join("v1/sorafs/storage/plan")
            .expect("fixed foreign path");
        for altered_message in [
            Client::operator_network_request_message(
                &other_network,
                &crate::http::Method::POST,
                &url,
                body,
                timestamp_ms,
                &headers.nonce,
            )
            .expect("bounded foreign-network operator message"),
            Client::operator_network_request_message(
                &network_id,
                &crate::http::Method::POST,
                &other_path,
                body,
                timestamp_ms,
                &headers.nonce,
            )
            .expect("bounded altered-path operator message"),
            Client::operator_network_request_message(
                &network_id,
                &crate::http::Method::POST,
                &url,
                br#"{"manifest_id_hex":"22"}"#,
                timestamp_ms,
                &headers.nonce,
            )
            .expect("bounded altered-body operator message"),
            Client::operator_network_request_message(
                &network_id,
                &crate::http::Method::POST,
                &url,
                body,
                timestamp_ms,
                "replayed-with-another-nonce",
            )
            .expect("bounded altered-nonce operator message"),
        ] {
            assert!(
                signature
                    .verify(operator_key_pair.public_key(), &altered_message)
                    .is_err(),
                "an altered network, path, body, or freshness tuple must fail"
            );
        }
        let fresh = operator_request_headers(&runtime, &network_id, &url, body)
            .expect("fresh operator request headers");
        assert_ne!(
            headers.nonce, fresh.nonce,
            "every dispatch needs a fresh nonce"
        );
    }
    #[test]
    fn operator_fetch_parser_ignores_unrelated_account_key_material() {
        let document = parse_config_document(&format!(
            r#"
torii_url = "https://registry.example/"

[account]
public_key = "deliberately-not-a-key"
private_key = "deliberately-not-a-key"

[musubi.fetch]
network_id = "{TEST_NETWORK_ID}"
client_id = "musubi-ci"
request_timeout_ms = 2500

[[musubi.fetch.provider_gateways]]
provider_id = "1111111111111111111111111111111111111111111111111111111111111111"
url = "https://provider.example/"
operator_public_key = "{TEST_OPERATOR_PUBLIC_KEY}"
operator_private_key_file = "provider.key"
"#,
        ))
        .expect("fixture TOML");
        let parsed = parse_fetch_subtree(&document).expect("public fetch subtree");
        assert_eq!(parsed.client_id.as_deref(), Some("musubi-ci"));
        assert_eq!(parsed.request_timeout_ms, Some(2_500));
        assert_eq!(
            parsed.network_id,
            Some(
                TEST_NETWORK_ID
                    .parse()
                    .expect("fixed canonical network identity")
            )
        );
        assert_eq!(parsed.provider_gateways.len(), 1);
        assert_eq!(
            parsed.provider_gateways[0].operator_private_key_file,
            "provider.key"
        );
    }
    #[test]
    fn prepared_fetch_config_uses_one_image_and_defers_runtime_operator_key_and_network() {
        let image = format!(
            r#"
[account]
private_key = "ignored-private-material"

[musubi.fetch]
network_id = "{TEST_NETWORK_ID}"
client_id = "private-client-label"
request_timeout_ms = 2500

[[musubi.fetch.provider_gateways]]
provider_id = "1111111111111111111111111111111111111111111111111111111111111111"
url = "https://provider.example/"
operator_public_key = "{TEST_OPERATOR_PUBLIC_KEY}"
operator_private_key_file = "keys/provider.key"
"#,
        );
        let platform_root = PathBuf::from("prepared-fetch-platform");
        let config_path = platform_root.join("client.toml");
        let prepared = PreparedMusubiArchiveFetchConfigV1::from_platform_config_bytes(
            &config_path,
            image.as_bytes(),
        )
        .expect("preparation must not open the absent operator key or resolve provider DNS");
        assert_eq!(prepared.providers.len(), 1);
        assert_eq!(
            prepared.providers[0].operator_private_key_path,
            std::env::current_dir()
                .expect("current directory")
                .join(&platform_root)
                .join("keys/provider.key")
        );
        let debug = format!("{prepared:?}");
        assert!(debug.contains("provider_count: 1"));
        let platform_root_text = platform_root.to_string_lossy();
        for redacted in [
            "ignored-private-material",
            "private-client-label",
            TEST_NETWORK_ID,
            "provider.example",
            "provider.key",
            platform_root_text.as_ref(),
        ] {
            assert!(!debug.contains(redacted));
        }
    }
    #[cfg(unix)]
    #[test]
    fn platform_loader_uses_only_the_fetch_operator_identity() {
        use std::os::unix::fs::PermissionsExt as _;
        let temporary = tempfile::tempdir().expect("temporary platform directory");
        let operator = KeyPair::try_random().expect("operator key");
        let key_path = temporary.path().join("provider.key");
        fs::write(
            &key_path,
            format!("{}\n", ExposedPrivateKey(operator.private_key().clone())),
        )
        .expect("write operator key");
        fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600))
            .expect("secure operator key");
        let config_path = temporary.path().join("client.toml");
        fs::write(
            &config_path,
            format!(
                r#"
[account]
public_key = "deliberately-not-a-key"
private_key = "deliberately-not-a-key"

[musubi.fetch]
network_id = "{TEST_NETWORK_ID}"
client_id = "musubi-ci"

[[musubi.fetch.provider_gateways]]
provider_id = "1111111111111111111111111111111111111111111111111111111111111111"
url = "https://8.8.8.8/"
operator_public_key = "{}"
operator_private_key_file = "provider.key"
"#,
                operator.public_key(),
            ),
        )
        .expect("write operator-authenticated platform config");
        let client = AuthenticatedMusubiArchiveFetchClientV1::load_platform_file(&config_path)
            .expect("invalid account keys must be irrelevant to fetch configuration");
        let debug = format!("{client:?}");
        assert!(debug.contains("provider_count: 1"));
        assert!(!debug.contains(&operator.public_key().to_string()));
        assert!(!debug.contains(&ExposedPrivateKey(operator.private_key().clone()).to_string()));
    }
    #[test]
    fn fetch_parser_rejects_missing_network_and_all_legacy_bearer_fields() {
        for source in [
            format!(
                r#"
[musubi.fetch]
network_id = "{TEST_NETWORK_ID}"
client_id = "musubi-ci"
bearer_token = "must-not-be-inline"
provider_gateways = []
"#,
            ),
            format!(
                r#"
[musubi.fetch]
network_id = "{TEST_NETWORK_ID}"
client_id = "musubi-ci"

[[musubi.fetch.provider_gateways]]
provider_id = "1111111111111111111111111111111111111111111111111111111111111111"
url = "https://provider.example/"
api_token_file = "provider.token"
operator_public_key = "{TEST_OPERATOR_PUBLIC_KEY}"
operator_private_key_file = "provider.key"
"#,
            ),
            format!(
                r#"
[musubi.fetch]
network_id = "{TEST_NETWORK_ID}"
client_id = "musubi-ci"

[[musubi.fetch.provider_gateways]]
provider_id = "1111111111111111111111111111111111111111111111111111111111111111"
url = "https://provider.example/"
token = "must-not-be-inline"
operator_public_key = "{TEST_OPERATOR_PUBLIC_KEY}"
operator_private_key_file = "provider.key"
"#,
            ),
            format!(
                r#"
[musubi.fetch]
client_id = "musubi-ci"

[[musubi.fetch.provider_gateways]]
provider_id = "1111111111111111111111111111111111111111111111111111111111111111"
url = "https://provider.example/"
operator_public_key = "{TEST_OPERATOR_PUBLIC_KEY}"
operator_private_key_file = "provider.key"
"#,
            ),
        ] {
            let value = parse_config_document(&source).expect("fixture TOML");
            assert!(parse_fetch_subtree(&value).is_err());
        }
    }
    #[test]
    fn prepared_fetch_rejects_noncanonical_operator_public_key_text() {
        let mut config = MusubiFetchConfig {
            network_id: Some(
                TEST_NETWORK_ID
                    .parse()
                    .expect("fixed canonical network identity"),
            ),
            client_id: None,
            request_timeout_ms: None,
            provider_gateways: vec![MusubiFetchProviderGatewayConfig {
                provider_id: "11".repeat(32),
                url: "https://provider.example/".to_owned(),
                operator_public_key: TEST_OPERATOR_PUBLIC_KEY.to_owned(),
                operator_private_key_file: "provider.key".to_owned(),
            }],
        };
        config.provider_gateways[0].operator_public_key = config.provider_gateways[0]
            .operator_public_key
            .to_ascii_lowercase();
        let error = PreparedMusubiArchiveFetchConfigV1::from_platform_config(
            Path::new("client.toml"),
            &config,
        )
        .expect_err("operator key aliases must not be normalized");
        assert_eq!(error.code(), "MUSUBI_ARCHIVE_FETCH_OPERATOR_KEY_INVALID");
    }
    #[test]
    fn gateway_urls_reject_redirect_and_dns_attack_primitives() {
        for invalid in [
            "http://provider.example/",
            "https://user:secret@provider.example/",
            "https://provider.example/path",
            "https://provider.example/?token=secret",
            "https://provider.example/#fragment",
            "https://provider.example:444/",
            "https://127.0.0.1/",
            "https://10.0.0.1/",
            " https://provider.example/",
        ] {
            assert!(
                parse_gateway_base_url(invalid).is_err(),
                "accepted unsafe URL {invalid}"
            );
        }
        assert!(parse_gateway_base_url("https://8.8.8.8/").is_ok());
    }
    #[cfg(unix)]
    #[test]
    fn operator_key_file_is_private_bounded_matching_and_redacted() {
        use std::os::unix::fs::{PermissionsExt as _, symlink};
        let temporary = tempfile::tempdir().expect("temporary directory");
        let operator = KeyPair::try_random().expect("operator key");
        let foreign = KeyPair::try_random().expect("foreign key");
        let path = temporary.path().join("provider.key");
        let encoded = ExposedPrivateKey(operator.private_key().clone()).to_string();
        fs::write(&path, format!("{encoded}\n")).expect("write operator key");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("secure operator key");
        let loaded = read_operator_key_pair(&path, operator.public_key())
            .expect("private operator key file");
        assert_eq!(loaded.public_key(), operator.public_key());
        let mismatch = read_operator_key_pair(&path, foreign.public_key())
            .expect_err("foreign operator key must fail before dispatch");
        assert_eq!(
            mismatch.code(),
            "MUSUBI_ARCHIVE_FETCH_OPERATOR_KEY_MISMATCH"
        );
        assert!(!format!("{mismatch:?}").contains(&encoded));
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("weaken key");
        let error = read_operator_key_pair(&path, operator.public_key())
            .expect_err("world-readable operator key must fail");
        assert_eq!(
            error.code(),
            "MUSUBI_ARCHIVE_FETCH_OPERATOR_KEY_FILE_PERMISSIONS"
        );
        assert!(!format!("{error:?}").contains(&encoded));
        fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).expect("make key read-only");
        let error = read_operator_key_pair(&path, operator.public_key())
            .expect_err("operator keys require the exact 0600 mode");
        assert_eq!(
            error.code(),
            "MUSUBI_ARCHIVE_FETCH_OPERATOR_KEY_FILE_PERMISSIONS"
        );
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("restore key mode");
        let symlink_path = temporary.path().join("provider-symlink.key");
        symlink(&path, &symlink_path).expect("create hostile key symlink");
        assert!(
            read_operator_key_pair(&symlink_path, operator.public_key()).is_err(),
            "operator key reads must never follow a replacement symlink"
        );
        let hardlink_path = temporary.path().join("provider-hardlink.key");
        fs::hard_link(&path, &hardlink_path).expect("create hostile key hard link");
        assert!(
            read_operator_key_pair(&path, operator.public_key()).is_err(),
            "multiply-linked operator key files must fail closed"
        );
    }
    #[test]
    fn gateway_stream_failures_keep_retry_and_integrity_classes() {
        let retryable_error = classify_gateway_fetch_error(&GatewayFetchError::UnexpectedStatus {
            provider: "redacted-provider".to_owned(),
            status: StatusCode::TOO_MANY_REQUESTS,
            body: None,
        });
        assert_eq!(
            retryable_error.class(),
            MusubiArchiveRuntimeFailureClassV1::Retryable
        );
        let integrity_error = classify_gateway_fetch_error(&GatewayFetchError::ResponseTooLarge {
            provider: "redacted-provider".to_owned(),
            limit: 4 * 1024 * 1024,
        });
        assert_eq!(
            integrity_error.class(),
            MusubiArchiveRuntimeFailureClassV1::Integrity
        );
        assert_eq!(
            integrity_error.integrity_surface(),
            Some(MusubiArchiveRuntimeIntegritySurfaceV1::ArchiveCommitment)
        );
        assert!(!format!("{integrity_error:?}").contains("redacted-provider"));
    }
    #[test]
    fn integrity_errors_bind_closed_metric_surface_without_string_classification() {
        assert_eq!(
            integrity("MUSUBI_ARCHIVE_TEST_COMMITMENT").integrity_surface(),
            Some(MusubiArchiveRuntimeIntegritySurfaceV1::ArchiveCommitment)
        );
        assert_eq!(
            control_integrity("MUSUBI_ARCHIVE_TEST_CONTROL").integrity_surface(),
            Some(MusubiArchiveRuntimeIntegritySurfaceV1::Other)
        );
        assert_eq!(
            retryable("MUSUBI_ARCHIVE_TEST_RETRY").integrity_surface(),
            None
        );
    }
    #[test]
    fn exact_plan_binding_rejects_file_plan_substitution() {
        let payload = [1_u8, 2, 3, 4];
        let payload_u64 = u64::try_from(payload.len()).expect("fixture length fits u64");
        let payload_u32 = u32::try_from(payload.len()).expect("fixture length fits u32");
        let mut plan = CarBuildPlan {
            chunk_profile: sorafs_car::chunker_registry::default_descriptor().profile,
            payload_digest: blake3::hash(&payload),
            content_length: payload_u64,
            chunks: vec![CarChunk {
                offset: 0,
                length: payload_u32,
                digest: *blake3::hash(&payload).as_bytes(),
                taikai_segment_hint: None,
            }],
            files: vec![FilePlan {
                path: vec!["Musubi.toml".to_owned()],
                first_chunk: 0,
                chunk_count: 1,
                size: payload_u64,
            }],
        };
        let original = exact_plan_binding(&plan).expect("bind canonical plan");
        plan.files[0].path[0] = "substituted.toml".to_owned();
        assert_ne!(
            original,
            exact_plan_binding(&plan).expect("bind substituted plan")
        );
    }
    #[test]
    fn stream_memory_preflight_counts_retained_vector_capacity() {
        let payload = [7_u8; 64];
        let mut plan = CarBuildPlan {
            chunk_profile: sorafs_car::chunker_registry::default_descriptor().profile,
            payload_digest: blake3::hash(&payload),
            content_length: payload.len() as u64,
            chunks: vec![CarChunk {
                offset: 0,
                length: payload.len() as u32,
                digest: *blake3::hash(&payload).as_bytes(),
                taikai_segment_hint: None,
            }],
            files: vec![FilePlan {
                path: vec!["source.ko".to_owned()],
                first_chunk: 0,
                chunk_count: 1,
                size: payload.len() as u64,
            }],
        };
        enforce_plan_memory_bound(&plan).expect("small exact plan fits stream envelope");
        let item_size = std::mem::size_of::<CarChunk>();
        let required_capacity = MAX_RETAINED_PLAN_HEAP_BYTES
            .div_ceil(item_size)
            .saturating_add(1);
        plan.chunks
            .reserve_exact(required_capacity.saturating_sub(plan.chunks.len()));
        let error = enforce_plan_memory_bound(&plan)
            .expect_err("overcapacity plan must fail before the worker clone");
        assert_eq!(error.code(), "MUSUBI_ARCHIVE_PLAN_MEMORY_LIMIT");
    }
}
