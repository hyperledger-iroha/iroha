//! Multi-source chunk retrieval for SoraFS payloads.
//!
//! The orchestrator defined in this module schedules chunk downloads across a
//! pool of providers, verifies returned data, and emits a consolidated result
//! that higher layers can stream into a CAR writer or on-disk store. It is
//! transport-agnostic: callers provide an async fetcher that knows how to talk
//! to their networking stack or storage adapters, while the scheduler handles
//! determinism, retry policy, and basic fairness.

use std::{
    collections::VecDeque,
    fmt,
    num::{NonZeroU32, NonZeroUsize},
    sync::Arc,
    time::Instant,
};

use futures::{Future, FutureExt, StreamExt, stream::FuturesUnordered};

use crate::{CarBuildPlan, ChunkFetchSpec};

/// Identifier used to reference providers that can serve SoraFS chunks.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProviderId(String);

impl ProviderId {
    /// Creates a new provider identifier.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProviderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Static configuration for a provider participating in multi-source fetch.
#[derive(Debug, Clone)]
pub struct FetchProvider {
    id: ProviderId,
    max_concurrent_chunks: NonZeroUsize,
    weight: NonZeroU32,
    metadata: Option<ProviderMetadata>,
}

impl FetchProvider {
    /// Creates a provider with the supplied identifier and default limits.
    ///
    /// By default a provider allows two in-flight chunk requests and carries a
    /// weight of one for the round-robin scheduler.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: ProviderId::new(id),
            max_concurrent_chunks: NonZeroUsize::new(2).expect("constant non-zero"),
            weight: NonZeroU32::new(1).expect("constant non-zero"),
            metadata: None,
        }
    }

    /// Updates the maximum number of concurrent chunk requests permitted.
    #[must_use]
    pub fn with_max_concurrent_chunks(mut self, value: NonZeroUsize) -> Self {
        self.max_concurrent_chunks = value;
        self
    }

    /// Updates the provider weight used by the round-robin scheduler.
    #[must_use]
    pub fn with_weight(mut self, weight: NonZeroU32) -> Self {
        self.weight = weight;
        self
    }

    /// Attaches provider metadata (e.g., capabilities, stakes) if available.
    #[must_use]
    pub fn with_metadata(mut self, metadata: ProviderMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Returns the provider identifier.
    #[must_use]
    pub fn id(&self) -> &ProviderId {
        &self.id
    }

    /// Returns the maximum number of in-flight chunk requests permitted.
    #[must_use]
    pub fn max_concurrent_chunks(&self) -> usize {
        self.max_concurrent_chunks.get()
    }

    /// Returns the provider scheduling weight.
    #[must_use]
    pub fn weight(&self) -> NonZeroU32 {
        self.weight
    }

    /// Returns optional metadata describing this provider.
    #[must_use]
    pub fn metadata(&self) -> Option<&ProviderMetadata> {
        self.metadata.as_ref()
    }
}

/// Supplemental metadata sourced from provider advertisements.
#[derive(Debug, Clone)]
pub struct ProviderMetadata {
    pub provider_id: Option<String>,
    pub profile_id: Option<String>,
    pub profile_aliases: Vec<String>,
    pub availability: Option<String>,
    pub stake_amount: Option<String>,
    pub max_streams: Option<u16>,
    pub refresh_deadline: Option<u64>,
    pub expires_at: Option<u64>,
    pub ttl_secs: Option<u64>,
    pub allow_unknown_capabilities: bool,
    pub capability_names: Vec<String>,
    pub rendezvous_topics: Vec<String>,
    pub notes: Option<String>,
    pub range_capability: Option<RangeCapability>,
    pub stream_budget: Option<StreamBudget>,
    pub transport_hints: Vec<TransportHint>,
    /// Optional admin endpoint exposing privacy telemetry.
    pub privacy_events_url: Option<String>,
}

impl ProviderMetadata {
    #[must_use]
    pub fn new() -> Self {
        Self {
            provider_id: None,
            profile_id: None,
            profile_aliases: Vec::new(),
            availability: None,
            stake_amount: None,
            max_streams: None,
            refresh_deadline: None,
            expires_at: None,
            ttl_secs: None,
            allow_unknown_capabilities: false,
            capability_names: Vec::new(),
            rendezvous_topics: Vec::new(),
            notes: None,
            range_capability: None,
            stream_budget: None,
            transport_hints: Vec::new(),
            privacy_events_url: None,
        }
    }
}

impl Default for ProviderMetadata {
    fn default() -> Self {
        Self::new()
    }
}

/// Range-capability metadata decoded from provider adverts.
#[derive(Debug, Clone)]
pub struct RangeCapability {
    pub max_chunk_span: u32,
    pub min_granularity: u32,
    pub supports_sparse_offsets: bool,
    pub requires_alignment: bool,
    pub supports_merkle_proof: bool,
}

/// Stream budget advertised by providers.
#[derive(Debug, Clone)]
pub struct StreamBudget {
    pub max_in_flight: u16,
    pub max_bytes_per_sec: u64,
    pub burst_bytes: Option<u64>,
}

/// Transport hint advertised by providers for ranged fetch.
#[derive(Debug, Clone)]
pub struct TransportHint {
    pub protocol: String,
    pub protocol_id: u8,
    pub priority: u8,
}

/// Normalised transport protocols understood by the fetcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportProtocolKind {
    /// HTTP range transport served by Torii.
    ToriiHttpRange,
    /// QUIC stream delivery.
    QuicStream,
    /// SoraNet relay transport.
    SoraNetRelay,
    /// Vendor-specific transport. The orchestrator ignores it for capability matching.
    VendorReserved,
    /// Transport protocol is unknown to the orchestrator.
    Unknown,
}

impl TransportProtocolKind {
    #[allow(clippy::match_same_arms)]
    fn from_hint(hint: &TransportHint) -> Self {
        match hint.protocol_id {
            1 => return Self::ToriiHttpRange,
            2 => return Self::QuicStream,
            3 => return Self::SoraNetRelay,
            255 => return Self::VendorReserved,
            _ => {}
        }

        let label = hint.protocol.trim().to_ascii_lowercase();
        match label.as_str() {
            "torii" | "torii_http_range" | "torii-http-range" | "toriihttp" | "torii-range" => {
                Self::ToriiHttpRange
            }
            "quic" | "quic_stream" | "quic-stream" | "quicstream" | "quic-streaming"
            | "quicstreaming" => Self::QuicStream,
            "soranet" | "soranet_relay" | "soranet-relay" | "soranetrelay" => Self::SoraNetRelay,
            "vendor" | "vendor_reserved" | "vendor-reserved" => Self::VendorReserved,
            _ => Self::Unknown,
        }
    }
}

impl TransportHint {
    /// Returns the normalised transport protocol advertised by this hint.
    #[must_use]
    pub fn protocol_kind(&self) -> TransportProtocolKind {
        TransportProtocolKind::from_hint(self)
    }
}

/// Reasons a provider cannot serve a specific chunk request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityMismatch {
    /// Provider metadata omitted the mandatory range capability descriptor.
    MissingRangeCapability,
    /// Chunk length exceeds the provider's advertised window.
    ChunkTooLarge {
        /// Length of the requested chunk in bytes.
        chunk_length: u32,
        /// Maximum contiguous span the provider is willing to serve.
        max_span: u32,
    },
    /// Chunk offset is not aligned to the advertised granularity.
    OffsetMisaligned {
        /// Byte offset of the chunk in the payload.
        offset: u64,
        /// Required alignment in bytes.
        required_alignment: u32,
    },
    /// Chunk length is not a multiple of the advertised granularity.
    LengthMisaligned {
        /// Chunk length in bytes.
        length: u32,
        /// Required alignment in bytes.
        required_alignment: u32,
    },
    /// Chunk length exceeds the provider's burst or rate budget.
    StreamBurstTooSmall {
        /// Chunk length in bytes.
        chunk_length: u32,
        /// Maximum burst window permitted (bytes).
        burst_limit: u64,
    },
}

impl fmt::Display for CapabilityMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingRangeCapability => write!(
                f,
                "provider metadata is missing the chunk_range_fetch capability descriptor"
            ),
            Self::ChunkTooLarge {
                chunk_length,
                max_span,
            } => write!(
                f,
                "chunk length {chunk_length} B exceeds provider max_chunk_span {max_span} B"
            ),
            Self::OffsetMisaligned {
                offset,
                required_alignment,
            } => write!(
                f,
                "chunk offset {offset} is not aligned to {required_alignment}-byte granularity"
            ),
            Self::LengthMisaligned {
                length,
                required_alignment,
            } => write!(
                f,
                "chunk length {length} B is not aligned to {required_alignment}-byte granularity"
            ),
            Self::StreamBurstTooSmall {
                chunk_length,
                burst_limit,
            } => write!(
                f,
                "chunk length {chunk_length} B exceeds provider stream burst budget {burst_limit} B"
            ),
        }
    }
}

