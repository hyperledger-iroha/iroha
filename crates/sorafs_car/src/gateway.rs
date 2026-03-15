//! HTTP gateway integration for the multi-source orchestrator.
//!
//! This module bridges the generic scheduling logic in [`multi_fetch`] with the
//! chunk-range endpoints served by Torii gateways. Callers provide the manifest
//! context together with per-provider connection details (base URL + stream
//! token) and receive a ready-to-use set of [`FetchProvider`] definitions plus
//! an async fetcher that issues chunk requests with the correct headers.

use std::{
    collections::HashMap,
    fmt,
    future::Future,
    num::NonZeroUsize,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use hex::FromHexError;
use norito::{
    decode_from_bytes,
    json::{self, Value},
};
use reqwest::{
    Client, StatusCode, Url,
    header::{HeaderMap, HeaderName, HeaderValue},
};
use sorafs_manifest::{ManifestV1, StreamTokenV1};
use thiserror::Error;

use crate::{
    moderation::{
        ModerationTokenError, ModerationTokenKey, ValidatedModerationProof,
        verify_token_for_context,
    },
    multi_fetch::{
        AttemptFailure, ChunkResponse, FetchOptions, FetchOutcome, FetchProvider, FetchRequest,
        MultiSourceError, PolicyBlockEvidence, ProviderMetadata, RangeCapability, StreamBudget,
        TransportHint,
    },
};

const HEADER_SORA_NONCE: &str = "x-sorafs-nonce";
const HEADER_SORA_CHUNKER: &str = "x-sorafs-chunker";
const HEADER_SORA_STREAM_TOKEN: &str = "x-sorafs-stream-token";
const HEADER_SORA_MANIFEST_ENVELOPE: &str = "x-sorafs-manifest-envelope";
const HEADER_SORA_CLIENT: &str = "x-sorafs-client";
const HEADER_SORA_REQ_BLINDED_CID: &str = "sora-req-blinded-cid";
const HEADER_SORA_REQ_SALT_EPOCH: &str = "sora-req-salt-epoch";
const HEADER_SORA_REQ_NONCE: &str = "sora-req-nonce";
const HEADER_SORA_MODERATION_TOKEN: &str = "sora-moderation-token";
const HEADER_SORA_DENYLIST_VERSION: &str = "sora-denylist-version";
const HEADER_SORA_CACHE_VERSION: &str = "sora-cache-version";

/// HTTP request issued by the gateway fetcher.
pub(crate) struct HttpRequest {
    pub url: Url,
    pub headers: HeaderMap,
}

/// HTTP response returned by the engine.
#[derive(Clone)]
pub(crate) struct HttpResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

/// Evidence returned by a gateway when a request is blocked by policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayFailureEvidence {
    /// Status code observed on the wire.
    pub observed_status: StatusCode,
    /// Canonicalised status for reporting/telemetry.
    pub canonical_status: StatusCode,
    /// Error code parsed from the response body (e.g., `denylisted`).
    pub code: Option<String>,
    /// Optional opaque moderation token for audit/appeal workflows.
    pub proof_token_b64: Option<String>,
    /// Parsed and verified moderation proof when a key was supplied.
    pub verified_proof: Option<ValidatedModerationProof>,
    /// Optional denylist version or digest advertised by the gateway.
    pub denylist_version: Option<String>,
    /// Optional cache version advertised by the gateway (falls back to the denylist version).
    pub cache_version: Option<String>,
    /// Human-readable message taken from the response body, when provided.
    pub message: Option<String>,
}

pub(crate) type HttpFuture = Pin<Box<dyn Future<Output = Result<HttpResponse, HttpError>> + Send>>;

/// Minimal async HTTP client abstraction used by the fetcher.
pub(crate) trait HttpEngine: Send + Sync {
    fn get(&self, request: HttpRequest) -> HttpFuture;
}

/// Errors surfaced by HTTP engines.
#[derive(Debug)]
#[allow(dead_code)]
pub(crate) enum HttpError {
    Transport(reqwest::Error),
    Body(reqwest::Error),
    Stub(String),
}

struct ReqwestEngine {
    client: Client,
}

impl ReqwestEngine {
    fn new(client: Client) -> Self {
        Self { client }
    }
}

impl HttpEngine for ReqwestEngine {
    fn get(&self, request: HttpRequest) -> HttpFuture {
        let client = self.client.clone();
        Box::pin(async move {
            let mut builder = client.get(request.url);
            builder = builder.headers(request.headers);
            let response = builder.send().await.map_err(HttpError::Transport)?;
            let status = response.status();
            let headers = response.headers().clone();
            let body = response.bytes().await.map_err(HttpError::Body)?.to_vec();
            Ok(HttpResponse {
                status,
                headers,
                body,
            })
        })
    }
}

/// Provider configuration required to contact a Torii gateway.
#[derive(Debug, Clone)]
pub struct GatewayProviderInput {
    /// Human-friendly alias used in orchestrator reports.
    pub name: String,
    /// Hex-encoded 32-byte provider identifier.
    pub provider_id_hex: String,
    /// Base URL for the provider's Torii gateway.
    ///
    /// The request paths required by this module are appended to the base URL,
    /// so callers may omit a trailing slash.
    pub base_url: String,
    /// Base64-encoded [`StreamTokenV1`] authorising chunk access.
    pub stream_token_b64: String,
    /// Optional admin endpoint exposing `/privacy/events` for relay telemetry.
    pub privacy_events_url: Option<String>,
}

/// Shared manifest context supplied to the gateway fetcher.
#[derive(Debug, Clone)]
pub struct GatewayFetchConfig {
    /// Hex-encoded manifest identifier used in the chunk endpoints.
    pub manifest_id_hex: String,
    /// Chunker handle advertised by the manifest (e.g. `sorafs.sf1@1.0.0`).
    pub chunker_handle: String,
    /// Optional governance envelope to satisfy gateway policy checks.
    pub manifest_envelope_b64: Option<String>,
    /// Optional client label for rate limiting / audit purposes.
    pub client_id: Option<String>,
    /// Optional manifest CID expectation (hex). When present the stream token
    /// must authorise the same CID.
    pub expected_manifest_cid_hex: Option<String>,
    /// Optional canonical blinded CID (base64url, no padding) passed via `Sora-Req-Blinded-CID`.
    pub blinded_cid_b64: Option<String>,
    /// Optional salt epoch associated with `blinded_cid_b64`.
    pub salt_epoch: Option<u32>,
    /// Optional cache version to enforce on gateway responses (falls back to denylist version when absent).
    pub expected_cache_version: Option<String>,
    /// Optional base64-encoded key used to verify opaque moderation proof tokens.
    pub moderation_token_key_b64: Option<String>,
}

impl GatewayFetchConfig {
    /// Normalise the manifest identifier for routing.
    fn manifest_id_normalised(&self) -> String {
        self.manifest_id_hex.trim().to_ascii_lowercase()
    }
}

/// Container bundling orchestrator providers with the HTTP fetcher.
#[derive(Clone)]
pub struct GatewayFetchContext {
    providers: Arc<[FetchProvider]>,
    fetcher: GatewayFetcher,
}

impl std::fmt::Debug for GatewayFetchContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GatewayFetchContext")
            .field("providers_len", &self.providers.len())
            .field("fetcher", &"<opaque>")
            .finish()
    }
}

impl GatewayFetchContext {
    /// Build a gateway fetch context from the supplied manifest and provider inputs.
    ///
    /// # Errors
    ///
    /// Returns [`GatewayBuildError`] when provider identifiers, stream tokens, or
    /// headers are malformed, or when the stream token metadata is inconsistent
    /// with the manifest context.
    pub fn new(
        config: GatewayFetchConfig,
        providers: impl IntoIterator<Item = GatewayProviderInput>,
    ) -> Result<Self, GatewayBuildError> {
        let client = Client::builder()
            .build()
            .map_err(GatewayBuildError::ClientBuild)?;
        Self::build_with_engine(config, providers, Arc::new(ReqwestEngine::new(client)))
    }

    pub(crate) fn build_with_engine(
        config: GatewayFetchConfig,
        providers: impl IntoIterator<Item = GatewayProviderInput>,
        engine: Arc<dyn HttpEngine>,
    ) -> Result<Self, GatewayBuildError> {
        let config = NormalisedConfig::from_config(config)?;
        let mut provider_map = HashMap::new();
        let mut fetch_providers = Vec::new();

        for input in providers {
            let descriptor = ProviderDescriptor::from_input(&config, input, &mut provider_map)?;
            fetch_providers.push(descriptor.provider.clone());
        }

        let fetcher = GatewayFetcher {
            inner: Arc::new(GatewayFetcherInner {
                manifest_id_hex: config.manifest_id.clone(),
                manifest_id_bytes: config.manifest_id_bytes,
                chunker_header: config.chunker_header.clone(),
                manifest_envelope: config.manifest_envelope.clone(),
                client_header: config.client_header.clone(),
                blinded_header: config.blinded_header.clone(),
                salt_epoch_header: config.salt_epoch_header.clone(),
                cache_version: config.cache_version.clone(),
                moderation_token_key: config.moderation_token_key.clone(),
                engine,
                providers: provider_map,
            }),
        };

        Ok(Self {
            providers: fetch_providers.into(),
            fetcher,
        })
    }

