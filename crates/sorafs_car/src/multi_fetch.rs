//! Multi-source chunk retrieval for SoraFS payloads.
//!
//! The orchestrator defined in this module schedules chunk downloads across a pool of providers,
//! verifies returned data, and emits a consolidated result that higher layers can stream into a CAR
//! writer or on-disk store. It is transport-agnostic: callers provide an async fetcher that knows
//! how to talk to their networking stack or storage adapters, while the scheduler handles
//! determinism, retry policy, and basic fairness.
use crate::{CarBuildPlan, CarPlanError, ChunkFetchSpec};
use futures::{Future, FutureExt, StreamExt, stream::FuturesUnordered};
use std::{
    collections::{BTreeMap, VecDeque},
    fmt,
    num::{NonZeroU32, NonZeroUsize},
    sync::Arc,
    time::Duration,
};
mod runtime;
use runtime::{ProviderRateWindow, classify_provider_error};

/// Maximum retained payload for the eager convenience API. Larger objects require a sink.
pub const MAX_EAGER_PAYLOAD_BYTES: u64 = 64 * 1024 * 1024;
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
            max_concurrent_chunks: NonZeroUsize::MIN.saturating_add(1),
            weight: NonZeroU32::MIN,
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
    /// Signed per-token request quota; consumed before dispatch, including failed requests.
    pub requests_per_minute: Option<u32>,
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
            requests_per_minute: None,
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
    /// Classify a hint by its advertised `protocol_id`; the `protocol` label is display-only.
    const fn from_hint(hint: &TransportHint) -> Self {
        match hint.protocol_id {
            1 => Self::ToriiHttpRange,
            2 => Self::QuicStream,
            3 => Self::SoraNetRelay,
            255 => Self::VendorReserved,
            _ => Self::Unknown,
        }
    }
}
impl TransportHint {
    /// Returns the transport protocol advertised by this hint's `protocol_id`.
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
    /// Maximum complete object accepted before allocating or dispatching any payload request.
    pub max_payload_bytes: u64,
    /// Maximum sum of chunks, files, and logical path components in the admitted plan.
    ///
    /// Receipts and canonical CAR geometry remain proportional to this separately bounded
    /// inventory. This does not describe the payload reservation tracked by the buffer limit.
    pub max_metadata_entries: usize,
    /// Maximum reserved bytes across running requests and the ordered delivery window.
    pub max_buffered_bytes: usize,
    /// Absolute fetch deadline, including provider cooldowns and sink delivery.
    pub session_timeout: Duration,
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
            max_payload_bytes: 8 * 1024 * 1024 * 1024,
            max_metadata_entries: 262_144,
            max_buffered_bytes: 16 * 1024 * 1024,
            session_timeout: Duration::from_secs(15 * 60),
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
            .field("max_payload_bytes", &self.max_payload_bytes)
            .field("max_metadata_entries", &self.max_metadata_entries)
            .field("max_buffered_bytes", &self.max_buffered_bytes)
            .field("session_timeout", &self.session_timeout)
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
impl FetchOptions {
    /// Admit complete object and metadata inventory limits before derived fetch allocations.
    pub fn validate_plan_limits(&self, plan: &CarBuildPlan) -> Result<(), MultiSourceError> {
        if self.max_payload_bytes == 0 || plan.content_length > self.max_payload_bytes {
            return Err(MultiSourceError::ResourceLimit(
                "complete payload exceeds the configured object limit",
            ));
        }
        let entries = plan
            .files
            .iter()
            .try_fold(plan.chunks.len(), |count, file| {
                count.checked_add(1)?.checked_add(file.path.len())
            });
        if self.max_metadata_entries == 0
            || self.max_metadata_entries > crate::CAR_PLAN_MAX_CHUNKS
            || entries.is_none_or(|count| count > self.max_metadata_entries)
        {
            return Err(MultiSourceError::ResourceLimit(
                "plan exceeds the configured metadata inventory limit",
            ));
        }
        Ok(())
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
    /// Exact canonical policy code parsed from the gateway response body.
    pub code: String,
    /// Governed decision source (`baseline` or `legal_safety_hold`).
    pub source: String,
    /// Lowercase hexadecimal digest of the active governed catalog.
    pub catalog_digest_hex: String,
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
/// Completed consuming fetch. Payload bytes have been delivered to the sink and released.
#[derive(Debug, Clone)]
pub struct StreamFetchOutcome {
    /// Receipts in plan order, including each verified chunk's length.
    pub chunk_receipts: Vec<ChunkReceipt>,
    /// Per-provider health statistics; throttling does not count as failure.
    pub provider_reports: Vec<ProviderReport>,
    /// Peak bytes reserved by running requests and completed chunks awaiting ordered delivery.
    pub peak_buffered_bytes: usize,
}

struct InternalFetchOutcome {
    retained: FetchOutcome,
    peak_buffered_bytes: usize,
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
        let mut burst_limit = budget
            .burst_bytes
            .filter(|value| *value > 0)
            .unwrap_or_else(|| budget.max_bytes_per_sec.max(1));
        if budget.max_bytes_per_sec != 0 {
            // A larger concurrency burst cannot authorize a single request that would exceed
            // the gateway's complete one-second byte quota in every possible window.
            burst_limit = burst_limit.min(budget.max_bytes_per_sec);
        }
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
    /// Configured complete-object or buffered-response resource limit was exceeded.
    ResourceLimit(&'static str),
    /// The absolute fetch deadline expired, including time spent in quota cooldowns.
    DeadlineExceeded,
    /// The fetch plan was malformed or its bounded fetch specification inventory could not be
    /// allocated.
    InvalidPlan(CarPlanError),
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
    /// Every currently available compatible provider was rejected by the score policy.
    NoPolicyEligibleProviders {
        /// Chunk index that could not be scheduled.
        chunk_index: usize,
        /// Available compatible providers rejected by the policy.
        providers: Vec<ProviderId>,
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
            Self::ResourceLimit(reason) => write!(f, "fetch resource limit exceeded: {reason}"),
            Self::DeadlineExceeded => f.write_str("fetch session deadline exceeded"),
            Self::InvalidPlan(error) => write!(f, "invalid multi-source fetch plan: {error}"),
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
            Self::NoPolicyEligibleProviders {
                chunk_index,
                providers,
            } => {
                let providers = providers
                    .iter()
                    .map(ProviderId::as_str)
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(
                    f,
                    "score policy rejected every available provider for chunk {chunk_index}: {providers}"
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
    fetch_plan_parallel_internal(plan, providers, fetcher, options, None)
        .await
        .map(|outcome| outcome.retained)
}
/// Consume verified chunks in plan order without retaining the complete payload.
///
/// The response reservation window includes unfinished requests and completed out-of-order
/// chunks. Observer callbacks must return promptly; an elapsed deadline is checked after every
/// callback. On failure the sink may contain a verified prefix and must not be published.
pub async fn fetch_plan_parallel_with_observer<F, Fut, E, O>(
    plan: &CarBuildPlan,
    providers: impl IntoIterator<Item = FetchProvider>,
    fetcher: F,
    options: FetchOptions,
    observer: O,
) -> Result<StreamFetchOutcome, MultiSourceError>
where
    F: Fn(FetchRequest) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<ChunkResponse, E>> + Send + 'static,
    E: std::error::Error + Send + Sync + 'static,
    O: ChunkObserver + 'static,
{
    let outcome =
        fetch_plan_parallel_internal(plan, providers, fetcher, options, Some(Box::new(observer)))
            .await?;
    Ok(StreamFetchOutcome {
        chunk_receipts: outcome.retained.chunk_receipts,
        provider_reports: outcome.retained.provider_reports,
        peak_buffered_bytes: outcome.peak_buffered_bytes,
    })
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
    rate_window: ProviderRateWindow,
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
            rate_window: ProviderRateWindow::new(&config),
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
    PolicyDenied(Vec<usize>),
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
    if states.is_empty() || states.len() != credits.len() {
        return ProviderSelectionOutcome::Unavailable;
    }
    let serviceable_exists = states
        .iter()
        .any(|state| !state.disabled && provider_can_serve_chunk(&state.config, spec).is_ok());
    let mut choice: Option<usize> = None;
    let mut max_credit = i64::MIN;
    let mut policy_denied = Vec::new();
    let mut policy_eligible_exists = false;
    let now = tokio::time::Instant::now();
    for (idx, state) in states.iter().enumerate() {
        if state.disabled || provider_can_serve_chunk(&state.config, spec).is_err() {
            continue;
        }
        let priority_delta = if let Some(policy) = score_policy {
            let stats = state.runtime_stats();
            let decision = policy.score(ProviderScoreContext {
                provider: &state.config,
                stats: &stats,
                spec,
            });
            if !decision.allow {
                policy_denied.push(idx);
                continue;
            }
            decision.priority_delta
        } else {
            0
        };
        // A compatible provider remains policy-eligible while its quota or capacity recovers.
        // Only a policy rejection by every compatible live provider is terminal.
        policy_eligible_exists = true;
        if !state.is_available() || state.rate_window.ready_at(u64::from(spec.length), now) > now {
            continue;
        }
        if let Some(limit) = state.burst_limit {
            let projected = state.bytes_inflight.saturating_add(u64::from(spec.length));
            if projected > limit {
                continue;
            }
        }
        credits[idx] = credits[idx]
            .saturating_add(state.config.weight().get() as i64)
            .saturating_add(priority_delta);
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
    if !policy_eligible_exists && !policy_denied.is_empty() {
        return ProviderSelectionOutcome::PolicyDenied(policy_denied);
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
    chunk_last_error: &mut BTreeMap<usize, AttemptError>,
    pending: &mut VecDeque<ChunkAttempt>,
    pending_front: &mut Option<ChunkAttempt>,
) -> Result<(), MultiSourceError> {
    {
        let state = &mut provider_states[provider_idx];
        state.record_failure(failure_threshold);
    }
    chunk_last_error.insert(spec.chunk_index, attempt_error.clone());
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
    if let Some(front) = pending_front.replace(retry) {
        pending.push_front(front);
    }
    Ok(())
}
/// Fetches chunks described by `plan` using the supplied providers and fetcher.
///
/// The orchestrator schedules chunk requests across providers using a weighted round-robin policy,
/// enforces per-provider and global concurrency limits, verifies returned data, and retries failed
/// chunks until they succeed or exhaust the configured retry budget.
async fn fetch_plan_parallel_internal<F, Fut, E>(
    plan: &CarBuildPlan,
    providers: impl IntoIterator<Item = FetchProvider>,
    fetcher: F,
    options: FetchOptions,
    mut observer: Option<BoxedObserver>,
) -> Result<InternalFetchOutcome, MultiSourceError>
where
    F: Fn(FetchRequest) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<ChunkResponse, E>> + Send + 'static,
    E: std::error::Error + Send + Sync + 'static,
{
    if !options.verify_lengths || !options.verify_digests {
        return Err(MultiSourceError::ResourceLimit(
            "chunk integrity verification must remain enabled",
        ));
    }
    if options.session_timeout.is_zero()
        || options.session_timeout > Duration::from_secs(24 * 60 * 60)
    {
        return Err(MultiSourceError::ResourceLimit(
            "session timeout must be between zero and 24 hours",
        ));
    }
    let deadline = tokio::time::Instant::now() + options.session_timeout;
    options.validate_plan_limits(plan)?;
    let consuming = observer.is_some();
    if !consuming && plan.content_length > MAX_EAGER_PAYLOAD_BYTES {
        return Err(MultiSourceError::ResourceLimit(
            "eager payload exceeds 64 MiB; use the consuming sink API",
        ));
    }
    let chunk_specs = plan
        .try_chunk_fetch_specs()
        .map_err(MultiSourceError::InvalidPlan)?;
    if options.max_buffered_bytes == 0
        || options.max_buffered_bytes > 256 * 1024 * 1024
        || chunk_specs
            .iter()
            .any(|spec| spec.length as usize > options.max_buffered_bytes)
    {
        return Err(MultiSourceError::ResourceLimit(
            "one chunk exceeds the configured reorder buffer",
        ));
    }
    let mut provider_states = Vec::new();
    for provider in providers {
        if provider_states.len() == 256 {
            return Err(MultiSourceError::ResourceLimit(
                "provider inventory exceeds 256",
            ));
        }
        if provider
            .metadata()
            .is_some_and(|metadata| metadata.requests_per_minute == Some(0))
        {
            return Err(MultiSourceError::ResourceLimit(
                "provider request quota must be positive",
            ));
        }
        provider_states.push(ProviderState::new(provider));
    }
    if provider_states.is_empty() {
        return Err(MultiSourceError::NoProviders);
    }
    let total_capacity = provider_states
        .iter()
        .map(ProviderState::capacity)
        .fold(0usize, usize::saturating_add);
    let global_limit = options
        .global_parallel_limit
        .unwrap_or(total_capacity)
        .max(1)
        .min(total_capacity)
        .min(256);
    let total_chunks = chunk_specs.len();
    let mut pending = VecDeque::new();
    let mut undispatched = 0usize;
    let mut pending_front: Option<ChunkAttempt> = None;
    let mut chunk_results = BTreeMap::<usize, Vec<u8>>::new();
    let mut chunk_receipts: Vec<Option<ChunkReceipt>> = vec![None; total_chunks];
    let mut chunk_last_error = BTreeMap::<usize, AttemptError>::new();
    let mut next_delivery = 0usize;
    let mut buffered_bytes = 0usize;
    let mut peak_buffered_bytes = 0usize;
    let mut payload_hasher = blake3::Hasher::new();
    let mut in_flight: FuturesUnordered<_> = FuturesUnordered::new();
    let fetcher = Arc::new(fetcher);
    let total_weight = provider_states
        .iter()
        .map(|state| i64::from(state.config.weight().get()))
        .sum();
    let mut provider_credits = vec![0i64; provider_states.len()];
    let mut completed = 0usize;
    while completed < total_chunks {
        if tokio::time::Instant::now() >= deadline {
            return Err(MultiSourceError::DeadlineExceeded);
        }
        while in_flight.len() < global_limit {
            let task = pending_front
                .take()
                .or_else(|| pending.pop_front())
                .or_else(|| {
                    let spec = chunk_specs.get(undispatched)?.clone();
                    undispatched += 1;
                    Some(ChunkAttempt { spec, attempts: 0 })
                });
            let Some(task) = task else { break };
            // Reserve both unfinished requests and completed out-of-order responses. A stalled
            // first chunk cannot cause the rest of the object to accumulate behind the sink.
            if buffered_bytes.saturating_add(task.spec.length as usize) > options.max_buffered_bytes
                || (consuming
                    && task.spec.chunk_index >= next_delivery.saturating_add(global_limit))
            {
                pending_front = Some(task);
                break;
            }
            let selection = select_weighted_provider(
                &provider_states,
                &mut provider_credits,
                total_weight,
                &task.spec,
                options.score_policy.as_deref(),
            );
            let provider_idx = match selection {
                ProviderSelectionOutcome::Selected(index) => index,
                ProviderSelectionOutcome::Unavailable => {
                    pending_front = Some(task);
                    break;
                }
                ProviderSelectionOutcome::Ineligible(reasons) => {
                    return Err(MultiSourceError::NoCompatibleProviders {
                        chunk_index: task.spec.chunk_index,
                        providers: reasons
                            .into_iter()
                            .map(|(index, reason)| {
                                (provider_states[index].config.id().clone(), reason)
                            })
                            .collect(),
                    });
                }
                ProviderSelectionOutcome::PolicyDenied(indices) => {
                    if in_flight.is_empty() {
                        return Err(MultiSourceError::NoPolicyEligibleProviders {
                            chunk_index: task.spec.chunk_index,
                            providers: indices
                                .into_iter()
                                .map(|index| provider_states[index].config.id().clone())
                                .collect(),
                        });
                    }
                    pending_front = Some(task);
                    break;
                }
            };
            let spec = task.spec;
            let attempt_number = task.attempts + 1;
            let state = &mut provider_states[provider_idx];
            state.inflight += 1;
            state.bytes_inflight = state.bytes_inflight.saturating_add(u64::from(spec.length));
            state
                .rate_window
                .reserve(u64::from(spec.length), tokio::time::Instant::now());
            buffered_bytes += spec.length as usize;
            peak_buffered_bytes = peak_buffered_bytes.max(buffered_bytes);
            let provider = state.config.clone();
            let fetcher = Arc::clone(&fetcher);
            in_flight.push(
                async move {
                    let start = tokio::time::Instant::now();
                    let result = fetcher(FetchRequest {
                        provider: Arc::clone(&provider),
                        spec: spec.clone(),
                        attempt: attempt_number,
                    })
                    .await;
                    JobOutcome {
                        provider_idx,
                        provider,
                        spec,
                        attempt: attempt_number,
                        result,
                        latency: start.elapsed(),
                    }
                }
                .boxed(),
            );
        }
        let now = tokio::time::Instant::now();
        let next_wake = pending_front.as_ref().and_then(|task| {
            provider_states
                .iter()
                .filter(|state| {
                    !state.disabled
                        && state.is_available()
                        && provider_can_serve_chunk(&state.config, &task.spec).is_ok()
                })
                .map(|state| state.rate_window.ready_at(u64::from(task.spec.length), now))
                .filter(|ready| *ready > now)
                .min()
        });
        if in_flight.is_empty() && next_wake.is_none() {
            let task = pending_front.as_ref().ok_or_else(|| {
                MultiSourceError::InternalInvariant(
                    "fetch has unfinished chunks without a pending request".into(),
                )
            })?;
            return Err(MultiSourceError::NoHealthyProviders {
                chunk_index: task.spec.chunk_index,
                attempts: task.attempts,
                last_error: chunk_last_error
                    .get(&task.spec.chunk_index)
                    .cloned()
                    .map(Box::new),
            });
        }
        let wake = next_wake.unwrap_or(deadline).min(deadline);
        let outcome = tokio::select! {
            result = in_flight.next(), if !in_flight.is_empty() => result,
            _ = runtime::sleep_until(wake) => None,
        };
        let Some(outcome) = outcome else { continue };
        let provider_idx = outcome.provider_idx;
        let chunk_index = outcome.spec.chunk_index;
        let provider_id = outcome.provider.id().clone();
        let state = &mut provider_states[provider_idx];
        state.inflight -= 1;
        state.bytes_inflight -= u64::from(outcome.spec.length);
        match outcome.result {
            Ok(response) => match verify_chunk(&outcome.spec, &response, &options) {
                Ok(()) => {
                    if chunk_receipts[chunk_index].is_some() {
                        return Err(MultiSourceError::InternalInvariant(format!(
                            "chunk {chunk_index} completed twice"
                        )));
                    }
                    state.record_success();
                    chunk_results.insert(chunk_index, response.bytes);
                    chunk_receipts[chunk_index] = Some(ChunkReceipt {
                        chunk_index,
                        provider: provider_id,
                        attempts: outcome.attempt,
                        latency_ms: outcome.latency.as_secs_f64() * 1_000.0,
                        bytes: outcome.spec.length,
                    });
                    chunk_last_error.remove(&chunk_index);
                    completed += 1;
                    if let Some(sink) = observer.as_mut() {
                        while let Some(bytes) = chunk_results.remove(&next_delivery) {
                            let receipt = chunk_receipts[next_delivery]
                                .as_ref()
                                .expect("completed chunk has a receipt");
                            payload_hasher.update(&bytes);
                            sink.on_chunk(ChunkDelivery {
                                chunk_index: next_delivery,
                                spec: &chunk_specs[next_delivery],
                                provider: &receipt.provider,
                                attempts: receipt.attempts,
                                latency_ms: receipt.latency_ms,
                                bytes: &bytes,
                            })
                            .map_err(|source| {
                                MultiSourceError::ObserverFailed {
                                    chunk_index: next_delivery,
                                    source,
                                }
                            })?;
                            buffered_bytes -= bytes.len();
                            next_delivery += 1;
                            if tokio::time::Instant::now() >= deadline {
                                return Err(MultiSourceError::DeadlineExceeded);
                            }
                        }
                    } else {
                        buffered_bytes -= outcome.spec.length as usize;
                    }
                }
                Err(reason) => {
                    buffered_bytes -= outcome.spec.length as usize;
                    let error = AttemptError {
                        provider: provider_id,
                        failure: AttemptFailure::InvalidChunk(reason),
                    };
                    handle_attempt_failure(
                        &mut provider_states,
                        provider_idx,
                        outcome.spec,
                        outcome.attempt,
                        error,
                        options.provider_failure_threshold,
                        options.per_chunk_retry_limit,
                        &mut chunk_last_error,
                        &mut pending,
                        &mut pending_front,
                    )?;
                }
            },
            Err(error) => {
                buffered_bytes -= outcome.spec.length as usize;
                let (failure, retry_after) = classify_provider_error(&error);
                if let Some(delay) = retry_after {
                    state.rate_window.throttle(delay);
                    // Quota denial did not attempt a payload read and does not spend the failure
                    // retry budget. The absolute session deadline bounds repeated denials.
                    pending.push_front(ChunkAttempt {
                        spec: outcome.spec,
                        attempts: outcome.attempt - 1,
                    });
                    if let Some(front) = pending_front.take() {
                        pending.push_back(front);
                    }
                    continue;
                }
                let error = AttemptError {
                    provider: provider_id,
                    failure,
                };
                handle_attempt_failure(
                    &mut provider_states,
                    provider_idx,
                    outcome.spec,
                    outcome.attempt,
                    error,
                    options.provider_failure_threshold,
                    options.per_chunk_retry_limit,
                    &mut chunk_last_error,
                    &mut pending,
                    &mut pending_front,
                )?;
            }
        }
    }
    let chunks: Vec<Vec<u8>> = chunk_results.into_values().collect();
    if !consuming {
        for bytes in &chunks {
            payload_hasher.update(bytes);
        }
    }
    if payload_hasher.finalize() != plan.payload_digest {
        return Err(MultiSourceError::InternalInvariant(
            "complete payload digest does not match the plan".into(),
        ));
    }
    if tokio::time::Instant::now() >= deadline {
        return Err(MultiSourceError::DeadlineExceeded);
    }
    let receipts = chunk_receipts
        .into_iter()
        .map(|receipt| {
            receipt.ok_or_else(|| {
                MultiSourceError::InternalInvariant("missing verified chunk receipt".into())
            })
        })
        .collect::<Result<_, _>>()?;
    Ok(InternalFetchOutcome {
        retained: FetchOutcome {
            chunks,
            chunk_receipts: receipts,
            provider_reports: provider_states
                .into_iter()
                .map(ProviderState::into_report)
                .collect(),
        },
        peak_buffered_bytes,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use futures::future::poll_fn;
    fn block_on<F: std::future::Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("test runtime")
            .block_on(future)
    }
    use sorafs_chunker::ChunkProfile;
    use std::{
        error::Error,
        num::{NonZeroU32, NonZeroUsize},
        sync::{
            Arc, Mutex,
            atomic::{AtomicUsize, Ordering},
        },
    };
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
    fn transport_hint_protocol_kind_ignores_display_label() {
        for (label, protocol_id, expected) in [
            ("quic", 0, TransportProtocolKind::Unknown),
            ("torii_http_range", 0, TransportProtocolKind::Unknown),
            ("vendor_reserved", 0, TransportProtocolKind::Unknown),
            ("custom", 42, TransportProtocolKind::Unknown),
            ("soranet", 1, TransportProtocolKind::ToriiHttpRange),
            ("torii", 2, TransportProtocolKind::QuicStream),
            ("quic", 3, TransportProtocolKind::SoraNetRelay),
            ("fixture", 255, TransportProtocolKind::VendorReserved),
        ] {
            let hint = TransportHint {
                protocol: label.to_owned(),
                protocol_id,
                priority: 0,
            };
            assert_eq!(hint.protocol_kind(), expected, "{label}/{protocol_id}");
        }
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
    fn provider_reports_offset_alignment_mismatch() {
        let chunk_offset = 6u64;
        let chunk_length = 8u32;
        let spec = ChunkFetchSpec {
            chunk_index: 0,
            offset: chunk_offset,
            length: chunk_length,
            digest: [0xAA; 32],
        };
        let mut metadata = ProviderMetadata::new();
        metadata.range_capability = Some(RangeCapability {
            max_chunk_span: chunk_length,
            min_granularity: 4,
            supports_sparse_offsets: false,
            requires_alignment: true,
            supports_merkle_proof: false,
        });
        let provider = FetchProvider::new("offset").with_metadata(metadata);
        match provider_can_serve_chunk(&provider, &spec) {
            Err(CapabilityMismatch::OffsetMisaligned {
                offset,
                required_alignment,
            }) => {
                assert_eq!(offset, chunk_offset);
                assert_eq!(required_alignment, 4);
            }
            other => panic!("expected offset-alignment mismatch, received {other:?}"),
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
        let profile = ChunkProfile {
            min_size: chunk_len,
            target_size: chunk_len,
            max_size: chunk_len,
            break_mask: 1,
        };
        let plan =
            CarBuildPlan::single_file_with_profile(&payload, profile).expect("weighted plan");
        assert_eq!(plan.chunks.len(), chunk_count);
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
    struct DenyAllPolicy;
    impl ScorePolicy for DenyAllPolicy {
        fn score(&self, _ctx: ProviderScoreContext<'_>) -> ProviderScoreDecision {
            ProviderScoreDecision {
                priority_delta: 0,
                allow: false,
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
    fn score_policy_rejecting_every_provider_returns_without_fetching() {
        let payload = b"policy-denied payload";
        let plan = plan_for_payload(payload);
        let providers = vec![FetchProvider::new("alpha"), FetchProvider::new("beta")];
        let fetch_calls = Arc::new(AtomicUsize::new(0));
        let fetch_calls_for_fetcher = Arc::clone(&fetch_calls);
        let fetcher = move |_req: FetchRequest| {
            let fetch_calls = Arc::clone(&fetch_calls_for_fetcher);
            async move {
                fetch_calls.fetch_add(1, Ordering::SeqCst);
                Ok::<ChunkResponse, TestError>(ChunkResponse::new(Vec::new()))
            }
        };
        let options = FetchOptions {
            score_policy: Some(Arc::new(DenyAllPolicy)),
            ..FetchOptions::default()
        };

        let error = block_on(fetch_plan_parallel(&plan, providers, fetcher, options))
            .expect_err("an all-deny score policy must fail without stalling");

        match error {
            MultiSourceError::NoPolicyEligibleProviders {
                chunk_index,
                providers,
            } => {
                assert_eq!(chunk_index, 0);
                assert_eq!(
                    providers.iter().map(ProviderId::as_str).collect::<Vec<_>>(),
                    ["alpha", "beta"]
                );
            }
            other => panic!("expected policy-eligibility error, received {other:?}"),
        }
        assert_eq!(fetch_calls.load(Ordering::SeqCst), 0);
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
        let expected: Vec<usize> = (0..outcome.chunk_receipts.len()).collect();
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
    include!("multi_fetch/resource_tests.rs");
}