/// Fetch-time configuration knobs for the orchestrator.
#[derive(Clone)]
pub struct FetchOptions {
    /// Verify returned chunk length against the plan.
    pub verify_lengths: bool,
    /// Verify returned chunk digest against the plan (BLAKE3-256).
    pub verify_digests: bool,
    /// Maximum number of attempts per chunk; `None` means unlimited retries.
    pub per_chunk_retry_limit: Option<usize>,
    /// Consecutive failures before a provider is marked disabled. `0` disables the guard.
    pub provider_failure_threshold: usize,
    /// Hard cap on total in-flight requests across all providers; `None` uses the
    /// sum of provider capacities.
    pub global_parallel_limit: Option<usize>,
    /// Optional scoring policy used to influence provider priority.
    pub score_policy: Option<Arc<dyn ScorePolicy>>,
}

impl Default for FetchOptions {
    fn default() -> Self {
        Self {
            verify_lengths: true,
            verify_digests: true,
            per_chunk_retry_limit: Some(3),
            provider_failure_threshold: 3,
            global_parallel_limit: None,
            score_policy: None,
        }
    }
}

impl fmt::Debug for FetchOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FetchOptions")
            .field("verify_lengths", &self.verify_lengths)
            .field("verify_digests", &self.verify_digests)
            .field("per_chunk_retry_limit", &self.per_chunk_retry_limit)
            .field(
                "provider_failure_threshold",
                &self.provider_failure_threshold,
            )
            .field("global_parallel_limit", &self.global_parallel_limit)
            .field(
                "score_policy",
                &self.score_policy.as_ref().map(|_| "ScorePolicy"),
            )
            .finish()
    }
}

/// Request metadata passed to the caller-supplied fetcher.
#[derive(Debug, Clone)]
pub struct FetchRequest {
    /// Provider to contact for this attempt.
    pub provider: Arc<FetchProvider>,
    /// Chunk specification describing offset, length, and digest expectation.
    pub spec: ChunkFetchSpec,
    /// 1-based attempt counter for the chunk.
    pub attempt: usize,
}

/// Successful chunk response returned by the fetcher.
#[derive(Debug, Clone)]
pub struct ChunkResponse {
    /// Raw chunk bytes as returned by the provider.
    pub bytes: Vec<u8>,
}

impl ChunkResponse {
    /// Creates a new response wrapper.
    #[must_use]
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }
}

/// Verification failures encountered while validating chunk contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChunkVerificationError {
    /// Chunk length differed from the plan.
    LengthMismatch { expected: u32, actual: usize },
    /// BLAKE3 digest differed from the plan.
    DigestMismatch {
        expected: [u8; 32],
        actual: [u8; 32],
    },
}

/// Categorises a failed chunk attempt.
#[derive(Debug, Clone)]
pub enum AttemptFailure {
    /// Transport or provider-level failure (timeout, HTTP error, etc.).
    Provider {
        message: String,
        policy_block: Option<PolicyBlockEvidence>,
    },
    /// Provider returned data that failed deterministic verification.
    InvalidChunk(ChunkVerificationError),
}

/// Evidence emitted when a gateway blocks a request for policy reasons.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyBlockEvidence {
    /// Status observed on the wire.
    pub observed_status: reqwest::StatusCode,
    /// Canonicalised status used for telemetry/reporting.
    pub canonical_status: reqwest::StatusCode,
    /// Canonical policy code parsed from the gateway response body.
    pub code: Option<String>,
    /// Cache version advertised by the gateway (or denylist version fallback).
    pub cache_version: Option<String>,
    /// Denylist version advertised by the gateway.
    pub denylist_version: Option<String>,
    /// Whether a moderation proof token was present on the response.
    pub proof_token_present: bool,
    /// Optional human-readable message extracted from the response body.
    pub message: Option<String>,
}

/// Detailed information about the most recent attempt for a chunk.
#[derive(Debug, Clone)]
pub struct AttemptError {
    /// Provider that served (or attempted to serve) the chunk.
    pub provider: ProviderId,
    /// Failure mode for the attempt.
    pub failure: AttemptFailure,
}

/// Receipt describing how a chunk was ultimately retrieved.
#[derive(Debug, Clone)]
pub struct ChunkReceipt {
    /// Index of the chunk within the plan (0-based).
    pub chunk_index: usize,
    /// Provider that successfully supplied the chunk.
    pub provider: ProviderId,
    /// Attempts taken to retrieve the chunk (including successes).
    pub attempts: usize,
    /// Latency of the successful attempt in milliseconds.
    pub latency_ms: f64,
    /// Size of the chunk payload in bytes.
    pub bytes: u32,
}

/// Aggregate report for a provider after a fetch session.
#[derive(Debug, Clone)]
pub struct ProviderReport {
    /// Provider configuration.
    pub provider: Arc<FetchProvider>,
    /// Number of successful chunk deliveries.
    pub successes: usize,
    /// Number of failed chunk attempts.
    pub failures: usize,
    /// Whether the provider was disabled due to consecutive failures.
    pub disabled: bool,
}

/// Final outcome returned by the orchestrator.
#[derive(Debug, Clone)]
pub struct FetchOutcome {
    /// Chunk payloads ordered by their index in the plan.
    pub chunks: Vec<Vec<u8>>,
    /// Receipts describing the serving provider for each chunk.
    pub chunk_receipts: Vec<ChunkReceipt>,
    /// Per-provider statistics from the session.
    pub provider_reports: Vec<ProviderReport>,
}

impl FetchOutcome {
    /// Concatenates chunks in plan order, returning the assembled payload.
    #[must_use]
    pub fn assemble_payload(&self) -> Vec<u8> {
        let total: usize = self.chunks.iter().map(Vec::len).sum();
        let mut out = Vec::with_capacity(total);
        for chunk in &self.chunks {
            out.extend_from_slice(chunk);
        }
        out
    }
}

/// Chunk payload delivered to observers when streaming is enabled.
pub struct ChunkDelivery<'a> {
    /// Index of the chunk within the plan (0-based).
    pub chunk_index: usize,
    /// Original chunk specification describing offset, length, and digest.
    pub spec: &'a ChunkFetchSpec,
    /// Provider that served the chunk.
    pub provider: &'a ProviderId,
    /// Attempts taken (including the successful attempt).
    pub attempts: usize,
    /// Latency of the successful attempt in milliseconds.
    pub latency_ms: f64,
    /// Verified chunk bytes.
    pub bytes: &'a [u8],
}

/// Errors emitted by chunk observers.
#[derive(Debug, Clone)]
pub struct ObserverError {
    message: String,
}