    /// Clone all provider descriptors for orchestrator scheduling.
    #[must_use]
    pub fn providers(&self) -> Vec<FetchProvider> {
        self.providers.to_vec()
    }

    /// Fetcher reference used to issue chunk requests.
    #[must_use]
    pub fn fetcher(&self) -> GatewayFetcher {
        self.fetcher.clone()
    }

    /// Convenience wrapper executing the orchestration end-to-end.
    pub async fn execute_plan(
        &self,
        plan: &crate::CarBuildPlan,
        options: FetchOptions,
    ) -> Result<FetchOutcome, MultiSourceError> {
        let fetcher = self.fetcher.clone();
        let providers = self.providers();
        crate::multi_fetch::fetch_plan_parallel(
            plan,
            providers,
            move |request| {
                let fetcher = fetcher.clone();
                async move { fetcher.fetch(request).await }
            },
            options,
        )
        .await
    }

    /// Fetch the manifest payload for this context using the configured providers.
    pub async fn fetch_manifest(&self) -> Result<GatewayFetchedManifest, GatewayManifestError> {
        self.fetcher.fetch_manifest().await
    }
}

/// Cloneable async fetcher issuing chunk requests to Torii gateways.
#[derive(Clone)]
pub struct GatewayFetcher {
    inner: Arc<GatewayFetcherInner>,
}

impl GatewayFetcher {
    /// Issue a single chunk request for the provided fetch metadata.
    pub async fn fetch(&self, request: FetchRequest) -> Result<ChunkResponse, GatewayFetchError> {
        self.inner.fetch(request).await
    }

    /// Expose the fetcher as a closure compatible with [`fetch_plan_parallel`].
    pub fn as_closure(
        &self,
    ) -> impl Fn(FetchRequest) -> GatewayFetchFuture + Send + Sync + Clone + 'static {
        let fetcher = self.clone();
        move |request| {
            let fetcher = fetcher.clone();
            Box::pin(async move { fetcher.fetch(request).await })
        }
    }

    /// Fetch the manifest payload associated with this context.
    pub async fn fetch_manifest(&self) -> Result<GatewayFetchedManifest, GatewayManifestError> {
        self.inner.fetch_manifest().await
    }
}

/// Future returned by the gateway fetcher closure.
pub type GatewayFetchFuture =
    Pin<Box<dyn Future<Output = Result<ChunkResponse, GatewayFetchError>> + Send>>;

struct GatewayFetcherInner {
    engine: Arc<dyn HttpEngine>,
    manifest_id_hex: String,
    manifest_id_bytes: [u8; 32],
    chunker_header: HeaderValue,
    manifest_envelope: Option<HeaderValue>,
    client_header: Option<HeaderValue>,
    blinded_header: Option<HeaderValue>,
    salt_epoch_header: Option<HeaderValue>,
    cache_version: Option<String>,
    moderation_token_key: Option<ModerationTokenKey>,
    providers: HashMap<String, Arc<ProviderRuntime>>,
}

impl GatewayFetcherInner {
    async fn fetch(&self, request: FetchRequest) -> Result<ChunkResponse, GatewayFetchError> {
        let provider_alias = request.provider.id().as_str().to_string();
        let provider = self
            .providers
            .get(&provider_alias)
            .ok_or_else(|| GatewayFetchError::UnknownProvider {
                provider: provider_alias.clone(),
            })?
            .clone();

        let digest_hex = hex::encode(request.spec.digest);
        let url = provider
            .base_url
            .join(&format!(
                "v1/sorafs/storage/chunk/{}/{}",
                self.manifest_id_hex, digest_hex
            ))
            .map_err(|source| GatewayFetchError::UrlJoin {
                provider: provider_alias.clone(),
                source,
            })?;

        let nonce = provider.next_nonce(request.spec.chunk_index);
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static(HEADER_SORA_CHUNKER),
            self.chunker_header.clone(),
        );
        headers.insert(
            HeaderName::from_static(HEADER_SORA_NONCE),
            header_value(&nonce, "X-SoraFS-Nonce"),
        );
        headers.insert(
            HeaderName::from_static(HEADER_SORA_STREAM_TOKEN),
            provider.stream_token.clone(),
        );
        if let Some(value) = &self.manifest_envelope {
            headers.insert(
                HeaderName::from_static(HEADER_SORA_MANIFEST_ENVELOPE),
                value.clone(),
            );
        }
        if let Some(value) = &self.client_header {
            headers.insert(HeaderName::from_static(HEADER_SORA_CLIENT), value.clone());
        }
        if let Some(value) = &self.blinded_header {
            headers.insert(
                HeaderName::from_static(HEADER_SORA_REQ_BLINDED_CID),
                value.clone(),
            );
            if let Some(epoch) = &self.salt_epoch_header {
                headers.insert(
                    HeaderName::from_static(HEADER_SORA_REQ_SALT_EPOCH),
                    epoch.clone(),
                );
            }
            headers.insert(
                HeaderName::from_static(HEADER_SORA_REQ_NONCE),
                header_value(&nonce, "Sora-Req-Nonce"),
            );
        }

        let response = self
            .engine
            .get(HttpRequest { url, headers })
            .await
            .map_err(|error| match error {
                HttpError::Transport(source) => GatewayFetchError::Request {
                    provider: provider_alias.clone(),
                    source,
                },
                HttpError::Body(source) => GatewayFetchError::RequestBody {
                    provider: provider_alias.clone(),
                    source,
                },
                HttpError::Stub(message) => GatewayFetchError::Stub {
                    provider: provider_alias.clone(),
                    message,
                },
            })?;

        let (_, cache_version) = observed_versions(&response.headers);
        if !response.status.is_success() {
            let proof_context = ModerationProofContext {
                manifest_id: &self.manifest_id_bytes,
                chunk_digest: Some(&request.spec.digest),
            };
            let evidence = extract_failure_evidence(
                &response,
                Some(proof_context),
                self.moderation_token_key.as_ref(),
            );
            if let Some(expected) = &self.cache_version
                && cache_version.as_deref() != Some(expected.as_str())
            {
                return Err(GatewayFetchError::CacheVersionMismatch {
                    provider: provider_alias.clone(),
                    expected: expected.clone(),
                    observed: cache_version.clone(),
                    status: response.status,
                    evidence,
                });
            }
            if let Some(evidence) = evidence {
                return Err(GatewayFetchError::PolicyBlocked {
                    provider: provider_alias,
                    evidence,
                });
            }
            let body = if response.body.is_empty() {
                None
            } else {
                Some(truncate(&String::from_utf8_lossy(&response.body), 512))
            };
            return Err(GatewayFetchError::UnexpectedStatus {
                provider: provider_alias,
                status: response.status,
                body,
            });
        }
        if let Some(expected) = &self.cache_version
            && cache_version.as_deref() != Some(expected.as_str())
        {
            return Err(GatewayFetchError::CacheVersionMismatch {
                provider: provider_alias,
                expected: expected.clone(),
                observed: cache_version,
                status: response.status,
                evidence: None,
            });
        }

