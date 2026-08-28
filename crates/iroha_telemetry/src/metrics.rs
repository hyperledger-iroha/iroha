//! [`Metrics`] and [`Status`]-related logic and functions.
#![allow(clippy::doc_markdown)]
mod manifest_status;
/// Low-cardinality metrics for the Musubi V1 package ecosystem.
pub mod musubi;
use crate::privacy::PrivacyDrainSnapshot;
use core::{
    convert::{TryFrom, TryInto},
    ops::Deref,
};
use iroha_config::{
    kura::FsyncMode,
    parameters::actual::{
        ConfidentialGas as ActualConfidentialGas, LaneRoutingPolicy as ActualLaneRoutingPolicy,
    },
};
use iroha_data_model::{
    block::consensus_v2::PERMISSIONED_TAG,
    da::types::DaRentQuote,
    nexus::MAX_ACTIVE_EXECUTION_LANES,
    offline::OfflineStatus,
    prelude::Quantity,
    soranet::privacy_metrics::{
        SoranetPrivacyBucketMetricsV1, SoranetPrivacyModeV1, SoranetPrivacySuppressionReasonV1,
    },
};
use iroha_schema::{Ident, IntoSchema, MetaMap, Metadata, TypeId, UnnamedFieldsMeta};
pub use manifest_status::{
    NexusLaneManifestValidatorBindingStatus, NexusLaneRuntimeUpgradeHookStatus,
};
use norito::{
    core::DecodeFromSlice,
    derive::{NoritoDeserialize, NoritoSerialize},
    json::{JsonDeserialize, JsonSerialize},
};
#[cfg(feature = "otel-exporter")]
use opentelemetry::{
    KeyValue,
    metrics::{Counter, Histogram as OtelHistogram, UpDownCounter},
};
use prometheus::{
    CounterVec, Encoder, Gauge, Histogram, HistogramOpts, HistogramVec, IntCounter, IntCounterVec,
    IntGauge, IntGaugeVec, Opts, Registry,
    core::{AtomicU64, GenericGauge, GenericGaugeVec},
};
pub use prometheus::{GaugeVec, core::Collector};
#[cfg(feature = "otel-exporter")]
use std::collections::HashMap;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::{
        Arc, Mutex, OnceLock, RwLock,
        atomic::{AtomicBool, AtomicU64 as StdAtomicU64, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
    vec::Vec,
};
/// Type for reporting amount of dropped messages for sumeragi
pub type DroppedMessagesCounter = IntCounter;
/// Type for reporting view change index of current round
pub type ViewChangesGauge = GenericGauge<AtomicU64>;
/// Thin wrapper around duration that `impl`s [`Default`]
#[derive(Debug, Clone, Copy)]
pub struct Uptime(pub Duration);
/// Bounded labels shared by the canonical SoraFS gateway active-request metrics.
#[derive(Debug, Clone, Copy)]
pub struct SorafsGatewayRequestMetricLabels<'a> {
    /// Stable route template, never a raw request path.
    pub endpoint: &'a str,
    /// Upper-case HTTP method.
    pub method: &'a str,
    /// Bounded gateway route variant.
    pub variant: &'a str,
    /// Negotiated chunker profile or `unknown` when it is unavailable.
    pub chunker: &'a str,
    /// Negotiated gateway profile or `unknown` when it is unavailable.
    pub profile: &'a str,
}
/// Bounded labels shared by the canonical SoraFS gateway response metrics.
#[derive(Debug, Clone, Copy)]
pub struct SorafsGatewayResponseMetricLabels<'a> {
    /// Request dimensions associated with the response.
    pub request: SorafsGatewayRequestMetricLabels<'a>,
    /// Bounded result (`success`, `error`, or `dropped`).
    pub result: &'a str,
    /// HTTP response status.
    pub status: u16,
    /// Bounded machine-readable error code, or `none` for successful responses.
    pub error_code: &'a str,
}
/// Common payload-free health values exported by a supervised SoraFS runtime.
#[derive(Debug, Clone, Copy, Default)]
pub struct SorafsRuntimeHealthMetricSnapshot {
    /// Whether the supervised runtime is live.
    pub live: bool,
    /// Whether the runtime is ready to serve committed projections.
    pub ready: bool,
    /// Whether every external dependency passed qualification and health checks.
    pub external_dependencies_ready: bool,
}
/// Payload-free journal and publication state for the SoraFS reputation runtime.
#[derive(Debug, Clone, Copy, Default)]
pub struct SorafsReputationPublicationMetricSnapshot {
    /// Whether the journal transaction submitter is qualified and ready.
    pub journal_transaction_submitter_ready: bool,
    /// Whether the current publication material is acknowledged.
    pub material_acknowledged: bool,
}
/// Payload-free values exported for the committed SoraFS reputation runtime.
#[derive(Debug, Clone, Copy, Default)]
pub struct SorafsReputationRuntimeMetricSnapshot {
    /// Common supervised-runtime health.
    pub runtime: SorafsRuntimeHealthMetricSnapshot,
    /// Journal submission and publication acknowledgement state.
    pub publication: SorafsReputationPublicationMetricSnapshot,
    /// Latest finalized height consumed by the runtime.
    pub latest_finalized_height: u64,
    /// Consecutive failed reconciliation attempts.
    pub consecutive_failures: u64,
    /// Number of providers represented by the committed projection.
    pub provider_count: u32,
}
/// Payload-free execution and finalized-projection state for hedging/billing.
#[derive(Debug, Clone, Copy, Default)]
pub struct SorafsHedgingBillingProjectionMetricSnapshot {
    /// Whether automatic hedge execution is enabled.
    pub automatic_execution_enabled: bool,
    /// Whether the most recent reconciliation tick is fresh.
    pub last_tick_fresh: bool,
    /// Whether a finalized projection is available.
    pub finalized_projection_ready: bool,
}
/// Payload-free values exported for the committed SoraFS hedging/billing runtime.
#[derive(Debug, Clone, Copy, Default)]
pub struct SorafsHedgingBillingRuntimeMetricSnapshot {
    /// Common supervised-runtime health.
    pub runtime: SorafsRuntimeHealthMetricSnapshot,
    /// Hedge execution and finalized-projection state.
    pub projection: SorafsHedgingBillingProjectionMetricSnapshot,
    /// Finalized height consumed by the service.
    pub finalized_height: u64,
    /// Current finalized ledger head height.
    pub finalized_head_height: u64,
    /// Difference between the finalized head and consumed projection.
    pub finalized_lag_blocks: u64,
    /// Next committed event sequence expected by the service.
    pub next_event_sequence: u64,
    /// Statements ready for external signing.
    pub ready_for_signing: u32,
    /// Signed statements ready for immutable publication.
    pub ready_for_publication: u32,
    /// Statements whose publication outcome requires reconciliation.
    pub publication_ambiguous: u32,
    /// Statements published immutably.
    pub published: u32,
    /// Published statements acknowledged by the authority.
    pub acknowledged: u32,
    /// Statements moved to the dead-letter queue.
    pub dead_letter: u32,
    /// Committed hedge intents retained by the projection.
    pub hedge_intents: u32,
}
type MicropaymentSampleSink = Arc<
    dyn Fn(&str, MicropaymentCreditSnapshot, MicropaymentTicketCounters) + Send + Sync + 'static,
>;
const SORAFS_REPUTATION_SCORE_LABEL_LIMIT: usize = 100;
const SORAFS_ORDERBOOK_EVENT_LABELS: [&str; 8] = [
    "policy_activated",
    "order_admitted",
    "order_cancelled",
    "trade_matched",
    "order_expired",
    "order_provider_revoked",
    "channel_expired",
    "receipt_recorded",
];
const SORAFS_ORDERBOOK_TIER_LABELS: [&str; 3] = ["hot", "warm", "archive"];
const SORAFS_ORDERBOOK_SIDE_LABELS: [&str; 2] = ["bid", "ask"];
const SORAFS_ORDERBOOK_PROJECTION_FAILURE_LABELS: [&str; 11] = [
    "telemetry_unavailable",
    "finalized_view_unavailable",
    "query_failed",
    "invalid_event_page",
    "invalid_order_page",
    "invalid_channel_page",
    "arithmetic_overflow",
    "order_capacity_exceeded",
    "channel_capacity_exceeded",
    "projection_mismatch",
    "other",
];
const SORAFS_ORDERBOOK_API_ROUTE_LABELS: [&str; 10] = [
    "orders",
    "cancel",
    "receipts",
    "book",
    "trades",
    "channels",
    "events",
    "events_stream",
    "events_ws",
    "other",
];
const SORAFS_ORDERBOOK_API_OUTCOME_LABELS: [&str; 2] = ["success", "error"];
const SORAFS_GATEWAY_COMPLIANCE_OPERATION_LABELS: [&str; 7] = [
    "feed",
    "status",
    "stage",
    "acknowledge",
    "promote",
    "rollback",
    "other",
];
const SORAFS_GATEWAY_COMPLIANCE_REQUEST_OUTCOME_LABELS: [&str; 8] = [
    "success",
    "authentication_failed",
    "authorization_failed",
    "invalid_request",
    "not_found",
    "conflict",
    "unavailable",
    "internal_error",
];
const SORAFS_GATEWAY_COMPLIANCE_SUBJECT_KIND_LABELS: [&str; 5] =
    ["provider", "manifest_digest", "cid", "url", "other"];
const SORAFS_GATEWAY_COMPLIANCE_DISPOSITION_LABELS: [&str; 3] = ["allow", "deny", "other"];
const SORAFS_GATEWAY_COMPLIANCE_DECISION_SOURCE_LABELS: [&str; 5] = [
    "no_match",
    "baseline",
    "accepted_appeal",
    "legal_safety_hold",
    "other",
];
const SORAFS_GATEWAY_COMPLIANCE_FAILURE_SURFACE_LABELS: [&str; 5] =
    ["control", "serving", "feed_sync", "startup", "other"];
const SORAFS_GATEWAY_COMPLIANCE_FAILURE_CLASS_LABELS: [&str; 10] = [
    "authentication",
    "authorization",
    "invalid_request",
    "not_found",
    "conflict",
    "unavailable",
    "expired_catalog",
    "upstream",
    "persistence",
    "internal",
];
fn current_unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or_default()
}
impl Default for Uptime {
    fn default() -> Self {
        Self(Duration::from_millis(0))
    }
}
impl norito::core::NoritoSerialize for Uptime {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        let pair = (self.0.as_secs(), self.0.subsec_nanos());
        norito::core::NoritoSerialize::serialize(&pair, writer)
    }
}
impl<'a> norito::core::NoritoDeserialize<'a> for Uptime {
    fn deserialize(archived: &'a norito::core::Archived<Uptime>) -> Self {
        let (secs, nanos): (u64, u32) =
            norito::core::NoritoDeserialize::deserialize(archived.cast());
        Uptime(Duration::from_secs(secs) + Duration::from_nanos(u64::from(nanos)))
    }
}
/// Snapshot of the configured stack settings for scheduler/prover pools and the guest VM.
#[derive(Clone, Copy, Debug, Default)]
pub struct StackSettingsSnapshot {
    /// Requested scheduler stack size in bytes.
    pub requested_scheduler_bytes: u64,
    /// Requested prover stack size in bytes.
    pub requested_prover_bytes: u64,
    /// Requested guest stack size in bytes.
    pub requested_guest_bytes: u64,
    /// Applied scheduler stack size in bytes after clamping.
    pub scheduler_bytes: u64,
    /// Applied prover stack size in bytes after clamping.
    pub prover_bytes: u64,
    /// Applied guest stack size in bytes after clamping.
    pub guest_bytes: u64,
    /// Whether the scheduler stack request was clamped to the allowed range.
    pub scheduler_clamped: bool,
    /// Whether the prover stack request was clamped to the allowed range.
    pub prover_clamped: bool,
    /// Whether the guest stack request was clamped to the allowed range.
    pub guest_clamped: bool,
    /// Count of times we fell back to an existing Rayon pool instead of honouring the requested stack.
    pub pool_fallback_total: u64,
    /// Count of times the guest stack budget was hit while constructing a VM memory image.
    pub budget_hit_total: u64,
    /// Gas→stack multiplier currently in effect.
    pub gas_to_stack_multiplier: u64,
}
static STACK_REQUESTED_SCHEDULER_BYTES: StdAtomicU64 = StdAtomicU64::new(0);
static STACK_REQUESTED_PROVER_BYTES: StdAtomicU64 = StdAtomicU64::new(0);
static STACK_REQUESTED_GUEST_BYTES: StdAtomicU64 = StdAtomicU64::new(0);
static STACK_APPLIED_SCHEDULER_BYTES: StdAtomicU64 = StdAtomicU64::new(0);
static STACK_APPLIED_PROVER_BYTES: StdAtomicU64 = StdAtomicU64::new(0);
static STACK_APPLIED_GUEST_BYTES: StdAtomicU64 = StdAtomicU64::new(0);
static STACK_SCHEDULER_CLAMPED: StdAtomicU64 = StdAtomicU64::new(0);
static STACK_PROVER_CLAMPED: StdAtomicU64 = StdAtomicU64::new(0);
static STACK_GUEST_CLAMPED: StdAtomicU64 = StdAtomicU64::new(0);
static STACK_POOL_FALLBACK_TOTAL: StdAtomicU64 = StdAtomicU64::new(0);
static STACK_BUDGET_HIT_TOTAL: StdAtomicU64 = StdAtomicU64::new(0);
static STACK_GAS_TO_STACK_MULTIPLIER: StdAtomicU64 = StdAtomicU64::new(0);
/// Record the latest requested/applied stack settings.
pub fn record_stack_limits(snapshot: StackSettingsSnapshot) {
    STACK_REQUESTED_SCHEDULER_BYTES.store(snapshot.requested_scheduler_bytes, Ordering::Relaxed);
    STACK_REQUESTED_PROVER_BYTES.store(snapshot.requested_prover_bytes, Ordering::Relaxed);
    STACK_REQUESTED_GUEST_BYTES.store(snapshot.requested_guest_bytes, Ordering::Relaxed);
    STACK_APPLIED_SCHEDULER_BYTES.store(snapshot.scheduler_bytes, Ordering::Relaxed);
    STACK_APPLIED_PROVER_BYTES.store(snapshot.prover_bytes, Ordering::Relaxed);
    STACK_APPLIED_GUEST_BYTES.store(snapshot.guest_bytes, Ordering::Relaxed);
    STACK_SCHEDULER_CLAMPED.store(u64::from(snapshot.scheduler_clamped), Ordering::Relaxed);
    STACK_PROVER_CLAMPED.store(u64::from(snapshot.prover_clamped), Ordering::Relaxed);
    STACK_GUEST_CLAMPED.store(u64::from(snapshot.guest_clamped), Ordering::Relaxed);
    STACK_POOL_FALLBACK_TOTAL.store(snapshot.pool_fallback_total, Ordering::Relaxed);
    STACK_BUDGET_HIT_TOTAL.store(snapshot.budget_hit_total, Ordering::Relaxed);
    if snapshot.gas_to_stack_multiplier != 0 {
        STACK_GAS_TO_STACK_MULTIPLIER
            .store(snapshot.gas_to_stack_multiplier.max(1), Ordering::Relaxed);
    }
    if let Some(metrics) = global() {
        metrics.apply_stack_snapshot(&stack_settings_snapshot());
    }
}
/// Record a change to the gas→stack multiplier used to derive guest stack limits.
pub fn record_stack_gas_multiplier(multiplier: u64) {
    STACK_GAS_TO_STACK_MULTIPLIER.store(multiplier.max(1), Ordering::Relaxed);
    if let Some(metrics) = global() {
        metrics.apply_stack_snapshot(&stack_settings_snapshot());
    }
}
/// Increment the counter tracking fallbacks to an already-initialised Rayon pool.
pub fn record_stack_pool_fallback() {
    STACK_POOL_FALLBACK_TOTAL.fetch_add(1, Ordering::Relaxed);
    if let Some(metrics) = global() {
        metrics.apply_stack_snapshot(&stack_settings_snapshot());
    }
}
/// Increment the counter tracking guest stack budget clamps at VM construction time.
pub fn record_stack_budget_hit() {
    STACK_BUDGET_HIT_TOTAL.fetch_add(1, Ordering::Relaxed);
    if let Some(metrics) = global() {
        metrics.apply_stack_snapshot(&stack_settings_snapshot());
    }
}
/// Snapshot the most recent stack settings for status/metric exports.
pub fn stack_settings_snapshot() -> StackSettingsSnapshot {
    StackSettingsSnapshot {
        requested_scheduler_bytes: STACK_REQUESTED_SCHEDULER_BYTES.load(Ordering::Relaxed),
        requested_prover_bytes: STACK_REQUESTED_PROVER_BYTES.load(Ordering::Relaxed),
        requested_guest_bytes: STACK_REQUESTED_GUEST_BYTES.load(Ordering::Relaxed),
        scheduler_bytes: STACK_APPLIED_SCHEDULER_BYTES.load(Ordering::Relaxed),
        prover_bytes: STACK_APPLIED_PROVER_BYTES.load(Ordering::Relaxed),
        guest_bytes: STACK_APPLIED_GUEST_BYTES.load(Ordering::Relaxed),
        scheduler_clamped: STACK_SCHEDULER_CLAMPED.load(Ordering::Relaxed) != 0,
        prover_clamped: STACK_PROVER_CLAMPED.load(Ordering::Relaxed) != 0,
        guest_clamped: STACK_GUEST_CLAMPED.load(Ordering::Relaxed) != 0,
        pool_fallback_total: STACK_POOL_FALLBACK_TOTAL.load(Ordering::Relaxed),
        budget_hit_total: STACK_BUDGET_HIT_TOTAL.load(Ordering::Relaxed),
        gas_to_stack_multiplier: STACK_GAS_TO_STACK_MULTIPLIER.load(Ordering::Relaxed),
    }
}
/// Helper container for fixed-size scheduler histogram buckets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LayerWidthBuckets([u64; 8]);
impl LayerWidthBuckets {
    /// Construct buckets directly from an array.
    pub const fn new(values: [u64; 8]) -> Self {
        Self(values)
    }
    /// Build buckets from a slice, truncating to the first eight entries.
    pub fn from_slice(values: &[u64]) -> Self {
        let mut buckets = [0_u64; 8];
        let len = values.len().min(8);
        buckets[..len].copy_from_slice(&values[..len]);
        Self(buckets)
    }
    /// Borrow the underlying bucket array.
    pub const fn as_array(&self) -> &[u64; 8] {
        &self.0
    }
    /// Consume the wrapper, returning the inner bucket array.
    pub const fn into_inner(self) -> [u64; 8] {
        self.0
    }
}
impl From<[u64; 8]> for LayerWidthBuckets {
    fn from(values: [u64; 8]) -> Self {
        Self(values)
    }
}
impl From<LayerWidthBuckets> for [u64; 8] {
    fn from(value: LayerWidthBuckets) -> Self {
        value.0
    }
}
impl norito::core::NoritoSerialize for LayerWidthBuckets {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        let payload = (
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5], self.0[6], self.0[7],
        );
        norito::core::NoritoSerialize::serialize(&payload, writer)
    }
}
impl<'a> norito::core::NoritoDeserialize<'a> for LayerWidthBuckets {
    fn deserialize(archived: &'a norito::core::Archived<Self>) -> Self {
        let payload: (u64, u64, u64, u64, u64, u64, u64, u64) =
            norito::core::NoritoDeserialize::deserialize(archived.cast());
        Self([
            payload.0, payload.1, payload.2, payload.3, payload.4, payload.5, payload.6, payload.7,
        ])
    }
}
impl<'a> DecodeFromSlice<'a> for LayerWidthBuckets {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let (payload, used) = <(u64, u64, u64, u64, u64, u64, u64, u64)>::decode_from_slice(bytes)?;
        Ok((
            Self([
                payload.0, payload.1, payload.2, payload.3, payload.4, payload.5, payload.6,
                payload.7,
            ]),
            used,
        ))
    }
}
impl<'a> DecodeFromSlice<'a> for Uptime {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let ((secs, nanos), used) = <(u64, u32)>::decode_from_slice(bytes)?;
        let duration =
            Duration::from_secs(secs).saturating_add(Duration::from_nanos(u64::from(nanos)));
        Ok((Uptime(duration), used))
    }
}
/// OpenTelemetry instrumentation for multi-source orchestrator metrics.
#[cfg_attr(not(feature = "otel-exporter"), derive(Copy))]
#[derive(Clone)]
pub struct SorafsFetchOtel {
    #[cfg(feature = "otel-exporter")]
    active_fetches: UpDownCounter<i64>,
    #[cfg(feature = "otel-exporter")]
    duration_ms: OtelHistogram<f64>,
    #[cfg(feature = "otel-exporter")]
    failures_total: Counter<u64>,
    #[cfg(feature = "otel-exporter")]
    retries_total: Counter<u64>,
    #[cfg(feature = "otel-exporter")]
    provider_failures_total: Counter<u64>,
    #[cfg(feature = "otel-exporter")]
    chunk_latency_ms: OtelHistogram<f64>,
    #[cfg(feature = "otel-exporter")]
    bytes_total: Counter<u64>,
    #[cfg(feature = "otel-exporter")]
    stalls_total: Counter<u64>,
    #[cfg(feature = "otel-exporter")]
    policy_events_total: Counter<u64>,
    #[cfg(feature = "otel-exporter")]
    pq_ratio: OtelHistogram<f64>,
    #[cfg(feature = "otel-exporter")]
    pq_candidate_ratio: OtelHistogram<f64>,
    #[cfg(feature = "otel-exporter")]
    pq_deficit_ratio: OtelHistogram<f64>,
    #[cfg(feature = "otel-exporter")]
    classical_ratio: OtelHistogram<f64>,
    #[cfg(feature = "otel-exporter")]
    classical_selected: OtelHistogram<f64>,
    #[cfg(feature = "otel-exporter")]
    brownouts_total: Counter<u64>,
    #[cfg(feature = "otel-exporter")]
    transport_events_total: Counter<u64>,
}
impl Default for SorafsFetchOtel {
    fn default() -> Self {
        Self::new()
    }
}
#[allow(clippy::unused_self, clippy::trivially_copy_pass_by_ref)] // retain &self API for OTEL-enabled builds
impl SorafsFetchOtel {
    /// Create a new OTEL instrumentation bundle.
    #[must_use]
    pub fn new() -> Self {
        #[cfg(feature = "otel-exporter")]
        {
            let meter = opentelemetry::global::meter("sorafs.fetch");
            let active_fetches = meter
                .i64_up_down_counter("sorafs.fetch.active")
                .with_description("Active SoraFS orchestrator fetch sessions.")
                .with_unit("sessions")
                .build();
            let duration_ms = meter
                .f64_histogram("sorafs.fetch.duration_ms")
                .with_description("Completed fetch duration in milliseconds.")
                .with_unit("ms")
                .build();
            let failures_total = meter
                .u64_counter("sorafs.fetch.failures_total")
                .with_description("Total number of orchestrator failures grouped by reason.")
                .build();
            let retries_total = meter
                .u64_counter("sorafs.fetch.retries_total")
                .with_description("Retry attempts triggered during orchestrator sessions.")
                .build();
            let provider_failures_total = meter
                .u64_counter("sorafs.fetch.provider_failures_total")
                .with_description("Provider-level failures observed while fetching chunks.")
                .build();
            let chunk_latency_ms = meter
                .f64_histogram("sorafs.fetch.chunk_latency_ms")
                .with_description("Latency per chunk fetch served by the orchestrator.")
                .with_unit("ms")
                .build();
            let bytes_total = meter
                .u64_counter("sorafs.fetch.bytes_total")
                .with_description("Total bytes delivered by the orchestrator grouped by provider.")
                .build();
            let stalls_total = meter
                .u64_counter("sorafs.fetch.stalls_total")
                .with_description("Chunks exceeding the configured latency cap.")
                .build();
            let policy_events_total = meter
                .u64_counter("sorafs.fetch.anonymity_events_total")
                .with_description("Anonymity policy events grouped by stage/outcome/reason.")
                .build();
            let pq_ratio = meter
                .f64_histogram("sorafs.fetch.pq_ratio")
                .with_description("PQ-capable relay ratio observed per session.")
                .with_unit("ratio")
                .build();
            let pq_candidate_ratio = meter
                .f64_histogram("sorafs.fetch.pq_candidate_ratio")
                .with_description("PQ-capable relay candidate ratio observed per session.")
                .with_unit("ratio")
                .build();
            let pq_deficit_ratio = meter
                .f64_histogram("sorafs.fetch.pq_deficit_ratio")
                .with_description("PQ policy shortfall ratio observed per session.")
                .with_unit("ratio")
                .build();
            let classical_ratio = meter
                .f64_histogram("sorafs.fetch.classical_ratio")
                .with_description("Classical relay selection ratio observed per session.")
                .with_unit("ratio")
                .build();
            let classical_selected = meter
                .f64_histogram("sorafs.fetch.classical_selected")
                .with_description("Classical relay selections observed per session.")
                .with_unit("relays")
                .build();
            let brownouts_total = meter
                .u64_counter("sorafs.fetch.brownouts_total")
                .with_description("Anonymity policy brownout events grouped by stage/reason.")
                .build();
            let transport_events_total = meter
                .u64_counter("sorafs.fetch.transport_events_total")
                .with_description("Transport events emitted by the orchestrator grouped by protocol/event/reason.")
                .build();
            Self {
                active_fetches,
                duration_ms,
                failures_total,
                retries_total,
                provider_failures_total,
                chunk_latency_ms,
                bytes_total,
                stalls_total,
                policy_events_total,
                pq_ratio,
                pq_candidate_ratio,
                pq_deficit_ratio,
                classical_ratio,
                classical_selected,
                brownouts_total,
                transport_events_total,
            }
        }
        #[cfg(not(feature = "otel-exporter"))]
        {
            Self {}
        }
    }
    /// Record fetch start for the manifest/region/job tuple.
    pub fn fetch_started(&self, manifest_id: &str, region: &str, job_id: &str) {
        #[cfg(feature = "otel-exporter")]
        {
            self.active_fetches.add(
                1,
                &self.manifest_attributes(manifest_id, region, Some(job_id)),
            );
        }
        let _ = (self, manifest_id, region, job_id);
    }
    /// Record fetch completion for the manifest/region/job tuple.
    pub fn fetch_finished(&self, manifest_id: &str, region: &str, job_id: &str) {
        #[cfg(feature = "otel-exporter")]
        {
            self.active_fetches.add(
                -1,
                &self.manifest_attributes(manifest_id, region, Some(job_id)),
            );
        }
        let _ = (self, manifest_id, region, job_id);
    }
    /// Record fetch duration (ms).
    pub fn record_duration(&self, manifest_id: &str, region: &str, job_id: &str, duration_ms: f64) {
        #[cfg(feature = "otel-exporter")]
        {
            self.duration_ms.record(
                duration_ms,
                &self.manifest_attributes(manifest_id, region, Some(job_id)),
            );
        }
        let _ = (self, manifest_id, region, job_id, duration_ms);
    }
    /// Increment failure counter.
    pub fn record_failure(
        &self,
        manifest_id: &str,
        region: &str,
        job_id: Option<&str>,
        reason: &str,
    ) {
        #[cfg(feature = "otel-exporter")]
        {
            let mut attrs = self.manifest_attributes(manifest_id, region, job_id);
            attrs.push(KeyValue::new("failure_reason", reason.to_string()));
            self.failures_total.add(1, &attrs);
        }
        let _ = (self, manifest_id, region, job_id, reason);
    }
    /// Increment retry counter.
    pub fn record_retries(
        &self,
        manifest_id: &str,
        region: &str,
        job_id: Option<&str>,
        provider_id: &str,
        reason: &str,
        count: u64,
    ) {
        #[cfg(feature = "otel-exporter")]
        {
            if count > 0 {
                let mut attrs = self.manifest_attributes(manifest_id, region, job_id);
                attrs.push(KeyValue::new("provider_id", provider_id.to_string()));
                attrs.push(KeyValue::new("retry_reason", reason.to_string()));
                self.retries_total.add(count, &attrs);
            }
        }
        let _ = (
            self,
            manifest_id,
            region,
            job_id,
            provider_id,
            reason,
            count,
        );
    }
    /// Record an anonymity policy event.
    pub fn record_policy_event(
        &self,
        manifest_id: &str,
        region: &str,
        job_id: Option<&str>,
        stage: &str,
        outcome: &str,
        reason: &str,
    ) {
        #[cfg(feature = "otel-exporter")]
        {
            let mut attrs = self.manifest_attributes(manifest_id, region, job_id);
            attrs.push(KeyValue::new("stage", stage.to_string()));
            attrs.push(KeyValue::new("outcome", outcome.to_string()));
            attrs.push(KeyValue::new("reason", reason.to_string()));
            self.policy_events_total.add(1, &attrs);
        }
        let _ = (self, manifest_id, region, job_id, stage, outcome, reason);
    }
    /// Record a transport event emitted by the orchestrator.
    pub fn record_transport_event(
        &self,
        manifest_id: &str,
        region: &str,
        job_id: Option<&str>,
        protocol: &str,
        event: &str,
        reason: &str,
    ) {
        #[cfg(feature = "otel-exporter")]
        {
            let mut attrs = self.manifest_attributes(manifest_id, region, job_id);
            attrs.push(KeyValue::new("protocol", protocol.to_string()));
            attrs.push(KeyValue::new("transport_event", event.to_string()));
            attrs.push(KeyValue::new("transport_reason", reason.to_string()));
            self.transport_events_total.add(1, &attrs);
        }
        let _ = (self, manifest_id, region, job_id, protocol, event, reason);
    }
    /// Record the observed PQ-capable relay ratio for a session.
    pub fn record_pq_ratio(
        &self,
        manifest_id: &str,
        region: &str,
        job_id: Option<&str>,
        stage: &str,
        ratio: f64,
    ) {
        #[cfg(feature = "otel-exporter")]
        {
            let mut attrs = self.manifest_attributes(manifest_id, region, job_id);
            attrs.push(KeyValue::new("stage", stage.to_string()));
            self.pq_ratio.record(ratio.clamp(0.0, 1.0), &attrs);
        }
        let _ = (self, manifest_id, region, job_id, stage, ratio);
    }
    /// Record the PQ-capable candidate ratio for a session.
    pub fn record_pq_candidate_ratio(
        &self,
        manifest_id: &str,
        region: &str,
        job_id: Option<&str>,
        stage: &str,
        ratio: f64,
    ) {
        #[cfg(feature = "otel-exporter")]
        {
            let mut attrs = self.manifest_attributes(manifest_id, region, job_id);
            attrs.push(KeyValue::new("stage", stage.to_string()));
            self.pq_candidate_ratio
                .record(ratio.clamp(0.0, 1.0), &attrs);
        }
        let _ = (self, manifest_id, region, job_id, stage, ratio);
    }
    /// Record the PQ policy shortfall ratio for a session.
    pub fn record_pq_deficit_ratio(
        &self,
        manifest_id: &str,
        region: &str,
        job_id: Option<&str>,
        stage: &str,
        ratio: f64,
    ) {
        #[cfg(feature = "otel-exporter")]
        {
            let mut attrs = self.manifest_attributes(manifest_id, region, job_id);
            attrs.push(KeyValue::new("stage", stage.to_string()));
            self.pq_deficit_ratio.record(ratio.clamp(0.0, 1.0), &attrs);
        }
        let _ = (self, manifest_id, region, job_id, stage, ratio);
    }
    /// Record the classical relay ratio for a session.
    pub fn record_classical_ratio(
        &self,
        manifest_id: &str,
        region: &str,
        job_id: Option<&str>,
        stage: &str,
        ratio: f64,
    ) {
        #[cfg(feature = "otel-exporter")]
        {
            let mut attrs = self.manifest_attributes(manifest_id, region, job_id);
            attrs.push(KeyValue::new("stage", stage.to_string()));
            self.classical_ratio.record(ratio.clamp(0.0, 1.0), &attrs);
        }
        let _ = (self, manifest_id, region, job_id, stage, ratio);
    }
    /// Record the number of classical relays selected for a session.
    pub fn record_classical_selected(
        &self,
        manifest_id: &str,
        region: &str,
        job_id: Option<&str>,
        stage: &str,
        selected: u64,
    ) {
        #[cfg(feature = "otel-exporter")]
        {
            let mut attrs = self.manifest_attributes(manifest_id, region, job_id);
            attrs.push(KeyValue::new("stage", stage.to_string()));
            self.classical_selected.record(selected as f64, &attrs);
        }
        let _ = (self, manifest_id, region, job_id, stage, selected);
    }
    /// Record an anonymity policy brownout event.
    pub fn record_brownout_event(
        &self,
        manifest_id: &str,
        region: &str,
        job_id: Option<&str>,
        stage: &str,
        reason: &str,
    ) {
        #[cfg(feature = "otel-exporter")]
        {
            let mut attrs = self.manifest_attributes(manifest_id, region, job_id);
            attrs.push(KeyValue::new("stage", stage.to_string()));
            attrs.push(KeyValue::new("reason", reason.to_string()));
            self.brownouts_total.add(1, &attrs);
        }
        let _ = (self, manifest_id, region, job_id, stage, reason);
    }
    /// Increment provider failure counter.
    pub fn record_provider_failure(
        &self,
        manifest_id: &str,
        region: &str,
        job_id: Option<&str>,
        provider_id: &str,
        reason: &str,
        count: u64,
    ) {
        #[cfg(feature = "otel-exporter")]
        {
            if count > 0 {
                let mut attrs = self.manifest_attributes(manifest_id, region, job_id);
                attrs.push(KeyValue::new("provider_id", provider_id.to_string()));
                attrs.push(KeyValue::new("failure_reason", reason.to_string()));
                self.provider_failures_total.add(count, &attrs);
            }
        }
        let _ = (
            self,
            manifest_id,
            region,
            job_id,
            provider_id,
            reason,
            count,
        );
    }
    /// Record per-chunk latency (milliseconds).
    pub fn record_chunk_latency(
        &self,
        manifest_id: &str,
        region: &str,
        job_id: Option<&str>,
        provider_id: &str,
        latency_ms: f64,
    ) {
        #[cfg(feature = "otel-exporter")]
        {
            let mut attrs = self.manifest_attributes(manifest_id, region, job_id);
            attrs.push(KeyValue::new("provider_id", provider_id.to_string()));
            self.chunk_latency_ms.record(latency_ms, &attrs);
        }
        let _ = (self, manifest_id, region, job_id, provider_id, latency_ms);
    }
    /// Record bytes delivered for a chunk.
    pub fn record_bytes(
        &self,
        manifest_id: &str,
        region: &str,
        job_id: Option<&str>,
        provider_id: &str,
        bytes: u64,
    ) {
        #[cfg(feature = "otel-exporter")]
        {
            if bytes > 0 {
                let mut attrs = self.manifest_attributes(manifest_id, region, job_id);
                attrs.push(KeyValue::new("provider_id", provider_id.to_string()));
                self.bytes_total.add(bytes, &attrs);
            }
        }
        let _ = (self, manifest_id, region, job_id, provider_id, bytes);
    }
    /// Increment stall counter for latency cap breaches.
    pub fn record_stall(
        &self,
        manifest_id: &str,
        region: &str,
        job_id: Option<&str>,
        provider_id: &str,
    ) {
        #[cfg(feature = "otel-exporter")]
        {
            let mut attrs = self.manifest_attributes(manifest_id, region, job_id);
            attrs.push(KeyValue::new("provider_id", provider_id.to_string()));
            self.stalls_total.add(1, &attrs);
        }
        let _ = (self, manifest_id, region, job_id, provider_id);
    }
    #[cfg(feature = "otel-exporter")]
    fn manifest_attributes(
        &self,
        manifest_id: &str,
        region: &str,
        job_id: Option<&str>,
    ) -> Vec<KeyValue> {
        let mut attrs = Vec::with_capacity(if job_id.is_some() { 3 } else { 2 });
        attrs.push(KeyValue::new("manifest_id", manifest_id.to_string()));
        attrs.push(KeyValue::new("region", region.to_string()));
        if let Some(job_id) = job_id {
            attrs.push(KeyValue::new("job_id", job_id.to_string()));
        }
        attrs
    }
}
/// OpenTelemetry instrumentation for FASTPQ execution mode resolutions.
#[cfg_attr(not(feature = "otel-exporter"), derive(Copy))]
#[derive(Clone)]
pub struct FastpqOtel {
    #[cfg(feature = "otel-exporter")]
    execution_mode_resolutions_total: Counter<u64>,
    #[cfg(feature = "otel-exporter")]
    poseidon_pipeline_resolutions_total: Counter<u64>,
}
impl Default for FastpqOtel {
    fn default() -> Self {
        Self::new()
    }
}
#[allow(clippy::unused_self)]
impl FastpqOtel {
    /// Create a new FASTPQ instrumentation bundle.
    #[must_use]
    pub fn new() -> Self {
        #[cfg(feature = "otel-exporter")]
        {
            let meter = opentelemetry::global::meter("fastpq.prover");
            let execution_mode_resolutions_total = meter
                .u64_counter("fastpq.execution_mode_resolutions_total")
                .with_description(
                    "FASTPQ execution mode resolutions (labels: requested, resolved, backend, device_class, chip_family, gpu_kind).",
                )
                .build();
            let poseidon_pipeline_resolutions_total = meter
                .u64_counter("fastpq.poseidon_pipeline_resolutions_total")
                .with_description(
                    "FASTPQ Poseidon pipeline resolutions (labels: requested, resolved, path, device_class, chip_family, gpu_kind).",
                )
                .build();
            Self {
                execution_mode_resolutions_total,
                poseidon_pipeline_resolutions_total,
            }
        }
        #[cfg(not(feature = "otel-exporter"))]
        {
            Self {}
        }
    }
    /// Record a FASTPQ execution mode resolution.
    #[cfg_attr(
        not(feature = "otel-exporter"),
        allow(clippy::trivially_copy_pass_by_ref)
    )]
    pub fn record_execution_mode(
        &self,
        requested: &str,
        resolved: &str,
        backend: &str,
        device_class: &str,
        chip_family: &str,
        gpu_kind: &str,
    ) {
        #[cfg(feature = "otel-exporter")]
        {
            self.execution_mode_resolutions_total.add(
                1,
                &[
                    KeyValue::new("requested", requested.to_owned()),
                    KeyValue::new("resolved", resolved.to_owned()),
                    KeyValue::new("backend", backend.to_owned()),
                    KeyValue::new("device_class", device_class.to_owned()),
                    KeyValue::new("chip_family", chip_family.to_owned()),
                    KeyValue::new("gpu_kind", gpu_kind.to_owned()),
                ],
            );
        }
        let _ = (
            self,
            requested,
            resolved,
            backend,
            device_class,
            chip_family,
            gpu_kind,
        );
    }
    /// Record a Poseidon pipeline resolution event.
    #[cfg_attr(
        not(feature = "otel-exporter"),
        allow(clippy::trivially_copy_pass_by_ref)
    )]
    pub fn record_poseidon_pipeline(
        &self,
        requested: &str,
        resolved: &str,
        path: &str,
        device_class: &str,
        chip_family: &str,
        gpu_kind: &str,
    ) {
        #[cfg(feature = "otel-exporter")]
        {
            self.poseidon_pipeline_resolutions_total.add(
                1,
                &[
                    KeyValue::new("requested", requested.to_owned()),
                    KeyValue::new("resolved", resolved.to_owned()),
                    KeyValue::new("path", path.to_owned()),
                    KeyValue::new("device_class", device_class.to_owned()),
                    KeyValue::new("chip_family", chip_family.to_owned()),
                    KeyValue::new("gpu_kind", gpu_kind.to_owned()),
                ],
            );
        }
        let _ = (
            self,
            requested,
            resolved,
            path,
            device_class,
            chip_family,
            gpu_kind,
        );
    }
}
/// Snapshot of a Metal queue lane captured by the FASTPQ runtime.
#[derive(Clone, Copy, Debug, Default)]
pub struct FastpqMetalQueueLaneSample {
    /// Zero-based lane index.
    pub index: usize,
    /// Number of dispatches observed during the sample window.
    pub dispatch_count: u64,
    /// Maximum concurrent command buffers observed for the lane.
    pub max_in_flight: u64,
    /// Milliseconds the lane spent executing commands.
    pub busy_ms: f64,
    /// Milliseconds this lane overlapped with other queues.
    pub overlap_ms: f64,
}
/// Aggregate Metal queue telemetry collected from the FASTPQ runtime.
#[derive(Clone, Debug)]
pub struct FastpqMetalQueueSample<'a> {
    /// Command semaphore limit for the device.
    pub limit: u64,
    /// Maximum number of in-flight buffers observed across all queues.
    pub max_in_flight: u64,
    /// Total dispatches recorded during the window.
    pub dispatch_count: u64,
    /// Sampling window length in milliseconds.
    pub window_ms: f64,
    /// Milliseconds spent executing commands across all queues.
    pub busy_ms: f64,
    /// Milliseconds spent overlapping GPU work across queues.
    pub overlap_ms: f64,
    /// Per-lane samples collected during the window.
    pub lanes: &'a [FastpqMetalQueueLaneSample],
}
/// OpenTelemetry instrumentation for repair scheduler metrics.
#[cfg_attr(not(feature = "otel-exporter"), derive(Copy))]
#[derive(Clone)]
pub struct SorafsRepairOtel {
    #[cfg(feature = "otel-exporter")]
    tasks_total: Counter<u64>,
    #[cfg(feature = "otel-exporter")]
    latency_minutes: OtelHistogram<f64>,
    #[cfg(feature = "otel-exporter")]
    backlog_oldest_age_seconds: OtelHistogram<f64>,
    #[cfg(feature = "otel-exporter")]
    queue_depth: OtelHistogram<f64>,
    #[cfg(feature = "otel-exporter")]
    lease_expired_total: Counter<u64>,
    #[cfg(feature = "otel-exporter")]
    slash_proposals_total: Counter<u64>,
}
impl Default for SorafsRepairOtel {
    fn default() -> Self {
        Self::new()
    }
}
#[allow(clippy::unused_self, clippy::trivially_copy_pass_by_ref)]
impl SorafsRepairOtel {
    /// Create a new instrumentation bundle for repair automation.
    #[must_use]
    pub fn new() -> Self {
        #[cfg(feature = "otel-exporter")]
        {
            let meter = opentelemetry::global::meter("sorafs.repair");
            let tasks_total = meter
                .u64_counter("sorafs.repair.tasks_total")
                .with_description("SoraFS repair task transitions grouped by status.")
                .build();
            let latency_minutes = meter
                .f64_histogram("sorafs.repair.latency_minutes")
                .with_description("SoraFS repair lifecycle latency in minutes.")
                .with_unit("min")
                .build();
            let backlog_oldest_age_seconds = meter
                .f64_histogram("sorafs.repair.backlog_oldest_age_seconds")
                .with_description("Age in seconds of the oldest queued SoraFS repair task.")
                .with_unit("s")
                .build();
            let queue_depth = meter
                .f64_histogram("sorafs.repair.queue_depth")
                .with_description("SoraFS repair queue depth per provider.")
                .with_unit("tasks")
                .build();
            let lease_expired_total = meter
                .u64_counter("sorafs.repair.lease_expired_total")
                .with_description("SoraFS repair lease expirations grouped by outcome.")
                .build();
            let slash_proposals_total = meter
                .u64_counter("sorafs.repair.slash_proposals_total")
                .with_description("SoraFS repair slash proposals grouped by outcome.")
                .build();
            Self {
                tasks_total,
                latency_minutes,
                backlog_oldest_age_seconds,
                queue_depth,
                lease_expired_total,
                slash_proposals_total,
            }
        }
        #[cfg(not(feature = "otel-exporter"))]
        {
            Self {}
        }
    }
    /// Record a task transition for the given status label.
    pub fn record_task_transition(&self, status: &'static str) {
        #[cfg(feature = "otel-exporter")]
        {
            self.tasks_total.add(
                1,
                &[opentelemetry::KeyValue::new("status", status.to_owned())],
            );
        }
        let _ = status;
    }
    /// Record repair latency in minutes for the supplied outcome label.
    pub fn record_latency(&self, minutes: f64, outcome: &'static str) {
        #[cfg(feature = "otel-exporter")]
        {
            self.latency_minutes.record(
                minutes,
                &[opentelemetry::KeyValue::new("outcome", outcome.to_owned())],
            );
        }
        let _ = (minutes, outcome);
    }
    /// Record the oldest queued repair task age in seconds.
    pub fn record_backlog_oldest_age_seconds(&self, age_secs: f64) {
        #[cfg(feature = "otel-exporter")]
        {
            self.backlog_oldest_age_seconds.record(age_secs, &[]);
        }
        let _ = age_secs;
    }
    /// Record the current repair queue depth for the supplied provider.
    pub fn record_queue_depth(&self, depth: u64, provider: &str) {
        #[cfg(feature = "otel-exporter")]
        {
            self.queue_depth.record(
                depth as f64,
                &[opentelemetry::KeyValue::new(
                    "provider",
                    provider.to_owned(),
                )],
            );
        }
        let _ = (depth, provider);
    }
    /// Record a lease expiry event for the supplied outcome label.
    pub fn record_lease_expired(&self, outcome: &'static str) {
        #[cfg(feature = "otel-exporter")]
        {
            self.lease_expired_total.add(
                1,
                &[opentelemetry::KeyValue::new("outcome", outcome.to_owned())],
            );
        }
        let _ = outcome;
    }
    /// Record a slash proposal transition for the supplied outcome label.
    pub fn record_slash_proposal(&self, outcome: &'static str) {
        #[cfg(feature = "otel-exporter")]
        {
            self.slash_proposals_total.add(
                1,
                &[opentelemetry::KeyValue::new("outcome", outcome.to_owned())],
            );
        }
        let _ = outcome;
    }
}
/// OpenTelemetry instrumentation for GC/retention sweeps.
#[cfg_attr(not(feature = "otel-exporter"), derive(Copy))]
#[derive(Clone)]
pub struct SorafsGcOtel {
    #[cfg(feature = "otel-exporter")]
    runs_total: Counter<u64>,
    #[cfg(feature = "otel-exporter")]
    evictions_total: Counter<u64>,
    #[cfg(feature = "otel-exporter")]
    bytes_freed_total: Counter<u64>,
    #[cfg(feature = "otel-exporter")]
    blocked_total: Counter<u64>,
}
impl Default for SorafsGcOtel {
    fn default() -> Self {
        Self::new()
    }
}
#[allow(clippy::unused_self, clippy::trivially_copy_pass_by_ref)]
impl SorafsGcOtel {
    /// Create a new instrumentation bundle for GC sweeps.
    #[must_use]
    pub fn new() -> Self {
        #[cfg(feature = "otel-exporter")]
        {
            let meter = opentelemetry::global::meter("sorafs.gc");
            let runs_total = meter
                .u64_counter("sorafs.gc.runs_total")
                .with_description("SoraFS GC runs grouped by result.")
                .build();
            let evictions_total = meter
                .u64_counter("sorafs.gc.evictions_total")
                .with_description("SoraFS GC evictions grouped by reason.")
                .build();
            let bytes_freed_total = meter
                .u64_counter("sorafs.gc.bytes_freed_total")
                .with_description("SoraFS GC freed bytes grouped by reason.")
                .build();
            let blocked_total = meter
                .u64_counter("sorafs.gc.blocked_total")
                .with_description("SoraFS GC evictions blocked grouped by reason.")
                .build();
            Self {
                runs_total,
                evictions_total,
                bytes_freed_total,
                blocked_total,
            }
        }
        #[cfg(not(feature = "otel-exporter"))]
        {
            Self {}
        }
    }
    /// Record a GC run with the supplied result label.
    pub fn record_run(&self, result: &'static str) {
        #[cfg(feature = "otel-exporter")]
        {
            self.runs_total
                .add(1, &[KeyValue::new("result", result.to_owned())]);
        }
        let _ = result;
    }
    /// Record a GC eviction with the supplied reason label and freed bytes.
    pub fn record_eviction(&self, reason: &str, freed_bytes: u64) {
        #[cfg(feature = "otel-exporter")]
        {
            let labels = [KeyValue::new("reason", reason.to_owned())];
            self.evictions_total.add(1, &labels);
            self.bytes_freed_total.add(freed_bytes, &labels);
        }
        let _ = (reason, freed_bytes);
    }
    /// Record a blocked GC eviction with the supplied reason label.
    pub fn record_blocked(&self, reason: &str) {
        #[cfg(feature = "otel-exporter")]
        {
            self.blocked_total
                .add(1, &[KeyValue::new("reason", reason.to_owned())]);
        }
        let _ = reason;
    }
}
/// OpenTelemetry instrumentation for reconciliation snapshots.
#[cfg_attr(not(feature = "otel-exporter"), derive(Copy))]
#[derive(Clone)]
pub struct SorafsReconciliationOtel {
    #[cfg(feature = "otel-exporter")]
    runs_total: Counter<u64>,
    #[cfg(feature = "otel-exporter")]
    divergence_total: Counter<u64>,
}
impl Default for SorafsReconciliationOtel {
    fn default() -> Self {
        Self::new()
    }
}
#[allow(clippy::unused_self, clippy::trivially_copy_pass_by_ref)]
impl SorafsReconciliationOtel {
    /// Create a new OTEL instrumentation bundle for reconciliation snapshots.
    #[must_use]
    pub fn new() -> Self {
        #[cfg(feature = "otel-exporter")]
        {
            let meter = opentelemetry::global::meter("sorafs.reconciliation");
            let runs_total = meter
                .u64_counter("sorafs.reconciliation.runs_total")
                .with_description("SoraFS reconciliation runs grouped by result.")
                .build();
            let divergence_total = meter
                .u64_counter("sorafs.reconciliation.divergence_total")
                .with_description("SoraFS reconciliation divergence count per run.")
                .build();
            Self {
                runs_total,
                divergence_total,
            }
        }
        #[cfg(not(feature = "otel-exporter"))]
        {
            Self {}
        }
    }
    /// Record a reconciliation run with the supplied result label.
    pub fn record_run(&self, result: &'static str) {
        #[cfg(feature = "otel-exporter")]
        {
            self.runs_total
                .add(1, &[KeyValue::new("result", result.to_owned())]);
        }
        let _ = result;
    }
    /// Record the divergence count observed in a reconciliation run.
    pub fn record_divergence(&self, count: u64) {
        #[cfg(feature = "otel-exporter")]
        {
            self.divergence_total.add(count, &[]);
        }
        let _ = count;
    }
}
/// OpenTelemetry instrumentation for Torii SoraFS gateway metrics.
#[cfg_attr(not(feature = "otel-exporter"), derive(Copy))]
#[derive(Clone)]
pub struct SorafsGatewayOtel {
    #[cfg(feature = "otel-exporter")]
    active_requests: UpDownCounter<i64>,
    #[cfg(feature = "otel-exporter")]
    responses_total: Counter<u64>,
    #[cfg(feature = "otel-exporter")]
    ttfb_ms: OtelHistogram<f64>,
    #[cfg(feature = "otel-exporter")]
    proof_verifications_total: Counter<u64>,
    #[cfg(feature = "otel-exporter")]
    proof_duration_ms: OtelHistogram<f64>,
}
impl Default for SorafsGatewayOtel {
    fn default() -> Self {
        Self::new()
    }
}
#[allow(clippy::unused_self, clippy::trivially_copy_pass_by_ref)]
impl SorafsGatewayOtel {
    /// Create a new OTEL instrumentation bundle for gateway metrics.
    #[must_use]
    pub fn new() -> Self {
        #[cfg(feature = "otel-exporter")]
        {
            let meter = opentelemetry::global::meter("sorafs.gateway");
            let active_requests = meter
                .i64_up_down_counter("sorafs.gateway.active")
                .with_description("Active SoraFS gateway HTTP requests.")
                .with_unit("requests")
                .build();
            let responses_total = meter
                .u64_counter("sorafs.gateway.responses_total")
                .with_description(
                    "Total SoraFS gateway responses grouped by endpoint and bounded outcome.",
                )
                .build();
            let ttfb_ms = meter
                .f64_histogram("sorafs.gateway.ttfb_ms")
                .with_description("Gateway time-to-first-byte histogram (milliseconds).")
                .with_unit("ms")
                .build();
            let proof_verifications_total = meter
                .u64_counter("sorafs.gateway.proof_verifications_total")
                .with_description("SoraFS proof verification outcomes grouped by profile.")
                .build();
            let proof_duration_ms = meter
                .f64_histogram("sorafs.gateway.proof_duration_ms")
                .with_description("SoraFS proof verification duration (milliseconds).")
                .with_unit("ms")
                .build();
            Self {
                active_requests,
                responses_total,
                ttfb_ms,
                proof_verifications_total,
                proof_duration_ms,
            }
        }
        #[cfg(not(feature = "otel-exporter"))]
        {
            Self {}
        }
    }
    /// Track the start of a gateway request for active request accounting.
    pub fn request_started_detailed(&self, labels: SorafsGatewayRequestMetricLabels<'_>) {
        #[cfg(feature = "otel-exporter")]
        {
            let attrs = Self::base_attrs(labels);
            self.active_requests.add(1, &attrs);
        }
        #[cfg(not(feature = "otel-exporter"))]
        {
            let _ = labels;
        }
    }
    /// Track the completion of a gateway request.
    pub fn request_completed_detailed(&self, labels: SorafsGatewayResponseMetricLabels<'_>) {
        #[cfg(feature = "otel-exporter")]
        {
            let active_attrs = Self::base_attrs(labels.request);
            self.active_requests.add(-1, &active_attrs);
            let mut attrs = active_attrs;
            attrs.push(KeyValue::new("result", labels.result.to_string()));
            attrs.push(KeyValue::new("status", labels.status.to_string()));
            attrs.push(KeyValue::new("error_code", labels.error_code.to_string()));
            self.responses_total.add(1, &attrs);
        }
        #[cfg(not(feature = "otel-exporter"))]
        {
            let _ = labels;
        }
    }
    /// Record a gateway latency observation with detailed labels.
    pub fn record_ttfb_detailed(
        &self,
        labels: SorafsGatewayResponseMetricLabels<'_>,
        latency_ms: f64,
    ) {
        #[cfg(feature = "otel-exporter")]
        {
            let mut attrs = Self::base_attrs(labels.request);
            attrs.push(KeyValue::new("result", labels.result.to_string()));
            attrs.push(KeyValue::new("status", labels.status.to_string()));
            attrs.push(KeyValue::new("error_code", labels.error_code.to_string()));
            self.ttfb_ms.record(latency_ms, &attrs);
        }
        #[cfg(not(feature = "otel-exporter"))]
        {
            let _ = (labels, latency_ms);
        }
    }
    /// Record a proof verification outcome using the gateway proof metrics.
    pub fn record_proof_verification(
        &self,
        profile_version: &str,
        outcome: &str,
        error_code: &str,
        latency_ms: f64,
    ) {
        #[cfg(feature = "otel-exporter")]
        {
            let attrs = [
                KeyValue::new("profile_version", profile_version.to_string()),
                KeyValue::new("result", outcome.to_string()),
                KeyValue::new("error_code", error_code.to_string()),
            ];
            self.proof_verifications_total.add(1, &attrs);
            self.proof_duration_ms.record(latency_ms, &attrs);
        }
        #[cfg(not(feature = "otel-exporter"))]
        {
            let _ = (profile_version, outcome, error_code, latency_ms);
        }
    }
    #[cfg(feature = "otel-exporter")]
    fn base_attrs(labels: SorafsGatewayRequestMetricLabels<'_>) -> Vec<KeyValue> {
        vec![
            KeyValue::new("endpoint", labels.endpoint.to_string()),
            KeyValue::new("method", labels.method.to_string()),
            KeyValue::new("variant", labels.variant.to_string()),
            KeyValue::new("chunker", labels.chunker.to_string()),
            KeyValue::new("profile", labels.profile.to_string()),
        ]
    }
}
#[cfg(feature = "otel-exporter")]
#[derive(Default, Clone)]
struct PorSnapshot {
    success: u64,
    failure: u64,
}
/// OpenTelemetry instrumentation for embedded SoraFS node metrics.
pub struct SorafsNodeOtel {
    #[cfg(feature = "otel-exporter")]
    por_success_total: Counter<u64>,
    #[cfg(feature = "otel-exporter")]
    por_failure_total: Counter<u64>,
    #[cfg(feature = "otel-exporter")]
    capacity_ratio_pct: OtelHistogram<f64>,
    #[cfg(feature = "otel-exporter")]
    deal_settlements_total: Counter<u64>,
    #[cfg(feature = "otel-exporter")]
    deal_publish_total: Counter<u64>,
    #[cfg(feature = "otel-exporter")]
    deal_expected_charge_nano: OtelHistogram<f64>,
    #[cfg(feature = "otel-exporter")]
    deal_client_debit_nano: OtelHistogram<f64>,
    #[cfg(feature = "otel-exporter")]
    deal_outstanding_nano: OtelHistogram<f64>,
    #[cfg(feature = "otel-exporter")]
    deal_bond_slash_nano: Counter<u64>,
    #[cfg(feature = "otel-exporter")]
    micropayment_charge_nano: OtelHistogram<f64>,
    #[cfg(feature = "otel-exporter")]
    micropayment_credit_generated_nano: OtelHistogram<f64>,
    #[cfg(feature = "otel-exporter")]
    micropayment_credit_applied_nano: OtelHistogram<f64>,
    #[cfg(feature = "otel-exporter")]
    micropayment_credit_carry_nano: OtelHistogram<f64>,
    #[cfg(feature = "otel-exporter")]
    micropayment_outstanding_nano: OtelHistogram<f64>,
    #[cfg(feature = "otel-exporter")]
    micropayment_tickets_processed_total: Counter<u64>,
    #[cfg(feature = "otel-exporter")]
    micropayment_tickets_won_total: Counter<u64>,
    #[cfg(feature = "otel-exporter")]
    micropayment_tickets_duplicate_total: Counter<u64>,
    #[cfg(feature = "otel-exporter")]
    por_totals: Arc<Mutex<HashMap<String, PorSnapshot>>>,
    micropayment_sink: RwLock<Option<MicropaymentSampleSink>>,
}
impl Clone for SorafsNodeOtel {
    fn clone(&self) -> Self {
        #[cfg(feature = "otel-exporter")]
        let por_success_total = self.por_success_total.clone();
        #[cfg(feature = "otel-exporter")]
        let por_failure_total = self.por_failure_total.clone();
        #[cfg(feature = "otel-exporter")]
        let capacity_ratio_pct = self.capacity_ratio_pct.clone();
        #[cfg(feature = "otel-exporter")]
        let deal_settlements_total = self.deal_settlements_total.clone();
        #[cfg(feature = "otel-exporter")]
        let deal_publish_total = self.deal_publish_total.clone();
        #[cfg(feature = "otel-exporter")]
        let deal_expected_charge_nano = self.deal_expected_charge_nano.clone();
        #[cfg(feature = "otel-exporter")]
        let deal_client_debit_nano = self.deal_client_debit_nano.clone();
        #[cfg(feature = "otel-exporter")]
        let deal_outstanding_nano = self.deal_outstanding_nano.clone();
        #[cfg(feature = "otel-exporter")]
        let deal_bond_slash_nano = self.deal_bond_slash_nano.clone();
        #[cfg(feature = "otel-exporter")]
        let micropayment_charge_nano = self.micropayment_charge_nano.clone();
        #[cfg(feature = "otel-exporter")]
        let micropayment_credit_generated_nano = self.micropayment_credit_generated_nano.clone();
        #[cfg(feature = "otel-exporter")]
        let micropayment_credit_applied_nano = self.micropayment_credit_applied_nano.clone();
        #[cfg(feature = "otel-exporter")]
        let micropayment_credit_carry_nano = self.micropayment_credit_carry_nano.clone();
        #[cfg(feature = "otel-exporter")]
        let micropayment_outstanding_nano = self.micropayment_outstanding_nano.clone();
        #[cfg(feature = "otel-exporter")]
        let micropayment_tickets_processed_total =
            self.micropayment_tickets_processed_total.clone();
        #[cfg(feature = "otel-exporter")]
        let micropayment_tickets_won_total = self.micropayment_tickets_won_total.clone();
        #[cfg(feature = "otel-exporter")]
        let micropayment_tickets_duplicate_total =
            self.micropayment_tickets_duplicate_total.clone();
        #[cfg(feature = "otel-exporter")]
        let por_totals = self.por_totals.clone();
        let micropayment_sink = self
            .micropayment_sink
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default();
        Self {
            #[cfg(feature = "otel-exporter")]
            por_success_total,
            #[cfg(feature = "otel-exporter")]
            por_failure_total,
            #[cfg(feature = "otel-exporter")]
            capacity_ratio_pct,
            #[cfg(feature = "otel-exporter")]
            deal_settlements_total,
            #[cfg(feature = "otel-exporter")]
            deal_publish_total,
            #[cfg(feature = "otel-exporter")]
            deal_expected_charge_nano,
            #[cfg(feature = "otel-exporter")]
            deal_client_debit_nano,
            #[cfg(feature = "otel-exporter")]
            deal_outstanding_nano,
            #[cfg(feature = "otel-exporter")]
            deal_bond_slash_nano,
            #[cfg(feature = "otel-exporter")]
            micropayment_charge_nano,
            #[cfg(feature = "otel-exporter")]
            micropayment_credit_generated_nano,
            #[cfg(feature = "otel-exporter")]
            micropayment_credit_applied_nano,
            #[cfg(feature = "otel-exporter")]
            micropayment_credit_carry_nano,
            #[cfg(feature = "otel-exporter")]
            micropayment_outstanding_nano,
            #[cfg(feature = "otel-exporter")]
            micropayment_tickets_processed_total,
            #[cfg(feature = "otel-exporter")]
            micropayment_tickets_won_total,
            #[cfg(feature = "otel-exporter")]
            micropayment_tickets_duplicate_total,
            #[cfg(feature = "otel-exporter")]
            por_totals,
            micropayment_sink: RwLock::new(micropayment_sink),
        }
    }
}
/// Aggregated micropayment credit measurements captured for a single sampling window.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Default,
    IntoSchema,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
pub struct MicropaymentCreditSnapshot {
    /// Exact deterministic charge accumulated during the sample.
    pub deterministic_charge: Quantity,
    /// Exact credit produced by micropayment winnings.
    pub credit_generated: Quantity,
    /// Exact credit immediately applied against the deterministic charge.
    pub credit_applied: Quantity,
    /// Exact credit carried forward for future windows.
    pub credit_carry: Quantity,
    /// Exact outstanding balance after applying credit.
    pub outstanding: Quantity,
}
/// Lottery ticket counters observed during micropayment sampling.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Default,
    IntoSchema,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
pub struct MicropaymentTicketCounters {
    /// Total tickets processed for the sample.
    pub processed: u64,
    /// Tickets that resulted in payouts.
    pub won: u64,
    /// Tickets ignored due to duplication.
    pub duplicate: u64,
}
#[allow(clippy::unused_self, clippy::trivially_copy_pass_by_ref)]
impl SorafsNodeOtel {
    /// Create a new OTEL instrumentation bundle for SoraFS nodes.
    #[allow(clippy::too_many_lines)]
    #[must_use]
    pub fn new() -> Self {
        #[cfg(feature = "otel-exporter")]
        {
            let meter = opentelemetry::global::meter("sorafs.node");
            let build_counter = |name: &'static str, description: &'static str| {
                meter
                    .u64_counter(name)
                    .with_description(description)
                    .build()
            };
            let build_histogram =
                |name: &'static str, description: &'static str, unit: &'static str| {
                    meter
                        .f64_histogram(name)
                        .with_description(description)
                        .with_unit(unit)
                        .build()
                };
            let por_success_total = build_counter(
                "sorafs.node.por_success_total",
                "Total successful PoR samples per provider.",
            );
            let por_failure_total = build_counter(
                "sorafs.node.por_failure_total",
                "Total failed PoR samples per provider.",
            );
            let capacity_ratio_pct = build_histogram(
                "sorafs.node.capacity_utilisation_pct",
                "Recorded storage utilisation ratio per provider (percent).",
                "percent",
            );
            let deal_settlements_total = build_counter(
                "sorafs.node.deal_settlements_total",
                "Total deal settlement windows recorded per provider and status.",
            );
            let deal_publish_total = build_counter(
                "sorafs.node.deal_publish_total",
                "Settlement artefact publish attempts per provider and outcome.",
            );
            let deal_histograms = [
                (
                    "sorafs.node.deal_expected_charge_nano",
                    "Deterministic settlement charges per window (nano XOR).",
                ),
                (
                    "sorafs.node.deal_client_debit_nano",
                    "Client credit debited during settlement windows (nano XOR).",
                ),
                (
                    "sorafs.node.deal_outstanding_nano",
                    "Outstanding balances carried after settlement (nano XOR).",
                ),
            ];
            let [
                deal_expected_charge_nano,
                deal_client_debit_nano,
                deal_outstanding_nano,
            ] = deal_histograms
                .map(|(name, description)| build_histogram(name, description, "nano"));
            let deal_bond_slash_nano = build_counter(
                "sorafs.node.deal_bond_slash_nano",
                "Total bond slashes applied during settlements (nano XOR, truncated to u64).",
            );
            let micropayment_histograms = [
                (
                    "sorafs.node.micropayment_charge_nano",
                    "Deterministic charge per usage sample (nano XOR).",
                ),
                (
                    "sorafs.node.micropayment_credit_generated_nano",
                    "Micropayment credit generated during usage samples (nano XOR).",
                ),
                (
                    "sorafs.node.micropayment_credit_applied_nano",
                    "Micropayment credit applied immediately against deterministic charges (nano XOR).",
                ),
                (
                    "sorafs.node.micropayment_credit_carry_nano",
                    "Micropayment credit carried forward after usage samples (nano XOR).",
                ),
                (
                    "sorafs.node.micropayment_outstanding_nano",
                    "Outstanding balance after applying micropayment credit (nano XOR).",
                ),
            ];
            let [
                micropayment_charge_nano,
                micropayment_credit_generated_nano,
                micropayment_credit_applied_nano,
                micropayment_credit_carry_nano,
                micropayment_outstanding_nano,
            ] = micropayment_histograms
                .map(|(name, description)| build_histogram(name, description, "nano"));
            let micropayment_counters = [
                (
                    "sorafs.node.micropayment_tickets_processed_total",
                    "Micropayment lottery tickets processed during usage samples.",
                ),
                (
                    "sorafs.node.micropayment_tickets_won_total",
                    "Micropayment lottery tickets that resulted in payouts.",
                ),
                (
                    "sorafs.node.micropayment_tickets_duplicate_total",
                    "Duplicate micropayment tickets ignored during usage samples.",
                ),
            ];
            let [
                micropayment_tickets_processed_total,
                micropayment_tickets_won_total,
                micropayment_tickets_duplicate_total,
            ] = micropayment_counters.map(|(name, description)| build_counter(name, description));
            Self {
                por_success_total,
                por_failure_total,
                capacity_ratio_pct,
                deal_settlements_total,
                deal_publish_total,
                deal_expected_charge_nano,
                deal_client_debit_nano,
                deal_outstanding_nano,
                deal_bond_slash_nano,
                micropayment_charge_nano,
                micropayment_credit_generated_nano,
                micropayment_credit_applied_nano,
                micropayment_credit_carry_nano,
                micropayment_outstanding_nano,
                micropayment_tickets_processed_total,
                micropayment_tickets_won_total,
                micropayment_tickets_duplicate_total,
                por_totals: Arc::new(Mutex::new(HashMap::new())),
                micropayment_sink: RwLock::new(None),
            }
        }
        #[cfg(not(feature = "otel-exporter"))]
        {
            Self {
                micropayment_sink: RwLock::new(None),
            }
        }
    }
    /// Record a storage scheduler snapshot.
    pub fn record_storage(
        &self,
        provider_id: &str,
        bytes_used: u64,
        bytes_capacity: u64,
        por_samples_success: u64,
        por_samples_failed: u64,
    ) {
        #[cfg(feature = "otel-exporter")]
        {
            let attrs = [KeyValue::new("provider_id", provider_id.to_string())];
            if bytes_capacity > 0 {
                let utilisation = (bytes_used as f64 / bytes_capacity as f64) * 100.0;
                self.capacity_ratio_pct.record(utilisation, &attrs);
            }
            let mut totals = self
                .por_totals
                .lock()
                .expect("sorafs node otel totals mutex poisoned");
            let entry = totals
                .entry(provider_id.to_string())
                .or_insert_with(PorSnapshot::default);
            if por_samples_success >= entry.success {
                let delta = por_samples_success - entry.success;
                if delta > 0 {
                    self.por_success_total.add(delta, &attrs);
                }
            } else {
                entry.success = 0;
            }
            if por_samples_failed >= entry.failure {
                let delta = por_samples_failed - entry.failure;
                if delta > 0 {
                    self.por_failure_total.add(delta, &attrs);
                }
            } else {
                entry.failure = 0;
            }
            entry.success = por_samples_success;
            entry.failure = por_samples_failed;
        }
        #[cfg(not(feature = "otel-exporter"))]
        {
            let _ = (
                provider_id,
                bytes_used,
                bytes_capacity,
                por_samples_success,
                por_samples_failed,
            );
        }
    }
    /// Record settlement telemetry for a completed deal window.
    pub fn record_deal_settlement(
        &self,
        provider_id: &str,
        status: &str,
        expected_charge: &Quantity,
        client_debit: &Quantity,
        bond_slash: &Quantity,
        outstanding: &Quantity,
    ) {
        #[cfg(feature = "otel-exporter")]
        {
            let provider = provider_id.to_string();
            let provider_attrs = [KeyValue::new("provider_id", provider.clone())];
            let settlement_attrs = [
                KeyValue::new("provider_id", provider),
                KeyValue::new("status", status.to_string()),
            ];
            self.deal_settlements_total.add(1, &settlement_attrs);
            self.deal_expected_charge_nano
                .record(quantity_to_nano_f64(expected_charge), &provider_attrs);
            self.deal_client_debit_nano
                .record(quantity_to_nano_f64(client_debit), &provider_attrs);
            self.deal_outstanding_nano
                .record(quantity_to_nano_f64(outstanding), &provider_attrs);
            if !bond_slash.is_zero() {
                let increment = quantity_to_nano_f64(bond_slash).min(u64::MAX as f64) as u64;
                self.deal_bond_slash_nano.add(increment, &provider_attrs);
            }
        }
        #[cfg(not(feature = "otel-exporter"))]
        {
            let _ = (
                provider_id,
                status,
                expected_charge,
                client_debit,
                bond_slash,
                outstanding,
            );
        }
    }
    /// Record the outcome of a settlement artefact publish attempt.
    pub fn record_settlement_publish(&self, provider_id: &str, result: &str) {
        #[cfg(feature = "otel-exporter")]
        {
            let attrs = [
                KeyValue::new("provider_id", provider_id.to_string()),
                KeyValue::new("result", result.to_string()),
            ];
            self.deal_publish_total.add(1, &attrs);
        }
        #[cfg(not(feature = "otel-exporter"))]
        {
            let _ = (provider_id, result);
        }
    }
    /// Record telemetry for a micropayment sampling window.
    pub fn record_micropayment_sample(
        &self,
        provider_id: &str,
        credits: MicropaymentCreditSnapshot,
        tickets: MicropaymentTicketCounters,
    ) {
        #[cfg(feature = "otel-exporter")]
        {
            let MicropaymentCreditSnapshot {
                deterministic_charge,
                credit_generated,
                credit_applied,
                credit_carry,
                outstanding,
            } = &credits;
            let MicropaymentTicketCounters {
                processed: tickets_processed,
                won: tickets_won,
                duplicate: tickets_duplicate,
            } = &tickets;
            let provider = provider_id.to_string();
            let attrs = [KeyValue::new("provider_id", provider)];
            self.micropayment_charge_nano
                .record(quantity_to_nano_f64(deterministic_charge), &attrs);
            self.micropayment_credit_generated_nano
                .record(quantity_to_nano_f64(credit_generated), &attrs);
            self.micropayment_credit_applied_nano
                .record(quantity_to_nano_f64(credit_applied), &attrs);
            self.micropayment_credit_carry_nano
                .record(quantity_to_nano_f64(credit_carry), &attrs);
            self.micropayment_outstanding_nano
                .record(quantity_to_nano_f64(outstanding), &attrs);
            if *tickets_processed > 0 {
                self.micropayment_tickets_processed_total
                    .add(*tickets_processed, &attrs);
            }
            if *tickets_won > 0 {
                self.micropayment_tickets_won_total
                    .add(*tickets_won, &attrs);
            }
            if *tickets_duplicate > 0 {
                self.micropayment_tickets_duplicate_total
                    .add(*tickets_duplicate, &attrs);
            }
        }
        #[cfg(not(feature = "otel-exporter"))]
        {
            let _ = (provider_id, &credits, &tickets);
        }
        if let Ok(guard) = self.micropayment_sink.read()
            && let Some(sink) = &*guard
        {
            sink(provider_id, credits, tickets);
        }
    }
    /// Replace the current micropayment sample sink used for cross-component telemetry.
    pub fn set_micropayment_sink(&self, sink: Option<MicropaymentSampleSink>) {
        *self
            .micropayment_sink
            .write()
            .expect("micropayment sink lock poisoned") = sink;
    }
}
impl Default for SorafsNodeOtel {
    fn default() -> Self {
        Self::new()
    }
}
impl norito::core::NoritoSerialize for MicropaymentCreditSnapshot {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        let payload = (
            self.deterministic_charge.clone(),
            self.credit_generated.clone(),
            self.credit_applied.clone(),
            self.credit_carry.clone(),
            self.outstanding.clone(),
        );
        norito::core::NoritoSerialize::serialize(&payload, writer)
    }
}
impl<'a> norito::core::NoritoDeserialize<'a> for MicropaymentCreditSnapshot {
    fn deserialize(archived: &'a norito::core::Archived<Self>) -> Self {
        let (deterministic_charge, credit_generated, credit_applied, credit_carry, outstanding): (
            Quantity,
            Quantity,
            Quantity,
            Quantity,
            Quantity,
        ) = norito::core::NoritoDeserialize::deserialize(archived.cast());
        Self {
            deterministic_charge,
            credit_generated,
            credit_applied,
            credit_carry,
            outstanding,
        }
    }
}
impl<'a> DecodeFromSlice<'a> for MicropaymentCreditSnapshot {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let (
            (deterministic_charge, credit_generated, credit_applied, credit_carry, outstanding),
            used,
        ) = <(Quantity, Quantity, Quantity, Quantity, Quantity)>::decode_from_slice(bytes)?;
        Ok((
            Self {
                deterministic_charge,
                credit_generated,
                credit_applied,
                credit_carry,
                outstanding,
            },
            used,
        ))
    }
}
impl norito::core::NoritoSerialize for MicropaymentTicketCounters {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        let payload = (self.processed, self.won, self.duplicate);
        norito::core::NoritoSerialize::serialize(&payload, writer)
    }
}
impl<'a> norito::core::NoritoDeserialize<'a> for MicropaymentTicketCounters {
    fn deserialize(archived: &'a norito::core::Archived<Self>) -> Self {
        let (processed, won, duplicate): (u64, u64, u64) =
            norito::core::NoritoDeserialize::deserialize(archived.cast());
        Self {
            processed,
            won,
            duplicate,
        }
    }
}
impl<'a> DecodeFromSlice<'a> for MicropaymentTicketCounters {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let ((processed, won, duplicate), used) = <(u64, u64, u64)>::decode_from_slice(bytes)?;
        Ok((
            Self {
                processed,
                won,
                duplicate,
            },
            used,
        ))
    }
}
/// Cached micropayment sample surfaced via `/status`.
#[derive(
    Clone,
    Debug,
    Default,
    IntoSchema,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
pub struct MicropaymentSampleStatus {
    /// Hex-encoded provider identifier associated with the sample.
    pub provider_id_hex: String,
    /// Aggregated credit snapshot for the sampling window.
    pub credits: MicropaymentCreditSnapshot,
    /// Ticket counters observed for the sampling window.
    pub tickets: MicropaymentTicketCounters,
}
impl norito::core::NoritoSerialize for MicropaymentSampleStatus {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        let payload = (
            self.provider_id_hex.clone(),
            self.credits.clone(),
            self.tickets,
        );
        norito::core::NoritoSerialize::serialize(&payload, writer)
    }
}
impl<'a> norito::core::NoritoDeserialize<'a> for MicropaymentSampleStatus {
    fn deserialize(archived: &'a norito::core::Archived<Self>) -> Self {
        let (provider_id_hex, credits, tickets): (
            String,
            MicropaymentCreditSnapshot,
            MicropaymentTicketCounters,
        ) = norito::core::NoritoDeserialize::deserialize(archived.cast());
        Self {
            provider_id_hex,
            credits,
            tickets,
        }
    }
}
impl<'a> DecodeFromSlice<'a> for MicropaymentSampleStatus {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let ((provider_id_hex, credits, tickets), used) = <(
            String,
            MicropaymentCreditSnapshot,
            MicropaymentTicketCounters,
        )>::decode_from_slice(bytes)?;
        Ok((
            Self {
                provider_id_hex,
                credits,
                tickets,
            },
            used,
        ))
    }
}
/// Snapshot of Taikai ingest health per (cluster, stream) surfaced via `/status`.
#[derive(
    Clone,
    Debug,
    Default,
    IntoSchema,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
pub struct TaikaiIngestStatus {
    /// Cluster label associated with the ingest pipeline.
    pub cluster: String,
    /// Stream identifier within the cluster.
    pub stream: String,
    /// Last observed encoder-to-ingest latency in milliseconds.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub last_latency_ms: Option<u32>,
    /// Last observed signed live-edge drift in milliseconds (negative = ahead).
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub last_live_edge_drift_ms: Option<i32>,
    /// Aggregated ingest error counters grouped by reason.
    #[norito(default)]
    #[norito(skip_serializing_if = "Vec::is_empty")]
    pub error_counts: Vec<TaikaiIngestErrorCounter>,
}
/// Aggregated error counter for a given reason.
#[derive(
    Clone,
    Debug,
    Default,
    IntoSchema,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
pub struct TaikaiIngestErrorCounter {
    /// Normalised error reason identifier (HTTP canonical reason or status code).
    pub reason: String,
    /// Total occurrences observed by the node.
    pub total: u64,
}
/// Maximum number of stream snapshots retained for Taikai ingest status.
const TAIKAI_INGEST_SNAPSHOT_CAP: usize = 256;
/// Maximum distinct error reasons tracked per Taikai stream snapshot.
const TAIKAI_INGEST_ERROR_REASON_CAP: usize = 32;
/// Maximum number of Taikai alias-rotation snapshots and metric children.
const TAIKAI_ALIAS_ROTATION_SNAPSHOT_CAP: usize = 256;
#[derive(Clone, Debug, Default)]
struct TaikaiIngestSnapshotInternal {
    last_latency_ms: Option<u32>,
    last_live_edge_drift_ms: Option<i32>,
    error_totals: BTreeMap<String, u64>,
}
/// Snapshot of alias rotation events coming from Taikai routing manifests.
#[derive(
    Clone,
    Debug,
    Default,
    IntoSchema,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
pub struct TaikaiAliasRotationStatus {
    /// Cluster label associated with the ingest pipeline.
    pub cluster: String,
    /// Event identifier.
    pub event: String,
    /// Stream identifier.
    pub stream: String,
    /// Namespace portion of the alias binding (e.g., `sora`).
    pub alias_namespace: String,
    /// Alias label bound to the TRM (e.g., `docs`).
    pub alias_name: String,
    /// Inclusive start of the manifest coverage window.
    pub window_start_sequence: u64,
    /// Inclusive end of the manifest coverage window.
    pub window_end_sequence: u64,
    /// Hex-encoded digest of the accepted routing manifest.
    pub manifest_digest_hex: String,
    /// Total rotations observed for this stream/event pair.
    pub rotations_total: u64,
    /// UNIX timestamp (seconds) when this snapshot was last updated.
    pub last_updated_unix: u64,
}
#[derive(Clone, Debug, Default)]
struct TaikaiAliasRotationSnapshotInternal {
    alias_namespace: String,
    alias_name: String,
    window_start_sequence: u64,
    window_end_sequence: u64,
    manifest_digest_hex: String,
    rotations_total: u64,
    last_updated_unix: u64,
}
type TaikaiAliasRotationSnapshots =
    Arc<RwLock<BTreeMap<(String, String, String), TaikaiAliasRotationSnapshotInternal>>>;
type TaikaiAliasRotationSnapshotOrder = Arc<RwLock<VecDeque<(String, String, String)>>>;
#[derive(Clone, Copy)]
struct TaikaiAliasRotationSnapshotArgs<'a> {
    cluster: &'a str,
    event: &'a str,
    stream: &'a str,
    alias_namespace: &'a str,
    alias_name: &'a str,
    window_start_sequence: u64,
    window_end_sequence: u64,
    manifest_digest_hex: &'a str,
}
static GLOBAL_FASTPQ_OTEL: OnceLock<Arc<FastpqOtel>> = OnceLock::new();
static GLOBAL_SORAFS_FETCH_OTEL: OnceLock<Arc<SorafsFetchOtel>> = OnceLock::new();
static GLOBAL_SORAFS_REPAIR_OTEL: OnceLock<Arc<SorafsRepairOtel>> = OnceLock::new();
static GLOBAL_SORAFS_RECONCILIATION_OTEL: OnceLock<Arc<SorafsReconciliationOtel>> = OnceLock::new();
static GLOBAL_SORAFS_GC_OTEL: OnceLock<Arc<SorafsGcOtel>> = OnceLock::new();
static GLOBAL_SORAFS_GATEWAY_OTEL: OnceLock<Arc<SorafsGatewayOtel>> = OnceLock::new();
static GLOBAL_SORAFS_NODE_OTEL: OnceLock<Arc<SorafsNodeOtel>> = OnceLock::new();
/// Retrieve the global FASTPQ OTEL metrics handle.
#[must_use]
pub fn global_fastpq_otel() -> Arc<FastpqOtel> {
    Arc::clone(GLOBAL_FASTPQ_OTEL.get_or_init(|| Arc::new(FastpqOtel::new())))
}
/// Retrieve the global OTEL metrics handle used by the orchestrator.
#[must_use]
pub fn global_sorafs_fetch_otel() -> Arc<SorafsFetchOtel> {
    Arc::clone(GLOBAL_SORAFS_FETCH_OTEL.get_or_init(|| Arc::new(SorafsFetchOtel::new())))
}
/// Retrieve the global OTEL metrics handle used by repair automation.
#[must_use]
pub fn global_sorafs_repair_otel() -> Arc<SorafsRepairOtel> {
    Arc::clone(GLOBAL_SORAFS_REPAIR_OTEL.get_or_init(|| Arc::new(SorafsRepairOtel::new())))
}
/// Retrieve the global OTEL metrics handle used by reconciliation snapshots.
#[must_use]
pub fn global_sorafs_reconciliation_otel() -> Arc<SorafsReconciliationOtel> {
    Arc::clone(
        GLOBAL_SORAFS_RECONCILIATION_OTEL.get_or_init(|| Arc::new(SorafsReconciliationOtel::new())),
    )
}
/// Retrieve the global OTEL metrics handle used by GC automation.
#[must_use]
pub fn global_sorafs_gc_otel() -> Arc<SorafsGcOtel> {
    Arc::clone(GLOBAL_SORAFS_GC_OTEL.get_or_init(|| Arc::new(SorafsGcOtel::new())))
}
/// Retrieve the global OTEL metrics handle used by Torii gateway endpoints.
#[must_use]
pub fn global_sorafs_gateway_otel() -> Arc<SorafsGatewayOtel> {
    Arc::clone(GLOBAL_SORAFS_GATEWAY_OTEL.get_or_init(|| Arc::new(SorafsGatewayOtel::new())))
}
/// Retrieve the global OTEL metrics handle used by embedded SoraFS nodes.
#[must_use]
pub fn global_sorafs_node_otel() -> Arc<SorafsNodeOtel> {
    Arc::clone(GLOBAL_SORAFS_NODE_OTEL.get_or_init(|| Arc::new(SorafsNodeOtel::new())))
}
#[cfg(test)]
mod tests {
    use super::*;
    use norito::{NoritoDeserialize, from_bytes, to_bytes};
    fn find_metric_line<'a>(dump: &'a str, prefix: &str) -> &'a str {
        dump.lines()
            .find(|line| line.starts_with(prefix))
            .unwrap_or_else(|| panic!("metric line starting with `{prefix}` not found"))
    }
    fn parse_metric_value(line: &str) -> f64 {
        line.split_whitespace()
            .last()
            .unwrap_or_else(|| panic!("metric line `{line}` missing value"))
            .parse::<f64>()
            .unwrap_or_else(|err| panic!("invalid metric value `{line}`: {err}"))
    }
    fn sample_dataspace_teu_status() -> NexusDataspaceTeuStatus {
        NexusDataspaceTeuStatus {
            lane_id: 0,
            dataspace_id: 0,
            fault_tolerance: 1,
            backlog: 1,
            age_slots: 0,
            virtual_finish: 0,
            tx_served: 0,
            alias: String::new(),
            description: None,
        }
    }
    #[test]
    fn dataspace_teu_status_roundtrips_fault_tolerance() {
        let mut status = sample_dataspace_teu_status();
        status.fault_tolerance = 2;
        let bytes = to_bytes(&status).expect("serialize status");
        let archived = from_bytes(&bytes).expect("deserialize status");
        let decoded = NexusDataspaceTeuStatus::deserialize(archived);
        assert_eq!(decoded.fault_tolerance, status.fault_tolerance);
    }
    #[test]
    fn recent_rejected_transactions_prune_after_window() {
        let metrics = Metrics::default();
        metrics.record_rejected_transactions(2, 1_000);
        metrics.record_rejected_transactions(3, 2_000);
        assert_eq!(metrics.last_rejection_at_ms(), Some(2_000));
        assert_eq!(metrics.txs_rejected_recent_5m(2_500), 5);
        assert_eq!(metrics.txs_rejected_recent_5m(302_000), 3);
        assert_eq!(metrics.txs_rejected_recent_5m(303_000), 0);
    }
    #[test]
    fn da_receipt_metrics_retain_only_the_latest_epoch_per_lane() {
        let metrics = Metrics::default();
        metrics.record_da_receipt_outcome(7, 3, 5, "attacker-controlled", false);
        metrics.set_da_receipt_cursor(7, 3, 5);
        metrics.set_da_receipt_cursor(7, 3, 4);
        metrics.set_da_receipt_cursor(7, 2, u64::MAX);
        metrics.set_da_receipt_cursor(7, 4, 1);
        let status = metrics.da_receipt_cursor_status();
        assert_eq!(status.len(), 1);
        assert_eq!(status[0].lane_id, 7);
        assert_eq!(status[0].epoch, 4);
        assert_eq!(status[0].highest_sequence, 1);
        assert_eq!(
            metrics
                .torii_da_receipts_total
                .with_label_values(&["unknown", "7"])
                .get(),
            1
        );
        assert_eq!(
            metrics
                .torii_da_receipt_epoch
                .with_label_values(&["7"])
                .get(),
            4
        );
        assert_eq!(
            metrics
                .torii_da_receipt_highest_sequence
                .with_label_values(&["7"])
                .get(),
            1
        );
        assert!(
            metrics
                .torii_da_receipts_total
                .collect()
                .iter()
                .flat_map(prometheus::proto::MetricFamily::get_metric)
                .flat_map(prometheus::proto::Metric::get_label)
                .all(|label| label.name() != "epoch"),
            "epoch must be a gauge value, never a Prometheus label"
        );
    }
    #[test]
    fn da_receipt_metric_lanes_are_capped_prunable_and_poison_tolerant() {
        let metrics = Metrics::default();
        for lane_id in 0..u32::try_from(MAX_ACTIVE_EXECUTION_LANES).expect("bound fits u32") {
            metrics.set_da_receipt_cursor(lane_id, 1, 1);
        }
        metrics.set_da_receipt_cursor(u32::MAX, 1, 1);
        assert_eq!(
            metrics.da_receipt_cursor_status().len(),
            MAX_ACTIVE_EXECUTION_LANES
        );
        assert!(
            metrics
                .da_receipt_cursor_status()
                .iter()
                .all(|cursor| cursor.lane_id != u32::MAX)
        );
        metrics.prune_da_receipt_lanes([7]);
        assert!(
            metrics
                .da_receipt_cursor_status()
                .iter()
                .all(|cursor| cursor.lane_id != 7)
        );
        let dump = metrics.try_to_string().expect("metrics should encode");
        assert!(
            !dump.lines().any(|line| {
                line.starts_with("torii_da_receipt") && line.contains("lane=\"7\"")
            }),
            "retired-lane receipt series must be removed"
        );
        let lanes = Arc::clone(&metrics.da_receipt_metric_lanes);
        let poisoned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = lanes.write().expect("lock should initially be healthy");
            panic!("poison DA receipt metric state");
        }));
        assert!(poisoned.is_err());
        metrics.set_da_receipt_cursor(7, 2, 3);
        assert!(
            metrics
                .da_receipt_cursor_status()
                .iter()
                .any(|cursor| cursor.lane_id == 7
                    && cursor.epoch == 2
                    && cursor.highest_sequence == 3),
            "metrics must recover the poisoned cache instead of panicking"
        );
    }
    #[cfg(not(feature = "otel-exporter"))]
    #[test]
    fn sorafs_node_otel_new_and_record_sample_do_not_panic_without_exporter() {
        let otel = SorafsNodeOtel::new();
        otel.record_micropayment_sample(
            "provider",
            MicropaymentCreditSnapshot {
                deterministic_charge: 10_u64.into(),
                credit_generated: 5_u64.into(),
                credit_applied: 4_u64.into(),
                credit_carry: 1_u64.into(),
                outstanding: 2_u64.into(),
            },
            MicropaymentTicketCounters {
                processed: 3,
                won: 1,
                duplicate: 0,
            },
        );
    }
    #[test]
    fn micropayment_credit_snapshot_norito_roundtrip_preserves_exact_quantities() {
        let snapshot = MicropaymentCreditSnapshot {
            deterministic_charge: "0.0000000001".parse().expect("canonical sub-nano quantity"),
            credit_generated: "340282366920938463463374607431768211456"
                .parse()
                .expect("canonical quantity wider than u128"),
            credit_applied: "1.25".parse().expect("canonical fractional quantity"),
            credit_carry: 0_u64.into(),
            outstanding: "0.000000000000000001"
                .parse()
                .expect("canonical exact quantity"),
        };
        let bytes = to_bytes(&snapshot).expect("encode exact micropayment snapshot");
        let archived = from_bytes::<MicropaymentCreditSnapshot>(&bytes)
            .expect("archive exact micropayment snapshot");
        let decoded = MicropaymentCreditSnapshot::deserialize(archived);
        assert_eq!(decoded, snapshot);
    }
    #[cfg(not(feature = "otel-exporter"))]
    #[test]
    fn sorafs_reconciliation_otel_new_and_record_do_not_panic_without_exporter() {
        let otel = SorafsReconciliationOtel::new();
        otel.record_run("success");
        otel.record_divergence(2);
    }
    #[test]
    fn records_fastpq_execution_mode_metrics() {
        let metrics = Metrics::default();
        metrics.record_fastpq_execution_mode("auto", "cpu", "none", "apple-m4", "m4", "integrated");
        let value = metrics
            .fastpq_execution_mode_total
            .with_label_values(&["auto", "cpu", "none", "apple-m4", "m4", "integrated"])
            .get();
        assert_eq!(value, 1, "FASTPQ execution mode counter increments");
    }
    #[test]
    fn records_fastpq_gpu_disable_and_parity_metrics() {
        let metrics = Metrics::default();
        metrics.inc_fastpq_gpu_disable(
            "poseidon_merkle_pairs",
            "cpu_parity_mismatch",
            "apple-m4",
            "m4",
            "integrated",
        );
        metrics.inc_fastpq_gpu_parity_failure(
            "poseidon_merkle_pairs",
            "cpu_parity_mismatch",
            "apple-m4",
            "m4",
            "integrated",
        );
        assert_eq!(
            metrics
                .fastpq_gpu_disable_total
                .with_label_values(&[
                    "poseidon_merkle_pairs",
                    "cpu_parity_mismatch",
                    "apple-m4",
                    "m4",
                    "integrated",
                ])
                .get(),
            1
        );
        assert_eq!(
            metrics
                .fastpq_gpu_parity_failure_total
                .with_label_values(&[
                    "poseidon_merkle_pairs",
                    "cpu_parity_mismatch",
                    "apple-m4",
                    "m4",
                    "integrated",
                ])
                .get(),
            1
        );
    }
    #[test]
    fn records_fastpq_proof_sidecar_metrics() {
        let metrics = Metrics::default();
        metrics.set_fastpq_proof_sidecar_queue_depth(7);
        metrics.inc_fastpq_proof_sidecar_event("enqueued");
        assert_eq!(metrics.fastpq_proof_sidecar_queue_depth.get(), 7);
        assert_eq!(
            metrics
                .fastpq_proof_sidecar_events_total
                .with_label_values(&["enqueued"])
                .get(),
            1
        );
    }
    #[test]
    fn records_fastpq_metal_queue_metrics() {
        let metrics = Metrics::default();
        let lanes = [
            FastpqMetalQueueLaneSample {
                index: 0,
                dispatch_count: 10,
                max_in_flight: 2,
                busy_ms: 25.0,
                overlap_ms: 5.0,
            },
            FastpqMetalQueueLaneSample {
                index: 1,
                dispatch_count: 5,
                max_in_flight: 1,
                busy_ms: 10.0,
                overlap_ms: 2.0,
            },
        ];
        let sample = FastpqMetalQueueSample {
            limit: 4,
            max_in_flight: 3,
            dispatch_count: 20,
            window_ms: 50.0,
            busy_ms: 30.0,
            overlap_ms: 12.5,
            lanes: &lanes,
        };
        metrics.record_fastpq_metal_queue_stats("apple-m4", "m4", "integrated", &sample);
        let depth_limit = metrics
            .fastpq_metal_queue_depth
            .with_label_values(&["apple-m4", "m4", "integrated", "limit"])
            .get();
        assert!(
            (depth_limit - 4.0).abs() < f64::EPSILON,
            "depth limit recorded"
        );
        let window_seconds = metrics
            .fastpq_metal_queue_depth
            .with_label_values(&["apple-m4", "m4", "integrated", "window_seconds"])
            .get();
        assert!(
            (window_seconds - 0.05).abs() < f64::EPSILON,
            "window seconds recorded"
        );
        let busy_ratio = metrics
            .fastpq_metal_queue_ratio
            .with_label_values(&["apple-m4", "m4", "integrated", "global", "busy"])
            .get();
        assert!(
            (busy_ratio - 0.6).abs() < 1e-9,
            "global busy ratio derived from sample"
        );
        let lane_busy_ratio = metrics
            .fastpq_metal_queue_ratio
            .with_label_values(&["apple-m4", "m4", "integrated", "lane-0", "busy"])
            .get();
        assert!(
            (lane_busy_ratio - 0.5).abs() < 1e-9,
            "lane duty cycle recorded"
        );
    }
    #[test]
    fn records_fastpq_zero_fill_metrics() {
        let metrics = Metrics::default();
        metrics.record_fastpq_zero_fill("apple-m4", "m4", "integrated", 0.25, 32_000);
        let duration = metrics
            .fastpq_zero_fill_duration_ms
            .with_label_values(&["apple-m4", "m4", "integrated"])
            .get();
        assert!((duration - 0.25).abs() < f64::EPSILON);
        let bandwidth = metrics
            .fastpq_zero_fill_bandwidth_gbps
            .with_label_values(&["apple-m4", "m4", "integrated"])
            .get();
        // (32_000 bytes * 8 bits) / (0.25 ms * 1e6) = 1.024 Gbps
        assert!((bandwidth - 1.024).abs() < 1e-6);
    }
    #[test]
    fn scheduler_layer_width_buckets_norito_json_roundtrip() {
        let values = [1, 2, 3, 4, 5, 6, 7, 8];
        let buckets = SchedulerLayerWidthBuckets::from(values);
        assert_eq!(buckets.as_slice(), &values);
        let bytes = to_bytes(&buckets).expect("serialize buckets");
        let archived =
            from_bytes::<SchedulerLayerWidthBuckets>(&bytes).expect("archived buckets payload");
        let decoded = norito::core::NoritoDeserialize::deserialize(archived);
        assert_eq!(decoded.as_slice(), &values);
        let json_bytes = norito::json::to_vec(&buckets).expect("JSON encode buckets");
        let parsed: SchedulerLayerWidthBuckets =
            norito::json::from_slice(&json_bytes).expect("JSON decode buckets");
        assert_eq!(parsed.as_slice(), &values);
        let json_repr = String::from_utf8(json_bytes).expect("utf8 json encoding");
        assert!(
            json_repr.contains('[') && json_repr.contains(']'),
            "unexpected JSON payload: {json_repr}"
        );
    }
    #[test]
    fn scheduler_layer_width_buckets_from_slice_pads_and_truncates() {
        let input = [42_u64, 7, 9];
        let buckets = SchedulerLayerWidthBuckets::from_slice(&input);
        let mut expected = [0_u64; 8];
        expected[..input.len()].copy_from_slice(&input);
        assert_eq!(buckets.as_slice(), &expected);
        let long_input = [11_u64; 16];
        let truncated = SchedulerLayerWidthBuckets::from_slice(&long_input);
        assert!(truncated.as_slice().iter().all(|&value| value == 11));
    }
    #[test]
    fn taikai_ingest_snapshot_tracks_latest_values() {
        let metrics = Metrics::default();
        metrics.observe_taikai_ingest_latency("cluster-a", "stream-main", 150);
        metrics.observe_taikai_live_edge_drift("cluster-a", "stream-main", -37);
        metrics.inc_taikai_ingest_error("cluster-a", "stream-main", "decode");
        let snapshots = metrics.taikai_ingest_status();
        assert_eq!(snapshots.len(), 1);
        let snapshot = &snapshots[0];
        assert_eq!(snapshot.cluster, "cluster-a");
        assert_eq!(snapshot.stream, "stream-main");
        assert_eq!(snapshot.last_latency_ms, Some(150));
        assert_eq!(snapshot.last_live_edge_drift_ms, Some(-37));
        assert_eq!(snapshot.error_counts.len(), 1);
        assert_eq!(snapshot.error_counts[0].reason, "decode");
        assert_eq!(snapshot.error_counts[0].total, 1);
    }
    #[test]
    fn taikai_ingest_snapshot_prunes_oldest_streams() {
        let metrics = Metrics::default();
        for idx in 0..=TAIKAI_INGEST_SNAPSHOT_CAP {
            let stream = format!("stream-{idx}");
            metrics.observe_taikai_ingest_latency("cluster-a", &stream, 10);
        }
        let snapshots = metrics.taikai_ingest_status();
        assert!(
            snapshots.len() <= TAIKAI_INGEST_SNAPSHOT_CAP,
            "expected snapshots to be bounded, found {} entries",
            snapshots.len()
        );
        let newest = format!("stream-{TAIKAI_INGEST_SNAPSHOT_CAP}");
        assert!(
            snapshots.iter().any(|snapshot| snapshot.stream == newest),
            "most recent stream should be retained"
        );
        assert!(
            snapshots
                .iter()
                .all(|snapshot| snapshot.stream != "stream-0"),
            "oldest stream should be evicted"
        );
        let dump = metrics.try_to_string().expect("metrics text");
        assert!(
            dump.lines().all(|line| {
                !line.starts_with("taikai_ingest_segment_latency_ms")
                    || !line.contains("stream=\"stream-0\"")
            }),
            "snapshot eviction must remove the Prometheus child for the oldest stream"
        );
        assert_eq!(
            dump.lines()
                .filter(|line| line.starts_with("taikai_ingest_segment_latency_ms_count{"))
                .count(),
            TAIKAI_INGEST_SNAPSHOT_CAP,
            "Prometheus latency stream cardinality must follow the snapshot cap"
        );
    }
    #[test]
    fn taikai_ingest_error_reasons_are_capped() {
        let metrics = Metrics::default();
        for idx in 0..=TAIKAI_INGEST_ERROR_REASON_CAP {
            metrics.inc_taikai_ingest_error("cluster-a", "stream-main", &format!("reason-{idx}"));
        }
        metrics.inc_taikai_ingest_error("cluster-a", "stream-main", "reason-new");
        let snapshots = metrics.taikai_ingest_status();
        let snapshot = snapshots
            .iter()
            .find(|entry| entry.stream == "stream-main")
            .expect("stream-main present");
        assert!(
            snapshot.error_counts.len() <= TAIKAI_INGEST_ERROR_REASON_CAP,
            "expected error reasons to be capped"
        );
        assert!(
            snapshot
                .error_counts
                .iter()
                .any(|entry| entry.reason == "reason-new"),
            "newest reason should be inserted after eviction"
        );
        assert!(
            snapshot
                .error_counts
                .iter()
                .all(|entry| entry.reason != "reason-0"),
            "oldest reason should be evicted to enforce the cap"
        );
        let dump = metrics.try_to_string().expect("metrics text");
        let error_lines = dump
            .lines()
            .filter(|line| {
                line.starts_with("taikai_ingest_errors_total{")
                    && line.contains("stream=\"stream-main\"")
            })
            .collect::<Vec<_>>();
        assert_eq!(
            error_lines.len(),
            TAIKAI_INGEST_ERROR_REASON_CAP,
            "Prometheus error cardinality must follow the reason cap"
        );
        assert!(
            error_lines
                .iter()
                .all(|line| !line.contains("reason=\"reason-0\"")),
            "reason eviction must remove the Prometheus child"
        );
    }
    #[test]
    fn taikai_ingest_drift_gauge_preserves_sign() {
        let metrics = Metrics::default();
        metrics.observe_taikai_live_edge_drift("cluster-a", "stream-main", -42);
        let dump = metrics.try_to_string().expect("metrics text");
        let line = find_metric_line(
            &dump,
            "taikai_ingest_live_edge_drift_signed_ms{cluster=\"cluster-a\"",
        );
        let value = parse_metric_value(line);
        assert!(
            (value + 42.0).abs() < 1e-6,
            "expected signed drift gauge to retain negative value, got {value}"
        );
    }
    #[test]
    fn taikai_alias_rotation_snapshot_tracks_latest_manifest() {
        let metrics = Metrics::default();
        metrics.record_taikai_alias_rotation(
            "cluster-a",
            "event-main",
            "stream-main",
            "sora",
            "docs",
            10,
            20,
            "deadbeef",
        );
        metrics.record_taikai_alias_rotation(
            "cluster-a",
            "event-main",
            "stream-main",
            "sora",
            "docs",
            10,
            24,
            "cafebabe",
        );
        let snapshots = metrics.taikai_alias_rotation_status();
        assert_eq!(snapshots.len(), 1);
        let snapshot = &snapshots[0];
        assert_eq!(snapshot.cluster, "cluster-a");
        assert_eq!(snapshot.event, "event-main");
        assert_eq!(snapshot.stream, "stream-main");
        assert_eq!(snapshot.alias_namespace, "sora");
        assert_eq!(snapshot.alias_name, "docs");
        assert_eq!(snapshot.window_start_sequence, 10);
        assert_eq!(snapshot.window_end_sequence, 24);
        assert_eq!(snapshot.manifest_digest_hex, "cafebabe");
        assert_eq!(snapshot.rotations_total, 2);
        assert!(snapshot.last_updated_unix > 0);
        let dump = metrics.try_to_string().expect("metrics text");
        let metric_line = find_metric_line(
            &dump,
            "taikai_trm_alias_rotations_total{alias_name=\"docs\",alias_namespace=\"sora\"",
        );
        assert!(
            metric_line.contains("cluster=\"cluster-a\"")
                && metric_line.contains("event=\"event-main\"")
                && metric_line.contains("stream=\"stream-main\""),
            "metric labels should include cluster/event/stream"
        );
        let observed = parse_metric_value(metric_line);
        assert!(
            (observed - 2.0).abs() < f64::EPSILON,
            "expected counter to reflect total rotations"
        );
    }
    #[test]
    fn taikai_alias_rotation_snapshots_and_metrics_are_capped() {
        let metrics = Metrics::default();
        for idx in 0..=TAIKAI_ALIAS_ROTATION_SNAPSHOT_CAP {
            let sequence = u64::try_from(idx).expect("test index fits u64");
            metrics.record_taikai_alias_rotation(
                "cluster-a",
                &format!("event-{idx}"),
                &format!("stream-{idx}"),
                "sora",
                "docs",
                sequence,
                sequence,
                "deadbeef",
            );
        }
        let snapshots = metrics.taikai_alias_rotation_status();
        assert_eq!(snapshots.len(), TAIKAI_ALIAS_ROTATION_SNAPSHOT_CAP);
        assert!(
            snapshots.iter().all(|snapshot| snapshot.event != "event-0"),
            "oldest alias-rotation snapshot must be evicted"
        );
        let dump = metrics.try_to_string().expect("metrics text");
        let rotation_lines = dump
            .lines()
            .filter(|line| line.starts_with("taikai_trm_alias_rotations_total{"))
            .collect::<Vec<_>>();
        assert_eq!(
            rotation_lines.len(),
            TAIKAI_ALIAS_ROTATION_SNAPSHOT_CAP,
            "Prometheus alias-rotation cardinality must follow the snapshot cap"
        );
        assert!(
            rotation_lines
                .iter()
                .all(|line| !line.contains("event=\"event-0\"")),
            "snapshot eviction must remove the oldest Prometheus child"
        );
    }
    #[test]
    fn duplicate_metric_panic_flag_follows_override() {
        let flag = duplicate_metrics_flag();
        let previous = flag.load(Ordering::Relaxed);
        set_duplicate_metrics_panic(true);
        assert!(duplicate_metrics_should_panic());
        set_duplicate_metrics_panic(false);
        assert!(!duplicate_metrics_should_panic());
        // Restore prior state to avoid leaking configuration between tests.
        flag.store(previous, Ordering::Relaxed);
    }
    #[test]
    fn metrics_default_registers_without_duplicate_metrics() {
        struct DuplicateMetricsGuard(bool);
        impl Drop for DuplicateMetricsGuard {
            fn drop(&mut self) {
                set_duplicate_metrics_panic(self.0);
            }
        }
        let flag = duplicate_metrics_flag();
        let previous = flag.load(Ordering::Relaxed);
        set_duplicate_metrics_panic(true);
        let _guard = DuplicateMetricsGuard(previous);
        let _metrics = Metrics::default();
    }
}
#[cfg(feature = "otel-exporter")]
fn install_otlp_metrics_exporter(
    endpoint: &str,
    service_name: &str,
    resource: &[(&str, &str)],
    interval: Duration,
) -> eyre::Result<()> {
    use opentelemetry_otlp::{MetricExporter, WithExportConfig};
    use opentelemetry_sdk::{
        Resource,
        metrics::{PeriodicReader, SdkMeterProvider},
    };
    let exporter = MetricExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint.to_owned())
        .build()?;
    let reader = PeriodicReader::builder(exporter)
        .with_interval(interval)
        .build();
    let mut attributes = Vec::with_capacity(resource.len() + 1);
    attributes.push(KeyValue::new("service.name", service_name.to_string()));
    for (key, value) in resource {
        attributes.push(KeyValue::new((*key).to_string(), (*value).to_string()));
    }
    let provider = SdkMeterProvider::builder()
        .with_resource(
            Resource::builder_empty()
                .with_attributes(attributes)
                .build(),
        )
        .with_reader(reader)
        .build();
    opentelemetry::global::set_meter_provider(provider);
    Ok(())
}
/// Install an OTLP exporter that streams SoraFS orchestrator metrics via OpenTelemetry.
///
/// # Errors
/// Returns an error if the OTLP exporter cannot be initialised with the provided settings.
#[cfg(feature = "otel-exporter")]
pub fn install_sorafs_fetch_otlp_exporter(
    endpoint: &str,
    service_name: &str,
    resource: &[(&str, &str)],
    interval: Duration,
) -> eyre::Result<()> {
    install_otlp_metrics_exporter(endpoint, service_name, resource, interval)
}
/// Stub exporter installer when the OTEL feature is disabled.
///
/// # Errors
/// Always returns an error indicating that the `otel-exporter` feature is disabled.
#[cfg(not(feature = "otel-exporter"))]
pub fn install_sorafs_fetch_otlp_exporter(
    _endpoint: &str,
    _service_name: &str,
    _resource: &[(&str, &str)],
    _interval: Duration,
) -> eyre::Result<()> {
    eyre::bail!("otel-exporter feature is disabled; enable it to emit OTLP telemetry");
}
/// Install an OTLP exporter that streams Torii gateway metrics via OpenTelemetry.
///
/// # Errors
/// Returns an error if the OTLP exporter cannot be initialised with the provided settings.
#[cfg(feature = "otel-exporter")]
pub fn install_sorafs_gateway_otlp_exporter(
    endpoint: &str,
    service_name: &str,
    resource: &[(&str, &str)],
    interval: Duration,
) -> eyre::Result<()> {
    install_otlp_metrics_exporter(endpoint, service_name, resource, interval)
}
/// Stub gateway exporter installer when the OTEL feature is disabled.
///
/// # Errors
/// Always returns an error indicating that the `otel-exporter` feature is disabled.
#[cfg(not(feature = "otel-exporter"))]
pub fn install_sorafs_gateway_otlp_exporter(
    _endpoint: &str,
    _service_name: &str,
    _resource: &[(&str, &str)],
    _interval: Duration,
) -> eyre::Result<()> {
    eyre::bail!("otel-exporter feature is disabled; enable it to emit OTLP telemetry");
}
/// Install an OTLP exporter that streams embedded node metrics via OpenTelemetry.
///
/// # Errors
/// Returns an error if the OTLP exporter cannot be initialised with the provided settings.
#[cfg(feature = "otel-exporter")]
pub fn install_sorafs_node_otlp_exporter(
    endpoint: &str,
    service_name: &str,
    resource: &[(&str, &str)],
    interval: Duration,
) -> eyre::Result<()> {
    install_otlp_metrics_exporter(endpoint, service_name, resource, interval)
}
/// Stub node exporter installer when the OTEL feature is disabled.
///
/// # Errors
/// Always returns an error indicating that the `otel-exporter` feature is disabled.
#[cfg(not(feature = "otel-exporter"))]
pub fn install_sorafs_node_otlp_exporter(
    _endpoint: &str,
    _service_name: &str,
    _resource: &[(&str, &str)],
    _interval: Duration,
) -> eyre::Result<()> {
    eyre::bail!("otel-exporter feature is disabled; enable it to emit OTLP telemetry");
}
include!("metrics/otel_tests.rs");
impl JsonSerialize for Uptime {
    fn json_serialize(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"secs\":");
        norito::json::JsonSerialize::json_serialize(&self.0.as_secs(), out);
        out.push(',');
        out.push_str("\"nanos\":");
        norito::json::JsonSerialize::json_serialize(&self.0.subsec_nanos(), out);
        out.push('}');
    }
}
impl JsonDeserialize for Uptime {
    fn json_deserialize(p: &mut norito::json::Parser<'_>) -> Result<Self, norito::json::Error> {
        let mut map = norito::json::MapVisitor::new(p)?;
        let mut secs: Option<u64> = None;
        let mut nanos: Option<u32> = None;
        while let Some(key) = map.next_key()? {
            match key.as_str() {
                "secs" => {
                    if secs.is_some() {
                        return Err(norito::json::Error::duplicate_field("secs"));
                    }
                    secs = Some(map.parse_value::<u64>()?);
                }
                "nanos" => {
                    if nanos.is_some() {
                        return Err(norito::json::Error::duplicate_field("nanos"));
                    }
                    nanos = Some(map.parse_value::<u32>()?);
                }
                _ => {
                    map.skip_value()?;
                }
            }
        }
        map.finish()?;
        let secs = secs.ok_or_else(|| norito::json::Error::missing_field("secs"))?;
        let nanos = nanos.ok_or_else(|| norito::json::Error::missing_field("nanos"))?;
        Ok(Uptime(
            Duration::from_secs(secs) + Duration::from_nanos(u64::from(nanos)),
        ))
    }
}
#[cfg(test)]
mod serde_tests {
    use super::*;
    use norito::{from_bytes, to_bytes};
    #[test]
    fn uptime_json_roundtrip() {
        let uptime = Uptime(Duration::new(5, 123));
        let json = norito::json::to_json(&uptime).expect("serialize uptime");
        assert_eq!(json, "{\"secs\":5,\"nanos\":123}");
        let decoded: Uptime = norito::json::from_json(&json).expect("deserialize uptime");
        assert_eq!(decoded.0, uptime.0);
    }
    #[test]
    fn status_json_roundtrip() {
        let status = Status {
            build: BuildStatus {
                version: "2.0.0-rc.test".to_owned(),
                git_commit_sha: "deadbeef".to_owned(),
                dpn_validator_release_commit: "feedface".to_owned(),
                cargo_features: "telemetry,zk-halo2".to_owned(),
                target_triple: "aarch64-apple-darwin".to_owned(),
            },
            peers: 3,
            blocks: 42,
            blocks_non_empty: 39,
            commit_time_ms: 12,
            txs_approved: 7,
            txs_rejected: 2,
            last_rejection_at_ms: Some(100),
            txs_rejected_recent_5m: 2,
            uptime: Uptime(Duration::new(9, 0)),
            view_changes: 4,
            queue_size: 5,
            observed_at_ms: 1_000,
            queue_queued: 3,
            queue_inflight: 2,
            last_block_committed_at_ms: 900,
            last_non_empty_block_committed_at_ms: 800,
            time_since_last_block_ms: 100,
            time_since_last_non_empty_block_ms: 200,
            crypto: CryptoStatus {
                sm_helpers_available: true,
                sm_openssl_preview_enabled: false,
                halo2: Halo2Status::default(),
            },
            stack: StackStatus::default(),
            offline: None,
            sumeragi: Some(SumeragiConsensusStatus::default()),
            governance: GovernanceStatus::default(),
            teu_lane_commit: Vec::new(),
            teu_dataspace_backlog: Vec::new(),
            dataspace_catalog: Vec::new(),
            nexus: None,
            tx_gossip: TxGossipSnapshot::default(),
            sorafs_micropayments: Vec::new(),
            taikai_alias_rotations: Vec::new(),
            taikai_ingest: Vec::new(),
            da_receipt_cursors: Vec::new(),
        };
        let json = norito::json::to_json(&status).expect("serialize status");
        let decoded: Status = norito::json::from_json(&json).expect("deserialize status");
        assert_eq!(decoded.peers, status.peers);
        assert_eq!(decoded.uptime.0, status.uptime.0);
    }
    #[test]
    fn sumeragi_consensus_status_norito_preserves_tx_queue_pressure_causes() {
        let status = SumeragiConsensusStatus {
            tx_queue_depth: 31,
            tx_queue_capacity: 64,
            tx_queue_retained_bytes: 98_304,
            tx_queue_max_retained_bytes: 131_072,
            tx_queue_saturated: true,
            tx_queue_saturated_by_count: false,
            tx_queue_saturated_by_bytes: true,
            tx_queue_saturated_by_age: true,
            tx_queue_oldest_queued_age_ms: 7_500,
            ..SumeragiConsensusStatus::default()
        };
        let bytes = to_bytes(&status).expect("encode sumeragi consensus status");
        let archived = from_bytes::<SumeragiConsensusStatus>(&bytes)
            .expect("archive sumeragi consensus status");
        let decoded: SumeragiConsensusStatus =
            norito::core::NoritoDeserialize::deserialize(archived);
        assert_eq!(decoded.tx_queue_depth, 31);
        assert_eq!(decoded.tx_queue_capacity, 64);
        assert_eq!(decoded.tx_queue_retained_bytes, 98_304);
        assert_eq!(decoded.tx_queue_max_retained_bytes, 131_072);
        assert!(decoded.tx_queue_saturated);
        assert!(!decoded.tx_queue_saturated_by_count);
        assert!(decoded.tx_queue_saturated_by_bytes);
        assert!(decoded.tx_queue_saturated_by_age);
        assert_eq!(decoded.tx_queue_oldest_queued_age_ms, 7_500);
    }
    #[test]
    fn status_stack_snapshot_exports_sizes() {
        let metrics = Metrics::default();
        let snapshot = StackSettingsSnapshot {
            requested_scheduler_bytes: 16 * 1024,
            requested_prover_bytes: 24 * 1024,
            requested_guest_bytes: 32 * 1024,
            scheduler_bytes: 64 * 1024,
            prover_bytes: 64 * 1024,
            guest_bytes: 64 * 1024,
            scheduler_clamped: true,
            prover_clamped: true,
            guest_clamped: true,
            pool_fallback_total: 2,
            budget_hit_total: 3,
            gas_to_stack_multiplier: 8,
        };
        record_stack_limits(snapshot);
        let snapshot_readback = stack_settings_snapshot();
        assert_eq!(
            snapshot_readback.pool_fallback_total, snapshot.pool_fallback_total,
            "stack snapshot should retain pool fallback count"
        );
        assert_eq!(
            snapshot_readback.budget_hit_total, snapshot.budget_hit_total,
            "stack snapshot should retain budget clamp count"
        );
        metrics.apply_stack_snapshot(&stack_settings_snapshot());
        let status = Status::from(&metrics);
        assert_eq!(status.stack.scheduler_bytes, snapshot.scheduler_bytes);
        assert_eq!(
            status.stack.gas_to_stack_multiplier,
            snapshot.gas_to_stack_multiplier
        );
        assert!(
            status.stack.guest_clamped,
            "guest clamp flag should surface in status"
        );
        assert_eq!(
            status.stack.pool_fallback_total,
            snapshot.pool_fallback_total
        );
        assert_eq!(
            metrics
                .ivm_stack_clamped
                .with_label_values(&["guest"])
                .get(),
            1,
            "clamp gauge should mark guest clamp"
        );
        assert_eq!(
            metrics.ivm_stack_budget_hit_total.get(),
            snapshot.budget_hit_total
        );
        record_stack_limits(StackSettingsSnapshot::default());
        metrics.apply_stack_snapshot(&stack_settings_snapshot());
    }
}
impl TypeId for Uptime {
    fn id() -> Ident {
        "Uptime".to_owned()
    }
}
impl IntoSchema for Uptime {
    fn type_name() -> Ident {
        Self::id()
    }
    fn update_schema_map(metamap: &mut MetaMap) {
        metamap.insert::<Self>(Metadata::Tuple(UnnamedFieldsMeta {
            types: vec![
                core::any::TypeId::of::<u64>(),
                core::any::TypeId::of::<u32>(),
            ],
        }));
    }
}
/// TEU bucket contributions for a lane envelope (per slot).
#[allow(missing_copy_implementations)]
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    IntoSchema,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
pub struct NexusLaneTeuBuckets {
    /// TEU sourced from configured per-lane floor allocation.
    pub floor: u64,
    /// TEU sourced from headroom scheduling after floor reservations.
    pub headroom: u64,
    /// TEU consumed by the must-serve slice (starvation guard).
    pub must_serve: u64,
    /// TEU consumed after circuit-breaker adjustments (caps lowered).
    pub circuit_breaker: u64,
}
#[allow(dead_code)]
impl NexusLaneTeuBuckets {
    const LABELS: [&'static str; 4] = ["floor", "headroom", "must_serve", "circuit_breaker"];
    /// Returns an iterator over bucket labels paired with their TEU amounts.
    pub fn iter(self) -> impl Iterator<Item = (&'static str, u64)> {
        [
            (Self::LABELS[0], self.floor),
            (Self::LABELS[1], self.headroom),
            (Self::LABELS[2], self.must_serve),
            (Self::LABELS[3], self.circuit_breaker),
        ]
        .into_iter()
    }
}
impl norito::core::NoritoSerialize for NexusLaneTeuBuckets {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        let payload = (
            self.floor,
            self.headroom,
            self.must_serve,
            self.circuit_breaker,
        );
        norito::core::NoritoSerialize::serialize(&payload, writer)
    }
}
impl<'a> norito::core::NoritoDeserialize<'a> for NexusLaneTeuBuckets {
    fn deserialize(archived: &'a norito::core::Archived<Self>) -> Self {
        let (floor, headroom, must_serve, circuit_breaker) =
            norito::core::NoritoDeserialize::deserialize(archived.cast());
        Self {
            floor,
            headroom,
            must_serve,
            circuit_breaker,
        }
    }
}
impl<'a> DecodeFromSlice<'a> for NexusLaneTeuBuckets {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let ((floor, headroom, must_serve, circuit_breaker), used) =
            <(u64, u64, u64, u64)>::decode_from_slice(bytes)?;
        Ok((
            Self {
                floor,
                headroom,
                must_serve,
                circuit_breaker,
            },
            used,
        ))
    }
}
/// Fixed-length histogram for scheduler layer widths.
#[derive(Clone, Copy, Debug, Default, IntoSchema)]
pub struct SchedulerLayerWidthBuckets {
    buckets: [u64; 8],
}
impl SchedulerLayerWidthBuckets {
    /// Construct from an exact array of buckets.
    pub const fn new(buckets: [u64; 8]) -> Self {
        Self { buckets }
    }
    /// Construct from an arbitrary slice, truncating or zero-padding as needed.
    pub fn from_slice(values: &[u64]) -> Self {
        let mut buckets = [0u64; 8];
        let len = values.len().min(8);
        buckets[..len].copy_from_slice(&values[..len]);
        Self { buckets }
    }
    /// Convert into the inner array.
    pub const fn into_inner(self) -> [u64; 8] {
        self.buckets
    }
    /// Borrow the buckets slice.
    pub const fn as_slice(&self) -> &[u64; 8] {
        &self.buckets
    }
    /// Return the buckets as a `Vec`.
    pub fn to_vec(self) -> Vec<u64> {
        self.buckets.to_vec()
    }
}
impl From<[u64; 8]> for SchedulerLayerWidthBuckets {
    fn from(value: [u64; 8]) -> Self {
        Self::new(value)
    }
}
impl norito::json::FastJsonWrite for SchedulerLayerWidthBuckets {
    fn write_json(&self, out: &mut String) {
        out.push('[');
        for (idx, value) in self.buckets.iter().enumerate() {
            if idx > 0 {
                out.push(',');
            }
            value.json_serialize(out);
        }
        out.push(']');
    }
}
impl norito::json::JsonDeserialize for SchedulerLayerWidthBuckets {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let values = Vec::<u64>::json_deserialize(parser)?;
        if values.len() != 8 {
            return Err(norito::json::Error::Message(format!(
                "expected 8 histogram buckets, got {}",
                values.len()
            )));
        }
        let mut buckets = [0u64; 8];
        buckets.copy_from_slice(values.as_slice());
        Ok(Self { buckets })
    }
}
impl norito::core::NoritoSerialize for SchedulerLayerWidthBuckets {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        let payload = (
            self.buckets[0],
            self.buckets[1],
            self.buckets[2],
            self.buckets[3],
            self.buckets[4],
            self.buckets[5],
            self.buckets[6],
            self.buckets[7],
        );
        norito::core::NoritoSerialize::serialize(&payload, writer)
    }
}
impl<'a> norito::core::NoritoDeserialize<'a> for SchedulerLayerWidthBuckets {
    fn deserialize(archived: &'a norito::core::Archived<Self>) -> Self {
        let (b0, b1, b2, b3, b4, b5, b6, b7) =
            norito::core::NoritoDeserialize::deserialize(archived.cast());
        Self {
            buckets: [b0, b1, b2, b3, b4, b5, b6, b7],
        }
    }
}
impl<'a> DecodeFromSlice<'a> for SchedulerLayerWidthBuckets {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let ((b0, b1, b2, b3, b4, b5, b6, b7), used) =
            <(u64, u64, u64, u64, u64, u64, u64, u64)>::decode_from_slice(bytes)?;
        Ok((
            Self {
                buckets: [b0, b1, b2, b3, b4, b5, b6, b7],
            },
            used,
        ))
    }
}
impl std::ops::Index<usize> for SchedulerLayerWidthBuckets {
    type Output = u64;
    fn index(&self, index: usize) -> &Self::Output {
        &self.buckets[index]
    }
}
/// TEU deferral counters per lane.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    IntoSchema,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
pub struct NexusLaneTeuDeferrals {
    /// Deferred because the lane exceeded its configured TEU cap.
    pub cap_exceeded: u64,
    /// Deferred because the slot envelope hit a hard limit (e.g., bytes, witnesses).
    pub envelope_limit: u64,
    /// Deferred because per-dataspace or per-group quota limits triggered.
    pub quota: u64,
    /// Deferred because a circuit-breaker lowered the cap.
    pub circuit_breaker: u64,
}
#[allow(dead_code)]
impl NexusLaneTeuDeferrals {
    /// Increments the deferral counter corresponding to the provided reason.
    pub fn increment(&mut self, reason: &str, amount: u64) {
        match reason {
            "cap_exceeded" => self.cap_exceeded = self.cap_exceeded.saturating_add(amount),
            "envelope_limit" => {
                self.envelope_limit = self.envelope_limit.saturating_add(amount);
            }
            "quota" => self.quota = self.quota.saturating_add(amount),
            "circuit_breaker" => {
                self.circuit_breaker = self.circuit_breaker.saturating_add(amount);
            }
            _ => {}
        }
    }
}
impl norito::core::NoritoSerialize for NexusLaneTeuDeferrals {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        let payload = (
            self.cap_exceeded,
            self.envelope_limit,
            self.quota,
            self.circuit_breaker,
        );
        norito::core::NoritoSerialize::serialize(&payload, writer)
    }
}
impl<'a> norito::core::NoritoDeserialize<'a> for NexusLaneTeuDeferrals {
    fn deserialize(archived: &'a norito::core::Archived<Self>) -> Self {
        let (cap_exceeded, envelope_limit, quota, circuit_breaker) =
            norito::core::NoritoDeserialize::deserialize(archived.cast());
        Self {
            cap_exceeded,
            envelope_limit,
            quota,
            circuit_breaker,
        }
    }
}
impl<'a> DecodeFromSlice<'a> for NexusLaneTeuDeferrals {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let ((cap_exceeded, envelope_limit, quota, circuit_breaker), used) =
            <(u64, u64, u64, u64)>::decode_from_slice(bytes)?;
        Ok((
            Self {
                cap_exceeded,
                envelope_limit,
                quota,
                circuit_breaker,
            },
            used,
        ))
    }
}
/// Snapshot of per-lane TEU scheduling state exposed via `/status`.
#[derive(
    Clone,
    Debug,
    Default,
    IntoSchema,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
pub struct NexusLaneTeuStatus {
    /// Numeric lane identifier.
    pub lane_id: u32,
    /// Configured TEU capacity for the current slot.
    pub capacity: u64,
    /// TEU committed in the latest slot envelope for this lane.
    pub committed: u64,
    /// Bucket breakdown for committed TEU.
    pub buckets: NexusLaneTeuBuckets,
    /// Aggregated TEU deferral counters.
    pub deferrals: NexusLaneTeuDeferrals,
    /// Number of times the must-serve slice was truncated (cumulative).
    pub must_serve_truncations: u64,
    /// Current circuit-breaker trigger level (0 = normal).
    pub trigger_level: u64,
    /// Starvation bound configured for this lane (in slots).
    pub starvation_bound_slots: u64,
    /// Latest block height recorded for this lane.
    pub block_height: u64,
    /// Slots since this lane last reached the global head height.
    pub finality_lag_slots: u64,
    /// Pending settlement backlog for this lane (micro XOR units).
    pub settlement_backlog_xor_micro: u128,
    /// Transactions executed in the latest block for this lane.
    pub tx_vertices: u64,
    /// Conflict edges among transactions executed for this lane.
    pub tx_edges: u64,
    /// Overlay chunks applied for this lane.
    pub overlay_count: u64,
    /// Total overlay instructions executed for this lane.
    pub overlay_instr_total: u64,
    /// Total overlay bytes executed for this lane.
    pub overlay_bytes_total: u64,
    /// Approximate number of RBC chunks attributed to this lane.
    pub rbc_chunks: u64,
    /// Approximate total RBC payload bytes attributed to this lane.
    pub rbc_bytes_total: u64,
    /// Peak scheduler layer width observed for this lane.
    pub peak_layer_width: u64,
    /// Number of scheduler layers executed for this lane.
    pub layer_count: u64,
    /// Average scheduler layer width (rounded) for this lane.
    pub avg_layer_width: u64,
    /// Median scheduler layer width for this lane.
    pub median_layer_width: u64,
    /// Scheduler utilization percentage (0..100) for this lane.
    pub scheduler_utilization_pct: u64,
    /// Histogram buckets for scheduler layer widths (le = [1,2,4,8,16,32,64,128]).
    pub layer_width_buckets: SchedulerLayerWidthBuckets,
    /// Detached overlay executions prepared in the latest block.
    pub detached_prepared: u64,
    /// Detached overlay merges applied in the latest block.
    pub detached_merged: u64,
    /// Detached overlay fallbacks applied in the latest block.
    pub detached_fallback: u64,
    /// Quarantine transactions executed for this lane.
    pub quarantine_executed: u64,
    /// Whether the lane's governance configuration requires a manifest.
    pub manifest_required: bool,
    /// Whether a manifest has been loaded for the lane.
    pub manifest_ready: bool,
    /// Human-readable alias for the lane.
    pub alias: String,
    /// Dataspace identifier associated with the lane.
    pub dataspace_id: u64,
    /// Dataspace alias associated with the lane.
    pub dataspace_alias: Option<String>,
    /// Declarative lane visibility derived from configuration.
    pub visibility: Option<String>,
    /// Storage profile configured for the lane.
    pub storage_profile: String,
    /// Declarative lane profile/type derived from configuration.
    pub lane_type: Option<String>,
    /// Governance module identifier attached to the lane.
    pub governance: Option<String>,
    /// Settlement policy identifier attached to the lane.
    pub settlement: Option<String>,
    /// Optional scheduler TEU capacity override advertised via lane metadata.
    pub scheduler_teu_capacity_override: Option<u64>,
    /// Optional scheduler starvation bound override advertised via lane metadata.
    pub scheduler_starvation_bound_override: Option<u64>,
    /// Source path of the active governance manifest, if available.
    pub manifest_path: Option<String>,
    /// Validators declared in the lane's governance manifest.
    pub manifest_validators: Vec<String>,
    /// Validator-account, peer, and Torii bindings declared by the manifest.
    #[norito(default)]
    pub manifest_validator_bindings: Vec<NexusLaneManifestValidatorBindingStatus>,
    /// Validator quorum required by the lane manifest.
    pub manifest_quorum: Option<u32>,
    /// Protected namespaces enforced by the lane manifest.
    pub manifest_protected_namespaces: Vec<String>,
    /// Runtime-upgrade governance hook snapshot when configured.
    pub manifest_runtime_upgrade: Option<NexusLaneRuntimeUpgradeHookStatus>,
}
impl norito::core::NoritoSerialize for NexusLaneTeuStatus {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        norito::core::NoritoSerialize::serialize(&NexusLaneTeuStatusPayload::from(self), writer)
    }
}
impl<'a> norito::core::NoritoDeserialize<'a> for NexusLaneTeuStatus {
    fn deserialize(archived: &'a norito::core::Archived<Self>) -> Self {
        let payload = NexusLaneTeuStatusPayload::deserialize(archived.cast());
        payload.into()
    }
}
impl<'a> DecodeFromSlice<'a> for NexusLaneTeuStatus {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let payload = norito::codec::decode_adaptive::<NexusLaneTeuStatusPayload>(bytes)?;
        Ok((payload.into(), bytes.len()))
    }
}
#[derive(Clone, Debug, NoritoSerialize, NoritoDeserialize)]
struct NexusLaneTeuStatusPayload {
    lane_id: u32,
    capacity: u64,
    committed: u64,
    buckets: NexusLaneTeuBuckets,
    deferrals: NexusLaneTeuDeferrals,
    must_serve_truncations: u64,
    trigger_level: u64,
    starvation_bound_slots: u64,
    block_height: u64,
    finality_lag_slots: u64,
    settlement_backlog_xor_micro: u128,
    tx_vertices: u64,
    tx_edges: u64,
    overlay_count: u64,
    overlay_instr_total: u64,
    overlay_bytes_total: u64,
    rbc_chunks: u64,
    rbc_bytes_total: u64,
    peak_layer_width: u64,
    layer_count: u64,
    avg_layer_width: u64,
    median_layer_width: u64,
    scheduler_utilization_pct: u64,
    layer_width_buckets: SchedulerLayerWidthBuckets,
    detached_prepared: u64,
    detached_merged: u64,
    detached_fallback: u64,
    quarantine_executed: u64,
    manifest_required: bool,
    manifest_ready: bool,
    alias: String,
    dataspace_id: u64,
    dataspace_alias: Option<String>,
    visibility: Option<String>,
    storage_profile: String,
    lane_type: Option<String>,
    governance: Option<String>,
    settlement: Option<String>,
    scheduler_teu_capacity_override: Option<u64>,
    scheduler_starvation_bound_override: Option<u64>,
    manifest_path: Option<String>,
    manifest_validators: Vec<String>,
    manifest_quorum: Option<u32>,
    manifest_protected_namespaces: Vec<String>,
    manifest_runtime_upgrade: Option<NexusLaneRuntimeUpgradeHookStatus>,
    #[norito(default)]
    manifest_validator_bindings: Vec<NexusLaneManifestValidatorBindingStatus>,
}
impl From<&NexusLaneTeuStatus> for NexusLaneTeuStatusPayload {
    fn from(value: &NexusLaneTeuStatus) -> Self {
        Self {
            lane_id: value.lane_id,
            capacity: value.capacity,
            committed: value.committed,
            buckets: value.buckets,
            deferrals: value.deferrals,
            must_serve_truncations: value.must_serve_truncations,
            trigger_level: value.trigger_level,
            starvation_bound_slots: value.starvation_bound_slots,
            block_height: value.block_height,
            finality_lag_slots: value.finality_lag_slots,
            settlement_backlog_xor_micro: value.settlement_backlog_xor_micro,
            tx_vertices: value.tx_vertices,
            tx_edges: value.tx_edges,
            overlay_count: value.overlay_count,
            overlay_instr_total: value.overlay_instr_total,
            overlay_bytes_total: value.overlay_bytes_total,
            rbc_chunks: value.rbc_chunks,
            rbc_bytes_total: value.rbc_bytes_total,
            peak_layer_width: value.peak_layer_width,
            layer_count: value.layer_count,
            avg_layer_width: value.avg_layer_width,
            median_layer_width: value.median_layer_width,
            scheduler_utilization_pct: value.scheduler_utilization_pct,
            layer_width_buckets: value.layer_width_buckets,
            detached_prepared: value.detached_prepared,
            detached_merged: value.detached_merged,
            detached_fallback: value.detached_fallback,
            quarantine_executed: value.quarantine_executed,
            manifest_required: value.manifest_required,
            manifest_ready: value.manifest_ready,
            alias: value.alias.clone(),
            dataspace_id: value.dataspace_id,
            dataspace_alias: value.dataspace_alias.clone(),
            visibility: value.visibility.clone(),
            storage_profile: value.storage_profile.clone(),
            lane_type: value.lane_type.clone(),
            governance: value.governance.clone(),
            settlement: value.settlement.clone(),
            scheduler_teu_capacity_override: value.scheduler_teu_capacity_override,
            scheduler_starvation_bound_override: value.scheduler_starvation_bound_override,
            manifest_path: value.manifest_path.clone(),
            manifest_validators: value.manifest_validators.clone(),
            manifest_quorum: value.manifest_quorum,
            manifest_protected_namespaces: value.manifest_protected_namespaces.clone(),
            manifest_runtime_upgrade: value.manifest_runtime_upgrade.clone(),
            manifest_validator_bindings: value.manifest_validator_bindings.clone(),
        }
    }
}
impl From<NexusLaneTeuStatusPayload> for NexusLaneTeuStatus {
    fn from(payload: NexusLaneTeuStatusPayload) -> Self {
        Self {
            lane_id: payload.lane_id,
            capacity: payload.capacity,
            committed: payload.committed,
            buckets: payload.buckets,
            deferrals: payload.deferrals,
            must_serve_truncations: payload.must_serve_truncations,
            trigger_level: payload.trigger_level,
            starvation_bound_slots: payload.starvation_bound_slots,
            block_height: payload.block_height,
            finality_lag_slots: payload.finality_lag_slots,
            settlement_backlog_xor_micro: payload.settlement_backlog_xor_micro,
            tx_vertices: payload.tx_vertices,
            tx_edges: payload.tx_edges,
            overlay_count: payload.overlay_count,
            overlay_instr_total: payload.overlay_instr_total,
            overlay_bytes_total: payload.overlay_bytes_total,
            rbc_chunks: payload.rbc_chunks,
            rbc_bytes_total: payload.rbc_bytes_total,
            peak_layer_width: payload.peak_layer_width,
            layer_count: payload.layer_count,
            avg_layer_width: payload.avg_layer_width,
            median_layer_width: payload.median_layer_width,
            scheduler_utilization_pct: payload.scheduler_utilization_pct,
            layer_width_buckets: payload.layer_width_buckets,
            detached_prepared: payload.detached_prepared,
            detached_merged: payload.detached_merged,
            detached_fallback: payload.detached_fallback,
            quarantine_executed: payload.quarantine_executed,
            manifest_required: payload.manifest_required,
            manifest_ready: payload.manifest_ready,
            alias: payload.alias,
            dataspace_id: payload.dataspace_id,
            dataspace_alias: payload.dataspace_alias,
            visibility: payload.visibility,
            storage_profile: payload.storage_profile,
            lane_type: payload.lane_type,
            governance: payload.governance,
            settlement: payload.settlement,
            scheduler_teu_capacity_override: payload.scheduler_teu_capacity_override,
            scheduler_starvation_bound_override: payload.scheduler_starvation_bound_override,
            manifest_path: payload.manifest_path,
            manifest_validators: payload.manifest_validators,
            manifest_quorum: payload.manifest_quorum,
            manifest_protected_namespaces: payload.manifest_protected_namespaces,
            manifest_runtime_upgrade: payload.manifest_runtime_upgrade,
            manifest_validator_bindings: payload.manifest_validator_bindings,
        }
    }
}
/// Configured dataspace entry exposed through `/status` for preflight checks.
#[derive(
    Clone,
    Debug,
    Default,
    IntoSchema,
    NoritoSerialize,
    NoritoDeserialize,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
pub struct NexusDataspaceCatalogStatus {
    /// Numeric lane identifier that services this dataspace.
    pub lane_id: u32,
    /// Human-readable lane alias.
    pub lane_alias: String,
    /// Numeric dataspace identifier.
    pub dataspace_id: u64,
    /// Human-readable dataspace alias.
    pub alias: String,
    /// Declarative lane visibility.
    pub visibility: String,
    /// Storage profile configured for the lane.
    pub storage_profile: String,
    /// Whether the lane requires a governance manifest.
    pub manifest_required: bool,
    /// Whether the required governance manifest is loaded.
    pub manifest_ready: bool,
    /// Whether the lane is sealed because the manifest is not ready.
    pub sealed: bool,
    /// Source path of the active governance manifest, if available.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub manifest_path: Option<String>,
    /// Protected namespaces enforced by the lane manifest.
    #[norito(default)]
    #[norito(skip_serializing_if = "Vec::is_empty")]
    pub protected_namespaces: Vec<String>,
}
/// Effective Nexus routing policy exposed through `/status`.
#[derive(
    Clone,
    Debug,
    Default,
    IntoSchema,
    NoritoSerialize,
    NoritoDeserialize,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
pub struct NexusRoutingPolicyStatus {
    /// Lane used when no policy rule matches.
    pub default_lane: u32,
    /// Dataspace used when no policy rule overrides it explicitly.
    pub default_dataspace: u64,
    /// Ordered routing rules evaluated by Nexus.
    pub rules: Vec<NexusRoutingRuleStatus>,
}
/// Effective Nexus routing rule exposed through `/status`.
#[derive(
    Clone,
    Debug,
    Default,
    IntoSchema,
    NoritoSerialize,
    NoritoDeserialize,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
pub struct NexusRoutingRuleStatus {
    /// Target lane identifier for the rule.
    pub lane: u32,
    /// Target dataspace identifier for the rule, when explicitly configured.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub dataspace_id: Option<u64>,
    /// Rule matcher.
    pub matcher: NexusRoutingMatcherStatus,
}
/// Nexus routing rule matcher exposed through `/status`.
#[derive(
    Clone,
    Debug,
    Default,
    IntoSchema,
    NoritoSerialize,
    NoritoDeserialize,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
pub struct NexusRoutingMatcherStatus {
    /// Optional authority/account string match.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
    /// Optional instruction label match.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub instruction: Option<String>,
    /// Optional operator-facing description.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}
/// Nexus status snapshot exposed through `/status`.
#[derive(
    Clone,
    Debug,
    Default,
    IntoSchema,
    NoritoSerialize,
    NoritoDeserialize,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
pub struct NexusStatus {
    /// Effective routing policy enforced by Nexus routing.
    pub routing_policy: NexusRoutingPolicyStatus,
}
impl From<&ActualLaneRoutingPolicy> for NexusRoutingPolicyStatus {
    fn from(policy: &ActualLaneRoutingPolicy) -> Self {
        Self {
            default_lane: policy.default_lane.as_u32(),
            default_dataspace: policy.default_dataspace.as_u64(),
            rules: policy
                .rules
                .iter()
                .map(|rule| NexusRoutingRuleStatus {
                    lane: rule.lane.as_u32(),
                    dataspace_id: rule.dataspace.map(iroha_data_model::DataSpaceId::as_u64),
                    matcher: NexusRoutingMatcherStatus {
                        account: rule.matcher.account.clone(),
                        instruction: rule.matcher.instruction.clone(),
                        description: rule.matcher.description.clone(),
                    },
                })
                .collect(),
        }
    }
}
impl NexusStatus {
    /// Build a status snapshot from the effective Nexus routing policy.
    #[must_use]
    pub fn from_routing_policy(policy: &ActualLaneRoutingPolicy) -> Self {
        Self {
            routing_policy: NexusRoutingPolicyStatus::from(policy),
        }
    }
}
/// Snapshot of per-dataspace scheduler state exposed via `/status`.
#[derive(
    Clone,
    Debug,
    Default,
    IntoSchema,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
pub struct NexusDataspaceTeuStatus {
    /// Numeric lane identifier for the dataspace queue.
    pub lane_id: u32,
    /// Numeric dataspace identifier within the lane.
    pub dataspace_id: u64,
    /// Fault tolerance value (f) used to size lane relay committees.
    pub fault_tolerance: u32,
    /// Pending TEU demand left after scheduling the slot envelope.
    pub backlog: u64,
    /// Slots since the dataspace was last served.
    pub age_slots: u64,
    /// Latest SFQ virtual-finish tag for audit/debugging.
    pub virtual_finish: u64,
    /// Cumulative transactions executed for this dataspace since node start.
    pub tx_served: u64,
    /// Human-readable alias for the dataspace.
    pub alias: String,
    /// Optional description provided in configuration.
    pub description: Option<String>,
}
impl norito::core::NoritoSerialize for NexusDataspaceTeuStatus {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        let payload = (
            self.lane_id,
            self.dataspace_id,
            self.fault_tolerance,
            self.backlog,
            self.age_slots,
            self.virtual_finish,
            self.tx_served,
            self.alias.clone(),
            self.description.clone(),
        );
        norito::core::NoritoSerialize::serialize(&payload, writer)
    }
}
impl<'a> norito::core::NoritoDeserialize<'a> for NexusDataspaceTeuStatus {
    fn deserialize(archived: &'a norito::core::Archived<Self>) -> Self {
        let (
            lane_id,
            dataspace_id,
            fault_tolerance,
            backlog,
            age_slots,
            virtual_finish,
            tx_served,
            alias,
            description,
        ) = norito::core::NoritoDeserialize::deserialize(archived.cast());
        Self {
            lane_id,
            dataspace_id,
            fault_tolerance,
            backlog,
            age_slots,
            virtual_finish,
            tx_served,
            alias,
            description,
        }
    }
}
impl<'a> DecodeFromSlice<'a> for NexusDataspaceTeuStatus {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let (
            (
                lane_id,
                dataspace_id,
                fault_tolerance,
                backlog,
                age_slots,
                virtual_finish,
                tx_served,
                alias,
                description,
            ),
            used,
        ) = <(u32, u64, u32, u64, u64, u64, u64, String, Option<String>)>::decode_from_slice(
            bytes,
        )?;
        Ok((
            Self {
                lane_id,
                dataspace_id,
                fault_tolerance,
                backlog,
                age_slots,
                virtual_finish,
                tx_served,
                alias,
                description,
            },
            used,
        ))
    }
}
/// Snapshot of core consensus state exposed via `/status`.
#[derive(
    Clone,
    Debug,
    Default,
    IntoSchema,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "first-release consensus telemetry exposes independent status flags without compatibility aliases"
)]
pub struct SumeragiConsensusStatus {
    /// Current runtime consensus mode tag.
    pub mode_tag: String,
    /// Current leader index (topology position).
    pub leader_index: u64,
    /// HighestQC height.
    pub highest_qc_height: u64,
    /// LockedQC height.
    pub locked_qc_height: u64,
    /// LockedQC view.
    pub locked_qc_view: u64,
    /// Signatures present on the most recently committed block.
    #[norito(default)]
    pub commit_signatures_present: u64,
    /// Signatures counted toward the commit quorum.
    #[norito(default)]
    pub commit_signatures_counted: u64,
    /// Signatures contributed by set-B validators.
    #[norito(default)]
    pub commit_signatures_set_b: u64,
    /// Required commit quorum size for the active topology.
    #[norito(default)]
    pub commit_signatures_required: u64,
    /// Latest commit certificate height (best-effort).
    #[norito(default)]
    pub commit_qc_height: u64,
    /// Latest commit certificate view (best-effort).
    #[norito(default)]
    pub commit_qc_view: u64,
    /// Latest commit certificate epoch (best-effort).
    #[norito(default)]
    pub commit_qc_epoch: u64,
    /// Signatures attached to the latest commit certificate.
    #[norito(default)]
    pub commit_qc_signatures_total: u64,
    /// Validator-set size for the latest commit certificate.
    #[norito(default)]
    pub commit_qc_validator_set_len: u64,
    /// Total gossip fallback invocations (collectors exhausted).
    pub gossip_fallback_total: u64,
    /// Total BlockCreated drops due to locked QC gate.
    pub block_created_dropped_by_lock_total: u64,
    /// Total BlockCreated drops due to hint mismatches.
    pub block_created_hint_mismatch_total: u64,
    /// Total BlockCreated drops due to proposal mismatches.
    pub block_created_proposal_mismatch_total: u64,
    /// Current number of transactions observed in the local queue.
    pub tx_queue_depth: u64,
    /// Configured queue capacity on this peer.
    pub tx_queue_capacity: u64,
    /// Estimated retained queue bytes on this peer.
    #[norito(default)]
    pub tx_queue_retained_bytes: u64,
    /// Configured retained queue byte budget on this peer.
    #[norito(default)]
    pub tx_queue_max_retained_bytes: u64,
    /// Whether the local transaction queue is saturated.
    pub tx_queue_saturated: bool,
    /// Whether the local transaction queue is saturated by transaction count.
    #[norito(default)]
    pub tx_queue_saturated_by_count: bool,
    /// Whether the local transaction queue is saturated by retained bytes.
    #[norito(default)]
    pub tx_queue_saturated_by_bytes: bool,
    /// Whether the local transaction queue is saturated by oldest queued age.
    #[norito(default)]
    pub tx_queue_saturated_by_age: bool,
    /// Oldest queued transaction age in milliseconds.
    #[norito(default)]
    pub tx_queue_oldest_queued_age_ms: u64,
    /// Epoch length in blocks (NPoS mode; zero when not applicable).
    #[norito(default)]
    pub epoch_length_blocks: u64,
    /// Commit window deadline offset from epoch start (blocks; zero when not applicable).
    #[norito(default)]
    pub epoch_commit_deadline_offset: u64,
    /// Reveal window deadline offset from epoch start (blocks; zero when not applicable).
    #[norito(default)]
    pub epoch_reveal_deadline_offset: u64,
    /// PRF epoch seed (hex) used for deterministic leader/collector selection (NPoS mode).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    prf_epoch_seed: Option<String>,
    /// Height associated with the recorded PRF context.
    #[norito(default)]
    pub prf_height: u64,
    /// View associated with the recorded PRF context.
    #[norito(default)]
    pub prf_view: u64,
    /// Total view-change proofs accepted (advanced the proof chain).
    #[norito(default)]
    pub view_change_proof_accepted_total: u64,
    /// Total view-change proofs ignored as stale/outdated.
    #[norito(default)]
    pub view_change_proof_stale_total: u64,
    /// Total view-change proofs rejected as invalid.
    #[norito(default)]
    pub view_change_proof_rejected_total: u64,
    /// Total view-change suggestions emitted locally.
    #[norito(default)]
    pub view_change_suggest_total: u64,
    /// Total installed view changes (proof advanced locally).
    #[norito(default)]
    pub view_change_install_total: u64,
    /// Total lanes that remain sealed awaiting governance manifests.
    #[norito(default)]
    pub lane_governance_sealed_total: u32,
    /// Aliases of lanes that remain sealed awaiting governance manifests.
    #[norito(default)]
    pub lane_governance_sealed_aliases: Vec<String>,
}
impl norito::core::NoritoSerialize for SumeragiConsensusStatus {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        let payload = SumeragiConsensusStatusPayload::from(self);
        norito::core::NoritoSerialize::serialize(&payload, writer)
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        SumeragiConsensusStatusPayload::from(self).encoded_len_hint()
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        SumeragiConsensusStatusPayload::from(self).encoded_len_exact()
    }
}
impl<'a> norito::core::NoritoDeserialize<'a> for SumeragiConsensusStatus {
    fn deserialize(archived: &'a norito::core::Archived<Self>) -> Self {
        let payload = SumeragiConsensusStatusPayload::deserialize(archived.cast());
        payload.into()
    }
}
impl<'a> norito::core::DecodeFromSlice<'a> for SumeragiConsensusStatus {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let payload = norito::codec::decode_adaptive::<SumeragiConsensusStatusPayload>(bytes)?;
        Ok((payload.into(), bytes.len()))
    }
}
#[derive(Clone, Debug, NoritoSerialize, NoritoDeserialize)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "serialized consensus telemetry payload mirrors independent first-release status flags"
)]
struct SumeragiConsensusStatusPayload {
    mode_tag: String,
    leader_index: u64,
    highest_qc_height: u64,
    locked_qc_height: u64,
    locked_qc_view: u64,
    gossip_fallback_total: u64,
    block_created_dropped_by_lock_total: u64,
    block_created_hint_mismatch_total: u64,
    block_created_proposal_mismatch_total: u64,
    tx_queue_depth: u64,
    tx_queue_capacity: u64,
    tx_queue_saturated: bool,
    epoch_length_blocks: u64,
    epoch_commit_deadline_offset: u64,
    epoch_reveal_deadline_offset: u64,
    prf_epoch_seed: Option<String>,
    prf_height: u64,
    prf_view: u64,
    view_change_proof_accepted_total: u64,
    view_change_proof_stale_total: u64,
    view_change_proof_rejected_total: u64,
    view_change_suggest_total: u64,
    view_change_install_total: u64,
    lane_governance_sealed_total: u32,
    lane_governance_sealed_aliases: Vec<String>,
    commit_signatures_present: u64,
    commit_signatures_counted: u64,
    commit_signatures_set_b: u64,
    commit_signatures_required: u64,
    commit_qc_height: u64,
    commit_qc_view: u64,
    commit_qc_epoch: u64,
    commit_qc_signatures_total: u64,
    commit_qc_validator_set_len: u64,
    tx_queue_retained_bytes: u64,
    tx_queue_max_retained_bytes: u64,
    tx_queue_saturated_by_count: bool,
    tx_queue_saturated_by_bytes: bool,
    tx_queue_saturated_by_age: bool,
    tx_queue_oldest_queued_age_ms: u64,
}
fn decode_field<'a, T: DecodeFromSlice<'a>>(
    bytes: &'a [u8],
    used: &mut usize,
) -> Result<T, norito::core::Error> {
    let (value, len) = T::decode_from_slice(&bytes[*used..])?;
    *used += len;
    Ok(value)
}
fn decode_prf_fields(
    bytes: &[u8],
    used: &mut usize,
) -> Result<(Option<String>, u64, u64), norito::core::Error> {
    if *used >= bytes.len() {
        return Ok((None, 0, 0));
    }
    let seed = decode_field::<Option<String>>(bytes, used)?;
    if *used >= bytes.len() {
        return Ok((seed, 0, 0));
    }
    let height = decode_field::<u64>(bytes, used)?;
    if *used >= bytes.len() {
        return Ok((seed, height, 0));
    }
    let view = decode_field::<u64>(bytes, used)?;
    Ok((seed, height, view))
}
fn decode_epoch_fields(
    bytes: &[u8],
    used: &mut usize,
) -> Result<(u64, u64, u64), norito::core::Error> {
    if *used >= bytes.len() {
        return Ok((0, 0, 0));
    }
    let length = decode_field::<u64>(bytes, used)?;
    if *used >= bytes.len() {
        return Ok((length, 0, 0));
    }
    let commit = decode_field::<u64>(bytes, used)?;
    if *used >= bytes.len() {
        return Ok((length, commit, 0));
    }
    let reveal = decode_field::<u64>(bytes, used)?;
    Ok((length, commit, reveal))
}
fn decode_view_change_fields(
    bytes: &[u8],
    used: &mut usize,
) -> Result<(u64, u64, u64, u64, u64), norito::core::Error> {
    if *used >= bytes.len() {
        return Ok((0, 0, 0, 0, 0));
    }
    let accepted = decode_field::<u64>(bytes, used)?;
    if *used >= bytes.len() {
        return Ok((accepted, 0, 0, 0, 0));
    }
    let stale = decode_field::<u64>(bytes, used)?;
    if *used >= bytes.len() {
        return Ok((accepted, stale, 0, 0, 0));
    }
    let rejected = decode_field::<u64>(bytes, used)?;
    if *used >= bytes.len() {
        return Ok((accepted, stale, rejected, 0, 0));
    }
    let suggest = decode_field::<u64>(bytes, used)?;
    if *used >= bytes.len() {
        return Ok((accepted, stale, rejected, suggest, 0));
    }
    let install = decode_field::<u64>(bytes, used)?;
    Ok((accepted, stale, rejected, suggest, install))
}
#[allow(clippy::type_complexity)]
fn decode_commit_fields(
    bytes: &[u8],
    used: &mut usize,
) -> Result<(u64, u64, u64, u64, u64, u64, u64, u64, u64), norito::core::Error> {
    if *used >= bytes.len() {
        return Ok((0, 0, 0, 0, 0, 0, 0, 0, 0));
    }
    let present = decode_field::<u64>(bytes, used)?;
    if *used >= bytes.len() {
        return Ok((present, 0, 0, 0, 0, 0, 0, 0, 0));
    }
    let counted = decode_field::<u64>(bytes, used)?;
    if *used >= bytes.len() {
        return Ok((present, counted, 0, 0, 0, 0, 0, 0, 0));
    }
    let set_b = decode_field::<u64>(bytes, used)?;
    if *used >= bytes.len() {
        return Ok((present, counted, set_b, 0, 0, 0, 0, 0, 0));
    }
    let required = decode_field::<u64>(bytes, used)?;
    if *used >= bytes.len() {
        return Ok((present, counted, set_b, required, 0, 0, 0, 0, 0));
    }
    let cert_height = decode_field::<u64>(bytes, used)?;
    if *used >= bytes.len() {
        return Ok((present, counted, set_b, required, cert_height, 0, 0, 0, 0));
    }
    let cert_view = decode_field::<u64>(bytes, used)?;
    if *used >= bytes.len() {
        return Ok((
            present,
            counted,
            set_b,
            required,
            cert_height,
            cert_view,
            0,
            0,
            0,
        ));
    }
    let cert_epoch = decode_field::<u64>(bytes, used)?;
    if *used >= bytes.len() {
        return Ok((
            present,
            counted,
            set_b,
            required,
            cert_height,
            cert_view,
            cert_epoch,
            0,
            0,
        ));
    }
    let cert_signatures = decode_field::<u64>(bytes, used)?;
    if *used >= bytes.len() {
        return Ok((
            present,
            counted,
            set_b,
            required,
            cert_height,
            cert_view,
            cert_epoch,
            cert_signatures,
            0,
        ));
    }
    let cert_validator_set_len = decode_field::<u64>(bytes, used)?;
    Ok((
        present,
        counted,
        set_b,
        required,
        cert_height,
        cert_view,
        cert_epoch,
        cert_signatures,
        cert_validator_set_len,
    ))
}
impl<'a> DecodeFromSlice<'a> for SumeragiConsensusStatusPayload {
    #[allow(clippy::too_many_lines)] // Decode enumerates every field in a fixed order for stable wire layouts.
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let mut used = 0;
        let mode_tag = decode_field::<String>(bytes, &mut used)?;
        let leader_index = decode_field::<u64>(bytes, &mut used)?;
        let highest_qc_height = decode_field::<u64>(bytes, &mut used)?;
        let locked_qc_height = decode_field::<u64>(bytes, &mut used)?;
        let locked_qc_view = decode_field::<u64>(bytes, &mut used)?;
        let gossip_fallback_total = decode_field::<u64>(bytes, &mut used)?;
        let block_created_dropped_by_lock_total = decode_field::<u64>(bytes, &mut used)?;
        let block_created_hint_mismatch_total = decode_field::<u64>(bytes, &mut used)?;
        let block_created_proposal_mismatch_total = decode_field::<u64>(bytes, &mut used)?;
        let tx_queue_depth = decode_field::<u64>(bytes, &mut used)?;
        let tx_queue_capacity = decode_field::<u64>(bytes, &mut used)?;
        let tx_queue_saturated = decode_field::<bool>(bytes, &mut used)?;
        let (epoch_length_blocks, epoch_commit_deadline_offset, epoch_reveal_deadline_offset) =
            decode_epoch_fields(bytes, &mut used)?;
        let (prf_epoch_seed, prf_height, prf_view) = decode_prf_fields(bytes, &mut used)?;
        let (
            view_change_proof_accepted_total,
            view_change_proof_stale_total,
            view_change_proof_rejected_total,
            view_change_suggest_total,
            view_change_install_total,
        ) = decode_view_change_fields(bytes, &mut used)?;
        let lane_governance_sealed_total = if used < bytes.len() {
            decode_field::<u32>(bytes, &mut used)?
        } else {
            0
        };
        let lane_governance_sealed_aliases = if used < bytes.len() {
            decode_field::<Vec<String>>(bytes, &mut used)?
        } else {
            Vec::new()
        };
        let (
            commit_signatures_present,
            commit_signatures_counted,
            commit_signatures_set_b,
            commit_signatures_required,
            commit_qc_height,
            commit_qc_view,
            commit_qc_epoch,
            commit_qc_signatures_total,
            commit_qc_validator_set_len,
        ) = decode_commit_fields(bytes, &mut used)?;
        let tx_queue_retained_bytes = if used < bytes.len() {
            decode_field::<u64>(bytes, &mut used)?
        } else {
            0
        };
        let tx_queue_max_retained_bytes = if used < bytes.len() {
            decode_field::<u64>(bytes, &mut used)?
        } else {
            0
        };
        let tx_queue_saturated_by_count = if used < bytes.len() {
            decode_field::<bool>(bytes, &mut used)?
        } else {
            false
        };
        let tx_queue_saturated_by_bytes = if used < bytes.len() {
            decode_field::<bool>(bytes, &mut used)?
        } else {
            false
        };
        let tx_queue_saturated_by_age = if used < bytes.len() {
            decode_field::<bool>(bytes, &mut used)?
        } else {
            false
        };
        let tx_queue_oldest_queued_age_ms = if used < bytes.len() {
            decode_field::<u64>(bytes, &mut used)?
        } else {
            0
        };
        Ok((
            Self {
                mode_tag,
                leader_index,
                highest_qc_height,
                locked_qc_height,
                locked_qc_view,
                gossip_fallback_total,
                block_created_dropped_by_lock_total,
                block_created_hint_mismatch_total,
                block_created_proposal_mismatch_total,
                tx_queue_depth,
                tx_queue_capacity,
                tx_queue_saturated,
                epoch_length_blocks,
                epoch_commit_deadline_offset,
                epoch_reveal_deadline_offset,
                prf_epoch_seed,
                prf_height,
                prf_view,
                view_change_proof_accepted_total,
                view_change_proof_stale_total,
                view_change_proof_rejected_total,
                view_change_suggest_total,
                view_change_install_total,
                lane_governance_sealed_total,
                lane_governance_sealed_aliases,
                commit_signatures_present,
                commit_signatures_counted,
                commit_signatures_set_b,
                commit_signatures_required,
                commit_qc_height,
                commit_qc_view,
                commit_qc_epoch,
                commit_qc_signatures_total,
                commit_qc_validator_set_len,
                tx_queue_retained_bytes,
                tx_queue_max_retained_bytes,
                tx_queue_saturated_by_count,
                tx_queue_saturated_by_bytes,
                tx_queue_saturated_by_age,
                tx_queue_oldest_queued_age_ms,
            },
            used,
        ))
    }
}
impl From<&SumeragiConsensusStatus> for SumeragiConsensusStatusPayload {
    fn from(status: &SumeragiConsensusStatus) -> Self {
        Self {
            mode_tag: status.mode_tag.clone(),
            leader_index: status.leader_index,
            highest_qc_height: status.highest_qc_height,
            locked_qc_height: status.locked_qc_height,
            locked_qc_view: status.locked_qc_view,
            gossip_fallback_total: status.gossip_fallback_total,
            block_created_dropped_by_lock_total: status.block_created_dropped_by_lock_total,
            block_created_hint_mismatch_total: status.block_created_hint_mismatch_total,
            block_created_proposal_mismatch_total: status.block_created_proposal_mismatch_total,
            tx_queue_depth: status.tx_queue_depth,
            tx_queue_capacity: status.tx_queue_capacity,
            tx_queue_retained_bytes: status.tx_queue_retained_bytes,
            tx_queue_max_retained_bytes: status.tx_queue_max_retained_bytes,
            tx_queue_saturated: status.tx_queue_saturated,
            tx_queue_saturated_by_count: status.tx_queue_saturated_by_count,
            tx_queue_saturated_by_bytes: status.tx_queue_saturated_by_bytes,
            tx_queue_saturated_by_age: status.tx_queue_saturated_by_age,
            tx_queue_oldest_queued_age_ms: status.tx_queue_oldest_queued_age_ms,
            epoch_length_blocks: status.epoch_length_blocks,
            epoch_commit_deadline_offset: status.epoch_commit_deadline_offset,
            epoch_reveal_deadline_offset: status.epoch_reveal_deadline_offset,
            prf_epoch_seed: status.prf_epoch_seed.clone(),
            prf_height: status.prf_height,
            prf_view: status.prf_view,
            view_change_proof_accepted_total: status.view_change_proof_accepted_total,
            view_change_proof_stale_total: status.view_change_proof_stale_total,
            view_change_proof_rejected_total: status.view_change_proof_rejected_total,
            view_change_suggest_total: status.view_change_suggest_total,
            view_change_install_total: status.view_change_install_total,
            lane_governance_sealed_total: status.lane_governance_sealed_total,
            lane_governance_sealed_aliases: status.lane_governance_sealed_aliases.clone(),
            commit_signatures_present: status.commit_signatures_present,
            commit_signatures_counted: status.commit_signatures_counted,
            commit_signatures_set_b: status.commit_signatures_set_b,
            commit_signatures_required: status.commit_signatures_required,
            commit_qc_height: status.commit_qc_height,
            commit_qc_view: status.commit_qc_view,
            commit_qc_epoch: status.commit_qc_epoch,
            commit_qc_signatures_total: status.commit_qc_signatures_total,
            commit_qc_validator_set_len: status.commit_qc_validator_set_len,
        }
    }
}
impl From<SumeragiConsensusStatusPayload> for SumeragiConsensusStatus {
    fn from(payload: SumeragiConsensusStatusPayload) -> Self {
        Self {
            mode_tag: payload.mode_tag,
            leader_index: payload.leader_index,
            highest_qc_height: payload.highest_qc_height,
            locked_qc_height: payload.locked_qc_height,
            locked_qc_view: payload.locked_qc_view,
            gossip_fallback_total: payload.gossip_fallback_total,
            block_created_dropped_by_lock_total: payload.block_created_dropped_by_lock_total,
            block_created_hint_mismatch_total: payload.block_created_hint_mismatch_total,
            block_created_proposal_mismatch_total: payload.block_created_proposal_mismatch_total,
            tx_queue_depth: payload.tx_queue_depth,
            tx_queue_capacity: payload.tx_queue_capacity,
            tx_queue_retained_bytes: payload.tx_queue_retained_bytes,
            tx_queue_max_retained_bytes: payload.tx_queue_max_retained_bytes,
            tx_queue_saturated: payload.tx_queue_saturated,
            tx_queue_saturated_by_count: payload.tx_queue_saturated_by_count,
            tx_queue_saturated_by_bytes: payload.tx_queue_saturated_by_bytes,
            tx_queue_saturated_by_age: payload.tx_queue_saturated_by_age,
            tx_queue_oldest_queued_age_ms: payload.tx_queue_oldest_queued_age_ms,
            epoch_length_blocks: payload.epoch_length_blocks,
            epoch_commit_deadline_offset: payload.epoch_commit_deadline_offset,
            epoch_reveal_deadline_offset: payload.epoch_reveal_deadline_offset,
            prf_epoch_seed: payload.prf_epoch_seed,
            prf_height: payload.prf_height,
            prf_view: payload.prf_view,
            view_change_proof_accepted_total: payload.view_change_proof_accepted_total,
            view_change_proof_stale_total: payload.view_change_proof_stale_total,
            view_change_proof_rejected_total: payload.view_change_proof_rejected_total,
            view_change_suggest_total: payload.view_change_suggest_total,
            view_change_install_total: payload.view_change_install_total,
            lane_governance_sealed_total: payload.lane_governance_sealed_total,
            lane_governance_sealed_aliases: payload.lane_governance_sealed_aliases,
            commit_signatures_present: payload.commit_signatures_present,
            commit_signatures_counted: payload.commit_signatures_counted,
            commit_signatures_set_b: payload.commit_signatures_set_b,
            commit_signatures_required: payload.commit_signatures_required,
            commit_qc_height: payload.commit_qc_height,
            commit_qc_view: payload.commit_qc_view,
            commit_qc_epoch: payload.commit_qc_epoch,
            commit_qc_signatures_total: payload.commit_qc_signatures_total,
            commit_qc_validator_set_len: payload.commit_qc_validator_set_len,
        }
    }
}
/// Cryptography-related status exposed via `/status`.
#[derive(
    Clone,
    Debug,
    IntoSchema,
    NoritoSerialize,
    NoritoDeserialize,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
pub struct CryptoStatus {
    /// Indicates whether SM helper syscalls are available in this build.
    #[norito(default)]
    pub sm_helpers_available: bool,
    /// Indicates whether the OpenSSL-backed SM preview helpers are enabled.
    #[norito(default)]
    pub sm_openssl_preview_enabled: bool,
    /// Halo2 verifier configuration snapshot.
    #[norito(default)]
    pub halo2: Halo2Status,
}
/// Snapshot of the active Halo2 verifier configuration.
#[derive(
    Clone,
    Debug,
    Default,
    IntoSchema,
    NoritoSerialize,
    NoritoDeserialize,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
pub struct Halo2Status {
    /// Whether Halo2 verification is enabled for the host.
    #[norito(default)]
    pub enabled: bool,
    /// Selected curve identifier (e.g., `pallas`, `pasta`).
    #[norito(default)]
    pub curve: String,
    /// Proof system backend (`ipa`, `unsupported`, etc.).
    #[norito(default)]
    pub backend: String,
    /// Maximum supported circuit size exponent (N = 2^k).
    #[norito(default)]
    pub max_k: u32,
    /// Soft verifier time budget in milliseconds.
    #[norito(default)]
    pub verifier_budget_ms: u64,
    /// Maximum proofs per batch verification.
    #[norito(default)]
    pub verifier_max_batch: u32,
}
#[allow(clippy::derivable_impls)]
impl Default for CryptoStatus {
    fn default() -> Self {
        Self {
            sm_helpers_available: cfg!(feature = "sm"),
            sm_openssl_preview_enabled: false,
            halo2: Halo2Status::default(),
        }
    }
}
/// Configured caps and frame limits for transaction gossip.
#[derive(
    Clone,
    Debug,
    IntoSchema,
    NoritoSerialize,
    NoritoDeserialize,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
pub struct TxGossipCaps {
    /// Max gossip frame size in bytes for transaction payloads.
    pub frame_cap_bytes: u64,
    /// Optional cap on public gossip targets (0 = broadcast).
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub public_target_cap: Option<u64>,
    /// Optional cap on restricted gossip targets (0 = commit topology).
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub restricted_target_cap: Option<u64>,
    /// Public-plane target reshuffle interval in milliseconds.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub public_target_reshuffle_ms: Option<u64>,
    /// Restricted-plane target reshuffle interval in milliseconds.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub restricted_target_reshuffle_ms: Option<u64>,
    /// Whether gossip for unknown dataspaces is dropped instead of routed via the restricted plane.
    #[norito(default)]
    pub drop_unknown_dataspace: bool,
    /// Fallback policy when restricted targets are unavailable (`drop` or `public_overlay`).
    #[norito(default)]
    pub restricted_fallback: String,
    /// Policy for restricted payloads when only the public overlay is available (`refuse` or `forward`).
    pub restricted_public_policy: String,
}
impl Default for TxGossipCaps {
    fn default() -> Self {
        Self {
            frame_cap_bytes: 0,
            public_target_cap: None,
            restricted_target_cap: None,
            public_target_reshuffle_ms: None,
            restricted_target_reshuffle_ms: None,
            drop_unknown_dataspace: false,
            restricted_fallback: "drop".to_string(),
            restricted_public_policy: "refuse".to_string(),
        }
    }
}
/// Snapshot of the most recent gossip target selection for a dataspace.
#[derive(
    Clone,
    Debug,
    Default,
    IntoSchema,
    NoritoSerialize,
    NoritoDeserialize,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
pub struct TxGossipStatus {
    /// Plane used for the gossip attempt (`public` or `restricted`).
    pub plane: String,
    /// Dataspace identifier.
    pub dataspace_id: u64,
    /// Human-friendly dataspace alias (if configured).
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub dataspace_alias: Option<String>,
    /// Lane ids included in the gossip batch.
    #[norito(default)]
    pub lane_ids: Vec<u32>,
    /// Number of peers targeted in the latest attempt.
    pub targets: u64,
    /// Peer ids targeted in the latest attempt.
    #[norito(default)]
    #[norito(skip_serializing_if = "Vec::is_empty")]
    pub target_peers: Vec<String>,
    /// Outcome of the latest attempt (`sent` or `dropped`).
    #[norito(default)]
    pub outcome: String,
    /// Whether restricted fallback was considered/used for this attempt.
    #[norito(default)]
    pub fallback_used: bool,
    /// Fallback surface used (e.g., `public_overlay`) when `fallback_used` is true.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub fallback_surface: Option<String>,
    /// Drop reason when the batch was refused.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Configured target cap for this plane (0 = broadcast/unlimited).
    pub target_cap: u64,
    /// Transactions included in the encoded frame.
    pub batch_txs: u64,
    /// Encoded frame length in bytes.
    pub frame_bytes: u64,
}
/// Aggregated transaction gossip snapshot for `/status`.
#[derive(
    Clone,
    Debug,
    Default,
    IntoSchema,
    NoritoSerialize,
    NoritoDeserialize,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
pub struct TxGossipSnapshot {
    /// Configured caps and frame limits.
    pub caps: TxGossipCaps,
    /// Latest target selections grouped by dataspace/plane.
    #[norito(default)]
    #[norito(skip_serializing_if = "Vec::is_empty")]
    pub targets: Vec<TxGossipStatus>,
}
/// Highest DA receipt sequence observed per lane/epoch.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    IntoSchema,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
pub struct DaReceiptCursorStatus {
    /// Numeric lane identifier.
    pub lane_id: u32,
    /// Epoch scoped to the lane.
    pub epoch: u64,
    /// Highest recorded receipt sequence for the lane/epoch.
    pub highest_sequence: u64,
}
/// Bounded per-lane state for DA receipt metrics.
#[derive(Clone, Copy, Debug, Default)]
struct DaReceiptMetricLane {
    cursor: Option<DaReceiptMetricCursor>,
}
/// Latest DA receipt cursor retained for one lane.
#[derive(Clone, Copy, Debug)]
struct DaReceiptMetricCursor {
    epoch: u64,
    highest_sequence: u64,
}
const DA_RECEIPT_OUTCOME_LABELS: [&str; 9] = [
    "stored",
    "duplicate",
    "duplicate_fingerprint_conflict",
    "receipt_conflict",
    "manifest_conflict",
    "stale_sequence",
    "sequence_gap",
    "error",
    "unknown",
];
fn bounded_da_receipt_outcome(outcome: &str) -> &'static str {
    match outcome {
        "stored" => "stored",
        "duplicate" => "duplicate",
        "duplicate_fingerprint_conflict" => "duplicate_fingerprint_conflict",
        "receipt_conflict" => "receipt_conflict",
        "manifest_conflict" => "manifest_conflict",
        "stale_sequence" => "stale_sequence",
        "sequence_gap" => "sequence_gap",
        "error" => "error",
        _ => "unknown",
    }
}
fn da_receipt_metric_lane(
    lanes: &mut BTreeMap<u32, DaReceiptMetricLane>,
    lane_id: u32,
) -> Option<&mut DaReceiptMetricLane> {
    let has_capacity = lanes.len() < MAX_ACTIVE_EXECUTION_LANES;
    match lanes.entry(lane_id) {
        std::collections::btree_map::Entry::Occupied(entry) => Some(entry.into_mut()),
        std::collections::btree_map::Entry::Vacant(entry) if has_capacity => {
            Some(entry.insert(DaReceiptMetricLane::default()))
        }
        std::collections::btree_map::Entry::Vacant(_) => None,
    }
}
fn update_da_receipt_metric_cursor(
    lane: &mut DaReceiptMetricLane,
    epoch: u64,
    sequence: u64,
) -> DaReceiptMetricCursor {
    let cursor = lane.cursor.get_or_insert(DaReceiptMetricCursor {
        epoch,
        highest_sequence: sequence,
    });
    if epoch > cursor.epoch {
        *cursor = DaReceiptMetricCursor {
            epoch,
            highest_sequence: sequence,
        };
    } else if epoch == cursor.epoch {
        cursor.highest_sequence = cursor.highest_sequence.max(sequence);
    }
    *cursor
}
/// Stack sizing snapshot for scheduler/prover pools and guest VMs.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    IntoSchema,
    NoritoSerialize,
    NoritoDeserialize,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
pub struct StackStatus {
    /// Requested scheduler stack size in bytes.
    #[norito(default)]
    pub requested_scheduler_bytes: u64,
    /// Requested prover stack size in bytes.
    #[norito(default)]
    pub requested_prover_bytes: u64,
    /// Requested guest stack size in bytes.
    #[norito(default)]
    pub requested_guest_bytes: u64,
    /// Applied scheduler stack size in bytes after clamping.
    #[norito(default)]
    pub scheduler_bytes: u64,
    /// Applied prover stack size in bytes after clamping.
    #[norito(default)]
    pub prover_bytes: u64,
    /// Applied guest stack size in bytes after clamping.
    #[norito(default)]
    pub guest_bytes: u64,
    /// Gas→stack multiplier currently in effect.
    #[norito(default)]
    pub gas_to_stack_multiplier: u64,
    /// Whether the scheduler stack request was clamped.
    #[norito(default)]
    pub scheduler_clamped: bool,
    /// Whether the prover stack request was clamped.
    #[norito(default)]
    pub prover_clamped: bool,
    /// Whether the guest stack request was clamped.
    #[norito(default)]
    pub guest_clamped: bool,
    /// Count of fallbacks to an existing Rayon pool when applying stack sizes.
    #[norito(default)]
    pub pool_fallback_total: u64,
    /// Count of VM constructions that hit the guest stack budget clamp.
    #[norito(default)]
    pub budget_hit_total: u64,
}
impl From<StackSettingsSnapshot> for StackStatus {
    fn from(snapshot: StackSettingsSnapshot) -> Self {
        Self {
            requested_scheduler_bytes: snapshot.requested_scheduler_bytes,
            requested_prover_bytes: snapshot.requested_prover_bytes,
            requested_guest_bytes: snapshot.requested_guest_bytes,
            scheduler_bytes: snapshot.scheduler_bytes,
            prover_bytes: snapshot.prover_bytes,
            guest_bytes: snapshot.guest_bytes,
            gas_to_stack_multiplier: snapshot.gas_to_stack_multiplier,
            scheduler_clamped: snapshot.scheduler_clamped,
            prover_clamped: snapshot.prover_clamped,
            guest_clamped: snapshot.guest_clamped,
            pool_fallback_total: snapshot.pool_fallback_total,
            budget_hit_total: snapshot.budget_hit_total,
        }
    }
}
/// Response body for the Torii GET `/status` endpoint.
#[derive(
    Clone,
    Debug,
    Default,
    IntoSchema,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
pub struct BuildStatus {
    /// Semantic version baked into this binary.
    pub version: String,
    /// Git commit SHA baked into this binary.
    pub git_commit_sha: String,
    /// DPN validator release commit baked into a Taira validator binary.
    pub dpn_validator_release_commit: String,
    /// Enabled Cargo features baked into this binary.
    pub cargo_features: String,
    /// Target triple used to compile this binary.
    pub target_triple: String,
}
impl BuildStatus {
    fn current() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            git_commit_sha: option_env!("VERGEN_GIT_SHA")
                .unwrap_or("unknown")
                .to_owned(),
            dpn_validator_release_commit: option_env!("IROHA_DPN_VALIDATOR_RELEASE_COMMIT")
                .unwrap_or("unknown")
                .to_owned(),
            cargo_features: option_env!("VERGEN_CARGO_FEATURES")
                .unwrap_or("unknown")
                .to_owned(),
            target_triple: option_env!("VERGEN_CARGO_TARGET_TRIPLE")
                .unwrap_or("unknown")
                .to_owned(),
        }
    }
}
/// Response body for the Torii GET `/status` endpoint.
#[derive(
    Clone,
    Debug,
    Default,
    IntoSchema,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
pub struct Status {
    /// Build metadata for the currently running node binary.
    #[norito(default)]
    pub build: BuildStatus,
    /// Millisecond UNIX timestamp when this status snapshot was observed.
    #[norito(default)]
    pub observed_at_ms: u64,
    /// Number of currently connected peers excluding the reporting peer
    pub peers: u64,
    /// Number of committed blocks (blockchain height)
    pub blocks: u64,
    /// Number of committed non-empty blocks
    pub blocks_non_empty: u64,
    /// Time (since block creation) it took for the latest block to be committed by _this_ peer
    pub commit_time_ms: u64,
    /// Number of approved transactions
    pub txs_approved: u64,
    /// Number of rejected transactions
    pub txs_rejected: u64,
    /// Millisecond UNIX timestamp when this node most recently observed rejected transactions.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub last_rejection_at_ms: Option<u64>,
    /// Number of rejected transactions observed by this node within the last five minutes.
    #[norito(default)]
    pub txs_rejected_recent_5m: u64,
    /// Uptime since genesis block creation
    pub uptime: Uptime,
    /// Number of view changes in the current round
    pub view_changes: u32,
    /// Number of transactions tracked by the queue (queued + in-flight)
    pub queue_size: u64,
    /// Number of transactions still queued for selection.
    #[norito(default)]
    pub queue_queued: u64,
    /// Number of transactions in-flight after selection.
    #[norito(default)]
    pub queue_inflight: u64,
    /// Millisecond UNIX timestamp when this peer last processed a committed block.
    #[norito(default)]
    pub last_block_committed_at_ms: u64,
    /// Millisecond UNIX timestamp when this peer last processed a committed non-empty block.
    #[norito(default)]
    pub last_non_empty_block_committed_at_ms: u64,
    /// Milliseconds since this peer last processed a committed block.
    #[norito(default)]
    pub time_since_last_block_ms: u64,
    /// Milliseconds since this peer last processed a committed non-empty block.
    #[norito(default)]
    pub time_since_last_non_empty_block_ms: u64,
    /// Cryptography feature snapshot (SM enablement flags).
    #[norito(default)]
    pub crypto: CryptoStatus,
    /// Stack sizing/configuration snapshot.
    #[norito(default)]
    pub stack: StackStatus,
    /// Universal offline-wallet protocol capability advertised by this build.
    ///
    /// This is never a node-health, startup, asset-enrollment, or dataspace readiness gate.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub offline: Option<OfflineStatus>,
    /// Summary of the consensus snapshot (leader, QCs, queue state).
    #[norito(skip_serializing_if = "Option::is_none")]
    pub sumeragi: Option<SumeragiConsensusStatus>,
    /// Governance telemetry snapshot (proposal counts, protections, activations)
    pub governance: GovernanceStatus,
    /// Nexus lane-level TEU scheduling snapshot
    pub teu_lane_commit: Vec<NexusLaneTeuStatus>,
    /// Nexus dataspace-level backlog snapshot
    pub teu_dataspace_backlog: Vec<NexusDataspaceTeuStatus>,
    /// Configured Nexus dataspace catalog joined with lane metadata.
    #[norito(default)]
    #[norito(skip_serializing_if = "Vec::is_empty")]
    pub dataspace_catalog: Vec<NexusDataspaceCatalogStatus>,
    /// Effective Nexus status derived from committed state/configuration.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub nexus: Option<NexusStatus>,
    /// Transaction gossip target/cap snapshots grouped by dataspace/plane.
    #[norito(default)]
    pub tx_gossip: TxGossipSnapshot,
    /// Latest SoraFS micropayment samples observed by this node.
    #[norito(default)]
    #[norito(skip_serializing_if = "Vec::is_empty")]
    pub sorafs_micropayments: Vec<MicropaymentSampleStatus>,
    /// Taikai alias rotation telemetry snapshots grouped by (cluster, event, stream).
    #[norito(default)]
    #[norito(skip_serializing_if = "Vec::is_empty")]
    pub taikai_alias_rotations: Vec<TaikaiAliasRotationStatus>,
    /// Taikai ingest telemetry snapshots grouped by (cluster, stream).
    #[norito(default)]
    #[norito(skip_serializing_if = "Vec::is_empty")]
    pub taikai_ingest: Vec<TaikaiIngestStatus>,
    /// Latest DA receipt cursor retained for each lane.
    #[norito(default)]
    #[norito(skip_serializing_if = "Vec::is_empty")]
    pub da_receipt_cursors: Vec<DaReceiptCursorStatus>,
}
#[derive(Clone, Debug, norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
struct StatusPayload {
    #[norito(default)]
    build: BuildStatus,
    #[norito(default)]
    observed_at_ms: u64,
    peers: u64,
    blocks: u64,
    blocks_non_empty: u64,
    commit_time_ms: u64,
    txs_approved: u64,
    txs_rejected: u64,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    last_rejection_at_ms: Option<u64>,
    #[norito(default)]
    txs_rejected_recent_5m: u64,
    uptime: Uptime,
    view_changes: u32,
    queue_size: u64,
    #[norito(default)]
    queue_queued: u64,
    #[norito(default)]
    queue_inflight: u64,
    #[norito(default)]
    last_block_committed_at_ms: u64,
    #[norito(default)]
    last_non_empty_block_committed_at_ms: u64,
    #[norito(default)]
    time_since_last_block_ms: u64,
    #[norito(default)]
    time_since_last_non_empty_block_ms: u64,
    #[norito(default)]
    crypto: CryptoStatus,
    #[norito(default)]
    stack: StackStatus,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    offline: Option<OfflineStatus>,
    #[norito(skip_serializing_if = "Option::is_none")]
    sumeragi: Option<SumeragiConsensusStatus>,
    governance: GovernanceStatus,
    teu_lane_commit: Vec<NexusLaneTeuStatus>,
    teu_dataspace_backlog: Vec<NexusDataspaceTeuStatus>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Vec::is_empty")]
    dataspace_catalog: Vec<NexusDataspaceCatalogStatus>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    nexus: Option<NexusStatus>,
    #[norito(default)]
    tx_gossip: TxGossipSnapshot,
    #[norito(default)]
    sorafs_micropayments: Vec<MicropaymentSampleStatus>,
    #[norito(default)]
    taikai_alias_rotations: Vec<TaikaiAliasRotationStatus>,
    #[norito(default)]
    taikai_ingest: Vec<TaikaiIngestStatus>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Vec::is_empty")]
    da_receipt_cursors: Vec<DaReceiptCursorStatus>,
}
impl From<&Status> for StatusPayload {
    fn from(status: &Status) -> Self {
        Self {
            build: status.build.clone(),
            observed_at_ms: status.observed_at_ms,
            peers: status.peers,
            blocks: status.blocks,
            blocks_non_empty: status.blocks_non_empty,
            commit_time_ms: status.commit_time_ms,
            txs_approved: status.txs_approved,
            txs_rejected: status.txs_rejected,
            last_rejection_at_ms: status.last_rejection_at_ms,
            txs_rejected_recent_5m: status.txs_rejected_recent_5m,
            uptime: status.uptime,
            view_changes: status.view_changes,
            queue_size: status.queue_size,
            queue_queued: status.queue_queued,
            queue_inflight: status.queue_inflight,
            last_block_committed_at_ms: status.last_block_committed_at_ms,
            last_non_empty_block_committed_at_ms: status.last_non_empty_block_committed_at_ms,
            time_since_last_block_ms: status.time_since_last_block_ms,
            time_since_last_non_empty_block_ms: status.time_since_last_non_empty_block_ms,
            crypto: status.crypto.clone(),
            stack: status.stack,
            offline: status.offline.clone(),
            sumeragi: status.sumeragi.clone(),
            governance: status.governance.clone(),
            teu_lane_commit: status.teu_lane_commit.clone(),
            teu_dataspace_backlog: status.teu_dataspace_backlog.clone(),
            dataspace_catalog: status.dataspace_catalog.clone(),
            nexus: status.nexus.clone(),
            tx_gossip: status.tx_gossip.clone(),
            sorafs_micropayments: status.sorafs_micropayments.clone(),
            taikai_alias_rotations: status.taikai_alias_rotations.clone(),
            taikai_ingest: status.taikai_ingest.clone(),
            da_receipt_cursors: status.da_receipt_cursors.clone(),
        }
    }
}
impl From<StatusPayload> for Status {
    fn from(payload: StatusPayload) -> Self {
        Self {
            build: payload.build,
            observed_at_ms: payload.observed_at_ms,
            peers: payload.peers,
            blocks: payload.blocks,
            blocks_non_empty: payload.blocks_non_empty,
            commit_time_ms: payload.commit_time_ms,
            txs_approved: payload.txs_approved,
            txs_rejected: payload.txs_rejected,
            last_rejection_at_ms: payload.last_rejection_at_ms,
            txs_rejected_recent_5m: payload.txs_rejected_recent_5m,
            uptime: payload.uptime,
            view_changes: payload.view_changes,
            queue_size: payload.queue_size,
            queue_queued: payload.queue_queued,
            queue_inflight: payload.queue_inflight,
            last_block_committed_at_ms: payload.last_block_committed_at_ms,
            last_non_empty_block_committed_at_ms: payload.last_non_empty_block_committed_at_ms,
            time_since_last_block_ms: payload.time_since_last_block_ms,
            time_since_last_non_empty_block_ms: payload.time_since_last_non_empty_block_ms,
            crypto: payload.crypto,
            stack: payload.stack,
            offline: payload.offline,
            sumeragi: payload.sumeragi,
            governance: payload.governance,
            teu_lane_commit: payload.teu_lane_commit,
            teu_dataspace_backlog: payload.teu_dataspace_backlog,
            dataspace_catalog: payload.dataspace_catalog,
            nexus: payload.nexus,
            tx_gossip: payload.tx_gossip,
            sorafs_micropayments: payload.sorafs_micropayments,
            taikai_alias_rotations: payload.taikai_alias_rotations,
            taikai_ingest: payload.taikai_ingest,
            da_receipt_cursors: payload.da_receipt_cursors,
        }
    }
}
impl<'a> DecodeFromSlice<'a> for Status {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let payload = norito::codec::decode_adaptive::<StatusPayload>(bytes)?;
        Ok((payload.into(), bytes.len()))
    }
}
/// Number of manifest activation records retained in telemetry snapshots.
pub const GOVERNANCE_MANIFEST_RECENT_CAP: usize = 8;
const REJECTION_RECENT_WINDOW_MS: u64 = 5 * 60 * 1_000;
const REJECTION_RECENT_EVENT_CAP: usize = 1_024;
/// Governance-related telemetry snapshot embedded into [`Status`].
#[derive(
    Clone,
    Debug,
    Default,
    IntoSchema,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
pub struct GovernanceStatus {
    /// Current proposal counts grouped by status.
    pub proposals: GovernanceProposalCounters,
    /// Protected-namespace enforcement counters.
    pub protected_namespace: GovernanceProtectedNamespaceCounters,
    /// Manifest admission outcomes observed at queue ingress.
    pub manifest_admission: GovernanceManifestAdmissionCounters,
    /// Manifest quorum enforcement counters.
    pub manifest_quorum: GovernanceManifestQuorumCounters,
    /// Recent manifest activations (most recent first).
    pub recent_manifest_activations: Vec<GovernanceManifestActivation>,
    /// Total lanes that remain sealed awaiting governance manifests.
    pub sealed_lanes_total: u32,
    /// Aliases of lanes that remain sealed awaiting governance manifests.
    pub sealed_lane_aliases: Vec<String>,
    /// Total registered citizens with an active bond.
    pub citizens_total: u64,
}
/// Counts of governance proposals per status.
#[derive(
    Copy,
    Clone,
    Debug,
    Default,
    IntoSchema,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
pub struct GovernanceProposalCounters {
    /// Proposals whose latest attempt is active or certified.
    pub proposed: u64,
    /// Proposals whose latest attempt was rejected.
    pub rejected: u64,
    /// Proposals that completed enactment.
    pub enacted: u64,
    /// Proposals whose certified compare-and-set predecessor was superseded.
    pub superseded: u64,
    /// Proposals whose certified effect failed atomically at execution.
    pub execution_failed: u64,
}
/// Counters tracking protected-namespace admission decisions.
#[derive(
    Copy,
    Clone,
    Debug,
    Default,
    IntoSchema,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
pub struct GovernanceProtectedNamespaceCounters {
    /// Total number of protected-namespace admission checks.
    pub total_checks: u64,
    /// Checks that passed and were allowed.
    pub allowed: u64,
    /// Checks that were rejected at admission time.
    pub rejected: u64,
}
/// Counters tracking manifest admission decisions (pre-quorum/protection breakdown).
#[derive(
    Copy,
    Clone,
    Debug,
    Default,
    IntoSchema,
    NoritoSerialize,
    NoritoDeserialize,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
pub struct GovernanceManifestAdmissionCounters {
    /// Total number of manifest admission checks.
    pub total_checks: u64,
    /// Admissions that succeeded.
    pub allowed: u64,
    /// Rejections due to missing manifest data.
    pub missing_manifest: u64,
    /// Rejections because the authority was not a manifest validator.
    pub non_validator_authority: u64,
    /// Rejections triggered by quorum enforcement.
    pub quorum_rejected: u64,
    /// Rejections triggered by protected-namespace policies.
    pub protected_namespace_rejected: u64,
    /// Rejections triggered by runtime hook policies.
    pub runtime_hook_rejected: u64,
}
/// Counters tracking manifest quorum enforcement.
#[derive(
    Copy,
    Clone,
    Debug,
    Default,
    IntoSchema,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
pub struct GovernanceManifestQuorumCounters {
    /// Total number of quorum evaluations.
    pub total_checks: u64,
    /// Evaluations that satisfied the manifest quorum.
    pub satisfied: u64,
    /// Evaluations rejected due to insufficient approvals.
    pub rejected: u64,
}
/// Record of a manifest activation produced by governance enactment.
#[derive(
    Clone,
    Debug,
    Default,
    IntoSchema,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
pub struct GovernanceManifestActivation {
    /// Canonical contract address whose manifest was activated.
    pub contract_address: String,
    /// Hex-encoded code hash pinned by the activation.
    pub code_hash_hex: String,
    /// Optional ABI hash associated with the activation.
    pub abi_hash_hex: Option<String>,
    /// Block height at which the activation was committed.
    pub height: u64,
    /// Wall-clock timestamp in milliseconds when the activation was recorded.
    pub activated_at_ms: u64,
}
impl norito::core::NoritoSerialize for Status {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        let payload = StatusPayload::from(self);
        norito::core::NoritoSerialize::serialize(&payload, writer)
    }
}
impl<'a> norito::core::NoritoDeserialize<'a> for Status {
    fn deserialize(archived: &'a norito::core::Archived<Status>) -> Self {
        let payload = StatusPayload::deserialize(archived.cast());
        payload.into()
    }
}
impl norito::core::NoritoSerialize for GovernanceStatus {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        let payload = (
            self.proposals,
            self.protected_namespace,
            self.manifest_admission,
            self.manifest_quorum,
            self.recent_manifest_activations.clone(),
            self.sealed_lanes_total,
            self.sealed_lane_aliases.clone(),
            self.citizens_total,
        );
        norito::core::NoritoSerialize::serialize(&payload, writer)
    }
}
impl<'a> norito::core::NoritoDeserialize<'a> for GovernanceStatus {
    fn deserialize(archived: &'a norito::core::Archived<GovernanceStatus>) -> Self {
        let (
            proposals,
            protected_namespace,
            manifest_admission,
            manifest_quorum,
            recent_manifest_activations,
            sealed_lanes_total,
            sealed_lane_aliases,
            citizens_total,
        ): (
            GovernanceProposalCounters,
            GovernanceProtectedNamespaceCounters,
            GovernanceManifestAdmissionCounters,
            GovernanceManifestQuorumCounters,
            Vec<GovernanceManifestActivation>,
            u32,
            Vec<String>,
            u64,
        ) = norito::core::NoritoDeserialize::deserialize(archived.cast());
        Self {
            proposals,
            protected_namespace,
            manifest_admission,
            manifest_quorum,
            recent_manifest_activations,
            sealed_lanes_total,
            sealed_lane_aliases,
            citizens_total,
        }
    }
}
impl<'a> DecodeFromSlice<'a> for GovernanceStatus {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let (
            (
                proposals,
                protected_namespace,
                manifest_admission,
                manifest_quorum,
                recent_manifest_activations,
                sealed_lanes_total,
                sealed_lane_aliases,
                citizens_total,
            ),
            used,
        ) = <(
            GovernanceProposalCounters,
            GovernanceProtectedNamespaceCounters,
            GovernanceManifestAdmissionCounters,
            GovernanceManifestQuorumCounters,
            Vec<GovernanceManifestActivation>,
            u32,
            Vec<String>,
            u64,
        )>::decode_from_slice(bytes)?;
        Ok((
            Self {
                proposals,
                protected_namespace,
                manifest_admission,
                manifest_quorum,
                recent_manifest_activations,
                sealed_lanes_total,
                sealed_lane_aliases,
                citizens_total,
            },
            used,
        ))
    }
}
impl norito::core::NoritoSerialize for GovernanceProposalCounters {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        let payload = (
            self.proposed,
            self.rejected,
            self.enacted,
            self.superseded,
            self.execution_failed,
        );
        norito::core::NoritoSerialize::serialize(&payload, writer)
    }
}
impl<'a> norito::core::NoritoDeserialize<'a> for GovernanceProposalCounters {
    fn deserialize(archived: &'a norito::core::Archived<GovernanceProposalCounters>) -> Self {
        let (proposed, rejected, enacted, superseded, execution_failed): (u64, u64, u64, u64, u64) =
            norito::core::NoritoDeserialize::deserialize(archived.cast());
        Self {
            proposed,
            rejected,
            enacted,
            superseded,
            execution_failed,
        }
    }
}
impl<'a> DecodeFromSlice<'a> for GovernanceProposalCounters {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let ((proposed, rejected, enacted, superseded, execution_failed), used) =
            <(u64, u64, u64, u64, u64)>::decode_from_slice(bytes)?;
        Ok((
            Self {
                proposed,
                rejected,
                enacted,
                superseded,
                execution_failed,
            },
            used,
        ))
    }
}
impl norito::core::NoritoSerialize for GovernanceProtectedNamespaceCounters {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        let payload = (self.total_checks, self.allowed, self.rejected);
        norito::core::NoritoSerialize::serialize(&payload, writer)
    }
}
impl<'a> norito::core::NoritoDeserialize<'a> for GovernanceProtectedNamespaceCounters {
    fn deserialize(
        archived: &'a norito::core::Archived<GovernanceProtectedNamespaceCounters>,
    ) -> Self {
        let (total_checks, allowed, rejected): (u64, u64, u64) =
            norito::core::NoritoDeserialize::deserialize(archived.cast());
        Self {
            total_checks,
            allowed,
            rejected,
        }
    }
}
impl<'a> DecodeFromSlice<'a> for GovernanceProtectedNamespaceCounters {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let ((total_checks, allowed, rejected), used) =
            <(u64, u64, u64)>::decode_from_slice(bytes)?;
        Ok((
            Self {
                total_checks,
                allowed,
                rejected,
            },
            used,
        ))
    }
}
impl<'a> DecodeFromSlice<'a> for GovernanceManifestAdmissionCounters {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let (
            (
                total_checks,
                allowed,
                missing_manifest,
                non_validator_authority,
                quorum_rejected,
                protected_namespace_rejected,
                runtime_hook_rejected,
            ),
            used,
        ) = <(u64, u64, u64, u64, u64, u64, u64)>::decode_from_slice(bytes)?;
        Ok((
            Self {
                total_checks,
                allowed,
                missing_manifest,
                non_validator_authority,
                quorum_rejected,
                protected_namespace_rejected,
                runtime_hook_rejected,
            },
            used,
        ))
    }
}
impl norito::core::NoritoSerialize for GovernanceManifestQuorumCounters {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        let payload = (self.total_checks, self.satisfied, self.rejected);
        norito::core::NoritoSerialize::serialize(&payload, writer)
    }
}
impl<'a> norito::core::NoritoDeserialize<'a> for GovernanceManifestQuorumCounters {
    fn deserialize(archived: &'a norito::core::Archived<GovernanceManifestQuorumCounters>) -> Self {
        let (total_checks, satisfied, rejected): (u64, u64, u64) =
            norito::core::NoritoDeserialize::deserialize(archived.cast());
        Self {
            total_checks,
            satisfied,
            rejected,
        }
    }
}
impl<'a> DecodeFromSlice<'a> for GovernanceManifestQuorumCounters {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let ((total_checks, satisfied, rejected), used) =
            <(u64, u64, u64)>::decode_from_slice(bytes)?;
        Ok((
            Self {
                total_checks,
                satisfied,
                rejected,
            },
            used,
        ))
    }
}
impl norito::core::NoritoSerialize for GovernanceManifestActivation {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        let payload = (
            self.contract_address.clone(),
            self.code_hash_hex.clone(),
            self.abi_hash_hex.clone(),
            self.height,
            self.activated_at_ms,
        );
        norito::core::NoritoSerialize::serialize(&payload, writer)
    }
}
impl<'a> norito::core::NoritoDeserialize<'a> for GovernanceManifestActivation {
    fn deserialize(archived: &'a norito::core::Archived<GovernanceManifestActivation>) -> Self {
        let (contract_address, code_hash_hex, abi_hash_hex, height, activated_at_ms): (
            String,
            String,
            Option<String>,
            u64,
            u64,
        ) = norito::core::NoritoDeserialize::deserialize(archived.cast());
        Self {
            contract_address,
            code_hash_hex,
            abi_hash_hex,
            height,
            activated_at_ms,
        }
    }
}
impl<'a> DecodeFromSlice<'a> for GovernanceManifestActivation {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let ((contract_address, code_hash_hex, abi_hash_hex, height, activated_at_ms), used) =
            <(String, String, Option<String>, u64, u64)>::decode_from_slice(bytes)?;
        Ok((
            Self {
                contract_address,
                code_hash_hex,
                abi_hash_hex,
                height,
                activated_at_ms,
            },
            used,
        ))
    }
}
fn build_sumeragi_status(metrics: &Metrics) -> SumeragiConsensusStatus {
    let commit_qc_height = metrics.sumeragi_commit_qc_height.get();
    let commit_qc_view = metrics.sumeragi_commit_qc_view.get();
    let highest_qc_height = metrics
        .sumeragi_highest_qc_height
        .get()
        .max(commit_qc_height);
    let raw_locked_qc_height = metrics.sumeragi_locked_qc_height.get();
    let raw_locked_qc_view = metrics.sumeragi_locked_qc_view.get();
    let (locked_qc_height, locked_qc_view) = if commit_qc_height > 0
        && (raw_locked_qc_height, raw_locked_qc_view) < (commit_qc_height, commit_qc_view)
    {
        (commit_qc_height, commit_qc_view)
    } else {
        (raw_locked_qc_height, raw_locked_qc_view)
    };
    SumeragiConsensusStatus {
        mode_tag: metrics.sumeragi_mode_tag(),
        leader_index: metrics.sumeragi_leader_index.get(),
        highest_qc_height,
        locked_qc_height,
        locked_qc_view,
        commit_signatures_present: metrics.sumeragi_commit_signatures_present.get(),
        commit_signatures_counted: metrics.sumeragi_commit_signatures_counted.get(),
        commit_signatures_set_b: metrics.sumeragi_commit_signatures_set_b.get(),
        commit_signatures_required: metrics.sumeragi_commit_signatures_required.get(),
        commit_qc_height,
        commit_qc_view,
        commit_qc_epoch: metrics.sumeragi_commit_qc_epoch.get(),
        commit_qc_signatures_total: metrics.sumeragi_commit_qc_signatures_total.get(),
        commit_qc_validator_set_len: metrics.sumeragi_commit_qc_validator_set_len.get(),
        gossip_fallback_total: metrics.sumeragi_gossip_fallback_total.get(),
        block_created_dropped_by_lock_total: metrics
            .sumeragi_block_created_dropped_by_lock_total
            .get(),
        block_created_hint_mismatch_total: metrics.sumeragi_block_created_hint_mismatch_total.get(),
        block_created_proposal_mismatch_total: metrics
            .sumeragi_block_created_proposal_mismatch_total
            .get(),
        tx_queue_depth: metrics.sumeragi_tx_queue_depth.get(),
        tx_queue_capacity: metrics.sumeragi_tx_queue_capacity.get(),
        tx_queue_retained_bytes: metrics.sumeragi_tx_queue_retained_bytes.get(),
        tx_queue_max_retained_bytes: metrics.sumeragi_tx_queue_max_retained_bytes.get(),
        tx_queue_saturated: metrics.sumeragi_tx_queue_saturated.get() != 0,
        tx_queue_saturated_by_count: metrics.sumeragi_tx_queue_saturated_by_count.get() != 0,
        tx_queue_saturated_by_bytes: metrics.sumeragi_tx_queue_saturated_by_bytes.get() != 0,
        tx_queue_saturated_by_age: metrics.sumeragi_tx_queue_saturated_by_age.get() != 0,
        tx_queue_oldest_queued_age_ms: metrics.sumeragi_tx_queue_oldest_queued_age_ms.get(),
        epoch_length_blocks: metrics.sumeragi_epoch_length_blocks.get(),
        epoch_commit_deadline_offset: metrics.sumeragi_epoch_commit_deadline_offset.get(),
        epoch_reveal_deadline_offset: metrics.sumeragi_epoch_reveal_deadline_offset.get(),
        view_change_proof_accepted_total: metrics
            .sumeragi_view_change_proof_total
            .with_label_values(&["accepted"])
            .get(),
        view_change_proof_stale_total: metrics
            .sumeragi_view_change_proof_total
            .with_label_values(&["stale"])
            .get(),
        view_change_proof_rejected_total: metrics
            .sumeragi_view_change_proof_total
            .with_label_values(&["rejected"])
            .get(),
        view_change_suggest_total: metrics.sumeragi_view_change_suggest_total.get(),
        view_change_install_total: metrics.sumeragi_view_change_install_total.get(),
        prf_epoch_seed: metrics
            .sumeragi_prf_epoch_seed_hex
            .read()
            .expect("sumeragi PRF seed lock poisoned")
            .clone(),
        prf_height: metrics.sumeragi_prf_height.get(),
        prf_view: metrics.sumeragi_prf_view.get(),
        lane_governance_sealed_total: u32::try_from(
            metrics
                .nexus_lane_governance_sealed_total
                .get()
                .min(u64::from(u32::MAX)),
        )
        .unwrap_or(u32::MAX),
        lane_governance_sealed_aliases: metrics.lane_governance_sealed_aliases(),
    }
}
fn governance_proposal_counters(metrics: &Metrics) -> GovernanceProposalCounters {
    let fetch = |label: &str| {
        metrics
            .governance_proposals_status
            .with_label_values(&[label])
            .get()
    };
    GovernanceProposalCounters {
        proposed: fetch("proposed"),
        rejected: fetch("rejected"),
        enacted: fetch("enacted"),
        superseded: fetch("superseded"),
        execution_failed: fetch("execution_failed"),
    }
}
fn governance_protected_namespace_counters(
    metrics: &Metrics,
) -> GovernanceProtectedNamespaceCounters {
    let allowed = metrics
        .governance_protected_namespace_total
        .with_label_values(&["allowed"])
        .get();
    let rejected = metrics
        .governance_protected_namespace_total
        .with_label_values(&["rejected"])
        .get();
    GovernanceProtectedNamespaceCounters {
        total_checks: allowed + rejected,
        allowed,
        rejected,
    }
}
fn governance_manifest_admission_counters(
    metrics: &Metrics,
) -> GovernanceManifestAdmissionCounters {
    let fetch = |label: &str| {
        metrics
            .governance_manifest_admission_total
            .with_label_values(&[label])
            .get()
    };
    let allowed = fetch("allowed");
    let missing_manifest = fetch("missing_manifest");
    let non_validator = fetch("non_validator_authority");
    let quorum_rejected = fetch("quorum_rejected");
    let protected_rejected = fetch("protected_namespace_rejected");
    let runtime_rejected = fetch("runtime_hook_rejected");
    GovernanceManifestAdmissionCounters {
        total_checks: allowed
            + missing_manifest
            + non_validator
            + quorum_rejected
            + protected_rejected
            + runtime_rejected,
        allowed,
        missing_manifest,
        non_validator_authority: non_validator,
        quorum_rejected,
        protected_namespace_rejected: protected_rejected,
        runtime_hook_rejected: runtime_rejected,
    }
}
fn governance_manifest_quorum_counters(metrics: &Metrics) -> GovernanceManifestQuorumCounters {
    let satisfied = metrics
        .governance_manifest_quorum_total
        .with_label_values(&["satisfied"])
        .get();
    let rejected = metrics
        .governance_manifest_quorum_total
        .with_label_values(&["rejected"])
        .get();
    GovernanceManifestQuorumCounters {
        total_checks: satisfied + rejected,
        satisfied,
        rejected,
    }
}
fn governance_recent_manifest_activations(metrics: &Metrics) -> Vec<GovernanceManifestActivation> {
    metrics
        .governance_manifest_recent
        .read()
        .expect("governance manifest cache lock poisoned")
        .iter()
        .cloned()
        .collect()
}
fn sealed_lanes_total(metrics: &Metrics) -> u32 {
    metrics
        .nexus_lane_governance_sealed_total
        .get()
        .min(u64::from(u32::MAX))
        .try_into()
        .unwrap_or(u32::MAX)
}
fn build_governance_status(metrics: &Metrics) -> GovernanceStatus {
    GovernanceStatus {
        proposals: governance_proposal_counters(metrics),
        protected_namespace: governance_protected_namespace_counters(metrics),
        manifest_admission: governance_manifest_admission_counters(metrics),
        manifest_quorum: governance_manifest_quorum_counters(metrics),
        recent_manifest_activations: governance_recent_manifest_activations(metrics),
        sealed_lanes_total: sealed_lanes_total(metrics),
        sealed_lane_aliases: metrics.lane_governance_sealed_aliases(),
        citizens_total: metrics.governance_citizens_total.get(),
    }
}
fn collect_teu_lane_commit(metrics: &Metrics) -> Vec<NexusLaneTeuStatus> {
    metrics
        .nexus_scheduler_lane_teu_status
        .read()
        .expect("lane TEU cache poisoned")
        .values()
        .cloned()
        .collect()
}
fn collect_dataspace_catalog(metrics: &Metrics) -> Vec<NexusDataspaceCatalogStatus> {
    let mut entries: Vec<_> = metrics
        .nexus_scheduler_lane_teu_status
        .read()
        .expect("lane TEU cache poisoned")
        .values()
        .map(|lane| {
            let alias = lane
                .dataspace_alias
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| format!("dataspace-{}", lane.dataspace_id));
            let visibility = lane
                .visibility
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "public".to_owned());
            NexusDataspaceCatalogStatus {
                lane_id: lane.lane_id,
                lane_alias: lane.alias.clone(),
                dataspace_id: lane.dataspace_id,
                alias,
                visibility,
                storage_profile: lane.storage_profile.clone(),
                manifest_required: lane.manifest_required,
                manifest_ready: lane.manifest_ready,
                sealed: lane.manifest_required && !lane.manifest_ready,
                manifest_path: lane.manifest_path.clone(),
                protected_namespaces: lane.manifest_protected_namespaces.clone(),
            }
        })
        .collect();
    entries.sort_by_key(|entry| (entry.lane_id, entry.dataspace_id));
    entries
}
fn collect_teu_dataspace_backlog(metrics: &Metrics) -> Vec<NexusDataspaceTeuStatus> {
    metrics
        .nexus_scheduler_dataspace_teu_status
        .read()
        .expect("dataspace TEU cache poisoned")
        .values()
        .cloned()
        .collect()
}
fn collect_da_receipt_cursors(metrics: &Metrics) -> Vec<DaReceiptCursorStatus> {
    metrics.da_receipt_cursor_status()
}
impl From<&Metrics> for Status {
    fn from(value: &Metrics) -> Self {
        let now_ms = current_unix_time_ms();
        let last_block_committed_at_ms = value.last_block_committed_at_ms.get();
        let last_non_empty_block_committed_at_ms = value.last_non_empty_block_committed_at_ms.get();
        let time_since_last_block_ms = if last_block_committed_at_ms == 0 {
            0
        } else {
            now_ms.saturating_sub(last_block_committed_at_ms)
        };
        let time_since_last_non_empty_block_ms = if last_non_empty_block_committed_at_ms == 0 {
            0
        } else {
            now_ms.saturating_sub(last_non_empty_block_committed_at_ms)
        };
        Self {
            build: BuildStatus::current(),
            observed_at_ms: now_ms,
            peers: value.connected_peers.get(),
            blocks: value.block_height.get(),
            blocks_non_empty: value.block_height_non_empty.get(),
            commit_time_ms: value.last_commit_time_ms.get(),
            txs_approved: value.txs.with_label_values(&["accepted"]).get(),
            txs_rejected: value.txs.with_label_values(&["rejected"]).get(),
            last_rejection_at_ms: value.last_rejection_at_ms(),
            txs_rejected_recent_5m: value.txs_rejected_recent_5m(now_ms),
            uptime: Uptime(Duration::from_millis(value.uptime_since_genesis_ms.get())),
            view_changes: value
                .view_changes
                .get()
                .try_into()
                .expect("INTERNAL BUG: Number of view changes exceeds u32::MAX"),
            queue_size: value.queue_size.get(),
            queue_queued: value.queue_queued.get(),
            queue_inflight: value.queue_inflight.get(),
            last_block_committed_at_ms,
            last_non_empty_block_committed_at_ms,
            time_since_last_block_ms,
            time_since_last_non_empty_block_ms,
            crypto: CryptoStatus {
                sm_helpers_available: cfg!(feature = "sm"),
                sm_openssl_preview_enabled: value.sm_openssl_preview.get() != 0,
                halo2: value
                    .halo2_status
                    .read()
                    .expect("halo2 status lock poisoned")
                    .clone(),
            },
            stack: stack_settings_snapshot().into(),
            offline: None,
            sumeragi: Some(build_sumeragi_status(value)),
            governance: build_governance_status(value),
            teu_lane_commit: collect_teu_lane_commit(value),
            teu_dataspace_backlog: collect_teu_dataspace_backlog(value),
            dataspace_catalog: collect_dataspace_catalog(value),
            nexus: None,
            tx_gossip: TxGossipSnapshot {
                caps: value
                    .tx_gossip_caps
                    .read()
                    .expect("tx gossip caps cache poisoned")
                    .clone(),
                targets: value
                    .tx_gossip_status
                    .read()
                    .expect("tx gossip status cache poisoned")
                    .clone(),
            },
            sorafs_micropayments: Vec::new(),
            taikai_alias_rotations: value.taikai_alias_rotation_status(),
            taikai_ingest: value.taikai_ingest_status(),
            da_receipt_cursors: collect_da_receipt_cursors(value),
        }
    }
}
impl<T> From<&T> for Status
where
    T: Deref<Target = Metrics>,
{
    fn from(value: &T) -> Self {
        Self::from(&**value)
    }
}
macro_rules! metric_field_type {
    (int_counter ($($args:tt)*)) => { IntCounter };
    (int_counter_vec ($($args:tt)*)) => { IntCounterVec };
    (int_gauge ($($args:tt)*)) => { IntGauge };
    (int_gauge_vec ($($args:tt)*)) => { IntGaugeVec };
    (gauge ($($args:tt)*)) => { GenericGauge<AtomicU64> };
    (gauge_vec ($($args:tt)*)) => { GenericGaugeVec<AtomicU64> };
    (float_counter_vec ($($args:tt)*)) => { CounterVec };
    (float_gauge ($($args:tt)*)) => { Gauge };
    (float_gauge_vec ($($args:tt)*)) => { GaugeVec };
    (histogram_with_buckets ($($args:tt)*)) => { Histogram };
    (histogram_vec ($($args:tt)*)) => { HistogramVec };
    (histogram_vec_with_buckets ($($args:tt)*)) => { HistogramVec };
    (view_changes_gauge ($($args:tt)*)) => { ViewChangesGauge };
    (dropped_messages_counter ($($args:tt)*)) => { DroppedMessagesCounter };
    (raw ($type:ty)) => { $type };
}
const EXPONENTIAL_LATENCY_BUCKETS_MS: [f64; 12] = [
    1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0, 128.0, 256.0, 512.0, 1_024.0, 2_048.0,
];
const MISSING_BLOCK_DWELL_BUCKETS_MS: [f64; 10] = [
    50.0, 100.0, 250.0, 500.0, 1_000.0, 2_500.0, 5_000.0, 10_000.0, 20_000.0, 60_000.0,
];
macro_rules! new_catalog_metric {
    ($metrics:ident, $field:ident, view_changes_gauge ()) => {
        $metrics.gauge(stringify!($field))
    };
    ($metrics:ident, $field:ident, dropped_messages_counter ()) => {
        $metrics.int_counter(stringify!($field))
    };
    ($metrics:ident, $field:ident, $kind:ident ()) => {
        $metrics.$kind(stringify!($field))
    };
    ($metrics:ident, $field:ident, $kind:ident ($($argument:expr),+ $(,)?)) => {
        $metrics.$kind(stringify!($field), $($argument),+)
    };
}
macro_rules! initialize_metrics {
    (@collect [$($fields:tt)*]) => { Self { $($fields)* } };
    (@collect [$($fields:tt)*] [$($field:ident)*] $($tail:tt)*) => {
        initialize_metrics!(@collect [$($fields)* $($field,)*] $($tail)*)
    };
    (@collect [$($fields:tt)*] $field:ident = $value:expr; $($tail:tt)*) => {
        initialize_metrics!(@collect [$($fields)* $field: $value,] $($tail)*)
    };
    ($($input:tt)*) => { initialize_metrics!(@collect [] $($input)*) };
}
macro_rules! define_metrics {
    (fields {$( $(#[$attribute:meta])* $visibility:vis $field:ident: $kind:ident ($($argument:tt)*); )*}
     prefix ($factory:ident) {$($prefix:tt)*}
     construct {$( [$($construct_field:ident)*] $( {$($construction_statement:tt)*} )? )*}
     suffix {$($suffix:tt)*}
     initialize ($result:ident) {$($initializer:tt)*} epilogue {$($epilogue:tt)*}) => {
        /// Prometheus metric registry plus cached status snapshots exposed by telemetry.
        pub struct Metrics { $( $(#[$attribute])* $visibility $field: metric_field_type!($kind ($($argument)*)), )* }
        impl Default for Metrics {
            #[allow(clippy::too_many_lines, clippy::similar_names, clippy::inconsistent_struct_constructor)]
            fn default() -> Self {
                $($prefix)*
                #[allow(unused_macro_rules)]
                macro_rules! construct_metric { $( ($field) => { new_catalog_metric!($factory, $field, $kind ($($argument)*)) }; )* }
                $(
                    $(let $construct_field;)*
                    $($construct_field = construct_metric!($construct_field);)*
                    $($($construction_statement)*)?
                )*
                $($suffix)*
                let $result = initialize_metrics!($($initializer)*);
                $($epilogue)*
            }
        }
    };
}
define_metrics! {
fields {
    /// Total number of transactions
    pub txs: int_counter_vec(&["type"]);
    /// Number of committed blocks (blockchain height)
    pub block_height: int_counter();
    /// Number of committed non-empty blocks
    pub block_height_non_empty: int_counter();
    /// Time (since block creation) it took for the latest block to reach _this_ peer
    pub last_commit_time_ms: gauge();
    /// Millisecond UNIX timestamp when this peer last processed a committed block.
    pub last_block_committed_at_ms: gauge();
    /// Millisecond UNIX timestamp when this peer last processed a committed non-empty block.
    pub last_non_empty_block_committed_at_ms: gauge();
    /// Block commit time trends
    pub commit_time_ms: histogram_with_buckets(
        prometheus::exponential_buckets(100.0, 4.0, 5).expect("inputs are valid"),
    );
    /// Slot duration histogram for NX-18 1-second finality tracking (milliseconds).
    pub slot_duration_ms: histogram_with_buckets(
        vec![
                        250.0, 500.0, 750.0, 1_000.0, 1_250.0, 1_500.0, 2_000.0, 3_000.0,
                    ],
    );
    /// Latest observed slot duration in milliseconds (mirrors NX-18 gauge requirement).
    pub slot_duration_ms_latest: gauge();
    /// Rolling data-availability quorum ratio (0–1) derived from slot outcomes.
    pub da_quorum_ratio: float_gauge();
    /// Number of currently connected peers excluding the reporting peer
    pub connected_peers: gauge();
    /// Cumulative peer churn events observed by the node (`connected` / `disconnected`).
    pub p2p_peer_churn_total: int_counter_vec(&["event"]);
    /// Uptime of the network, starting from commit of the genesis block
    pub uptime_since_genesis_ms: gauge();
    /// Number of domains.
    pub domains: gauge();
    /// Total number of users per domain
    pub accounts: gauge_vec(&["domain"]);
    /// Transaction amounts.
    pub tx_amounts: histogram_with_buckets(
        // Amounts can vary wildly.
                    // Capturing range
                    //   from -10^10 to 10^10
                    //   with the step of 2 decimal points (10 steps)
                    vec![
                        -10_00_00_00_00.0,
                        -10_00_00_00.0,
                        -10_00_00.0,
                        -10_00.0,
                        -10.0,
                        0.0,
                        10.0,
                        10_00.0,
                        10_00_00.0,
                        10_00_00_00.0,
                        10_00_00_00_00.0,
                    ],
    );
    /// Queries handled by this peer
    pub isi: int_counter_vec(&["type", "success_status"]);
    /// Query handle time Histogram
    pub isi_times: histogram_vec(&["type"]);
    /// Number of view changes in the current round
    pub view_changes: view_changes_gauge();
    /// Number of transactions tracked by the queue (queued + in-flight)
    pub queue_size: gauge();
    /// Number of transactions still queued for selection.
    pub queue_queued: gauge();
    /// Number of transactions in-flight after selection.
    pub queue_inflight: gauge();
    /// Kura fsync policy state (1=always, 2=batched).
    pub kura_fsync_enabled: gauge();
    /// Kura fsync failures grouped by target (data/index/hashes).
    pub kura_fsync_failures_total: int_counter_vec(&["target"]);
    /// Kura fsync latency histogram (milliseconds) grouped by target.
    pub kura_fsync_latency_ms: histogram_vec_with_buckets(
        EXPONENTIAL_LATENCY_BUCKETS_MS.to_vec(),
                    &["target"],
    );
    /// AMX prepare phase latency histogram (milliseconds) labelled by lane id.
    pub amx_prepare_ms: histogram_vec_with_buckets(
        EXPONENTIAL_LATENCY_BUCKETS_MS.to_vec(),
                    &["lane"],
    );
    /// AMX commit/merge phase latency histogram (milliseconds) labelled by lane id.
    pub amx_commit_ms: histogram_vec_with_buckets(
        EXPONENTIAL_LATENCY_BUCKETS_MS.to_vec(),
                    &["lane"],
    );
    /// AMX abort counter grouped by lane id and abort stage.
    pub amx_abort_total: int_counter_vec(&["lane", "stage"]);
    /// AXT policy validation failures grouped by lane and reason.
    pub axt_policy_reject_total: int_counter_vec(&["lane", "reason"]);
    /// Stable version hash (truncated to u64) for the active AXT policy snapshot.
    pub axt_policy_snapshot_version: gauge();
    /// Cache hydration events for AXT policy snapshots grouped by event label.
    pub axt_policy_snapshot_cache_events_total: int_counter_vec(&["event"]);
    /// Dataspace proof cache events grouped by event label.
    pub axt_proof_cache_events_total: int_counter_vec(&["event"]);
    /// Per-dataspace proof cache state (labels: dsid, status, manifest_root_hex, verified_slot; value = expiry_slot_with_skew).
    pub axt_proof_cache_state: int_gauge_vec(
        &["dsid", "status", "manifest_root_hex", "verified_slot"],
    );
    /// IVM execution latency histogram (milliseconds) labelled by lane id.
    pub ivm_exec_ms: histogram_vec_with_buckets(
        EXPONENTIAL_LATENCY_BUCKETS_MS.to_vec(),
                    &["lane"],
    );
    /// SM helper syscalls observed (grouped by kind/mode).
    pub sm_syscall_total: int_counter_vec(&["kind", "mode"]);
    /// SM helper syscall failures (grouped by kind/mode/reason).
    pub sm_syscall_failures_total: int_counter_vec(&["kind", "mode", "reason"]);
    /// Toggle state for the OpenSSL-backed SM preview helpers (0/1).
    pub sm_openssl_preview: gauge();
    /// Toggle state for Halo2 verifier availability (0/1).
    pub zk_halo2_enabled: gauge();
    /// Active Halo2 curve identifier (as a numeric label).
    pub zk_halo2_curve_id: gauge();
    /// Active Halo2 backend identifier (as a numeric label).
    pub zk_halo2_backend_id: gauge();
    /// Maximum supported Halo2 circuit exponent (k).
    pub zk_halo2_max_k: gauge();
    /// Halo2 verifier soft budget in milliseconds.
    pub zk_halo2_verifier_budget_ms: gauge();
    /// Maximum proofs allowed in a Halo2 batch verification.
    pub zk_halo2_verifier_max_batch: gauge();
    /// Number of worker threads serving ZK lane verification.
    pub zk_halo2_verifier_worker_threads: gauge();
    /// Effective ZK lane queue capacity.
    pub zk_halo2_verifier_queue_cap: gauge();
    /// Count of ZK lane admissions that required a bounded wait.
    pub zk_lane_enqueue_wait_total: int_counter();
    /// Count of ZK lane admissions that timed out under saturation.
    pub zk_lane_enqueue_timeout_total: int_counter();
    /// ZK lane dropped-task counter labeled by terminal reason.
    pub zk_lane_drop_total: int_counter_vec(&["reason"]);
    /// Count of important tasks enqueued into the ZK lane retry ring.
    pub zk_lane_retry_enqueued_total: int_counter();
    /// Count of tasks replayed from the ZK lane retry ring.
    pub zk_lane_retry_replayed_total: int_counter();
    /// Count of tasks dropped after exhausting ZK lane retry attempts.
    pub zk_lane_retry_exhausted_total: int_counter();
    /// Current number of tasks buffered in ZK lane dispatch backlog.
    pub zk_lane_pending_depth: gauge();
    /// Current number of tasks buffered in the ZK lane retry ring.
    pub zk_lane_retry_ring_depth: gauge();
    /// Events emitted when verifier cache hits/misses occur (labels: cache,event).
    pub zk_verifier_cache_events_total: int_counter_vec(&["cache", "event"]);
    /// Base gas charged when verifying a confidential proof.
    pub confidential_gas_base_verify: gauge();
    /// Gas charged per public input exposed by a confidential proof.
    pub confidential_gas_per_public_input: gauge();
    /// Gas charged per byte of a confidential proof.
    pub confidential_gas_per_proof_byte: gauge();
    /// Gas charged per nullifier referenced by a confidential transaction.
    pub confidential_gas_per_nullifier: gauge();
    /// Gas charged per commitment emitted by a confidential transaction.
    pub confidential_gas_per_commitment: gauge();
    /// Lower 64 bits of the canonical IVM gas schedule hash.
    pub ivm_gas_schedule_hash_lo: gauge();
    /// Upper 64 bits of the canonical IVM gas schedule hash.
    pub ivm_gas_schedule_hash_hi: gauge();
    /// Requested/applied stack sizes for scheduler/prover/guest (bytes).
    pub ivm_stack_bytes: gauge_vec(&["kind", "state"]);
    /// Stack clamp flags for scheduler/prover/guest (0 = no clamp, 1 = clamped).
    pub ivm_stack_clamped: gauge_vec(&["kind"]);
    /// Gas→stack multiplier currently in effect.
    pub ivm_stack_gas_multiplier: gauge();
    /// Number of times a pre-existing global Rayon pool forced a stack-size fallback.
    pub ivm_stack_pool_fallback_total: int_counter();
    /// VM constructions that hit the guest stack budget clamp.
    pub ivm_stack_budget_hit_total: int_counter();
    /// Confidential Merkle-tree commitment counts per asset.
    pub confidential_tree_commitments: gauge_vec(&["asset_id"]);
    /// Confidential Merkle-tree depth per asset.
    pub confidential_tree_depth: gauge_vec(&["asset_id"]);
    /// Confidential Merkle-tree root history entries per asset.
    pub confidential_root_history_entries: gauge_vec(&["asset_id"]);
    /// Confidential frontier checkpoints per asset.
    pub confidential_frontier_checkpoints: gauge_vec(&["asset_id"]);
    /// Height of the latest recorded frontier checkpoint per asset.
    pub confidential_frontier_last_height: gauge_vec(&["asset_id"]);
    /// Commitment count captured at the latest frontier checkpoint per asset.
    pub confidential_frontier_last_commitments: gauge_vec(&["asset_id"]);
    /// Confidential root eviction counter per asset.
    pub confidential_root_evictions_total: int_counter_vec(&["asset_id"]);
    /// Confidential frontier eviction counter per asset.
    pub confidential_frontier_evictions_total: int_counter_vec(&["asset_id"]);
    /// Latest TWAP price exported by the oracle (local per XOR).
    pub oracle_price_local_per_xor: float_gauge();
    /// TWAP window length used by the oracle (seconds).
    pub oracle_twap_window_seconds: gauge();
    /// Effective haircut basis points applied by the oracle.
    pub oracle_haircut_basis_points: gauge();
    /// Oracle staleness (seconds) at the time of the last settlement quote.
    pub oracle_staleness_seconds: float_gauge();
    /// Count of observations aggregated per feed/slot.
    pub oracle_observations_total: int_counter_vec(&["feed_id"]);
    /// Aggregation wall-clock duration (milliseconds) grouped by feed.
    pub oracle_aggregation_duration_ms: histogram_vec_with_buckets(
        vec![1.0, 2.5, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0],
                    &["feed_id"],
    );
    /// Total oracle rewards emitted per feed (mantissa units).
    pub oracle_rewards_total: int_counter_vec(&["feed_id"]);
    /// Total oracle penalties applied per feed (mantissa units).
    pub oracle_penalties_total: int_counter_vec(&["feed_id"]);
    /// Total feed events aggregated per feed (regardless of evidence).
    pub oracle_feed_events_total: int_counter_vec(&["feed_id"]);
    /// Feed events that carried at least one evidence hash.
    pub oracle_feed_events_with_evidence_total: int_counter_vec(&["feed_id"]);
    /// Count of evidence hashes attached to feed events per feed.
    pub oracle_evidence_hashes_total: int_counter_vec(&["feed_id"]);
    /// FASTPQ execution mode resolutions grouped by requested/resolved/backend/device labels.
    pub fastpq_execution_mode_total: int_counter_vec(
        &[
                        "requested",
                        "resolved",
                        "backend",
                        "device_class",
                        "chip_family",
                        "gpu_kind",
                    ],
    );
    /// FASTPQ Poseidon pipeline resolutions grouped by requested/resolved/path/device labels.
    pub fastpq_poseidon_pipeline_total: int_counter_vec(
        &[
                        "requested",
                        "resolved",
                        "path",
                        "device_class",
                        "chip_family",
                        "gpu_kind",
                    ],
    );
    /// FASTPQ GPU accelerator disable events grouped by accelerator/reason/device labels.
    pub fastpq_gpu_disable_total: int_counter_vec(
        &[
                        "accelerator",
                        "reason",
                        "device_class",
                        "chip_family",
                        "gpu_kind",
                    ],
    );
    /// FASTPQ sampled GPU parity failures grouped by accelerator/reason/device labels.
    pub fastpq_gpu_parity_failure_total: int_counter_vec(
        &[
                        "accelerator",
                        "reason",
                        "device_class",
                        "chip_family",
                        "gpu_kind",
                    ],
    );
    /// FASTPQ proof sidecar queue depth.
    pub fastpq_proof_sidecar_queue_depth: gauge();
    /// FASTPQ proof sidecar persistence events grouped by event.
    pub fastpq_proof_sidecar_events_total: int_counter_vec(&["event"]);
    /// FASTPQ Metal queue duty-cycle ratios grouped by device/queue/metric.
    pub fastpq_metal_queue_ratio: float_gauge_vec(
        &["device_class", "chip_family", "gpu_kind", "queue", "metric"],
    );
    /// FASTPQ Metal queue depth snapshots grouped by device/metric.
    pub fastpq_metal_queue_depth: float_gauge_vec(
        &["device_class", "chip_family", "gpu_kind", "metric"],
    );
    /// FASTPQ host zero-fill duration samples grouped by device class (milliseconds).
    pub fastpq_zero_fill_duration_ms: float_gauge_vec(
        &["device_class", "chip_family", "gpu_kind"],
    );
    /// FASTPQ host zero-fill bandwidth grouped by device class (gigabits per second).
    pub fastpq_zero_fill_bandwidth_gbps: float_gauge_vec(
        &["device_class", "chip_family", "gpu_kind"],
    );
    /// Settlement events grouped by kind/outcome/reason.
    pub settlement_events_total: int_counter_vec(&["kind", "outcome", "reason"]);
    /// Settlement finality outcomes grouped by kind/outcome/state.
    pub settlement_finality_events_total: int_counter_vec(&["kind", "outcome", "final_state"],);
    /// PvP FX window observations (milliseconds between committed legs).
    pub settlement_fx_window_ms: histogram_vec(&["kind", "order", "atomicity"]);
    /// Per-lane/dataspace settlement buffer level recorded in micro XOR.
    pub settlement_buffer_xor: float_gauge_vec(&["lane_id", "dataspace_id"]);
    /// Configured settlement buffer capacity per lane/dataspace (micro XOR).
    pub settlement_buffer_capacity_xor: float_gauge_vec(&["lane_id", "dataspace_id"],);
    /// Encoded settlement buffer status (0 = normal, 1 = alert, 2 = throttle, 3 = XOR-only, 4 = halt).
    pub settlement_buffer_status: float_gauge_vec(&["lane_id", "dataspace_id"]);
    /// Per-lane/dataspace realised haircut variance recorded in micro XOR.
    pub settlement_pnl_xor: float_gauge_vec(&["lane_id", "dataspace_id"]);
    /// Effective haircut basis points applied per lane/dataspace in the latest block.
    pub settlement_haircut_bp: float_gauge_vec(&["lane_id", "dataspace_id"]);
    /// Swap-line utilisation snapshots (micro XOR) grouped by lane/dataspace/profile.
    pub settlement_swapline_utilisation: float_gauge_vec(&["lane_id", "dataspace_id", "profile"],);
    /// Settlement conversion counters grouped by lane/dataspace/source token.
    pub settlement_conversion_total: int_counter_vec(&["lane_id", "dataspace_id", "source_token"],);
    /// Cumulative settlement haircut totals grouped by lane/dataspace (XOR units).
    pub settlement_haircut_total: float_counter_vec(&["lane_id", "dataspace_id"]);
    /// Subscription billing attempts grouped by pricing kind.
    pub subscription_billing_attempts_total: int_counter_vec(&["pricing"]);
    /// Subscription billing outcomes grouped by pricing kind and result.
    pub subscription_billing_outcomes_total: int_counter_vec(&["pricing", "result"],);
    /// Viral incentive lifecycle events grouped by event kind.
    pub social_events_total: int_counter_vec(&["event"]);
    /// Latest viral reward budget spend for the active day.
    pub social_budget_spent: float_gauge();
    /// Campaign spend across the full promotion window.
    pub social_campaign_spent: float_gauge();
    /// Configured campaign cap (0 = unlimited).
    pub social_campaign_cap: float_gauge();
    /// Remaining campaign budget (0 when cap is unlimited).
    pub social_campaign_remaining: float_gauge();
    /// Whether the promotion window is active (1 = active, 0 = inactive).
    pub social_campaign_active: float_gauge();
    /// Whether the viral flows are halted (1 = halted, 0 = flowing).
    pub social_halted: float_gauge();
    /// Viral incentive rejections grouped by failure reason.
    pub social_rejections_total: int_counter_vec(&["reason"]);
    /// Multisig direct-sign validation rejections.
    pub multisig_direct_sign_reject_total: int_counter();
    /// Open viral escrows currently tracked on-ledger.
    pub social_open_escrows: gauge();
    /// Transactions currently queued as observed by consensus.
    pub sumeragi_tx_queue_depth: gauge();
    /// Transaction queue capacity observed by consensus.
    pub sumeragi_tx_queue_capacity: gauge();
    /// Estimated retained transaction queue bytes observed by consensus.
    pub sumeragi_tx_queue_retained_bytes: gauge();
    /// Retained transaction queue byte budget observed by consensus.
    pub sumeragi_tx_queue_max_retained_bytes: gauge();
    /// Queue saturation flag observed by consensus (0 = healthy, 1 = saturated).
    pub sumeragi_tx_queue_saturated: gauge();
    /// Transaction count saturation flag observed by consensus (0 = inactive, 1 = active).
    pub sumeragi_tx_queue_saturated_by_count: gauge();
    /// Retained-byte saturation flag observed by consensus (0 = inactive, 1 = active).
    pub sumeragi_tx_queue_saturated_by_bytes: gauge();
    /// Oldest-queued-age saturation flag observed by consensus (0 = inactive, 1 = active).
    pub sumeragi_tx_queue_saturated_by_age: gauge();
    /// Oldest queued transaction age in milliseconds observed by consensus.
    pub sumeragi_tx_queue_oldest_queued_age_ms: gauge();
    /// Total pending blocks tracked by consensus.
    pub sumeragi_pending_blocks_total: gauge();
    /// Pending blocks that currently gate proposals/view changes.
    pub sumeragi_pending_blocks_blocking: gauge();
    /// Commit inflight queue depth (inflight + queued commit work).
    pub sumeragi_commit_inflight_queue_depth: gauge();
    /// Outstanding missing-block requests observed locally.
    pub sumeragi_missing_block_requests: gauge();
    /// Age in milliseconds of the oldest missing-block request.
    pub sumeragi_missing_block_oldest_ms: gauge();
    /// Retry window for missing-block fetches in milliseconds.
    pub sumeragi_missing_block_retry_window_ms: gauge();
    /// Dwell time from first QC arrival until payload observation (milliseconds).
    pub sumeragi_missing_block_dwell_ms: histogram_with_buckets(
        MISSING_BLOCK_DWELL_BUCKETS_MS.to_vec(),
    );
    /// Epoch length in blocks for NPoS scheduling (0 when not applicable).
    pub sumeragi_epoch_length_blocks: gauge();
    /// Commit window deadline offset from epoch start, in blocks.
    pub sumeragi_epoch_commit_deadline_offset: gauge();
    /// Reveal window deadline offset from epoch start, in blocks.
    pub sumeragi_epoch_reveal_deadline_offset: gauge();
    /// Tiered state: entries retained in the hot tier after the latest snapshot.
    pub state_tiered_hot_entries: gauge();
    /// Tiered state: deterministic key-plus-value hot budget weight after the latest snapshot.
    pub state_tiered_hot_bytes: gauge();
    /// Tiered state: entries spilled to the cold tier after the latest snapshot.
    pub state_tiered_cold_entries: gauge();
    /// Tiered state: total bytes written to the cold tier in the latest snapshot.
    pub state_tiered_cold_bytes: gauge();
    /// Tiered state: cold entries reused without re-encoding in the latest snapshot.
    pub state_tiered_cold_reused_entries: gauge();
    /// Tiered state: total bytes reused from cold payloads in the latest snapshot.
    pub state_tiered_cold_reused_bytes: gauge();
    /// Tiered state: entries promoted into the hot tier in the latest snapshot.
    pub state_tiered_hot_promotions: gauge();
    /// Tiered state: entries demoted into the cold tier in the latest snapshot.
    pub state_tiered_hot_demotions: gauge();
    /// Tiered state: hot-tier key budget overflow in the latest snapshot.
    pub state_tiered_hot_budget_overflow_keys: gauge();
    /// Tiered state: hot-tier byte budget overflow in the latest snapshot.
    pub state_tiered_hot_budget_overflow_bytes: gauge();
    /// Tiered state: last recorded snapshot index.
    pub state_tiered_last_snapshot_index: gauge();
    /// Storage budget: bytes used per component.
    pub storage_budget_bytes_used: gauge_vec(&["component"]);
    /// Storage budget: configured cap per component.
    pub storage_budget_bytes_limit: gauge_vec(&["component"]);
    /// Storage budget: cap exceed events per component.
    pub storage_budget_exceeded_total: int_counter_vec(&["component"]);
    /// DA storage: cache outcomes per component.
    pub storage_da_cache_total: int_counter_vec(&["component", "result"]);
    /// DA storage: churn bytes per component and direction.
    pub storage_da_churn_bytes_total: int_counter_vec(&["component", "direction"]);
    /// Governance: proposal counts grouped by status
    pub governance_proposals_status: gauge_vec(&["status"]);
    /// Parliament: accepted lifecycle transitions grouped by a closed transition kind.
    pub governance_parliament_transitions_total: int_counter_vec(&["transition"]);
    /// Parliament: Core-derived no-result outcomes grouped by a closed failure class.
    pub governance_parliament_no_result_total: int_counter_vec(&["class"]);
    /// Parliament: committed attempt counts grouped by a closed status.
    pub governance_parliament_attempts_by_status: gauge_vec(&["status"]);
    /// Parliament: committed attempt counts grouped by a closed stage.
    pub governance_parliament_attempts_by_stage: gauge_vec(&["stage"]);
    /// Governance: latest council members count.
    pub governance_council_members: gauge();
    /// Governance: latest council alternates count.
    pub governance_council_alternates: gauge();
    /// Governance: total candidates considered in the latest draw.
    pub governance_council_candidates: gauge();
    /// Governance: epoch index of the latest persisted council.
    pub governance_council_epoch: gauge();
    /// Governance: total registered citizens.
    pub governance_citizens_total: gauge();
    /// Governance: citizen service discipline events (decline|no_show|misconduct).
    pub governance_citizen_service_events_total: int_counter_vec(&["event"]);
    /// Governance: protected-namespace enforcement counters (outcome = allowed|rejected)
    pub governance_protected_namespace_total: int_counter_vec(&["outcome"]);
    /// Governance: manifest admission outcomes (result label)
    pub governance_manifest_admission_total: int_counter_vec(&["result"]);
    /// Governance: manifest quorum enforcement counters (outcome = satisfied|rejected)
    pub governance_manifest_quorum_total: int_counter_vec(&["outcome"]);
    /// Governance: manifest hook enforcement counters (hook, outcome)
    pub governance_manifest_hook_total: int_counter_vec(&["hook", "outcome"]);
    /// Governance: manifest activation events (event = manifest_inserted|instance_bound)
    pub governance_manifest_activations_total: int_counter_vec(&["event"]);
    /// Governance: recent manifest activations kept for status snapshots
    pub governance_manifest_recent: raw(Arc<RwLock<VecDeque<GovernanceManifestActivation>>>);
    /// Governance: bond lifecycle events (lock_created|lock_extended|lock_unlocked).
    pub governance_bond_events_total: int_counter_vec(&["event"]);
    /// Cached Taikai ingest telemetry per (cluster, stream) for status snapshots.
    taikai_ingest_snapshots: raw(Arc<RwLock<BTreeMap<(String, String), TaikaiIngestSnapshotInternal>>>);
    /// Insertion order for Taikai ingest snapshots (bounded).
    taikai_ingest_snapshot_order: raw(Arc<RwLock<VecDeque<(String, String)>>>);
    /// Bounded DA receipt metric state keyed only by lane.
    da_receipt_metric_lanes: raw(Arc<RwLock<BTreeMap<u32, DaReceiptMetricLane>>>);
    /// Recent rejected-transaction batches retained for `/status` freshness reporting.
    recent_rejection_events: raw(Mutex<VecDeque<(u64, u64)>>);
    /// Millisecond UNIX timestamp when the latest rejected transaction batch was observed.
    last_rejection_at_ms: raw(StdAtomicU64);
    taikai_alias_rotation_snapshots: raw(TaikaiAliasRotationSnapshots);
    /// Insertion order for Taikai alias-rotation snapshots (bounded).
    taikai_alias_rotation_snapshot_order: raw(TaikaiAliasRotationSnapshotOrder);
    /// Alias service usage grouped by lane and event kind.
    pub alias_usage_total: int_counter_vec(&["lane", "event"]);
    /// PSP fraud: accepted assessments by tenant/band/lane/subnet
    pub fraud_psp_assessments_total: int_counter_vec(&["tenant", "band", "lane", "subnet"],);
    /// PSP fraud: transactions missing assessments (labeled by cause)
    pub fraud_psp_missing_assessment_total: int_counter_vec(
        &["tenant", "lane", "subnet", "cause"],
    );
    /// PSP fraud: invalid metadata fields encountered during admission
    pub fraud_psp_invalid_metadata_total: int_counter_vec(&["tenant", "field", "lane", "subnet"],);
    /// PSP fraud: attestation verification outcomes (tenant/engine/lane/status)
    pub fraud_psp_attestation_total: int_counter_vec(
        &["tenant", "engine", "lane", "subnet", "status"],
    );
    /// PSP fraud: latency histogram as reported by PSPs (milliseconds)
    pub fraud_psp_latency_ms: histogram_vec_with_buckets(
        prometheus::exponential_buckets(5.0, 1.8, 12).expect("inputs are valid"),
                    &["tenant", "lane", "subnet"],
    );
    /// PSP fraud: risk score distribution (basis points)
    pub fraud_psp_score_bps: histogram_vec_with_buckets(
        prometheus::linear_buckets(0.0, 500.0, 21).expect("inputs are valid"),
                    &["tenant", "band", "lane", "subnet"],
    );
    /// PSP fraud: outcome mismatches between scoring and PSP disposition
    pub fraud_psp_outcome_mismatch_total: int_counter_vec(
        &["tenant", "direction", "lane", "subnet"],
    );
    /// Streaming HPKE rekeys accepted grouped by suite identifier.
    pub streaming_hpke_rekeys_total: int_counter_vec(&["suite"]);
    /// Streaming content key rotations processed.
    pub streaming_gck_rotations_total: int_counter();
    /// Streaming QUIC datagrams sent.
    pub streaming_quic_datagrams_sent_total: int_counter();
    /// Streaming QUIC datagrams dropped.
    pub streaming_quic_datagrams_dropped_total: int_counter();
    /// Streaming FEC parity bucket occupancy.
    pub streaming_fec_parity_current: gauge_vec(&["bucket"]);
    /// Streaming feedback timeout events.
    pub streaming_feedback_timeout_total: int_counter();
    /// Telemetry redaction events grouped by reason.
    pub telemetry_redaction_total: int_counter_vec(&["reason"]);
    /// Telemetry redaction skips grouped by reason.
    pub telemetry_redaction_skipped_total: int_counter_vec(&["reason"]);
    /// Telemetry field truncations.
    pub telemetry_truncation_total: int_counter();
    /// Streaming privacy telemetry redaction failures.
    pub streaming_privacy_redaction_fail_total: int_counter();
    /// Streaming encode latency (milliseconds).
    pub streaming_encode_latency_ms: histogram_with_buckets(
        prometheus::exponential_buckets(1.0, 2.0, 10).expect("inputs are valid"),
    );
    /// Streaming encode audio jitter (milliseconds).
    pub streaming_encode_audio_jitter_ms: histogram_with_buckets(
        prometheus::exponential_buckets(0.5, 2.0, 10).expect("inputs are valid"),
    );
    /// Streaming encode maximum audio jitter (milliseconds).
    pub streaming_encode_audio_max_jitter_ms: gauge();
    /// ISO bridge reference-data status (-1 failed, 0 missing, 1 loaded).
    pub iso_reference_status: int_gauge_vec(&["dataset"]);
    /// ISO bridge reference-data age in seconds (per dataset).
    pub iso_reference_age_seconds: int_gauge_vec(&["dataset"]);
    /// ISO bridge reference-data record counts.
    pub iso_reference_records: int_gauge_vec(&["dataset"]);
    /// ISO bridge reference-data refresh interval (seconds).
    pub iso_reference_refresh_interval_secs: int_gauge_vec(&["dataset"]);
    /// Streaming encode dropped layers.
    pub streaming_encode_dropped_layers_total: int_counter();
    /// Streaming decode buffer size (milliseconds).
    pub streaming_decode_buffer_ms: histogram_with_buckets(
        prometheus::exponential_buckets(10.0, 1.8, 10).expect("inputs are valid"),
    );
    /// Streaming decode dropped frames.
    pub streaming_decode_dropped_frames_total: int_counter();
    /// Streaming decode maximum queue depth (milliseconds).
    pub streaming_decode_max_queue_ms: histogram_with_buckets(
        prometheus::exponential_buckets(10.0, 1.8, 10).expect("inputs are valid"),
    );
    /// Streaming decode audio/video drift (milliseconds, absolute average).
    pub streaming_decode_av_drift_ms: histogram_with_buckets(
        prometheus::exponential_buckets(0.5, 2.0, 10).expect("inputs are valid"),
    );
    /// Streaming decode maximum audio/video drift (milliseconds).
    pub streaming_decode_max_drift_ms: gauge();
    /// Viewer-reported audio jitter (milliseconds).
    pub streaming_audio_jitter_ms: histogram_with_buckets(
        prometheus::exponential_buckets(0.5, 2.0, 10).expect("inputs are valid"),
    );
    /// Viewer-reported maximum audio jitter (milliseconds).
    pub streaming_audio_max_jitter_ms: gauge();
    /// Viewer-reported audio/video drift (milliseconds, absolute average).
    pub streaming_av_drift_ms: histogram_with_buckets(
        prometheus::exponential_buckets(0.5, 2.0, 10).expect("inputs are valid"),
    );
    /// Viewer-reported maximum audio/video drift (milliseconds).
    pub streaming_av_max_drift_ms: gauge();
    /// Viewer-reported EWMA audio/video drift (milliseconds, signed).
    pub streaming_av_drift_ewma_ms: int_gauge();
    /// Aggregation window for viewer sync diagnostics (milliseconds).
    pub streaming_av_sync_window_ms: gauge();
    /// Viewer sync violations observed (count).
    pub streaming_av_sync_violation_total: int_counter();
    /// Streaming network round-trip time (milliseconds).
    pub streaming_network_rtt_ms: histogram_with_buckets(
        prometheus::exponential_buckets(1.0, 1.8, 12).expect("inputs are valid"),
    );
    /// Streaming network packet loss percentage (basis points).
    pub streaming_network_loss_percent_x100: histogram_with_buckets(
        prometheus::linear_buckets(0.0, 50.0, 21).expect("inputs are valid"),
    );
    /// Streaming network FEC repairs performed.
    pub streaming_network_fec_repairs_total: int_counter();
    /// Streaming network FEC failures encountered.
    pub streaming_network_fec_failures_total: int_counter();
    /// Streaming network datagram reinjects issued.
    pub streaming_network_datagram_reinjects_total: int_counter();
    /// Streaming energy consumption at encoder (milliwatts).
    pub streaming_energy_encoder_mw: histogram_with_buckets(
        prometheus::exponential_buckets(10.0, 1.8, 12).expect("inputs are valid"),
    );
    /// Streaming energy consumption at decoder (milliwatts).
    pub streaming_energy_decoder_mw: histogram_with_buckets(
        prometheus::exponential_buckets(10.0, 1.8, 12).expect("inputs are valid"),
    );
    /// Routed-trace audit outcomes grouped by trace identifier and status.
    pub nexus_audit_outcome_total: int_counter_vec(&["trace_id", "status"]);
    /// UNIX timestamp (seconds) of the most recent routed-trace audit outcome per trace.
    pub nexus_audit_outcome_last_timestamp: gauge_vec(&["trace_id"]);
    /// Space Directory manifest revisions observed per dataspace.
    pub nexus_space_directory_revision_total: int_counter_vec(&["dataspace", "dataspace_id"],);
    /// Active UAID capability manifests per dataspace/profile.
    pub nexus_space_directory_active_manifests: gauge_vec(
        &["dataspace", "dataspace_id", "profile"],
    );
    /// Capability manifest revocations grouped by dataspace and reason.
    pub nexus_space_directory_revocations_total: int_counter_vec(
        &["dataspace", "dataspace_id", "reason"],
    );
    /// Kaigi: relay registrations grouped by domain.
    pub kaigi_relay_registered_total: int_counter_vec(&["domain"]);
    /// Kaigi: bandwidth class distribution for relay registrations.
    pub kaigi_relay_registration_bandwidth: histogram_vec_with_buckets(
        prometheus::linear_buckets(1.0, 1.0, 8).expect("inputs are valid"),
                    &["domain"],
    );
    /// Kaigi: relay manifest updates grouped by domain and action.
    pub kaigi_relay_manifest_updates_total: int_counter_vec(&["domain", "action"]);
    /// Kaigi: relay manifest updates grouped only by domain for bounded diagnostics.
    pub kaigi_relay_manifest_updates_by_domain_total: raw(IntCounterVec);
    /// Kaigi: relay manifest hop-count distribution per domain.
    pub kaigi_relay_manifest_hop_count: histogram_vec_with_buckets(
        prometheus::linear_buckets(0.0, 1.0, 9).expect("inputs are valid"),
                    &["domain"],
    );
    /// Kaigi: relay failovers grouped by domain and call.
    pub kaigi_relay_failover_total: int_counter_vec(&["domain", "call"]);
    /// Kaigi: relay failovers grouped only by domain for bounded diagnostics.
    pub kaigi_relay_failovers_by_domain_total: raw(IntCounterVec);
    /// Kaigi: relay failover hop-count distribution per domain.
    pub kaigi_relay_failover_hop_count: histogram_vec_with_buckets(
        prometheus::linear_buckets(0.0, 1.0, 9).expect("inputs are valid"),
                    &["domain"],
    );
    /// Kaigi: relay health reports grouped by domain and status.
    pub kaigi_relay_health_reports_total: int_counter_vec(&["domain", "status"]);
    /// Kaigi: relay health reports grouped only by domain for bounded diagnostics.
    pub kaigi_relay_health_reports_by_domain_total: raw(IntCounterVec);
    /// Kaigi: current relay health state labelled by domain and relay.
    pub kaigi_relay_health_state: int_gauge_vec(&["domain", "relay"]);
    /// Number of sumeragi dropped messages
    pub dropped_messages: dropped_messages_counter();
    /// Number of dropped Sumeragi block messages due to full channel (consensus path)
    pub sumeragi_dropped_block_messages_total: int_counter();
    /// Number of dropped Sumeragi control messages due to full channel (control path)
    pub sumeragi_dropped_control_messages_total: int_counter();
    /// Sumeragi: votes accepted at proxy tail (cumulative)
    pub sumeragi_tail_votes_total: int_counter();
    /// Sumeragi: votes sent grouped by phase (prevote, precommit, available)
    pub sumeragi_votes_sent_total: int_counter_vec(&["phase"]);
    /// Sumeragi: votes received grouped by phase (prevote, precommit, available)
    pub sumeragi_votes_received_total: int_counter_vec(&["phase"]);
    /// Sumeragi: quorum certificates sent grouped by kind (prevote, precommit, available)
    pub sumeragi_qc_sent_total: int_counter_vec(&["kind"]);
    /// Sumeragi: quorum certificates received grouped by kind (prevote, precommit, available)
    pub sumeragi_qc_received_total: int_counter_vec(&["kind"]);
    /// Sumeragi: QC validation errors grouped by reason.
    pub sumeragi_qc_validation_errors_total: int_counter_vec(&["reason"]);
    /// Sumeragi: validation rejects before voting grouped by reason.
    pub sumeragi_validation_reject_total: int_counter_vec(&["reason"]);
    /// Sumeragi: validation gate last reject reason code (0=none, 1=stateless, 2=execution, 3=prev_hash, 4=prev_height, 5=topology).
    pub sumeragi_validation_reject_last_reason: gauge();
    /// Sumeragi: block height of the last validation gate reject (0 when unset).
    pub sumeragi_validation_reject_last_height: gauge();
    /// Sumeragi: view of the last validation gate reject (0 when unset).
    pub sumeragi_validation_reject_last_view: gauge();
    /// Sumeragi: unix timestamp (ms) of the last validation gate reject (0 when unset).
    pub sumeragi_validation_reject_last_timestamp_ms: gauge();
    /// Sumeragi: block-sync ShareBlocks dropped because no request was tracked.
    pub sumeragi_block_sync_share_blocks_unsolicited_total: int_counter();
    /// Sumeragi: consensus message drops/deferrals grouped by kind, outcome, and reason.
    pub sumeragi_consensus_message_handling_total: int_counter_vec(&["kind", "outcome", "reason"],);
    /// Sumeragi: commit-conflict detections (cumulative).
    pub sumeragi_commit_conflict_detected_total: int_counter();
    /// Sumeragi: view-change triggers grouped by cause.
    pub sumeragi_view_change_cause_total: int_counter_vec(&["cause"]);
    /// Sumeragi: unix timestamp (ms) of the last view-change trigger grouped by cause.
    pub sumeragi_view_change_cause_last_timestamp_ms: gauge_vec(&["cause"]);
    /// Sumeragi: QC signer tallies grouped by phase and whether the signer was counted for quorum.
    pub sumeragi_qc_signer_counts: histogram_vec_with_buckets(
        prometheus::linear_buckets(0.0, 1.0, 64).expect("valid signer buckets"),
                    &["phase", "kind"],
    );
    /// Sumeragi: invalid-signature drops grouped by message kind and throttle outcome.
    pub sumeragi_invalid_signature_total: int_counter_vec(&["kind", "outcome"]);
    /// Sumeragi: widen-before-rotate events (cumulative)
    pub sumeragi_widen_before_rotate_total: int_counter();
    /// Sumeragi: view-change suggestions emitted (cumulative)
    pub sumeragi_view_change_suggest_total: int_counter();
    /// Sumeragi: view-change installs observed (cumulative)
    pub sumeragi_view_change_install_total: int_counter();
    /// Sumeragi: view-change rotations after no proposal observed before cutoff (cumulative).
    pub sumeragi_proposal_gap_total: int_counter();
    /// Sumeragi: view-change proof counters grouped by outcome (accepted|stale|rejected)
    pub sumeragi_view_change_proof_total: gauge_vec(&["outcome"]);
    /// Sumeragi: Witness-availability QC assembled (cumulative)
    pub sumeragi_wa_qc_assembled_total: int_counter();
    /// Sumeragi: certificate size histogram (signatures per committed block)
    pub sumeragi_cert_size: histogram_with_buckets(
        prometheus::exponential_buckets(1.0, 1.8, 10).expect("valid"),
    );
    /// Sumeragi: signatures present on the block during commit validation (all roles).
    pub sumeragi_commit_signatures_present: gauge();
    /// Sumeragi: signatures counted toward the commit quorum (leader + validators in Set A/B).
    pub sumeragi_commit_signatures_counted: gauge();
    /// Sumeragi: Set B validator signatures present on the block during commit validation.
    pub sumeragi_commit_signatures_set_b: gauge();
    /// Sumeragi: required commit quorum size for the active topology.
    pub sumeragi_commit_signatures_required: gauge();
    /// Sumeragi: latest commit certificate height (best-effort).
    pub sumeragi_commit_qc_height: gauge();
    /// Sumeragi: latest commit certificate view (best-effort).
    pub sumeragi_commit_qc_view: gauge();
    /// Sumeragi: latest commit certificate epoch (best-effort).
    pub sumeragi_commit_qc_epoch: gauge();
    /// Sumeragi: signatures attached to the latest commit certificate.
    pub sumeragi_commit_qc_signatures_total: gauge();
    /// Sumeragi: validator-set size for the latest commit certificate.
    pub sumeragi_commit_qc_validator_set_len: gauge();
    /// Sumeragi: gossip fallback invocations (collectors exhausted).
    pub sumeragi_gossip_fallback_total: int_counter();
    /// Sumeragi: BlockCreated drops due to locked QC gate (sanity check failures).
    pub sumeragi_block_created_dropped_by_lock_total: int_counter();
    /// Sumeragi: BlockCreated rejects due to hint mismatch (height/view/parent).
    pub sumeragi_block_created_hint_mismatch_total: int_counter();
    /// Sumeragi: BlockCreated rejects due to proposal mismatch (header/payload).
    pub sumeragi_block_created_proposal_mismatch_total: int_counter();
    /// Nexus: lane relay envelopes rejected during validation (grouped by error kind).
    pub lane_relay_invalid_total: int_counter_vec(&["error"]);
    /// Nexus: emergency validator override usage for lane relay (grouped by outcome).
    pub lane_relay_emergency_override_total: int_counter_vec(&["lane", "dataspace", "outcome"],);
    /// Sumeragi: latest PRF epoch seed (hex) observed for collector selection.
    pub sumeragi_prf_epoch_seed_hex: raw(Arc<RwLock<Option<String>>>);
    /// Snapshot of Halo2 verifier configuration for status endpoints.
    pub halo2_status: raw(Arc<RwLock<Halo2Status>>);
    /// Sumeragi: height associated with the current PRF context.
    pub sumeragi_prf_height: gauge();
    /// Sumeragi: view associated with the current PRF context.
    pub sumeragi_prf_view: gauge();
    /// Sumeragi: deterministic membership view hash (truncated to u64).
    pub sumeragi_membership_view_hash: gauge();
    /// Sumeragi: height associated with the membership view hash snapshot.
    pub sumeragi_membership_height: gauge();
    /// Sumeragi: view associated with the membership view hash snapshot.
    pub sumeragi_membership_view: gauge();
    /// Sumeragi: epoch associated with the membership view hash snapshot.
    pub sumeragi_membership_epoch: gauge();
    /// Sumeragi: current runtime mode tag.
    pub sumeragi_mode_tag: raw(Arc<RwLock<String>>);
    /// Sumeragi: current leader index (gauge)
    pub sumeragi_leader_index: gauge();
    /// Sumeragi: highest QC height (gauge)
    pub sumeragi_highest_qc_height: gauge();
    /// Sumeragi: locked QC height (gauge)
    pub sumeragi_locked_qc_height: gauge();
    /// Sumeragi: locked QC view (gauge)
    pub sumeragi_locked_qc_view: gauge();
    /// Sumeragi: NEW_VIEW receipts per (height, view)
    pub sumeragi_new_view_receipts_by_hv: gauge_vec(&["height", "view"]);
    /// Sumeragi: NEW_VIEW messages published (cumulative)
    pub sumeragi_new_view_publish_total: int_counter();
    /// Sumeragi: NEW_VIEW messages received and accepted (cumulative)
    pub sumeragi_new_view_recv_total: int_counter();
    /// Sumeragi: NEW_VIEW messages dropped because HighestQC is behind the locked QC
    pub sumeragi_new_view_dropped_by_lock_total: int_counter();
    /// Sumeragi: missing-block fetch planning outcomes (labels: outcome=requested|backoff|no_targets)
    pub sumeragi_missing_block_fetch_total: int_counter_vec(&["outcome"]);
    /// Sumeragi: missing-block fetch target kind (labels: target=signers|topology)
    pub sumeragi_missing_block_fetch_target_total: int_counter_vec(&["target"]);
    /// Sumeragi: elapsed milliseconds from first-seen certificate to missing-block fetch request
    pub sumeragi_missing_block_fetch_dwell_ms: histogram_with_buckets(
        prometheus::exponential_buckets(10.0, 2.0, 8).expect("inputs are valid"),
    );
    /// Sumeragi: number of peers targeted when requesting a missing block payload
    pub sumeragi_missing_block_fetch_targets: histogram_with_buckets(
        prometheus::exponential_buckets(1.0, 2.0, 6).expect("inputs are valid"),
    );
    /// Block-sync QCs quarantined because local context was missing.
    pub blocksync_qc_quarantine_total: int_counter();
    /// Quarantined block-sync QCs that were revalidated successfully.
    pub blocksync_qc_revalidated_total: int_counter();
    /// Block-sync QCs dropped permanently after bounded revalidation.
    pub blocksync_qc_final_drop_total: int_counter_vec(&["reason"]);
    /// QCs deferred due to missing payload.
    pub qc_deferred_missing_payload_total: int_counter();
    /// Deferred QCs resolved after payload arrival.
    pub qc_deferred_resolved_total: int_counter();
    /// Deferred QCs expired after bounded retries.
    pub qc_deferred_expired_total: int_counter();
    /// Consensus deferrals caused by empty commit topology.
    pub consensus_empty_commit_topology_defer_total: int_counter();
    /// Empty-topology recoveries escalated to forced view changes.
    pub consensus_empty_commit_topology_escalation_total: int_counter();
    /// Recovery state-machine transitions labeled by state.
    pub consensus_recovery_state_transitions_total: int_counter_vec(&["state"]);
    /// Height-scoped missing-block recoveries escalated via deterministic hard cap.
    pub consensus_missing_block_height_escalation_total: int_counter();
    /// Sidecar mismatches quarantined in fail-closed mode.
    pub consensus_sidecar_quarantine_total: int_counter();
    /// Sidecar mismatches final-dropped after retry/TTL bounds.
    pub consensus_sidecar_final_drop_total: int_counter();
    /// Range-pull escalation attempts triggered by dependency recovery.
    pub blocksync_range_pull_escalation_total: int_counter();
    /// Successful range-pull recoveries.
    pub blocksync_range_pull_success_total: int_counter();
    /// Range-pull recoveries that expired without progress.
    pub blocksync_range_pull_failure_total: int_counter();
    /// Stuck-round duration observed while recovery waits for dependencies.
    pub consensus_recovery_stuck_round_seconds: histogram_with_buckets(
        prometheus::exponential_buckets(0.1, 2.0, 10).expect("inputs are valid"),
    );
    /// Sumeragi DA availability: missing availability artifacts (labeled by reason)
    pub sumeragi_da_gate_block_total: int_counter_vec(&["reason"]);
    /// Sumeragi DA availability: last recorded reason code (0=none,1=missing_local_data,3=manifest_missing,4=manifest_hash_mismatch,5=manifest_read_failed,6=manifest_spool_scan)
    pub sumeragi_da_gate_last_reason: gauge();
    /// Sumeragi DA availability: last satisfaction code (0=none,1=missing_data_recovered)
    pub sumeragi_da_gate_last_satisfied: gauge();
    /// Sumeragi DA availability: satisfaction transitions (labeled by gate)
    pub sumeragi_da_gate_satisfied_total: int_counter_vec(&["gate"]);
    /// Sumeragi DA manifest guard: outcomes labeled by result/reason.
    pub sumeragi_da_manifest_guard_total: int_counter_vec(&["result", "reason"]);
    /// Sumeragi DA manifest cache: outcomes labeled by result.
    pub sumeragi_da_manifest_cache_total: int_counter_vec(&["result"]);
    /// Sumeragi DA spool cache: outcomes labeled by kind/result.
    pub sumeragi_da_spool_cache_total: int_counter_vec(&["kind", "result"]);
    /// Sumeragi DA pin intent spool: outcomes labeled by result/reason.
    pub sumeragi_da_pin_intent_spool_total: int_counter_vec(&["result", "reason"]);
    /// Sumeragi availability: votes ingested by this collector (cumulative)
    pub sumeragi_da_votes_ingested_total: int_counter();
    /// Sumeragi QC assembly latency histogram (milliseconds) labeled by `kind`
    pub sumeragi_qc_assembly_latency_ms: histogram_vec_with_buckets(
        vec![
                        5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2000.0, 5000.0,
                    ],
                    &["kind"],
    );
    /// Sumeragi QC last observed latency gauge (milliseconds) labeled by `kind`
    pub sumeragi_qc_last_latency_ms: gauge_vec(&["kind"]);
    /// Sumeragi: kura persistence failures grouped by outcome (retry|abort)
    pub sumeragi_kura_store_failures_total: int_counter_vec(&["outcome"]);
    /// Sumeragi: last recorded kura persistence retry attempt (gauge)
    pub sumeragi_kura_store_last_retry_attempt: gauge();
    /// Sumeragi: last recorded kura persistence retry backoff in milliseconds (gauge)
    pub sumeragi_kura_store_last_retry_backoff_ms: gauge();
    /// Sumeragi pacemaker: proposals deferred due to transaction-queue back-pressure (cumulative)
    pub sumeragi_pacemaker_backpressure_deferrals_total: int_counter();
    /// Sumeragi pacemaker: backpressure deferrals grouped by reason (cumulative)
    pub sumeragi_pacemaker_backpressure_deferrals_by_reason_total: int_counter_vec(&["reason"],);
    /// Sumeragi pacemaker: backpressure deferral durations (ms) grouped by reason
    pub sumeragi_pacemaker_backpressure_deferral_duration_ms: histogram_vec_with_buckets(
        vec![
                            5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0,
                            20000.0,
                        ],
                        &["reason"],
    );
    /// Sumeragi pacemaker: backpressure deferral active state (0/1) grouped by reason
    pub sumeragi_pacemaker_backpressure_deferral_active: gauge_vec(&["reason"],);
    /// Sumeragi pacemaker: backpressure deferral age (ms) grouped by reason
    pub sumeragi_pacemaker_backpressure_deferral_age_ms: gauge_vec(&["reason"],);
    /// Sumeragi pacemaker: evaluation duration in the tick loop (ms)
    pub sumeragi_pacemaker_eval_ms: histogram_with_buckets(
        vec![1.0, 2.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0],
    );
    /// Sumeragi pacemaker: proposal attempt duration in the tick loop (ms)
    pub sumeragi_pacemaker_propose_ms: histogram_with_buckets(
        vec![1.0, 2.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0],
    );
    /// Sumeragi commit pipeline stage durations (ms) labeled by stage.
    pub sumeragi_commit_stage_ms: histogram_vec_with_buckets(
        vec![
                        1.0, 2.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2000.0, 5000.0,
                        10000.0, 20000.0,
                    ],
                    &["stage"],
    );
    /// State commit: state_write_lock wait duration (ms) during block commit.
    pub state_commit_write_lock_wait_ms: histogram_with_buckets(
        vec![
                        1.0, 2.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0,
                        10000.0,
                    ],
    );
    /// State commit: state_write_lock hold duration (ms) during block commit.
    pub state_commit_write_lock_hold_ms: histogram_with_buckets(
        vec![
                        1.0, 2.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0,
                        10000.0,
                    ],
    );
    /// Sumeragi pacemaker: commit pipeline executions triggered by timer tick (cumulative, labeled by mode/outcome)
    pub sumeragi_commit_pipeline_tick_total: int_counter_vec(&["mode", "outcome"]);
    /// Sumeragi pacemaker: prevote-quorum timeouts (cumulative, labeled by mode)
    pub sumeragi_prevote_timeout_total: int_counter_vec(&["mode"]);
    /// Sumeragi: membership mismatches detected (labeled by peer, height, view)
    pub sumeragi_membership_mismatch_total: int_counter_vec(&["peer", "height", "view"],);
    /// Sumeragi: peers currently flagged for membership mismatch (0/1 gauge)
    pub sumeragi_membership_mismatch_active: gauge_vec(&["peer"]);
    /// Sumeragi: post attempts to peers (cumulative), labeled by peer id
    pub sumeragi_post_to_peer_total: int_counter_vec(&["peer"]);
    /// Sumeragi: background-post enqueued tasks (cumulative), labeled by kind {Post,Broadcast}
    pub sumeragi_bg_post_enqueued_total: int_counter_vec(&["kind"]);
    /// Sumeragi: background-post queue full events (cumulative), labeled by kind
    pub sumeragi_bg_post_overflow_total: int_counter_vec(&["kind"]);
    /// Sumeragi: background-post drops when the worker queue is unavailable (cumulative), labeled by kind
    pub sumeragi_bg_post_drop_total: int_counter_vec(&["kind"]);
    /// Sumeragi: background-post queue depth (approximate, global)
    pub sumeragi_bg_post_queue_depth: gauge();
    /// Sumeragi: background-post queue depth by peer (collector), labeled by peer id
    pub sumeragi_bg_post_queue_depth_by_peer: gauge_vec(&["peer"]);
    /// Sumeragi: background-post age histogram (milliseconds) labeled by kind {Post,Broadcast}
    pub sumeragi_bg_post_age_ms: histogram_vec_with_buckets(
        prometheus::exponential_buckets(1.0, 2.0, 12).expect("inputs are valid"),
                    &["kind"],
    );
    /// Sumeragi: pacemaker current backoff window (ms)
    pub sumeragi_pacemaker_backoff_ms: gauge();
    /// Sumeragi: pacemaker RTT floor (ms)
    pub sumeragi_pacemaker_rtt_floor_ms: gauge();
    /// Sumeragi: pacemaker backoff multiplier (gauge)
    pub sumeragi_pacemaker_backoff_multiplier: gauge();
    /// Sumeragi: pacemaker RTT floor multiplier (gauge)
    pub sumeragi_pacemaker_rtt_floor_multiplier: gauge();
    /// Sumeragi: pacemaker maximum backoff cap (ms)
    pub sumeragi_pacemaker_max_backoff_ms: gauge();
    /// Sumeragi: pacemaker jitter band applied to window (ms, signed magnitude)
    pub sumeragi_pacemaker_jitter_ms: gauge();
    /// Sumeragi: pacemaker jitter config as permille of window (0..=1000)
    pub sumeragi_pacemaker_jitter_frac_permille: gauge();
    /// Sumeragi: elapsed time in the current round (ms)
    pub sumeragi_pacemaker_round_elapsed_ms: gauge();
    /// Sumeragi: current view timeout target window (ms)
    pub sumeragi_pacemaker_view_timeout_target_ms: gauge();
    /// Sumeragi: remaining time until current view timeout (ms)
    pub sumeragi_pacemaker_view_timeout_remaining_ms: gauge();
    /// Sumeragi: per-phase latency histogram (ms), labeled by `phase` (propose|collect|commit)
    pub sumeragi_phase_latency_ms: histogram_vec_with_buckets(
        vec![
                        5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2000.0, 5000.0,
                    ],
                    &["phase"],
    );
    /// Sumeragi: per-phase latency EMA (ms), labeled by `phase`
    pub sumeragi_phase_latency_ema_ms: gauge_vec(&["phase"]);
    /// Sumeragi: aggregate pipeline EMA latency (ms) across pacemaker-controlled phases.
    pub sumeragi_phase_total_ema_ms: gauge();
    /// Number of p2p dropped post messages (bounded mode)
    pub p2p_dropped_posts: gauge();
    /// Number of p2p dropped broadcast messages (bounded mode)
    pub p2p_dropped_broadcasts: gauge();
    /// Number of inbound messages dropped because subscriber queues were full.
    pub p2p_subscriber_queue_full_total: gauge();
    /// Per-topic inbound drops caused by subscriber queues being full.
    pub p2p_subscriber_queue_full_by_topic_total: gauge_vec(&["topic"]);
    /// Number of inbound messages dropped because no subscriber matches the topic.
    pub p2p_subscriber_unrouted_total: gauge();
    /// Per-topic inbound drops caused by no subscriber matches.
    pub p2p_subscriber_unrouted_by_topic_total: gauge_vec(&["topic"]);
    /// Number of p2p handshake failures
    pub p2p_handshake_failures: gauge();
    /// Number of low-priority post messages throttled
    pub p2p_low_post_throttled_total: gauge();
    /// Number of low-priority broadcast deliveries throttled
    pub p2p_low_broadcast_throttled_total: gauge();
    /// Number of per-peer post channel overflows (bounded per-topic channels)
    pub p2p_post_overflow_total: gauge();
    /// Per-topic breakdown for post channel overflows
    pub p2p_post_overflow_by_topic: gauge_vec(&["priority", "topic"]);
    /// Consensus ingress drops grouped by topic and reason.
    pub consensus_ingress_drop_total: int_counter_vec(&["topic", "reason"]);
    /// Number of DNS interval-based refresh cycles performed.
    pub p2p_dns_refresh_total: gauge();
    /// Number of DNS TTL-based refresh cycles performed.
    pub p2p_dns_ttl_refresh_total: gauge();
    /// Number of DNS resolution/connection failures for hostname peers.
    pub p2p_dns_resolution_fail_total: gauge();
    /// Number of DNS reconnect successes after refresh cycles.
    pub p2p_dns_reconnect_success_total: gauge();
    /// Number of scheduled per-address connect backoffs
    pub p2p_backoff_scheduled_total: gauge();
    /// Number of deferred outbound frames enqueued while peer sessions were unavailable.
    pub p2p_deferred_send_enqueued_total: gauge();
    /// Number of deferred outbound frames dropped (expiry, overflow, stale generation).
    pub p2p_deferred_send_dropped_total: gauge();
    /// Number of reconnect attempts triggered while deferring outbound frames.
    pub p2p_session_reconnect_total: gauge();
    /// Cumulative reconnect retry delay (seconds, rounded up from milliseconds).
    pub p2p_connect_retry_seconds: gauge();
    /// Number of incoming connections rejected by per-IP throttle
    pub p2p_accept_throttled_total: gauge();
    /// Number of accept throttle bucket evictions (idle/capacity).
    pub p2p_accept_bucket_evictions_total: gauge();
    /// Current number of active accept throttle buckets.
    pub p2p_accept_buckets_current: gauge();
    /// Prefix cache hits/misses for accept throttle (label `result`).
    pub p2p_accept_prefix_cache_total: gauge_vec(&["result"]);
    /// Accept throttle decisions (label `scope` = prefix|ip, `decision` = allowed|throttled).
    pub p2p_accept_throttle_decisions_total: gauge_vec(&["scope", "decision"],);
    /// Number of incoming connections rejected due to incoming cap
    pub p2p_incoming_cap_reject_total: gauge();
    /// Number of incoming connections rejected due to total cap
    pub p2p_total_cap_reject_total: gauge();
    /// Number of accepted unauthenticated transports rejected by the concurrent source cap.
    pub p2p_preauth_source_cap_reject_total: gauge();
    /// Trust score per peer (label `peer_id`).
    pub p2p_trust_score: int_gauge_vec(&["peer_id"]);
    /// Trust penalties applied (label `reason`).
    pub p2p_trust_penalties_total: int_counter_vec(&["reason"]);
    /// Trust decay ticks applied (label `peer_id`).
    pub p2p_trust_decay_ticks_total: int_counter_vec(&["peer_id"]);
    /// Trust gossip frames skipped grouped by direction and reason.
    pub p2p_trust_gossip_skipped_total: int_counter_vec(&["direction", "reason"]);
    /// Transaction gossip batches sent (labels: plane, dataspace).
    pub tx_gossip_sent_total: int_counter_vec(&["plane", "dataspace"]);
    /// Transaction gossip batches dropped (labels: plane, dataspace, reason).
    pub tx_gossip_dropped_total: int_counter_vec(&["plane", "dataspace", "reason"]);
    /// Latest transaction gossip target count (labels: plane, dataspace).
    pub tx_gossip_targets: gauge_vec(&["plane", "dataspace"]);
    /// Fallback attempts for restricted gossip (labels: plane, dataspace, surface).
    pub tx_gossip_fallback_total: int_counter_vec(&["plane", "dataspace", "surface"],);
    /// Configured frame cap for transaction gossip (bytes).
    pub tx_gossip_frame_cap_bytes: gauge();
    /// Configured cap for public gossip targets (0 = broadcast).
    pub tx_gossip_public_target_cap: gauge();
    /// Configured cap for restricted gossip targets (0 = commit topology).
    pub tx_gossip_restricted_target_cap: gauge();
    /// Public-plane target reshuffle interval in milliseconds.
    pub tx_gossip_public_target_reshuffle_ms: gauge();
    /// Restricted-plane target reshuffle interval in milliseconds.
    pub tx_gossip_restricted_target_reshuffle_ms: gauge();
    /// Whether unknown dataspaces are dropped (1) or routed via the restricted plane (0).
    pub tx_gossip_drop_unknown_dataspace: gauge();
    /// Restricted gossip fallback policy (0 = drop, 1 = public overlay).
    pub tx_gossip_restricted_fallback: gauge();
    /// Configured policy for restricted payloads when only the public overlay is available (0 = refuse, 1 = forward).
    pub tx_gossip_restricted_public_policy: gauge();
    /// Cached status snapshot for the latest gossip target selections.
    pub tx_gossip_status: raw(Arc<RwLock<Vec<TxGossipStatus>>>);
    /// Cached configured caps for status exports.
    pub tx_gossip_caps: raw(Arc<RwLock<TxGossipCaps>>);
    /// Accepted inbound SCION P2P connections
    pub p2p_scion_inbound_total: gauge();
    /// Successful outbound SCION P2P connections
    pub p2p_scion_outbound_total: gauge();
    /// Network message queue depth by priority (High/Low).
    pub p2p_queue_depth: gauge_vec(&["priority"]);
    /// Bounded network message queue drops split by priority and kind
    pub p2p_queue_dropped_total: gauge_vec(&["priority", "kind"]);
    /// Handshake latency histogram emulation (buckets by `le` in ms)
    pub p2p_handshake_ms_bucket: gauge_vec(&["le"]);
    /// Sum of observed handshake latencies in milliseconds
    pub p2p_handshake_ms_sum: gauge();
    /// Count of observed handshakes
    pub p2p_handshake_ms_count: gauge();
    /// Handshake error taxonomy
    pub p2p_handshake_error_total: gauge_vec(&["kind"]);
    /// Topic frame cap violations
    pub p2p_frame_cap_violations_total: gauge_vec(&["topic"]);
    /// Runtime: upgrade lifecycle events (labeled by kind: proposed|activated|canceled)
    pub runtime_upgrade_events_total: int_counter_vec(&["kind"]);
    /// Runtime: provenance rejection events (labeled by reason)
    pub runtime_upgrade_provenance_rejections_total: int_counter_vec(&["reason"]);
    /// Runtime: ABI version accepted by this node.
    pub runtime_abi_version: gauge();
    /// IVM opcode pre-decode cache hits (cumulative)
    pub ivm_cache_hits: gauge();
    /// IVM opcode pre-decode cache misses (cumulative)
    pub ivm_cache_misses: gauge();
    /// IVM opcode pre-decode cache evictions (cumulative)
    pub ivm_cache_evictions: gauge();
    /// IVM opcode pre-decode decoded streams (cumulative)
    pub ivm_cache_decoded_streams: gauge();
    /// IVM opcode pre-decode decoded operations (cumulative)
    pub ivm_cache_decoded_ops_total: gauge();
    /// IVM opcode pre-decode decode failures (cumulative)
    pub ivm_cache_decode_failures: gauge();
    /// IVM opcode pre-decode total decode time in nanoseconds (cumulative)
    pub ivm_cache_decode_time_ns_total: gauge();
    /// IVM: histogram of highest general-purpose register index touched per execution.
    pub ivm_register_max_index: histogram_with_buckets(
        vec![
                        16.0, 32.0, 48.0, 64.0, 96.0, 128.0, 160.0, 192.0, 224.0, 256.0, 320.0, 384.0,
                        448.0, 512.0,
                    ],
    );
    /// IVM: histogram of unique general-purpose registers touched per execution.
    pub ivm_register_unique_count: histogram_with_buckets(
        vec![
                        8.0, 16.0, 24.0, 32.0, 64.0, 96.0, 128.0, 160.0, 192.0, 224.0, 256.0, 320.0, 384.0,
                        448.0, 512.0,
                    ],
    );
    /// Merkle root computations using GPU acceleration (cumulative)
    pub merkle_root_gpu_total: int_counter();
    /// Merkle root computations using CPU (cumulative)
    pub merkle_root_cpu_total: int_counter();
    /// IVM memory commit duration (milliseconds), labelled by commit path.
    pub ivm_memory_commit_ms: histogram_vec_with_buckets(
        prometheus::exponential_buckets(0.1, 2.0, 16).expect("inputs are valid"),
                    &["path"],
    );
    /// IVM memory commit dirty chunk count, labelled by commit path.
    pub ivm_memory_commit_dirty_chunks: histogram_vec_with_buckets(
        prometheus::exponential_buckets(1.0, 2.0, 20).expect("inputs are valid"),
                    &["path"],
    );
    /// IVM Merkle cache full rebuilds.
    pub ivm_merkle_rebuild_total: int_counter();
    /// IVM Merkle cache incremental leaf updates.
    pub ivm_merkle_incremental_leaf_updates_total: int_counter();
    /// Number of DAG vertices (transactions) in the latest validated block
    pub pipeline_dag_vertices: gauge();
    /// Number of DAG edges (conflicts) in the latest validated block
    pub pipeline_dag_edges: gauge();
    /// Conflict rate of DAG edges in basis points for the latest validated block
    pub pipeline_conflict_rate_bps: gauge();
    /// Cumulative access-set source counts used by the scheduler (labels: source)
    pub pipeline_access_set_source_total: int_counter_vec(&["source"]);
    /// Number of independent components (DSF partitions) in the latest validated block
    pub pipeline_comp_count: gauge();
    /// Size of the largest independent component in the latest validated block
    pub pipeline_comp_max: gauge();
    /// Component-size histogram buckets labeled by `le` (component count per bucket)
    pub pipeline_comp_hist_bucket: gauge_vec(&["le"]);
    /// Peak layer width (max transactions in any layer) for the latest validated block
    pub pipeline_peak_layer_width: gauge();
    /// Average layer width (rounded) for the latest validated block
    pub pipeline_layer_avg_width: gauge();
    /// Median layer width for the latest validated block
    pub pipeline_layer_median_width: gauge();
    /// Nexus: cumulative count of config diffs applied per knob/profile.
    pub nexus_config_diff_total: int_counter_vec(&["knob", "profile"]);
    /// Number of Nexus lane catalog entries configured on this node.
    pub nexus_lane_configured_total: gauge();
    /// Nexus: per-lane governance seal status (1 = sealed, 0 = ready).
    pub nexus_lane_governance_sealed: gauge_vec(&["lane"]);
    /// Nexus: total number of lanes still sealed (missing manifest).
    pub nexus_lane_governance_sealed_total: gauge();
    /// Nexus: aliases of lanes still sealed (for status snapshots).
    pub nexus_lane_governance_sealed_aliases: raw(Arc<RwLock<Vec<String>>>);
    /// Nexus: lifecycle plan applications grouped by outcome.
    pub nexus_lane_lifecycle_applied_total: int_counter_vec(&["result"]);
    /// Nexus: latest block height observed per lane.
    pub nexus_lane_block_height: gauge_vec(&["lane", "dataspace"]);
    /// Nexus: finality lag in slots per lane (head height − lane height).
    pub nexus_lane_finality_lag_slots: gauge_vec(&["lane", "dataspace"]);
    /// Nexus: settlement backlog (XOR) per lane/dataspace pair.
    pub nexus_lane_settlement_backlog_xor: float_gauge_vec(&["lane", "dataspace"]);
    /// Nexus scheduler: configured TEU capacity for the current slot per lane.
    pub nexus_scheduler_lane_teu_capacity: gauge_vec(&["lane"]);
    /// Nexus scheduler: TEU committed in the current slot per lane.
    pub nexus_scheduler_lane_teu_slot_committed: gauge_vec(&["lane"]);
    /// Nexus scheduler: active circuit-breaker trigger level (0 = normal) per lane.
    pub nexus_scheduler_lane_trigger_level: gauge_vec(&["lane"]);
    /// Nexus scheduler: starvation bound in slots per lane.
    pub nexus_scheduler_starvation_bound_slots: gauge_vec(&["lane"]);
    /// Nexus scheduler: committed TEU bucket breakdown per lane (floor/headroom/etc.).
    pub nexus_scheduler_lane_teu_slot_breakdown: gauge_vec(&["lane", "bucket"],);
    /// Nexus scheduler: cumulative TEU deferrals by reason per lane.
    pub nexus_scheduler_lane_teu_deferral_total: int_counter_vec(&["lane", "reason"],);
    /// Nexus scheduler: structured headroom telemetry events per lane.
    pub nexus_scheduler_lane_headroom_events_total: int_counter_vec(&["lane"]);
    /// Nexus scheduler: cumulative must-serve truncations per lane.
    pub nexus_scheduler_must_serve_truncations_total: int_counter_vec(&["lane"]);
    /// Nexus scheduler: per-lane TEU snapshots exposed via `/status`.
    pub nexus_scheduler_lane_teu_status: raw(Arc<RwLock<BTreeMap<u32, NexusLaneTeuStatus>>>);
    /// Nexus scheduler: TEU backlog per dataspace (labeled by lane).
    pub nexus_scheduler_dataspace_teu_backlog: gauge_vec(&["lane", "dataspace"],);
    /// Nexus scheduler: dataspace age (slots since service) labeled by lane.
    pub nexus_scheduler_dataspace_age_slots: gauge_vec(&["lane", "dataspace"],);
    /// Nexus scheduler: dataspace SFQ virtual finish tag labeled by lane.
    pub nexus_scheduler_dataspace_virtual_finish: gauge_vec(&["lane", "dataspace"],);
    /// Nexus scheduler: per-dataspace TEU snapshots exposed via `/status`.
    pub nexus_scheduler_dataspace_teu_status: raw(Arc<RwLock<BTreeMap<(u32, u64), NexusDataspaceTeuStatus>>>);
    /// Nexus public-lane validator counts grouped by lifecycle status (pending, active, jailed, exiting, exited, slashed).
    pub nexus_public_lane_validator_total: int_gauge_vec(&["lane", "status"]);
    /// Nexus public-lane validator activations grouped by lane.
    pub nexus_public_lane_validator_activation_total: int_counter_vec(&["lane"]);
    /// Nexus public-lane validator registration rejects grouped by reason.
    pub nexus_public_lane_validator_reject_total: int_counter_vec(&["reason"]);
    /// Nexus public-lane bonded stake per lane (Quantity rendered as float).
    pub nexus_public_lane_stake_bonded: float_gauge_vec(&["lane"]);
    /// Nexus public-lane pending-unbond amount per lane.
    pub nexus_public_lane_unbond_pending: float_gauge_vec(&["lane"]);
    /// Nexus public-lane cumulative rewards recorded per lane.
    pub nexus_public_lane_reward_total: float_gauge_vec(&["lane"]);
    /// Nexus public-lane slash event counter per lane.
    pub nexus_public_lane_slash_total: int_counter_vec(&["lane"]);
    /// Number of scheduler layers in the latest validated block
    pub pipeline_layer_count: gauge();
    /// Average parallelism utilization in percent (0..100) for the latest validated block
    pub pipeline_scheduler_utilization_pct: gauge();
    /// Layer-width histogram buckets labeled by `le` (layer count per bucket)
    pub pipeline_layer_width_hist_bucket: gauge_vec(&["le"]);
    /// Number of per-transaction overlays built in the latest validated block
    pub pipeline_overlay_count: gauge();
    /// Total number of instructions across overlays in the latest validated block
    pub pipeline_overlay_instructions: gauge();
    /// Total Norito-encoded bytes across overlays in the latest validated block
    pub pipeline_overlay_bytes: gauge();
    /// Number of transactions classified into the quarantine lane in the latest validated block
    pub pipeline_quarantine_classified: gauge();
    /// Number of transactions rejected due to quarantine overflow in the latest validated block
    pub pipeline_quarantine_overflow: gauge();
    /// Number of transactions executed in the quarantine lane in the latest validated block
    pub pipeline_quarantine_executed: gauge();
    /// Detached pipeline: number of txs prepared for detached execution in latest validated block
    pub pipeline_detached_prepared: gauge();
    /// Detached pipeline: number of txs whose detached delta merged successfully
    pub pipeline_detached_merged: gauge();
    /// Detached pipeline: number of txs that fell back to direct apply
    pub pipeline_detached_fallback: gauge();
    /// Detached pipeline: fallback count by reason for the latest validated block
    pub pipeline_detached_fallback_reason: gauge_vec(&["reason"]);
    /// BLS signature micro-batches verified via aggregate (same-message) in latest block
    pub pipeline_sig_bls_agg_same: gauge();
    /// BLS signature micro-batches verified via aggregate (multi-message) in latest block
    pub pipeline_sig_bls_agg_multi: gauge();
    /// BLS signature micro-batches verified via deterministic per-signature path in latest block
    pub pipeline_sig_bls_deterministic: gauge();
    /// Cumulative same-message BLS aggregate verification attempts labeled by lane and result.
    pub pipeline_sig_bls_agg_same_total: int_counter_vec(&["lane", "result"]);
    /// Cumulative multi-message BLS aggregate verification attempts labeled by lane and result.
    pub pipeline_sig_bls_agg_multi_total: int_counter_vec(&["lane", "result"]);
    /// Pipeline stage durations (ms) labeled by stage name
    pub pipeline_stage_ms: histogram_vec_with_buckets(
        prometheus::exponential_buckets(1.0, 2.0, 12).expect("inputs are valid"),
                    &["lane", "stage"],
    );
    /// Total gas used by the latest validated block
    pub block_gas_used: gauge();
    /// Confidential gas charged to the latest transaction.
    pub confidential_gas_tx_used: gauge();
    /// Confidential gas charged in the current block.
    pub confidential_gas_block_used: gauge();
    /// Monotonic counter of confidential gas units consumed.
    pub confidential_gas_total: int_counter();
    /// Total fee units charged in the latest validated block
    pub block_fee_total_units: gauge();
    /// Scale associated with `block_fee_total_units`
    pub block_fee_total_scale: gauge();
    /// Merge ledger: total entries appended (cumulative)
    pub merge_ledger_entries_total: int_counter();
    /// Merge ledger: latest committed epoch id
    pub merge_ledger_latest_epoch: gauge();
    /// Merge ledger: latest global state root hex snapshot
    pub merge_ledger_latest_root_hex: raw(Arc<RwLock<Option<String>>>);
    /// Torii: filter expression depth by endpoint
    pub torii_filter_depth: histogram_vec_with_buckets(
        vec![1.0, 2.0, 3.0, 5.0, 8.0, 13.0],
                    &["endpoint"],
    );
    /// Torii: match count (items) by endpoint
    pub torii_filter_match_count: histogram_vec_with_buckets(
        prometheus::exponential_buckets(1.0, 2.0, 12).expect("inputs are valid"),
                    &["endpoint"],
    );
    /// Torii: scan latency (milliseconds) by endpoint
    pub torii_scan_ms: histogram_vec_with_buckets(
        prometheus::exponential_buckets(0.5, 2.0, 14).expect("inputs are valid"),
                    &["endpoint"],
    );
    /// Torii: stream row count (number of serialized items) by endpoint
    pub torii_stream_rows: histogram_vec_with_buckets(
        prometheus::exponential_buckets(1.0, 2.0, 16).expect("inputs are valid"),
                    &["endpoint"],
    );
    /// Torii: transaction admission latency (seconds) by lane and endpoint
    pub torii_lane_admission_latency_seconds: histogram_vec_with_buckets(
        prometheus::exponential_buckets(0.001, 2.0, 16).expect("inputs are valid"),
                    &["lane_id", "endpoint"],
    );
    /// Torii: route-stage latency (seconds) by route kind, stage, and outcome
    pub torii_route_stage_latency_seconds: histogram_vec_with_buckets(
        prometheus::exponential_buckets(0.000_001, 2.0, 20).expect("inputs are valid"),
                    &["route_kind", "stage", "outcome"],
    );
    /// Torii: attachment rejects grouped by reason.
    pub torii_attachment_reject_total: int_counter_vec(&["reason"]);
    /// Torii: attachment sanitization latency (milliseconds).
    pub torii_attachment_sanitize_ms: histogram_vec_with_buckets(
        prometheus::exponential_buckets(0.5, 2.0, 14).expect("inputs are valid"),
                    &[],
    );
    /// Torii: background prover attachment size distribution by status/content type
    pub torii_zk_prover_attachment_bytes: histogram_vec_with_buckets(
        prometheus::exponential_buckets(256.0, 2.0, 12).expect("inputs are valid"),
                    &["status", "content_type"],
    );
    /// Torii: background prover processing latency by status
    pub torii_zk_prover_latency_ms: histogram_vec_with_buckets(
        prometheus::exponential_buckets(5.0, 2.0, 12).expect("inputs are valid"),
                    &["status"],
    );
    /// Torii: background prover garbage-collection counter
    pub torii_zk_prover_gc_total: int_counter();
    /// Torii: background prover in-flight attachment gauge
    pub torii_zk_prover_inflight: gauge();
    /// Torii: background prover pending attachment gauge
    pub torii_zk_prover_pending: gauge();
    /// Torii: IVM prove helper in-flight job gauge
    pub torii_zk_ivm_prove_inflight: gauge();
    /// Torii: IVM prove helper queued job gauge
    pub torii_zk_ivm_prove_queued: gauge();
    /// Torii: background prover last-scan processed bytes gauge
    pub torii_zk_prover_last_scan_bytes: gauge();
    /// Torii: background prover last-scan wall-clock duration gauge
    pub torii_zk_prover_last_scan_ms: gauge();
    /// Torii: background prover budget exhaustion counter (labeled by reason)
    pub torii_zk_prover_budget_exhausted_total: int_counter_vec(&["reason"]);
    /// Torii: snapshot-lane query requests total, labeled by mode (ephemeral|stored)
    pub torii_query_snapshot_requests: int_counter_vec(&["mode"]);
    /// Torii: snapshot-lane first-batch latency (ms), labeled by mode
    pub torii_query_snapshot_first_batch_ms: histogram_vec_with_buckets(
        prometheus::exponential_buckets(0.5, 2.0, 14).expect("inputs are valid"),
                    &["mode"],
    );
    /// Torii: snapshot-lane gas consumed units total, labeled by mode
    pub torii_query_snapshot_gas_consumed_units_total: int_counter_vec(&["mode"]);
    /// Snapshot query lane: first-batch latency (ms) by cursor mode
    pub query_snapshot_lane_first_batch_ms: histogram_vec_with_buckets(
        prometheus::exponential_buckets(0.5, 2.0, 14).expect("inputs are valid"),
                    &["mode"],
    );
    /// Snapshot query lane: first-batch item counts by cursor mode
    pub query_snapshot_lane_first_batch_items: histogram_vec_with_buckets(
        vec![
                        1.0, 2.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1_000.0,
                    ],
                    &["mode"],
    );
    /// Snapshot query lane: remaining items gauge by cursor mode
    pub query_snapshot_lane_remaining_items: gauge_vec(&["mode"]);
    /// Snapshot query lane: cursors emitted total by cursor mode
    pub query_snapshot_lane_cursors_total: int_counter_vec(&["mode"]);
    // Torii Connect (Iroha Connect) metrics
    /// Torii Connect: total WS sessions (gauge)
    pub torii_connect_sessions_total: gauge();
    /// Torii Connect: active session objects (gauge)
    pub torii_connect_sessions_active: gauge();
    /// Torii pre-auth: rejected connections before authentication, labeled by reason
    pub torii_pre_auth_reject_total: int_counter_vec(&["reason"]);
    /// Torii operator auth events (action, result, reason).
    pub torii_operator_auth_total: int_counter_vec(&["action", "result", "reason"]);
    /// Torii operator auth lockouts (action, reason).
    pub torii_operator_auth_lockout_total: int_counter_vec(&["action", "reason"]);
    /// Torii admission rejects due to exceeding signature-count limits.
    pub torii_signature_limit_total: int_counter();
    /// Torii admission rejects due to signature-count limits, labeled by authority type.
    pub torii_signature_limit_by_authority_total: int_counter_vec(&["authority"]);
    /// Last observed signature count when enforcing Torii signature limits.
    pub torii_signature_limit_last_count: gauge();
    /// Configured signature cap recorded during the last signature-limit enforcement.
    pub torii_signature_limit_max: gauge();
    /// Torii admission rejects when NTS is unhealthy for time-sensitive transactions.
    pub torii_nts_unhealthy_reject_total: int_counter();
    /// Torii admission rejects for direct multisig signing attempts.
    pub torii_multisig_direct_sign_reject_total: int_counter();
    /// Torii SoraFS provider admission counters (result, reason).
    pub torii_sorafs_admission_total: int_counter_vec(&["result", "reason"]);
    /// Torii SoraFS capacity telemetry rejections (provider, reason).
    pub torii_sorafs_capacity_telemetry_rejections_total: int_counter_vec(&["provider", "reason"],);
    /// Torii SoraFS declared capacity gauge (GiB) per provider.
    pub torii_sorafs_capacity_declared_gib: gauge_vec(&["provider"]);
    /// Torii SoraFS effective capacity gauge (GiB) per provider.
    pub torii_sorafs_capacity_effective_gib: gauge_vec(&["provider"]);
    /// Torii SoraFS utilised capacity gauge (GiB) per provider.
    pub torii_sorafs_capacity_utilised_gib: gauge_vec(&["provider"]);
    /// Torii SoraFS outstanding capacity gauge (GiB) per provider.
    pub torii_sorafs_capacity_outstanding_gib: gauge_vec(&["provider"]);
    /// Torii SoraFS accumulated GiB·hours per provider.
    pub torii_sorafs_capacity_gibhours_total: float_gauge_vec(&["provider"]);
    /// Torii SoraFS egress byte counters per provider and source.
    pub torii_sorafs_egress_bytes: float_gauge_vec(&["provider", "source"]);
    /// Torii SoraFS egress counter drift ratio per provider and source.
    pub torii_sorafs_egress_drift_ratio: float_gauge_vec(&["provider", "source"]);
    /// SoraFS Governance DAG publication attempts grouped by payload kind, result, and sink.
    pub sorafs_governance_dag_publish_total: int_counter_vec(&["payload_kind", "result", "sink"],);
    /// SoraFS Governance DAG published bytes grouped by payload kind and sink.
    pub sorafs_governance_dag_published_bytes_total: int_counter_vec(&["payload_kind", "sink"],);
    /// SoraFS Governance DAG last successful publish timestamp grouped by payload kind and sink.
    pub sorafs_governance_dag_last_publish_timestamp_seconds: gauge_vec(&["payload_kind", "sink"],);
    /// SoraFS Governance DAG publish backlog grouped by sink.
    pub sorafs_governance_dag_backlog: gauge_vec(&["sink"]);
    /// SoraFS Governance DAG head age in seconds grouped by sink.
    pub sorafs_governance_dag_head_age_seconds: gauge_vec(&["sink"]);
    /// Committed SoraFS orderbook transitions grouped by a closed event-kind vocabulary.
    pub torii_sorafs_orderbook_finalized_events_total: int_counter_vec(&["event"]);
    /// Authoritative open order depth in GiB grouped by the closed tier/side vocabulary.
    pub torii_sorafs_orderbook_open_depth_gib: gauge_vec(&["tier", "side"]);
    /// Lag between the latest book mutation and its exhaustive bounded matcher scan.
    pub torii_sorafs_orderbook_matcher_lag_seconds: gauge();
    /// Authoritative count of open settlement channels.
    pub torii_sorafs_orderbook_settlement_backlog: gauge();
    /// Age of the oldest authoritative open settlement channel.
    pub torii_sorafs_orderbook_oldest_settlement_age_seconds: gauge();
    /// Time until the earliest authoritative open settlement channel expires.
    pub torii_sorafs_orderbook_escrow_runway_seconds: gauge();
    /// Whether a complete immutable finalized orderbook projection was published.
    pub torii_sorafs_orderbook_finalized_projection_ready: gauge();
    /// Finalized block height of the last complete orderbook projection.
    pub torii_sorafs_orderbook_finalized_projection_height: gauge();
    /// Finalized block timestamp of the last complete orderbook projection.
    pub torii_sorafs_orderbook_finalized_projection_timestamp_seconds: gauge();
    /// Fail-closed finalized orderbook projection failures by a closed reason vocabulary.
    pub torii_sorafs_orderbook_finalized_projection_failures_total: int_counter_vec(&["reason"],);
    /// Authoritative orderbook revision in the last complete projection.
    pub torii_sorafs_orderbook_book_revision: gauge();
    /// Latest exhaustively scanned authoritative orderbook revision.
    pub torii_sorafs_orderbook_matcher_scan_book_revision: gauge();
    /// Orderbook API requests grouped by a closed route/outcome vocabulary.
    pub torii_sorafs_orderbook_api_requests_total: int_counter_vec(&["route", "outcome"],);
    /// Gateway compliance control requests grouped by closed operation/outcome vocabularies.
    pub torii_sorafs_gateway_compliance_requests_total: int_counter_vec(&["operation", "outcome"],);
    /// Gateway compliance serving decisions grouped only by bounded policy dimensions.
    pub torii_sorafs_gateway_compliance_serving_decisions_total: int_counter_vec(
        &["subject_kind", "disposition", "source"],
    );
    /// Gateway compliance failures grouped by closed surface/class vocabularies.
    pub torii_sorafs_gateway_compliance_failures_total: int_counter_vec(&["surface", "class"],);
    /// Sequence of the catalog currently used by the serving path.
    pub torii_sorafs_gateway_compliance_serving_catalog_sequence: gauge();
    /// Expiry Unix second of the catalog currently used by the serving path.
    pub torii_sorafs_gateway_compliance_serving_catalog_valid_until_seconds: gauge();
    /// Whether the gateway has a fresh, verified catalog available to the serving path.
    pub torii_sorafs_gateway_compliance_ready: gauge();
    /// Torii SoraFS hedging XOR/USD reference price in micro-USD by cluster.
    pub torii_sorafs_hedging_xor_usd_reference_price_micro_usd: gauge_vec(&["cluster"],);
    /// Torii SoraFS hedging feed lag in seconds by cluster and source.
    pub torii_sorafs_hedging_feed_lag_seconds: gauge_vec(&["cluster", "source"],);
    /// Torii SoraFS hedging feed divergence in basis points by cluster and source.
    pub torii_sorafs_hedging_feed_divergence_bps: gauge_vec(&["cluster", "source"],);
    /// Torii SoraFS hedging exposure drift in basis points by cluster and asset.
    pub torii_sorafs_hedging_exposure_drift_bps: gauge_vec(&["cluster", "asset"],);
    /// Torii SoraFS billing statement generation counters by cluster and account type.
    pub torii_sorafs_billing_statement_generation_total: int_counter_vec(
        &["cluster", "account_type"],
    );
    /// Torii SoraFS billing statement failure counters by cluster and account type.
    pub torii_sorafs_billing_statement_failure_total: int_counter_vec(
        &["cluster", "account_type"],
    );
    /// Torii SoraFS billing statement acknowledgement backlog by cluster.
    pub torii_sorafs_billing_statement_ack_backlog: gauge_vec(&["cluster"]);
    /// Torii SoraFS billing escrow runway in seconds by cluster and account type.
    pub torii_sorafs_billing_escrow_runway_seconds: gauge_vec(&["cluster", "account_type"],);
    /// Torii SoraFS reserve providers grouped by lifecycle stage.
    pub torii_sorafs_reserve_lifecycle_stage_providers: gauge_vec(&["stage"]);
    /// Torii SoraFS outstanding reserve credit principal in micro-XOR by lifecycle stage.
    pub torii_sorafs_reserve_credit_draw_micro_xor: float_gauge_vec(&["stage"]);
    /// Torii SoraFS reserve credit shortfall in micro-XOR by lifecycle stage.
    pub torii_sorafs_reserve_credit_shortfall_micro_xor: float_gauge_vec(&["stage"],);
    /// Torii SoraFS reserve accrued interest in micro-XOR by lifecycle stage.
    pub torii_sorafs_reserve_accrued_interest_micro_xor: float_gauge_vec(&["stage"],);
    /// Torii SoraFS providers currently in default.
    pub torii_sorafs_reserve_defaulted_providers: gauge();
    /// Torii SoraFS open reserve appeals awaiting decision.
    pub torii_sorafs_reserve_appeal_backlog: gauge();
    /// Torii SoraFS reserve movements grouped by custody status.
    pub torii_sorafs_reserve_custody_movements: gauge_vec(&["status"]);
    /// Torii SoraFS reserve movements reconciled with terminal chain custody evidence.
    pub torii_sorafs_reserve_chain_reconciled_movements: gauge_vec(&["status"],);
    /// Whether the reserve telemetry projection has caught up and reconciled at one finalized view.
    pub torii_sorafs_reserve_finalized_projection_ready: gauge();
    /// Finalized block height represented by the latest complete reserve telemetry projection.
    pub torii_sorafs_reserve_finalized_projection_height: gauge();
    /// Failed reserve finalized-projection refresh attempts.
    pub torii_sorafs_reserve_finalized_projection_failure_total: int_counter();
    /// Torii SoraFS reserve service requests grouped by route and result.
    pub torii_sorafs_reserve_service_requests_total: int_counter_vec(&["route", "result"],);
    /// Torii SoraFS reserve service rate-limit events grouped by route and reason.
    pub torii_sorafs_reserve_service_rate_limit_total: int_counter_vec(&["route", "reason"],);
    /// SoraFS reputation ingest lag observed when a snapshot is published.
    pub sorafs_reputation_ingest_lag_seconds: gauge();
    /// SoraFS reputation snapshot age observed when a snapshot is published.
    pub sorafs_reputation_snapshot_age_seconds: gauge();
    /// SoraFS reputation snapshot generation time as a Unix timestamp.
    pub sorafs_reputation_snapshot_generated_at_unix: gauge();
    /// SoraFS reputation provider count in the latest accepted snapshot.
    pub sorafs_reputation_provider_count: gauge();
    /// SoraFS reputation providers currently below the low-score threshold.
    pub sorafs_reputation_low_score_providers: gauge();
    /// SoraFS reputation provider scores, bounded to the top-N providers.
    pub sorafs_reputation_score: float_gauge_vec(&["provider_id"]);
    /// SoraFS reputation threshold crossings by level.
    pub sorafs_reputation_threshold_crossings_total: int_counter_vec(&["level"]);
    /// Whether the committed finalized-ledger reputation runtime has completed a successful poll.
    pub sorafs_reputation_runtime_live: gauge();
    /// Whether every required committed reputation runtime path is ready.
    pub sorafs_reputation_runtime_ready: gauge();
    /// Whether all identity-pinned external reputation adapters are healthy.
    pub sorafs_reputation_runtime_dependencies_ready: gauge();
    /// Whether the still-required native journal transaction submitter is healthy.
    pub sorafs_reputation_journal_transaction_submitter_ready: gauge();
    /// Latest finalized block height represented by the committed projector.
    pub sorafs_reputation_runtime_finalized_height: gauge();
    /// Consecutive failed exact-anchor reconciliation attempts.
    pub sorafs_reputation_runtime_consecutive_failures: gauge();
    /// Whether the exact signed result is durably acknowledged.
    pub sorafs_reputation_runtime_material_acknowledged: gauge();
    /// Provider accumulators retained by the committed projector.
    pub sorafs_reputation_runtime_provider_count: gauge();
    /// Supervised committed-runtime reconciliation ticks.
    pub sorafs_reputation_runtime_ticks_total: int_counter_vec(&["result"]);
    /// Whether the committed hedging/billing runtime completed a successful tick.
    pub sorafs_hedging_billing_runtime_live: gauge();
    /// Whether the committed hedging/billing runtime is release-ready.
    pub sorafs_hedging_billing_runtime_ready: gauge();
    /// Whether all identity-pinned hedging/billing adapters are healthy.
    pub sorafs_hedging_billing_runtime_dependencies_ready: gauge();
    /// Whether automatic hedge execution is enabled (always zero in V1).
    pub sorafs_hedging_billing_automatic_execution_enabled: gauge();
    /// Whether the most recent successful hedging/billing runtime tick is fresh.
    pub sorafs_hedging_billing_last_tick_fresh: gauge();
    /// Whether the billing projection is anchored to an admissibly recent finalized head.
    pub sorafs_hedging_billing_finalized_projection_ready: gauge();
    /// Latest finalized height projected into billing state.
    pub sorafs_hedging_billing_finalized_height: gauge();
    /// Latest finalized ledger head observed by the hedging/billing runtime.
    pub sorafs_hedging_billing_finalized_head_height: gauge();
    /// Finalized blocks between the observed ledger head and billing projection.
    pub sorafs_hedging_billing_finalized_lag_blocks: gauge();
    /// First finalized billing journal sequence not yet projected.
    pub sorafs_hedging_billing_next_event_sequence: gauge();
    /// Statements waiting for external software signing.
    pub sorafs_hedging_billing_ready_for_signing: gauge();
    /// Signed statements waiting for immutable publication.
    pub sorafs_hedging_billing_ready_for_publication: gauge();
    /// Ambiguous statement publications awaiting authoritative lookup.
    pub sorafs_hedging_billing_publication_ambiguous: gauge();
    /// Published statements waiting for acknowledgement.
    pub sorafs_hedging_billing_published: gauge();
    /// Durably acknowledged statements.
    pub sorafs_hedging_billing_acknowledged: gauge();
    /// Terminal billing statement delivery dead letters.
    pub sorafs_hedging_billing_dead_letter: gauge();
    /// Generated, never-automatically-executed hedge intents.
    pub sorafs_hedging_billing_hedge_intents: gauge();
    /// Supervised hedging/billing reconciliation ticks.
    pub sorafs_hedging_billing_runtime_ticks_total: int_counter_vec(&["result"]);
    /// SoraFS reputation provider labels currently exported by the score gauge.
    pub sorafs_reputation_score_tracked_providers: raw(Arc<RwLock<BTreeSet<String>>>);
    /// SoraFS reputation low-score state from the previous accepted snapshot.
    pub sorafs_reputation_low_score_state: raw(Arc<RwLock<BTreeMap<String, bool>>>);
    /// Torii SoraFS fee projection (nano units) per provider.
    pub torii_sorafs_fee_projection_nanos: float_gauge_vec(&["provider"]);
    /// Torii SoraFS dispute submissions labelled by result.
    pub torii_sorafs_disputes_total: int_counter_vec(&["result"]);
    /// Torii SoraFS replication orders issued per provider.
    pub torii_sorafs_orders_issued_total: gauge_vec(&["provider"]);
    /// Torii SoraFS replication orders completed per provider.
    pub torii_sorafs_orders_completed_total: gauge_vec(&["provider"]);
    /// Torii SoraFS replication orders failed per provider.
    pub torii_sorafs_orders_failed_total: gauge_vec(&["provider"]);
    /// Torii SoraFS outstanding order count per provider.
    pub torii_sorafs_outstanding_orders: gauge_vec(&["provider"]);
    /// Torii SoraFS uptime success (basis points) per provider.
    pub torii_sorafs_uptime_bps: int_gauge_vec(&["provider"]);
    /// Torii SoraFS PoR success (basis points) per provider.
    pub torii_sorafs_por_bps: int_gauge_vec(&["provider"]);
    /// Torii SoraFS PoR scheduler challenges grouped by result.
    pub torii_sorafs_por_challenges_total: int_counter_vec(&["result"]);
    /// Torii SoraFS PoR forced challenges emitted by the scheduler.
    pub torii_sorafs_por_forced_challenges_total: int_counter();
    /// Torii SoraFS PoR duplicate samples observed while scheduling challenges.
    pub torii_sorafs_por_sampling_duplicates_total: int_counter();
    /// Torii SoraFS PoR ingestion backlog per manifest/provider pair.
    pub torii_sorafs_por_ingest_backlog: gauge_vec(&["manifest", "provider"]);
    /// Torii SoraFS PoR ingestion failures per manifest/provider pair.
    pub torii_sorafs_por_ingest_failures_total: gauge_vec(&["manifest", "provider"],);
    /// Torii SoraFS repair task transitions by status.
    pub torii_sorafs_repair_tasks_total: int_counter_vec(&["status"]);
    /// Torii SoraFS repair latency histogram (minutes) grouped by outcome.
    pub torii_sorafs_repair_latency_minutes: histogram_vec_with_buckets(
        vec![1.0, 2.0, 5.0, 10.0, 15.0, 30.0, 60.0, 120.0, 240.0, 480.0],
                    &["outcome"],
    );
    /// Torii SoraFS repair queue depth per provider.
    pub torii_sorafs_repair_queue_depth: gauge_vec(&["provider"]);
    /// Torii SoraFS oldest queued repair age (seconds).
    pub torii_sorafs_repair_backlog_oldest_age_seconds: gauge();
    /// Torii SoraFS repair lease expirations grouped by outcome.
    pub torii_sorafs_repair_lease_expired_total: int_counter_vec(&["outcome"]);
    /// Torii SoraFS slash proposals submitted grouped by outcome.
    pub torii_sorafs_slash_proposals_total: int_counter_vec(&["outcome"]);
    /// Torii SoraFS reconciliation runs grouped by result.
    pub torii_sorafs_reconciliation_runs_total: int_counter_vec(&["result"]);
    /// Torii SoraFS reconciliation divergence count from the latest snapshot.
    pub torii_sorafs_reconciliation_divergence_count: gauge();
    /// Torii SoraFS GC runs grouped by result.
    pub torii_sorafs_gc_runs_total: int_counter_vec(&["result"]);
    /// Torii SoraFS GC evictions grouped by reason.
    pub torii_sorafs_gc_evictions_total: int_counter_vec(&["reason"]);
    /// Torii SoraFS GC bytes freed grouped by reason.
    pub torii_sorafs_gc_bytes_freed_total: int_counter_vec(&["reason"]);
    /// Torii SoraFS GC blocked evictions grouped by reason.
    pub torii_sorafs_gc_blocked_total: int_counter_vec(&["reason"]);
    /// Torii SoraFS expired manifests observed by GC sweeps.
    pub torii_sorafs_gc_expired_manifests: gauge();
    /// Torii SoraFS age of the oldest expired manifest (seconds).
    pub torii_sorafs_gc_oldest_expired_age_seconds: gauge();
    /// Torii SoraFS storage bytes used per provider.
    pub torii_sorafs_storage_bytes_used: gauge_vec(&["provider"]);
    /// Torii SoraFS storage capacity bytes per provider.
    pub torii_sorafs_storage_bytes_capacity: gauge_vec(&["provider"]);
    /// Finalized-ledger SoraFS provider ingests in flight per provider.
    pub sorafs_provider_ingest_inflight: gauge_vec(&["provider"]);
    /// Torii SoraFS fetch workers in flight per provider.
    pub torii_sorafs_storage_fetch_inflight: gauge_vec(&["provider"]);
    /// Torii SoraFS fetch throughput (bytes/sec) per provider.
    pub torii_sorafs_storage_fetch_bytes_per_sec: gauge_vec(&["provider"]);
    /// Torii SoraFS PoR workers in flight per provider.
    pub torii_sorafs_storage_por_inflight: gauge_vec(&["provider"]);
    /// Torii SoraFS PoR samples marked successful per provider.
    pub torii_sorafs_storage_por_samples_success_total: gauge_vec(&["provider"],);
    /// Torii SoraFS PoR samples marked failed per provider.
    pub torii_sorafs_storage_por_samples_failed_total: gauge_vec(&["provider"],);
    /// Active SoraFS gateway requests grouped by the canonical request dimensions.
    pub sorafs_gateway_active: int_gauge_vec(
        &["endpoint", "method", "variant", "chunker", "profile"],
    );
    /// Completed SoraFS gateway responses grouped by bounded outcome dimensions.
    pub sorafs_gateway_responses_total: int_counter_vec(
        &[
                        "endpoint",
                        "method",
                        "variant",
                        "chunker",
                        "profile",
                        "result",
                        "status",
                        "error_code",
                    ],
    );
    /// SoraFS gateway time-to-first-byte histogram in milliseconds.
    pub sorafs_gateway_ttfb_ms: histogram_vec_with_buckets(
        vec![
                        5.0, 10.0, 25.0, 50.0, 100.0, 120.0, 200.0, 500.0, 1000.0, 2500.0, 5000.0,
                    ],
                    &[
                        "endpoint",
                        "method",
                        "variant",
                        "chunker",
                        "profile",
                        "result",
                        "status",
                        "error_code",
                    ],
    );
    /// SoraFS proof verification outcomes grouped by profile and bounded error code.
    pub sorafs_gateway_proof_verifications_total: int_counter_vec(
        &["profile_version", "result", "error_code"],
    );
    /// SoraFS proof verification duration histogram in milliseconds.
    pub sorafs_gateway_proof_duration_ms: histogram_vec_with_buckets(
        vec![
                        5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0,
                    ],
                    &["profile_version", "result", "error_code"],
    );
    /// Torii SoraFS chunk-range request counters (endpoint, result).
    pub torii_sorafs_chunk_range_requests_total: int_counter_vec(&["endpoint", "status"],);
    /// Torii SoraFS chunk-range bytes served per endpoint.
    pub torii_sorafs_chunk_range_bytes_total: int_counter_vec(&["endpoint"]);
    /// Count of providers advertising SoraFS range fetch capability grouped by feature.
    pub torii_sorafs_provider_range_capability_total: int_gauge_vec(&["feature"]);
    /// SoraFS committed routing-authority cache events grouped by bounded outcome.
    pub torii_sorafs_routing_authority_cache_total: int_counter_vec(&["outcome"]);
    /// SoraFS range fetch throttle events grouped by reason.
    pub torii_sorafs_range_fetch_throttle_events_total: int_counter_vec(&["reason"],);
    /// Active SoraFS range fetch streams guarded by tokens (node-wide).
    pub torii_sorafs_range_fetch_concurrency_current: int_gauge();
    /// Torii SoraFS proof streams currently active (per proof kind).
    pub torii_sorafs_proof_stream_inflight: int_gauge_vec(&["kind"]);
    /// Torii SoraFS proof stream outcomes grouped by result and reason.
    pub torii_sorafs_proof_stream_events_total: int_counter_vec(&["kind", "result", "reason"],);
    /// Torii SoraFS proof stream latency histogram in milliseconds.
    pub torii_sorafs_proof_stream_latency_ms: histogram_vec_with_buckets(
        vec![
                        5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0,
                    ],
                    &["kind"],
    );
    /// Torii SoraFS proof-health alerts grouped by provider, trigger, and penalty outcome.
    pub torii_sorafs_proof_health_alerts_total: int_counter_vec(
        &["provider_id", "trigger", "penalty"],
    );
    /// Torii SoraFS proof-health PDP failure counts captured at the last alert per provider.
    pub torii_sorafs_proof_health_pdp_failures: int_gauge_vec(&["provider_id"]);
    /// Torii SoraFS proof-health PoTR breach counts captured at the last alert per provider.
    pub torii_sorafs_proof_health_potr_breaches: int_gauge_vec(&["provider_id"]);
    /// Torii SoraFS proof-health penalty amount (nano-XOR) observed at the last alert per provider.
    pub torii_sorafs_proof_health_penalty_nano: gauge_vec(&["provider_id"]);
    /// Torii SoraFS proof-health telemetry window end epoch recorded at the last alert per provider.
    pub torii_sorafs_proof_health_window_end_epoch: gauge_vec(&["provider_id"],);
    /// Torii SoraFS proof-health cooldown flag recorded at the last alert per provider.
    pub torii_sorafs_proof_health_cooldown: int_gauge_vec(&["provider_id"]);
    /// GAR policy violations grouped by reason/detail.
    pub torii_sorafs_gar_violations_total: int_counter_vec(&["reason", "detail"]);
    /// Gateway refusal counters grouped by reason/profile/provider/scope.
    pub torii_sorafs_gateway_refusals_total: int_counter_vec(
        &["reason", "profile", "provider_id", "scope"],
    );
    /// Canonical SoraFS gateway fixture metadata (value = release timestamp, labels = version/profile/digest).
    pub torii_sorafs_gateway_fixture_info: int_gauge_vec(
        &["version", "profile", "fixtures_digest"],
    );
    /// SoraFS pin registry manifest counts grouped by status.
    pub torii_sorafs_registry_manifests_total: gauge_vec(&["status"]);
    /// SoraFS manifest alias total (active entries tracked on-chain).
    pub torii_sorafs_registry_aliases_total: gauge();
    /// Consensus-maintained count of retained SoraFS pin lifecycle records.
    pub torii_sorafs_pin_retained_manifests: gauge();
    /// Consensus-maintained aggregate bytes represented by live SoraFS pins.
    pub torii_sorafs_pin_live_content_bytes: gauge();
    /// Alias cache evaluation outcomes (fresh/refresh/expired/hard-expired).
    pub torii_sorafs_alias_cache_refresh_total: int_counter_vec(&["result", "reason"],);
    /// Observed alias proof age when served (seconds).
    pub torii_sorafs_alias_cache_age_seconds: histogram_with_buckets(
        vec![
                        30.0, 60.0, 120.0, 300.0, 600.0, 900.0, 1_200.0, 1_800.0, 3_600.0, 7_200.0,
                    ],
    );
    /// Seconds remaining until the active gateway TLS certificate expires.
    pub torii_sorafs_tls_cert_expiry_seconds: float_gauge();
    /// Gateway TLS renewal attempts grouped by result.
    pub torii_sorafs_tls_renewal_total: int_counter_vec(&["result"]);
    /// Whether ECH is currently enabled for the gateway (0 = disabled, 1 = enabled).
    pub torii_sorafs_tls_ech_enabled: int_gauge();
    /// Gauge exposing the canonical SoraFS gateway fixture version (label = version).
    pub torii_sorafs_gateway_fixture_version: int_gauge_vec(&["version"]);
    /// SoraFS replication order counts grouped by status.
    pub torii_sorafs_registry_orders_total: gauge_vec(&["status"]);
    /// SoraFS replication SLA outcomes (met, missed, pending).
    pub torii_sorafs_replication_sla_total: gauge_vec(&["outcome"]);
    /// Outstanding SoraFS replication backlog (pending order count).
    pub torii_sorafs_replication_backlog_total: gauge();
    /// Completion latency aggregates for SoraFS replication orders (epochs).
    pub torii_sorafs_replication_completion_latency_epochs: float_gauge_vec(&["stat"],);
    /// Deadline slack aggregates for pending SoraFS replication orders (epochs).
    pub torii_sorafs_replication_deadline_slack_epochs: float_gauge_vec(&["stat"]);
    /// Rejections at the SoraNet privacy ingest endpoints grouped by endpoint/reason.
    pub soranet_privacy_ingest_reject_total: int_counter_vec(&["endpoint", "reason"],);
    /// Aggregated SoraNet circuit outcomes keyed by relay mode and fixed event kind.
    pub soranet_privacy_circuit_events_total: int_counter_vec(&["mode", "kind"],);
    /// PoW validation failures grouped by relay mode and fixed reason.
    pub soranet_privacy_pow_rejects_total: int_counter_vec(&["mode", "reason"],);
    /// Count of SoraNet PoW revocation-store failures grouped by reason.
    pub soranet_pow_revocation_store_total: int_counter_vec(&["reason"]);
    /// Aggregated SoraNet throttling events keyed by relay mode and fixed scope.
    pub soranet_privacy_throttles_total: int_counter_vec(&["mode", "scope"],);
    /// Aggregated verified byte totals emitted per relay mode.
    pub soranet_privacy_verified_bytes_total: int_counter_vec(&["mode"],);
    /// Average active circuits in the latest emitted bucket for each mode.
    pub soranet_privacy_active_circuits_avg: float_gauge_vec(&["mode"],);
    /// Maximum active circuits in the latest emitted bucket for each mode.
    pub soranet_privacy_active_circuits_max: float_gauge_vec(&["mode"],);
    /// Open privacy buckets still accumulating contributors (per relay mode).
    pub soranet_privacy_open_buckets: float_gauge_vec(&["mode"]);
    /// Pending collector share accumulators grouped by relay mode.
    pub soranet_privacy_pending_collectors: float_gauge_vec(&["mode"]);
    /// Suppressed bucket counts recorded during the latest drain, grouped by reason.
    pub soranet_privacy_snapshot_suppressed: float_gauge_vec(&["reason"]);
    /// Suppressed bucket counts recorded during the latest drain, grouped by mode and reason.
    pub soranet_privacy_snapshot_suppressed_by_mode: float_gauge_vec(&["mode", "reason"],);
    /// Buckets drained during the latest collector flush.
    pub soranet_privacy_snapshot_drained: int_gauge();
    /// Ratio of suppressed to drained buckets observed in the latest flush.
    pub soranet_privacy_snapshot_suppression_ratio: float_gauge();
    /// Completed privacy buckets evicted due to retention.
    pub soranet_privacy_evicted_buckets_total: int_counter();
    /// Suppression indicator for the latest emitted bucket in each relay mode.
    pub soranet_privacy_bucket_suppressed: float_gauge_vec(&["mode"],);
    /// Suppressed bucket counters grouped by relay mode and suppression reason.
    pub soranet_privacy_suppression_total: int_counter_vec(&["mode", "reason"]);
    /// Start time of the latest emitted bucket in each relay mode.
    pub soranet_privacy_latest_bucket_start_unixtime: int_gauge_vec(&["mode"],);
    /// RTT percentile gauges for the latest emitted bucket in each relay mode.
    pub soranet_privacy_rtt_millis: float_gauge_vec(&["mode", "percentile"],);
    /// Aggregated GAR abuse reports keyed only by relay mode.
    ///
    /// Per-category hashes remain in the bounded structured bucket payload. They are deliberately
    /// excluded from Prometheus labels so authenticated category rotation cannot grow the registry.
    pub soranet_privacy_gar_reports_total: int_counter_vec(&["mode"],);
    /// UNIX timestamp of the last successful privacy poll.
    pub soranet_privacy_last_poll_unixtime: int_gauge();
    /// Privacy polling failures grouped by provider alias.
    pub soranet_privacy_poll_errors_total: int_counter_vec(&["provider"]);
    /// Privacy collector enabled flag (0 = disabled, 1 = active).
    pub soranet_privacy_collector_enabled: int_gauge();
    /// Active multi-source orchestrator fetches per manifest/region.
    pub sorafs_orchestrator_active_fetches: int_gauge_vec(&["manifest_id", "region"],);
    /// Multi-source orchestrator fetch duration histogram (milliseconds).
    pub sorafs_orchestrator_fetch_duration_ms: histogram_vec_with_buckets(
        prometheus::exponential_buckets(10.0, 1.8, 12).expect("valid buckets"),
                    &["manifest_id", "region"],
    );
    /// Multi-source orchestrator failures grouped by reason.
    pub sorafs_orchestrator_fetch_failures_total: int_counter_vec(
        &["manifest_id", "region", "reason"],
    );
    /// Multi-source orchestrator retries aggregated per provider.
    pub sorafs_orchestrator_retries_total: int_counter_vec(
        &["manifest_id", "provider_id", "reason"],
    );
    /// Multi-source orchestrator provider failures aggregated per provider.
    pub sorafs_orchestrator_provider_failures_total: int_counter_vec(
        &["manifest_id", "provider_id", "reason"],
    );
    /// Multi-source orchestrator per-chunk latency histogram (milliseconds).
    pub sorafs_orchestrator_chunk_latency_ms: histogram_vec_with_buckets(
        prometheus::exponential_buckets(5.0, 1.7, 16).expect("valid buckets"),
                    &["manifest_id", "provider_id"],
    );
    /// Multi-source orchestrator byte counter aggregated per manifest/provider.
    pub sorafs_orchestrator_bytes_total: int_counter_vec(&["manifest_id", "provider_id"],);
    /// Multi-source orchestrator stall counter (chunks exceeding latency cap).
    pub sorafs_orchestrator_stalls_total: int_counter_vec(&["manifest_id", "provider_id"],);
    /// Transport-layer events emitted by the multi-source orchestrator.
    pub sorafs_orchestrator_transport_events_total: int_counter_vec(
        &["region", "protocol", "event", "reason"],
    );
    /// SoraFS anonymity policy events grouped by stage/outcome/reason/region.
    pub sorafs_orchestrator_policy_events_total: int_counter_vec(
        &["region", "stage", "outcome", "reason"],
    );
    /// Distribution of SoraFS PQ-capable relay ratios grouped by stage/region.
    pub sorafs_orchestrator_pq_ratio: histogram_vec_with_buckets(
        vec![0.0, 0.25, 0.5, 0.66, 0.75, 1.0],
                    &["region", "stage"],
    );
    /// Distribution of SoraFS PQ-capable candidate ratios grouped by stage/region.
    pub sorafs_orchestrator_pq_candidate_ratio: histogram_vec_with_buckets(
        vec![0.0, 0.25, 0.5, 0.75, 1.0],
                    &["region", "stage"],
    );
    /// Distribution of PQ policy shortfalls grouped by stage/region.
    pub sorafs_orchestrator_pq_deficit_ratio: histogram_vec_with_buckets(
        vec![0.0, 0.1, 0.25, 0.5, 0.75, 1.0],
                    &["region", "stage"],
    );
    /// Distribution of classical relay ratios grouped by stage/region.
    pub sorafs_orchestrator_classical_ratio: histogram_vec_with_buckets(
        vec![0.0, 0.25, 0.5, 0.75, 1.0],
                    &["region", "stage"],
    );
    /// Distribution of classical relay selections grouped by stage/region.
    pub sorafs_orchestrator_classical_selected: histogram_vec_with_buckets(
        vec![0.0, 1.0, 2.0, 3.0, 4.0, 8.0, 16.0],
                    &["region", "stage"],
    );
    /// Aggregate GiB-month usage derived from DA rent quotes grouped by cluster/storage class.
    pub torii_da_rent_gib_months_total: int_counter_vec(&["cluster", "storage_class"],);
    /// Aggregate base rent (micro XOR) derived from DA rent quotes.
    pub torii_da_rent_base_micro_total: float_counter_vec(&["cluster", "storage_class"],);
    /// Aggregate protocol reserve contributions (micro XOR) derived from DA rent quotes.
    pub torii_da_protocol_reserve_micro_total: float_counter_vec(&["cluster", "storage_class"],);
    /// Aggregate provider reward payouts (micro XOR) derived from DA rent quotes.
    pub torii_da_provider_reward_micro_total: float_counter_vec(&["cluster", "storage_class"],);
    /// Aggregate PDP bonus payouts (micro XOR) derived from DA rent quotes.
    pub torii_da_pdp_bonus_micro_total: float_counter_vec(&["cluster", "storage_class"],);
    /// Aggregate PoTR bonus payouts (micro XOR) derived from DA rent quotes.
    pub torii_da_potr_bonus_micro_total: float_counter_vec(&["cluster", "storage_class"],);
    /// DA receipt ingest outcomes grouped by bounded outcome/lane labels.
    pub torii_da_receipts_total: int_counter_vec(&["outcome", "lane"]);
    /// Current DA receipt epoch per lane.
    pub torii_da_receipt_epoch: gauge_vec(&["lane"]);
    /// Highest DA receipt sequence observed in the current epoch per lane.
    pub torii_da_receipt_highest_sequence: gauge_vec(&["lane"]);
    /// DA chunking + erasure coding duration (seconds).
    pub torii_da_chunking_seconds: histogram_with_buckets(
        vec![
                        0.001, 0.005, 0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
                    ],
    );
    /// DA spool worker batch outcomes.
    pub torii_da_spool_batches_total: int_counter_vec(&["outcome"]);
    /// DA spool worker artifact outcomes.
    pub torii_da_spool_artifacts_total: int_counter_vec(&["kind", "outcome"]);
    /// Current DA spool worker queue depth.
    pub torii_da_spool_queue_depth: gauge();
    /// DA spool worker batch disk-write duration (milliseconds).
    pub torii_da_spool_batch_write_ms: histogram_with_buckets(
        vec![
                        0.1, 0.5, 1.0, 2.5, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1_000.0,
                    ],
    );
    /// DA shard cursor events grouped by outcome/lane/shard.
    pub da_shard_cursor_events_total: int_counter_vec(&["event", "lane", "shard"]);
    /// Latest block height recorded for each shard cursor advance.
    pub da_shard_cursor_height: int_gauge_vec(&["lane", "shard"]);
    /// Lag in blocks between the validated height and the last shard cursor advance.
    pub da_shard_cursor_lag_blocks: int_gauge_vec(&["lane", "shard"]);
    /// Taikai ingest latency histogram grouped by cluster/stream.
    pub taikai_ingest_segment_latency_ms: histogram_vec_with_buckets(
        vec![
                        10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1_000.0, 2_000.0, 4_000.0,
                    ],
                    &["cluster", "stream"],
    );
    /// Taikai live-edge drift histogram grouped by cluster/stream (absolute value).
    pub taikai_ingest_live_edge_drift_ms: histogram_vec_with_buckets(
        vec![
                        50.0, 100.0, 250.0, 500.0, 1_000.0, 1_500.0, 2_000.0, 3_000.0,
                    ],
                    &["cluster", "stream"],
    );
    /// Signed live-edge drift gauge grouped by cluster/stream (negative = ahead).
    pub taikai_ingest_live_edge_drift_signed_ms: float_gauge_vec(&["cluster", "stream"],);
    /// Taikai ingest failures grouped by cluster/stream/reason.
    pub taikai_ingest_errors_total: int_counter_vec(&["cluster", "stream", "reason"],);
    /// Taikai routing manifest alias rotations grouped by cluster/event/stream/alias.
    pub taikai_trm_alias_rotations_total: int_counter_vec(
        &[
                        "cluster",
                        "event",
                        "stream",
                        "alias_namespace",
                        "alias_name",
                    ],
    );
    /// Taikai viewer rebuffer events grouped by cluster/stream.
    pub taikai_viewer_rebuffer_events_total: int_counter_vec(&["cluster", "stream"],);
    /// Taikai viewer playback segments grouped by cluster/stream.
    pub taikai_viewer_playback_segments_total: int_counter_vec(&["cluster", "stream"],);
    /// Taikai viewer CEK fetch duration histogram grouped by cluster/lane.
    pub taikai_viewer_cek_fetch_duration_ms: histogram_vec_with_buckets(
        vec![5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0],
                    &["cluster", "lane"],
    );
    /// Taikai viewer PQ circuit health gauge grouped by cluster.
    pub taikai_viewer_pq_circuit_health: float_gauge_vec(&["cluster"]);
    /// Taikai viewer CEK rotation age in seconds grouped by lane.
    pub taikai_viewer_cek_rotation_seconds_ago: gauge_vec(&["lane"]);
    /// Taikai viewer alerts firing counter grouped by cluster/alertname.
    pub taikai_viewer_alerts_firing_total: int_counter_vec(&["cluster", "alertname"],);
    /// Count of SoraFS anonymity policy brownouts grouped by stage/reason/region.
    pub sorafs_orchestrator_brownouts_total: int_counter_vec(&["region", "stage", "reason"],);
    /// Configured SoraNet base payout (nano XOR) applied per epoch.
    pub soranet_reward_base_payout_nanos: gauge();
    /// SoraNet reward events grouped by relay/result label.
    pub soranet_reward_events_total: int_counter_vec(&["relay", "result"]);
    /// Aggregated XOR payouts (nano units) grouped by relay/result.
    pub soranet_reward_payout_nanos_total: int_counter_vec(&["relay", "result"]);
    /// Count of SoraNet reward skips grouped by relay/reason.
    pub soranet_reward_skips_total: int_counter_vec(&["relay", "reason"]);
    /// Aggregated XOR adjustments (nano units) grouped by relay/kind.
    pub soranet_reward_adjustment_nanos_total: int_counter_vec(&["relay", "kind"]);
    /// Dispute lifecycle counters grouped by action label.
    pub soranet_reward_disputes_total: int_counter_vec(&["action"]);
    /// Torii HTTP requests grouped by catalog route metadata and bounded response outcome.
    pub torii_http_requests_total: int_counter_vec(
        &[
                        "route_id",
                        "route_template",
                        "surface",
                        "representation",
                        "error_code",
                        "content_type",
                        "method",
                        "status",
                    ],
    );
    /// Torii HTTP request latency in seconds grouped by catalog route metadata.
    pub torii_http_request_duration_seconds: histogram_vec_with_buckets(
        prometheus::exponential_buckets(0.005, 2.0, 13).expect("inputs are valid"),
                    &[
                        "route_id",
                        "route_template",
                        "surface",
                        "representation",
                        "content_type",
                        "method",
                    ],
    );
    /// Torii HTTP request payload size (bytes) grouped by catalog route metadata.
    pub torii_http_request_bytes_total: int_counter_vec(
        &[
                        "route_id",
                        "route_template",
                        "surface",
                        "representation",
                        "content_type",
                        "method",
                    ],
    );
    /// Torii HTTP response payload size (bytes) grouped by catalog route metadata and outcome.
    pub torii_http_response_bytes_total: int_counter_vec(
        &[
                        "route_id",
                        "route_template",
                        "surface",
                        "representation",
                        "error_code",
                        "content_type",
                        "method",
                        "status",
                    ],
    );
    /// Torii API-token-gated endpoint hits grouped by endpoint and bounded token state.
    pub torii_api_token_hits_total: int_counter_vec(&["endpoint", "token_state"]);
    /// Content gateway requests grouped by outcome label.
    pub torii_content_requests_total: int_counter_vec(&["outcome"]);
    /// Content gateway response latency in seconds grouped by outcome.
    pub torii_content_request_duration_seconds: histogram_vec_with_buckets(
        prometheus::exponential_buckets(0.005, 2.0, 13).expect("inputs are valid"),
                    &["outcome"],
    );
    /// Content gateway bytes served grouped by outcome label.
    pub torii_content_response_bytes_total: int_counter_vec(&["outcome"]);
    /// Proof endpoint requests grouped by endpoint/outcome.
    pub torii_proof_requests_total: int_counter_vec(&["endpoint", "outcome"]);
    /// Proof endpoint latency in seconds grouped by endpoint/outcome.
    pub torii_proof_request_duration_seconds: histogram_vec_with_buckets(
        prometheus::exponential_buckets(0.001, 2.0, 12).expect("inputs are valid"),
                    &["endpoint", "outcome"],
    );
    /// Proof endpoint bytes served grouped by endpoint/outcome.
    pub torii_proof_response_bytes_total: int_counter_vec(&["endpoint", "outcome"]);
    /// Proof endpoint cache hits grouped by endpoint.
    pub torii_proof_cache_hits_total: int_counter_vec(&["endpoint"]);
    /// Torii request latency in seconds grouped by connection scheme.
    pub torii_request_duration_seconds: histogram_vec_with_buckets(
        prometheus::exponential_buckets(0.005, 2.0, 13).expect("inputs are valid"),
                    &["scheme"],
    );
    /// Torii request failures grouped by connection scheme and status code.
    pub torii_request_failures_total: int_counter_vec(&["scheme", "code"]);
    /// Explorer endpoint requests grouped by endpoint and outcome.
    pub torii_explorer_requests_total: int_counter_vec(&["endpoint", "outcome"]);
    /// Explorer endpoint latency in seconds grouped by endpoint and outcome.
    pub torii_explorer_request_duration_seconds: histogram_vec_with_buckets(
        prometheus::exponential_buckets(0.001, 2.0, 14).expect("inputs are valid"),
                    &["endpoint", "outcome"],
    );
    /// Norito-RPC gate decisions grouped by rollout stage and outcome.
    pub torii_norito_rpc_gate_total: int_counter_vec(&["stage", "outcome"]);
    /// Proof endpoints throttled by rate limiter (labeled by endpoint).
    pub torii_proof_throttled_total: int_counter_vec(&["endpoint"]);
    /// Torii contract endpoints throttled by rate limiter (labeled by endpoint).
    pub torii_contract_throttled_total: int_counter_vec(&["endpoint"]);
    /// Torii contract endpoints returning errors (labeled by endpoint).
    pub torii_contract_errors_total: int_counter_vec(&["endpoint"]);
    /// SNS registrar outcomes grouped by result and suffix.
    pub sns_registrar_status_total: int_counter_vec(&["result", "suffix"]);
    /// Torii account address rejects grouped by endpoint/reason.
    pub torii_address_invalid_total: int_counter_vec(&["endpoint", "reason"]);
    /// Torii account literal selections grouped by endpoint/format.
    pub torii_account_literal_total: int_counter_vec(&["endpoint", "format"]);
    /// Torii Norito RPC decode failures grouped by payload kind/reason.
    pub torii_norito_decode_failures_total: int_counter_vec(&["payload_kind", "reason"],);
    /// Torii pre-auth: active connections tracked by scheme (http/ws)
    pub torii_active_connections_total: gauge_vec(&["scheme"]);
    /// Torii Connect: sessions with buffered frames (gauge)
    pub torii_connect_buffered_sessions: gauge();
    /// Torii Connect: total buffered bytes across sessions (gauge)
    pub torii_connect_total_buffer_bytes: gauge();
    /// Torii Connect: dedupe cache size (gauge)
    pub torii_connect_dedupe_size: gauge();
    /// Torii Connect: per-IP session counts (gauge vec labeled by ip)
    pub torii_connect_per_ip_sessions: gauge_vec(&["ip"]);
    /// NTS: network time offset vs local clock (signed, milliseconds)
    pub nts_offset_ms: int_gauge();
    /// NTS: confidence (MAD) in milliseconds
    pub nts_confidence_ms: gauge();
    /// NTS: number of peers currently contributing samples
    pub nts_peers_sampled: gauge();
    /// NTS: number of samples used in aggregation (post-filter)
    pub nts_samples_used: gauge();
    /// NTS: health status (1 = healthy, 0 = unhealthy)
    pub nts_healthy: int_gauge();
    /// NTS: fallback indicator (1 = local time fallback, 0 = NTS offset)
    pub nts_fallback: int_gauge();
    /// NTS: minimum sample threshold check (1 = ok, 0 = fail)
    pub nts_min_samples_ok: int_gauge();
    /// NTS: offset bound check (1 = ok, 0 = fail)
    pub nts_offset_ok: int_gauge();
    /// NTS: confidence bound check (1 = ok, 0 = fail)
    pub nts_confidence_ok: int_gauge();
    /// NTS: RTT histogram buckets labeled by `le` (ms)
    pub nts_rtt_ms_bucket: gauge_vec(&["le"]);
    /// NTS: RTT histogram sum of ms
    pub nts_rtt_ms_sum: gauge();
    /// NTS: RTT histogram count of observations
    pub nts_rtt_ms_count: gauge();
    /// Aggregate verification latency (ms) by event kind.
    pub zk_verify_latency_ms: histogram_vec_with_buckets(
        prometheus::exponential_buckets(1.0, 2.0, 15).expect("inputs are valid"),
                    &["backend", "status"],
    );
    /// Aggregate verification proof size (bytes) by event kind.
    pub zk_verify_proof_bytes: histogram_vec_with_buckets(
        prometheus::exponential_buckets(256.0, 2.0, 12).expect("inputs are valid"),
                    &["backend", "status"],
    );
    /// Serializes finalized orderbook projection updates with Prometheus exposition.
    ///
    /// A scrape therefore observes either the preceding complete projection or
    /// its complete successor, never the intermediate ready=0 update sequence.
    sorafs_orderbook_projection_exposition_lock: raw(Mutex<()>);
    /// Serializes gateway-compliance serving-catalog updates with exposition.
    sorafs_gateway_compliance_exposition_lock: raw(Mutex<()>);
    /// Low-cardinality Musubi V1 registry, publication, cache, and storage metrics.
    pub musubi: raw(musubi::MusubiMetrics);
    /// Internal use only. Needed for generating the response.
    registry: raw(Registry);
}
prefix (metrics) {
    let registry = Registry::new();
    let musubi = musubi::MusubiMetrics::new(&registry);
    let mut metric_specs = MetricSpecCursor::v2();
    let mut metrics = MetricFactory::new(&registry, &mut metric_specs);
}
construct {
    [txs isi isi_times tx_amounts block_height block_height_non_empty last_commit_time_ms
        last_block_committed_at_ms last_non_empty_block_committed_at_ms commit_time_ms
        slot_duration_ms slot_duration_ms_latest da_quorum_ratio sm_syscall_total]
    {
        for (kind, mode) in [
            ("hash", "-"),
            ("verify", "-"),
            ("seal", "gcm"),
            ("open", "gcm"),
            ("seal", "ccm"),
            ("open", "ccm"),
        ] {
            let _ = sm_syscall_total.with_label_values(&[kind, mode]);
        }
    }
    [sm_openssl_preview zk_halo2_enabled zk_halo2_curve_id zk_halo2_backend_id zk_halo2_max_k
        zk_halo2_verifier_budget_ms zk_halo2_verifier_max_batch zk_halo2_verifier_worker_threads
        zk_halo2_verifier_queue_cap zk_lane_enqueue_wait_total zk_lane_enqueue_timeout_total
        zk_lane_drop_total zk_lane_retry_enqueued_total zk_lane_retry_replayed_total
        zk_lane_retry_exhausted_total zk_lane_pending_depth zk_lane_retry_ring_depth
        zk_verifier_cache_events_total confidential_gas_base_verify
        confidential_gas_per_public_input confidential_gas_per_proof_byte
        confidential_gas_per_nullifier confidential_gas_per_commitment ivm_gas_schedule_hash_lo
        ivm_gas_schedule_hash_hi confidential_tree_commitments confidential_tree_depth
        confidential_root_history_entries confidential_frontier_checkpoints
        confidential_frontier_last_height confidential_frontier_last_commitments
        confidential_root_evictions_total confidential_frontier_evictions_total
        oracle_price_local_per_xor oracle_twap_window_seconds oracle_haircut_basis_points
        oracle_staleness_seconds oracle_observations_total oracle_aggregation_duration_ms
        oracle_rewards_total oracle_penalties_total oracle_feed_events_total
        oracle_feed_events_with_evidence_total oracle_evidence_hashes_total
        fastpq_execution_mode_total fastpq_poseidon_pipeline_total fastpq_gpu_disable_total
        fastpq_gpu_parity_failure_total fastpq_proof_sidecar_queue_depth
        fastpq_proof_sidecar_events_total fastpq_metal_queue_ratio fastpq_metal_queue_depth
        fastpq_zero_fill_duration_ms fastpq_zero_fill_bandwidth_gbps sm_syscall_failures_total
        settlement_events_total]
    {
        for kind in ["dvp", "pvp"] {
            let _ = settlement_events_total.with_label_values(&[kind, "success", "-"]);
            for reason in [
                "insufficient_funds",
                "counterparty_mismatch",
                "unsupported_policy",
                "zero_quantity",
                "missing_entity",
                "math_error",
                "other",
            ] {
                let _ = settlement_events_total.with_label_values(&[kind, "failure", reason]);
            }
        }
    }
    [settlement_finality_events_total]
    {
        for (kind, states) in [
            ("dvp", ["none", "delivery_only", "payment_only", "both"]),
            ("pvp", ["none", "primary_only", "counter_only", "both"]),
        ] {
            for outcome in ["success", "failure"] {
                for state in states {
                    let _ =
                        settlement_finality_events_total.with_label_values(&[kind, outcome, state]);
                }
            }
        }
    }
    [settlement_fx_window_ms]
    {
        for kind in ["pvp"] {
            for order in ["delivery_then_payment", "payment_then_delivery"] {
                for atomicity in ["all_or_nothing", "commit_first_leg", "commit_second_leg"] {
                    let _ = settlement_fx_window_ms.with_label_values(&[kind, order, atomicity]);
                }
            }
        }
    }
    [settlement_buffer_xor settlement_buffer_capacity_xor settlement_buffer_status
        settlement_pnl_xor settlement_haircut_bp settlement_swapline_utilisation
        settlement_conversion_total settlement_haircut_total subscription_billing_attempts_total
        subscription_billing_outcomes_total]
    {
        for pricing in ["fixed", "usage"] {
            let _ = subscription_billing_attempts_total.with_label_values(&[pricing]);
            for result in ["paid", "failed", "suspended", "skipped"] {
                let _ = subscription_billing_outcomes_total.with_label_values(&[pricing, result]);
            }
        }
    }
    [social_events_total]
    {
        for event in [
            "reward_paid",
            "escrow_created",
            "escrow_released",
            "escrow_cancelled",
        ] {
            let _ = social_events_total.with_label_values(&[event]);
        }
    }
    [social_budget_spent social_campaign_spent social_campaign_cap social_campaign_remaining
        social_campaign_active social_halted social_rejections_total]
    {
        for reason in [
            "halted",
            "promo_window",
            "binding_not_found",
            "binding_not_follow",
            "binding_expired",
            "deny_uaid",
            "deny_binding",
            "daily_cap",
            "binding_cap",
            "campaign_cap",
            "budget_exhausted",
            "duplicate_escrow",
            "zero_amount",
            "escrow_missing",
            "escrow_owner_mismatch",
        ] {
            let _ = social_rejections_total.with_label_values(&[reason]);
        }
    }
    [multisig_direct_sign_reject_total social_open_escrows connected_peers p2p_peer_churn_total]
    {
        for event in ["connected", "disconnected"] {
            let _ = p2p_peer_churn_total.with_label_values(&[event]);
        }
    }
    [uptime_since_genesis_ms domains accounts view_changes queue_size queue_queued
        queue_inflight kura_fsync_enabled kura_fsync_failures_total]
    [kura_fsync_latency_ms]
    [amx_prepare_ms amx_commit_ms amx_abort_total axt_policy_reject_total
        axt_policy_snapshot_version axt_policy_snapshot_cache_events_total
        axt_proof_cache_events_total axt_proof_cache_state ivm_exec_ms ivm_stack_bytes
        ivm_stack_clamped ivm_stack_gas_multiplier ivm_stack_pool_fallback_total
        ivm_stack_budget_hit_total sumeragi_tx_queue_depth sumeragi_tx_queue_capacity
        sumeragi_tx_queue_retained_bytes sumeragi_tx_queue_max_retained_bytes
        sumeragi_tx_queue_saturated sumeragi_tx_queue_saturated_by_count
        sumeragi_tx_queue_saturated_by_bytes sumeragi_tx_queue_saturated_by_age
        sumeragi_tx_queue_oldest_queued_age_ms sumeragi_pending_blocks_total
        sumeragi_pending_blocks_blocking sumeragi_commit_inflight_queue_depth]
    [sumeragi_missing_block_requests sumeragi_missing_block_oldest_ms
        sumeragi_missing_block_retry_window_ms sumeragi_missing_block_dwell_ms
        sumeragi_epoch_length_blocks sumeragi_epoch_commit_deadline_offset
        sumeragi_epoch_reveal_deadline_offset state_tiered_hot_entries state_tiered_hot_bytes
        state_tiered_cold_entries state_tiered_cold_bytes state_tiered_cold_reused_entries
        state_tiered_cold_reused_bytes state_tiered_hot_promotions state_tiered_hot_demotions
        state_tiered_hot_budget_overflow_keys state_tiered_hot_budget_overflow_bytes
        state_tiered_last_snapshot_index storage_budget_bytes_used storage_budget_bytes_limit
        storage_budget_exceeded_total storage_da_cache_total storage_da_churn_bytes_total
        governance_proposals_status governance_parliament_transitions_total
        governance_parliament_no_result_total governance_parliament_attempts_by_status
        governance_parliament_attempts_by_stage]
    {
        for status in [
            "proposed",
            "rejected",
            "enacted",
            "superseded",
            "execution_failed",
        ] {
            governance_proposals_status
                .with_label_values(&[status])
                .set(0);
        }
        for transition in [
            "escalate_risk",
            "complete_qualification",
            "register_sortition_request",
            "consume_sortition_pulse_batch",
            "begin_invitation_acceptance",
            "fail_body_election_no_roster",
            "seal_body_roster",
            "advance_body_phase",
            "record_attempt_absence",
            "endorse_public_finding",
            "fail_public_finding_no_result",
            "register_ballot_attempt",
            "close_ballot_registration",
            "freeze_ballot_survivors",
            "freeze_timed_ovn_corpus",
            "begin_ballot_opening_batch",
            "fail_ballot_no_result",
            "finalize_opened_ballot",
            "mark_enacted",
            "mark_superseded",
            "mark_execution_failed",
            "record_invitation_response",
            "register_ballot_participant",
            "record_ballot_dropout",
        ] {
            let _ = governance_parliament_transitions_total
                .with_label_values(&[transition]);
        }
        for class in [
            "public_finding_quorum_unreachable",
            "public_finding_deadline_expired",
            "ballot_registration_deadline_expired",
            "ballot_survivor_deadline_expired",
            "ballot_commitment_deadline_expired",
            "ballot_release_pulse_unavailable",
            "ballot_opening_deadline_expired",
            "sortition_retries_exhausted",
        ] {
            let _ = governance_parliament_no_result_total.with_label_values(&[class]);
        }
        for status in [
            "active",
            "certified",
            "rejected",
            "enacted",
            "superseded",
            "execution_failed",
        ] {
            governance_parliament_attempts_by_status
                .with_label_values(&[status])
                .set(0);
        }
        for stage in [
            "qualification",
            "rules",
            "agenda",
            "interest",
            "review",
            "coordination",
            "mpc",
            "fma",
            "oversight",
            "policy_jury",
            "confirmation_jury",
            "certification",
            "enactment",
        ] {
            governance_parliament_attempts_by_stage
                .with_label_values(&[stage])
                .set(0);
        }
    }
    [governance_council_members governance_council_alternates governance_council_candidates
        governance_council_epoch governance_citizens_total governance_citizen_service_events_total]
    {
        for event in ["decline", "no_show", "misconduct"] {
            let _ = governance_citizen_service_events_total.with_label_values(&[event]);
        }
    }
    [governance_protected_namespace_total]
    {
        for outcome in ["allowed", "rejected"] {
            let _ = governance_protected_namespace_total.with_label_values(&[outcome]);
        }
    }
    [governance_manifest_admission_total]
    {
        for result in [
            "allowed",
            "missing_manifest",
            "non_validator_authority",
            "quorum_rejected",
            "protected_namespace_rejected",
            "runtime_hook_rejected",
        ] {
            let _ = governance_manifest_admission_total.with_label_values(&[result]);
        }
    }
    [governance_manifest_quorum_total]
    {
        for outcome in ["satisfied", "rejected"] {
            let _ = governance_manifest_quorum_total.with_label_values(&[outcome]);
        }
    }
    [governance_manifest_hook_total]
    {
        for hook in ["runtime_upgrade"] {
            for outcome in ["allowed", "rejected"] {
                let _ = governance_manifest_hook_total.with_label_values(&[hook, outcome]);
            }
        }
    }
    [governance_manifest_activations_total]
    {
        for event in ["manifest_inserted", "instance_bound"] {
            let _ = governance_manifest_activations_total.with_label_values(&[event]);
        }
    }
    [governance_bond_events_total]
    {
        for event in ["lock_created", "lock_extended", "lock_unlocked"] {
            let _ = governance_bond_events_total.with_label_values(&[event]);
        }
        let governance_manifest_recent = Arc::new(RwLock::new(VecDeque::with_capacity(
            GOVERNANCE_MANIFEST_RECENT_CAP,
        )));
        let taikai_ingest_snapshots = Arc::new(RwLock::new(BTreeMap::<
            (String, String),
            TaikaiIngestSnapshotInternal,
        >::new()));
        let taikai_ingest_snapshot_order = Arc::new(RwLock::new(VecDeque::with_capacity(
            TAIKAI_INGEST_SNAPSHOT_CAP,
        )));
        let taikai_alias_rotation_snapshots: TaikaiAliasRotationSnapshots =
            Arc::new(RwLock::new(BTreeMap::new()));
        let taikai_alias_rotation_snapshot_order: TaikaiAliasRotationSnapshotOrder = Arc::new(
            RwLock::new(VecDeque::with_capacity(
                TAIKAI_ALIAS_ROTATION_SNAPSHOT_CAP,
            )),
        );
        let da_receipt_metric_lanes: Arc<RwLock<BTreeMap<u32, DaReceiptMetricLane>>> =
            Arc::new(RwLock::new(BTreeMap::new()));
        let recent_rejection_events =
            Mutex::new(VecDeque::with_capacity(REJECTION_RECENT_EVENT_CAP));
        let last_rejection_at_ms = StdAtomicU64::new(0);
    }
    [alias_usage_total iso_reference_status iso_reference_age_seconds iso_reference_records
        iso_reference_refresh_interval_secs]
    {
        for dataset in ["isin_cusip", "bic_lei", "mic_directory"] {
            let _ = iso_reference_status.with_label_values(&[dataset]);
            let _ = iso_reference_age_seconds.with_label_values(&[dataset]);
            let _ = iso_reference_records.with_label_values(&[dataset]);
            let _ = iso_reference_refresh_interval_secs.with_label_values(&[dataset]);
        }
    }
    [fraud_psp_assessments_total fraud_psp_missing_assessment_total
        fraud_psp_invalid_metadata_total fraud_psp_attestation_total fraud_psp_latency_ms
        fraud_psp_score_bps fraud_psp_outcome_mismatch_total streaming_hpke_rekeys_total]
    {
        for suite in ["x25519", "kyber768"] {
            let _ = streaming_hpke_rekeys_total.with_label_values(&[suite]);
        }
    }
    [streaming_gck_rotations_total streaming_quic_datagrams_sent_total
        streaming_quic_datagrams_dropped_total streaming_fec_parity_current
        streaming_feedback_timeout_total]
    {
        for bucket in ["0", "1", "2", "3", "4", "ge5"] {
            streaming_fec_parity_current
                .with_label_values(&[bucket])
                .set(0);
        }
    }
    [telemetry_redaction_total]
    {
        for reason in ["keyword", "explicit"] {
            let _ = telemetry_redaction_total.with_label_values(&[reason]);
        }
    }
    [telemetry_redaction_skipped_total]
    {
        for reason in ["allowlist", "disabled", "unsupported"] {
            let _ = telemetry_redaction_skipped_total.with_label_values(&[reason]);
        }
    }
    [telemetry_truncation_total streaming_privacy_redaction_fail_total
        streaming_encode_latency_ms streaming_encode_audio_jitter_ms
        streaming_encode_audio_max_jitter_ms]
    {
        streaming_encode_audio_max_jitter_ms.set(0);
    }
    [streaming_encode_dropped_layers_total streaming_decode_buffer_ms
        streaming_decode_dropped_frames_total streaming_decode_max_queue_ms
        streaming_decode_av_drift_ms streaming_decode_max_drift_ms]
    {
        streaming_decode_max_drift_ms.set(0);
    }
    [streaming_audio_jitter_ms streaming_audio_max_jitter_ms]
    {
        streaming_audio_max_jitter_ms.set(0);
    }
    [streaming_av_drift_ms streaming_av_max_drift_ms]
    {
        streaming_av_max_drift_ms.set(0);
    }
    [streaming_av_drift_ewma_ms]
    {
        streaming_av_drift_ewma_ms.set(0);
    }
    [streaming_av_sync_window_ms]
    {
        streaming_av_sync_window_ms.set(0);
    }
    [streaming_av_sync_violation_total streaming_network_rtt_ms
        streaming_network_loss_percent_x100 streaming_network_fec_repairs_total
        streaming_network_fec_failures_total streaming_network_datagram_reinjects_total
        streaming_energy_encoder_mw streaming_energy_decoder_mw nexus_audit_outcome_total
        nexus_audit_outcome_last_timestamp nexus_space_directory_revision_total
        nexus_space_directory_active_manifests nexus_space_directory_revocations_total
        kaigi_relay_registered_total kaigi_relay_registration_bandwidth
        kaigi_relay_manifest_updates_total kaigi_relay_manifest_hop_count
        kaigi_relay_failover_total kaigi_relay_failover_hop_count kaigi_relay_health_reports_total
        kaigi_relay_health_state dropped_messages sumeragi_dropped_block_messages_total
        sumeragi_dropped_control_messages_total p2p_dropped_posts p2p_dropped_broadcasts
        p2p_subscriber_queue_full_total p2p_subscriber_queue_full_by_topic_total
        p2p_subscriber_unrouted_total p2p_subscriber_unrouted_by_topic_total p2p_handshake_failures
        p2p_low_post_throttled_total p2p_low_broadcast_throttled_total p2p_post_overflow_total
        p2p_post_overflow_by_topic consensus_ingress_drop_total p2p_dns_refresh_total
        p2p_dns_ttl_refresh_total p2p_dns_resolution_fail_total p2p_dns_reconnect_success_total
        p2p_backoff_scheduled_total p2p_deferred_send_enqueued_total
        p2p_deferred_send_dropped_total p2p_session_reconnect_total p2p_connect_retry_seconds
        p2p_accept_throttled_total p2p_accept_bucket_evictions_total p2p_accept_buckets_current
        p2p_accept_prefix_cache_total p2p_accept_throttle_decisions_total
        p2p_incoming_cap_reject_total p2p_total_cap_reject_total
        p2p_preauth_source_cap_reject_total p2p_trust_score
        p2p_trust_penalties_total p2p_trust_decay_ticks_total p2p_trust_gossip_skipped_total]
    {
        for direction in ["send", "recv"] {
            for reason in ["peer_capability_off", "local_capability_off"] {
                let _ = p2p_trust_gossip_skipped_total.with_label_values(&[direction, reason]);
            }
        }
    }
    [p2p_scion_inbound_total p2p_scion_outbound_total
        tx_gossip_sent_total tx_gossip_dropped_total tx_gossip_targets tx_gossip_fallback_total
        tx_gossip_frame_cap_bytes tx_gossip_public_target_cap tx_gossip_restricted_target_cap
        tx_gossip_public_target_reshuffle_ms tx_gossip_restricted_target_reshuffle_ms
        tx_gossip_drop_unknown_dataspace tx_gossip_restricted_fallback
        tx_gossip_restricted_public_policy]
    {
        let tx_gossip_status = Arc::new(RwLock::new(Vec::new()));
        let tx_gossip_caps = Arc::new(RwLock::new(TxGossipCaps::default()));
    }
    [sumeragi_new_view_receipts_by_hv sumeragi_post_to_peer_total
        sumeragi_bg_post_enqueued_total sumeragi_bg_post_overflow_total sumeragi_bg_post_drop_total
        sumeragi_bg_post_queue_depth sumeragi_bg_post_queue_depth_by_peer sumeragi_bg_post_age_ms
        sumeragi_new_view_publish_total sumeragi_new_view_recv_total
        sumeragi_new_view_dropped_by_lock_total sumeragi_commit_conflict_detected_total
        sumeragi_missing_block_fetch_total sumeragi_missing_block_fetch_target_total
        sumeragi_missing_block_fetch_dwell_ms sumeragi_missing_block_fetch_targets
        blocksync_qc_quarantine_total blocksync_qc_revalidated_total blocksync_qc_final_drop_total
        qc_deferred_missing_payload_total qc_deferred_resolved_total qc_deferred_expired_total
        consensus_empty_commit_topology_defer_total
        consensus_empty_commit_topology_escalation_total consensus_recovery_state_transitions_total
        consensus_missing_block_height_escalation_total consensus_sidecar_quarantine_total
        consensus_sidecar_final_drop_total blocksync_range_pull_escalation_total
        blocksync_range_pull_success_total blocksync_range_pull_failure_total
        consensus_recovery_stuck_round_seconds sumeragi_da_gate_block_total
        sumeragi_da_gate_last_reason sumeragi_da_gate_last_satisfied
        sumeragi_da_gate_satisfied_total sumeragi_da_manifest_guard_total
        sumeragi_da_manifest_cache_total sumeragi_da_spool_cache_total
        sumeragi_da_pin_intent_spool_total]
    [sumeragi_da_votes_ingested_total sumeragi_qc_assembly_latency_ms
        sumeragi_qc_last_latency_ms sumeragi_kura_store_failures_total
        sumeragi_kura_store_last_retry_attempt sumeragi_kura_store_last_retry_backoff_ms
        sumeragi_pacemaker_backpressure_deferrals_total
        sumeragi_pacemaker_backpressure_deferrals_by_reason_total
        sumeragi_pacemaker_backpressure_deferral_duration_ms
        sumeragi_pacemaker_backpressure_deferral_active
        sumeragi_pacemaker_backpressure_deferral_age_ms sumeragi_pacemaker_eval_ms
        sumeragi_pacemaker_propose_ms sumeragi_commit_stage_ms state_commit_write_lock_wait_ms
        state_commit_write_lock_hold_ms sumeragi_commit_pipeline_tick_total
        sumeragi_prevote_timeout_total sumeragi_membership_mismatch_total
        sumeragi_membership_mismatch_active sumeragi_highest_qc_height sumeragi_locked_qc_height
        sumeragi_locked_qc_view]
        // Sumeragi pacemaker gauges
    [sumeragi_pacemaker_backoff_ms sumeragi_pacemaker_rtt_floor_ms
        sumeragi_pacemaker_backoff_multiplier sumeragi_pacemaker_rtt_floor_multiplier
        sumeragi_pacemaker_max_backoff_ms sumeragi_pacemaker_jitter_ms
        sumeragi_pacemaker_jitter_frac_permille sumeragi_pacemaker_round_elapsed_ms
        sumeragi_pacemaker_view_timeout_target_ms sumeragi_pacemaker_view_timeout_remaining_ms
        sumeragi_phase_latency_ms sumeragi_phase_latency_ema_ms sumeragi_phase_total_ema_ms
        p2p_queue_depth p2p_queue_dropped_total p2p_handshake_ms_bucket p2p_handshake_ms_sum
        p2p_handshake_ms_count p2p_handshake_error_total p2p_frame_cap_violations_total]
        // Runtime upgrade metrics
    [runtime_upgrade_events_total runtime_upgrade_provenance_rejections_total
        runtime_abi_version]
        // Sumeragi consensus counters/histogram
    [sumeragi_tail_votes_total sumeragi_votes_sent_total sumeragi_votes_received_total
        sumeragi_qc_sent_total sumeragi_qc_received_total sumeragi_qc_validation_errors_total
        sumeragi_qc_signer_counts sumeragi_invalid_signature_total]
    {
        for label in ["prevote", "precommit", "available"] {
            let _ = sumeragi_votes_sent_total.with_label_values(&[label]);
            let _ = sumeragi_votes_received_total.with_label_values(&[label]);
            let _ = sumeragi_qc_sent_total.with_label_values(&[label]);
            let _ = sumeragi_qc_received_total.with_label_values(&[label]);
        }
        for label in [
            "bitmap_length_mismatch",
            "signer_out_of_bounds",
            "insufficient_signers",
            "missing_votes",
            "duplicate_signers",
            "aggregate_mismatch",
            "subject_mismatch",
            "invalid_signature",
        ] {
            let _ = sumeragi_qc_validation_errors_total.with_label_values(&[label]);
        }
    }
    [sumeragi_validation_reject_total]
    {
        for label in [
            "stateless",
            "execution",
            "prev_hash",
            "prev_height",
            "topology",
        ] {
            let _ = sumeragi_validation_reject_total.with_label_values(&[label]);
        }
    }
    [sumeragi_validation_reject_last_reason sumeragi_validation_reject_last_height
        sumeragi_validation_reject_last_view sumeragi_validation_reject_last_timestamp_ms]
    [sumeragi_block_sync_share_blocks_unsolicited_total
        sumeragi_consensus_message_handling_total sumeragi_view_change_cause_total
        sumeragi_view_change_cause_last_timestamp_ms]
    {
        for label in [
            "commit_failure",
            "quorum_timeout",
            "stake_quorum_timeout",
            "censorship_evidence",
            "da_gate",
            "missing_payload",
            "missing_qc",
            "validation_reject",
        ] {
            let _ = sumeragi_view_change_cause_total.with_label_values(&[label]);
            let _ = sumeragi_view_change_cause_last_timestamp_ms.with_label_values(&[label]);
        }
        for phase in ["prevote", "precommit", "available", "commit"] {
            for kind in ["present", "counted"] {
                let _ = sumeragi_qc_signer_counts.with_label_values(&[phase, kind]);
            }
        }
        for outcome in ["logged", "throttled"] {
            let _ = sumeragi_invalid_signature_total.with_label_values(&["vote", outcome]);
        }
    }
    [sumeragi_widen_before_rotate_total sumeragi_view_change_suggest_total
        sumeragi_view_change_install_total sumeragi_proposal_gap_total
        sumeragi_view_change_proof_total]
    {
        for label in ["accepted", "stale", "rejected"] {
            let _ = sumeragi_view_change_proof_total.with_label_values(&[label]);
        }
    }
    [sumeragi_wa_qc_assembled_total sumeragi_cert_size sumeragi_commit_signatures_present
        sumeragi_commit_signatures_counted sumeragi_commit_signatures_set_b
        sumeragi_commit_signatures_required sumeragi_commit_qc_height sumeragi_commit_qc_view
        sumeragi_commit_qc_epoch sumeragi_commit_qc_signatures_total
        sumeragi_commit_qc_validator_set_len sumeragi_gossip_fallback_total
        sumeragi_block_created_dropped_by_lock_total sumeragi_block_created_hint_mismatch_total
        sumeragi_block_created_proposal_mismatch_total lane_relay_invalid_total
        lane_relay_emergency_override_total]
    {
        let sumeragi_prf_epoch_seed_hex: Arc<RwLock<Option<String>>> = Arc::new(RwLock::new(None));
        let sumeragi_mode_tag: Arc<RwLock<String>> =
            Arc::new(RwLock::new(PERMISSIONED_TAG.to_string()));
        let halo2_status: Arc<RwLock<Halo2Status>> = Arc::new(RwLock::new(Halo2Status::default()));
    }
    [sumeragi_prf_height sumeragi_prf_view sumeragi_membership_view_hash
        sumeragi_membership_height sumeragi_membership_view sumeragi_membership_epoch
        sumeragi_leader_index ivm_cache_hits ivm_cache_misses ivm_cache_evictions
        ivm_cache_decoded_streams ivm_cache_decoded_ops_total ivm_cache_decode_failures
        ivm_cache_decode_time_ns_total ivm_register_max_index ivm_register_unique_count
        merkle_root_gpu_total merkle_root_cpu_total ivm_memory_commit_ms
        ivm_memory_commit_dirty_chunks ivm_merkle_rebuild_total
        ivm_merkle_incremental_leaf_updates_total pipeline_dag_vertices pipeline_dag_edges
        pipeline_conflict_rate_bps pipeline_access_set_source_total pipeline_comp_count
        pipeline_comp_max pipeline_comp_hist_bucket pipeline_peak_layer_width
        pipeline_layer_avg_width pipeline_layer_median_width nexus_config_diff_total
        nexus_lane_configured_total
        nexus_lane_governance_sealed nexus_lane_governance_sealed_total
        nexus_lane_lifecycle_applied_total]
    {
        let nexus_lane_governance_sealed_aliases = Arc::new(RwLock::new(Vec::new()));
    }
    [nexus_lane_block_height nexus_lane_finality_lag_slots nexus_lane_settlement_backlog_xor
        nexus_public_lane_validator_total nexus_public_lane_validator_activation_total
        nexus_public_lane_validator_reject_total nexus_public_lane_stake_bonded
        nexus_public_lane_unbond_pending nexus_public_lane_reward_total
        nexus_public_lane_slash_total nexus_scheduler_lane_teu_capacity
        nexus_scheduler_lane_teu_slot_committed nexus_scheduler_lane_trigger_level
        nexus_scheduler_starvation_bound_slots nexus_scheduler_lane_teu_slot_breakdown
        nexus_scheduler_lane_teu_deferral_total nexus_scheduler_lane_headroom_events_total
        nexus_scheduler_must_serve_truncations_total nexus_scheduler_dataspace_teu_backlog
        nexus_scheduler_dataspace_age_slots nexus_scheduler_dataspace_virtual_finish]
    {
        let nexus_scheduler_lane_teu_status =
            Arc::new(RwLock::new(BTreeMap::<u32, NexusLaneTeuStatus>::new()));
        let nexus_scheduler_dataspace_teu_status = Arc::new(RwLock::new(BTreeMap::<
            (u32, u64),
            NexusDataspaceTeuStatus,
        >::new()));
    }
    [pipeline_layer_count pipeline_scheduler_utilization_pct pipeline_layer_width_hist_bucket
        pipeline_overlay_count pipeline_overlay_instructions pipeline_overlay_bytes
        pipeline_quarantine_classified pipeline_quarantine_overflow pipeline_quarantine_executed
        pipeline_stage_ms pipeline_detached_prepared pipeline_detached_merged
        pipeline_detached_fallback pipeline_detached_fallback_reason merge_ledger_entries_total
        merge_ledger_latest_epoch]
    {
        let merge_ledger_latest_root_hex: Arc<RwLock<Option<String>>> = Arc::new(RwLock::new(None));

        // Torii metrics (app-facing): record filter complexity, match counts,
        // scan latencies, and approximate stream sizes. Labeled by endpoint.
    }
    [torii_filter_depth torii_filter_match_count torii_scan_ms torii_stream_rows
        torii_lane_admission_latency_seconds torii_route_stage_latency_seconds
        torii_attachment_reject_total torii_attachment_sanitize_ms torii_zk_prover_attachment_bytes
        torii_zk_prover_latency_ms torii_zk_prover_gc_total torii_zk_prover_inflight
        torii_zk_prover_pending torii_zk_ivm_prove_inflight torii_zk_ivm_prove_queued
        torii_zk_prover_last_scan_bytes torii_zk_prover_last_scan_ms
        torii_zk_prover_budget_exhausted_total]
        // Snapshot-lane counters
    [torii_query_snapshot_requests torii_query_snapshot_first_batch_ms
        torii_query_snapshot_gas_consumed_units_total query_snapshot_lane_first_batch_ms
        query_snapshot_lane_first_batch_items query_snapshot_lane_remaining_items
        query_snapshot_lane_cursors_total]
        // Torii Connect (Iroha Connect) metrics
    [torii_connect_sessions_total torii_connect_sessions_active torii_pre_auth_reject_total
        torii_operator_auth_total torii_operator_auth_lockout_total torii_signature_limit_total
        torii_signature_limit_by_authority_total torii_signature_limit_last_count
        torii_signature_limit_max torii_nts_unhealthy_reject_total
        torii_multisig_direct_sign_reject_total torii_sorafs_admission_total
        torii_sorafs_capacity_telemetry_rejections_total torii_sorafs_capacity_declared_gib
        torii_sorafs_capacity_effective_gib torii_sorafs_capacity_utilised_gib
        torii_sorafs_capacity_outstanding_gib torii_sorafs_capacity_gibhours_total
        torii_sorafs_egress_bytes torii_sorafs_egress_drift_ratio
        sorafs_governance_dag_publish_total sorafs_governance_dag_published_bytes_total
        sorafs_governance_dag_last_publish_timestamp_seconds sorafs_governance_dag_backlog
        sorafs_governance_dag_head_age_seconds torii_sorafs_orderbook_finalized_events_total
        torii_sorafs_orderbook_open_depth_gib torii_sorafs_orderbook_matcher_lag_seconds
        torii_sorafs_orderbook_settlement_backlog
        torii_sorafs_orderbook_oldest_settlement_age_seconds
        torii_sorafs_orderbook_escrow_runway_seconds
        torii_sorafs_orderbook_finalized_projection_ready
        torii_sorafs_orderbook_finalized_projection_height
        torii_sorafs_orderbook_finalized_projection_timestamp_seconds
        torii_sorafs_orderbook_finalized_projection_failures_total
        torii_sorafs_orderbook_book_revision torii_sorafs_orderbook_matcher_scan_book_revision
        torii_sorafs_orderbook_api_requests_total]
    {
        for event in SORAFS_ORDERBOOK_EVENT_LABELS {
            let _ = torii_sorafs_orderbook_finalized_events_total.with_label_values(&[event]);
        }
        for tier in SORAFS_ORDERBOOK_TIER_LABELS {
            for side in SORAFS_ORDERBOOK_SIDE_LABELS {
                let _ = torii_sorafs_orderbook_open_depth_gib.with_label_values(&[tier, side]);
            }
        }
        for reason in SORAFS_ORDERBOOK_PROJECTION_FAILURE_LABELS {
            let _ = torii_sorafs_orderbook_finalized_projection_failures_total
                .with_label_values(&[reason]);
        }
        for route in SORAFS_ORDERBOOK_API_ROUTE_LABELS {
            for outcome in SORAFS_ORDERBOOK_API_OUTCOME_LABELS {
                let _ =
                    torii_sorafs_orderbook_api_requests_total.with_label_values(&[route, outcome]);
            }
        }
    }
    [torii_sorafs_gateway_compliance_requests_total
        torii_sorafs_gateway_compliance_serving_decisions_total
        torii_sorafs_gateway_compliance_failures_total
        torii_sorafs_gateway_compliance_serving_catalog_sequence
        torii_sorafs_gateway_compliance_serving_catalog_valid_until_seconds
        torii_sorafs_gateway_compliance_ready]
    {
        for operation in SORAFS_GATEWAY_COMPLIANCE_OPERATION_LABELS {
            for outcome in SORAFS_GATEWAY_COMPLIANCE_REQUEST_OUTCOME_LABELS {
                let _ = torii_sorafs_gateway_compliance_requests_total
                    .with_label_values(&[operation, outcome]);
            }
        }
        for subject_kind in SORAFS_GATEWAY_COMPLIANCE_SUBJECT_KIND_LABELS {
            for disposition in SORAFS_GATEWAY_COMPLIANCE_DISPOSITION_LABELS {
                for source in SORAFS_GATEWAY_COMPLIANCE_DECISION_SOURCE_LABELS {
                    let _ = torii_sorafs_gateway_compliance_serving_decisions_total
                        .with_label_values(&[subject_kind, disposition, source]);
                }
            }
        }
        for surface in SORAFS_GATEWAY_COMPLIANCE_FAILURE_SURFACE_LABELS {
            for class in SORAFS_GATEWAY_COMPLIANCE_FAILURE_CLASS_LABELS {
                let _ = torii_sorafs_gateway_compliance_failures_total
                    .with_label_values(&[surface, class]);
            }
        }
    }
    [torii_sorafs_hedging_xor_usd_reference_price_micro_usd
        torii_sorafs_hedging_feed_lag_seconds torii_sorafs_hedging_feed_divergence_bps
        torii_sorafs_hedging_exposure_drift_bps torii_sorafs_billing_statement_generation_total
        torii_sorafs_billing_statement_failure_total torii_sorafs_billing_statement_ack_backlog
        torii_sorafs_billing_escrow_runway_seconds torii_sorafs_reserve_lifecycle_stage_providers
        torii_sorafs_reserve_credit_draw_micro_xor torii_sorafs_reserve_credit_shortfall_micro_xor
        torii_sorafs_reserve_accrued_interest_micro_xor torii_sorafs_reserve_defaulted_providers
        torii_sorafs_reserve_appeal_backlog torii_sorafs_reserve_custody_movements
        torii_sorafs_reserve_chain_reconciled_movements
        torii_sorafs_reserve_finalized_projection_ready
        torii_sorafs_reserve_finalized_projection_height
        torii_sorafs_reserve_finalized_projection_failure_total
        torii_sorafs_reserve_service_requests_total torii_sorafs_reserve_service_rate_limit_total
        sorafs_reputation_ingest_lag_seconds sorafs_reputation_snapshot_age_seconds
        sorafs_reputation_snapshot_generated_at_unix sorafs_reputation_provider_count
        sorafs_reputation_low_score_providers sorafs_reputation_score
        sorafs_reputation_threshold_crossings_total sorafs_reputation_runtime_live
        sorafs_reputation_runtime_ready sorafs_reputation_runtime_dependencies_ready
        sorafs_reputation_journal_transaction_submitter_ready
        sorafs_reputation_runtime_finalized_height sorafs_reputation_runtime_consecutive_failures
        sorafs_reputation_runtime_material_acknowledged sorafs_reputation_runtime_provider_count
        sorafs_reputation_runtime_ticks_total sorafs_hedging_billing_runtime_live
        sorafs_hedging_billing_runtime_ready sorafs_hedging_billing_runtime_dependencies_ready
        sorafs_hedging_billing_automatic_execution_enabled sorafs_hedging_billing_last_tick_fresh
        sorafs_hedging_billing_finalized_projection_ready sorafs_hedging_billing_finalized_height
        sorafs_hedging_billing_finalized_head_height sorafs_hedging_billing_finalized_lag_blocks
        sorafs_hedging_billing_next_event_sequence sorafs_hedging_billing_ready_for_signing
        sorafs_hedging_billing_ready_for_publication sorafs_hedging_billing_publication_ambiguous
        sorafs_hedging_billing_published sorafs_hedging_billing_acknowledged
        sorafs_hedging_billing_dead_letter sorafs_hedging_billing_hedge_intents
        sorafs_hedging_billing_runtime_ticks_total torii_sorafs_fee_projection_nanos
        torii_sorafs_disputes_total torii_sorafs_orders_issued_total
        torii_sorafs_orders_completed_total torii_sorafs_orders_failed_total
        torii_sorafs_outstanding_orders torii_sorafs_uptime_bps torii_sorafs_por_bps
        torii_sorafs_por_challenges_total torii_sorafs_por_forced_challenges_total
        torii_sorafs_por_sampling_duplicates_total torii_sorafs_por_ingest_backlog
        torii_sorafs_por_ingest_failures_total torii_sorafs_repair_tasks_total
        torii_sorafs_repair_latency_minutes torii_sorafs_repair_queue_depth
        torii_sorafs_repair_backlog_oldest_age_seconds torii_sorafs_repair_lease_expired_total
        torii_sorafs_slash_proposals_total torii_sorafs_reconciliation_runs_total
        torii_sorafs_reconciliation_divergence_count torii_sorafs_gc_runs_total
        torii_sorafs_gc_evictions_total torii_sorafs_gc_bytes_freed_total
        torii_sorafs_gc_blocked_total torii_sorafs_gc_expired_manifests
        torii_sorafs_gc_oldest_expired_age_seconds torii_sorafs_storage_bytes_used
        torii_sorafs_storage_bytes_capacity sorafs_provider_ingest_inflight
        torii_sorafs_storage_fetch_inflight torii_sorafs_storage_fetch_bytes_per_sec
        torii_sorafs_storage_por_inflight torii_sorafs_storage_por_samples_success_total
        torii_sorafs_storage_por_samples_failed_total sorafs_gateway_active
        sorafs_gateway_responses_total sorafs_gateway_ttfb_ms
        sorafs_gateway_proof_verifications_total sorafs_gateway_proof_duration_ms
        torii_sorafs_chunk_range_requests_total torii_sorafs_chunk_range_bytes_total
        torii_sorafs_provider_range_capability_total torii_sorafs_routing_authority_cache_total
        torii_sorafs_range_fetch_throttle_events_total torii_sorafs_range_fetch_concurrency_current
        torii_sorafs_proof_stream_inflight torii_sorafs_proof_stream_events_total
        torii_sorafs_proof_stream_latency_ms torii_sorafs_proof_health_alerts_total
        torii_sorafs_proof_health_pdp_failures torii_sorafs_proof_health_potr_breaches
        torii_sorafs_proof_health_penalty_nano torii_sorafs_proof_health_window_end_epoch
        torii_sorafs_proof_health_cooldown torii_sorafs_gar_violations_total
        torii_sorafs_gateway_refusals_total torii_sorafs_gateway_fixture_info
        torii_sorafs_registry_manifests_total torii_sorafs_registry_aliases_total
        torii_sorafs_pin_retained_manifests torii_sorafs_pin_live_content_bytes
        torii_sorafs_alias_cache_refresh_total torii_sorafs_alias_cache_age_seconds
        torii_sorafs_tls_cert_expiry_seconds torii_sorafs_tls_renewal_total
        torii_sorafs_tls_ech_enabled torii_sorafs_gateway_fixture_version
        torii_sorafs_registry_orders_total torii_sorafs_replication_sla_total
        torii_sorafs_replication_backlog_total torii_sorafs_replication_completion_latency_epochs
        torii_sorafs_replication_deadline_slack_epochs soranet_privacy_circuit_events_total
        soranet_privacy_ingest_reject_total soranet_privacy_pow_rejects_total
        soranet_pow_revocation_store_total soranet_privacy_throttles_total
        soranet_privacy_verified_bytes_total soranet_privacy_active_circuits_avg
        soranet_privacy_active_circuits_max soranet_privacy_open_buckets
        soranet_privacy_pending_collectors soranet_privacy_snapshot_suppressed
        soranet_privacy_snapshot_suppressed_by_mode soranet_privacy_snapshot_drained
        soranet_privacy_snapshot_suppression_ratio soranet_privacy_evicted_buckets_total
        soranet_privacy_bucket_suppressed soranet_privacy_suppression_total
        soranet_privacy_latest_bucket_start_unixtime soranet_privacy_rtt_millis
        soranet_privacy_gar_reports_total
        soranet_privacy_last_poll_unixtime soranet_privacy_poll_errors_total
        soranet_privacy_collector_enabled sorafs_orchestrator_active_fetches
        sorafs_orchestrator_fetch_duration_ms sorafs_orchestrator_fetch_failures_total
        sorafs_orchestrator_retries_total sorafs_orchestrator_provider_failures_total
        sorafs_orchestrator_chunk_latency_ms sorafs_orchestrator_bytes_total
        sorafs_orchestrator_stalls_total sorafs_orchestrator_transport_events_total
        sorafs_orchestrator_policy_events_total sorafs_orchestrator_pq_ratio
        sorafs_orchestrator_pq_candidate_ratio sorafs_orchestrator_pq_deficit_ratio
        sorafs_orchestrator_classical_ratio sorafs_orchestrator_classical_selected
        torii_da_rent_gib_months_total torii_da_rent_base_micro_total
        torii_da_protocol_reserve_micro_total torii_da_provider_reward_micro_total
        torii_da_pdp_bonus_micro_total torii_da_potr_bonus_micro_total torii_da_receipts_total
        torii_da_receipt_epoch torii_da_receipt_highest_sequence torii_da_chunking_seconds
        torii_da_spool_batches_total torii_da_spool_artifacts_total torii_da_spool_queue_depth
        torii_da_spool_batch_write_ms da_shard_cursor_events_total da_shard_cursor_height
        da_shard_cursor_lag_blocks taikai_ingest_segment_latency_ms
        taikai_ingest_live_edge_drift_ms taikai_ingest_live_edge_drift_signed_ms
        taikai_ingest_errors_total taikai_trm_alias_rotations_total
        taikai_viewer_rebuffer_events_total taikai_viewer_playback_segments_total
        taikai_viewer_cek_fetch_duration_ms taikai_viewer_pq_circuit_health
        taikai_viewer_cek_rotation_seconds_ago taikai_viewer_alerts_firing_total
        sorafs_orchestrator_brownouts_total soranet_reward_base_payout_nanos]
    {
        soranet_reward_base_payout_nanos.set(0);
    }
    [soranet_reward_events_total soranet_reward_payout_nanos_total soranet_reward_skips_total
        soranet_reward_adjustment_nanos_total soranet_reward_disputes_total
        torii_http_requests_total torii_http_request_duration_seconds
        torii_http_request_bytes_total torii_http_response_bytes_total torii_api_token_hits_total
        torii_content_requests_total torii_content_request_duration_seconds
        torii_content_response_bytes_total torii_proof_requests_total
        torii_proof_request_duration_seconds torii_proof_response_bytes_total
        torii_proof_cache_hits_total torii_request_duration_seconds torii_request_failures_total
        torii_explorer_requests_total torii_explorer_request_duration_seconds
        torii_norito_rpc_gate_total torii_address_invalid_total torii_account_literal_total
        torii_norito_decode_failures_total torii_proof_throttled_total
        torii_contract_throttled_total torii_contract_errors_total sns_registrar_status_total
        torii_active_connections_total torii_connect_buffered_sessions
        torii_connect_total_buffer_bytes torii_connect_dedupe_size torii_connect_per_ip_sessions
        zk_verify_latency_ms zk_verify_proof_bytes]
        // Block-level gas and fees (latest block)
    [block_gas_used confidential_gas_tx_used confidential_gas_block_used confidential_gas_total
        block_fee_total_units block_fee_total_scale]
        // Network Time Service (basic gauges)
    [nts_offset_ms nts_confidence_ms nts_peers_sampled nts_samples_used nts_healthy nts_fallback
        nts_min_samples_ok nts_offset_ok nts_confidence_ok nts_rtt_ms_bucket nts_rtt_ms_sum
        nts_rtt_ms_count]
        // BLS signature verification counters per latest block
    [pipeline_sig_bls_agg_same pipeline_sig_bls_agg_multi pipeline_sig_bls_deterministic
        pipeline_sig_bls_agg_same_total pipeline_sig_bls_agg_multi_total]
}
suffix {
    metrics.finish();

    // These postdate the sealed v2 catalog. Register them directly so the
    // catalog's construction-order and hash invariants remain unchanged.
    let kaigi_relay_manifest_updates_by_domain_total = IntCounterVec::new(
        Opts::new(
            "kaigi_relay_manifest_updates_by_domain_total",
            "Kaigi relay manifest updates grouped only by domain for bounded diagnostics",
        ),
        &["domain"],
    )
    .expect("Infallible");
    let kaigi_relay_failovers_by_domain_total = IntCounterVec::new(
        Opts::new(
            "kaigi_relay_failovers_by_domain_total",
            "Kaigi relay failovers grouped only by domain for bounded diagnostics",
        ),
        &["domain"],
    )
    .expect("Infallible");
    let kaigi_relay_health_reports_by_domain_total = IntCounterVec::new(
        Opts::new(
            "kaigi_relay_health_reports_by_domain_total",
            "Kaigi relay health reports grouped only by domain for bounded diagnostics",
        ),
        &["domain"],
    )
    .expect("Infallible");
    for metric in [
        &kaigi_relay_manifest_updates_by_domain_total,
        &kaigi_relay_failovers_by_domain_total,
        &kaigi_relay_health_reports_by_domain_total,
    ] {
        register_guarded(&registry, metric);
    }
}
initialize (metrics) {
    [txs block_height block_height_non_empty last_commit_time_ms last_block_committed_at_ms
        last_non_empty_block_committed_at_ms commit_time_ms slot_duration_ms
        slot_duration_ms_latest da_quorum_ratio connected_peers p2p_peer_churn_total
        uptime_since_genesis_ms domains accounts tx_amounts isi isi_times view_changes queue_size
        queue_queued queue_inflight kura_fsync_enabled kura_fsync_failures_total
        kura_fsync_latency_ms sm_syscall_total sm_syscall_failures_total sm_openssl_preview
        zk_halo2_enabled zk_halo2_curve_id zk_halo2_backend_id zk_halo2_max_k
        zk_halo2_verifier_budget_ms zk_halo2_verifier_max_batch zk_halo2_verifier_worker_threads
        zk_halo2_verifier_queue_cap zk_lane_enqueue_wait_total zk_lane_enqueue_timeout_total
        zk_lane_drop_total zk_lane_retry_enqueued_total zk_lane_retry_replayed_total
        zk_lane_retry_exhausted_total zk_lane_pending_depth zk_lane_retry_ring_depth
        zk_verifier_cache_events_total confidential_gas_base_verify
        confidential_gas_per_public_input confidential_gas_per_proof_byte
        confidential_gas_per_nullifier confidential_gas_per_commitment ivm_gas_schedule_hash_lo
        ivm_gas_schedule_hash_hi ivm_stack_bytes ivm_stack_clamped ivm_stack_gas_multiplier
        ivm_stack_pool_fallback_total ivm_stack_budget_hit_total confidential_tree_commitments
        confidential_tree_depth confidential_root_history_entries confidential_frontier_checkpoints
        confidential_frontier_last_height confidential_frontier_last_commitments
        confidential_root_evictions_total confidential_frontier_evictions_total
        oracle_price_local_per_xor oracle_twap_window_seconds oracle_haircut_basis_points
        oracle_staleness_seconds oracle_observations_total oracle_aggregation_duration_ms
        oracle_rewards_total oracle_penalties_total oracle_feed_events_total
        oracle_feed_events_with_evidence_total oracle_evidence_hashes_total
        fastpq_execution_mode_total fastpq_poseidon_pipeline_total fastpq_gpu_disable_total
        fastpq_gpu_parity_failure_total fastpq_proof_sidecar_queue_depth
        fastpq_proof_sidecar_events_total fastpq_metal_queue_ratio fastpq_metal_queue_depth
        fastpq_zero_fill_duration_ms fastpq_zero_fill_bandwidth_gbps settlement_events_total
        settlement_finality_events_total settlement_fx_window_ms settlement_buffer_xor
        settlement_buffer_capacity_xor settlement_buffer_status settlement_pnl_xor
        settlement_haircut_bp settlement_swapline_utilisation settlement_conversion_total
        settlement_haircut_total subscription_billing_attempts_total
        subscription_billing_outcomes_total social_events_total social_budget_spent
        social_campaign_spent social_campaign_cap social_campaign_remaining social_campaign_active
        social_halted social_rejections_total multisig_direct_sign_reject_total social_open_escrows
        sumeragi_tx_queue_depth sumeragi_tx_queue_capacity sumeragi_tx_queue_retained_bytes
        sumeragi_tx_queue_max_retained_bytes sumeragi_tx_queue_saturated
        sumeragi_tx_queue_saturated_by_count sumeragi_tx_queue_saturated_by_bytes
        sumeragi_tx_queue_saturated_by_age sumeragi_tx_queue_oldest_queued_age_ms
        sumeragi_pending_blocks_total sumeragi_pending_blocks_blocking
        sumeragi_commit_inflight_queue_depth sumeragi_missing_block_requests
        sumeragi_missing_block_oldest_ms sumeragi_missing_block_retry_window_ms
        sumeragi_missing_block_dwell_ms sumeragi_epoch_length_blocks
        sumeragi_epoch_commit_deadline_offset sumeragi_epoch_reveal_deadline_offset
        state_tiered_hot_entries state_tiered_hot_bytes state_tiered_cold_entries
        state_tiered_cold_bytes state_tiered_cold_reused_entries state_tiered_cold_reused_bytes
        state_tiered_hot_promotions state_tiered_hot_demotions state_tiered_hot_budget_overflow_keys
        state_tiered_hot_budget_overflow_bytes state_tiered_last_snapshot_index
        storage_budget_bytes_used storage_budget_bytes_limit storage_budget_exceeded_total
        storage_da_cache_total storage_da_churn_bytes_total governance_proposals_status
        governance_parliament_transitions_total governance_parliament_no_result_total
        governance_parliament_attempts_by_status governance_parliament_attempts_by_stage
        governance_council_members governance_council_alternates governance_council_candidates
        governance_council_epoch governance_citizens_total governance_citizen_service_events_total
        governance_protected_namespace_total governance_manifest_admission_total
        governance_manifest_quorum_total governance_manifest_hook_total
        governance_manifest_activations_total governance_bond_events_total
        governance_manifest_recent taikai_ingest_snapshots taikai_ingest_snapshot_order
        da_receipt_metric_lanes recent_rejection_events last_rejection_at_ms
        taikai_alias_rotation_snapshots taikai_alias_rotation_snapshot_order
        alias_usage_total iso_reference_status
        iso_reference_age_seconds iso_reference_records iso_reference_refresh_interval_secs
        fraud_psp_assessments_total fraud_psp_missing_assessment_total
        fraud_psp_invalid_metadata_total fraud_psp_attestation_total fraud_psp_latency_ms
        fraud_psp_score_bps fraud_psp_outcome_mismatch_total streaming_hpke_rekeys_total
        streaming_gck_rotations_total streaming_quic_datagrams_sent_total
        streaming_quic_datagrams_dropped_total streaming_fec_parity_current
        streaming_feedback_timeout_total telemetry_redaction_total
        telemetry_redaction_skipped_total telemetry_truncation_total
        streaming_privacy_redaction_fail_total streaming_encode_latency_ms
        streaming_encode_audio_jitter_ms streaming_encode_audio_max_jitter_ms
        streaming_encode_dropped_layers_total streaming_decode_buffer_ms
        streaming_decode_dropped_frames_total streaming_decode_max_queue_ms
        streaming_decode_av_drift_ms streaming_decode_max_drift_ms streaming_audio_jitter_ms
        streaming_audio_max_jitter_ms streaming_av_drift_ms streaming_av_max_drift_ms
        streaming_av_drift_ewma_ms streaming_av_sync_window_ms streaming_av_sync_violation_total
        streaming_network_rtt_ms streaming_network_loss_percent_x100
        streaming_network_fec_repairs_total streaming_network_fec_failures_total
        streaming_network_datagram_reinjects_total streaming_energy_encoder_mw
        streaming_energy_decoder_mw nexus_audit_outcome_total nexus_audit_outcome_last_timestamp
        nexus_space_directory_revision_total nexus_space_directory_active_manifests
        nexus_space_directory_revocations_total kaigi_relay_registered_total
        kaigi_relay_registration_bandwidth kaigi_relay_manifest_updates_total
        kaigi_relay_manifest_updates_by_domain_total kaigi_relay_manifest_hop_count
        kaigi_relay_failover_total kaigi_relay_failovers_by_domain_total
        kaigi_relay_failover_hop_count kaigi_relay_health_reports_total
        kaigi_relay_health_reports_by_domain_total kaigi_relay_health_state dropped_messages
        sumeragi_dropped_block_messages_total sumeragi_dropped_control_messages_total
        p2p_dropped_posts p2p_dropped_broadcasts p2p_subscriber_queue_full_total
        p2p_subscriber_queue_full_by_topic_total p2p_subscriber_unrouted_total
        p2p_subscriber_unrouted_by_topic_total p2p_handshake_failures p2p_low_post_throttled_total
        p2p_low_broadcast_throttled_total p2p_post_overflow_total p2p_post_overflow_by_topic
        consensus_ingress_drop_total p2p_dns_refresh_total p2p_dns_ttl_refresh_total
        p2p_dns_resolution_fail_total p2p_dns_reconnect_success_total p2p_backoff_scheduled_total
        p2p_deferred_send_enqueued_total p2p_deferred_send_dropped_total
        p2p_session_reconnect_total p2p_connect_retry_seconds p2p_accept_throttled_total
        p2p_accept_bucket_evictions_total p2p_accept_buckets_current p2p_accept_prefix_cache_total
        p2p_accept_throttle_decisions_total p2p_incoming_cap_reject_total
        p2p_total_cap_reject_total p2p_preauth_source_cap_reject_total p2p_trust_score
        p2p_trust_penalties_total
        p2p_trust_decay_ticks_total p2p_trust_gossip_skipped_total tx_gossip_sent_total
        tx_gossip_dropped_total tx_gossip_targets tx_gossip_fallback_total
        tx_gossip_frame_cap_bytes tx_gossip_public_target_cap tx_gossip_restricted_target_cap
        tx_gossip_public_target_reshuffle_ms tx_gossip_restricted_target_reshuffle_ms
        tx_gossip_drop_unknown_dataspace tx_gossip_restricted_fallback
        tx_gossip_restricted_public_policy tx_gossip_status tx_gossip_caps p2p_scion_inbound_total
        p2p_scion_outbound_total p2p_queue_depth
        p2p_queue_dropped_total p2p_handshake_ms_bucket p2p_handshake_ms_sum p2p_handshake_ms_count
        p2p_handshake_error_total p2p_frame_cap_violations_total runtime_upgrade_events_total
        runtime_upgrade_provenance_rejections_total runtime_abi_version sumeragi_tail_votes_total
        sumeragi_votes_sent_total sumeragi_votes_received_total sumeragi_qc_sent_total
        sumeragi_qc_received_total sumeragi_qc_validation_errors_total
        sumeragi_validation_reject_total sumeragi_validation_reject_last_reason
        sumeragi_validation_reject_last_height sumeragi_validation_reject_last_view
        sumeragi_validation_reject_last_timestamp_ms sumeragi_block_sync_share_blocks_unsolicited_total
        sumeragi_consensus_message_handling_total sumeragi_view_change_cause_total
        sumeragi_view_change_cause_last_timestamp_ms sumeragi_qc_signer_counts
        sumeragi_invalid_signature_total sumeragi_widen_before_rotate_total
        sumeragi_view_change_suggest_total sumeragi_view_change_install_total
        sumeragi_proposal_gap_total sumeragi_view_change_proof_total sumeragi_wa_qc_assembled_total
        sumeragi_cert_size sumeragi_commit_signatures_present sumeragi_commit_signatures_counted
        sumeragi_commit_signatures_set_b sumeragi_commit_signatures_required
        sumeragi_commit_qc_height sumeragi_commit_qc_view sumeragi_commit_qc_epoch
        sumeragi_commit_qc_signatures_total sumeragi_commit_qc_validator_set_len
        sumeragi_gossip_fallback_total sumeragi_block_created_dropped_by_lock_total
        sumeragi_block_created_hint_mismatch_total sumeragi_block_created_proposal_mismatch_total
        lane_relay_invalid_total lane_relay_emergency_override_total sumeragi_prf_epoch_seed_hex
        halo2_status sumeragi_prf_height sumeragi_prf_view sumeragi_membership_view_hash
        sumeragi_membership_height sumeragi_membership_view sumeragi_membership_epoch
        sumeragi_mode_tag sumeragi_leader_index sumeragi_highest_qc_height
        sumeragi_locked_qc_height sumeragi_locked_qc_view sumeragi_new_view_receipts_by_hv
        sumeragi_new_view_publish_total sumeragi_new_view_recv_total
        sumeragi_new_view_dropped_by_lock_total sumeragi_commit_conflict_detected_total
        sumeragi_missing_block_fetch_total sumeragi_missing_block_fetch_target_total
        sumeragi_missing_block_fetch_dwell_ms sumeragi_missing_block_fetch_targets
        blocksync_qc_quarantine_total blocksync_qc_revalidated_total blocksync_qc_final_drop_total
        qc_deferred_missing_payload_total qc_deferred_resolved_total qc_deferred_expired_total
        consensus_empty_commit_topology_defer_total
        consensus_empty_commit_topology_escalation_total consensus_recovery_state_transitions_total
        consensus_missing_block_height_escalation_total consensus_sidecar_quarantine_total
        consensus_sidecar_final_drop_total blocksync_range_pull_escalation_total
        blocksync_range_pull_success_total blocksync_range_pull_failure_total
        consensus_recovery_stuck_round_seconds sumeragi_da_gate_block_total
        sumeragi_da_gate_last_reason sumeragi_da_gate_last_satisfied
        sumeragi_da_gate_satisfied_total sumeragi_da_manifest_guard_total
        sumeragi_da_manifest_cache_total sumeragi_da_spool_cache_total
        sumeragi_da_pin_intent_spool_total sumeragi_da_votes_ingested_total
        sumeragi_qc_assembly_latency_ms sumeragi_qc_last_latency_ms
        sumeragi_kura_store_failures_total
        sumeragi_kura_store_last_retry_attempt sumeragi_kura_store_last_retry_backoff_ms
        sumeragi_pacemaker_backpressure_deferrals_total
        sumeragi_pacemaker_backpressure_deferrals_by_reason_total
        sumeragi_pacemaker_backpressure_deferral_duration_ms
        sumeragi_pacemaker_backpressure_deferral_active
        sumeragi_pacemaker_backpressure_deferral_age_ms sumeragi_pacemaker_eval_ms
        sumeragi_pacemaker_propose_ms sumeragi_commit_stage_ms state_commit_write_lock_wait_ms
        state_commit_write_lock_hold_ms sumeragi_commit_pipeline_tick_total
        sumeragi_prevote_timeout_total sumeragi_membership_mismatch_total
        sumeragi_membership_mismatch_active sumeragi_post_to_peer_total
        sumeragi_bg_post_enqueued_total sumeragi_bg_post_overflow_total sumeragi_bg_post_drop_total
        sumeragi_bg_post_queue_depth sumeragi_bg_post_queue_depth_by_peer sumeragi_bg_post_age_ms
        sumeragi_pacemaker_backoff_ms sumeragi_pacemaker_rtt_floor_ms
        sumeragi_pacemaker_backoff_multiplier sumeragi_pacemaker_rtt_floor_multiplier
        sumeragi_pacemaker_max_backoff_ms sumeragi_pacemaker_jitter_ms
        sumeragi_pacemaker_jitter_frac_permille sumeragi_pacemaker_round_elapsed_ms
        sumeragi_pacemaker_view_timeout_target_ms sumeragi_pacemaker_view_timeout_remaining_ms
        sumeragi_phase_latency_ms sumeragi_phase_latency_ema_ms sumeragi_phase_total_ema_ms
        ivm_cache_hits ivm_cache_misses ivm_cache_evictions ivm_cache_decoded_streams
        ivm_cache_decoded_ops_total ivm_cache_decode_failures ivm_cache_decode_time_ns_total
        ivm_register_max_index ivm_register_unique_count merkle_root_gpu_total
        merkle_root_cpu_total ivm_memory_commit_ms ivm_memory_commit_dirty_chunks
        ivm_merkle_rebuild_total ivm_merkle_incremental_leaf_updates_total pipeline_dag_vertices
        pipeline_dag_edges pipeline_conflict_rate_bps pipeline_access_set_source_total
        pipeline_comp_count pipeline_comp_max pipeline_comp_hist_bucket pipeline_peak_layer_width
        pipeline_layer_avg_width pipeline_layer_median_width nexus_config_diff_total
        nexus_lane_configured_total nexus_lane_governance_sealed nexus_lane_governance_sealed_total
        nexus_lane_governance_sealed_aliases nexus_lane_lifecycle_applied_total
        nexus_lane_block_height nexus_lane_finality_lag_slots nexus_lane_settlement_backlog_xor
        nexus_public_lane_validator_total nexus_public_lane_validator_activation_total
        nexus_public_lane_validator_reject_total nexus_public_lane_stake_bonded
        nexus_public_lane_unbond_pending nexus_public_lane_reward_total
        nexus_public_lane_slash_total nexus_scheduler_lane_teu_capacity
        nexus_scheduler_lane_teu_slot_committed nexus_scheduler_lane_trigger_level
        nexus_scheduler_starvation_bound_slots nexus_scheduler_lane_teu_slot_breakdown
        nexus_scheduler_lane_teu_deferral_total nexus_scheduler_lane_headroom_events_total
        nexus_scheduler_must_serve_truncations_total nexus_scheduler_lane_teu_status
        nexus_scheduler_dataspace_teu_backlog nexus_scheduler_dataspace_age_slots
        nexus_scheduler_dataspace_virtual_finish nexus_scheduler_dataspace_teu_status
        pipeline_layer_count pipeline_scheduler_utilization_pct pipeline_layer_width_hist_bucket
        pipeline_overlay_count pipeline_overlay_instructions pipeline_overlay_bytes
        pipeline_quarantine_classified pipeline_quarantine_overflow pipeline_quarantine_executed
        pipeline_stage_ms amx_prepare_ms amx_commit_ms amx_abort_total axt_policy_reject_total
        axt_policy_snapshot_version axt_policy_snapshot_cache_events_total
        axt_proof_cache_events_total axt_proof_cache_state ivm_exec_ms pipeline_detached_prepared
        pipeline_detached_merged pipeline_detached_fallback pipeline_detached_fallback_reason
        merge_ledger_entries_total merge_ledger_latest_epoch merge_ledger_latest_root_hex
        pipeline_sig_bls_agg_same pipeline_sig_bls_agg_multi pipeline_sig_bls_deterministic
        pipeline_sig_bls_agg_same_total pipeline_sig_bls_agg_multi_total block_gas_used
        confidential_gas_tx_used confidential_gas_block_used confidential_gas_total
        block_fee_total_units block_fee_total_scale torii_filter_depth torii_filter_match_count
        torii_scan_ms torii_stream_rows torii_lane_admission_latency_seconds
        torii_route_stage_latency_seconds torii_attachment_reject_total
        torii_attachment_sanitize_ms torii_zk_prover_attachment_bytes torii_zk_prover_latency_ms
        torii_zk_prover_gc_total torii_zk_prover_inflight torii_zk_prover_pending
        torii_zk_ivm_prove_inflight torii_zk_ivm_prove_queued torii_zk_prover_last_scan_bytes
        torii_zk_prover_last_scan_ms torii_zk_prover_budget_exhausted_total
        torii_query_snapshot_requests torii_query_snapshot_first_batch_ms
        torii_query_snapshot_gas_consumed_units_total query_snapshot_lane_first_batch_ms
        query_snapshot_lane_first_batch_items query_snapshot_lane_remaining_items
        query_snapshot_lane_cursors_total torii_connect_sessions_total
        torii_connect_sessions_active torii_pre_auth_reject_total torii_operator_auth_total
        torii_operator_auth_lockout_total torii_signature_limit_total
        torii_signature_limit_by_authority_total torii_signature_limit_last_count
        torii_signature_limit_max torii_nts_unhealthy_reject_total
        torii_multisig_direct_sign_reject_total torii_sorafs_admission_total
        torii_sorafs_capacity_telemetry_rejections_total torii_sorafs_capacity_declared_gib
        torii_sorafs_capacity_effective_gib torii_sorafs_capacity_utilised_gib
        torii_sorafs_capacity_outstanding_gib torii_sorafs_capacity_gibhours_total
        torii_sorafs_egress_bytes torii_sorafs_egress_drift_ratio
        sorafs_governance_dag_publish_total sorafs_governance_dag_published_bytes_total
        sorafs_governance_dag_last_publish_timestamp_seconds sorafs_governance_dag_backlog
        sorafs_governance_dag_head_age_seconds torii_sorafs_orderbook_finalized_events_total
        torii_sorafs_orderbook_open_depth_gib torii_sorafs_orderbook_matcher_lag_seconds
        torii_sorafs_orderbook_settlement_backlog
        torii_sorafs_orderbook_oldest_settlement_age_seconds
        torii_sorafs_orderbook_escrow_runway_seconds
        torii_sorafs_orderbook_finalized_projection_ready
        torii_sorafs_orderbook_finalized_projection_height
        torii_sorafs_orderbook_finalized_projection_timestamp_seconds
        torii_sorafs_orderbook_finalized_projection_failures_total
        torii_sorafs_orderbook_book_revision torii_sorafs_orderbook_matcher_scan_book_revision
        torii_sorafs_orderbook_api_requests_total torii_sorafs_gateway_compliance_requests_total
        torii_sorafs_gateway_compliance_serving_decisions_total
        torii_sorafs_gateway_compliance_failures_total
        torii_sorafs_gateway_compliance_serving_catalog_sequence
        torii_sorafs_gateway_compliance_serving_catalog_valid_until_seconds
        torii_sorafs_gateway_compliance_ready
        torii_sorafs_hedging_xor_usd_reference_price_micro_usd
        torii_sorafs_hedging_feed_lag_seconds torii_sorafs_hedging_feed_divergence_bps
        torii_sorafs_hedging_exposure_drift_bps torii_sorafs_billing_statement_generation_total
        torii_sorafs_billing_statement_failure_total torii_sorafs_billing_statement_ack_backlog
        torii_sorafs_billing_escrow_runway_seconds torii_sorafs_reserve_lifecycle_stage_providers
        torii_sorafs_reserve_credit_draw_micro_xor torii_sorafs_reserve_credit_shortfall_micro_xor
        torii_sorafs_reserve_accrued_interest_micro_xor torii_sorafs_reserve_defaulted_providers
        torii_sorafs_reserve_appeal_backlog torii_sorafs_reserve_custody_movements
        torii_sorafs_reserve_chain_reconciled_movements
        torii_sorafs_reserve_finalized_projection_ready
        torii_sorafs_reserve_finalized_projection_height
        torii_sorafs_reserve_finalized_projection_failure_total
        torii_sorafs_reserve_service_requests_total torii_sorafs_reserve_service_rate_limit_total
        sorafs_reputation_ingest_lag_seconds sorafs_reputation_snapshot_age_seconds
        sorafs_reputation_snapshot_generated_at_unix sorafs_reputation_provider_count
        sorafs_reputation_low_score_providers sorafs_reputation_score
        sorafs_reputation_threshold_crossings_total sorafs_reputation_runtime_live
        sorafs_reputation_runtime_ready sorafs_reputation_runtime_dependencies_ready
        sorafs_reputation_journal_transaction_submitter_ready
        sorafs_reputation_runtime_finalized_height sorafs_reputation_runtime_consecutive_failures
        sorafs_reputation_runtime_material_acknowledged sorafs_reputation_runtime_provider_count
        sorafs_reputation_runtime_ticks_total sorafs_hedging_billing_runtime_live
        sorafs_hedging_billing_runtime_ready sorafs_hedging_billing_runtime_dependencies_ready
        sorafs_hedging_billing_automatic_execution_enabled sorafs_hedging_billing_last_tick_fresh
        sorafs_hedging_billing_finalized_projection_ready sorafs_hedging_billing_finalized_height
        sorafs_hedging_billing_finalized_head_height sorafs_hedging_billing_finalized_lag_blocks
        sorafs_hedging_billing_next_event_sequence sorafs_hedging_billing_ready_for_signing
        sorafs_hedging_billing_ready_for_publication sorafs_hedging_billing_publication_ambiguous
        sorafs_hedging_billing_published sorafs_hedging_billing_acknowledged
        sorafs_hedging_billing_dead_letter sorafs_hedging_billing_hedge_intents
        sorafs_hedging_billing_runtime_ticks_total]
    sorafs_reputation_score_tracked_providers = Arc::new(RwLock::new(BTreeSet::new()));
    sorafs_reputation_low_score_state = Arc::new(RwLock::new(BTreeMap::new()));
    [torii_sorafs_fee_projection_nanos torii_sorafs_disputes_total
        torii_sorafs_orders_issued_total torii_sorafs_orders_completed_total
        torii_sorafs_orders_failed_total torii_sorafs_outstanding_orders torii_sorafs_uptime_bps
        torii_sorafs_por_bps torii_sorafs_por_challenges_total
        torii_sorafs_por_forced_challenges_total torii_sorafs_por_sampling_duplicates_total
        torii_sorafs_por_ingest_backlog torii_sorafs_por_ingest_failures_total
        torii_sorafs_repair_tasks_total torii_sorafs_repair_latency_minutes
        torii_sorafs_repair_queue_depth torii_sorafs_repair_backlog_oldest_age_seconds
        torii_sorafs_repair_lease_expired_total torii_sorafs_slash_proposals_total
        torii_sorafs_reconciliation_runs_total torii_sorafs_reconciliation_divergence_count
        torii_sorafs_gc_runs_total torii_sorafs_gc_evictions_total
        torii_sorafs_gc_bytes_freed_total torii_sorafs_gc_blocked_total
        torii_sorafs_gc_expired_manifests torii_sorafs_gc_oldest_expired_age_seconds
        torii_sorafs_storage_bytes_used torii_sorafs_storage_bytes_capacity
        sorafs_provider_ingest_inflight torii_sorafs_storage_fetch_inflight
        torii_sorafs_storage_fetch_bytes_per_sec torii_sorafs_storage_por_inflight
        torii_sorafs_storage_por_samples_success_total
        torii_sorafs_storage_por_samples_failed_total sorafs_gateway_active
        sorafs_gateway_responses_total sorafs_gateway_ttfb_ms
        sorafs_gateway_proof_verifications_total sorafs_gateway_proof_duration_ms
        torii_sorafs_chunk_range_requests_total torii_sorafs_chunk_range_bytes_total
        torii_sorafs_provider_range_capability_total torii_sorafs_routing_authority_cache_total
        torii_sorafs_range_fetch_throttle_events_total torii_sorafs_range_fetch_concurrency_current
        torii_sorafs_proof_stream_inflight torii_sorafs_proof_stream_events_total
        torii_sorafs_proof_stream_latency_ms torii_sorafs_proof_health_alerts_total
        torii_sorafs_proof_health_pdp_failures torii_sorafs_proof_health_potr_breaches
        torii_sorafs_proof_health_penalty_nano torii_sorafs_proof_health_window_end_epoch
        torii_sorafs_proof_health_cooldown torii_sorafs_gar_violations_total
        torii_sorafs_gateway_refusals_total torii_sorafs_gateway_fixture_info
        torii_sorafs_registry_manifests_total torii_sorafs_registry_aliases_total
        torii_sorafs_pin_retained_manifests torii_sorafs_pin_live_content_bytes
        torii_sorafs_alias_cache_refresh_total torii_sorafs_alias_cache_age_seconds
        torii_sorafs_tls_cert_expiry_seconds torii_sorafs_tls_renewal_total
        torii_sorafs_tls_ech_enabled torii_sorafs_gateway_fixture_version
        torii_sorafs_registry_orders_total torii_sorafs_replication_sla_total
        torii_sorafs_replication_backlog_total torii_sorafs_replication_completion_latency_epochs
        torii_sorafs_replication_deadline_slack_epochs soranet_privacy_ingest_reject_total
        soranet_privacy_circuit_events_total soranet_privacy_pow_rejects_total
        soranet_pow_revocation_store_total soranet_privacy_throttles_total
        soranet_privacy_verified_bytes_total soranet_privacy_active_circuits_avg
        soranet_privacy_active_circuits_max soranet_privacy_open_buckets
        soranet_privacy_pending_collectors soranet_privacy_snapshot_suppressed
        soranet_privacy_snapshot_suppressed_by_mode soranet_privacy_snapshot_drained
        soranet_privacy_snapshot_suppression_ratio soranet_privacy_evicted_buckets_total
        soranet_privacy_bucket_suppressed soranet_privacy_suppression_total
        soranet_privacy_latest_bucket_start_unixtime soranet_privacy_rtt_millis
        soranet_privacy_gar_reports_total
        soranet_privacy_last_poll_unixtime soranet_privacy_poll_errors_total
        soranet_privacy_collector_enabled sorafs_orchestrator_active_fetches
        sorafs_orchestrator_fetch_duration_ms sorafs_orchestrator_fetch_failures_total
        sorafs_orchestrator_retries_total sorafs_orchestrator_provider_failures_total
        sorafs_orchestrator_chunk_latency_ms sorafs_orchestrator_bytes_total
        sorafs_orchestrator_stalls_total sorafs_orchestrator_transport_events_total
        sorafs_orchestrator_policy_events_total sorafs_orchestrator_pq_ratio
        sorafs_orchestrator_pq_candidate_ratio sorafs_orchestrator_pq_deficit_ratio
        sorafs_orchestrator_classical_ratio sorafs_orchestrator_classical_selected
        torii_da_rent_gib_months_total torii_da_rent_base_micro_total
        torii_da_protocol_reserve_micro_total torii_da_provider_reward_micro_total
        torii_da_pdp_bonus_micro_total torii_da_potr_bonus_micro_total torii_da_receipts_total
        torii_da_receipt_epoch torii_da_receipt_highest_sequence torii_da_chunking_seconds
        torii_da_spool_batches_total torii_da_spool_artifacts_total torii_da_spool_queue_depth
        torii_da_spool_batch_write_ms da_shard_cursor_events_total da_shard_cursor_height
        da_shard_cursor_lag_blocks taikai_ingest_segment_latency_ms
        taikai_ingest_live_edge_drift_ms taikai_ingest_live_edge_drift_signed_ms
        taikai_ingest_errors_total taikai_trm_alias_rotations_total
        taikai_viewer_rebuffer_events_total taikai_viewer_playback_segments_total
        taikai_viewer_cek_fetch_duration_ms taikai_viewer_pq_circuit_health
        taikai_viewer_cek_rotation_seconds_ago taikai_viewer_alerts_firing_total
        sorafs_orchestrator_brownouts_total soranet_reward_base_payout_nanos
        soranet_reward_events_total soranet_reward_payout_nanos_total soranet_reward_skips_total
        soranet_reward_adjustment_nanos_total soranet_reward_disputes_total
        torii_http_requests_total torii_http_request_duration_seconds
        torii_http_request_bytes_total torii_http_response_bytes_total torii_api_token_hits_total
        torii_content_requests_total torii_content_request_duration_seconds
        torii_content_response_bytes_total torii_proof_requests_total
        torii_proof_request_duration_seconds torii_proof_response_bytes_total
        torii_proof_cache_hits_total torii_request_duration_seconds torii_request_failures_total
        torii_explorer_requests_total torii_explorer_request_duration_seconds
        torii_norito_rpc_gate_total torii_address_invalid_total torii_account_literal_total
        torii_norito_decode_failures_total torii_proof_throttled_total
        torii_contract_throttled_total torii_contract_errors_total sns_registrar_status_total
        torii_active_connections_total torii_connect_buffered_sessions
        torii_connect_total_buffer_bytes torii_connect_dedupe_size torii_connect_per_ip_sessions
        zk_verify_latency_ms zk_verify_proof_bytes nts_offset_ms nts_confidence_ms
        nts_peers_sampled nts_samples_used nts_healthy nts_fallback nts_min_samples_ok
        nts_offset_ok nts_confidence_ok nts_rtt_ms_bucket nts_rtt_ms_sum nts_rtt_ms_count]
    sorafs_orderbook_projection_exposition_lock = Mutex::new(());
    sorafs_gateway_compliance_exposition_lock = Mutex::new(());
    [musubi registry]
}
epilogue {
    metrics.apply_stack_snapshot(&stack_settings_snapshot());
    metrics
}
}
const METRIC_CATALOG_V2: &str = include_str!("metrics/catalog_v2.tsv");
const METRIC_CATALOG_V2_HEADER: &str = "# iroha-telemetry-metric-catalog-v2";
const METRIC_CATALOG_V2_ROWS: usize = 801;
const METRIC_CATALOG_V2_REGISTERED: usize = 756;
const METRIC_CATALOG_V2_BYTES: usize = 109_426;
#[cfg(test)]
const METRIC_CATALOG_V2_BLAKE3: &str =
    "62cba32dfc1d08ef52d5721497a312e0c879fcf9a2e5a2e5d69e8ebb2a5a0812";

#[derive(Clone, Copy)]
struct MetricSpec {
    name: &'static str,
    help: &'static str,
    registered: bool,
}

struct MetricSpecCursor {
    lines: std::str::Lines<'static>,
    row: usize,
    registered: usize,
}

impl MetricSpecCursor {
    fn v2() -> Self {
        let mut lines = METRIC_CATALOG_V2.lines();
        assert_eq!(
            lines.next(),
            Some(METRIC_CATALOG_V2_HEADER),
            "unexpected metric catalog header"
        );
        assert_eq!(
            METRIC_CATALOG_V2.len(),
            METRIC_CATALOG_V2_BYTES,
            "metric catalog byte length changed"
        );
        Self {
            lines,
            row: 0,
            registered: 0,
        }
    }

    fn spec(&mut self, expected_key: &str) -> MetricSpec {
        let row = self.row + 1;
        let line = self
            .lines
            .next()
            .unwrap_or_else(|| panic!("metric catalog ended before `{expected_key}` at row {row}"));
        let mut fields = line.split('\t');
        let key = fields.next().expect("metric catalog row has a key");
        let name = fields.next().expect("metric catalog row has a name");
        let help = fields.next().expect("metric catalog row has help text");
        let registered = match fields.next() {
            Some("registered") => true,
            Some("unregistered") => false,
            Some(value) => panic!("metric catalog row {row} has invalid registration `{value}`"),
            None => panic!("metric catalog row {row} has no registration state"),
        };
        if registered {
            self.registered += 1;
        }
        assert!(
            fields.next().is_none(),
            "metric catalog row {row} has extra fields"
        );
        assert_eq!(
            key, expected_key,
            "metric catalog row {row} is out of construction order"
        );
        self.row = row;
        MetricSpec {
            name,
            help,
            registered,
        }
    }

    fn finish(&mut self) {
        assert_eq!(
            self.row, METRIC_CATALOG_V2_ROWS,
            "metric catalog construction count changed"
        );
        assert_eq!(
            self.registered, METRIC_CATALOG_V2_REGISTERED,
            "metric catalog registration count changed"
        );
        assert!(
            self.lines.next().is_none(),
            "metric catalog has unconsumed rows after row {}",
            self.row
        );
    }
}

// Guarded registration panics on duplicate names when requested in debug builds and treats the
// Prometheus duplicate error as the sole recoverable registration failure otherwise.
fn register_guarded<C: Collector + Clone + 'static>(registry: &Registry, metric: &C) {
    if let Err(err) = registry.register(Box::new(metric.clone())) {
        let is_duplicate = matches!(&err, prometheus::Error::AlreadyReg);
        assert!(
            !(duplicate_metrics_should_panic() && is_duplicate),
            "Duplicate metric registration attempted: {err}"
        );
        assert!(is_duplicate, "Metric registration failed: {err}");
        #[cfg(debug_assertions)]
        {
            eprintln!(
                "Duplicate metric registration attempted: {:?}: {err}",
                metric.desc()
            );
        }
    }
}

struct MetricFactory<'a> {
    registry: &'a Registry,
    specs: &'a mut MetricSpecCursor,
}

impl<'a> MetricFactory<'a> {
    fn new(registry: &'a Registry, specs: &'a mut MetricSpecCursor) -> Self {
        Self { registry, specs }
    }

    fn collect<C: Collector + Clone + 'static>(
        &self,
        metric: Result<C, prometheus::Error>,
        registered: bool,
    ) -> C {
        let metric = metric.expect("metric catalog contains valid Prometheus descriptors");
        if registered {
            register_guarded(self.registry, &metric);
        }
        metric
    }

    fn int_counter(&mut self, key: &str) -> IntCounter {
        let spec = self.specs.spec(key);
        self.collect(
            IntCounter::with_opts(Opts::new(spec.name, spec.help)),
            spec.registered,
        )
    }

    fn int_counter_vec(&mut self, key: &str, labels: &[&str]) -> IntCounterVec {
        let spec = self.specs.spec(key);
        self.collect(
            IntCounterVec::new(Opts::new(spec.name, spec.help), labels),
            spec.registered,
        )
    }

    fn int_gauge(&mut self, key: &str) -> IntGauge {
        let spec = self.specs.spec(key);
        self.collect(
            IntGauge::with_opts(Opts::new(spec.name, spec.help)),
            spec.registered,
        )
    }

    fn int_gauge_vec(&mut self, key: &str, labels: &[&str]) -> IntGaugeVec {
        let spec = self.specs.spec(key);
        self.collect(
            IntGaugeVec::new(Opts::new(spec.name, spec.help), labels),
            spec.registered,
        )
    }

    fn gauge(&mut self, key: &str) -> GenericGauge<AtomicU64> {
        let spec = self.specs.spec(key);
        self.collect(
            GenericGauge::with_opts(Opts::new(spec.name, spec.help)),
            spec.registered,
        )
    }

    fn gauge_vec(&mut self, key: &str, labels: &[&str]) -> GenericGaugeVec<AtomicU64> {
        let spec = self.specs.spec(key);
        self.collect(
            GenericGaugeVec::new(Opts::new(spec.name, spec.help), labels),
            spec.registered,
        )
    }

    fn float_counter_vec(&mut self, key: &str, labels: &[&str]) -> CounterVec {
        let spec = self.specs.spec(key);
        self.collect(
            CounterVec::new(Opts::new(spec.name, spec.help), labels),
            spec.registered,
        )
    }

    fn float_gauge(&mut self, key: &str) -> Gauge {
        let spec = self.specs.spec(key);
        self.collect(
            Gauge::with_opts(Opts::new(spec.name, spec.help)),
            spec.registered,
        )
    }

    fn float_gauge_vec(&mut self, key: &str, labels: &[&str]) -> GaugeVec {
        let spec = self.specs.spec(key);
        self.collect(
            GaugeVec::new(Opts::new(spec.name, spec.help), labels),
            spec.registered,
        )
    }

    fn histogram_with_buckets(&mut self, key: &str, buckets: Vec<f64>) -> Histogram {
        let spec = self.specs.spec(key);
        self.collect(
            Histogram::with_opts(HistogramOpts::new(spec.name, spec.help).buckets(buckets)),
            spec.registered,
        )
    }

    fn histogram_vec(&mut self, key: &str, labels: &[&str]) -> HistogramVec {
        let spec = self.specs.spec(key);
        self.collect(
            HistogramVec::new(HistogramOpts::new(spec.name, spec.help), labels),
            spec.registered,
        )
    }

    fn histogram_vec_with_buckets(
        &mut self,
        key: &str,
        buckets: Vec<f64>,
        labels: &[&str],
    ) -> HistogramVec {
        let spec = self.specs.spec(key);
        self.collect(
            HistogramVec::new(
                HistogramOpts::new(spec.name, spec.help).buckets(buckets),
                labels,
            ),
            spec.registered,
        )
    }

    fn finish(self) {
        self.specs.finish();
    }
}

#[cfg(test)]
mod metric_catalog_tests {
    use std::collections::BTreeSet;

    use super::{
        METRIC_CATALOG_V2, METRIC_CATALOG_V2_BLAKE3, METRIC_CATALOG_V2_BYTES,
        METRIC_CATALOG_V2_HEADER, METRIC_CATALOG_V2_REGISTERED, METRIC_CATALOG_V2_ROWS, Metrics,
    };

    #[test]
    fn v1_catalog_is_complete_and_unique() {
        let mut lines = METRIC_CATALOG_V2.lines();
        assert_eq!(lines.next(), Some(METRIC_CATALOG_V2_HEADER));
        let mut keys = BTreeSet::new();
        let mut names = BTreeSet::new();
        let mut registered = 0;
        for (index, line) in lines.enumerate() {
            let row = index + 1;
            let mut fields = line.split('\t');
            let key = fields.next().expect("metric catalog row has a key");
            let name = fields.next().expect("metric catalog row has a name");
            let help = fields.next().expect("metric catalog row has help text");
            match fields.next() {
                Some("registered") => registered += 1,
                Some("unregistered") => {}
                value => panic!("catalog row {row} has invalid registration state {value:?}"),
            }
            assert!(
                fields.next().is_none(),
                "catalog row {row} has extra fields"
            );
            assert!(!key.is_empty() && !name.is_empty() && !help.is_empty());
            assert!(keys.insert(key), "duplicate metric key `{key}`");
            assert!(names.insert(name), "duplicate metric name `{name}`");
        }
        assert_eq!(keys.len(), METRIC_CATALOG_V2_ROWS);
        assert_eq!(registered, METRIC_CATALOG_V2_REGISTERED);
        assert_eq!(METRIC_CATALOG_V2.len(), METRIC_CATALOG_V2_BYTES);
        assert_eq!(
            blake3::hash(METRIC_CATALOG_V2.as_bytes()).to_hex().as_str(),
            METRIC_CATALOG_V2_BLAKE3
        );
        let _ = Metrics::default();
    }
}

static GLOBAL_METRICS: OnceLock<Arc<Metrics>> = OnceLock::new();
/// Retrieve the globally installed metrics registry, if any.
#[must_use]
pub fn global() -> Option<&'static Arc<Metrics>> {
    GLOBAL_METRICS.get()
}
/// Install the global metrics registry. Returns the input on failure if a registry
/// was already installed.
///
/// # Errors
/// Returns the provided `metrics` back if a global registry was already installed.
pub fn install_global(metrics: Arc<Metrics>) -> Result<(), Arc<Metrics>> {
    GLOBAL_METRICS.set(metrics)
}
/// Fetch the global metrics handle if available, otherwise install a default instance.
pub fn global_or_default() -> Arc<Metrics> {
    Arc::clone(GLOBAL_METRICS.get_or_init(|| Arc::new(Metrics::default())))
}
static DUPLICATE_METRICS_PANIC: OnceLock<AtomicBool> = OnceLock::new();
fn duplicate_metrics_default() -> bool {
    #[cfg(debug_assertions)]
    {
        matches!(
            std::env::var("IROHA_METRICS_PANIC_ON_DUPLICATE")
                .map(|v| v.to_ascii_lowercase())
                .as_deref(),
            Ok("1" | "true" | "yes")
        )
    }
    #[cfg(not(debug_assertions))]
    {
        false
    }
}
fn duplicate_metrics_flag() -> &'static AtomicBool {
    DUPLICATE_METRICS_PANIC.get_or_init(|| AtomicBool::new(duplicate_metrics_default()))
}
fn duplicate_metrics_should_panic() -> bool {
    duplicate_metrics_flag().load(Ordering::Relaxed)
}
/// Override duplicate-metric panic behaviour (preferred over env vars).
pub fn set_duplicate_metrics_panic(enabled: bool) {
    duplicate_metrics_flag().store(enabled, Ordering::Relaxed);
}
/// Buffer gauge values for a settlement lane.
#[derive(Clone, Copy, Debug)]
pub struct LaneSettlementBuffer {
    /// Remaining XOR amount in the buffer (micro units converted to `f64`).
    pub remaining: f64,
    /// Maximum XOR capacity configured for the buffer.
    pub capacity: f64,
    /// Status indicator encoded as `0.0` (normal), `1.0` (alert), `2.0` (throttle),
    /// `3.0` (XOR-only), or `4.0` (halt).
    pub status: f64,
}
/// Swapline utilisation metrics for a settlement lane.
#[derive(Clone, Copy, Debug)]
pub struct LaneSwaplineSnapshot<'a> {
    /// Liquidity profile label emitted by the router.
    pub profile: &'a str,
    /// XOR utilisation attributed to the swapline in micro units.
    pub utilisation_micro: u128,
}
/// Complete settlement snapshot for a single lane.
#[derive(Clone, Copy, Debug)]
pub struct LaneSettlementSnapshot<'a> {
    /// Lane identifier used as a Prometheus label.
    pub lane_id: &'a str,
    /// Dataspace identifier used as a Prometheus label.
    pub dataspace_id: &'a str,
    /// Total XOR due for the settlement batch (micro units).
    pub xor_due_micro: u128,
    /// Variance between expected and realised XOR (micro units).
    pub variance_micro: u128,
    /// Applied haircut expressed in basis points.
    pub haircut_bps: u16,
    /// Optional swapline utilisation telemetry.
    pub swapline: Option<LaneSwaplineSnapshot<'a>>,
    /// Optional settlement buffer occupancy telemetry.
    pub buffer: Option<LaneSettlementBuffer>,
}
/// Complete metrics projection derived from one finalized `SoraFS` reserve view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SorafsReserveFinalizedProjection {
    /// Height of the finalized ledger view used to derive every projected value.
    pub finalized_height: u64,
    /// Provider counts ordered as active, warning, grace, delinquent, and default.
    pub lifecycle_stage_counts: [u64; 5],
    /// Outstanding credit principal per lifecycle stage, in micro-XOR.
    pub credit_principal_micro_xor: [u128; 5],
    /// Credit shortfall per lifecycle stage, in micro-XOR.
    pub credit_shortfall_micro_xor: [u128; 5],
    /// Accrued interest per lifecycle stage, in micro-XOR.
    pub accrued_interest_micro_xor: [u128; 5],
    /// Number of reserve appeals that remain open.
    pub open_appeals: u64,
    /// Custody movement counts ordered as pending, approved, and rejected.
    pub custody_counts: [u64; 3],
    /// Chain-reconciled movement counts ordered as approved and rejected.
    pub chain_reconciled_counts: [u64; 2],
}
impl Metrics {
    fn lock_sorafs_orderbook_projection_exposition(&self) -> std::sync::MutexGuard<'_, ()> {
        match self.sorafs_orderbook_projection_exposition_lock.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
    fn lock_sorafs_gateway_compliance_exposition(&self) -> std::sync::MutexGuard<'_, ()> {
        match self.sorafs_gateway_compliance_exposition_lock.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
    fn prune_recent_rejection_events(events: &mut VecDeque<(u64, u64)>, now_ms: u64) {
        let cutoff_ms = now_ms.saturating_sub(REJECTION_RECENT_WINDOW_MS);
        while matches!(events.front(), Some((timestamp_ms, _)) if *timestamp_ms < cutoff_ms) {
            events.pop_front();
        }
        while events.len() > REJECTION_RECENT_EVENT_CAP {
            events.pop_front();
        }
    }
    fn to_f64(value: u64) -> f64 {
        #[allow(clippy::cast_precision_loss)]
        {
            value as f64
        }
    }
    fn ratio_or_zero(numerator_ms: f64, window_ms: f64) -> f64 {
        if window_ms <= 0.0 {
            return 0.0;
        }
        let ratio = numerator_ms / window_ms;
        ratio.clamp(0.0, 1.0)
    }
    /// Record a newly observed batch of rejected transactions for `/status` freshness reporting.
    pub fn record_rejected_transactions(&self, count: u64, observed_at_ms: u64) {
        if count == 0 {
            return;
        }
        self.last_rejection_at_ms
            .store(observed_at_ms, Ordering::Relaxed);
        let mut events = self
            .recent_rejection_events
            .lock()
            .expect("recent rejection event cache poisoned");
        events.push_back((observed_at_ms, count));
        Self::prune_recent_rejection_events(&mut events, observed_at_ms);
    }
    /// Return the latest rejection timestamp observed by this node, if any.
    #[must_use]
    pub fn last_rejection_at_ms(&self) -> Option<u64> {
        match self.last_rejection_at_ms.load(Ordering::Relaxed) {
            0 => None,
            timestamp_ms => Some(timestamp_ms),
        }
    }
    /// Return the number of rejected transactions observed within the last five minutes.
    #[must_use]
    pub fn txs_rejected_recent_5m(&self, now_ms: u64) -> u64 {
        let mut events = self
            .recent_rejection_events
            .lock()
            .expect("recent rejection event cache poisoned");
        Self::prune_recent_rejection_events(&mut events, now_ms);
        events
            .iter()
            .fold(0_u64, |total, (_, count)| total.saturating_add(*count))
    }
    /// Update stack sizing gauges and counters from the latest snapshot.
    pub fn apply_stack_snapshot(&self, snapshot: &StackSettingsSnapshot) {
        self.ivm_stack_bytes
            .with_label_values(&["scheduler", "requested"])
            .set(snapshot.requested_scheduler_bytes);
        self.ivm_stack_bytes
            .with_label_values(&["prover", "requested"])
            .set(snapshot.requested_prover_bytes);
        self.ivm_stack_bytes
            .with_label_values(&["guest", "requested"])
            .set(snapshot.requested_guest_bytes);
        self.ivm_stack_bytes
            .with_label_values(&["scheduler", "applied"])
            .set(snapshot.scheduler_bytes);
        self.ivm_stack_bytes
            .with_label_values(&["prover", "applied"])
            .set(snapshot.prover_bytes);
        self.ivm_stack_bytes
            .with_label_values(&["guest", "applied"])
            .set(snapshot.guest_bytes);
        self.ivm_stack_clamped
            .with_label_values(&["scheduler"])
            .set(u64::from(snapshot.scheduler_clamped));
        self.ivm_stack_clamped
            .with_label_values(&["prover"])
            .set(u64::from(snapshot.prover_clamped));
        self.ivm_stack_clamped
            .with_label_values(&["guest"])
            .set(u64::from(snapshot.guest_clamped));
        self.ivm_stack_gas_multiplier
            .set(snapshot.gas_to_stack_multiplier.max(1));
        self.ivm_stack_pool_fallback_total.reset();
        self.ivm_stack_pool_fallback_total
            .inc_by(snapshot.pool_fallback_total);
        self.ivm_stack_budget_hit_total.reset();
        self.ivm_stack_budget_hit_total
            .inc_by(snapshot.budget_hit_total);
    }
    /// Record the current fsync policy used by Kura storage.
    pub fn set_kura_fsync_mode(&self, mode: FsyncMode) {
        let value = match mode {
            FsyncMode::Always => 1,
            FsyncMode::Batched => 2,
        };
        self.kura_fsync_enabled.set(value);
    }
    /// Record a fsync failure for the given target.
    pub fn inc_kura_fsync_failure(&self, target: &str) {
        self.kura_fsync_failures_total
            .with_label_values(&[target])
            .inc();
    }
    /// Observe fsync latency in milliseconds for the given target.
    pub fn record_kura_fsync_latency(&self, target: &str, duration: Duration) {
        self.kura_fsync_latency_ms
            .with_label_values(&[target])
            .observe(duration.as_secs_f64() * 1000.0);
    }
    /// Update the active Space Directory manifest gauge for a specific dataspace/profile.
    pub fn set_space_directory_active_manifests(
        &self,
        dataspace: &str,
        dataspace_id: &str,
        profile: &str,
        count: u64,
    ) {
        self.nexus_space_directory_active_manifests
            .with_label_values(&[dataspace, dataspace_id, profile])
            .set(count);
    }
    /// Record the latest block height observed for a lane/dataspace pair.
    pub fn set_lane_block_height(&self, lane: &str, dataspace: &str, height: u64) {
        self.nexus_lane_block_height
            .with_label_values(&[lane, dataspace])
            .set(height);
    }
    /// Record the finality lag (in slots) for a lane/dataspace pair.
    pub fn set_lane_finality_lag(&self, lane: &str, dataspace: &str, lag: u64) {
        self.nexus_lane_finality_lag_slots
            .with_label_values(&[lane, dataspace])
            .set(lag);
    }
    /// Record the settlement backlog (XOR) for a lane/dataspace pair.
    pub fn set_lane_settlement_backlog(&self, lane: &str, dataspace: &str, backlog_micro: u128) {
        self.nexus_lane_settlement_backlog_xor
            .with_label_values(&[lane, dataspace])
            .set(u128_to_f64(backlog_micro));
    }
    /// Record aggregated DA rent usage and incentive breakdowns for telemetry dashboards.
    pub fn record_da_rent_quote(
        &self,
        cluster: &str,
        storage_class: &str,
        gib_months: u64,
        quote: &DaRentQuote,
    ) {
        self.torii_da_rent_gib_months_total
            .with_label_values(&[cluster, storage_class])
            .inc_by(gib_months);
        let labels = [cluster, storage_class];
        self.torii_da_rent_base_micro_total
            .with_label_values(&labels)
            .inc_by(quantity_to_micro_f64(quote.base_rent.as_quantity()));
        self.torii_da_protocol_reserve_micro_total
            .with_label_values(&labels)
            .inc_by(quantity_to_micro_f64(quote.protocol_reserve.as_quantity()));
        self.torii_da_provider_reward_micro_total
            .with_label_values(&labels)
            .inc_by(quantity_to_micro_f64(quote.provider_reward.as_quantity()));
        self.torii_da_pdp_bonus_micro_total
            .with_label_values(&labels)
            .inc_by(quantity_to_micro_f64(quote.pdp_bonus.as_quantity()));
        self.torii_da_potr_bonus_micro_total
            .with_label_values(&labels)
            .inc_by(quantity_to_micro_f64(quote.potr_bonus.as_quantity()));
    }
    /// Record a DA receipt ingest outcome and optionally advance the cursor gauge.
    pub fn record_da_receipt_outcome(
        &self,
        lane_id: u32,
        epoch: u64,
        sequence: u64,
        outcome: &str,
        cursor_advanced: bool,
    ) {
        let lane_label = lane_id.to_string();
        let outcome = bounded_da_receipt_outcome(outcome);
        let mut lanes = self
            .da_receipt_metric_lanes
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(lane) = da_receipt_metric_lane(&mut lanes, lane_id) else {
            return;
        };
        self.torii_da_receipts_total
            .with_label_values(&[outcome, &lane_label])
            .inc();
        if cursor_advanced {
            let cursor = update_da_receipt_metric_cursor(lane, epoch, sequence);
            self.torii_da_receipt_epoch
                .with_label_values(&[&lane_label])
                .set(cursor.epoch);
            self.torii_da_receipt_highest_sequence
                .with_label_values(&[&lane_label])
                .set(cursor.highest_sequence);
        }
    }
    /// Observe DA chunking + erasure coding duration in seconds.
    pub fn observe_da_chunking_seconds(&self, seconds: f64) {
        self.torii_da_chunking_seconds.observe(seconds);
    }
    /// Record a Torii DA spool batch write outcome.
    pub fn record_torii_da_spool_batch(&self, outcome: &'static str, write_ms: f64) {
        self.torii_da_spool_batches_total
            .with_label_values(&[outcome])
            .inc();
        self.torii_da_spool_batch_write_ms
            .observe(write_ms.max(0.0));
    }
    /// Record Torii DA spool artifact outcomes.
    pub fn record_torii_da_spool_artifact(
        &self,
        kind: &'static str,
        outcome: &'static str,
        count: u64,
    ) {
        if count == 0 {
            return;
        }
        self.torii_da_spool_artifacts_total
            .with_label_values(&[kind, outcome])
            .inc_by(count);
    }
    /// Set the current Torii DA spool queue depth.
    pub fn set_torii_da_spool_queue_depth(&self, depth: u64) {
        self.torii_da_spool_queue_depth.set(depth);
    }
    /// Update the latest DA receipt cursor retained for a lane.
    pub fn set_da_receipt_cursor(&self, lane_id: u32, epoch: u64, sequence: u64) {
        let lane_label = lane_id.to_string();
        let mut lanes = self
            .da_receipt_metric_lanes
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(lane) = da_receipt_metric_lane(&mut lanes, lane_id) else {
            return;
        };
        let cursor = update_da_receipt_metric_cursor(lane, epoch, sequence);
        self.torii_da_receipt_epoch
            .with_label_values(&[&lane_label])
            .set(cursor.epoch);
        self.torii_da_receipt_highest_sequence
            .with_label_values(&[&lane_label])
            .set(cursor.highest_sequence);
    }
    /// Remove all DA receipt metric state for retired lanes.
    pub fn prune_da_receipt_lanes(&self, lane_ids: impl IntoIterator<Item = u32>) {
        let mut lanes = self
            .da_receipt_metric_lanes
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for lane_id in lane_ids {
            if lanes.remove(&lane_id).is_none() {
                continue;
            }
            let lane_label = lane_id.to_string();
            let _ = self
                .torii_da_receipt_epoch
                .remove_label_values(&[&lane_label]);
            let _ = self
                .torii_da_receipt_highest_sequence
                .remove_label_values(&[&lane_label]);
            for outcome in DA_RECEIPT_OUTCOME_LABELS {
                let _ = self
                    .torii_da_receipts_total
                    .remove_label_values(&[outcome, &lane_label]);
            }
        }
    }
    /// Snapshot the latest DA receipt cursor retained for each lane.
    pub fn da_receipt_cursor_status(&self) -> Vec<DaReceiptCursorStatus> {
        self.da_receipt_metric_lanes
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .filter_map(|(&lane_id, lane)| {
                lane.cursor.map(|cursor| DaReceiptCursorStatus {
                    lane_id,
                    epoch: cursor.epoch,
                    highest_sequence: cursor.highest_sequence,
                })
            })
            .collect()
    }
    /// Record a DA shard cursor event and track the latest block height per cursor.
    pub fn record_da_shard_cursor_event(
        &self,
        event: &str,
        lane_id: u32,
        shard_id: u32,
        block_height: u64,
    ) {
        let lane_label = lane_id.to_string();
        let shard_label = shard_id.to_string();
        self.da_shard_cursor_events_total
            .with_label_values(&[event, &lane_label, &shard_label])
            .inc();
        let height = i64::try_from(block_height).unwrap_or(i64::MAX);
        self.da_shard_cursor_height
            .with_label_values(&[&lane_label, &shard_label])
            .set(height);
    }
    /// Record the lag (in blocks) between the validated height and the last shard cursor advance.
    pub fn set_da_shard_cursor_lag(&self, lane_id: u32, shard_id: u32, lag_blocks: i64) {
        let lane_label = lane_id.to_string();
        let shard_label = shard_id.to_string();
        self.da_shard_cursor_lag_blocks
            .with_label_values(&[&lane_label, &shard_label])
            .set(lag_blocks);
    }
    /// Increment the manifest revision counter for a dataspace.
    pub fn inc_space_directory_revision(&self, dataspace: &str, dataspace_id: &str) {
        self.nexus_space_directory_revision_total
            .with_label_values(&[dataspace, dataspace_id])
            .inc();
    }
    /// Increment the manifest revocation counter for a dataspace/reason.
    pub fn inc_space_directory_revocations(
        &self,
        dataspace: &str,
        dataspace_id: &str,
        reason: &str,
    ) {
        self.nexus_space_directory_revocations_total
            .with_label_values(&[dataspace, dataspace_id, reason])
            .inc();
    }
    /// Replace the cached list of sealed lane aliases used by status snapshots.
    pub fn set_lane_governance_sealed_aliases(&self, aliases: Vec<String>) {
        if let Ok(mut guard) = self.nexus_lane_governance_sealed_aliases.write() {
            *guard = aliases;
        }
    }
    /// Snapshot the cached sealed lane aliases.
    pub fn lane_governance_sealed_aliases(&self) -> Vec<String> {
        self.nexus_lane_governance_sealed_aliases
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }
    /// Cache the current consensus mode tag for status exports.
    pub fn set_sumeragi_mode_tag(&self, mode_tag: &str) {
        if let Ok(mut guard) = self.sumeragi_mode_tag.write() {
            *guard = mode_tag.to_string();
        }
    }
    /// Snapshot the cached consensus mode tag.
    pub fn sumeragi_mode_tag(&self) -> String {
        self.sumeragi_mode_tag
            .read()
            .map_or_else(|_| PERMISSIONED_TAG.to_string(), |guard| guard.clone())
    }
    /// Record the canonical IVM gas schedule hash (split into two 64-bit gauges).
    pub fn set_ivm_gas_schedule_hash(&self, hash: &[u8; 32]) {
        let lo = u64::from_be_bytes(hash[..8].try_into().expect("slice length guarded"));
        let hi = u64::from_be_bytes(hash[8..16].try_into().expect("slice length guarded"));
        self.ivm_gas_schedule_hash_lo.set(lo);
        self.ivm_gas_schedule_hash_hi.set(hi);
    }
    /// Record the current confidential gas schedule.
    pub fn set_confidential_gas_schedule(&self, gas: &ActualConfidentialGas) {
        self.confidential_gas_base_verify.set(gas.proof_base);
        self.confidential_gas_per_public_input
            .set(gas.per_public_input);
        self.confidential_gas_per_proof_byte.set(gas.per_proof_byte);
        self.confidential_gas_per_nullifier.set(gas.per_nullifier);
        self.confidential_gas_per_commitment.set(gas.per_commitment);
    }
    /// Record a rejected Torii account identifier along with the failure reason.
    pub fn inc_torii_address_invalid(&self, endpoint: &str, reason: &str) {
        self.torii_address_invalid_total
            .with_label_values(&[endpoint, reason])
            .inc();
    }
    /// Record an SNS registrar outcome grouped by result and suffix.
    pub fn inc_sns_registrar_status(&self, result: &str, suffix: &str) {
        self.sns_registrar_status_total
            .with_label_values(&[result, suffix])
            .inc();
    }
    /// Increment the account literal selection counter.
    pub fn inc_torii_account_literal(&self, endpoint: &str, format: &str) {
        self.torii_account_literal_total
            .with_label_values(&[endpoint, format])
            .inc();
    }
    /// Record a Norito-RPC gate observation with rollout stage/outcome labels.
    pub fn inc_torii_norito_rpc_gate(&self, stage: &str, outcome: &str) {
        self.torii_norito_rpc_gate_total
            .with_label_values(&[stage, outcome])
            .inc();
    }
    /// Record an API-token-gated Torii endpoint hit without exposing token material.
    pub fn inc_torii_api_token_hit(&self, endpoint: &str, token_state: &str) {
        self.torii_api_token_hits_total
            .with_label_values(&[endpoint, token_state])
            .inc();
    }
    /// Record an operator auth event with action/result/reason labels.
    pub fn inc_torii_operator_auth(&self, action: &str, result: &str, reason: &str) {
        self.torii_operator_auth_total
            .with_label_values(&[action, result, reason])
            .inc();
    }
    /// Record an operator auth lockout with action/reason labels.
    pub fn inc_torii_operator_auth_lockout(&self, action: &str, reason: &str) {
        self.torii_operator_auth_lockout_total
            .with_label_values(&[action, reason])
            .inc();
    }
    /// Record a Norito-RPC decode failure emitted by Torii.
    pub fn inc_torii_norito_decode_failure(&self, payload_kind: &str, reason: &str) {
        self.torii_norito_decode_failures_total
            .with_label_values(&[payload_kind, reason])
            .inc();
    }
    /// Record a rejected attachment during Torii sanitization.
    pub fn inc_torii_attachment_reject(&self, reason: &str) {
        self.torii_attachment_reject_total
            .with_label_values(&[reason])
            .inc();
    }
    /// Record attachment sanitization latency in milliseconds.
    pub fn observe_torii_attachment_sanitize_ms(&self, millis: u64) {
        let millis = u32::try_from(millis).unwrap_or(u32::MAX);
        self.torii_attachment_sanitize_ms
            .with_label_values::<&str>(&[])
            .observe(f64::from(millis));
    }
    /// Record FASTPQ execution mode resolution metrics.
    pub fn record_fastpq_execution_mode(
        &self,
        requested: &str,
        resolved: &str,
        backend: &str,
        device_class: &str,
        chip_family: &str,
        gpu_kind: &str,
    ) {
        self.fastpq_execution_mode_total
            .with_label_values(&[
                requested,
                resolved,
                backend,
                device_class,
                chip_family,
                gpu_kind,
            ])
            .inc();
        #[cfg(feature = "otel-exporter")]
        {
            let otel = global_fastpq_otel();
            otel.record_execution_mode(
                requested,
                resolved,
                backend,
                device_class,
                chip_family,
                gpu_kind,
            );
        }
    }
    /// Record Poseidon pipeline resolution metrics for FASTPQ.
    pub fn record_fastpq_poseidon_mode(
        &self,
        requested: &str,
        resolved: &str,
        path: &str,
        device_class: &str,
        chip_family: &str,
        gpu_kind: &str,
    ) {
        self.fastpq_poseidon_pipeline_total
            .with_label_values(&[
                requested,
                resolved,
                path,
                device_class,
                chip_family,
                gpu_kind,
            ])
            .inc();
        #[cfg(feature = "otel-exporter")]
        {
            let otel = global_fastpq_otel();
            otel.record_poseidon_pipeline(
                requested,
                resolved,
                path,
                device_class,
                chip_family,
                gpu_kind,
            );
        }
    }
    /// Increment a FASTPQ GPU accelerator disable event counter.
    pub fn inc_fastpq_gpu_disable(
        &self,
        accelerator: &str,
        reason: &str,
        device_class: &str,
        chip_family: &str,
        gpu_kind: &str,
    ) {
        self.fastpq_gpu_disable_total
            .with_label_values(&[accelerator, reason, device_class, chip_family, gpu_kind])
            .inc();
    }
    /// Increment a FASTPQ sampled GPU parity failure counter.
    pub fn inc_fastpq_gpu_parity_failure(
        &self,
        accelerator: &str,
        reason: &str,
        device_class: &str,
        chip_family: &str,
        gpu_kind: &str,
    ) {
        self.fastpq_gpu_parity_failure_total
            .with_label_values(&[accelerator, reason, device_class, chip_family, gpu_kind])
            .inc();
    }
    /// Set FASTPQ proof sidecar queue depth.
    pub fn set_fastpq_proof_sidecar_queue_depth(&self, depth: u64) {
        self.fastpq_proof_sidecar_queue_depth.set(depth);
    }
    /// Increment a FASTPQ proof sidecar persistence event counter.
    pub fn inc_fastpq_proof_sidecar_event(&self, event: &str) {
        self.fastpq_proof_sidecar_events_total
            .with_label_values(&[event])
            .inc();
    }
    /// Record aggregated Metal queue statistics for FASTPQ.
    pub fn record_fastpq_metal_queue_stats(
        &self,
        device_class: &str,
        chip_family: &str,
        gpu_kind: &str,
        sample: &FastpqMetalQueueSample<'_>,
    ) {
        self.fastpq_metal_queue_depth
            .with_label_values(&[device_class, chip_family, gpu_kind, "limit"])
            .set(Self::to_f64(sample.limit));
        self.fastpq_metal_queue_depth
            .with_label_values(&[device_class, chip_family, gpu_kind, "max_in_flight"])
            .set(Self::to_f64(sample.max_in_flight));
        self.fastpq_metal_queue_depth
            .with_label_values(&[device_class, chip_family, gpu_kind, "dispatch_count"])
            .set(Self::to_f64(sample.dispatch_count));
        self.fastpq_metal_queue_depth
            .with_label_values(&[device_class, chip_family, gpu_kind, "window_seconds"])
            .set(sample.window_ms.max(0.0) / 1_000.0);
        let window_ms = sample.window_ms.max(0.0);
        for (metric, value) in [("busy", sample.busy_ms), ("overlap", sample.overlap_ms)] {
            self.fastpq_metal_queue_ratio
                .with_label_values(&[device_class, chip_family, gpu_kind, "global", metric])
                .set(Self::ratio_or_zero(value, window_ms));
        }
        for lane in sample.lanes {
            let queue_label = format!("lane-{}", lane.index);
            for (metric, value) in [("busy", lane.busy_ms), ("overlap", lane.overlap_ms)] {
                self.fastpq_metal_queue_ratio
                    .with_label_values(&[
                        device_class,
                        chip_family,
                        gpu_kind,
                        queue_label.as_str(),
                        metric,
                    ])
                    .set(Self::ratio_or_zero(value, window_ms));
            }
        }
    }
    /// Record host zero-fill telemetry for FASTPQ Metal runs.
    pub fn record_fastpq_zero_fill(
        &self,
        device_class: &str,
        chip_family: &str,
        gpu_kind: &str,
        duration_ms: f64,
        bytes: u64,
    ) {
        let sanitized_duration = if duration_ms.is_finite() && duration_ms >= 0.0 {
            duration_ms
        } else {
            0.0
        };
        self.fastpq_zero_fill_duration_ms
            .with_label_values(&[device_class, chip_family, gpu_kind])
            .set(sanitized_duration);
        let bandwidth = if sanitized_duration > 0.0 {
            // Convert bytes/ms → Gbps using 1e6 as the multiplier (1000 ms * 1e9 bits).
            (u64_to_f64(bytes) * 8.0) / (sanitized_duration * 1_000_000.0)
        } else {
            0.0
        };
        self.fastpq_zero_fill_bandwidth_gbps
            .with_label_values(&[device_class, chip_family, gpu_kind])
            .set(bandwidth);
    }
    /// Record ISO bridge reference-data gauges.
    pub fn record_iso_reference_dataset(
        &self,
        dataset: &str,
        status: i64,
        age_seconds: Option<u64>,
        record_count: Option<usize>,
    ) {
        self.iso_reference_status
            .with_label_values(&[dataset])
            .set(status);
        let age_value = age_seconds
            .map(|age| age.min(i64::MAX as u64))
            .and_then(|age| i64::try_from(age).ok())
            .unwrap_or(-1);
        self.iso_reference_age_seconds
            .with_label_values(&[dataset])
            .set(age_value);
        let records_value = record_count
            .and_then(|count| u64::try_from(count).ok())
            .map(|count| count.min(i64::MAX as u64))
            .and_then(|count| i64::try_from(count).ok())
            .unwrap_or(-1);
        self.iso_reference_records
            .with_label_values(&[dataset])
            .set(records_value);
    }
    /// Record per-lane settlement telemetry for the latest block.
    pub fn record_lane_settlement_snapshot(&self, snapshot: LaneSettlementSnapshot<'_>) {
        let base_labels = [snapshot.lane_id, snapshot.dataspace_id];
        if let Some(buffer) = snapshot.buffer {
            self.settlement_buffer_xor
                .with_label_values(&base_labels)
                .set(buffer.remaining);
            self.settlement_buffer_capacity_xor
                .with_label_values(&base_labels)
                .set(buffer.capacity);
            self.settlement_buffer_status
                .with_label_values(&base_labels)
                .set(buffer.status);
        } else {
            self.settlement_buffer_xor
                .with_label_values(&base_labels)
                .set(u128_to_f64(snapshot.xor_due_micro));
            self.settlement_buffer_capacity_xor
                .with_label_values(&base_labels)
                .set(0.0);
            self.settlement_buffer_status
                .with_label_values(&base_labels)
                .set(0.0);
        }
        self.settlement_pnl_xor
            .with_label_values(&base_labels)
            .set(u128_to_f64(snapshot.variance_micro));
        self.settlement_haircut_bp
            .with_label_values(&base_labels)
            .set(f64::from(snapshot.haircut_bps));
        if let Some(swapline) = snapshot.swapline {
            let swap_labels = [snapshot.lane_id, snapshot.dataspace_id, swapline.profile];
            self.settlement_swapline_utilisation
                .with_label_values(&swap_labels)
                .set(u128_to_f64(swapline.utilisation_micro));
        }
        self.set_lane_settlement_backlog(base_labels[0], base_labels[1], snapshot.xor_due_micro);
    }
    /// Increment conversion counters for a lane/dataspace/source token trio.
    pub fn inc_settlement_conversion_total(
        &self,
        lane: &str,
        dataspace: &str,
        source: &str,
        count: u64,
    ) {
        if count == 0 {
            return;
        }
        self.settlement_conversion_total
            .with_label_values(&[lane, dataspace, source])
            .inc_by(count);
    }
    /// Increment the cumulative haircut total (XOR units) for a lane/dataspace pair.
    pub fn inc_settlement_haircut_total(&self, lane: &str, dataspace: &str, haircut_micro: u128) {
        if haircut_micro == 0 {
            return;
        }
        self.settlement_haircut_total
            .with_label_values(&[lane, dataspace])
            .inc_by(u128_to_f64(haircut_micro) / 1_000_000.0);
    }
    /// Update queue/backlog telemetry for the SoraNet privacy aggregator.
    pub fn record_soranet_privacy_queue_snapshot(&self, snapshot: &PrivacyDrainSnapshot) {
        for mode in [
            SoranetPrivacyModeV1::Entry,
            SoranetPrivacyModeV1::Middle,
            SoranetPrivacyModeV1::Exit,
        ] {
            let open_count = snapshot.open_buckets.get(&mode).copied().unwrap_or(0);
            let open_count = u32::try_from(open_count)
                .expect("open bucket count must fit into a u32 for export");
            self.soranet_privacy_open_buckets
                .with_label_values(&[mode.as_label()])
                .set(f64::from(open_count));
            let pending_collectors = snapshot.collector_backlog.get(&mode).copied().unwrap_or(0);
            let pending_collectors = u32::try_from(pending_collectors)
                .expect("pending collector count must fit into a u32 for export");
            self.soranet_privacy_pending_collectors
                .with_label_values(&[mode.as_label()])
                .set(f64::from(pending_collectors));
        }
        let drained_gauge = i64::try_from(snapshot.drained_buckets).unwrap_or(i64::MAX);
        self.soranet_privacy_snapshot_drained.set(drained_gauge);
        let suppressed_total: u64 = snapshot.suppressed_counts.values().copied().sum();
        let ratio = if snapshot.drained_buckets == 0 {
            0.0
        } else {
            let drained = u32::try_from(snapshot.drained_buckets).unwrap_or(u32::MAX);
            let suppressed = u32::try_from(suppressed_total).unwrap_or(u32::MAX);
            f64::from(suppressed) / f64::from(drained)
        };
        self.soranet_privacy_snapshot_suppression_ratio.set(ratio);
        for reason in [
            SoranetPrivacySuppressionReasonV1::InsufficientContributors,
            SoranetPrivacySuppressionReasonV1::CollectorSuppressed,
            SoranetPrivacySuppressionReasonV1::CollectorWindowElapsed,
            SoranetPrivacySuppressionReasonV1::ForcedFlushWindowElapsed,
        ] {
            let count = snapshot
                .suppressed_counts
                .get(&reason)
                .copied()
                .unwrap_or(0);
            let count =
                u32::try_from(count).expect("suppressed count must fit into a u32 for export");
            self.soranet_privacy_snapshot_suppressed
                .with_label_values(&[reason.as_label()])
                .set(f64::from(count));
        }
        for mode in [
            SoranetPrivacyModeV1::Entry,
            SoranetPrivacyModeV1::Middle,
            SoranetPrivacyModeV1::Exit,
        ] {
            let suppressed = snapshot.suppressed_by_mode.get(&mode);
            for reason in [
                SoranetPrivacySuppressionReasonV1::InsufficientContributors,
                SoranetPrivacySuppressionReasonV1::CollectorSuppressed,
                SoranetPrivacySuppressionReasonV1::CollectorWindowElapsed,
                SoranetPrivacySuppressionReasonV1::ForcedFlushWindowElapsed,
            ] {
                let count = suppressed
                    .and_then(|map| map.get(&reason))
                    .copied()
                    .unwrap_or(0);
                let count =
                    u32::try_from(count).expect("suppressed count must fit into a u32 for export");
                self.soranet_privacy_snapshot_suppressed_by_mode
                    .with_label_values(&[mode.as_label(), reason.as_label()])
                    .set(f64::from(count));
            }
        }
        if snapshot.evicted_completed > 0 {
            self.soranet_privacy_evicted_buckets_total
                .inc_by(snapshot.evicted_completed);
        }
    }
    /// Update Prometheus metrics with a newly aggregated SoraNet privacy bucket.
    ///
    /// Only fixed-cardinality labels are exported. Bucket timestamps and GAR category hashes remain
    /// available in the structured bucket payload, while this process-local registry exposes
    /// cumulative counters and the latest per-mode gauges without retaining one series per bucket.
    pub fn record_soranet_privacy_bucket(&self, bucket: &SoranetPrivacyBucketMetricsV1) {
        let mode_label = bucket.mode.as_label();
        self.soranet_privacy_latest_bucket_start_unixtime
            .with_label_values(&[mode_label])
            .set(i64::try_from(bucket.bucket_start_unix).unwrap_or(i64::MAX));
        if bucket.is_suppressed() {
            self.record_suppressed_soranet_privacy_bucket(mode_label, bucket);
            return;
        }
        self.soranet_privacy_bucket_suppressed
            .with_label_values(&[mode_label])
            .set(0.0);
        for (kind, value) in [
            ("accepted", bucket.handshake_accept_total),
            ("pow_rejected", bucket.handshake_pow_reject_total),
            ("downgrade", bucket.handshake_downgrade_total),
            ("timeout", bucket.handshake_timeout_total),
            ("other_failure", bucket.handshake_other_failure_total),
        ] {
            if value > 0 {
                self.soranet_privacy_circuit_events_total
                    .with_label_values(&[mode_label, kind])
                    .inc_by(value);
            }
        }
        for entry in &bucket.pow_rejects_by_reason {
            if entry.count > 0 {
                self.soranet_privacy_pow_rejects_total
                    .with_label_values(&[mode_label, entry.reason.as_label()])
                    .inc_by(entry.count);
            }
        }
        for (scope, value) in [
            ("congestion", bucket.throttle_congestion_total),
            ("cooldown", bucket.throttle_cooldown_total),
            ("emergency", bucket.throttle_emergency_total),
            ("remote_quota", bucket.throttle_remote_total),
        ] {
            if value > 0 {
                self.soranet_privacy_throttles_total
                    .with_label_values(&[mode_label, scope])
                    .inc_by(value);
            }
        }
        if bucket.verified_bytes_total > 0 {
            let bytes = if bucket.verified_bytes_total > u128::from(u64::MAX) {
                u64::MAX
            } else {
                u64::try_from(bucket.verified_bytes_total).unwrap_or(u64::MAX)
            };
            self.soranet_privacy_verified_bytes_total
                .with_label_values(&[mode_label])
                .inc_by(bytes);
        }
        let avg_value = bucket.active_circuits_mean.map_or(0.0, Self::to_f64);
        self.soranet_privacy_active_circuits_avg
            .with_label_values(&[mode_label])
            .set(avg_value);
        let max_value = bucket.active_circuits_max.map_or(0.0, Self::to_f64);
        self.soranet_privacy_active_circuits_max
            .with_label_values(&[mode_label])
            .set(max_value);
        for percentile in ["p50", "p90", "p99"] {
            self.soranet_privacy_rtt_millis
                .with_label_values(&[mode_label, percentile])
                .set(0.0);
        }
        for percentile in &bucket.rtt_percentiles_ms {
            if matches!(percentile.label.as_str(), "p50" | "p90" | "p99") {
                self.soranet_privacy_rtt_millis
                    .with_label_values(&[mode_label, percentile.label.as_str()])
                    .set(Self::to_f64(percentile.value_ms));
            }
        }
        let gar_reports = bucket
            .gar_abuse_counts
            .iter()
            .fold(0_u64, |total, entry| total.saturating_add(entry.count));
        if gar_reports > 0 {
            self.soranet_privacy_gar_reports_total
                .with_label_values(&[mode_label])
                .inc_by(gar_reports);
        }
    }

    fn record_suppressed_soranet_privacy_bucket(
        &self,
        mode_label: &str,
        bucket: &SoranetPrivacyBucketMetricsV1,
    ) {
        let reason_label = bucket
            .suppression_reason
            .map_or("unknown", SoranetPrivacySuppressionReasonV1::as_label);
        self.soranet_privacy_bucket_suppressed
            .with_label_values(&[mode_label])
            .set(1.0);
        self.soranet_privacy_suppression_total
            .with_label_values(&[mode_label, reason_label])
            .inc();
        self.soranet_privacy_active_circuits_avg
            .with_label_values(&[mode_label])
            .set(0.0);
        self.soranet_privacy_active_circuits_max
            .with_label_values(&[mode_label])
            .set(0.0);
        for percentile in ["p50", "p90", "p99"] {
            self.soranet_privacy_rtt_millis
                .with_label_values(&[mode_label, percentile])
                .set(0.0);
        }
    }

    /// Update the privacy collector enabled flag.
    pub fn set_soranet_privacy_collector_enabled(&self, enabled: bool) {
        self.soranet_privacy_collector_enabled
            .set(i64::from(enabled));
    }
    /// Record the latest SoraFS metering snapshot for a provider.
    #[allow(clippy::too_many_arguments)]
    pub fn record_sorafs_metering(
        &self,
        provider: &str,
        declared_gib: u64,
        effective_gib: u64,
        utilised_gib: u64,
        outstanding_gib: u64,
        outstanding_orders: u64,
        gib_hours: f64,
        orders_issued: u64,
        orders_completed: u64,
        orders_failed: u64,
        uptime_bps: u32,
        por_bps: u32,
    ) {
        self.torii_sorafs_capacity_declared_gib
            .with_label_values(&[provider])
            .set(declared_gib);
        self.torii_sorafs_capacity_effective_gib
            .with_label_values(&[provider])
            .set(effective_gib);
        self.torii_sorafs_capacity_utilised_gib
            .with_label_values(&[provider])
            .set(utilised_gib);
        self.torii_sorafs_capacity_outstanding_gib
            .with_label_values(&[provider])
            .set(outstanding_gib);
        self.torii_sorafs_capacity_gibhours_total
            .with_label_values(&[provider])
            .set(gib_hours);
        self.torii_sorafs_orders_issued_total
            .with_label_values(&[provider])
            .set(orders_issued);
        self.torii_sorafs_orders_completed_total
            .with_label_values(&[provider])
            .set(orders_completed);
        self.torii_sorafs_orders_failed_total
            .with_label_values(&[provider])
            .set(orders_failed);
        self.torii_sorafs_outstanding_orders
            .with_label_values(&[provider])
            .set(outstanding_orders);
        let uptime = clamp_u32_to_i64(uptime_bps);
        self.torii_sorafs_uptime_bps
            .with_label_values(&[provider])
            .set(uptime);
        let por = clamp_u32_to_i64(por_bps);
        self.torii_sorafs_por_bps
            .with_label_values(&[provider])
            .set(por);
    }
    /// Record SoraFS egress counters and their drift against billing bytes.
    pub fn record_sorafs_egress_reconciliation(
        &self,
        provider: &str,
        billing_bytes: u64,
        gateway_bytes: Option<u64>,
        orchestrator_bytes: Option<u64>,
    ) {
        let billing = u64_to_f64(billing_bytes);
        self.torii_sorafs_egress_bytes
            .with_label_values(&[provider, "billing"])
            .set(billing);
        self.torii_sorafs_egress_drift_ratio
            .with_label_values(&[provider, "billing"])
            .set(0.0);
        for (source, bytes) in [
            ("gateway", gateway_bytes),
            ("orchestrator", orchestrator_bytes),
        ] {
            let Some(observed) = bytes else {
                let _ = self
                    .torii_sorafs_egress_bytes
                    .remove_label_values(&[provider, source]);
                let _ = self
                    .torii_sorafs_egress_drift_ratio
                    .remove_label_values(&[provider, source]);
                continue;
            };
            let observed_value = u64_to_f64(observed);
            self.torii_sorafs_egress_bytes
                .with_label_values(&[provider, source])
                .set(observed_value);
            let denominator = billing.max(1.0);
            let drift = (observed_value - billing).abs() / denominator;
            self.torii_sorafs_egress_drift_ratio
                .with_label_values(&[provider, source])
                .set(drift);
        }
    }
    /// Record a SoraFS Governance DAG publication attempt.
    pub fn record_sorafs_governance_dag_publish(
        &self,
        payload_kind: &str,
        result: &str,
        sink: &str,
        bytes: u64,
        timestamp_seconds: u64,
    ) {
        self.sorafs_governance_dag_publish_total
            .with_label_values(&[payload_kind, result, sink])
            .inc();
        if result == "success" {
            self.sorafs_governance_dag_published_bytes_total
                .with_label_values(&[payload_kind, sink])
                .inc_by(bytes);
            self.sorafs_governance_dag_last_publish_timestamp_seconds
                .with_label_values(&[payload_kind, sink])
                .set(timestamp_seconds);
        }
    }
    /// Set SoraFS Governance DAG publication backlog for a sink.
    pub fn set_sorafs_governance_dag_backlog(&self, sink: &str, backlog: u64) {
        self.sorafs_governance_dag_backlog
            .with_label_values(&[sink])
            .set(backlog);
    }
    /// Set SoraFS Governance DAG head age in seconds for a sink.
    pub fn set_sorafs_governance_dag_head_age_seconds(&self, sink: &str, age_seconds: u64) {
        self.sorafs_governance_dag_head_age_seconds
            .with_label_values(&[sink])
            .set(age_seconds);
    }
    /// Mark the SoraFS orderbook telemetry projection unready without changing
    /// the last complete finalized snapshot.
    pub fn mark_sorafs_orderbook_finalized_projection_unready(&self) {
        let _projection_exposition_guard = self.lock_sorafs_orderbook_projection_exposition();
        self.torii_sorafs_orderbook_finalized_projection_ready
            .set(0);
    }
    /// Record a fail-closed finalized SoraFS orderbook projection failure.
    pub fn record_sorafs_orderbook_finalized_projection_failure(&self, reason: &str) {
        let _projection_exposition_guard = self.lock_sorafs_orderbook_projection_exposition();
        let reason = match reason {
            "telemetry_unavailable" => "telemetry_unavailable",
            "finalized_view_unavailable" => "finalized_view_unavailable",
            "query_failed" => "query_failed",
            "invalid_event_page" => "invalid_event_page",
            "invalid_order_page" => "invalid_order_page",
            "invalid_channel_page" => "invalid_channel_page",
            "arithmetic_overflow" => "arithmetic_overflow",
            "order_capacity_exceeded" => "order_capacity_exceeded",
            "channel_capacity_exceeded" => "channel_capacity_exceeded",
            "projection_mismatch" => "projection_mismatch",
            _ => "other",
        };
        self.torii_sorafs_orderbook_finalized_projection_ready
            .set(0);
        self.torii_sorafs_orderbook_finalized_projection_failures_total
            .with_label_values(&[reason])
            .inc();
    }
    /// Publish one complete immutable finalized SoraFS orderbook projection.
    ///
    /// Every label is selected from a closed vocabulary. Projection mutation
    /// is serialized with exposition and the ready bit is written last, so a
    /// scrape sees either the preceding or succeeding complete snapshot.
    #[allow(clippy::too_many_arguments)]
    pub fn record_sorafs_orderbook_finalized_projection(
        &self,
        finalized_height: u64,
        finalized_timestamp_seconds: u64,
        event_count_deltas: [u64; 8],
        open_depth_gib: [[u64; 2]; 3],
        matcher_lag_seconds: u64,
        settlement_backlog: u64,
        oldest_settlement_age_seconds: u64,
        escrow_runway_seconds: u64,
        book_revision: u64,
        matcher_scan_book_revision: u64,
    ) {
        let _projection_exposition_guard = self.lock_sorafs_orderbook_projection_exposition();
        self.torii_sorafs_orderbook_finalized_projection_ready
            .set(0);
        for (event, delta) in SORAFS_ORDERBOOK_EVENT_LABELS
            .into_iter()
            .zip(event_count_deltas)
        {
            self.torii_sorafs_orderbook_finalized_events_total
                .with_label_values(&[event])
                .inc_by(delta);
        }
        for (tier_index, tier) in SORAFS_ORDERBOOK_TIER_LABELS.into_iter().enumerate() {
            for (side_index, side) in SORAFS_ORDERBOOK_SIDE_LABELS.into_iter().enumerate() {
                self.torii_sorafs_orderbook_open_depth_gib
                    .with_label_values(&[tier, side])
                    .set(open_depth_gib[tier_index][side_index]);
            }
        }
        self.torii_sorafs_orderbook_matcher_lag_seconds
            .set(matcher_lag_seconds);
        self.torii_sorafs_orderbook_settlement_backlog
            .set(settlement_backlog);
        self.torii_sorafs_orderbook_oldest_settlement_age_seconds
            .set(oldest_settlement_age_seconds);
        self.torii_sorafs_orderbook_escrow_runway_seconds
            .set(escrow_runway_seconds);
        self.torii_sorafs_orderbook_finalized_projection_height
            .set(finalized_height);
        self.torii_sorafs_orderbook_finalized_projection_timestamp_seconds
            .set(finalized_timestamp_seconds);
        self.torii_sorafs_orderbook_book_revision.set(book_revision);
        self.torii_sorafs_orderbook_matcher_scan_book_revision
            .set(matcher_scan_book_revision);
        self.torii_sorafs_orderbook_finalized_projection_ready
            .set(1);
    }
    /// Record one SoraFS orderbook API response using bounded labels.
    pub fn record_sorafs_orderbook_api_request(&self, route: &str, is_error: bool) {
        let route = match route {
            "/v1/sorafs/orderbook/orders" => "orders",
            "/v1/sorafs/orderbook/cancel" => "cancel",
            "/v1/sorafs/orderbook/receipts" => "receipts",
            "/v1/sorafs/orderbook/book" => "book",
            "/v1/sorafs/orderbook/trades" => "trades",
            "/v1/sorafs/orderbook/channels" => "channels",
            "/v1/sorafs/orderbook/events" => "events",
            "/v1/sorafs/orderbook/events/stream" => "events_stream",
            "/v1/sorafs/orderbook/events/ws" => "events_ws",
            _ => "other",
        };
        let outcome = if is_error { "error" } else { "success" };
        self.torii_sorafs_orderbook_api_requests_total
            .with_label_values(&[route, outcome])
            .inc();
    }
    /// Record one authenticated SoraFS gateway-compliance control response.
    ///
    /// Caller-provided values are collapsed into fixed vocabularies before reaching Prometheus, so
    /// paths, feed identities, and payload data can never create label cardinality.
    pub fn record_sorafs_gateway_compliance_request(&self, operation: &str, outcome: &str) {
        let operation = match operation {
            "feed" => "feed",
            "status" => "status",
            "stage" => "stage",
            "acknowledge" => "acknowledge",
            "promote" => "promote",
            "rollback" => "rollback",
            _ => "other",
        };
        let outcome = match outcome {
            "success" => "success",
            "authentication_failed" => "authentication_failed",
            "authorization_failed" => "authorization_failed",
            "invalid_request" => "invalid_request",
            "not_found" => "not_found",
            "conflict" => "conflict",
            "unavailable" => "unavailable",
            _ => "internal_error",
        };
        self.torii_sorafs_gateway_compliance_requests_total
            .with_label_values(&[operation, outcome])
            .inc();
    }
    /// Record one SoraFS gateway-compliance serving decision with bounded labels.
    pub fn record_sorafs_gateway_compliance_serving_decision(
        &self,
        subject_kind: &str,
        disposition: &str,
        source: &str,
    ) {
        let subject_kind = match subject_kind {
            "provider" => "provider",
            "manifest_digest" => "manifest_digest",
            "cid" => "cid",
            "url" => "url",
            _ => "other",
        };
        let disposition = match disposition {
            "allow" => "allow",
            "deny" => "deny",
            _ => "other",
        };
        let source = match source {
            "no_match" => "no_match",
            "baseline" => "baseline",
            "accepted_appeal" => "accepted_appeal",
            "legal_safety_hold" => "legal_safety_hold",
            _ => "other",
        };
        self.torii_sorafs_gateway_compliance_serving_decisions_total
            .with_label_values(&[subject_kind, disposition, source])
            .inc();
    }
    /// Record one SoraFS gateway-compliance failure with bounded labels.
    pub fn record_sorafs_gateway_compliance_failure(&self, surface: &str, class: &str) {
        let surface = match surface {
            "control" => "control",
            "serving" => "serving",
            "feed_sync" => "feed_sync",
            "startup" => "startup",
            _ => "other",
        };
        let class = match class {
            "authentication" => "authentication",
            "authorization" => "authorization",
            "invalid_request" => "invalid_request",
            "not_found" => "not_found",
            "conflict" => "conflict",
            "unavailable" => "unavailable",
            "expired_catalog" => "expired_catalog",
            "upstream" => "upstream",
            "persistence" => "persistence",
            _ => "internal",
        };
        self.torii_sorafs_gateway_compliance_failures_total
            .with_label_values(&[surface, class])
            .inc();
    }
    /// Publish one atomic gateway-compliance serving-catalog snapshot.
    ///
    /// The ready bit is cleared before the sequence and expiry change and is
    /// restored last. Prometheus exposition holds the same lock, so a scrape
    /// observes one complete snapshot rather than a mixed transition.
    pub fn record_sorafs_gateway_compliance_serving_catalog(
        &self,
        sequence: Option<u64>,
        valid_until_unix: Option<u64>,
        ready: bool,
    ) {
        let _exposition_guard = self.lock_sorafs_gateway_compliance_exposition();
        self.torii_sorafs_gateway_compliance_ready.set(0);
        self.torii_sorafs_gateway_compliance_serving_catalog_sequence
            .set(sequence.unwrap_or_default());
        self.torii_sorafs_gateway_compliance_serving_catalog_valid_until_seconds
            .set(valid_until_unix.unwrap_or_default());
        if ready {
            self.torii_sorafs_gateway_compliance_ready.set(1);
        }
    }
    /// Mark the serving policy unavailable without publishing partial state.
    pub fn mark_sorafs_gateway_compliance_unready(&self) {
        let _exposition_guard = self.lock_sorafs_gateway_compliance_exposition();
        self.torii_sorafs_gateway_compliance_ready.set(0);
    }
    /// Set the latest SoraFS hedging XOR/USD reference price in micro-USD.
    pub fn set_sorafs_hedging_reference_price_micro_usd(
        &self,
        cluster: &str,
        price_micro_usd: u64,
    ) {
        self.torii_sorafs_hedging_xor_usd_reference_price_micro_usd
            .with_label_values(&[cluster])
            .set(price_micro_usd);
    }
    /// Set SoraFS hedging feed lag in seconds for one source.
    pub fn set_sorafs_hedging_feed_lag_seconds(
        &self,
        cluster: &str,
        source: &str,
        lag_seconds: u64,
    ) {
        self.torii_sorafs_hedging_feed_lag_seconds
            .with_label_values(&[cluster, source])
            .set(lag_seconds);
    }
    /// Set SoraFS hedging feed divergence in basis points for one source.
    pub fn set_sorafs_hedging_feed_divergence_bps(
        &self,
        cluster: &str,
        source: &str,
        divergence_bps: u64,
    ) {
        self.torii_sorafs_hedging_feed_divergence_bps
            .with_label_values(&[cluster, source])
            .set(divergence_bps);
    }
    /// Set SoraFS hedging exposure drift in basis points for one asset.
    pub fn set_sorafs_hedging_exposure_drift_bps(
        &self,
        cluster: &str,
        asset: &str,
        drift_bps: u64,
    ) {
        self.torii_sorafs_hedging_exposure_drift_bps
            .with_label_values(&[cluster, asset])
            .set(drift_bps);
    }
    /// Record a SoraFS billing statement generation attempt.
    pub fn record_sorafs_billing_statement_generation(
        &self,
        cluster: &str,
        account_type: &str,
        succeeded: bool,
    ) {
        self.torii_sorafs_billing_statement_generation_total
            .with_label_values(&[cluster, account_type])
            .inc();
        if !succeeded {
            self.torii_sorafs_billing_statement_failure_total
                .with_label_values(&[cluster, account_type])
                .inc();
        }
    }
    /// Set SoraFS billing statement acknowledgement backlog for a cluster.
    pub fn set_sorafs_billing_statement_ack_backlog(&self, cluster: &str, backlog: u64) {
        self.torii_sorafs_billing_statement_ack_backlog
            .with_label_values(&[cluster])
            .set(backlog);
    }
    /// Set SoraFS billing escrow runway in seconds for one account type.
    pub fn set_sorafs_billing_escrow_runway_seconds(
        &self,
        cluster: &str,
        account_type: &str,
        seconds: u64,
    ) {
        self.torii_sorafs_billing_escrow_runway_seconds
            .with_label_values(&[cluster, account_type])
            .set(seconds);
    }
    /// Publish one complete, reconciled SoraFS reserve projection from a single finalized view.
    pub fn record_sorafs_reserve_finalized_projection(
        &self,
        projection: &SorafsReserveFinalizedProjection,
    ) {
        const STAGES: [&str; 5] = ["active", "warning", "grace", "delinquent", "default"];
        const CUSTODY_STATUSES: [&str; 3] = ["pending", "approved", "rejected"];
        const RECONCILED_STATUSES: [&str; 2] = ["approved", "rejected"];
        self.torii_sorafs_reserve_lifecycle_stage_providers.reset();
        for (stage, count) in STAGES.into_iter().zip(projection.lifecycle_stage_counts) {
            self.torii_sorafs_reserve_lifecycle_stage_providers
                .with_label_values(&[stage])
                .set(count);
        }
        self.torii_sorafs_reserve_credit_draw_micro_xor.reset();
        self.torii_sorafs_reserve_credit_shortfall_micro_xor.reset();
        self.torii_sorafs_reserve_accrued_interest_micro_xor.reset();
        for (index, stage) in STAGES.into_iter().enumerate() {
            self.torii_sorafs_reserve_credit_draw_micro_xor
                .with_label_values(&[stage])
                .set(u128_to_f64(projection.credit_principal_micro_xor[index]));
            self.torii_sorafs_reserve_credit_shortfall_micro_xor
                .with_label_values(&[stage])
                .set(u128_to_f64(projection.credit_shortfall_micro_xor[index]));
            self.torii_sorafs_reserve_accrued_interest_micro_xor
                .with_label_values(&[stage])
                .set(u128_to_f64(projection.accrued_interest_micro_xor[index]));
        }
        self.torii_sorafs_reserve_defaulted_providers
            .set(projection.lifecycle_stage_counts[4]);
        self.torii_sorafs_reserve_appeal_backlog
            .set(projection.open_appeals);
        self.torii_sorafs_reserve_custody_movements.reset();
        for (status, count) in CUSTODY_STATUSES.into_iter().zip(projection.custody_counts) {
            self.torii_sorafs_reserve_custody_movements
                .with_label_values(&[status])
                .set(count);
        }
        self.torii_sorafs_reserve_chain_reconciled_movements.reset();
        for (status, count) in RECONCILED_STATUSES
            .into_iter()
            .zip(projection.chain_reconciled_counts)
        {
            self.torii_sorafs_reserve_chain_reconciled_movements
                .with_label_values(&[status])
                .set(count);
        }
        self.torii_sorafs_reserve_finalized_projection_height
            .set(projection.finalized_height);
        self.torii_sorafs_reserve_finalized_projection_ready.set(1);
    }
    /// Mark the finalized reserve projection unavailable without publishing partial gauges.
    pub fn mark_sorafs_reserve_finalized_projection_unready(&self) {
        self.torii_sorafs_reserve_finalized_projection_ready.set(0);
    }
    /// Record a failed finalized reserve projection attempt.
    pub fn record_sorafs_reserve_finalized_projection_failure(&self) {
        self.mark_sorafs_reserve_finalized_projection_unready();
        self.torii_sorafs_reserve_finalized_projection_failure_total
            .inc();
    }
    /// Record a SoraFS reserve service request outcome.
    pub fn record_sorafs_reserve_service_request(&self, route: &str, result: &str) {
        let route = match route {
            "top_up" | "withdrawal" | "movement_decision" | "credit_draw" | "credit_repay"
            | "appeal" | "appeal_decision" | "policy" | "providers" | "provider" | "movements"
            | "movement" | "appeals" | "appeal_detail" | "events" | "events_stream"
            | "events_ws" => route,
            _ => "unknown",
        };
        let result = match result {
            "accepted" | "ok" | "bad_request" | "unauthorized" | "forbidden" | "not_found"
            | "conflict" | "too_many_requests" | "unavailable" | "error" => result,
            _ => "unknown",
        };
        self.torii_sorafs_reserve_service_requests_total
            .with_label_values(&[route, result])
            .inc();
    }
    /// Increment a SoraFS reserve service rate-limit counter.
    pub fn inc_sorafs_reserve_service_rate_limit(&self, route: &str, reason: &str) {
        let route = match route {
            "top_up" | "withdrawal" | "movement_decision" | "credit_draw" | "credit_repay"
            | "appeal" | "appeal_decision" | "policy" | "providers" | "provider" | "movements"
            | "movement" | "appeals" | "appeal_detail" | "events" | "events_stream"
            | "events_ws" => route,
            _ => "unknown",
        };
        let reason = match reason {
            "quota" | "concurrency" | "authentication" | "ingress" => reason,
            _ => "unknown",
        };
        self.torii_sorafs_reserve_service_rate_limit_total
            .with_label_values(&[route, reason])
            .inc();
    }
    /// Record the latest accepted SoraFS reputation snapshot metrics.
    pub fn record_sorafs_reputation_snapshot(
        &self,
        generated_at_unix: u64,
        observed_at_unix: u64,
        provider_scores: &[(&str, u16, bool)],
    ) {
        let snapshot_age = observed_at_unix.saturating_sub(generated_at_unix);
        self.sorafs_reputation_ingest_lag_seconds.set(snapshot_age);
        self.sorafs_reputation_snapshot_age_seconds
            .set(snapshot_age);
        self.sorafs_reputation_snapshot_generated_at_unix
            .set(generated_at_unix);
        self.sorafs_reputation_provider_count
            .set(u64::try_from(provider_scores.len()).unwrap_or(u64::MAX));
        let mut current_low_score_state = BTreeMap::new();
        let mut low_score_providers = 0_u64;
        for (provider_id, _, low_score) in provider_scores.iter().copied() {
            if low_score {
                low_score_providers = low_score_providers.saturating_add(1);
            }
            current_low_score_state.insert(provider_id.to_owned(), low_score);
        }
        self.sorafs_reputation_low_score_providers
            .set(low_score_providers);
        {
            let mut previous_low_score_state = self
                .sorafs_reputation_low_score_state
                .write()
                .expect("SoraFS reputation low-score state lock poisoned");
            for (provider_id, low_score) in &current_low_score_state {
                let Some(previous_low_score) = previous_low_score_state.get(provider_id) else {
                    continue;
                };
                match (*previous_low_score, *low_score) {
                    (false, true) => self
                        .sorafs_reputation_threshold_crossings_total
                        .with_label_values(&["low_score"])
                        .inc(),
                    (true, false) => self
                        .sorafs_reputation_threshold_crossings_total
                        .with_label_values(&["recovered"])
                        .inc(),
                    _ => {}
                }
            }
            *previous_low_score_state = current_low_score_state;
        }
        let mut ranked_scores = provider_scores.to_vec();
        ranked_scores.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(right.0)));
        let mut next_tracked = BTreeSet::new();
        for (provider_id, score_bps, _) in ranked_scores
            .into_iter()
            .take(SORAFS_REPUTATION_SCORE_LABEL_LIMIT)
        {
            self.sorafs_reputation_score
                .with_label_values(&[provider_id])
                .set(f64::from(score_bps));
            next_tracked.insert(provider_id.to_owned());
        }
        let mut tracked = self
            .sorafs_reputation_score_tracked_providers
            .write()
            .expect("SoraFS reputation score label set lock poisoned");
        let stale_providers: Vec<String> = tracked.difference(&next_tracked).cloned().collect();
        for provider_id in stale_providers {
            let _ = self
                .sorafs_reputation_score
                .remove_label_values(&[provider_id.as_str()]);
        }
        *tracked = next_tracked;
    }
    /// Record payload-free committed reputation runtime status.
    pub fn record_sorafs_reputation_runtime_status(
        &self,
        snapshot: SorafsReputationRuntimeMetricSnapshot,
    ) {
        self.sorafs_reputation_runtime_live
            .set(u64::from(snapshot.runtime.live));
        self.sorafs_reputation_runtime_ready
            .set(u64::from(snapshot.runtime.ready));
        self.sorafs_reputation_runtime_dependencies_ready
            .set(u64::from(snapshot.runtime.external_dependencies_ready));
        self.sorafs_reputation_journal_transaction_submitter_ready
            .set(u64::from(
                snapshot.publication.journal_transaction_submitter_ready,
            ));
        self.sorafs_reputation_runtime_finalized_height
            .set(snapshot.latest_finalized_height);
        self.sorafs_reputation_runtime_consecutive_failures
            .set(snapshot.consecutive_failures);
        self.sorafs_reputation_runtime_material_acknowledged
            .set(u64::from(snapshot.publication.material_acknowledged));
        self.sorafs_reputation_runtime_provider_count
            .set(u64::from(snapshot.provider_count));
    }
    /// Increment one bounded committed reputation runtime tick result.
    pub fn inc_sorafs_reputation_runtime_tick(&self, result: &str) {
        let result = match result {
            "success" | "failure" | "panic" => result,
            _ => "unknown",
        };
        self.sorafs_reputation_runtime_ticks_total
            .with_label_values(&[result])
            .inc();
    }
    /// Record payload-free committed hedging/billing runtime status.
    pub fn record_sorafs_hedging_billing_runtime_status(
        &self,
        snapshot: SorafsHedgingBillingRuntimeMetricSnapshot,
    ) {
        self.sorafs_hedging_billing_runtime_live
            .set(u64::from(snapshot.runtime.live));
        self.sorafs_hedging_billing_runtime_ready
            .set(u64::from(snapshot.runtime.ready));
        self.sorafs_hedging_billing_runtime_dependencies_ready
            .set(u64::from(snapshot.runtime.external_dependencies_ready));
        self.sorafs_hedging_billing_automatic_execution_enabled
            .set(u64::from(snapshot.projection.automatic_execution_enabled));
        self.sorafs_hedging_billing_last_tick_fresh
            .set(u64::from(snapshot.projection.last_tick_fresh));
        self.sorafs_hedging_billing_finalized_projection_ready
            .set(u64::from(snapshot.projection.finalized_projection_ready));
        self.sorafs_hedging_billing_finalized_height
            .set(snapshot.finalized_height);
        self.sorafs_hedging_billing_finalized_head_height
            .set(snapshot.finalized_head_height);
        self.sorafs_hedging_billing_finalized_lag_blocks
            .set(snapshot.finalized_lag_blocks);
        self.sorafs_hedging_billing_next_event_sequence
            .set(snapshot.next_event_sequence);
        self.sorafs_hedging_billing_ready_for_signing
            .set(u64::from(snapshot.ready_for_signing));
        self.sorafs_hedging_billing_ready_for_publication
            .set(u64::from(snapshot.ready_for_publication));
        self.sorafs_hedging_billing_publication_ambiguous
            .set(u64::from(snapshot.publication_ambiguous));
        self.sorafs_hedging_billing_published
            .set(u64::from(snapshot.published));
        self.sorafs_hedging_billing_acknowledged
            .set(u64::from(snapshot.acknowledged));
        self.sorafs_hedging_billing_dead_letter
            .set(u64::from(snapshot.dead_letter));
        self.sorafs_hedging_billing_hedge_intents
            .set(u64::from(snapshot.hedge_intents));
    }
    /// Increment one bounded committed hedging/billing runtime tick result.
    pub fn inc_sorafs_hedging_billing_runtime_tick(&self, result: &str) {
        let result = match result {
            "success" | "failure" | "panic" => result,
            _ => "unknown",
        };
        self.sorafs_hedging_billing_runtime_ticks_total
            .with_label_values(&[result])
            .inc();
    }
    /// Record a rejected SoraFS capacity telemetry window.
    pub fn record_sorafs_capacity_telemetry_reject(&self, provider: &str, reason: &str) {
        self.torii_sorafs_capacity_telemetry_rejections_total
            .with_label_values(&[provider, reason])
            .inc();
    }
    /// Record the latest SoraFS fee projection for `provider`.
    pub fn record_sorafs_fee_projection(&self, provider: &str, fee: &Quantity) {
        let gauge_value = quantity_to_nano_f64(fee);
        self.torii_sorafs_fee_projection_nanos
            .with_label_values(&[provider])
            .set(gauge_value);
    }
    /// Increment the capacity dispute counter for the provided result label.
    pub fn inc_sorafs_disputes(&self, result: &str) {
        self.torii_sorafs_disputes_total
            .with_label_values(&[result])
            .inc();
    }
    /// Increment the repair task counter for a status label.
    pub fn inc_sorafs_repair_tasks(&self, status: &str) {
        self.torii_sorafs_repair_tasks_total
            .with_label_values(&[status])
            .inc();
    }
    /// Observe repair latency in minutes for the supplied outcome label.
    pub fn observe_sorafs_repair_latency(&self, outcome: &str, minutes: f64) {
        self.torii_sorafs_repair_latency_minutes
            .with_label_values(&[outcome])
            .observe(minutes.max(0.0));
    }
    /// Record repair queue depth per provider.
    pub fn record_sorafs_repair_queue_depths(&self, depths: &[(String, u64)]) {
        self.torii_sorafs_repair_queue_depth.reset();
        for (provider, depth) in depths {
            self.torii_sorafs_repair_queue_depth
                .with_label_values(&[provider])
                .set(*depth);
        }
    }
    /// Record the age (seconds) of the oldest queued repair task.
    pub fn set_sorafs_repair_backlog_oldest_age_seconds(&self, age_secs: u64) {
        self.torii_sorafs_repair_backlog_oldest_age_seconds
            .set(age_secs);
    }
    /// Increment the repair lease-expired counter for a given outcome label.
    pub fn inc_sorafs_repair_lease_expired(&self, outcome: &str) {
        self.torii_sorafs_repair_lease_expired_total
            .with_label_values(&[outcome])
            .inc();
    }
    /// Increment the slash proposal counter for a given outcome label.
    pub fn inc_sorafs_slash_proposals(&self, outcome: &str) {
        self.torii_sorafs_slash_proposals_total
            .with_label_values(&[outcome])
            .inc();
    }
    /// Increment the reconciliation run counter for the provided result label.
    pub fn inc_sorafs_reconciliation_runs(&self, result: &str) {
        self.torii_sorafs_reconciliation_runs_total
            .with_label_values(&[result])
            .inc();
    }
    /// Record the reconciliation divergence count for the latest snapshot.
    pub fn set_sorafs_reconciliation_divergence_count(&self, count: u64) {
        self.torii_sorafs_reconciliation_divergence_count.set(count);
    }
    /// Increment the GC run counter for the provided result label.
    pub fn inc_sorafs_gc_runs(&self, result: &str) {
        self.torii_sorafs_gc_runs_total
            .with_label_values(&[result])
            .inc();
    }
    /// Increment the GC eviction counter for the provided reason label.
    pub fn inc_sorafs_gc_evictions(&self, reason: &str) {
        self.torii_sorafs_gc_evictions_total
            .with_label_values(&[reason])
            .inc();
    }
    /// Add freed bytes for GC, labeled by eviction reason.
    pub fn add_sorafs_gc_freed_bytes(&self, reason: &str, bytes: u64) {
        self.torii_sorafs_gc_bytes_freed_total
            .with_label_values(&[reason])
            .inc_by(bytes);
    }
    /// Increment the GC blocked counter for the provided reason label.
    pub fn inc_sorafs_gc_blocked(&self, reason: &str) {
        self.torii_sorafs_gc_blocked_total
            .with_label_values(&[reason])
            .inc();
    }
    /// Record the expired manifest snapshot observed by GC.
    pub fn set_sorafs_gc_expired_snapshot(&self, expired_count: u64, oldest_age_secs: u64) {
        self.torii_sorafs_gc_expired_manifests.set(expired_count);
        self.torii_sorafs_gc_oldest_expired_age_seconds
            .set(oldest_age_secs);
    }
    /// Record the latest storage scheduler snapshot for a provider.
    #[allow(clippy::too_many_arguments)]
    pub fn record_sorafs_storage(
        &self,
        provider: &str,
        bytes_used: u64,
        bytes_capacity: u64,
        provider_ingest_inflight: u64,
        fetch_inflight: u64,
        fetch_bytes_per_sec: u64,
        por_inflight: u64,
        por_samples_success: u64,
        por_samples_failed: u64,
    ) {
        self.torii_sorafs_storage_bytes_used
            .with_label_values(&[provider])
            .set(bytes_used);
        self.torii_sorafs_storage_bytes_capacity
            .with_label_values(&[provider])
            .set(bytes_capacity);
        self.sorafs_provider_ingest_inflight
            .with_label_values(&[provider])
            .set(provider_ingest_inflight);
        self.torii_sorafs_storage_fetch_inflight
            .with_label_values(&[provider])
            .set(fetch_inflight);
        self.torii_sorafs_storage_fetch_bytes_per_sec
            .with_label_values(&[provider])
            .set(fetch_bytes_per_sec);
        self.torii_sorafs_storage_por_inflight
            .with_label_values(&[provider])
            .set(por_inflight);
        self.torii_sorafs_storage_por_samples_success_total
            .with_label_values(&[provider])
            .set(por_samples_success);
        self.torii_sorafs_storage_por_samples_failed_total
            .with_label_values(&[provider])
            .set(por_samples_failed);
        #[cfg(feature = "otel-exporter")]
        {
            let otel = global_sorafs_node_otel();
            otel.record_storage(
                provider,
                bytes_used,
                bytes_capacity,
                por_samples_success,
                por_samples_failed,
            );
        }
    }
    /// Record the PoR ingestion backlog for a manifest/provider pair.
    pub fn record_sorafs_por_ingestion_backlog(
        &self,
        provider: &str,
        manifest: &str,
        pending: u64,
    ) {
        self.torii_sorafs_por_ingest_backlog
            .with_label_values(&[manifest, provider])
            .set(pending);
    }
    /// Record the cumulative PoR ingestion failures for a manifest/provider pair.
    pub fn record_sorafs_por_ingestion_failures(
        &self,
        provider: &str,
        manifest: &str,
        failures_total: u64,
    ) {
        self.torii_sorafs_por_ingest_failures_total
            .with_label_values(&[manifest, provider])
            .set(failures_total);
    }
    /// Record a PoR challenge emitted by the scheduler.
    pub fn record_sorafs_por_scheduler_challenge(&self, forced: bool, duplicate_samples: usize) {
        let result = if forced { "forced" } else { "scheduled" };
        self.torii_sorafs_por_challenges_total
            .with_label_values(&[result])
            .inc();
        if forced {
            self.torii_sorafs_por_forced_challenges_total.inc();
        }
        if duplicate_samples > 0 {
            let duplicate_samples = u64::try_from(duplicate_samples).unwrap_or(u64::MAX);
            self.torii_sorafs_por_sampling_duplicates_total
                .inc_by(duplicate_samples);
        }
    }
    /// Record a PoR scheduler run failure.
    pub fn record_sorafs_por_scheduler_failure(&self) {
        self.torii_sorafs_por_challenges_total
            .with_label_values(&["failed"])
            .inc();
    }
    /// Record the current pin registry snapshot and replication SLA aggregates.
    #[allow(clippy::too_many_arguments)]
    pub fn record_sorafs_registry(
        &self,
        manifests_pending: u64,
        manifests_approved: u64,
        manifests_retired: u64,
        alias_total: u64,
        orders_pending: u64,
        orders_completed: u64,
        orders_expired: u64,
        sla_met: u64,
        sla_missed: u64,
        completion_latencies: &[f64],
        deadline_slack_epochs: &[f64],
    ) {
        self.torii_sorafs_registry_manifests_total
            .with_label_values(&["pending"])
            .set(manifests_pending);
        self.torii_sorafs_registry_manifests_total
            .with_label_values(&["approved"])
            .set(manifests_approved);
        self.torii_sorafs_registry_manifests_total
            .with_label_values(&["retired"])
            .set(manifests_retired);
        self.torii_sorafs_registry_aliases_total.set(alias_total);
        self.torii_sorafs_registry_orders_total
            .with_label_values(&["pending"])
            .set(orders_pending);
        self.torii_sorafs_registry_orders_total
            .with_label_values(&["completed"])
            .set(orders_completed);
        self.torii_sorafs_registry_orders_total
            .with_label_values(&["expired"])
            .set(orders_expired);
        self.torii_sorafs_replication_backlog_total
            .set(orders_pending);
        self.torii_sorafs_replication_sla_total
            .with_label_values(&["met"])
            .set(sla_met);
        self.torii_sorafs_replication_sla_total
            .with_label_values(&["missed"])
            .set(sla_missed);
        self.torii_sorafs_replication_sla_total
            .with_label_values(&["pending"])
            .set(orders_pending);
        record_gauge_stats(
            &self.torii_sorafs_replication_completion_latency_epochs,
            completion_latencies,
        );
        record_gauge_stats(
            &self.torii_sorafs_replication_deadline_slack_epochs,
            deadline_slack_epochs,
        );
    }
    /// Record the O(1), consensus-maintained global SoraFS pin resource summary.
    pub fn record_sorafs_pin_resource_usage(
        &self,
        retained_manifests: u64,
        live_content_bytes: u64,
    ) {
        self.torii_sorafs_pin_retained_manifests
            .set(retained_manifests);
        self.torii_sorafs_pin_live_content_bytes
            .set(live_content_bytes);
    }
    /// Increment the active fetch gauge for the orchestrator.
    pub fn sorafs_orchestrator_fetch_started(&self, manifest_id: &str, region: &str) {
        self.sorafs_orchestrator_active_fetches
            .with_label_values(&[manifest_id, region])
            .inc();
    }
    /// Decrement the active fetch gauge for the orchestrator.
    pub fn sorafs_orchestrator_fetch_finished(&self, manifest_id: &str, region: &str) {
        self.sorafs_orchestrator_active_fetches
            .with_label_values(&[manifest_id, region])
            .dec();
    }
    /// Observe fetch duration (milliseconds) for the orchestrator.
    pub fn record_sorafs_orchestrator_duration(
        &self,
        manifest_id: &str,
        region: &str,
        duration_ms: f64,
    ) {
        self.sorafs_orchestrator_fetch_duration_ms
            .with_label_values(&[manifest_id, region])
            .observe(duration_ms);
    }
    /// Increment orchestrator failure counter for the provided reason.
    pub fn inc_sorafs_orchestrator_failure(&self, manifest_id: &str, region: &str, reason: &str) {
        self.sorafs_orchestrator_fetch_failures_total
            .with_label_values(&[manifest_id, region, reason])
            .inc();
    }
    /// Increment orchestrator retry counter for the given provider.
    pub fn inc_sorafs_orchestrator_retries(
        &self,
        manifest_id: &str,
        provider_id: &str,
        reason: &str,
        count: u64,
    ) {
        if count == 0 {
            return;
        }
        self.sorafs_orchestrator_retries_total
            .with_label_values(&[manifest_id, provider_id, reason])
            .inc_by(count);
    }
    /// Increment orchestrator provider failure counter for the given provider.
    pub fn inc_sorafs_orchestrator_provider_failures(
        &self,
        manifest_id: &str,
        provider_id: &str,
        reason: &str,
        count: u64,
    ) {
        if count == 0 {
            return;
        }
        self.sorafs_orchestrator_provider_failures_total
            .with_label_values(&[manifest_id, provider_id, reason])
            .inc_by(count);
    }
    /// Record per-chunk latency (milliseconds) for successful chunk deliveries.
    pub fn record_sorafs_orchestrator_chunk_latency(
        &self,
        manifest_id: &str,
        provider_id: &str,
        latency_ms: f64,
    ) {
        self.sorafs_orchestrator_chunk_latency_ms
            .with_label_values(&[manifest_id, provider_id])
            .observe(latency_ms);
    }
    /// Increment the orchestrator byte counter for successful chunk deliveries.
    pub fn inc_sorafs_orchestrator_bytes(&self, manifest_id: &str, provider_id: &str, bytes: u64) {
        if bytes == 0 {
            return;
        }
        self.sorafs_orchestrator_bytes_total
            .with_label_values(&[manifest_id, provider_id])
            .inc_by(bytes);
    }
    /// Increment the orchestrator stall counter when chunk latency exceeds the configured cap.
    pub fn inc_sorafs_orchestrator_stall(&self, manifest_id: &str, provider_id: &str) {
        self.sorafs_orchestrator_stalls_total
            .with_label_values(&[manifest_id, provider_id])
            .inc();
    }
    /// Increment the transport event counter for the orchestrator.
    pub fn inc_sorafs_orchestrator_transport_event(
        &self,
        region: &str,
        protocol: &str,
        event: &str,
        reason: &str,
    ) {
        self.sorafs_orchestrator_transport_events_total
            .with_label_values(&[region, protocol, event, reason])
            .inc();
    }
    /// Record an anonymity policy event for the orchestrator.
    pub fn record_sorafs_orchestrator_policy_event(
        &self,
        stage: &str,
        region: &str,
        outcome: &str,
        reason: &str,
    ) {
        self.sorafs_orchestrator_policy_events_total
            .with_label_values(&[region, stage, outcome, reason])
            .inc();
    }
    /// Observe the PQ-capable relay selection ratio for a session.
    pub fn record_sorafs_orchestrator_pq_ratio(&self, stage: &str, region: &str, ratio: f64) {
        self.sorafs_orchestrator_pq_ratio
            .with_label_values(&[region, stage])
            .observe(ratio.clamp(0.0, 1.0));
    }
    /// Observe the PQ-capable relay candidate ratio for a session.
    pub fn record_sorafs_orchestrator_pq_candidate_ratio(
        &self,
        stage: &str,
        region: &str,
        ratio: f64,
    ) {
        self.sorafs_orchestrator_pq_candidate_ratio
            .with_label_values(&[region, stage])
            .observe(ratio.clamp(0.0, 1.0));
    }
    /// Observe the PQ policy shortfall ratio for a session.
    pub fn record_sorafs_orchestrator_pq_deficit_ratio(
        &self,
        stage: &str,
        region: &str,
        ratio: f64,
    ) {
        self.sorafs_orchestrator_pq_deficit_ratio
            .with_label_values(&[region, stage])
            .observe(ratio.clamp(0.0, 1.0));
    }
    /// Observe the classical relay selection ratio for a session.
    pub fn record_sorafs_orchestrator_classical_ratio(
        &self,
        stage: &str,
        region: &str,
        ratio: f64,
    ) {
        self.sorafs_orchestrator_classical_ratio
            .with_label_values(&[region, stage])
            .observe(ratio.clamp(0.0, 1.0));
    }
    /// Observe the classical relay selection count for a session.
    pub fn record_sorafs_orchestrator_classical_selected(
        &self,
        stage: &str,
        region: &str,
        selected: u64,
    ) {
        const MAX_EXACT_INT: u64 = 1u64 << f64::MANTISSA_DIGITS;
        let clamped = selected.min(MAX_EXACT_INT - 1);
        #[allow(clippy::cast_precision_loss)]
        let value = clamped as f64;
        self.sorafs_orchestrator_classical_selected
            .with_label_values(&[region, stage])
            .observe(value);
    }
    fn update_taikai_snapshot<F>(&self, cluster: &str, stream: &str, update: F)
    where
        F: FnOnce(&mut TaikaiIngestSnapshotInternal),
    {
        let key = (cluster.to_owned(), stream.to_owned());
        let evicted = if let (Ok(mut snapshots), Ok(mut order)) = (
            self.taikai_ingest_snapshots.write(),
            self.taikai_ingest_snapshot_order.write(),
        ) {
            let mut evicted = None;
            if !snapshots.contains_key(&key) {
                if snapshots.len() >= TAIKAI_INGEST_SNAPSHOT_CAP
                    && let Some(evicted_key) = order.pop_front()
                {
                    evicted = snapshots
                        .remove(&evicted_key)
                        .map(|snapshot| (evicted_key, snapshot));
                }
                order.push_back(key.clone());
            } else if let Some(position) = order.iter().position(|entry| entry == &key) {
                order.remove(position);
                order.push_back(key.clone());
            }
            let entry = snapshots
                .entry(key)
                .or_insert_with(TaikaiIngestSnapshotInternal::default);
            update(entry);
            evicted
        } else {
            None
        };
        if let Some(((cluster, stream), snapshot)) = evicted {
            self.remove_taikai_ingest_metric_children(&cluster, &stream, &snapshot);
        }
    }
    fn remove_taikai_ingest_metric_children(
        &self,
        cluster: &str,
        stream: &str,
        snapshot: &TaikaiIngestSnapshotInternal,
    ) {
        let _ = self
            .taikai_ingest_segment_latency_ms
            .remove_label_values(&[cluster, stream]);
        let _ = self
            .taikai_ingest_live_edge_drift_ms
            .remove_label_values(&[cluster, stream]);
        let _ = self
            .taikai_ingest_live_edge_drift_signed_ms
            .remove_label_values(&[cluster, stream]);
        for reason in snapshot.error_totals.keys() {
            let _ = self
                .taikai_ingest_errors_total
                .remove_label_values(&[cluster, stream, reason]);
        }
    }
    /// Record the latest encoder-to-ingest latency for the given stream.
    pub fn record_taikai_ingest_latency_snapshot(
        &self,
        cluster: &str,
        stream: &str,
        latency_ms: u32,
    ) {
        self.update_taikai_snapshot(cluster, stream, |snapshot| {
            snapshot.last_latency_ms = Some(latency_ms);
        });
    }
    /// Record the latest live-edge drift for the given stream.
    pub fn record_taikai_ingest_drift_snapshot(&self, cluster: &str, stream: &str, drift_ms: i32) {
        self.update_taikai_snapshot(cluster, stream, |snapshot| {
            snapshot.last_live_edge_drift_ms = Some(drift_ms);
        });
    }
    /// Record an ingest error for the given stream and reason.
    pub fn record_taikai_ingest_error_snapshot(&self, cluster: &str, stream: &str, reason: &str) {
        let reason = reason.to_owned();
        let mut evicted_reason = None;
        self.update_taikai_snapshot(cluster, stream, |snapshot| {
            if snapshot.error_totals.contains_key(&reason)
                || snapshot.error_totals.len() < TAIKAI_INGEST_ERROR_REASON_CAP
            {
                *snapshot.error_totals.entry(reason).or_insert(0) += 1;
                return;
            }
            // Evict the lexicographically earliest entry to bound memory usage.
            if let Some((evicted, _)) = snapshot.error_totals.pop_first() {
                evicted_reason = Some(evicted);
                snapshot.error_totals.insert(reason, 1);
            }
        });
        if let Some(reason) = evicted_reason {
            let _ = self
                .taikai_ingest_errors_total
                .remove_label_values(&[cluster, stream, &reason]);
        }
    }
    fn record_taikai_alias_rotation_snapshot(&self, snapshot: TaikaiAliasRotationSnapshotArgs<'_>) {
        let key = (
            snapshot.cluster.to_owned(),
            snapshot.event.to_owned(),
            snapshot.stream.to_owned(),
        );
        let evicted_labels = if let (Ok(mut snapshots), Ok(mut order)) = (
            self.taikai_alias_rotation_snapshots.write(),
            self.taikai_alias_rotation_snapshot_order.write(),
        ) {
            let mut evicted_labels = None;
            if !snapshots.contains_key(&key) {
                if snapshots.len() >= TAIKAI_ALIAS_ROTATION_SNAPSHOT_CAP
                    && let Some(evicted_key) = order.pop_front()
                    && let Some(evicted) = snapshots.remove(&evicted_key)
                {
                    evicted_labels = Some((
                        evicted_key.0,
                        evicted_key.1,
                        evicted_key.2,
                        evicted.alias_namespace,
                        evicted.alias_name,
                    ));
                }
                order.push_back(key.clone());
            } else if let Some(position) = order.iter().position(|entry| entry == &key) {
                order.remove(position);
                order.push_back(key.clone());
            }
            let entry = snapshots
                .entry(key.clone())
                .or_insert_with(TaikaiAliasRotationSnapshotInternal::default);
            if entry.rotations_total > 0
                && (entry.alias_namespace != snapshot.alias_namespace
                    || entry.alias_name != snapshot.alias_name)
            {
                evicted_labels = Some((
                    key.0,
                    key.1,
                    key.2,
                    entry.alias_namespace.clone(),
                    entry.alias_name.clone(),
                ));
            }
            snapshot
                .alias_namespace
                .clone_into(&mut entry.alias_namespace);
            snapshot.alias_name.clone_into(&mut entry.alias_name);
            entry.window_start_sequence = snapshot.window_start_sequence;
            entry.window_end_sequence = snapshot.window_end_sequence;
            snapshot
                .manifest_digest_hex
                .clone_into(&mut entry.manifest_digest_hex);
            entry.rotations_total = entry.rotations_total.saturating_add(1);
            entry.last_updated_unix = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            evicted_labels
        } else {
            None
        };
        if let Some((cluster, event, stream, alias_namespace, alias_name)) = evicted_labels {
            let _ = self.taikai_trm_alias_rotations_total.remove_label_values(&[
                &cluster,
                &event,
                &stream,
                &alias_namespace,
                &alias_name,
            ]);
        }
    }
    /// Snapshot the current Taikai ingest telemetry for status payloads
    /// (bounded by `TAIKAI_INGEST_SNAPSHOT_CAP` streams).
    pub fn taikai_ingest_status(&self) -> Vec<TaikaiIngestStatus> {
        self.taikai_ingest_snapshots
            .read()
            .map(|guard| {
                guard
                    .iter()
                    .map(|((cluster, stream), snapshot)| TaikaiIngestStatus {
                        cluster: cluster.clone(),
                        stream: stream.clone(),
                        last_latency_ms: snapshot.last_latency_ms,
                        last_live_edge_drift_ms: snapshot.last_live_edge_drift_ms,
                        error_counts: snapshot
                            .error_totals
                            .iter()
                            .map(|(reason, total)| TaikaiIngestErrorCounter {
                                reason: reason.clone(),
                                total: *total,
                            })
                            .collect(),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
    /// Snapshot the current alias rotation telemetry for status payloads
    /// (bounded by `TAIKAI_ALIAS_ROTATION_SNAPSHOT_CAP` entries).
    pub fn taikai_alias_rotation_status(&self) -> Vec<TaikaiAliasRotationStatus> {
        self.taikai_alias_rotation_snapshots
            .read()
            .map(|guard| {
                guard
                    .iter()
                    .map(
                        |((cluster, event, stream), snapshot)| TaikaiAliasRotationStatus {
                            cluster: cluster.clone(),
                            event: event.clone(),
                            stream: stream.clone(),
                            alias_namespace: snapshot.alias_namespace.clone(),
                            alias_name: snapshot.alias_name.clone(),
                            window_start_sequence: snapshot.window_start_sequence,
                            window_end_sequence: snapshot.window_end_sequence,
                            manifest_digest_hex: snapshot.manifest_digest_hex.clone(),
                            rotations_total: snapshot.rotations_total,
                            last_updated_unix: snapshot.last_updated_unix,
                        },
                    )
                    .collect()
            })
            .unwrap_or_default()
    }
    /// Observe encoder-to-ingest latency for a Taikai segment.
    pub fn observe_taikai_ingest_latency(&self, cluster: &str, stream: &str, latency_ms: u32) {
        self.taikai_ingest_segment_latency_ms
            .with_label_values(&[cluster, stream])
            .observe(f64::from(latency_ms));
        self.record_taikai_ingest_latency_snapshot(cluster, stream, latency_ms);
    }
    /// Observe live-edge drift for a Taikai segment (absolute histogram + signed gauge).
    pub fn observe_taikai_live_edge_drift(&self, cluster: &str, stream: &str, drift_ms: i32) {
        let magnitude = drift_ms.unsigned_abs();
        self.taikai_ingest_live_edge_drift_ms
            .with_label_values(&[cluster, stream])
            .observe(f64::from(magnitude));
        self.taikai_ingest_live_edge_drift_signed_ms
            .with_label_values(&[cluster, stream])
            .set(f64::from(drift_ms));
        self.record_taikai_ingest_drift_snapshot(cluster, stream, drift_ms);
    }
    /// Increment the Taikai ingest error counter.
    pub fn inc_taikai_ingest_error(&self, cluster: &str, stream: &str, reason: &str) {
        self.taikai_ingest_errors_total
            .with_label_values(&[cluster, stream, reason])
            .inc();
        self.record_taikai_ingest_error_snapshot(cluster, stream, reason);
    }
    /// Record a Taikai alias rotation event derived from a routing manifest.
    #[allow(clippy::too_many_arguments)]
    pub fn record_taikai_alias_rotation(
        &self,
        cluster: &str,
        event: &str,
        stream: &str,
        alias_namespace: &str,
        alias_name: &str,
        window_start_sequence: u64,
        window_end_sequence: u64,
        manifest_digest_hex: &str,
    ) {
        self.taikai_trm_alias_rotations_total
            .with_label_values(&[cluster, event, stream, alias_namespace, alias_name])
            .inc();
        self.record_taikai_alias_rotation_snapshot(TaikaiAliasRotationSnapshotArgs {
            cluster,
            event,
            stream,
            alias_namespace,
            alias_name,
            window_start_sequence,
            window_end_sequence,
            manifest_digest_hex,
        });
    }
    /// Record Taikai viewer rebuffer events.
    pub fn inc_taikai_viewer_rebuffer(&self, cluster: &str, stream: &str, count: u64) {
        if count == 0 {
            return;
        }
        self.taikai_viewer_rebuffer_events_total
            .with_label_values(&[cluster, stream])
            .inc_by(count);
    }
    /// Record Taikai viewer playback segments.
    pub fn inc_taikai_viewer_segments(&self, cluster: &str, stream: &str, count: u64) {
        if count == 0 {
            return;
        }
        self.taikai_viewer_playback_segments_total
            .with_label_values(&[cluster, stream])
            .inc_by(count);
    }
    /// Observe CEK fetch duration for a Taikai lane.
    pub fn observe_taikai_viewer_cek_fetch_duration(
        &self,
        cluster: &str,
        lane: &str,
        duration_ms: u32,
    ) {
        self.taikai_viewer_cek_fetch_duration_ms
            .with_label_values(&[cluster, lane])
            .observe(f64::from(duration_ms));
    }
    /// Update PQ circuit health percentage for a cluster.
    pub fn set_taikai_viewer_pq_health(&self, cluster: &str, percent: f64) {
        self.taikai_viewer_pq_circuit_health
            .with_label_values(&[cluster])
            .set(percent.clamp(0.0, 100.0));
    }
    /// Update the seconds elapsed since the last CEK rotation for a lane.
    pub fn set_taikai_viewer_cek_rotation_age(&self, lane: &str, seconds: u64) {
        self.taikai_viewer_cek_rotation_seconds_ago
            .with_label_values(&[lane])
            .set(seconds);
    }
    /// Increment the Taikai viewer alert firing counter.
    pub fn inc_taikai_viewer_alert_firing(&self, cluster: &str, alertname: &str) {
        self.taikai_viewer_alerts_firing_total
            .with_label_values(&[cluster, alertname])
            .inc();
    }
    /// Increment the anonymity policy brownout counter for the session.
    pub fn inc_sorafs_orchestrator_brownout(&self, stage: &str, region: &str, reason: &str) {
        self.sorafs_orchestrator_brownouts_total
            .with_label_values(&[region, stage, reason])
            .inc();
    }
    /// Update the configured base payout (nano XOR) used by SoraNet rewards.
    pub fn set_soranet_reward_base_payout(&self, nanos: u128) {
        let value = u64::try_from(nanos).unwrap_or(u64::MAX);
        self.soranet_reward_base_payout_nanos.set(value);
    }
    /// Record a SoraNet reward event and associated payout volume.
    pub fn record_soranet_reward(&self, relay: &str, nanos: u128, result: &str) {
        self.soranet_reward_events_total
            .with_label_values(&[relay, result])
            .inc();
        if nanos > 0 {
            let amount = u64::try_from(nanos).unwrap_or(u64::MAX);
            self.soranet_reward_payout_nanos_total
                .with_label_values(&[relay, result])
                .inc_by(amount);
        }
    }
    /// Record a SoraNet reward skip with the provided reason label.
    pub fn record_soranet_reward_skip(&self, relay: &str, reason: &str) {
        self.soranet_reward_skips_total
            .with_label_values(&[relay, reason])
            .inc();
    }
    /// Record a SoraNet dispute adjustment.
    pub fn record_soranet_adjustment(&self, relay: &str, nanos: u128, kind: &str) {
        if nanos == 0 {
            return;
        }
        let amount = u64::try_from(nanos).unwrap_or(u64::MAX);
        self.soranet_reward_adjustment_nanos_total
            .with_label_values(&[relay, kind])
            .inc_by(amount);
    }
    /// Increment the SoraNet dispute lifecycle counter for the provided action.
    pub fn inc_soranet_dispute(&self, action: &str) {
        self.soranet_reward_disputes_total
            .with_label_values(&[action])
            .inc();
    }
    /// Record a SoraNet PoW revocation-store failure.
    pub fn inc_soranet_pow_revocation_store(&self, reason: &str) {
        self.soranet_pow_revocation_store_total
            .with_label_values(&[reason])
            .inc();
    }
    /// Record proof endpoint request outcome and payload size.
    pub fn record_torii_proof_request(
        &self,
        endpoint: &str,
        outcome: &str,
        bytes: u64,
        duration: Duration,
    ) {
        self.torii_proof_requests_total
            .with_label_values(&[endpoint, outcome])
            .inc();
        self.torii_proof_request_duration_seconds
            .with_label_values(&[endpoint, outcome])
            .observe(duration.as_secs_f64());
        if bytes > 0 {
            self.torii_proof_response_bytes_total
                .with_label_values(&[endpoint, outcome])
                .inc_by(bytes);
        }
    }
    /// Record explorer endpoint request outcome and latency.
    pub fn record_torii_explorer_request(&self, endpoint: &str, outcome: &str, duration: Duration) {
        self.torii_explorer_requests_total
            .with_label_values(&[endpoint, outcome])
            .inc();
        self.torii_explorer_request_duration_seconds
            .with_label_values(&[endpoint, outcome])
            .observe(duration.as_secs_f64());
    }
    /// Increment proof endpoint cache hit counter.
    pub fn inc_torii_proof_cache_hit(&self, endpoint: &str) {
        self.torii_proof_cache_hits_total
            .with_label_values(&[endpoint])
            .inc();
    }
    /// Increment proof throttling counter for the provided endpoint label.
    pub fn inc_torii_proof_throttled(&self, endpoint: &str) {
        self.torii_proof_throttled_total
            .with_label_values(&[endpoint])
            .inc();
    }
    /// Record alias cache observations emitted by the SoraFS gateway.
    pub fn record_sorafs_alias_cache(&self, result: &str, reason: &str, age_secs: f64) {
        self.torii_sorafs_alias_cache_refresh_total
            .with_label_values(&[result, reason])
            .inc();
        self.torii_sorafs_alias_cache_age_seconds.observe(age_secs);
    }
    /// Update gateway TLS state gauges.
    pub fn set_sorafs_tls_state(&self, ech_enabled: bool, expiry: Option<Duration>) {
        let expiry_secs = expiry.map_or(0.0, |duration| duration.as_secs_f64());
        self.torii_sorafs_tls_cert_expiry_seconds.set(expiry_secs);
        self.torii_sorafs_tls_ech_enabled
            .set(i64::from(u8::from(ech_enabled)));
    }
    /// Record the outcome of a gateway TLS renewal attempt.
    pub fn record_sorafs_tls_renewal(&self, result: &str) {
        self.torii_sorafs_tls_renewal_total
            .with_label_values(&[result])
            .inc();
    }
    /// Publish the active SoraFS gateway fixture version gauge.
    pub fn set_sorafs_gateway_fixture_version(&self, version: &str) {
        self.torii_sorafs_gateway_fixture_version.reset();
        self.torii_sorafs_gateway_fixture_version
            .with_label_values(&[version])
            .set(1);
    }
    /// Increment canonical active-request accounting for a SoraFS gateway route.
    pub fn start_sorafs_gateway_request(&self, labels: SorafsGatewayRequestMetricLabels<'_>) {
        self.sorafs_gateway_active
            .with_label_values(&[
                labels.endpoint,
                labels.method,
                labels.variant,
                labels.chunker,
                labels.profile,
            ])
            .inc();
        #[cfg(feature = "otel-exporter")]
        global_sorafs_gateway_otel().request_started_detailed(labels);
    }
    /// Complete canonical active-request accounting and record response/TTFB metrics.
    pub fn finish_sorafs_gateway_request(
        &self,
        labels: SorafsGatewayResponseMetricLabels<'_>,
        ttfb_ms: f64,
    ) {
        let request = labels.request;
        let request_labels = [
            request.endpoint,
            request.method,
            request.variant,
            request.chunker,
            request.profile,
        ];
        self.sorafs_gateway_active
            .with_label_values(&request_labels)
            .dec();
        let status = labels.status.to_string();
        let response_labels = [
            request.endpoint,
            request.method,
            request.variant,
            request.chunker,
            request.profile,
            labels.result,
            status.as_str(),
            labels.error_code,
        ];
        self.sorafs_gateway_responses_total
            .with_label_values(&response_labels)
            .inc();
        self.sorafs_gateway_ttfb_ms
            .with_label_values(&response_labels)
            .observe(ttfb_ms);
        #[cfg(feature = "otel-exporter")]
        {
            let otel = global_sorafs_gateway_otel();
            otel.request_completed_detailed(labels);
            otel.record_ttfb_detailed(labels, ttfb_ms);
        }
    }
    /// Record one canonical SoraFS proof-verification outcome and duration.
    pub fn record_sorafs_gateway_proof_verification(
        &self,
        profile_version: &str,
        result: &str,
        error_code: &str,
        duration_ms: f64,
    ) {
        let labels = [profile_version, result, error_code];
        self.sorafs_gateway_proof_verifications_total
            .with_label_values(&labels)
            .inc();
        self.sorafs_gateway_proof_duration_ms
            .with_label_values(&labels)
            .observe(duration_ms);
        #[cfg(feature = "otel-exporter")]
        global_sorafs_gateway_otel().record_proof_verification(
            profile_version,
            result,
            error_code,
            duration_ms,
        );
    }
    /// Increment the in-flight proof stream gauge for a given proof kind.
    pub fn inc_sorafs_proof_stream_inflight(&self, kind: &str) {
        self.torii_sorafs_proof_stream_inflight
            .with_label_values(&[kind])
            .inc();
    }
    /// Decrement the in-flight proof stream gauge for a given proof kind.
    pub fn dec_sorafs_proof_stream_inflight(&self, kind: &str) {
        self.torii_sorafs_proof_stream_inflight
            .with_label_values(&[kind])
            .dec();
    }
    /// Record a proof stream outcome and optional latency.
    pub fn record_sorafs_proof_stream_event(
        &self,
        kind: &str,
        result: &str,
        reason: Option<&str>,
        provider_id: Option<&str>,
        tier: Option<&str>,
        latency_ms: Option<f64>,
    ) {
        let reason_label = reason.unwrap_or("ok");
        self.torii_sorafs_proof_stream_events_total
            .with_label_values(&[kind, result, reason_label])
            .inc();
        if let Some(value) = latency_ms {
            self.torii_sorafs_proof_stream_latency_ms
                .with_label_values(&[kind])
                .observe(value);
        }
        let _ = (provider_id, tier);
    }
    /// Record proof-health alert metrics for the given provider.
    #[allow(clippy::too_many_arguments)]
    pub fn record_sorafs_proof_health_alert(
        &self,
        provider_id: &str,
        trigger: &str,
        penalty_applied: bool,
        pdp_failures: u32,
        potr_breaches: u32,
        penalty_nano: u128,
        cooldown_active: bool,
        window_end_epoch: u64,
    ) {
        let penalty_label = if penalty_applied {
            "penalty_applied"
        } else {
            "suppressed"
        };
        self.torii_sorafs_proof_health_alerts_total
            .with_label_values(&[provider_id, trigger, penalty_label])
            .inc();
        self.torii_sorafs_proof_health_pdp_failures
            .with_label_values(&[provider_id])
            .set(i64::from(pdp_failures));
        self.torii_sorafs_proof_health_potr_breaches
            .with_label_values(&[provider_id])
            .set(i64::from(potr_breaches));
        let penalty_value =
            u64::try_from(penalty_nano.min(u128::from(u64::MAX))).expect("clamped to u64");
        self.torii_sorafs_proof_health_penalty_nano
            .with_label_values(&[provider_id])
            .set(penalty_value);
        self.torii_sorafs_proof_health_window_end_epoch
            .with_label_values(&[provider_id])
            .set(window_end_epoch);
        self.torii_sorafs_proof_health_cooldown
            .with_label_values(&[provider_id])
            .set(i64::from(cooldown_active));
    }
    /// Record chunk-range fetch metadata emitted by the SoraFS gateway.
    #[allow(clippy::too_many_arguments)]
    pub fn record_sorafs_chunk_range(
        &self,
        endpoint: &str,
        status: u16,
        bytes: u64,
        chunker: Option<&str>,
        profile: Option<&str>,
        provider_id: Option<&str>,
        tier: Option<&str>,
        latency_ms: Option<f64>,
    ) {
        let status_label = status.to_string();
        self.torii_sorafs_chunk_range_requests_total
            .with_label_values(&[endpoint, status_label.as_str()])
            .inc();
        if bytes > 0 {
            self.torii_sorafs_chunk_range_bytes_total
                .with_label_values(&[endpoint])
                .inc_by(bytes);
        }
        let _ = (chunker, profile, provider_id, tier, latency_ms);
    }
    /// Set the provider range capability counters for the supplied feature label.
    pub fn set_sorafs_provider_range_capability(&self, feature: &str, count: i64) {
        self.torii_sorafs_provider_range_capability_total
            .with_label_values(&[feature])
            .set(count);
    }
    /// Record one bounded committed routing-authority cache outcome.
    pub fn inc_sorafs_routing_authority_cache(&self, outcome: &str) {
        let outcome = match outcome {
            "hit" | "rebuild" | "rebuild_failure" | "stale_rejected" | "fork_rejected" => outcome,
            _ => "invalid",
        };
        self.torii_sorafs_routing_authority_cache_total
            .with_label_values(&[outcome])
            .inc();
    }
    /// Record a throttle event triggered while serving range fetch requests.
    pub fn inc_sorafs_range_fetch_throttle(&self, reason: &str) {
        self.torii_sorafs_range_fetch_throttle_events_total
            .with_label_values(&[reason])
            .inc();
    }
    /// Increment the active range fetch concurrency gauge.
    pub fn inc_sorafs_range_fetch_concurrency(&self) {
        self.torii_sorafs_range_fetch_concurrency_current.inc();
    }
    /// Decrement the active range fetch concurrency gauge.
    pub fn dec_sorafs_range_fetch_concurrency(&self) {
        self.torii_sorafs_range_fetch_concurrency_current.dec();
    }
    /// Record a GAR policy violation observed by the gateway.
    pub fn record_sorafs_gar_violation(&self, reason: &str, detail: &str) {
        self.torii_sorafs_gar_violations_total
            .with_label_values(&[reason, detail])
            .inc();
    }
    /// Record a deterministic gateway refusal emitted by Torii.
    pub fn record_sorafs_gateway_refusal(
        &self,
        status: u16,
        reason: &str,
        profile: &str,
        provider_id: &str,
        scope: &str,
    ) {
        self.torii_sorafs_gateway_refusals_total
            .with_label_values(&[reason, profile, provider_id, scope])
            .inc();
        let _ = status;
    }
    /// Publish metadata about the canonical SoraFS gateway fixture bundle.
    pub fn set_sorafs_gateway_fixture_metadata(
        &self,
        version: &str,
        profile: &str,
        digest_hex: &str,
        released_at_unix: u64,
    ) {
        let gauge_value = i64::try_from(released_at_unix).unwrap_or(i64::MAX);
        self.torii_sorafs_gateway_fixture_info
            .with_label_values(&[version, profile, digest_hex])
            .set(gauge_value);
    }
    /// Convert the current [`Metrics`] into a Prometheus-readable format.
    ///
    /// # Errors
    /// - If [`Encoder`] fails to encode the data
    /// - If the buffer produced by [`Encoder`] causes [`String::from_utf8`] to fail.
    pub fn try_to_string(&self) -> eyre::Result<String> {
        let _projection_exposition_guard = self.lock_sorafs_orderbook_projection_exposition();
        let _gateway_compliance_exposition_guard = self.lock_sorafs_gateway_compliance_exposition();
        let mut buffer = Vec::new();
        let encoder = prometheus::TextEncoder::new();
        let metric_families = self.registry.gather();
        Encoder::encode(&encoder, &metric_families, &mut buffer)?;
        Ok(String::from_utf8(buffer)?)
    }
}
include!("metrics/tail_projection.rs");
#[cfg(test)]
#[path = "metrics/test.rs"]
mod test;