impl ObserverError {
    /// Creates a new observer error with the provided message.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ObserverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ObserverError {}

impl From<&str> for ObserverError {
    fn from(message: &str) -> Self {
        Self::new(message)
    }
}

impl From<String> for ObserverError {
    fn from(message: String) -> Self {
        Self::new(message)
    }
}

/// Observer invoked when chunks become available in plan order.
pub trait ChunkObserver: Send {
    /// Called once per chunk after verification succeeds.
    fn on_chunk(&mut self, delivery: ChunkDelivery<'_>) -> Result<(), ObserverError>;
}

impl<F> ChunkObserver for F
where
    F: for<'a> FnMut(ChunkDelivery<'a>) -> Result<(), ObserverError> + Send,
{
    fn on_chunk(&mut self, delivery: ChunkDelivery<'_>) -> Result<(), ObserverError> {
        self(delivery)
    }
}

fn effective_capacity(provider: &FetchProvider) -> usize {
    let configured = provider.max_concurrent_chunks();
    let mut capacity = configured;
    if let Some(metadata) = provider.metadata()
        && let Some(budget) = &metadata.stream_budget
    {
        let budget_limit = usize::from(budget.max_in_flight.max(1));
        capacity = capacity.min(budget_limit.max(1));
    }
    capacity.max(1)
}

pub(crate) fn provider_can_serve_chunk(
    provider: &FetchProvider,
    spec: &ChunkFetchSpec,
) -> Result<(), CapabilityMismatch> {
    let Some(metadata) = provider.metadata() else {
        return Ok(());
    };

    let range = metadata
        .range_capability
        .as_ref()
        .ok_or(CapabilityMismatch::MissingRangeCapability)?;

    if spec.length > range.max_chunk_span {
        return Err(CapabilityMismatch::ChunkTooLarge {
            chunk_length: spec.length,
            max_span: range.max_chunk_span,
        });
    }

    if range.requires_alignment {
        let alignment = u64::from(range.min_granularity.max(1));
        if !spec.offset.is_multiple_of(alignment) {
            return Err(CapabilityMismatch::OffsetMisaligned {
                offset: spec.offset,
                required_alignment: range.min_granularity,
            });
        }
        if u64::from(spec.length) % alignment != 0 {
            return Err(CapabilityMismatch::LengthMisaligned {
                length: spec.length,
                required_alignment: range.min_granularity,
            });
        }
    }

    if let Some(budget) = &metadata.stream_budget {
        let burst_limit = budget
            .burst_bytes
            .filter(|value| *value > 0)
            .unwrap_or_else(|| budget.max_bytes_per_sec.max(1));
        if u64::from(spec.length) > burst_limit {
            return Err(CapabilityMismatch::StreamBurstTooSmall {
                chunk_length: spec.length,
                burst_limit,
            });
        }
    }

    Ok(())
}

/// Errors returned by the multi-source fetch orchestrator.
#[derive(Debug)]
pub enum MultiSourceError {
    /// No providers were supplied.
    NoProviders,
    /// All providers became unavailable before completing the plan.
    NoHealthyProviders {
        chunk_index: usize,
        attempts: usize,
        last_error: Option<Box<AttemptError>>,
    },
    /// Providers were available but none could satisfy the capability constraints.
    NoCompatibleProviders {
        /// Chunk index that could not be scheduled.
        chunk_index: usize,
        /// Providers evaluated alongside their incompatibility reason.
        providers: Vec<(ProviderId, CapabilityMismatch)>,
    },
    /// Reached the retry limit for a chunk.
    ExhaustedRetries {
        chunk_index: usize,
        attempts: usize,
        last_error: Box<AttemptError>,
    },
    /// Observer (streaming callback) returned an error.
    ObserverFailed {
        chunk_index: usize,
        source: ObserverError,
    },
    /// Internal invariant violation (should not occur; indicates a logic bug).
    InternalInvariant(String),
}

impl fmt::Display for MultiSourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoProviders => write!(f, "no providers available for multi-source fetch"),
            Self::NoHealthyProviders {
                chunk_index,
                attempts,
                ..
            } => write!(
                f,
                "no healthy providers remaining for chunk {chunk_index} after {attempts} attempt(s)"
            ),
            Self::NoCompatibleProviders {
                chunk_index,
                providers,
            } => {
                let details = providers
                    .iter()
                    .map(|(provider, reason)| format!("{provider}: {reason}"))
                    .collect::<Vec<_>>()
                    .join("; ");
                write!(
                    f,
                    "no compatible providers for chunk {chunk_index}: {details}"
                )
            }
            Self::ExhaustedRetries {
                chunk_index,
                attempts,
                ..
            } => write!(
                f,
                "retry budget exhausted for chunk {chunk_index} after {attempts} attempt(s)"
            ),
            Self::ObserverFailed {
                chunk_index,
                source,
            } => write!(f, "chunk observer failed for chunk {chunk_index}: {source}"),
            Self::InternalInvariant(reason) => {
                write!(f, "internal orchestrator invariant violated: {reason}")
            }
        }
    }
}

impl std::error::Error for MultiSourceError {}

struct ChunkAttempt {
    spec: ChunkFetchSpec,
    attempts: usize,
}

type BoxedObserver = Box<dyn ChunkObserver + 'static>;

pub async fn fetch_plan_parallel<F, Fut, E>(
    plan: &CarBuildPlan,
    providers: impl IntoIterator<Item = FetchProvider>,
    fetcher: F,
    options: FetchOptions,
) -> Result<FetchOutcome, MultiSourceError>
where
    F: Fn(FetchRequest) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<ChunkResponse, E>> + Send + 'static,
    E: std::error::Error + Send + Sync + 'static,
{
    fetch_plan_parallel_internal(plan, providers, fetcher, options, None).await
}

pub async fn fetch_plan_parallel_with_observer<F, Fut, E, O>(
    plan: &CarBuildPlan,
    providers: impl IntoIterator<Item = FetchProvider>,
    fetcher: F,
    options: FetchOptions,
    observer: O,
) -> Result<FetchOutcome, MultiSourceError>
where
    F: Fn(FetchRequest) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<ChunkResponse, E>> + Send + 'static,
    E: std::error::Error + Send + Sync + 'static,
    O: ChunkObserver + 'static,
{
    fetch_plan_parallel_internal(plan, providers, fetcher, options, Some(Box::new(observer))).await
}

#[derive(Clone)]
struct ProviderState {
    config: Arc<FetchProvider>,
    capacity: usize,
    burst_limit: Option<u64>,
    bytes_inflight: u64,
    inflight: usize,
    failures: usize,
    consecutive_failures: usize,
    successes: usize,
    disabled: bool,
}

impl std::fmt::Debug for ProviderState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderState")
            .field("config", &self.config)
            .field("capacity", &self.capacity)
            .field("burst_limit", &self.burst_limit)
            .field("bytes_inflight", &self.bytes_inflight)
            .field("inflight", &self.inflight)
            .field("failures", &self.failures)
            .field("consecutive_failures", &self.consecutive_failures)
            .field("successes", &self.successes)
            .field("disabled", &self.disabled)
            .finish()
    }
}

impl ProviderState {
    fn new(config: FetchProvider) -> Self {
        let capacity = effective_capacity(&config);
        let burst_limit = config
            .metadata()
            .and_then(|metadata| metadata.stream_budget.as_ref())
            .map(|budget| {
                budget
                    .burst_bytes
                    .filter(|value| *value > 0)
                    .unwrap_or_else(|| budget.max_bytes_per_sec.max(1))
            });
        Self {
            config: Arc::new(config),
            capacity,
            burst_limit,
            bytes_inflight: 0,
            inflight: 0,
            failures: 0,
            consecutive_failures: 0,
            successes: 0,
            disabled: false,
        }
    }

    fn capacity(&self) -> usize {
        self.capacity
    }

    fn is_available(&self) -> bool {
        !self.disabled && self.inflight < self.capacity()
    }

    fn record_success(&mut self) {
        self.successes += 1;
        self.consecutive_failures = 0;
    }

    fn record_failure(&mut self, threshold: usize) {
        self.failures += 1;
        self.consecutive_failures += 1;
        if threshold != 0 && self.consecutive_failures >= threshold {
            self.disabled = true;
        }
    }

    fn into_report(self) -> ProviderReport {
        ProviderReport {
            provider: self.config,
            successes: self.successes,
            failures: self.failures,
            disabled: self.disabled,
        }
    }

    fn runtime_stats(&self) -> ProviderStats {
        ProviderStats {
            inflight: self.inflight,
            bytes_inflight: self.bytes_inflight,
            successes: self.successes,
            failures: self.failures,
            consecutive_failures: self.consecutive_failures,
            disabled: self.disabled,
        }
    }
}

struct JobOutcome<E> {
    provider_idx: usize,
    provider: Arc<FetchProvider>,
    spec: ChunkFetchSpec,
    attempt: usize,
    result: Result<ChunkResponse, E>,
    latency: std::time::Duration,
}

enum ProviderSelectionOutcome {
    Selected(usize),
    Unavailable,
    Ineligible(Vec<(usize, CapabilityMismatch)>),
}

fn has_active_providers(states: &[ProviderState]) -> bool {
    states.iter().any(|state| !state.disabled)
}

/// Snapshot of provider runtime statistics available to score policies.
#[derive(Debug, Clone)]
pub struct ProviderStats {
    pub inflight: usize,
    pub bytes_inflight: u64,
    pub successes: usize,
    pub failures: usize,
    pub consecutive_failures: usize,
    pub disabled: bool,
}

/// Context supplied to custom score policies.
pub struct ProviderScoreContext<'a> {
    pub provider: &'a FetchProvider,
    pub stats: &'a ProviderStats,
    pub spec: &'a ChunkFetchSpec,
}

/// Decision returned by a score policy.
#[derive(Debug, Clone, Copy)]
pub struct ProviderScoreDecision {
    pub priority_delta: i64,
    pub allow: bool,
}

impl ProviderScoreDecision {
    #[must_use]
    pub fn allow() -> Self {
        Self {
            priority_delta: 0,
            allow: true,
        }
    }
}

impl Default for ProviderScoreDecision {
    fn default() -> Self {
        Self::allow()
    }
}