        Ok(ChunkResponse::new(response.body))
    }

    async fn fetch_manifest(&self) -> Result<GatewayFetchedManifest, GatewayManifestError> {
        if self.providers.is_empty() {
            return Err(GatewayManifestError::NoProviders);
        }

        let manifest_path = format!("v1/sorafs/storage/manifest/{}", self.manifest_id_hex);

        let mut last_error: Option<GatewayManifestError> = None;

        for (alias, runtime) in &self.providers {
            let url = match runtime.base_url.join(&manifest_path) {
                Ok(url) => url,
                Err(err) => {
                    last_error = Some(GatewayManifestError::Request {
                        provider: alias.clone(),
                        error: format!("failed to join manifest URL: {err}"),
                    });
                    continue;
                }
            };

            let mut headers = HeaderMap::new();
            if let Some(envelope) = &self.manifest_envelope {
                headers.insert(
                    HeaderName::from_static(HEADER_SORA_MANIFEST_ENVELOPE),
                    envelope.clone(),
                );
            }
            if let Some(client) = &self.client_header {
                headers.insert(HeaderName::from_static(HEADER_SORA_CLIENT), client.clone());
            }
            if let Some(blinded) = &self.blinded_header {
                headers.insert(
                    HeaderName::from_static(HEADER_SORA_REQ_BLINDED_CID),
                    blinded.clone(),
                );
                if let Some(epoch) = &self.salt_epoch_header {
                    headers.insert(
                        HeaderName::from_static(HEADER_SORA_REQ_SALT_EPOCH),
                        epoch.clone(),
                    );
                }
            }

            let response = match self.engine.get(HttpRequest { url, headers }).await {
                Ok(response) => response,
                Err(err) => {
                    let error = match err {
                        HttpError::Transport(source) => format!("request failed: {source}"),
                        HttpError::Body(source) => format!("body read failed: {source}"),
                        HttpError::Stub(message) => message,
                    };
                    last_error = Some(GatewayManifestError::Request {
                        provider: alias.clone(),
                        error,
                    });
                    continue;
                }
            };

            let proof_context = ModerationProofContext {
                manifest_id: &self.manifest_id_bytes,
                chunk_digest: None,
            };
            let (_, cache_version) = observed_versions(&response.headers);

            if !response.status.is_success() {
                let evidence = extract_failure_evidence(
                    &response,
                    Some(proof_context),
                    self.moderation_token_key.as_ref(),
                );
                if let Some(expected) = &self.cache_version
                    && cache_version.as_deref() != Some(expected.as_str())
                {
                    last_error = Some(GatewayManifestError::CacheVersionMismatch {
                        provider: alias.clone(),
                        expected: expected.clone(),
                        observed: cache_version.clone(),
                        status: response.status,
                    });
                    continue;
                }
                if let Some(evidence) = evidence {
                    let mut detail = format!(
                        "gateway blocked request (status={}, code={:?}",
                        evidence.canonical_status, evidence.code
                    );
                    if evidence.proof_token_b64.is_some() {
                        detail.push_str(", proof_token=present");
                    }
                    if let Some(denylist) = &evidence.denylist_version {
                        detail.push_str(&format!(", denylist={denylist}"));
                    }
                    if let Some(message) = &evidence.message {
                        detail.push_str(&format!(", message={message}"));
                    }
                    detail.push(')');

                    last_error = Some(GatewayManifestError::Status {
                        provider: alias.clone(),
                        status: evidence.canonical_status,
                        body: Some(detail),
                    });
                    continue;
                }
                let body = if response.body.is_empty() {
                    None
                } else {
                    Some(truncate(&String::from_utf8_lossy(&response.body), 512))
                };
                last_error = Some(GatewayManifestError::Status {
                    provider: alias.clone(),
                    status: response.status,
                    body,
                });
                continue;
            }

            if let Some(expected) = &self.cache_version
                && cache_version.as_deref() != Some(expected.as_str())
            {
                last_error = Some(GatewayManifestError::CacheVersionMismatch {
                    provider: alias.clone(),
                    expected: expected.clone(),
                    observed: cache_version.clone(),
                    status: response.status,
                });
                continue;
            }

            match parse_manifest_response(alias, &response.body, cache_version.clone()) {
                Ok(manifest) => return Ok(manifest),
                Err(err) => last_error = Some(err),
            }
        }

        Err(last_error.unwrap_or(GatewayManifestError::NoProviders))
    }
}

fn header_value(value: impl AsRef<str>, name: &str) -> HeaderValue {
    HeaderValue::from_str(value.as_ref())
        .unwrap_or_else(|_| panic!("{name} header produced invalid value: {}", value.as_ref()))
}

fn truncate(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_string();
    }
    let mut truncated = text.chars().take(max).collect::<String>();
    truncated.push('…');
    truncated
}

#[derive(Clone, Copy)]
struct ModerationProofContext<'a> {
    manifest_id: &'a [u8; 32],
    chunk_digest: Option<&'a [u8; 32]>,
}

fn observed_versions(headers: &HeaderMap) -> (Option<String>, Option<String>) {
    let denylist_version = headers
        .get(HEADER_SORA_DENYLIST_VERSION)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    let cache_header = headers
        .get(HEADER_SORA_CACHE_VERSION)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    let cache_version = cache_header.or_else(|| denylist_version.clone());
    (denylist_version, cache_version)
}

fn extract_failure_evidence(
    response: &HttpResponse,
    proof_context: Option<ModerationProofContext<'_>>,
    proof_key: Option<&ModerationTokenKey>,
) -> Option<GatewayFailureEvidence> {
    let proof_token = response
        .headers
        .get(HEADER_SORA_MODERATION_TOKEN)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    let (denylist_version, cache_version) = observed_versions(&response.headers);
    let (code, message) = parse_failure_body(&response.body);
    let canonical_status = canonical_status(code.as_deref(), response.status);
    let verified_proof = match (proof_key, proof_token.as_deref(), proof_context) {
        (Some(key), Some(token), Some(ctx)) => {
            match (cache_version.as_deref(), denylist_version.as_deref()) {
                (Some(cache), Some(denylist)) => verify_token_for_context(
                    token,
                    key,
                    ctx.manifest_id,
                    ctx.chunk_digest,
                    denylist,
                    cache,
                )
                .ok(),
                _ => None,
            }
        }
        _ => None,
    };

    if proof_token.is_some()
        || denylist_version.is_some()
        || matches!(response.status, StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS)
        || code.is_some()
    {
        return Some(GatewayFailureEvidence {
            observed_status: response.status,
            canonical_status,
            code,
            proof_token_b64: proof_token,
            denylist_version,
            cache_version,
            message,
            verified_proof,
        });
    }
    None
}

fn canonical_status(code: Option<&str>, observed: StatusCode) -> StatusCode {
    match code {
        Some("denylisted") => StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS,
        _ => observed,
    }
}

fn parse_failure_body(bytes: &[u8]) -> (Option<String>, Option<String>) {
    if bytes.is_empty() {
        return (None, None);
    }
    if let Ok(value) = json::from_slice::<Value>(bytes) {
        let code = value
            .get("error")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let message = value
            .get("message")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .or_else(|| value.as_str().map(ToOwned::to_owned));
        return (code, message);
    }

    (None, Some(truncate(&String::from_utf8_lossy(bytes), 512)))
}

#[derive(Debug)]
struct ProviderRuntime {
    base_url: Url,
    stream_token: HeaderValue,
    token_id: String,
    nonce: AtomicU64,
    _privacy_events_url: Option<Url>,
}

impl ProviderRuntime {
    fn next_nonce(&self, chunk_index: usize) -> String {
        let counter = self.nonce.fetch_add(1, Ordering::Relaxed);
        format!("{}-{chunk_index}-{counter}", self.token_id)
    }
}

#[derive(Debug)]
struct ProviderDescriptor {
    provider: FetchProvider,
}

#[derive(Debug)]
struct NormalisedConfig {
    manifest_id: String,
    manifest_id_bytes: [u8; 32],
    chunker_handle: String,
    chunker_header: HeaderValue,
    manifest_envelope: Option<HeaderValue>,
    client_header: Option<HeaderValue>,
    expected_manifest_cid_hex: Option<String>,
    blinded_header: Option<HeaderValue>,
    salt_epoch_header: Option<HeaderValue>,
    cache_version: Option<String>,
    moderation_token_key: Option<ModerationTokenKey>,
}

impl NormalisedConfig {
    fn from_config(config: GatewayFetchConfig) -> Result<Self, GatewayBuildError> {
        let manifest_id = config.manifest_id_normalised();
        if manifest_id.len() != 64 || !manifest_id.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(GatewayBuildError::InvalidManifestId { manifest_id });
        }
        let manifest_bytes =
            hex::decode(&manifest_id).map_err(|_| GatewayBuildError::InvalidManifestId {
                manifest_id: manifest_id.clone(),
            })?;
        let manifest_id_bytes: [u8; 32] =
            manifest_bytes
                .try_into()
                .map_err(|_| GatewayBuildError::InvalidManifestId {
                    manifest_id: manifest_id.clone(),
                })?;

        let GatewayFetchConfig {
            manifest_id_hex: _,
            chunker_handle,
            manifest_envelope_b64,
            client_id,
            expected_manifest_cid_hex,
            blinded_cid_b64,
            salt_epoch,
            expected_cache_version,
            moderation_token_key_b64,
        } = config;

        let chunker_handle = chunker_handle.trim().to_string();
        if chunker_handle.is_empty() {
            return Err(GatewayBuildError::EmptyChunkerHandle);
        }

        let chunker_header = HeaderValue::from_str(&chunker_handle).map_err(|_| {
            GatewayBuildError::InvalidHeader {
                header: HEADER_SORA_CHUNKER,
                reason: "chunker handle contains invalid ASCII",
            }
        })?;

        let manifest_envelope = if let Some(value) = manifest_envelope_b64 {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Err(GatewayBuildError::InvalidHeader {
                    header: HEADER_SORA_MANIFEST_ENVELOPE,
                    reason: "manifest envelope must not be empty",
                });
            }
            let decoded = STANDARD.decode(trimmed.as_bytes()).map_err(|_| {
                GatewayBuildError::InvalidHeader {
                    header: HEADER_SORA_MANIFEST_ENVELOPE,
                    reason: "manifest envelope must contain valid base64",
                }
            })?;
            if decoded.is_empty() {
                return Err(GatewayBuildError::InvalidHeader {
                    header: HEADER_SORA_MANIFEST_ENVELOPE,
                    reason: "manifest envelope must not be empty",
                });
            }
            Some(
                HeaderValue::from_str(trimmed).map_err(|_| GatewayBuildError::InvalidHeader {
                    header: HEADER_SORA_MANIFEST_ENVELOPE,
                    reason: "manifest envelope must contain valid ASCII",
                })?,
            )
        } else {
            None
        };

        let client_header = if let Some(id) = client_id {
            let trimmed = id.trim();
            if trimmed.is_empty() {
                return Err(GatewayBuildError::InvalidHeader {
                    header: HEADER_SORA_CLIENT,
                    reason: "client identifier must not be empty",
                });
            }
            Some(
                HeaderValue::from_str(trimmed).map_err(|_| GatewayBuildError::InvalidHeader {
                    header: HEADER_SORA_CLIENT,
                    reason: "client identifier must contain valid ASCII",
                })?,
            )
        } else {
            None
        };

        let expected_manifest_cid_hex =
            expected_manifest_cid_hex.map(|cid| cid.trim().to_ascii_lowercase());

        let (blinded_header, salt_epoch_header) = match (blinded_cid_b64, salt_epoch) {
            (Some(blinded), Some(epoch)) => {
                let trimmed = blinded.trim();
                if trimmed.is_empty() {
                    return Err(GatewayBuildError::InvalidHeader {
                        header: HEADER_SORA_REQ_BLINDED_CID,
                        reason: "value must not be empty",
                    });
                }
                let decoded = URL_SAFE_NO_PAD.decode(trimmed.as_bytes()).map_err(|_| {
                    GatewayBuildError::InvalidHeader {
                        header: HEADER_SORA_REQ_BLINDED_CID,
                        reason: "value must be URL-safe base64 without padding",
                    }
                })?;
                if decoded.len() != 32 {
                    return Err(GatewayBuildError::InvalidHeader {
                        header: HEADER_SORA_REQ_BLINDED_CID,
                        reason: "decoded value must be 32 bytes",
                    });
                }
                let header = HeaderValue::from_str(trimmed).map_err(|_| {
                    GatewayBuildError::InvalidHeader {
                        header: HEADER_SORA_REQ_BLINDED_CID,
                        reason: "value must contain valid ASCII",
                    }
                })?;
                let epoch_string = epoch.to_string();
                let epoch_header = HeaderValue::from_str(&epoch_string).map_err(|_| {
                    GatewayBuildError::InvalidHeader {
                        header: HEADER_SORA_REQ_SALT_EPOCH,
                        reason: "epoch must be ASCII digits",
                    }
                })?;
                (Some(header), Some(epoch_header))
            }
            (Some(_), None) => return Err(GatewayBuildError::MissingSaltEpoch),
            (None, Some(_)) => return Err(GatewayBuildError::SaltEpochWithoutBlindedCid),
            (None, None) => (None, None),
        };

        let cache_version = if let Some(version) = expected_cache_version {
            let trimmed = version.trim();
            if trimmed.is_empty() {
                return Err(GatewayBuildError::InvalidHeader {
                    header: HEADER_SORA_CACHE_VERSION,
                    reason: "expected cache version must not be empty",
                });
            }
            Some(trimmed.to_string())
        } else {
            None
        };

        let moderation_token_key = if let Some(value) = moderation_token_key_b64 {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Err(GatewayBuildError::InvalidModerationTokenKey {
                    source: ModerationTokenError::EmptyField("moderation token key"),
                });
            }
            Some(
                ModerationTokenKey::from_base64(trimmed)
                    .map_err(|source| GatewayBuildError::InvalidModerationTokenKey { source })?,
            )
        } else {
            None
        };

        Ok(Self {
            manifest_id,
            manifest_id_bytes,
            chunker_handle,
            chunker_header,
            manifest_envelope,
            client_header,
            expected_manifest_cid_hex,
            blinded_header,
            salt_epoch_header,
            cache_version,
            moderation_token_key,
        })
    }
}

impl ProviderDescriptor {
    fn from_input(
        config: &NormalisedConfig,
        input: GatewayProviderInput,
        providers: &mut HashMap<String, Arc<ProviderRuntime>>,
    ) -> Result<Self, GatewayBuildError> {
        if providers.contains_key(&input.name) {
            return Err(GatewayBuildError::DuplicateProvider {
                provider_id: input.name.clone(),
            });
        }
        let provider_id_hex = input.provider_id_hex.trim().to_ascii_lowercase();
        let provider_id = decode_provider_id(&provider_id_hex).map_err(|_| {
            GatewayBuildError::InvalidProviderId {
                provider_id: provider_id_hex.clone(),
            }
        })?;

        let base_url = parse_base_url(&input.base_url).map_err(|source| {
            GatewayBuildError::InvalidBaseUrl {
                provider_id: provider_id_hex.clone(),
                source,
            }
        })?;

        let privacy_events_url = match input.privacy_events_url.as_ref() {
            Some(raw) => {
                let trimmed = raw.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(parse_privacy_url(trimmed).map_err(|source| {
                        GatewayBuildError::InvalidPrivacyUrl {
                            provider_id: provider_id_hex.clone(),
                            source,
                        }
                    })?)
                }
            }
            None => None,
        };

        let token = decode_stream_token(&input.stream_token_b64).map_err(|source| {
            GatewayBuildError::InvalidStreamToken {
                provider_id: provider_id_hex.clone(),
                source,
            }
        })?;

        if token.body.provider_id != provider_id {
            return Err(GatewayBuildError::ProviderIdMismatch {
                provider_id: provider_id_hex.clone(),
                token_provider_id: hex::encode(token.body.provider_id),
            });
        }

        if token.body.profile_handle.trim() != config.chunker_handle {
            return Err(GatewayBuildError::ProfileMismatch {
                provider_id: provider_id_hex.clone(),
                token_profile: token.body.profile_handle.clone(),
                expected: config.chunker_handle.clone(),
            });
        }

        if let Some(expected_cid) = &config.expected_manifest_cid_hex {
            let token_cid = hex::encode(token.body.manifest_cid.as_slice());
            if token_cid != *expected_cid {
                return Err(GatewayBuildError::ManifestCidMismatch {
                    provider_id: provider_id_hex.clone(),
                    expected: expected_cid.clone(),
                    actual: token_cid,
                });
            }
        }

        let max_streams = usize::from(token.body.max_streams);
        let capacity = NonZeroUsize::new(max_streams).ok_or_else(|| {
            GatewayBuildError::ZeroStreamCapacity {
                provider_id: provider_id_hex.clone(),
            }
        })?;

        let mut metadata = ProviderMetadata::new();
        metadata.provider_id = Some(provider_id_hex.clone());
        metadata.profile_id = Some(config.chunker_handle.clone());
        metadata.max_streams = Some(token.body.max_streams);
        metadata
            .capability_names
            .push("chunk_range_fetch".to_string());
        metadata.transport_hints.push(TransportHint {
            protocol: "torii-http-range".to_string(),
            protocol_id: 1,
            priority: 0,
        });
        metadata.privacy_events_url = privacy_events_url.as_ref().map(ToString::to_string);
        metadata.stream_budget = Some(StreamBudget {
            max_in_flight: token.body.max_streams,
            max_bytes_per_sec: token.body.rate_limit_bytes,
            burst_bytes: None,
        });
        metadata.range_capability = Some(RangeCapability {
            max_chunk_span: u32::MAX,
            min_granularity: 1,
            supports_sparse_offsets: true,
            requires_alignment: false,
            supports_merkle_proof: true,
        });
        metadata.profile_aliases.push(input.name.clone());

        let provider = FetchProvider::new(input.name.clone())
            .with_max_concurrent_chunks(capacity)
            .with_metadata(metadata);

        let runtime = ProviderRuntime {
            base_url,
            stream_token: HeaderValue::from_str(input.stream_token_b64.trim()).map_err(|_| {
                GatewayBuildError::InvalidHeader {
                    header: HEADER_SORA_STREAM_TOKEN,
                    reason: "stream token must contain valid ASCII",
                }
            })?,
            token_id: token.body.token_id.clone(),
            nonce: AtomicU64::new(0),
            _privacy_events_url: privacy_events_url,
        };

        providers.insert(input.name, Arc::new(runtime));

        Ok(Self { provider })
    }
}

fn decode_provider_id(value: &str) -> Result<[u8; 32], FromHexError> {
    let mut bytes = [0u8; 32];
    let decoded = hex::decode(value)?;
    bytes.copy_from_slice(&decoded);
    Ok(bytes)
}

fn parse_base_url(value: &str) -> Result<Url, url::ParseError> {
    let trimmed = value.trim();
    if trimmed.ends_with('/') {
        Url::parse(trimmed)
    } else {
        let mut with_slash = trimmed.to_string();
        with_slash.push('/');
        Url::parse(&with_slash)
    }
}

fn parse_privacy_url(value: &str) -> Result<Url, url::ParseError> {
    Url::parse(value)
}

fn decode_stream_token(value: &str) -> Result<StreamTokenV1, StreamTokenDecodeError> {
    let trimmed = value.trim();
    let bytes = STANDARD
        .decode(trimmed.as_bytes())
        .map_err(StreamTokenDecodeError::InvalidBase64)?;
    decode_from_bytes(&bytes).map_err(StreamTokenDecodeError::InvalidPayload)
}