/// Trait implemented by custom provider scoring policies.
pub trait ScorePolicy: Send + Sync {
    fn score(&self, ctx: ProviderScoreContext<'_>) -> ProviderScoreDecision;
}

fn select_weighted_provider(
    states: &[ProviderState],
    credits: &mut [i64],
    total_weight: i64,
    spec: &ChunkFetchSpec,
    score_policy: Option<&dyn ScorePolicy>,
) -> ProviderSelectionOutcome {
    if states.is_empty() {
        return ProviderSelectionOutcome::Unavailable;
    }
    debug_assert_eq!(states.len(), credits.len());

    let serviceable_exists = states
        .iter()
        .any(|state| !state.disabled && provider_can_serve_chunk(&state.config, spec).is_ok());

    let mut choice: Option<usize> = None;
    let mut max_credit = i64::MIN;
    for (idx, state) in states.iter().enumerate() {
        if state.disabled || !state.is_available() {
            continue;
        }
        if provider_can_serve_chunk(&state.config, spec).is_err() {
            continue;
        }
        if let Some(limit) = state.burst_limit {
            let projected = state.bytes_inflight.saturating_add(u64::from(spec.length));
            if projected > limit {
                continue;
            }
        }
        if let Some(policy) = score_policy {
            let stats = state.runtime_stats();
            let decision = policy.score(ProviderScoreContext {
                provider: &state.config,
                stats: &stats,
                spec,
            });
            if !decision.allow {
                continue;
            }
            credits[idx] = credits[idx]
                .saturating_add(state.config.weight().get() as i64)
                .saturating_add(decision.priority_delta);
        } else {
            credits[idx] = credits[idx].saturating_add(state.config.weight().get() as i64);
        }
        if credits[idx] > max_credit || choice.is_none() {
            max_credit = credits[idx];
            choice = Some(idx);
        }
    }

    if let Some(idx) = choice {
        credits[idx] -= total_weight;
        return ProviderSelectionOutcome::Selected(idx);
    }

    if !serviceable_exists {
        let mut reasons = Vec::new();
        for (idx, state) in states.iter().enumerate() {
            if state.disabled {
                continue;
            }
            if let Err(reason) = provider_can_serve_chunk(&state.config, spec) {
                reasons.push((idx, reason));
            }
        }
        if !reasons.is_empty() {
            return ProviderSelectionOutcome::Ineligible(reasons);
        }
    }

    ProviderSelectionOutcome::Unavailable
}

fn verify_chunk(
    spec: &ChunkFetchSpec,
    response: &ChunkResponse,
    options: &FetchOptions,
) -> Result<(), ChunkVerificationError> {
    if options.verify_lengths && response.bytes.len() != spec.length as usize {
        return Err(ChunkVerificationError::LengthMismatch {
            expected: spec.length,
            actual: response.bytes.len(),
        });
    }

    if options.verify_digests {
        let digest = blake3::hash(&response.bytes);
        if digest.as_bytes() != &spec.digest {
            return Err(ChunkVerificationError::DigestMismatch {
                expected: spec.digest,
                actual: *digest.as_bytes(),
            });
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn handle_attempt_failure(
    provider_states: &mut [ProviderState],
    provider_idx: usize,
    spec: ChunkFetchSpec,
    attempt: usize,
    attempt_error: AttemptError,
    failure_threshold: usize,
    retry_limit: Option<usize>,
    chunk_last_error: &mut [Option<AttemptError>],
    pending: &mut VecDeque<ChunkAttempt>,
    pending_front: &mut Option<ChunkAttempt>,
) -> Result<(), MultiSourceError> {
    {
        let state = &mut provider_states[provider_idx];
        state.record_failure(failure_threshold);
    }

    chunk_last_error[spec.chunk_index] = Some(attempt_error.clone());

    if let Some(limit) = retry_limit
        && attempt >= limit
    {
        return Err(MultiSourceError::ExhaustedRetries {
            chunk_index: spec.chunk_index,
            attempts: attempt,
            last_error: Box::new(attempt_error),
        });
    }

    if !has_active_providers(provider_states) {
        return Err(MultiSourceError::NoHealthyProviders {
            chunk_index: spec.chunk_index,
            attempts: attempt,
            last_error: Some(Box::new(attempt_error)),
        });
    }

    let retry = ChunkAttempt {
        spec,
        attempts: attempt,
    };
    if pending_front.is_none() {
        *pending_front = Some(retry);
    } else {
        pending.push_front(retry);
    }
    Ok(())
}

struct DeliveryError {
    chunk_index: usize,
    error: ObserverError,
}

fn deliver_ready_chunks(
    observer: &mut dyn ChunkObserver,
    specs: &[ChunkFetchSpec],
    chunk_results: &[Option<Vec<u8>>],
    chunk_receipts: &[Option<ChunkReceipt>],
    next_delivery: &mut usize,
) -> Result<(), DeliveryError> {
    while *next_delivery < specs.len() {
        let idx = *next_delivery;
        let Some(bytes) = chunk_results[idx].as_ref() else {
            break;
        };
        let Some(receipt) = chunk_receipts[idx].as_ref() else {
            return Err(DeliveryError {
                chunk_index: idx,
                error: ObserverError::new("missing chunk receipt while delivering to observer"),
            });
        };
        let delivery = ChunkDelivery {
            chunk_index: idx,
            spec: &specs[idx],
            provider: &receipt.provider,
            attempts: receipt.attempts,
            latency_ms: receipt.latency_ms,
            bytes,
        };
        observer.on_chunk(delivery).map_err(|error| DeliveryError {
            chunk_index: idx,
            error,
        })?;
        *next_delivery += 1;
    }
    Ok(())
}

/// Fetches chunks described by `plan` using the supplied providers and fetcher.
///
/// The orchestrator schedules chunk requests across providers using a weighted
/// round-robin policy, enforces per-provider and global concurrency limits,
/// verifies returned data, and retries failed chunks until they succeed or
/// exhaust the configured retry budget.
async fn fetch_plan_parallel_internal<F, Fut, E>(
    plan: &CarBuildPlan,
    providers: impl IntoIterator<Item = FetchProvider>,
    fetcher: F,
    options: FetchOptions,
    mut observer: Option<BoxedObserver>,
) -> Result<FetchOutcome, MultiSourceError>
where
    F: Fn(FetchRequest) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<ChunkResponse, E>> + Send + 'static,
    E: std::error::Error + Send + Sync + 'static,
{
    let mut provider_states: Vec<ProviderState> =
        providers.into_iter().map(ProviderState::new).collect();
    if provider_states.is_empty() {
        return Err(MultiSourceError::NoProviders);
    }

    let total_capacity: usize = provider_states.iter().map(ProviderState::capacity).sum();
    if total_capacity == 0 {
        return Err(MultiSourceError::NoProviders);
    }

    let mut global_limit = options.global_parallel_limit.unwrap_or(total_capacity);
    if global_limit == 0 {
        global_limit = total_capacity;
    }
    if global_limit == 0 {
        global_limit = 1;
    }
    global_limit = global_limit.min(total_capacity).max(1);

    let chunk_specs = plan.chunk_fetch_specs();
    let total_chunks = chunk_specs.len();

    let mut pending: VecDeque<ChunkAttempt> = chunk_specs
        .iter()
        .cloned()
        .map(|spec| ChunkAttempt { spec, attempts: 0 })
        .collect();
    if total_chunks == 0 {
        return Ok(FetchOutcome {
            chunks: Vec::new(),
            chunk_receipts: Vec::new(),
            provider_reports: provider_states
                .into_iter()
                .map(ProviderState::into_report)
                .collect(),
        });
    }

    let mut chunk_results: Vec<Option<Vec<u8>>> = vec![None; total_chunks];
    let mut chunk_receipts: Vec<Option<ChunkReceipt>> = vec![None; total_chunks];
    let mut chunk_last_error: Vec<Option<AttemptError>> = vec![None; total_chunks];
    let mut next_delivery = 0usize;

    let mut in_flight: FuturesUnordered<_> = FuturesUnordered::new();
    let fetcher = Arc::new(fetcher);
    let total_weight: i64 = provider_states
        .iter()
        .map(|state| state.config.weight().get() as i64)
        .sum();
    let mut provider_credits = vec![0i64; provider_states.len()];
    let score_policy = options.score_policy.as_deref();
    let mut completed = 0usize;
    let mut pending_front: Option<ChunkAttempt> = None;
    let retry_limit = options.per_chunk_retry_limit;
    let failure_threshold = options.provider_failure_threshold;

    while completed < total_chunks {
        while in_flight.len() < global_limit {
            let next_task = if let Some(task) = pending_front.take() {
                Some(task)
            } else {
                pending.pop_front()
            };
            let Some(task) = next_task else {
                break;
            };

            let selection = select_weighted_provider(
                &provider_states,
                &mut provider_credits,
                total_weight,
                &task.spec,
                score_policy,
            );
            let provider_idx = match selection {
                ProviderSelectionOutcome::Selected(idx) => idx,
                ProviderSelectionOutcome::Unavailable => {
                    if pending_front.is_none() {
                        pending_front = Some(task);
                    } else {
                        pending.push_front(task);
                    }
                    break;
                }
                ProviderSelectionOutcome::Ineligible(reasons) => {
                    let providers = reasons
                        .into_iter()
                        .map(|(idx, reason)| (provider_states[idx].config.id().clone(), reason))
                        .collect();
                    return Err(MultiSourceError::NoCompatibleProviders {
                        chunk_index: task.spec.chunk_index,
                        providers,
                    });
                }
            };

            let spec = task.spec.clone();
            let attempt_number = task.attempts + 1;
            provider_states[provider_idx].inflight += 1;
            if provider_states[provider_idx].burst_limit.is_some() {
                provider_states[provider_idx].bytes_inflight = provider_states[provider_idx]
                    .bytes_inflight
                    .saturating_add(u64::from(spec.length));
            }

            let provider = provider_states[provider_idx].config.clone();
            let fetcher = Arc::clone(&fetcher);

            let future = async move {
                let request = FetchRequest {
                    provider: Arc::clone(&provider),
                    spec: spec.clone(),
                    attempt: attempt_number,
                };
                let start = Instant::now();
                let result = fetcher(request).await;
                let latency = start.elapsed();
                JobOutcome {
                    provider_idx,
                    provider,
                    spec,
                    attempt: attempt_number,
                    result,
                    latency,
                }
            };

            in_flight.push(future.boxed());
        }

        if completed >= total_chunks {
            break;
        }

        if in_flight.is_empty() {
            let stalled = pending_front.take().or_else(|| pending.pop_front());
            if let Some(task) = stalled {
                if !has_active_providers(&provider_states) {
                    let last_error = chunk_last_error[task.spec.chunk_index].clone();
                    return Err(MultiSourceError::NoHealthyProviders {
                        chunk_index: task.spec.chunk_index,
                        attempts: task.attempts,
                        last_error: last_error.map(Box::new),
                    });
                }
                if pending_front.is_none() {
                    pending_front = Some(task);
                } else {
                    pending.push_front(task);
                }
            } else {
                break;
            }
        }

        let Some(outcome) = in_flight.next().await else {
            continue;
        };

        let provider_idx = outcome.provider_idx;
        let chunk_index = outcome.spec.chunk_index;
        let attempt = outcome.attempt;
        let provider_id = outcome.provider.id().clone();

        {
            let state = &mut provider_states[provider_idx];
            if state.inflight == 0 {
                return Err(MultiSourceError::InternalInvariant(format!(
                    "provider '{provider_id}' inflight underflow"
                )));
            }
            state.inflight -= 1;
            if state.burst_limit.is_some() {
                state.bytes_inflight = state
                    .bytes_inflight
                    .saturating_sub(u64::from(outcome.spec.length));
            }
        }

        match outcome.result {
            Ok(response) => match verify_chunk(&outcome.spec, &response, &options) {
                Ok(()) => {
                    {
                        let state = &mut provider_states[provider_idx];
                        state.record_success();
                    }
                    if chunk_results[chunk_index].is_some() {
                        return Err(MultiSourceError::InternalInvariant(format!(
                            "chunk {chunk_index} resolved multiple times"
                        )));
                    }
                    chunk_results[chunk_index] = Some(response.bytes);
                    chunk_receipts[chunk_index] = Some(ChunkReceipt {
                        chunk_index,
                        provider: provider_id,
                        attempts: attempt,
                        latency_ms: outcome.latency.as_secs_f64() * 1_000.0,
                        bytes: outcome.spec.length,
                    });
                    chunk_last_error[chunk_index] = None;
                    completed += 1;
                    if let Some(observer_ref) = observer.as_mut()
                        && let Err(error) = deliver_ready_chunks(
                            observer_ref.as_mut(),
                            &chunk_specs,
                            &chunk_results,
                            &chunk_receipts,
                            &mut next_delivery,
                        )
                    {
                        return Err(MultiSourceError::ObserverFailed {
                            chunk_index: error.chunk_index,
                            source: error.error,
                        });
                    }
                }
                Err(reason) => {
                    let failure = AttemptFailure::InvalidChunk(reason);
                    let attempt_error = AttemptError {
                        provider: provider_id,
                        failure,
                    };
                    handle_attempt_failure(
                        &mut provider_states,
                        provider_idx,
                        outcome.spec,
                        attempt,
                        attempt_error,
                        failure_threshold,
                        retry_limit,
                        &mut chunk_last_error,
                        &mut pending,
                        &mut pending_front,
                    )?;
                }
            },
            Err(err) => {
                let failure = AttemptFailure::Provider {
                    message: err.to_string(),
                    policy_block: None,
                };
                let attempt_error = AttemptError {
                    provider: provider_id,
                    failure,
                };
                handle_attempt_failure(
                    &mut provider_states,
                    provider_idx,
                    outcome.spec,
                    attempt,
                    attempt_error,
                    failure_threshold,
                    retry_limit,
                    &mut chunk_last_error,
                    &mut pending,
                    &mut pending_front,
                )?;
            }
        }
    }

    if completed != total_chunks {
        return Err(MultiSourceError::InternalInvariant(format!(
            "fetch completed {completed} of {total_chunks} chunks"
        )));
    }

    let chunks: Vec<Vec<u8>> = chunk_results
        .into_iter()
        .enumerate()
        .map(|(idx, maybe)| {
            maybe.ok_or_else(|| {
                MultiSourceError::InternalInvariant(format!(
                    "missing chunk payload for index {idx}"
                ))
            })
        })
        .collect::<Result<_, _>>()?;

    let receipts: Vec<ChunkReceipt> = chunk_receipts
        .into_iter()
        .enumerate()
        .map(|(idx, maybe)| {
            maybe.ok_or_else(|| {
                MultiSourceError::InternalInvariant(format!(
                    "missing chunk receipt for index {idx}"
                ))
            })
        })
        .collect::<Result<_, _>>()?;

    let provider_reports = provider_states
        .into_iter()
        .map(ProviderState::into_report)
        .collect();

    Ok(FetchOutcome {
        chunks,
        chunk_receipts: receipts,
        provider_reports,
    })
}

#[cfg(test)]
mod tests {
    use std::{
        error::Error,
        num::{NonZeroU32, NonZeroUsize},
        sync::{
            Arc, Mutex,
            atomic::{AtomicUsize, Ordering},
        },
    };

    use futures::{executor::block_on, future::poll_fn};
    use sorafs_chunker::ChunkProfile;

    use super::*;
    use crate::CarChunk;

    #[derive(Debug, Clone)]
    struct TestError(&'static str);

    #[test]
    fn transport_hint_protocol_kind_prefers_identifier() {
        let hint = TransportHint {
            protocol: "soranet".to_owned(),
            protocol_id: 3,
            priority: 0,
        };
        assert_eq!(hint.protocol_kind(), TransportProtocolKind::SoraNetRelay);
    }

    #[test]
    fn transport_hint_protocol_kind_matches_label_variants() {
        let quic_hint = TransportHint {
            protocol: "Quic-Stream".to_owned(),
            protocol_id: 0,
            priority: 0,
        };
        assert_eq!(quic_hint.protocol_kind(), TransportProtocolKind::QuicStream);

        let torii_hint = TransportHint {
            protocol: "torii_http_range".to_owned(),
            protocol_id: 0,
            priority: 0,
        };
        assert_eq!(
            torii_hint.protocol_kind(),
            TransportProtocolKind::ToriiHttpRange
        );

        let vendor_hint = TransportHint {
            protocol: "vendor_reserved".to_owned(),
            protocol_id: 0,
            priority: 0,
        };
        assert_eq!(
            vendor_hint.protocol_kind(),
            TransportProtocolKind::VendorReserved
        );

        let unknown_hint = TransportHint {
            protocol: "custom".to_owned(),
            protocol_id: 42,
            priority: 0,
        };
        assert_eq!(unknown_hint.protocol_kind(), TransportProtocolKind::Unknown);
    }

    impl fmt::Display for TestError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(self.0)
        }
    }

    impl Error for TestError {}

    fn plan_for_payload(payload: &[u8]) -> CarBuildPlan {
        CarBuildPlan::single_file(payload).expect("plan")
    }

    fn record_max(counter: &AtomicUsize, value: usize) {
        let mut current = counter.load(Ordering::SeqCst);
        while value > current {
            match counter.compare_exchange(current, value, Ordering::SeqCst, Ordering::SeqCst) {
                Ok(_) => break,
                Err(next) => current = next,
            }
        }
    }

    async fn cooperative_yield() {
        let mut yielded = false;
        poll_fn(|cx| {
            if yielded {
                std::task::Poll::Ready(())
            } else {
                yielded = true;
                cx.waker().wake_by_ref();
                std::task::Poll::Pending
            }
        })
        .await;
    }

    #[test]
    fn provider_capacity_respects_stream_budget() {
        let mut metadata = ProviderMetadata::new();
        metadata.stream_budget = Some(StreamBudget {
            max_in_flight: 1,
            max_bytes_per_sec: 1024,
            burst_bytes: Some(1024),
        });
        let provider = FetchProvider::new("throttled")
            .with_max_concurrent_chunks(NonZeroUsize::new(4).unwrap())
            .with_metadata(metadata);
        let state = ProviderState::new(provider);
        assert_eq!(state.capacity(), 1);
    }

    #[test]
    fn orchestrator_errors_when_all_providers_incompatible() {
        let payload: Vec<u8> = (0..=255u8).cycle().take(8 * 1024).collect();
        let plan = plan_for_payload(&payload);
        let chunk_len = plan
            .chunks
            .first()
            .map(|chunk| chunk.length)
            .expect("at least one chunk");
        assert!(chunk_len > 1, "chunk length must exceed 1 byte for test");
        let mut metadata = ProviderMetadata::new();
        metadata.range_capability = Some(RangeCapability {
            max_chunk_span: chunk_len - 1,
            min_granularity: 1,
            supports_sparse_offsets: false,
            requires_alignment: false,
            supports_merkle_proof: false,
        });
        let providers = vec![FetchProvider::new("limited").with_metadata(metadata)];
        let shared_payload = Arc::new(payload.clone());
        let fetcher = move |req: FetchRequest| {
            let payload = Arc::clone(&shared_payload);
            async move {
                let start = req.spec.offset as usize;
                let end = start + req.spec.length as usize;
                let bytes = payload[start..end].to_vec();
                Ok::<ChunkResponse, TestError>(ChunkResponse::new(bytes))
            }
        };

        let result = block_on(fetch_plan_parallel(
            &plan,
            providers,
            fetcher,
            FetchOptions::default(),
        ));

        match result {
            Err(MultiSourceError::NoCompatibleProviders {
                chunk_index,
                providers,
            }) => {
                assert_eq!(chunk_index, 0);
                assert_eq!(providers.len(), 1);
                let (provider_id, reason) = &providers[0];
                assert_eq!(provider_id.as_str(), "limited");
                assert_eq!(
                    *reason,
                    CapabilityMismatch::ChunkTooLarge {
                        chunk_length: chunk_len,
                        max_span: chunk_len - 1
                    }
                );
            }
            other => panic!("expected NoCompatibleProviders error, received {other:?}"),
        }
    }

    #[test]
    fn orchestrator_prefers_compatible_provider_when_mixed() {
        let payload: Vec<u8> = (0..=255u8).cycle().take(32 * 1024).collect();
        let plan = plan_for_payload(&payload);
        let chunk_len = plan
            .chunks
            .first()
            .map(|chunk| chunk.length)
            .expect("plan has chunks");
        assert!(chunk_len > 1, "chunk length must exceed one byte");

        let mut limited_metadata = ProviderMetadata::new();
        limited_metadata.range_capability = Some(RangeCapability {
            max_chunk_span: chunk_len - 1,
            min_granularity: 1,
            supports_sparse_offsets: false,
            requires_alignment: false,
            supports_merkle_proof: false,
        });
        let limited = FetchProvider::new("limited").with_metadata(limited_metadata);

        let mut full_metadata = ProviderMetadata::new();
        full_metadata.range_capability = Some(RangeCapability {
            max_chunk_span: chunk_len * 2,
            min_granularity: 1,
            supports_sparse_offsets: true,
            requires_alignment: false,
            supports_merkle_proof: false,
        });
        let compatible = FetchProvider::new("compatible").with_metadata(full_metadata);

        let providers = vec![limited, compatible];
        let shared_payload = Arc::new(payload.clone());
        let fetcher = move |req: FetchRequest| {
            let payload = Arc::clone(&shared_payload);
            async move {
                let start = req.spec.offset as usize;
                let end = start + req.spec.length as usize;
                let bytes = payload[start..end].to_vec();
                Ok::<ChunkResponse, TestError>(ChunkResponse::new(bytes))
            }
        };

        let outcome = block_on(fetch_plan_parallel(
            &plan,
            providers,
            fetcher,
            FetchOptions::default(),
        ))
        .expect("fetch succeeds with compatible provider");

        let limited_report = outcome
            .provider_reports
            .iter()
            .find(|report| report.provider.id().as_str() == "limited")
            .expect("limited provider report");
        assert_eq!(limited_report.successes, 0);
        assert_eq!(limited_report.failures, 0);
        assert!(!limited_report.disabled);

        let compatible_report = outcome
            .provider_reports
            .iter()
            .find(|report| report.provider.id().as_str() == "compatible")
            .expect("compatible provider report");
        assert_eq!(compatible_report.successes, plan.chunks.len());
        assert!(!compatible_report.disabled);
    }

    #[test]
    fn orchestrator_errors_when_provider_missing_range_capability() {
        let payload: Vec<u8> = (0..=255u8).cycle().take(8 * 1024).collect();
        let plan = plan_for_payload(&payload);

        let mut metadata = ProviderMetadata::new();
        metadata.stream_budget = Some(StreamBudget {
            max_in_flight: 1,
            max_bytes_per_sec: 1_000_000,
            burst_bytes: Some(1_000_000),
        });
        let providers = vec![FetchProvider::new("fallback").with_metadata(metadata)];

        let shared_payload = Arc::new(payload.clone());
        let fetcher = move |req: FetchRequest| {
            let payload = Arc::clone(&shared_payload);
            async move {
                let start = req.spec.offset as usize;
                let end = start + req.spec.length as usize;
                let bytes = payload[start..end].to_vec();
                Ok::<ChunkResponse, TestError>(ChunkResponse::new(bytes))
            }
        };

        match block_on(fetch_plan_parallel(
            &plan,
            providers,
            fetcher,
            FetchOptions::default(),
        )) {
            Err(MultiSourceError::NoCompatibleProviders {
                chunk_index,
                providers,
            }) => {
                assert_eq!(chunk_index, 0);
                assert_eq!(providers.len(), 1);
                let (provider_id, reason) = &providers[0];
                assert_eq!(provider_id.as_str(), "fallback");
                assert_eq!(*reason, CapabilityMismatch::MissingRangeCapability);
            }
            other => panic!("expected NoCompatibleProviders error, received {other:?}"),
        }
    }

    #[test]
    fn orchestrator_falls_back_when_stream_budget_rejects_primary() {
        let payload: Vec<u8> = (0..=255u8).cycle().take(32 * 1024).collect();
        let plan = plan_for_payload(&payload);
        let chunk_len = plan
            .chunks
            .first()
            .map(|chunk| chunk.length)
            .expect("plan has chunks");

        let mut limited_metadata = ProviderMetadata::new();
        limited_metadata.range_capability = Some(RangeCapability {
            max_chunk_span: chunk_len,
            min_granularity: 1,
            supports_sparse_offsets: false,
            requires_alignment: false,
            supports_merkle_proof: false,
        });
        limited_metadata.stream_budget = Some(StreamBudget {
            max_in_flight: 1,
            max_bytes_per_sec: u64::from(chunk_len - 1),
            burst_bytes: Some(u64::from(chunk_len - 1)),
        });

        let mut healthy_metadata = ProviderMetadata::new();
        healthy_metadata.range_capability = Some(RangeCapability {
            max_chunk_span: chunk_len * 2,
            min_granularity: 1,
            supports_sparse_offsets: true,
            requires_alignment: false,
            supports_merkle_proof: false,
        });
        healthy_metadata.stream_budget = Some(StreamBudget {
            max_in_flight: 4,
            max_bytes_per_sec: u64::from(chunk_len) * 8,
            burst_bytes: None,
        });

        let providers = vec![
            FetchProvider::new("limited").with_metadata(limited_metadata),
            FetchProvider::new("healthy")
                .with_max_concurrent_chunks(NonZeroUsize::new(2).unwrap())
                .with_metadata(healthy_metadata),
        ];

        let shared_payload = Arc::new(payload.clone());
        let fetcher = move |req: FetchRequest| {
            let payload = Arc::clone(&shared_payload);
            async move {
                let start = req.spec.offset as usize;
                let end = start + req.spec.length as usize;
                let bytes = payload[start..end].to_vec();
                Ok::<ChunkResponse, TestError>(ChunkResponse::new(bytes))
            }
        };

        let outcome = block_on(fetch_plan_parallel(
            &plan,
            providers,
            fetcher,
            FetchOptions::default(),
        ))
        .expect("fetch succeeds");

        let limited_report = outcome
            .provider_reports
            .iter()
            .find(|report| report.provider.id().as_str() == "limited")
            .expect("limited provider present");
        let healthy_report = outcome
            .provider_reports
            .iter()
            .find(|report| report.provider.id().as_str() == "healthy")
            .expect("healthy provider present");

        assert_eq!(limited_report.successes, 0);
        assert_eq!(limited_report.failures, 0);
        assert!(!limited_report.disabled);
        assert_eq!(healthy_report.successes, plan.chunks.len());
        assert_eq!(healthy_report.failures, 0);

        for receipt in outcome.chunk_receipts {
            assert_eq!(receipt.provider.as_str(), "healthy");
        }
    }

    #[test]
    fn orchestrator_reports_length_alignment_mismatch() {
        let payload: Vec<u8> = (0u8..12u8).collect();
        let plan = plan_for_payload(&payload);
        let chunk_len = plan
            .chunks
            .first()
            .map(|chunk| chunk.length)
            .expect("plan contains at least one chunk");
        assert!(
            chunk_len > 2,
            "chunk length {chunk_len} too small to trigger alignment mismatch"
        );
        let required_alignment = chunk_len.saturating_sub(1).max(2);

        let mut metadata = ProviderMetadata::new();
        metadata.range_capability = Some(RangeCapability {
            max_chunk_span: chunk_len,
            min_granularity: required_alignment,
            supports_sparse_offsets: false,
            requires_alignment: true,
            supports_merkle_proof: false,
        });

        let providers = vec![FetchProvider::new("aligned").with_metadata(metadata)];
        let shared_payload = Arc::new(payload.clone());
        let fetcher = move |req: FetchRequest| {
            let payload = Arc::clone(&shared_payload);
            async move {
                let start = req.spec.offset as usize;
                let end = start + req.spec.length as usize;
                Ok::<ChunkResponse, TestError>(ChunkResponse::new(payload[start..end].to_vec()))
            }
        };

        let result = block_on(fetch_plan_parallel(
            &plan,
            providers,
            fetcher,
            FetchOptions::default(),
        ));

        match result {
            Err(MultiSourceError::NoCompatibleProviders {
                chunk_index,
                providers,
            }) => {
                assert_eq!(chunk_index, 0);
                assert_eq!(providers.len(), 1);
                let (provider_id, reason) = &providers[0];
                assert_eq!(provider_id.as_str(), "aligned");
                match reason {
                    CapabilityMismatch::LengthMisaligned {
                        length,
                        required_alignment: alignment,
                    } => {
                        assert_eq!(*length, chunk_len);
                        assert_eq!(*alignment, required_alignment);
                    }
                    other => panic!("unexpected mismatch reason: {other:?}"),
                }
            }
            other => panic!("expected NoCompatibleProviders error, received {other:?}"),
        }
    }

    #[test]
    fn orchestrator_reports_offset_alignment_mismatch() {
        let payload: Vec<u8> = (0u8..64u8).collect();
        let chunk_offset = 6u64;
        let chunk_length = 8u32;
        let chunk_end = (chunk_offset as usize).saturating_add(chunk_length as usize);
        assert!(chunk_end <= payload.len(), "payload too small for chunk");
        let chunk_digest = *blake3::hash(&payload[chunk_offset as usize..chunk_end]).as_bytes();
        let plan = CarBuildPlan {
            chunk_profile: ChunkProfile::DEFAULT,
            payload_digest: blake3::hash(&payload),
            content_length: payload.len() as u64,
            chunks: vec![CarChunk {
                offset: chunk_offset,
                length: chunk_length,
                digest: chunk_digest,
                taikai_segment_hint: None,
            }],
            files: Vec::new(),
        };

        let mut metadata = ProviderMetadata::new();
        metadata.range_capability = Some(RangeCapability {
            max_chunk_span: chunk_length,
            min_granularity: 4,
            supports_sparse_offsets: false,
            requires_alignment: true,
            supports_merkle_proof: false,
        });

        let providers = vec![FetchProvider::new("offset").with_metadata(metadata)];
        let shared_payload = Arc::new(payload.clone());
        let fetcher = move |req: FetchRequest| {
            let payload = Arc::clone(&shared_payload);
            async move {
                let start = req.spec.offset as usize;
                let end = start + req.spec.length as usize;
                Ok::<ChunkResponse, TestError>(ChunkResponse::new(payload[start..end].to_vec()))
            }
        };

        let result = block_on(fetch_plan_parallel(
            &plan,
            providers,
            fetcher,
            FetchOptions::default(),
        ));

        match result {
            Err(MultiSourceError::NoCompatibleProviders {
                chunk_index,
                providers,
            }) => {
                assert_eq!(chunk_index, 0);
                assert_eq!(providers.len(), 1);
                let (provider_id, reason) = &providers[0];
                assert_eq!(provider_id.as_str(), "offset");
                match reason {
                    CapabilityMismatch::OffsetMisaligned {
                        offset,
                        required_alignment,
                    } => {
                        assert_eq!(*offset, chunk_offset);
                        assert_eq!(*required_alignment, 4);
                    }
                    other => panic!("unexpected mismatch reason: {other:?}"),
                }
            }
            other => panic!("expected NoCompatibleProviders error, received {other:?}"),
        }
    }

    #[test]
    fn stream_budget_max_in_flight_limits_parallelism() {
        let payload: Vec<u8> = (0..0x100000u32).map(|value| (value % 251) as u8).collect();
        let plan = plan_for_payload(&payload);
        assert!(
            plan.chunks.len() > 1,
            "plan must contain multiple chunks to test concurrency"
        );
        let max_chunk_len = plan
            .chunks
            .iter()
            .map(|chunk| chunk.length)
            .max()
            .expect("at least one chunk present");

        let mut metadata = ProviderMetadata::new();
        metadata.range_capability = Some(RangeCapability {
            max_chunk_span: max_chunk_len,
            min_granularity: 1,
            supports_sparse_offsets: false,
            requires_alignment: false,
            supports_merkle_proof: false,
        });
        metadata.stream_budget = Some(StreamBudget {
            max_in_flight: 1,
            max_bytes_per_sec: 512 * 1024,
            burst_bytes: Some(512 * 1024),
        });

        let provider = FetchProvider::new("throttled")
            .with_max_concurrent_chunks(NonZeroUsize::new(4).unwrap())
            .with_metadata(metadata);
        let shared_payload = Arc::new(payload.clone());
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let shared_payload_for_fetcher = Arc::clone(&shared_payload);
        let active_for_fetcher = Arc::clone(&active);
        let peak_for_fetcher = Arc::clone(&peak);
        let fetcher = move |req: FetchRequest| {
            let payload = Arc::clone(&shared_payload_for_fetcher);
            let active = Arc::clone(&active_for_fetcher);
            let peak = Arc::clone(&peak_for_fetcher);
            async move {
                let start = req.spec.offset as usize;
                let length = req.spec.length as usize;
                let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                record_max(&peak, current);
                assert!(
                    current <= 1,
                    "expected at most one in-flight chunk, saw {current}"
                );
                cooperative_yield().await;
                let end = start + length;
                let bytes = payload[start..end].to_vec();
                active.fetch_sub(1, Ordering::SeqCst);
                Ok::<ChunkResponse, TestError>(ChunkResponse::new(bytes))
            }
        };

        let outcome = block_on(fetch_plan_parallel(
            &plan,
            vec![provider],
            fetcher,
            FetchOptions::default(),
        ))
        .expect("fetch succeeds");

        assert_eq!(outcome.chunks.len(), plan.chunks.len());
        assert!(
            peak.load(Ordering::SeqCst) <= 1,
            "observed more than one in-flight chunk"
        );
    }

    #[test]
    fn weighted_scheduler_respects_provider_weights() {
        let chunk_count = 8;
        let chunk_len = 1024;
        let mut payload = Vec::with_capacity(chunk_count * chunk_len);
        for idx in 0..chunk_count * chunk_len {
            payload.push((idx % 251) as u8);
        }

        let chunks: Vec<CarChunk> = (0..chunk_count)
            .map(|i| {
                let offset = (i * chunk_len) as u64;
                let slice = &payload[i * chunk_len..(i + 1) * chunk_len];
                let digest = blake3::hash(slice);
                CarChunk {
                    offset,
                    length: chunk_len as u32,
                    digest: *digest.as_bytes(),
                    taikai_segment_hint: None,
                }
            })
            .collect();

        let plan = CarBuildPlan {
            chunk_profile: ChunkProfile::DEFAULT,
            payload_digest: blake3::hash(&payload),
            content_length: payload.len() as u64,
            chunks,
            files: Vec::new(),
        };

        let providers = vec![
            FetchProvider::new("heavy")
                .with_max_concurrent_chunks(NonZeroUsize::new(2).unwrap())
                .with_weight(NonZeroU32::new(3).unwrap()),
            FetchProvider::new("light")
                .with_max_concurrent_chunks(NonZeroUsize::new(2).unwrap())
                .with_weight(NonZeroU32::new(1).unwrap()),
        ];

        let shared_payload = Arc::new(payload.clone());
        let outcome = block_on(fetch_plan_parallel(
            &plan,
            providers,
            move |request: FetchRequest| {
                let payload = Arc::clone(&shared_payload);
                async move {
                    let start = request.spec.offset as usize;
                    let end = start + request.spec.length as usize;
                    Ok::<ChunkResponse, TestError>(ChunkResponse::new(payload[start..end].to_vec()))
                }
            },
            FetchOptions::default(),
        ))
        .expect("fetch succeeds");

        let heavy = outcome
            .provider_reports
            .iter()
            .find(|report| report.provider.id().as_str() == "heavy")
            .expect("heavy provider present");
        let light = outcome
            .provider_reports
            .iter()
            .find(|report| report.provider.id().as_str() == "light")
            .expect("light provider present");

        assert_eq!(heavy.successes + light.successes, chunk_count);
        assert!(
            heavy.successes >= light.successes,
            "heavy {} <= light {}",
            heavy.successes,
            light.successes
        );
        assert!(heavy.successes >= 4, "heavy successes {}", heavy.successes);
        assert!(light.successes >= 1, "light successes {}", light.successes);
    }

    #[test]
    fn multi_provider_fetch_succeeds() {
        let payload: Vec<u8> = (0..=255u8).cycle().take(32 * 1024).collect();
        let plan = plan_for_payload(&payload);
        let providers = vec![
            FetchProvider::new("alpha").with_max_concurrent_chunks(NonZeroUsize::new(2).unwrap()),
            FetchProvider::new("beta").with_max_concurrent_chunks(NonZeroUsize::new(2).unwrap()),
        ];
        let shared_payload = Arc::new(payload.clone());
        let fetcher = move |req: FetchRequest| {
            let payload = Arc::clone(&shared_payload);
            async move {
                let start = req.spec.offset as usize;
                let end = start + req.spec.length as usize;
                let bytes = payload[start..end].to_vec();
                Ok::<ChunkResponse, TestError>(ChunkResponse::new(bytes))
            }
        };

        let outcome = block_on(fetch_plan_parallel(
            &plan,
            providers,
            fetcher,
            FetchOptions::default(),
        ))
        .expect("fetch succeeds");

        assert_eq!(outcome.chunks.len(), plan.chunks.len());
        let mut assembled = Vec::new();
        for chunk in &outcome.chunks {
            assembled.extend_from_slice(chunk);
        }
        assert_eq!(assembled, payload);

        let total_successes: usize = outcome
            .provider_reports
            .iter()
            .map(|report| report.successes)
            .sum();
        assert_eq!(total_successes, plan.chunks.len());
        assert_eq!(outcome.chunk_receipts.len(), plan.chunks.len());
    }

    struct DenyAlphaPolicy;

    impl ScorePolicy for DenyAlphaPolicy {
        fn score(&self, ctx: ProviderScoreContext<'_>) -> ProviderScoreDecision {
            if ctx.provider.id().as_str() == "alpha" {
                ProviderScoreDecision {
                    priority_delta: 0,
                    allow: false,
                }
            } else {
                ProviderScoreDecision::allow()
            }
        }
    }

    #[test]
    fn score_policy_can_filter_providers() {
        let payload: Vec<u8> = (0..=255u8).cycle().take(16 * 1024).collect();
        let plan = plan_for_payload(&payload);
        let providers = vec![FetchProvider::new("alpha"), FetchProvider::new("beta")];
        let shared_payload = Arc::new(payload.clone());
        let fetcher = move |req: FetchRequest| {
            let payload = Arc::clone(&shared_payload);
            async move {
                let start = req.spec.offset as usize;
                let end = start + req.spec.length as usize;
                Ok::<ChunkResponse, TestError>(ChunkResponse::new(payload[start..end].to_vec()))
            }
        };

        let options = FetchOptions {
            score_policy: Some(Arc::new(DenyAlphaPolicy)),
            ..FetchOptions::default()
        };

        let outcome = block_on(fetch_plan_parallel(&plan, providers, fetcher, options))
            .expect("fetch succeeds with filtered providers");

        let beta_report = outcome
            .provider_reports
            .iter()
            .find(|report| report.provider.id().as_str() == "beta")
            .expect("beta provider present");
        assert_eq!(beta_report.successes, plan.chunks.len());

        let alpha_report = outcome
            .provider_reports
            .iter()
            .find(|report| report.provider.id().as_str() == "alpha")
            .expect("alpha provider present");
        assert_eq!(alpha_report.successes, 0);
        assert_eq!(alpha_report.failures, 0);
        assert!(!alpha_report.disabled, "policy should skip before failure");
    }

    #[test]
    fn orchestrator_failover_to_backup_provider() {
        let payload: Vec<u8> = (0..=255u8).cycle().take(16 * 1024).collect();
        let plan = plan_for_payload(&payload);
        let providers = vec![
            FetchProvider::new("primary").with_max_concurrent_chunks(NonZeroUsize::new(2).unwrap()),
            FetchProvider::new("backup").with_max_concurrent_chunks(NonZeroUsize::new(2).unwrap()),
        ];
        let shared_payload = Arc::new(payload.clone());
        let fetcher = move |req: FetchRequest| {
            let payload = Arc::clone(&shared_payload);
            async move {
                if req.provider.id().as_str() == "primary" {
                    Err::<ChunkResponse, TestError>(TestError("primary offline"))
                } else {
                    let start = req.spec.offset as usize;
                    let end = start + req.spec.length as usize;
                    let bytes = payload[start..end].to_vec();
                    Ok::<ChunkResponse, TestError>(ChunkResponse::new(bytes))
                }
            }
        };

        let outcome = block_on(fetch_plan_parallel(
            &plan,
            providers,
            fetcher,
            FetchOptions::default(),
        ))
        .expect("backup provider should complete fetch");

        let backup_report = outcome
            .provider_reports
            .iter()
            .find(|report| report.provider.id().as_str() == "backup")
            .expect("backup present");
        assert_eq!(backup_report.successes, plan.chunks.len());

        let primary_report = outcome
            .provider_reports
            .iter()
            .find(|report| report.provider.id().as_str() == "primary")
            .expect("primary present");
        assert!(primary_report.failures > 0);
    }

    #[test]
    fn digest_mismatch_triggers_error_after_retries() {
        let payload: Vec<u8> = (0..=255u8).cycle().take(8 * 1024).collect();
        let plan = plan_for_payload(&payload);
        let providers = vec![
            FetchProvider::new("alpha").with_max_concurrent_chunks(NonZeroUsize::new(1).unwrap()),
            FetchProvider::new("beta").with_max_concurrent_chunks(NonZeroUsize::new(1).unwrap()),
        ];
        let shared_payload = Arc::new(payload);
        let fetcher = move |req: FetchRequest| {
            let payload = Arc::clone(&shared_payload);
            async move {
                let start = req.spec.offset as usize;
                let end = start + req.spec.length as usize;
                let mut bytes = payload[start..end].to_vec();
                if let Some(first) = bytes.first_mut() {
                    *first = first.wrapping_add(1);
                }
                Ok::<ChunkResponse, TestError>(ChunkResponse::new(bytes))
            }
        };

        let options = FetchOptions {
            per_chunk_retry_limit: Some(2),
            provider_failure_threshold: 0,
            ..FetchOptions::default()
        };

        let result =
            block_on(fetch_plan_parallel(&plan, providers, fetcher, options)).expect_err("fails");

        match result {
            MultiSourceError::ExhaustedRetries { last_error, .. } => match last_error.failure {
                AttemptFailure::InvalidChunk(ChunkVerificationError::DigestMismatch { .. }) => {}
                other => panic!("unexpected failure mode: {other:?}"),
            },
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn streaming_observer_receives_chunks_in_order() {
        let payload = vec![0x5a; 8192];
        let plan = plan_for_payload(&payload);

        let shared_payload = Arc::new(payload.clone());
        let deliveries = Arc::new(Mutex::new(Vec::new()));
        let deliveries_for_observer = Arc::clone(&deliveries);

        let outcome = block_on(fetch_plan_parallel_with_observer(
            &plan,
            vec![FetchProvider::new("alpha")],
            {
                let payload = Arc::clone(&shared_payload);
                move |request: FetchRequest| {
                    let payload = Arc::clone(&payload);
                    async move {
                        let start = request.spec.offset as usize;
                        let end = start + request.spec.length as usize;
                        Ok::<ChunkResponse, TestError>(ChunkResponse::new(
                            payload[start..end].to_vec(),
                        ))
                    }
                }
            },
            FetchOptions::default(),
            move |delivery: ChunkDelivery<'_>| {
                deliveries_for_observer
                    .lock()
                    .expect("lock deliveries")
                    .push(delivery.chunk_index);
                Ok(())
            },
        ))
        .expect("fetch succeeds");

        let expected: Vec<usize> = (0..outcome.chunks.len()).collect();
        let observed = deliveries.lock().expect("lock deliveries").clone();
        assert_eq!(observed, expected);
    }

    #[test]
    fn streaming_observer_failure_propagates() {
        let payload = vec![0x1f; 2 * 256 * 1024];
        let plan = plan_for_payload(&payload);
        assert!(plan.chunks.len() > 1, "expected multiple chunks");
        let shared_payload = Arc::new(payload.clone());

        let error = block_on(fetch_plan_parallel_with_observer(
            &plan,
            vec![FetchProvider::new("alpha")],
            {
                let payload = Arc::clone(&shared_payload);
                move |request: FetchRequest| {
                    let payload = Arc::clone(&payload);
                    async move {
                        let start = request.spec.offset as usize;
                        let end = start + request.spec.length as usize;
                        Ok::<ChunkResponse, TestError>(ChunkResponse::new(
                            payload[start..end].to_vec(),
                        ))
                    }
                }
            },
            FetchOptions::default(),
            |delivery: ChunkDelivery<'_>| {
                if delivery.chunk_index == 1 {
                    return Err(ObserverError::new("observer failure"));
                }
                Ok(())
            },
        ))
        .expect_err("observer error should propagate");

        match error {
            MultiSourceError::ObserverFailed {
                chunk_index,
                source,
            } => {
                assert_eq!(chunk_index, 1);
                assert_eq!(source.to_string(), "observer failure");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