/// Errors emitted while constructing the gateway fetcher.
#[derive(Debug, Error)]
pub enum GatewayBuildError {
    #[error("failed to construct HTTP client: {0}")]
    ClientBuild(reqwest::Error),
    #[error("manifest identifier must be a 32-byte hex string: {manifest_id}")]
    InvalidManifestId { manifest_id: String },
    #[error("chunker handle must not be empty")]
    EmptyChunkerHandle,
    #[error("invalid {header} header: {reason}")]
    InvalidHeader {
        header: &'static str,
        reason: &'static str,
    },
    #[error("invalid moderation proof key: {source}")]
    InvalidModerationTokenKey {
        #[source]
        source: ModerationTokenError,
    },
    #[error("Sora-Req-Salt-Epoch must be provided when Sora-Req-Blinded-CID is set")]
    MissingSaltEpoch,
    #[error("Sora-Req-Salt-Epoch cannot be supplied without Sora-Req-Blinded-CID")]
    SaltEpochWithoutBlindedCid,
    #[error("duplicate provider identifier `{provider_id}`")]
    DuplicateProvider { provider_id: String },
    #[error("provider identifier `{provider_id}` must be 32-byte hex")]
    InvalidProviderId { provider_id: String },
    #[error(
        "stream token provider id `{token_provider_id}` does not match provider `{provider_id}`"
    )]
    ProviderIdMismatch {
        provider_id: String,
        token_provider_id: String,
    },
    #[error("provider `{provider_id}` URL parse error: {source}")]
    InvalidBaseUrl {
        provider_id: String,
        source: url::ParseError,
    },
    #[error("provider `{provider_id}` privacy events URL parse error: {source}")]
    InvalidPrivacyUrl {
        provider_id: String,
        source: url::ParseError,
    },
    #[error("provider `{provider_id}` stream token decode error: {source}")]
    InvalidStreamToken {
        provider_id: String,
        source: StreamTokenDecodeError,
    },
    #[error("provider `{provider_id}` stream token declares zero max_streams")]
    ZeroStreamCapacity { provider_id: String },
    #[error(
        "provider `{provider_id}` stream token profile `{token_profile}` \
         does not match expected chunker handle `{expected}`"
    )]
    ProfileMismatch {
        provider_id: String,
        token_profile: String,
        expected: String,
    },
    #[error(
        "provider `{provider_id}` stream token manifest CID mismatch (expected {expected}, got {actual})"
    )]
    ManifestCidMismatch {
        provider_id: String,
        expected: String,
        actual: String,
    },
}

/// Errors surfaced while fetching manifests from gateways.
#[derive(Debug, Error)]
pub enum GatewayManifestError {
    /// No providers were registered with the fetch context.
    #[error("no providers configured for manifest fetch")]
    NoProviders,
    /// HTTP request failed (transport or body read error).
    #[error("provider `{provider}` manifest request failed: {error}")]
    Request { provider: String, error: String },
    /// Manifest endpoint returned a non-success status.
    #[error("provider `{provider}` manifest request returned status {status}: {body:?}")]
    Status {
        provider: String,
        status: StatusCode,
        body: Option<String>,
    },
    /// Gateway did not advertise the expected cache/denylist version.
    #[error(
        "provider `{provider}` manifest response advertised cache version {observed:?} but expected {expected} (status {status})"
    )]
    CacheVersionMismatch {
        provider: String,
        expected: String,
        observed: Option<String>,
        status: StatusCode,
    },
    /// Manifest response was missing a required field.
    #[error("provider `{provider}` manifest response missing `{field}`")]
    MissingField {
        provider: String,
        field: &'static str,
    },
    /// Manifest payload failed to decode from Base64.
    #[error("provider `{provider}` manifest payload base64 decode failed: {error}")]
    Base64 { provider: String, error: String },
    /// Manifest payload failed to decode as Norito.
    #[error("provider `{provider}` manifest decode failed: {error}")]
    Decode { provider: String, error: String },
    /// Manifest digest provided by the gateway did not match the decoded payload.
    #[error("provider `{provider}` manifest digest mismatch (expected {expected}, got {actual})")]
    DigestMismatch {
        provider: String,
        expected: String,
        actual: String,
    },
}

/// Parsed manifest details fetched from a gateway endpoint.
#[derive(Debug, Clone)]
pub struct GatewayFetchedManifest {
    /// Raw manifest payload as returned by the gateway.
    pub manifest_bytes: Vec<u8>,
    /// Decoded Norito manifest.
    pub manifest: ManifestV1,
    /// Canonical manifest digest advertised by the gateway.
    pub manifest_digest: blake3::Hash,
    /// BLAKE3 digest of the payload captured during ingestion.
    pub payload_digest: blake3::Hash,
    /// Total content length recorded for the manifest.
    pub content_length: u64,
    /// Number of chunks recorded by the gateway.
    pub chunk_count: u64,
    /// Chunking profile handle stored alongside the manifest.
    pub chunk_profile_handle: String,
    /// Cache version advertised by the gateway response.
    pub cache_version: Option<String>,
}

fn parse_manifest_response(
    provider: &str,
    body: &[u8],
    cache_version: Option<String>,
) -> Result<GatewayFetchedManifest, GatewayManifestError> {
    let value: Value = json::from_slice(body).map_err(|err| GatewayManifestError::Decode {
        provider: provider.to_string(),
        error: err.to_string(),
    })?;
    let manifest_b64 = value
        .get("manifest_b64")
        .and_then(Value::as_str)
        .ok_or_else(|| GatewayManifestError::MissingField {
            provider: provider.to_string(),
            field: "manifest_b64",
        })?;
    let manifest_bytes =
        STANDARD
            .decode(manifest_b64.as_bytes())
            .map_err(|err| GatewayManifestError::Base64 {
                provider: provider.to_string(),
                error: err.to_string(),
            })?;
    let manifest: ManifestV1 =
        decode_from_bytes(&manifest_bytes).map_err(|err| GatewayManifestError::Decode {
            provider: provider.to_string(),
            error: err.to_string(),
        })?;
    let manifest_digest_hex = value
        .get("manifest_digest_hex")
        .and_then(Value::as_str)
        .ok_or_else(|| GatewayManifestError::MissingField {
            provider: provider.to_string(),
            field: "manifest_digest_hex",
        })?
        .to_ascii_lowercase();
    let computed_digest = manifest
        .digest()
        .map_err(|err| GatewayManifestError::Decode {
            provider: provider.to_string(),
            error: err.to_string(),
        })?;
    let computed_digest_hex = hex::encode(computed_digest.as_bytes());
    if manifest_digest_hex != computed_digest_hex {
        return Err(GatewayManifestError::DigestMismatch {
            provider: provider.to_string(),
            expected: manifest_digest_hex,
            actual: computed_digest_hex,
        });
    }

    let payload_digest_hex = value
        .get("payload_digest_hex")
        .and_then(Value::as_str)
        .ok_or_else(|| GatewayManifestError::MissingField {
            provider: provider.to_string(),
            field: "payload_digest_hex",
        })?
        .to_ascii_lowercase();
    let payload_bytes =
        hex::decode(&payload_digest_hex).map_err(|err| GatewayManifestError::Decode {
            provider: provider.to_string(),
            error: format!("payload_digest_hex decode failed: {err}"),
        })?;
    if payload_bytes.len() != 32 {
        return Err(GatewayManifestError::Decode {
            provider: provider.to_string(),
            error: "payload_digest_hex must decode to 32 bytes".into(),
        });
    }
    let mut payload_digest_bytes = [0u8; 32];
    payload_digest_bytes.copy_from_slice(&payload_bytes);
    let payload_digest = blake3::Hash::from_bytes(payload_digest_bytes);

    let content_length = value
        .get("content_length")
        .and_then(Value::as_u64)
        .ok_or_else(|| GatewayManifestError::MissingField {
            provider: provider.to_string(),
            field: "content_length",
        })?;
    let chunk_count = value
        .get("chunk_count")
        .and_then(Value::as_u64)
        .ok_or_else(|| GatewayManifestError::MissingField {
            provider: provider.to_string(),
            field: "chunk_count",
        })?;
    let chunk_profile_handle = value
        .get("chunk_profile_handle")
        .and_then(Value::as_str)
        .ok_or_else(|| GatewayManifestError::MissingField {
            provider: provider.to_string(),
            field: "chunk_profile_handle",
        })?
        .to_string();

    Ok(GatewayFetchedManifest {
        manifest_bytes,
        manifest,
        manifest_digest: computed_digest,
        payload_digest,
        content_length,
        chunk_count,
        chunk_profile_handle,
        cache_version,
    })
}

/// Errors encountered while fetching chunks from a gateway.
#[derive(Debug, Error)]
pub enum GatewayFetchError {
    #[error("no configuration registered for provider `{provider}`")]
    UnknownProvider { provider: String },
    #[error("failed to join chunk URL for provider `{provider}`: {source}")]
    UrlJoin {
        provider: String,
        source: url::ParseError,
    },
    #[error("request to provider `{provider}` failed: {source}")]
    Request {
        provider: String,
        #[source]
        source: reqwest::Error,
    },
    #[error(
        "provider `{provider}` cache version mismatch (expected {expected}, observed {observed:?}) with status {status}"
    )]
    CacheVersionMismatch {
        provider: String,
        expected: String,
        observed: Option<String>,
        status: StatusCode,
        evidence: Option<GatewayFailureEvidence>,
    },
    #[error("failed to read response from provider `{provider}`: {source}")]
    RequestBody {
        provider: String,
        #[source]
        source: reqwest::Error,
    },
    #[error(
        "provider `{provider}` blocked request (status={status}, code={code:?}, token_present={token_present}, denylist={denylist:?}, cache_version={cache_version:?}, message={message:?})",
        status = .evidence.canonical_status,
        code = .evidence.code,
        token_present = .evidence.proof_token_b64.is_some(),
        denylist = .evidence.denylist_version,
        cache_version = .evidence.cache_version,
        message = .evidence.message,
    )]
    PolicyBlocked {
        provider: String,
        evidence: GatewayFailureEvidence,
    },
    #[error("provider `{provider}` returned unexpected status {status}: {body:?}")]
    UnexpectedStatus {
        provider: String,
        status: StatusCode,
        body: Option<String>,
    },
    #[error("provider `{provider}` stub error: {message}")]
    Stub { provider: String, message: String },
}

impl From<GatewayFetchError> for AttemptFailure {
    fn from(error: GatewayFetchError) -> Self {
        let message = match &error {
            GatewayFetchError::PolicyBlocked { evidence, .. } => {
                format!(
                    "{error} (observed_status={}, canonical_status={})",
                    evidence.observed_status, evidence.canonical_status
                )
            }
            GatewayFetchError::CacheVersionMismatch { .. } => error.to_string(),
            _ => error.to_string(),
        };
        let policy_block = match &error {
            GatewayFetchError::PolicyBlocked { evidence, .. } => {
                Some(PolicyBlockEvidence::from(evidence))
            }
            GatewayFetchError::CacheVersionMismatch { evidence, .. } => {
                evidence.as_ref().map(PolicyBlockEvidence::from)
            }
            _ => None,
        };
        AttemptFailure::Provider {
            message,
            policy_block,
        }
    }
}

impl From<&GatewayFailureEvidence> for PolicyBlockEvidence {
    fn from(evidence: &GatewayFailureEvidence) -> Self {
        PolicyBlockEvidence {
            observed_status: evidence.observed_status,
            canonical_status: evidence.canonical_status,
            code: evidence.code.clone(),
            cache_version: evidence.cache_version.clone(),
            denylist_version: evidence.denylist_version.clone(),
            proof_token_present: evidence.proof_token_b64.is_some(),
            message: evidence.message.clone(),
        }
    }
}

/// Stream token decoding errors surfaced during configuration.
#[derive(Debug, Error)]
pub enum StreamTokenDecodeError {
    #[error("stream token is not valid base64")]
    InvalidBase64(base64::DecodeError),
    #[error("stream token payload is not valid Norito")]
    InvalidPayload(norito::Error),
}

impl fmt::Display for GatewayFetcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "GatewayFetcher(providers={})",
            self.inner.providers.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    };

    use sorafs_chunker::ChunkProfile;
    use sorafs_manifest::StreamTokenBodyV1;

    use super::*;
    use crate::{
        CarBuildPlan, ChunkFetchSpec, moderation::encode_token, multi_fetch::FetchProvider,
    };

    fn sample_payload(len: usize) -> Vec<u8> {
        (0..len).map(|idx| (idx % 251) as u8).collect()
    }

    fn sample_stream_token(
        manifest_cid_hex: &str,
        provider_id_hex: &str,
        profile: &str,
        max_streams: u16,
    ) -> StreamTokenV1 {
        StreamTokenV1 {
            body: StreamTokenBodyV1 {
                token_id: "01J9TK3GR0XM6YQF7WQXA9Z2SF".to_string(),
                manifest_cid: hex::decode(manifest_cid_hex).expect("cid hex"),
                provider_id: {
                    let mut bytes = [0u8; 32];
                    bytes.copy_from_slice(&hex::decode(provider_id_hex).expect("provider hex"));
                    bytes
                },
                profile_handle: profile.to_string(),
                max_streams,
                ttl_epoch: 9_999_999_999,
                rate_limit_bytes: 8 * 1024 * 1024,
                issued_at: 1_735_000_000,
                requests_per_minute: 120,
                token_pk_version: 1,
            },
            signature: vec![0; 64],
        }
    }

    fn encode_token_b64(token: &StreamTokenV1) -> String {
        let bytes = norito::to_bytes(token).expect("encode token");
        STANDARD.encode(bytes)
    }

    fn plan_for_payload(payload: &[u8]) -> CarBuildPlan {
        CarBuildPlan::single_file_with_profile(payload, ChunkProfile::DEFAULT).expect("build plan")
    }

    fn manifest_id_from_payload(payload: &[u8]) -> String {
        hex::encode(blake3::hash(payload).as_bytes())
    }

    fn provider_id_hex() -> String {
        "ab".repeat(32)
    }

    fn chunker_handle() -> String {
        "sorafs.sf1@1.0.0".to_string()
    }

    #[test]
    fn manifest_envelope_rejects_invalid_base64() {
        let config = GatewayFetchConfig {
            manifest_id_hex: manifest_id_from_payload(&[1, 2, 3, 4]),
            chunker_handle: chunker_handle(),
            manifest_envelope_b64: Some("!!not-base64!!".to_string()),
            client_id: None,
            expected_manifest_cid_hex: None,
            blinded_cid_b64: None,
            salt_epoch: None,
            expected_cache_version: None,
            moderation_token_key_b64: None,
        };

        let err = NormalisedConfig::from_config(config).expect_err("manifest envelope should fail");
        match err {
            GatewayBuildError::InvalidHeader { header, reason } => {
                assert_eq!(header, HEADER_SORA_MANIFEST_ENVELOPE);
                assert_eq!(reason, "manifest envelope must contain valid base64");
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn provider_id_mismatch_is_rejected() {
        let payload = sample_payload(1024);
        let manifest_id_hex = manifest_id_from_payload(&payload);
        let provider_id = provider_id_hex();
        let token_provider_id = "cd".repeat(32);
        let chunker = chunker_handle();
        let token = sample_stream_token(&manifest_id_hex, &token_provider_id, &chunker, 2);
        let token_b64 = encode_token_b64(&token);

        let config = GatewayFetchConfig {
            manifest_id_hex: manifest_id_hex.clone(),
            chunker_handle: chunker,
            manifest_envelope_b64: None,
            client_id: None,
            expected_manifest_cid_hex: None,
            blinded_cid_b64: None,
            salt_epoch: None,
            expected_cache_version: None,
            moderation_token_key_b64: None,
        };
        let input = GatewayProviderInput {
            name: "provider-1".to_string(),
            provider_id_hex: provider_id.clone(),
            base_url: "https://example.invalid".to_string(),
            stream_token_b64: token_b64,
            privacy_events_url: None,
        };

        let err = GatewayFetchContext::new(config, vec![input]).expect_err("should fail");
        match err {
            GatewayBuildError::ProviderIdMismatch {
                provider_id: found,
                token_provider_id: token_id,
            } => {
                assert_eq!(found, provider_id);
                assert_eq!(token_id, token_provider_id);
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn gateway_fetcher_serves_chunk_successfully() {
        let payload = sample_payload(8 * 1024);
        let plan = plan_for_payload(&payload);
        let manifest_id_hex = manifest_id_from_payload(&payload);
        let provider_id = provider_id_hex();
        let chunker_handle = chunker_handle();
        let manifest_cid_hex = manifest_id_hex.clone();
        let token = sample_stream_token(&manifest_cid_hex, &provider_id, &chunker_handle, 4);
        let token_b64 = encode_token_b64(&token);

        let path = format!(
            "/v2/sorafs/storage/chunk/{}/{}",
            manifest_id_hex,
            hex::encode(plan.chunk_fetch_specs()[0].digest.as_slice())
        );
        let mut responses = HashMap::new();
        responses.insert(
            path.clone(),
            HttpResponse {
                status: StatusCode::OK,
                headers: HeaderMap::new(),
                body: payload[0..plan.chunk_fetch_specs()[0].length as usize].to_vec(),
            },
        );
        let engine = Arc::new(MockHttpEngine::new(responses));

        let config = GatewayFetchConfig {
            manifest_id_hex: manifest_id_hex.clone(),
            chunker_handle: chunker_handle.clone(),
            manifest_envelope_b64: Some("ZW52ZWxvcGU=".to_string()),
            client_id: Some("orchestrator".to_string()),
            expected_manifest_cid_hex: Some(manifest_cid_hex),
            blinded_cid_b64: None,
            salt_epoch: None,
            expected_cache_version: None,
            moderation_token_key_b64: None,
        };
        let provider_input = GatewayProviderInput {
            name: "alpha".to_string(),
            provider_id_hex: provider_id.clone(),
            base_url: "https://gateway.example/".to_string(),
            stream_token_b64: token_b64.clone(),
            privacy_events_url: None,
        };

        let context =
            GatewayFetchContext::build_with_engine(config, [provider_input], engine.clone())
                .expect("context");

        let outcome = context
            .execute_plan(&plan, FetchOptions::default())
            .await
            .expect("fetch outcome");
        assert_eq!(outcome.chunks.len(), plan.chunk_fetch_specs().len());
        assert_eq!(
            outcome.chunks[0],
            payload[0..plan.chunk_fetch_specs()[0].length as usize]
        );

        let requests = engine.recorded();
        assert_eq!(requests.len(), 1);
        let request = &requests[0];
        assert_eq!(request.path, path);
        assert_eq!(
            request
                .headers
                .get(HEADER_SORA_STREAM_TOKEN)
                .and_then(|value| value.to_str().ok()),
            Some(token_b64.as_str())
        );
        assert_eq!(
            request
                .headers
                .get(HEADER_SORA_CHUNKER)
                .and_then(|value| value.to_str().ok()),
            Some(chunker_handle.as_str())
        );
        assert!(request.headers.contains_key(HEADER_SORA_NONCE));
        assert_eq!(
            request
                .headers
                .get(HEADER_SORA_MANIFEST_ENVELOPE)
                .and_then(|value| value.to_str().ok()),
            Some("ZW52ZWxvcGU=")
        );
        assert_eq!(
            request
                .headers
                .get(HEADER_SORA_CLIENT)
                .and_then(|value| value.to_str().ok()),
            Some("orchestrator")
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn gateway_fetcher_sets_blinded_headers() {
        let payload = sample_payload(2048);
        let plan = plan_for_payload(&payload);
        let manifest_id_hex = manifest_id_from_payload(&payload);
        let provider_id = provider_id_hex();
        let chunker_handle = chunker_handle();
        let token = sample_stream_token(&manifest_id_hex, &provider_id, &chunker_handle, 2);
        let token_b64 = encode_token_b64(&token);
        let blinded_b64 = URL_SAFE_NO_PAD.encode([0u8; 32]);
        let salt_epoch = 42u32;

        let path = format!(
            "/v2/sorafs/storage/chunk/{}/{}",
            manifest_id_hex,
            hex::encode(plan.chunk_fetch_specs()[0].digest.as_slice())
        );
        let mut responses = HashMap::new();
        responses.insert(
            path.clone(),
            HttpResponse {
                status: StatusCode::OK,
                headers: HeaderMap::new(),
                body: payload[0..plan.chunk_fetch_specs()[0].length as usize].to_vec(),
            },
        );
        let engine = Arc::new(MockHttpEngine::new(responses));

        let config = GatewayFetchConfig {
            manifest_id_hex: manifest_id_hex.clone(),
            chunker_handle: chunker_handle.clone(),
            manifest_envelope_b64: None,
            client_id: None,
            expected_manifest_cid_hex: Some(manifest_id_hex.clone()),
            blinded_cid_b64: Some(blinded_b64.clone()),
            salt_epoch: Some(salt_epoch),
            expected_cache_version: None,
            moderation_token_key_b64: None,
        };
        let provider_input = GatewayProviderInput {
            name: "alpha".to_string(),
            provider_id_hex: provider_id.clone(),
            base_url: "https://gateway.example/".to_string(),
            stream_token_b64: token_b64,
            privacy_events_url: None,
        };

        let context =
            GatewayFetchContext::build_with_engine(config, [provider_input], engine.clone())
                .expect("context");
        context
            .execute_plan(&plan, FetchOptions::default())
            .await
            .expect("fetch outcome");

        let requests = engine.recorded();
        assert_eq!(requests.len(), 1);
        let request = &requests[0];
        let epoch_value = salt_epoch.to_string();
        assert_eq!(request.path, path);
        assert_eq!(
            request
                .headers
                .get(HeaderName::from_static(HEADER_SORA_REQ_BLINDED_CID))
                .and_then(|value| value.to_str().ok()),
            Some(blinded_b64.as_str())
        );
        assert_eq!(
            request
                .headers
                .get(HeaderName::from_static(HEADER_SORA_REQ_SALT_EPOCH))
                .and_then(|value| value.to_str().ok()),
            Some(epoch_value.as_str())
        );
        let req_nonce = request
            .headers
            .get(HeaderName::from_static(HEADER_SORA_REQ_NONCE))
            .and_then(|value| value.to_str().ok());
        let sorafs_nonce = request
            .headers
            .get(HeaderName::from_static(HEADER_SORA_NONCE))
            .and_then(|value| value.to_str().ok());
        assert_eq!(req_nonce, sorafs_nonce);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn gateway_fetcher_propagates_error_status() {
        let payload = sample_payload(1024);
        let plan = plan_for_payload(&payload);
        let manifest_id_hex = manifest_id_from_payload(&payload);
        let provider_id = provider_id_hex();
        let chunker_handle = chunker_handle();
        let token = sample_stream_token(&manifest_id_hex, &provider_id, &chunker_handle, 2);
        let token_b64 = encode_token_b64(&token);

        let path = format!(
            "/v2/sorafs/storage/chunk/{}/{}",
            manifest_id_hex,
            hex::encode(plan.chunk_fetch_specs()[0].digest.as_slice())
        );
        let mut responses = HashMap::new();
        responses.insert(
            path.clone(),
            HttpResponse {
                status: StatusCode::TOO_MANY_REQUESTS,
                headers: HeaderMap::new(),
                body: br#"{"error":"stream_token_rate_limited"}"#.to_vec(),
            },
        );
        let engine = Arc::new(MockHttpEngine::new(responses));

        let config = GatewayFetchConfig {
            manifest_id_hex: manifest_id_hex.clone(),
            chunker_handle: chunker_handle.clone(),
            manifest_envelope_b64: None,
            client_id: None,
            expected_manifest_cid_hex: Some(manifest_id_hex.clone()),
            blinded_cid_b64: None,
            salt_epoch: None,
            expected_cache_version: None,
            moderation_token_key_b64: None,
        };
        let provider_input = GatewayProviderInput {
            name: "alpha".to_string(),
            provider_id_hex: provider_id.clone(),
            base_url: "https://gateway.example/".to_string(),
            stream_token_b64: token_b64.clone(),
            privacy_events_url: None,
        };
        let context =
            GatewayFetchContext::build_with_engine(config, [provider_input], engine.clone())
                .expect("context");
        let fetcher = context.fetcher();
        let provider: FetchProvider = context.providers()[0].clone();
        let spec: ChunkFetchSpec = plan.chunk_fetch_specs()[0].clone();

        let request = FetchRequest {
            provider: Arc::new(provider),
            spec,
            attempt: 1,
        };

        let error = fetcher.fetch(request).await.expect_err("should fail");
        match error {
            GatewayFetchError::PolicyBlocked { evidence, .. } => {
                assert_eq!(evidence.canonical_status, StatusCode::TOO_MANY_REQUESTS);
                assert_eq!(evidence.code.as_deref(), Some("stream_token_rate_limited"));
            }
            other => panic!("unexpected error {other:?}"),
        }

        let recorded = engine.recorded();
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].path, path);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn gateway_fetcher_surfaces_policy_block_evidence() {
        let payload = sample_payload(2048);
        let plan = plan_for_payload(&payload);
        let manifest_id_hex = manifest_id_from_payload(&payload);
        let provider_id = provider_id_hex();
        let chunker_handle = chunker_handle();
        let token = sample_stream_token(&manifest_id_hex, &provider_id, &chunker_handle, 2);
        let token_b64 = encode_token_b64(&token);

        let path = format!(
            "/v2/sorafs/storage/chunk/{}/{}",
            manifest_id_hex,
            hex::encode(plan.chunk_fetch_specs()[0].digest.as_slice())
        );
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static(HEADER_SORA_MODERATION_TOKEN),
            HeaderValue::from_static("dG9rZW4="),
        );
        headers.insert(
            HeaderName::from_static(HEADER_SORA_DENYLIST_VERSION),
            HeaderValue::from_static("v1"),
        );
        headers.insert(
            HeaderName::from_static(HEADER_SORA_CACHE_VERSION),
            HeaderValue::from_static("cache-v1"),
        );
        let mut responses = HashMap::new();
        responses.insert(
            path.clone(),
            HttpResponse {
                status: StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS,
                headers,
                body: br#"{"error":"denylisted","message":"blocked"}"#.to_vec(),
            },
        );
        let engine = Arc::new(MockHttpEngine::new(responses));

        let config = GatewayFetchConfig {
            manifest_id_hex: manifest_id_hex.clone(),
            chunker_handle: chunker_handle.clone(),
            manifest_envelope_b64: None,
            client_id: None,
            expected_manifest_cid_hex: Some(manifest_id_hex.clone()),
            blinded_cid_b64: None,
            salt_epoch: None,
            expected_cache_version: None,
            moderation_token_key_b64: None,
        };
        let provider_input = GatewayProviderInput {
            name: "alpha".to_string(),
            provider_id_hex: provider_id.clone(),
            base_url: "https://gateway.example/".to_string(),
            stream_token_b64: token_b64,
            privacy_events_url: None,
        };

        let context =
            GatewayFetchContext::build_with_engine(config, [provider_input], engine.clone())
                .expect("context");
        let request = FetchRequest {
            provider: Arc::new(context.providers()[0].clone()),
            spec: plan.chunk_fetch_specs()[0].clone(),
            attempt: 1,
        };

        let error = context
            .fetcher()
            .fetch(request)
            .await
            .expect_err("should be blocked");
        match error {
            GatewayFetchError::PolicyBlocked { evidence, .. } => {
                assert_eq!(
                    evidence.observed_status,
                    StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS
                );
                assert_eq!(
                    evidence.canonical_status,
                    StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS
                );
                assert_eq!(evidence.code.as_deref(), Some("denylisted"));
                assert_eq!(evidence.proof_token_b64.as_deref(), Some("dG9rZW4="));
                assert_eq!(evidence.denylist_version.as_deref(), Some("v1"));
                assert_eq!(evidence.cache_version.as_deref(), Some("cache-v1"));
                assert_eq!(evidence.message.as_deref(), Some("blocked"));
            }
            other => panic!("unexpected error {other:?}"),
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn gateway_fetcher_rejects_cache_version_mismatch() {
        let payload = sample_payload(512);
        let plan = plan_for_payload(&payload);
        let manifest_id_hex = manifest_id_from_payload(&payload);
        let provider_id = provider_id_hex();
        let chunker_handle = chunker_handle();
        let token = sample_stream_token(&manifest_id_hex, &provider_id, &chunker_handle, 1);
        let token_b64 = encode_token_b64(&token);

        let path = format!(
            "/v2/sorafs/storage/chunk/{}/{}",
            manifest_id_hex,
            hex::encode(plan.chunk_fetch_specs()[0].digest.as_slice())
        );
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static(HEADER_SORA_CACHE_VERSION),
            HeaderValue::from_static("cache-v1"),
        );
        let mut responses = HashMap::new();
        responses.insert(
            path.clone(),
            HttpResponse {
                status: StatusCode::OK,
                headers,
                body: payload[0..plan.chunk_fetch_specs()[0].length as usize].to_vec(),
            },
        );
        let engine = Arc::new(MockHttpEngine::new(responses));

        let config = GatewayFetchConfig {
            manifest_id_hex: manifest_id_hex.clone(),
            chunker_handle: chunker_handle.clone(),
            manifest_envelope_b64: None,
            client_id: None,
            expected_manifest_cid_hex: Some(manifest_id_hex.clone()),
            blinded_cid_b64: None,
            salt_epoch: None,
            expected_cache_version: Some("cache-v2".to_string()),
            moderation_token_key_b64: None,
        };
        let provider_input = GatewayProviderInput {
            name: "alpha".to_string(),
            provider_id_hex: provider_id.clone(),
            base_url: "https://gateway.example/".to_string(),
            stream_token_b64: token_b64,
            privacy_events_url: None,
        };

        let context =
            GatewayFetchContext::build_with_engine(config, [provider_input], engine.clone())
                .expect("context");
        let request = FetchRequest {
            provider: Arc::new(context.providers()[0].clone()),
            spec: plan.chunk_fetch_specs()[0].clone(),
            attempt: 1,
        };

        let err = context
            .fetcher()
            .fetch(request)
            .await
            .expect_err("should fail");
        match err {
            GatewayFetchError::CacheVersionMismatch {
                expected,
                observed,
                status,
                ..
            } => {
                assert_eq!(expected, "cache-v2");
                assert_eq!(observed.as_deref(), Some("cache-v1"));
                assert_eq!(status, StatusCode::OK);
            }
            other => panic!("unexpected error {other:?}"),
        }

        let recorded = engine.recorded();
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].path, path);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn gateway_policy_evidence_includes_verified_proof() {
        let payload = sample_payload(1024);
        let plan = plan_for_payload(&payload);
        let manifest_id_hex = manifest_id_from_payload(&payload);
        let provider_id = provider_id_hex();
        let chunker_handle = chunker_handle();
        let token = sample_stream_token(&manifest_id_hex, &provider_id, &chunker_handle, 1);
        let token_b64 = encode_token_b64(&token);
        let chunk_digest_hex = hex::encode(plan.chunk_fetch_specs()[0].digest.as_slice());
        let proof_key = ModerationTokenKey::from_bytes([7u8; 32]);
        let proof_token = encode_token(
            &proof_key,
            &manifest_id_hex,
            Some(&chunk_digest_hex),
            "dl-v1",
            "cache-v1",
        )
        .expect("encode moderation token");
        let proof_key_b64 = STANDARD.encode(proof_key.as_ref());

        let path = format!(
            "/v2/sorafs/storage/chunk/{}/{}",
            manifest_id_hex, chunk_digest_hex
        );
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static(HEADER_SORA_MODERATION_TOKEN),
            HeaderValue::from_str(&proof_token).expect("store proof token"),
        );
        headers.insert(
            HeaderName::from_static(HEADER_SORA_DENYLIST_VERSION),
            HeaderValue::from_static("dl-v1"),
        );
        headers.insert(
            HeaderName::from_static(HEADER_SORA_CACHE_VERSION),
            HeaderValue::from_static("cache-v1"),
        );
        let mut responses = HashMap::new();
        responses.insert(
            path.clone(),
            HttpResponse {
                status: StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS,
                headers,
                body: br#"{"error":"denylisted"}"#.to_vec(),
            },
        );
        let engine = Arc::new(MockHttpEngine::new(responses));

        let config = GatewayFetchConfig {
            manifest_id_hex: manifest_id_hex.clone(),
            chunker_handle: chunker_handle.clone(),
            manifest_envelope_b64: None,
            client_id: None,
            expected_manifest_cid_hex: Some(manifest_id_hex.clone()),
            blinded_cid_b64: None,
            salt_epoch: None,
            expected_cache_version: Some("cache-v1".to_string()),
            moderation_token_key_b64: Some(proof_key_b64),
        };
        let provider_input = GatewayProviderInput {
            name: "alpha".to_string(),
            provider_id_hex: provider_id.clone(),
            base_url: "https://gateway.example/".to_string(),
            stream_token_b64: token_b64,
            privacy_events_url: None,
        };

        let context = GatewayFetchContext::build_with_engine(config, [provider_input], engine)
            .expect("context");
        let request = FetchRequest {
            provider: Arc::new(context.providers()[0].clone()),
            spec: plan.chunk_fetch_specs()[0].clone(),
            attempt: 1,
        };

        let err = context.fetcher().fetch(request).await.expect_err("blocked");
        match err {
            GatewayFetchError::PolicyBlocked { evidence, .. } => {
                let proof = evidence.verified_proof.expect("proof attached");
                assert_eq!(proof.body.cache_version, "cache-v1");
                assert_eq!(proof.body.denylist_version, "dl-v1");
                assert_eq!(
                    proof.body.chunk_digest,
                    Some(plan.chunk_fetch_specs()[0].digest)
                );
                let mut expected_manifest = [0u8; 32];
                expected_manifest.copy_from_slice(&hex::decode(manifest_id_hex).unwrap());
                assert_eq!(proof.body.manifest_id, expected_manifest);
            }
            other => panic!("unexpected error {other:?}"),
        }
    }

    #[test]
    fn failure_evidence_falls_back_to_denylist_version_for_cache_binding() {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static(HEADER_SORA_DENYLIST_VERSION),
            HeaderValue::from_static("dl-v2"),
        );
        let response = HttpResponse {
            status: StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS,
            headers,
            body: br#"{"error":"denylisted"}"#.to_vec(),
        };

        let evidence = extract_failure_evidence(&response, None, None).expect("evidence");
        assert_eq!(evidence.cache_version.as_deref(), Some("dl-v2"));
    }

    #[derive(Clone)]
    struct RecordedRequest {
        path: String,
        headers: HeaderMap,
    }

    struct MockHttpEngine {
        responses: HashMap<String, HttpResponse>,
        recorded: Mutex<Vec<RecordedRequest>>,
    }

    impl MockHttpEngine {
        fn new(responses: HashMap<String, HttpResponse>) -> Self {
            Self {
                responses,
                recorded: Mutex::new(Vec::new()),
            }
        }

        fn recorded(&self) -> Vec<RecordedRequest> {
            self.recorded.lock().unwrap().clone()
        }
    }

    impl HttpEngine for MockHttpEngine {
        fn get(&self, request: HttpRequest) -> HttpFuture {
            let path = request.url.path().to_string();
            let headers = request.headers.clone();
            self.recorded.lock().unwrap().push(RecordedRequest {
                path: path.clone(),
                headers,
            });

            let maybe = self.responses.get(&path).cloned();
            let error_message = format!("no stubbed response for {path}");
            Box::pin(async move { maybe.ok_or_else(|| HttpError::Stub(error_message.clone())) })
        }
    }

    #[test]
    fn attempt_failure_preserves_policy_block_evidence() {
        let evidence = GatewayFailureEvidence {
            observed_status: StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS,
            canonical_status: StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS,
            code: Some("denylisted".to_string()),
            proof_token_b64: Some("token".to_string()),
            denylist_version: Some("dl-v1".to_string()),
            cache_version: Some("cache-v1".to_string()),
            message: Some("blocked".to_string()),
            verified_proof: None,
        };
        let error = GatewayFetchError::PolicyBlocked {
            provider: "alpha".to_string(),
            evidence: evidence.clone(),
        };
        let failure = AttemptFailure::from(error);

        match failure {
            AttemptFailure::Provider {
                policy_block: Some(policy),
                message,
            } => {
                assert!(message.contains("canonical_status"));
                assert_eq!(policy.cache_version.as_deref(), Some("cache-v1"));
                assert_eq!(policy.denylist_version.as_deref(), Some("dl-v1"));
                assert!(policy.proof_token_present);
                assert_eq!(policy.canonical_status, evidence.canonical_status);
                assert_eq!(policy.observed_status, evidence.observed_status);
            }
            other => panic!("unexpected attempt failure: {other:?}"),
        }
    }
}
