//! [`Metrics`] and [`Status`]-related logic and functions.
#![allow(clippy::doc_markdown)]

use core::{
    convert::{TryFrom, TryInto},
    ops::Deref,
};
#[cfg(feature = "otel-exporter")]
use std::{collections::HashMap, sync::Mutex};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{
        Arc, OnceLock, RwLock,
        atomic::{AtomicBool, AtomicU64 as StdAtomicU64, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
    vec::Vec,
};

use iroha_config::{kura::FsyncMode, parameters::actual::ConfidentialGas as ActualConfidentialGas};
use iroha_data_model::{
    block::consensus::PERMISSIONED_TAG,
    da::types::DaRentQuote,
    offline::{
        AndroidIntegrityPolicy, OfflineTransferRejectionPlatform, OfflineTransferRejectionReason,
    },
    soranet::privacy_metrics::{
        SoranetPrivacyBucketMetricsV1, SoranetPrivacyModeV1, SoranetPrivacySuppressionReasonV1,
    },
};
use iroha_schema::{Ident, IntoSchema, MetaMap, Metadata, TypeId, UnnamedFieldsMeta};
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

use crate::privacy::PrivacyDrainSnapshot;

/// Type for reporting amount of dropped messages for sumeragi
pub type DroppedMessagesCounter = IntCounter;
/// Type for reporting view change index of current round
pub type ViewChangesGauge = GenericGauge<AtomicU64>;

/// Thin wrapper around duration that `impl`s [`Default`]
#[derive(Debug, Clone, Copy)]
pub struct Uptime(pub Duration);

type MicropaymentSampleSink = Arc<
    dyn Fn(&str, MicropaymentCreditSnapshot, MicropaymentTicketCounters) + Send + Sync + 'static,
>;

impl Default for Uptime {
    fn default() -> Self {
        Self(Duration::from_millis(0))
    }
}

impl norito::core::NoritoSerialize for Uptime {
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
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
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
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

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for LayerWidthBuckets {
    fn write_json(&self, out: &mut String) {
        out.push('[');
        for (idx, bucket) in self.0.iter().enumerate() {
            if idx > 0 {
                out.push(',');
            }
            JsonSerialize::json_serialize(bucket, out);
        }
        out.push(']');
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for LayerWidthBuckets {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let values = Vec::<u64>::json_deserialize(parser)?;
        if values.len() != 8 {
            return Err(norito::json::Error::Message(format!(
                "expected 8 layer width buckets, got {}",
                values.len()
            )));
        }
        let mut buckets = [0_u64; 8];
        buckets.copy_from_slice(&values);
        Ok(Self(buckets))
    }
}

fn encode_hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
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
                .init();

            let duration_ms = meter
                .f64_histogram("sorafs.fetch.duration_ms")
                .with_description("Completed fetch duration in milliseconds.")
                .with_unit("ms")
                .init();

            let failures_total = meter
                .u64_counter("sorafs.fetch.failures_total")
                .with_description("Total number of orchestrator failures grouped by reason.")
                .init();

            let retries_total = meter
                .u64_counter("sorafs.fetch.retries_total")
                .with_description("Retry attempts triggered during orchestrator sessions.")
                .init();

            let provider_failures_total = meter
                .u64_counter("sorafs.fetch.provider_failures_total")
                .with_description("Provider-level failures observed while fetching chunks.")
                .init();
            let chunk_latency_ms = meter
                .f64_histogram("sorafs.fetch.chunk_latency_ms")
                .with_description("Latency per chunk fetch served by the orchestrator.")
                .with_unit("ms")
                .init();
            let bytes_total = meter
                .u64_counter("sorafs.fetch.bytes_total")
                .with_description("Total bytes delivered by the orchestrator grouped by provider.")
                .init();
            let stalls_total = meter
                .u64_counter("sorafs.fetch.stalls_total")
                .with_description("Chunks exceeding the configured latency cap.")
                .init();
            let policy_events_total = meter
                .u64_counter("sorafs.fetch.anonymity_events_total")
                .with_description("Anonymity policy events grouped by stage/outcome/reason.")
                .init();
            let pq_ratio = meter
                .f64_histogram("sorafs.fetch.pq_ratio")
                .with_description("PQ-capable relay ratio observed per session.")
                .with_unit("ratio")
                .init();
            let pq_candidate_ratio = meter
                .f64_histogram("sorafs.fetch.pq_candidate_ratio")
                .with_description("PQ-capable relay candidate ratio observed per session.")
                .with_unit("ratio")
                .init();
            let pq_deficit_ratio = meter
                .f64_histogram("sorafs.fetch.pq_deficit_ratio")
                .with_description("PQ policy shortfall ratio observed per session.")
                .with_unit("ratio")
                .init();
            let classical_ratio = meter
                .f64_histogram("sorafs.fetch.classical_ratio")
                .with_description("Classical relay selection ratio observed per session.")
                .with_unit("ratio")
                .init();
            let classical_selected = meter
                .f64_histogram("sorafs.fetch.classical_selected")
                .with_description("Classical relay selections observed per session.")
                .with_unit("relays")
                .init();
            let brownouts_total = meter
                .u64_counter("sorafs.fetch.brownouts_total")
                .with_description("Anonymity policy brownout events grouped by stage/reason.")
                .init();
            let transport_events_total = meter
                .u64_counter("sorafs.fetch.transport_events_total")
                .with_description("Transport events emitted by the orchestrator grouped by protocol/event/reason.")
                .init();

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
            self.pq_ratio.observe(ratio.clamp(0.0, 1.0), &attrs);
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
                .observe(ratio.clamp(0.0, 1.0), &attrs);
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
            self.pq_deficit_ratio.observe(ratio.clamp(0.0, 1.0), &attrs);
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
            self.classical_ratio.observe(ratio.clamp(0.0, 1.0), &attrs);
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
            self.classical_selected.observe(selected as f64, &attrs);
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
                .init();
            let poseidon_pipeline_resolutions_total = meter
                .u64_counter("fastpq.poseidon_pipeline_resolutions_total")
                .with_description(
                    "FASTPQ Poseidon pipeline resolutions (labels: requested, resolved, path, device_class, chip_family, gpu_kind).",
                )
                .init();
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
                .init();
            let latency_minutes = meter
                .f64_histogram("sorafs.repair.latency_minutes")
                .with_description("SoraFS repair lifecycle latency in minutes.")
                .with_unit("min")
                .init();
            let backlog_oldest_age_seconds = meter
                .f64_histogram("sorafs.repair.backlog_oldest_age_seconds")
                .with_description("Age in seconds of the oldest queued SoraFS repair task.")
                .with_unit("s")
                .init();
            let queue_depth = meter
                .f64_histogram("sorafs.repair.queue_depth")
                .with_description("SoraFS repair queue depth per provider.")
                .with_unit("tasks")
                .init();
            let lease_expired_total = meter
                .u64_counter("sorafs.repair.lease_expired_total")
                .with_description("SoraFS repair lease expirations grouped by outcome.")
                .init();
            let slash_proposals_total = meter
                .u64_counter("sorafs.repair.slash_proposals_total")
                .with_description("SoraFS repair slash proposals grouped by outcome.")
                .init();
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
                .init();
            let evictions_total = meter
                .u64_counter("sorafs.gc.evictions_total")
                .with_description("SoraFS GC evictions grouped by reason.")
                .init();
            let bytes_freed_total = meter
                .u64_counter("sorafs.gc.bytes_freed_total")
                .with_description("SoraFS GC freed bytes grouped by reason.")
                .init();
            let blocked_total = meter
                .u64_counter("sorafs.gc.blocked_total")
                .with_description("SoraFS GC evictions blocked grouped by reason.")
                .init();
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
                .init();
            let divergence_total = meter
                .u64_counter("sorafs.reconciliation.divergence_total")
                .with_description("SoraFS reconciliation divergence count per run.")
                .init();
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
    requests_total: Counter<u64>,
    #[cfg(feature = "otel-exporter")]
    ttfb_ms: OtelHistogram<f64>,
    #[cfg(feature = "otel-exporter")]
    proof_events_total: Counter<u64>,
    #[cfg(feature = "otel-exporter")]
    proof_latency_ms: OtelHistogram<f64>,
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
                .init();

            let requests_total = meter
                .u64_counter("sorafs.gateway.requests_total")
                .with_description(
                    "Total SoraFS gateway requests grouped by endpoint/result/reason.",
                )
                .init();

            let ttfb_ms = meter
                .f64_histogram("sorafs.gateway.ttfb_ms")
                .with_description("Gateway time-to-first-byte histogram (milliseconds).")
                .with_unit("ms")
                .init();

            let proof_events_total = meter
                .u64_counter("sorafs.gateway.proof_events_total")
                .with_description("Proof stream outcomes grouped by kind/result/reason.")
                .init();

            let proof_latency_ms = meter
                .f64_histogram("sorafs.gateway.proof_latency_ms")
                .with_description("Proof stream latency (milliseconds).")
                .with_unit("ms")
                .init();

            Self {
                active_requests,
                requests_total,
                ttfb_ms,
                proof_events_total,
                proof_latency_ms,
            }
        }
        #[cfg(not(feature = "otel-exporter"))]
        {
            Self {}
        }
    }

    /// Track the start of a gateway request for active request accounting.
    #[allow(clippy::too_many_arguments)] // arguments map directly to metric dimensions
    pub fn request_started_detailed(
        &self,
        endpoint: &str,
        method: &str,
        variant: Option<&str>,
        chunker: Option<&str>,
        profile: Option<&str>,
    ) {
        #[cfg(feature = "otel-exporter")]
        {
            let attrs = Self::base_attrs(
                endpoint,
                Some(method),
                variant,
                chunker,
                profile,
                None,
                None,
            );
            self.active_requests.add(1, &attrs);
        }
        #[cfg(not(feature = "otel-exporter"))]
        {
            let _ = (endpoint, method, variant, chunker, profile);
        }
    }

    /// Track the completion of a gateway request.
    #[allow(clippy::too_many_arguments)]
    pub fn request_completed_detailed(
        &self,
        endpoint: &str,
        method: &str,
        variant: Option<&str>,
        chunker: Option<&str>,
        profile: Option<&str>,
        provider_id: Option<&str>,
        tier: Option<&str>,
        outcome: &str,
        status: u16,
        reason: Option<&str>,
    ) {
        #[cfg(feature = "otel-exporter")]
        {
            let active_attrs = Self::base_attrs(
                endpoint,
                Some(method),
                variant,
                chunker,
                profile,
                None,
                None,
            );
            self.active_requests.add(-1, &active_attrs);

            let mut attrs = Self::base_attrs(
                endpoint,
                Some(method),
                variant,
                chunker,
                profile,
                provider_id,
                tier,
            );
            attrs.push(KeyValue::new("result", outcome.to_string()));
            if let Some(reason) = reason.filter(|value| !value.is_empty()) {
                attrs.push(KeyValue::new("reason", reason.to_string()));
            }
            attrs.push(KeyValue::new("status", status.to_string()));
            self.requests_total.add(1, &attrs);
        }
        #[cfg(not(feature = "otel-exporter"))]
        {
            let _ = (
                endpoint,
                method,
                variant,
                chunker,
                profile,
                provider_id,
                tier,
                outcome,
                status,
                reason,
            );
        }
    }

    /// Record a gateway latency observation with detailed labels.
    #[allow(clippy::too_many_arguments)]
    pub fn record_ttfb_detailed(
        &self,
        endpoint: &str,
        method: &str,
        variant: Option<&str>,
        chunker: Option<&str>,
        profile: Option<&str>,
        provider_id: Option<&str>,
        tier: Option<&str>,
        outcome: &str,
        status: u16,
        reason: Option<&str>,
        latency_ms: f64,
    ) {
        #[cfg(feature = "otel-exporter")]
        {
            let mut attrs = Self::base_attrs(
                endpoint,
                Some(method),
                variant,
                chunker,
                profile,
                provider_id,
                tier,
            );
            attrs.push(KeyValue::new("result", outcome.to_string()));
            attrs.push(KeyValue::new("status", status.to_string()));
            if let Some(reason) = reason.filter(|value| !value.is_empty()) {
                attrs.push(KeyValue::new("reason", reason.to_string()));
            }
            self.ttfb_ms.record(latency_ms, &attrs);
        }
        #[cfg(not(feature = "otel-exporter"))]
        {
            let _ = (
                endpoint,
                method,
                variant,
                chunker,
                profile,
                provider_id,
                tier,
                outcome,
                status,
                reason,
                latency_ms,
            );
        }
    }

    /// Record a proof verification outcome using the gateway proof metrics.
    pub fn record_proof_verification(
        &self,
        profile_version: &str,
        outcome: &str,
        reason: Option<&str>,
        latency_ms: f64,
    ) {
        #[cfg(feature = "otel-exporter")]
        {
            let mut attrs = Vec::with_capacity(4);
            attrs.push(KeyValue::new("kind", "verification".to_string()));
            attrs.push(KeyValue::new("result", outcome.to_string()));
            attrs.push(KeyValue::new(
                "profile_version",
                profile_version.to_string(),
            ));
            if let Some(reason) = reason.filter(|value| !value.is_empty()) {
                attrs.push(KeyValue::new("reason", reason.to_string()));
            }
            self.proof_events_total.add(1, &attrs);
            self.proof_latency_ms.record(latency_ms, &attrs);
        }
        #[cfg(not(feature = "otel-exporter"))]
        {
            let _ = (profile_version, outcome, reason, latency_ms);
        }
    }

    /// Record a gateway request outcome.
    #[allow(clippy::needless_pass_by_value, clippy::too_many_arguments)]
    pub fn record_request(
        &self,
        endpoint: &str,
        result: &str,
        reason: Option<&str>,
        chunker: Option<&str>,
        profile: Option<&str>,
        provider_id: Option<&str>,
        tier: Option<&str>,
        status: Option<u16>,
    ) {
        #[cfg(feature = "otel-exporter")]
        {
            let mut attrs =
                Self::base_attrs(endpoint, None, None, chunker, profile, provider_id, tier);
            attrs.push(KeyValue::new("result", result.to_string()));
            if let Some(reason) = reason.filter(|value| !value.is_empty()) {
                attrs.push(KeyValue::new("reason", reason.to_string()));
            }
            if let Some(status) = status {
                attrs.push(KeyValue::new("status", status.to_string()));
            }
            self.requests_total.add(1, &attrs);
        }
        #[cfg(not(feature = "otel-exporter"))]
        {
            let _ = (
                endpoint,
                result,
                reason,
                chunker,
                profile,
                provider_id,
                tier,
                status,
            );
        }
    }

    /// Observe a gateway TTFB measurement.
    pub fn observe_ttfb(
        &self,
        endpoint: &str,
        chunker: Option<&str>,
        profile: Option<&str>,
        provider_id: Option<&str>,
        tier: Option<&str>,
        latency_ms: f64,
    ) {
        #[cfg(feature = "otel-exporter")]
        {
            let attrs = Self::base_attrs(endpoint, None, None, chunker, profile, provider_id, tier);
            self.ttfb_ms.record(latency_ms, &attrs);
        }
        #[cfg(not(feature = "otel-exporter"))]
        {
            let _ = (endpoint, chunker, profile, provider_id, tier, latency_ms);
        }
    }

    /// Record a proof stream outcome.
    pub fn record_proof_event(
        &self,
        kind: &str,
        result: &str,
        reason: Option<&str>,
        provider_id: Option<&str>,
        tier: Option<&str>,
        latency_ms: Option<f64>,
    ) {
        #[cfg(feature = "otel-exporter")]
        {
            let mut attrs = Vec::with_capacity(6);
            attrs.push(KeyValue::new("kind", kind.to_string()));
            attrs.push(KeyValue::new("result", result.to_string()));
            if let Some(reason) = reason.filter(|value| !value.is_empty()) {
                attrs.push(KeyValue::new("reason", reason.to_string()));
            }
            if let Some(provider) = provider_id.filter(|value| !value.is_empty()) {
                attrs.push(KeyValue::new("provider_id", provider.to_string()));
            }
            if let Some(tier) = tier.filter(|value| !value.is_empty()) {
                attrs.push(KeyValue::new("tier", tier.to_string()));
            }
            self.proof_events_total.add(1, &attrs);
            if let Some(latency_ms) = latency_ms {
                self.proof_latency_ms.record(latency_ms, &attrs);
            }
        }
        #[cfg(not(feature = "otel-exporter"))]
        {
            let _ = (kind, result, reason, provider_id, tier, latency_ms);
        }
    }

    #[cfg(feature = "otel-exporter")]
    fn base_attrs(
        endpoint: &str,
        method: Option<&str>,
        variant: Option<&str>,
        chunker: Option<&str>,
        profile: Option<&str>,
        provider_id: Option<&str>,
        tier: Option<&str>,
    ) -> Vec<KeyValue> {
        let mut attrs = Vec::with_capacity(7);
        attrs.push(KeyValue::new("endpoint", endpoint.to_string()));
        if let Some(method) = method.filter(|value| !value.is_empty()) {
            attrs.push(KeyValue::new("method", method.to_string()));
        }
        if let Some(variant) = variant.filter(|value| !value.is_empty()) {
            attrs.push(KeyValue::new("variant", variant.to_string()));
        }
        if let Some(chunker) = chunker.filter(|value| !value.is_empty()) {
            attrs.push(KeyValue::new("chunker", chunker.to_string()));
        }
        if let Some(profile) = profile.filter(|value| !value.is_empty()) {
            attrs.push(KeyValue::new("profile", profile.to_string()));
        }
        if let Some(provider) = provider_id.filter(|value| !value.is_empty()) {
            attrs.push(KeyValue::new("provider_id", provider.to_string()));
        }
        if let Some(tier) = tier.filter(|value| !value.is_empty()) {
            attrs.push(KeyValue::new("tier", tier.to_string()));
        }
        attrs
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
    Copy,
    PartialEq,
    Eq,
    Default,
    IntoSchema,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
pub struct MicropaymentCreditSnapshot {
    /// Deterministic charge accumulated during the sample (nano XOR).
    pub deterministic_charge: u128,
    /// Credit produced by micropayment winnings (nano XOR).
    pub credit_generated: u128,
    /// Credit immediately applied against the deterministic charge (nano XOR).
    pub credit_applied: u128,
    /// Credit carried forward for future windows (nano XOR).
    pub credit_carry: u128,
    /// Outstanding balance after applying credit (nano XOR).
    pub outstanding: u128,
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

            let build_counter = |name: &str, description: &str| {
                meter.u64_counter(name).with_description(description).init()
            };
            let build_histogram = |name: &str, description: &str, unit: &str| {
                meter
                    .f64_histogram(name)
                    .with_description(description)
                    .with_unit(unit)
                    .init()
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
        expected_charge_nano: u128,
        client_debit_nano: u128,
        bond_slash_nano: u128,
        outstanding_nano: u128,
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
                .record(expected_charge_nano as f64, &provider_attrs);
            self.deal_client_debit_nano
                .record(client_debit_nano as f64, &provider_attrs);
            self.deal_outstanding_nano
                .record(outstanding_nano as f64, &provider_attrs);
            if bond_slash_nano > 0 {
                let increment = u128::min(bond_slash_nano, u64::MAX as u128) as u64;
                self.deal_bond_slash_nano.add(increment, &provider_attrs);
            }
        }
        #[cfg(not(feature = "otel-exporter"))]
        {
            let _ = (
                provider_id,
                status,
                expected_charge_nano,
                client_debit_nano,
                bond_slash_nano,
                outstanding_nano,
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
                .record(*deterministic_charge as f64, &attrs);
            self.micropayment_credit_generated_nano
                .record(*credit_generated as f64, &attrs);
            self.micropayment_credit_applied_nano
                .record(*credit_applied as f64, &attrs);
            self.micropayment_credit_carry_nano
                .record(*credit_carry as f64, &attrs);
            self.micropayment_outstanding_nano
                .record(*outstanding as f64, &attrs);
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
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
        let payload = (
            self.deterministic_charge,
            self.credit_generated,
            self.credit_applied,
            self.credit_carry,
            self.outstanding,
        );
        norito::core::NoritoSerialize::serialize(&payload, writer)
    }
}

impl<'a> norito::core::NoritoDeserialize<'a> for MicropaymentCreditSnapshot {
    fn deserialize(archived: &'a norito::core::Archived<Self>) -> Self {
        let (deterministic_charge, credit_generated, credit_applied, credit_carry, outstanding): (
            u128,
            u128,
            u128,
            u128,
            u128,
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
        ) = <(u128, u128, u128, u128, u128)>::decode_from_slice(bytes)?;
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
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
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
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
        let payload = (self.provider_id_hex.clone(), self.credits, self.tickets);
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
    use norito::{NoritoDeserialize, from_bytes, to_bytes};

    use super::*;

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

    fn sample_lane_teu_status() -> NexusLaneTeuStatus {
        NexusLaneTeuStatus {
            lane_id: 0,
            capacity: 20,
            committed: 10,
            buckets: NexusLaneTeuBuckets::default(),
            deferrals: NexusLaneTeuDeferrals::default(),
            must_serve_truncations: 0,
            trigger_level: 0,
            starvation_bound_slots: 4,
            block_height: 1,
            finality_lag_slots: 0,
            settlement_backlog_xor_micro: 0,
            tx_vertices: 1,
            tx_edges: 0,
            overlay_count: 0,
            overlay_instr_total: 0,
            overlay_bytes_total: 0,
            rbc_chunks: 0,
            rbc_bytes_total: 0,
            peak_layer_width: 0,
            layer_count: 0,
            avg_layer_width: 0,
            median_layer_width: 0,
            scheduler_utilization_pct: 0,
            layer_width_buckets: SchedulerLayerWidthBuckets::default(),
            detached_prepared: 0,
            detached_merged: 0,
            detached_fallback: 0,
            quarantine_executed: 0,
            manifest_required: false,
            manifest_ready: false,
            alias: String::new(),
            dataspace_id: 0,
            dataspace_alias: None,
            visibility: None,
            storage_profile: String::new(),
            lane_type: None,
            governance: None,
            settlement: None,
            scheduler_teu_capacity_override: None,
            scheduler_starvation_bound_override: None,
            manifest_path: None,
            manifest_validators: Vec::new(),
            manifest_quorum: None,
            manifest_protected_namespaces: Vec::new(),
            manifest_runtime_upgrade: None,
        }
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
    fn status_strip_nexus_clears_lane_fields() {
        let mut status = Status {
            teu_lane_commit: vec![sample_lane_teu_status()],
            teu_dataspace_backlog: vec![sample_dataspace_teu_status()],
            da_receipt_cursors: vec![DaReceiptCursorStatus {
                lane_id: 0,
                epoch: 1,
                highest_sequence: 2,
            }],
            sumeragi: Some(SumeragiConsensusStatus {
                lane_governance_sealed_total: 1,
                lane_governance_sealed_aliases: vec!["lane-x".into()],
                ..SumeragiConsensusStatus::default()
            }),
            ..Status::default()
        };

        status.strip_nexus();

        assert!(status.teu_lane_commit.is_empty());
        assert!(status.teu_dataspace_backlog.is_empty());
        assert!(status.da_receipt_cursors.is_empty());
        let consensus = status.sumeragi.expect("consensus present");
        assert_eq!(consensus.lane_governance_sealed_total, 0);
        assert!(consensus.lane_governance_sealed_aliases.is_empty());
    }

    #[cfg(not(feature = "otel-exporter"))]
    #[test]
    fn sorafs_node_otel_new_and_record_sample_do_not_panic_without_exporter() {
        let otel = SorafsNodeOtel::new();
        otel.record_micropayment_sample(
            "provider",
            MicropaymentCreditSnapshot {
                deterministic_charge: 10,
                credit_generated: 5,
                credit_applied: 4,
                credit_carry: 1,
                outstanding: 2,
            },
            MicropaymentTicketCounters {
                processed: 3,
                won: 1,
                duplicate: 0,
            },
        );
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
    fn records_offline_transfer_rejections() {
        let metrics = Metrics::default();
        metrics.record_offline_transfer_rejection(
            OfflineTransferRejectionPlatform::Apple,
            OfflineTransferRejectionReason::PlatformAttestationInvalid,
        );
        let value = metrics
            .offline_transfer_rejections_total
            .with_label_values(&[
                OfflineTransferRejectionPlatform::Apple.as_label(),
                OfflineTransferRejectionReason::PlatformAttestationInvalid.as_label(),
            ])
            .get();
        assert_eq!(value, 1);
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
    fn metrics_export_strips_lane_labels_when_nexus_disabled() {
        let metrics = Metrics::default();
        metrics.set_lane_block_height("lane-0", "global", 7);
        metrics.txs.with_label_values(&["committed"]).inc();

        let enabled = metrics
            .try_to_string_with_nexus_gate(true)
            .expect("metrics text");
        assert!(
            enabled.contains("nexus_lane_block_height"),
            "lane metrics should be present when Nexus is enabled"
        );

        let filtered = metrics
            .try_to_string_with_nexus_gate(false)
            .expect("filtered metrics");
        assert!(
            !filtered.contains("nexus_lane_block_height"),
            "lane metrics should be stripped when Nexus is disabled: {filtered}"
        );
        assert!(
            filtered.contains("txs{type=\"committed\"}"),
            "non-lane metrics must remain after filtering: {filtered}"
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
    use opentelemetry_otlp::WithExportConfig;
    use opentelemetry_sdk::{
        Resource,
        metrics::{PeriodicReader, SdkMeterProvider},
        runtime::Tokio,
    };

    let exporter = opentelemetry_otlp::new_exporter()
        .tonic()
        .with_endpoint(endpoint.to_owned());

    let reader = PeriodicReader::builder(exporter, Tokio)
        .with_interval(interval)
        .build();

    let mut attributes = Vec::with_capacity(resource.len() + 1);
    attributes.push(KeyValue::new("service.name", service_name.to_string()));
    for (key, value) in resource {
        attributes.push(KeyValue::new((*key).to_string(), (*value).to_string()));
    }

    let provider = SdkMeterProvider::builder()
        .with_resource(Resource::new(attributes))
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

#[cfg(test)]
mod otel_tests {
    use std::sync::Arc;

    use super::*;

    #[test]
    fn global_fetch_otel_is_singleton() {
        let first = global_sorafs_fetch_otel();
        let second = global_sorafs_fetch_otel();
        assert!(
            Arc::ptr_eq(&first, &second),
            "expected OTEL handle to be singleton"
        );
    }

    #[test]
    fn global_gateway_otel_is_singleton() {
        let first = global_sorafs_gateway_otel();
        let second = global_sorafs_gateway_otel();
        assert!(
            Arc::ptr_eq(&first, &second),
            "expected gateway OTEL handle to be singleton"
        );
    }

    #[cfg(not(feature = "otel-exporter"))]
    #[test]
    fn installing_exporter_without_feature_fails() {
        let result = install_sorafs_fetch_otlp_exporter(
            "http://127.0.0.1:4317",
            "sorafs-orchestrator",
            &[],
            Duration::from_secs(5),
        );
        assert!(
            result.is_err(),
            "expected exporter installation to fail without otel-exporter feature"
        );
    }
}

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
            peers: 3,
            blocks: 42,
            blocks_non_empty: 39,
            commit_time_ms: 12,
            txs_approved: 7,
            txs_rejected: 2,
            uptime: Uptime(Duration::new(9, 0)),
            view_changes: 4,
            queue_size: 5,
            crypto: CryptoStatus {
                sm_helpers_available: true,
                sm_openssl_preview_enabled: false,
                halo2: Halo2Status::default(),
            },
            stack: StackStatus::default(),
            sumeragi: Some(SumeragiConsensusStatus::default()),
            governance: GovernanceStatus::default(),
            teu_lane_commit: Vec::new(),
            teu_dataspace_backlog: Vec::new(),
            tx_gossip: TxGossipSnapshot::default(),
            da_reschedule_total: 0,
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
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
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
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
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
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
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
    /// Validator quorum required by the lane manifest.
    pub manifest_quorum: Option<u32>,
    /// Protected namespaces enforced by the lane manifest.
    pub manifest_protected_namespaces: Vec<String>,
    /// Runtime-upgrade governance hook snapshot when configured.
    pub manifest_runtime_upgrade: Option<NexusLaneRuntimeUpgradeHookStatus>,
}

impl norito::core::NoritoSerialize for NexusLaneTeuStatus {
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
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
        }
    }
}

/// Snapshot of the runtime-upgrade governance hook declared in a lane manifest.
#[derive(
    Clone,
    Debug,
    Default,
    IntoSchema,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
pub struct NexusLaneRuntimeUpgradeHookStatus {
    /// Whether runtime-upgrade instructions are permitted.
    pub allow: bool,
    /// Whether runtime-upgrade instructions must include manifest metadata.
    pub require_metadata: bool,
    /// Metadata key enforced by the manifest.
    #[norito(default)]
    pub metadata_key: Option<String>,
    /// Allowed metadata identifiers declared by the manifest.
    pub allowed_ids: Vec<String>,
}

impl norito::core::NoritoSerialize for NexusLaneRuntimeUpgradeHookStatus {
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
        let payload = (
            self.allow,
            self.require_metadata,
            self.metadata_key.clone(),
            self.allowed_ids.clone(),
        );
        norito::core::NoritoSerialize::serialize(&payload, writer)
    }
}

impl<'a> norito::core::NoritoDeserialize<'a> for NexusLaneRuntimeUpgradeHookStatus {
    fn deserialize(archived: &'a norito::core::Archived<Self>) -> Self {
        let (allow, require_metadata, metadata_key, allowed_ids) =
            norito::core::NoritoDeserialize::deserialize(archived.cast());
        Self {
            allow,
            require_metadata,
            metadata_key,
            allowed_ids,
        }
    }
}

impl<'a> DecodeFromSlice<'a> for NexusLaneRuntimeUpgradeHookStatus {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let ((allow, require_metadata, metadata_key, allowed_ids), used) =
            <(bool, bool, Option<String>, Vec<String>)>::decode_from_slice(bytes)?;
        Ok((
            Self {
                allow,
                require_metadata,
                metadata_key,
                allowed_ids,
            },
            used,
        ))
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
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
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
pub struct SumeragiConsensusStatus {
    /// Current runtime consensus mode tag.
    pub mode_tag: String,
    /// Staged consensus mode awaiting activation, if any.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub staged_mode_tag: Option<String>,
    /// Activation height for the staged consensus mode (if any).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub staged_mode_activation_height: Option<u64>,
    /// Blocks elapsed since activation height passed without applying the staged mode (if any).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub mode_activation_lag_blocks: Option<u64>,
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
    /// Whether the local transaction queue is saturated.
    pub tx_queue_saturated: bool,
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
    /// Total DA deadline reschedules pushing blocks into future slots.
    pub da_reschedule_total: u64,
    /// Total RBC DELIVER deferrals due to missing READY quorum.
    pub rbc_deliver_defer_ready_total: u64,
    /// Total RBC DELIVER deferrals due to missing chunks.
    pub rbc_deliver_defer_chunks_total: u64,
    /// Current number of persisted RBC sessions on disk.
    pub rbc_store_sessions: u64,
    /// Current persisted RBC payload bytes on disk.
    pub rbc_store_bytes: u64,
    /// Current RBC store pressure level (0 = normal, 1 = soft limit, 2 = hard limit).
    pub rbc_store_pressure_level: u8,
    /// Total number of times proposal assembly was deferred due to RBC store pressure.
    pub rbc_store_backpressure_deferrals_total: u64,
    /// Total number of RBC persist requests dropped due to full async queues.
    #[norito(default)]
    pub rbc_store_persist_drops_total: u64,
    /// Total number of RBC sessions evicted due to TTL or capacity enforcement.
    pub rbc_store_evictions_total: u64,
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

impl SumeragiConsensusStatus {
    /// Drop lane-specific fields when Nexus lanes are disabled.
    pub fn clear_nexus_fields(&mut self) {
        self.lane_governance_sealed_total = 0;
        self.lane_governance_sealed_aliases.clear();
    }
}

impl norito::core::NoritoSerialize for SumeragiConsensusStatus {
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
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
struct SumeragiConsensusStatusPayload {
    mode_tag: String,
    staged_mode_tag: Option<String>,
    staged_mode_activation_height: Option<u64>,
    mode_activation_lag_blocks: Option<u64>,
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
    da_reschedule_total: u64,
    rbc_deliver_defer_ready_total: u64,
    rbc_deliver_defer_chunks_total: u64,
    rbc_store_sessions: u64,
    rbc_store_bytes: u64,
    rbc_store_pressure_level: u8,
    rbc_store_backpressure_deferrals_total: u64,
    rbc_store_persist_drops_total: u64,
    rbc_store_evictions_total: u64,
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

#[allow(clippy::type_complexity)]
fn decode_rbc_fields(
    bytes: &[u8],
    used: &mut usize,
) -> Result<(u64, u64, u64, u64, u64, u8, u64, u64, u64), norito::core::Error> {
    if *used >= bytes.len() {
        return Ok((0, 0, 0, 0, 0, 0, 0, 0, 0));
    }

    let reschedules = decode_field::<u64>(bytes, used)?;
    if *used >= bytes.len() {
        return Ok((reschedules, 0, 0, 0, 0, 0, 0, 0, 0));
    }

    let defer_ready = decode_field::<u64>(bytes, used)?;
    if *used >= bytes.len() {
        return Ok((reschedules, defer_ready, 0, 0, 0, 0, 0, 0, 0));
    }

    let defer_chunks = decode_field::<u64>(bytes, used)?;
    if *used >= bytes.len() {
        return Ok((reschedules, defer_ready, defer_chunks, 0, 0, 0, 0, 0, 0));
    }

    let sessions = decode_field::<u64>(bytes, used)?;
    if *used >= bytes.len() {
        return Ok((
            reschedules,
            defer_ready,
            defer_chunks,
            sessions,
            0,
            0,
            0,
            0,
            0,
        ));
    }

    let bytes_total = decode_field::<u64>(bytes, used)?;
    if *used >= bytes.len() {
        return Ok((
            reschedules,
            defer_ready,
            defer_chunks,
            sessions,
            bytes_total,
            0,
            0,
            0,
            0,
        ));
    }

    let level = decode_field::<u8>(bytes, used)?;
    if *used >= bytes.len() {
        return Ok((
            reschedules,
            defer_ready,
            defer_chunks,
            sessions,
            bytes_total,
            level,
            0,
            0,
            0,
        ));
    }

    let deferrals = decode_field::<u64>(bytes, used)?;
    if *used >= bytes.len() {
        return Ok((
            reschedules,
            defer_ready,
            defer_chunks,
            sessions,
            bytes_total,
            level,
            deferrals,
            0,
            0,
        ));
    }

    let persist_drops = decode_field::<u64>(bytes, used)?;
    if *used >= bytes.len() {
        return Ok((
            reschedules,
            defer_ready,
            defer_chunks,
            sessions,
            bytes_total,
            level,
            deferrals,
            persist_drops,
            0,
        ));
    }

    let evictions = decode_field::<u64>(bytes, used)?;
    Ok((
        reschedules,
        defer_ready,
        defer_chunks,
        sessions,
        bytes_total,
        level,
        deferrals,
        persist_drops,
        evictions,
    ))
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
        let (mode_tag, staged_mode_tag, staged_mode_activation_height, mode_activation_lag_blocks) =
            if bytes.is_empty() {
                (PERMISSIONED_TAG.to_string(), None, None, None)
            } else if let Ok(tag) = decode_field::<String>(bytes, &mut used) {
                let staged_tag = if used < bytes.len() {
                    decode_field::<Option<String>>(bytes, &mut used)?
                } else {
                    None
                };
                let staged_activation = if used < bytes.len() {
                    decode_field::<Option<u64>>(bytes, &mut used)?
                } else {
                    None
                };
                let lag_blocks = if used < bytes.len() {
                    decode_field::<Option<u64>>(bytes, &mut used)?
                } else {
                    None
                };
                (tag, staged_tag, staged_activation, lag_blocks)
            } else {
                used = 0;
                (PERMISSIONED_TAG.to_string(), None, None, None)
            };
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
        let (
            da_reschedule_total,
            rbc_deliver_defer_ready_total,
            rbc_deliver_defer_chunks_total,
            rbc_store_sessions,
            rbc_store_bytes,
            rbc_store_pressure_level,
            rbc_store_backpressure_deferrals_total,
            rbc_store_persist_drops_total,
            rbc_store_evictions_total,
        ) = decode_rbc_fields(bytes, &mut used)?;
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

        Ok((
            Self {
                mode_tag,
                staged_mode_tag,
                staged_mode_activation_height,
                mode_activation_lag_blocks,
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
                da_reschedule_total,
                rbc_deliver_defer_ready_total,
                rbc_deliver_defer_chunks_total,
                rbc_store_sessions,
                rbc_store_bytes,
                rbc_store_pressure_level,
                rbc_store_backpressure_deferrals_total,
                rbc_store_persist_drops_total,
                rbc_store_evictions_total,
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
            },
            used,
        ))
    }
}
impl From<&SumeragiConsensusStatus> for SumeragiConsensusStatusPayload {
    fn from(status: &SumeragiConsensusStatus) -> Self {
        Self {
            mode_tag: status.mode_tag.clone(),
            staged_mode_tag: status.staged_mode_tag.clone(),
            staged_mode_activation_height: status.staged_mode_activation_height,
            mode_activation_lag_blocks: status.mode_activation_lag_blocks,
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
            tx_queue_saturated: status.tx_queue_saturated,
            epoch_length_blocks: status.epoch_length_blocks,
            epoch_commit_deadline_offset: status.epoch_commit_deadline_offset,
            epoch_reveal_deadline_offset: status.epoch_reveal_deadline_offset,
            da_reschedule_total: status.da_reschedule_total,
            rbc_deliver_defer_ready_total: status.rbc_deliver_defer_ready_total,
            rbc_deliver_defer_chunks_total: status.rbc_deliver_defer_chunks_total,
            rbc_store_sessions: status.rbc_store_sessions,
            rbc_store_bytes: status.rbc_store_bytes,
            rbc_store_pressure_level: status.rbc_store_pressure_level,
            rbc_store_backpressure_deferrals_total: status.rbc_store_backpressure_deferrals_total,
            rbc_store_persist_drops_total: status.rbc_store_persist_drops_total,
            rbc_store_evictions_total: status.rbc_store_evictions_total,
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
            staged_mode_tag: payload.staged_mode_tag,
            staged_mode_activation_height: payload.staged_mode_activation_height,
            mode_activation_lag_blocks: payload.mode_activation_lag_blocks,
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
            tx_queue_saturated: payload.tx_queue_saturated,
            epoch_length_blocks: payload.epoch_length_blocks,
            epoch_commit_deadline_offset: payload.epoch_commit_deadline_offset,
            epoch_reveal_deadline_offset: payload.epoch_reveal_deadline_offset,
            da_reschedule_total: payload.da_reschedule_total,
            rbc_deliver_defer_ready_total: payload.rbc_deliver_defer_ready_total,
            rbc_deliver_defer_chunks_total: payload.rbc_deliver_defer_chunks_total,
            rbc_store_sessions: payload.rbc_store_sessions,
            rbc_store_bytes: payload.rbc_store_bytes,
            rbc_store_pressure_level: payload.rbc_store_pressure_level,
            rbc_store_backpressure_deferrals_total: payload.rbc_store_backpressure_deferrals_total,
            rbc_store_persist_drops_total: payload.rbc_store_persist_drops_total,
            rbc_store_evictions_total: payload.rbc_store_evictions_total,
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

/// Internal key for `(lane, epoch)` receipt cursors.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct DaReceiptCursorKey {
    lane_id: u32,
    epoch: u64,
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
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
pub struct Status {
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
    /// Uptime since genesis block creation
    pub uptime: Uptime,
    /// Number of view changes in the current round
    pub view_changes: u32,
    /// Number of transactions tracked by the queue (queued + in-flight)
    pub queue_size: u64,
    /// Total number of DA deadline reschedules observed by this peer.
    #[norito(default)]
    pub da_reschedule_total: u64,
    /// Cryptography feature snapshot (SM enablement flags).
    #[norito(default)]
    pub crypto: CryptoStatus,
    /// Stack sizing/configuration snapshot.
    #[norito(default)]
    pub stack: StackStatus,
    /// Summary of the consensus snapshot (leader, QCs, queue state).
    #[norito(skip_serializing_if = "Option::is_none")]
    pub sumeragi: Option<SumeragiConsensusStatus>,
    /// Governance telemetry snapshot (proposal counts, protections, activations)
    pub governance: GovernanceStatus,
    /// Nexus lane-level TEU scheduling snapshot
    pub teu_lane_commit: Vec<NexusLaneTeuStatus>,
    /// Nexus dataspace-level backlog snapshot
    pub teu_dataspace_backlog: Vec<NexusDataspaceTeuStatus>,
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
    /// DA receipt cursors grouped by (lane, epoch).
    #[norito(default)]
    #[norito(skip_serializing_if = "Vec::is_empty")]
    pub da_receipt_cursors: Vec<DaReceiptCursorStatus>,
}

impl Status {
    /// Remove Nexus lane/dataspace telemetry when Nexus mode is disabled.
    pub fn strip_nexus(&mut self) {
        self.teu_lane_commit.clear();
        self.teu_dataspace_backlog.clear();
        self.da_receipt_cursors.clear();
        if let Some(consensus) = self.sumeragi.as_mut() {
            consensus.clear_nexus_fields();
        }
    }
}

#[derive(Clone, Debug, norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
struct StatusPayload {
    peers: u64,
    blocks: u64,
    blocks_non_empty: u64,
    commit_time_ms: u64,
    txs_approved: u64,
    txs_rejected: u64,
    uptime: Uptime,
    view_changes: u32,
    queue_size: u64,
    #[norito(default)]
    da_reschedule_total: u64,
    #[norito(default)]
    crypto: CryptoStatus,
    #[norito(default)]
    stack: StackStatus,
    #[norito(skip_serializing_if = "Option::is_none")]
    sumeragi: Option<SumeragiConsensusStatus>,
    governance: GovernanceStatus,
    teu_lane_commit: Vec<NexusLaneTeuStatus>,
    teu_dataspace_backlog: Vec<NexusDataspaceTeuStatus>,
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
            peers: status.peers,
            blocks: status.blocks,
            blocks_non_empty: status.blocks_non_empty,
            commit_time_ms: status.commit_time_ms,
            txs_approved: status.txs_approved,
            txs_rejected: status.txs_rejected,
            uptime: status.uptime,
            view_changes: status.view_changes,
            queue_size: status.queue_size,
            da_reschedule_total: status.da_reschedule_total,
            crypto: status.crypto.clone(),
            stack: status.stack,
            sumeragi: status.sumeragi.clone(),
            governance: status.governance.clone(),
            teu_lane_commit: status.teu_lane_commit.clone(),
            teu_dataspace_backlog: status.teu_dataspace_backlog.clone(),
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
            peers: payload.peers,
            blocks: payload.blocks,
            blocks_non_empty: payload.blocks_non_empty,
            commit_time_ms: payload.commit_time_ms,
            txs_approved: payload.txs_approved,
            txs_rejected: payload.txs_rejected,
            uptime: payload.uptime,
            view_changes: payload.view_changes,
            queue_size: payload.queue_size,
            da_reschedule_total: payload.da_reschedule_total,
            crypto: payload.crypto,
            stack: payload.stack,
            sumeragi: payload.sumeragi,
            governance: payload.governance,
            teu_lane_commit: payload.teu_lane_commit,
            teu_dataspace_backlog: payload.teu_dataspace_backlog,
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
    /// Proposals currently awaiting review.
    pub proposed: u64,
    /// Proposals approved but not yet enacted.
    pub approved: u64,
    /// Proposals rejected by governance.
    pub rejected: u64,
    /// Proposals that completed enactment.
    pub enacted: u64,
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
    /// Namespace whose manifest was activated.
    pub namespace: String,
    /// Identifier of the deployed contract.
    pub contract_id: String,
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
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
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
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
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
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
        let payload = (self.proposed, self.approved, self.rejected, self.enacted);
        norito::core::NoritoSerialize::serialize(&payload, writer)
    }
}

impl<'a> norito::core::NoritoDeserialize<'a> for GovernanceProposalCounters {
    fn deserialize(archived: &'a norito::core::Archived<GovernanceProposalCounters>) -> Self {
        let (proposed, approved, rejected, enacted): (u64, u64, u64, u64) =
            norito::core::NoritoDeserialize::deserialize(archived.cast());
        Self {
            proposed,
            approved,
            rejected,
            enacted,
        }
    }
}

impl<'a> DecodeFromSlice<'a> for GovernanceProposalCounters {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let ((proposed, approved, rejected, enacted), used) =
            <(u64, u64, u64, u64)>::decode_from_slice(bytes)?;
        Ok((
            Self {
                proposed,
                approved,
                rejected,
                enacted,
            },
            used,
        ))
    }
}

impl norito::core::NoritoSerialize for GovernanceProtectedNamespaceCounters {
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
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
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
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
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
        let payload = (
            self.namespace.clone(),
            self.contract_id.clone(),
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
        let (namespace, contract_id, code_hash_hex, abi_hash_hex, height, activated_at_ms): (
            String,
            String,
            String,
            Option<String>,
            u64,
            u64,
        ) = norito::core::NoritoDeserialize::deserialize(archived.cast());
        Self {
            namespace,
            contract_id,
            code_hash_hex,
            abi_hash_hex,
            height,
            activated_at_ms,
        }
    }
}

impl<'a> DecodeFromSlice<'a> for GovernanceManifestActivation {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let ((namespace, contract_id, code_hash_hex, abi_hash_hex, height, activated_at_ms), used) =
            <(String, String, String, Option<String>, u64, u64)>::decode_from_slice(bytes)?;
        Ok((
            Self {
                namespace,
                contract_id,
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
    SumeragiConsensusStatus {
        mode_tag: metrics.sumeragi_mode_tag(),
        staged_mode_tag: metrics.sumeragi_staged_mode_tag(),
        staged_mode_activation_height: metrics.sumeragi_staged_mode_activation_height(),
        mode_activation_lag_blocks: metrics.sumeragi_mode_activation_lag_blocks(),
        leader_index: metrics.sumeragi_leader_index.get(),
        highest_qc_height: metrics.sumeragi_highest_qc_height.get(),
        locked_qc_height: metrics.sumeragi_locked_qc_height.get(),
        locked_qc_view: metrics.sumeragi_locked_qc_view.get(),
        commit_signatures_present: metrics.sumeragi_commit_signatures_present.get(),
        commit_signatures_counted: metrics.sumeragi_commit_signatures_counted.get(),
        commit_signatures_set_b: metrics.sumeragi_commit_signatures_set_b.get(),
        commit_signatures_required: metrics.sumeragi_commit_signatures_required.get(),
        commit_qc_height: metrics.sumeragi_commit_qc_height.get(),
        commit_qc_view: metrics.sumeragi_commit_qc_view.get(),
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
        tx_queue_saturated: metrics.sumeragi_tx_queue_saturated.get() != 0,
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
        da_reschedule_total: metrics.sumeragi_rbc_da_reschedule_total.get(),
        rbc_store_sessions: metrics.sumeragi_rbc_store_sessions.get(),
        rbc_store_bytes: metrics.sumeragi_rbc_store_bytes.get(),
        rbc_store_pressure_level: u8::try_from(metrics.sumeragi_rbc_store_pressure.get())
            .unwrap_or(0),
        rbc_store_backpressure_deferrals_total: metrics
            .sumeragi_rbc_backpressure_deferrals_total
            .get(),
        rbc_store_persist_drops_total: metrics.sumeragi_rbc_persist_drops_total.get(),
        rbc_deliver_defer_ready_total: metrics.sumeragi_rbc_deliver_defer_ready_total.get(),
        rbc_deliver_defer_chunks_total: metrics.sumeragi_rbc_deliver_defer_chunks_total.get(),
        rbc_store_evictions_total: metrics.sumeragi_rbc_store_evictions_total.get(),
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
        approved: fetch("approved"),
        rejected: fetch("rejected"),
        enacted: fetch("enacted"),
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
        Self {
            peers: value.connected_peers.get(),
            blocks: value.block_height.get(),
            blocks_non_empty: value.block_height_non_empty.get(),
            commit_time_ms: value.last_commit_time_ms.get(),
            txs_approved: value.txs.with_label_values(&["accepted"]).get(),
            txs_rejected: value.txs.with_label_values(&["rejected"]).get(),
            uptime: Uptime(Duration::from_millis(value.uptime_since_genesis_ms.get())),
            view_changes: value
                .view_changes
                .get()
                .try_into()
                .expect("INTERNAL BUG: Number of view changes exceeds u32::MAX"),
            queue_size: value.queue_size.get(),
            da_reschedule_total: value.sumeragi_rbc_da_reschedule_total.get(),
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
            sumeragi: Some(build_sumeragi_status(value)),
            governance: build_governance_status(value),
            teu_lane_commit: collect_teu_lane_commit(value),
            teu_dataspace_backlog: collect_teu_dataspace_backlog(value),
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

/// Prometheus metric registry plus cached status snapshots exposed by telemetry.
pub struct Metrics {
    /// Total number of transactions
    pub txs: IntCounterVec,
    /// Number of committed blocks (blockchain height)
    pub block_height: IntCounter,
    /// Number of committed non-empty blocks
    pub block_height_non_empty: IntCounter,
    /// Time (since block creation) it took for the latest block to reach _this_ peer
    pub last_commit_time_ms: GenericGauge<AtomicU64>,
    /// Block commit time trends
    pub commit_time_ms: Histogram,
    /// Slot duration histogram for NX-18 1-second finality tracking (milliseconds).
    pub slot_duration_ms: Histogram,
    /// Latest observed slot duration in milliseconds (mirrors NX-18 gauge requirement).
    pub slot_duration_ms_latest: GenericGauge<AtomicU64>,
    /// Rolling data-availability quorum ratio (0–1) derived from slot outcomes.
    pub da_quorum_ratio: Gauge,
    /// Number of currently connected peers excluding the reporting peer
    pub connected_peers: GenericGauge<AtomicU64>,
    /// Cumulative peer churn events observed by the node (`connected` / `disconnected`).
    pub p2p_peer_churn_total: IntCounterVec,
    /// Uptime of the network, starting from commit of the genesis block
    pub uptime_since_genesis_ms: GenericGauge<AtomicU64>,
    /// Number of domains.
    pub domains: GenericGauge<AtomicU64>,
    /// Total number of users per domain
    pub accounts: GenericGaugeVec<AtomicU64>,
    /// Transaction amounts.
    pub tx_amounts: Histogram,
    /// Queries handled by this peer
    pub isi: IntCounterVec,
    /// Query handle time Histogram
    pub isi_times: HistogramVec,
    /// Number of view changes in the current round
    pub view_changes: ViewChangesGauge,
    /// Number of transactions tracked by the queue (queued + in-flight)
    pub queue_size: GenericGauge<AtomicU64>,
    /// Number of transactions still queued for selection.
    pub queue_queued: GenericGauge<AtomicU64>,
    /// Number of transactions in-flight after selection.
    pub queue_inflight: GenericGauge<AtomicU64>,
    /// Kura fsync policy state (0=off, 1=on, 2=batched).
    pub kura_fsync_enabled: GenericGauge<AtomicU64>,
    /// Kura fsync failures grouped by target (data/index/hashes).
    pub kura_fsync_failures_total: IntCounterVec,
    /// Kura fsync latency histogram (milliseconds) grouped by target.
    pub kura_fsync_latency_ms: HistogramVec,
    /// AMX prepare phase latency histogram (milliseconds) labelled by lane id.
    pub amx_prepare_ms: HistogramVec,
    /// AMX commit/merge phase latency histogram (milliseconds) labelled by lane id.
    pub amx_commit_ms: HistogramVec,
    /// AMX abort counter grouped by lane id and abort stage.
    pub amx_abort_total: IntCounterVec,
    /// AXT policy validation failures grouped by lane and reason.
    pub axt_policy_reject_total: IntCounterVec,
    /// Stable version hash (truncated to u64) for the active AXT policy snapshot.
    pub axt_policy_snapshot_version: GenericGauge<AtomicU64>,
    /// Cache hydration events for AXT policy snapshots grouped by event label.
    pub axt_policy_snapshot_cache_events_total: IntCounterVec,
    /// Dataspace proof cache events grouped by event label.
    pub axt_proof_cache_events_total: IntCounterVec,
    /// Per-dataspace proof cache state (labels: dsid, status, manifest_root_hex, verified_slot; value = expiry_slot_with_skew).
    pub axt_proof_cache_state: IntGaugeVec,
    /// IVM execution latency histogram (milliseconds) labelled by lane id.
    pub ivm_exec_ms: HistogramVec,
    /// SM helper syscalls observed (grouped by kind/mode).
    pub sm_syscall_total: IntCounterVec,
    /// SM helper syscall failures (grouped by kind/mode/reason).
    pub sm_syscall_failures_total: IntCounterVec,
    /// Toggle state for the OpenSSL-backed SM preview helpers (0/1).
    pub sm_openssl_preview: GenericGauge<AtomicU64>,
    /// Toggle state for Halo2 verifier availability (0/1).
    pub zk_halo2_enabled: GenericGauge<AtomicU64>,
    /// Active Halo2 curve identifier (as a numeric label).
    pub zk_halo2_curve_id: GenericGauge<AtomicU64>,
    /// Active Halo2 backend identifier (as a numeric label).
    pub zk_halo2_backend_id: GenericGauge<AtomicU64>,
    /// Maximum supported Halo2 circuit exponent (k).
    pub zk_halo2_max_k: GenericGauge<AtomicU64>,
    /// Halo2 verifier soft budget in milliseconds.
    pub zk_halo2_verifier_budget_ms: GenericGauge<AtomicU64>,
    /// Maximum proofs allowed in a Halo2 batch verification.
    pub zk_halo2_verifier_max_batch: GenericGauge<AtomicU64>,
    /// Number of worker threads serving ZK lane verification.
    pub zk_halo2_verifier_worker_threads: GenericGauge<AtomicU64>,
    /// Effective ZK lane queue capacity.
    pub zk_halo2_verifier_queue_cap: GenericGauge<AtomicU64>,
    /// Count of ZK lane admissions that required a bounded wait.
    pub zk_lane_enqueue_wait_total: IntCounter,
    /// Count of ZK lane admissions that timed out under saturation.
    pub zk_lane_enqueue_timeout_total: IntCounter,
    /// ZK lane dropped-task counter labeled by terminal reason.
    pub zk_lane_drop_total: IntCounterVec,
    /// Count of important tasks enqueued into the ZK lane retry ring.
    pub zk_lane_retry_enqueued_total: IntCounter,
    /// Count of tasks replayed from the ZK lane retry ring.
    pub zk_lane_retry_replayed_total: IntCounter,
    /// Count of tasks dropped after exhausting ZK lane retry attempts.
    pub zk_lane_retry_exhausted_total: IntCounter,
    /// Current number of tasks buffered in ZK lane dispatch backlog.
    pub zk_lane_pending_depth: GenericGauge<AtomicU64>,
    /// Current number of tasks buffered in the ZK lane retry ring.
    pub zk_lane_retry_ring_depth: GenericGauge<AtomicU64>,
    /// Events emitted when verifier cache hits/misses occur (labels: cache,event).
    pub zk_verifier_cache_events_total: IntCounterVec,
    /// Base gas charged when verifying a confidential proof.
    pub confidential_gas_base_verify: GenericGauge<AtomicU64>,
    /// Gas charged per public input exposed by a confidential proof.
    pub confidential_gas_per_public_input: GenericGauge<AtomicU64>,
    /// Gas charged per byte of a confidential proof.
    pub confidential_gas_per_proof_byte: GenericGauge<AtomicU64>,
    /// Gas charged per nullifier referenced by a confidential transaction.
    pub confidential_gas_per_nullifier: GenericGauge<AtomicU64>,
    /// Gas charged per commitment emitted by a confidential transaction.
    pub confidential_gas_per_commitment: GenericGauge<AtomicU64>,
    /// Lower 64 bits of the canonical IVM gas schedule hash.
    pub ivm_gas_schedule_hash_lo: GenericGauge<AtomicU64>,
    /// Upper 64 bits of the canonical IVM gas schedule hash.
    pub ivm_gas_schedule_hash_hi: GenericGauge<AtomicU64>,
    /// Requested/applied stack sizes for scheduler/prover/guest (bytes).
    pub ivm_stack_bytes: GenericGaugeVec<AtomicU64>,
    /// Stack clamp flags for scheduler/prover/guest (0 = no clamp, 1 = clamped).
    pub ivm_stack_clamped: GenericGaugeVec<AtomicU64>,
    /// Gas→stack multiplier currently in effect.
    pub ivm_stack_gas_multiplier: GenericGauge<AtomicU64>,
    /// Number of times a pre-existing global Rayon pool forced a stack-size fallback.
    pub ivm_stack_pool_fallback_total: IntCounter,
    /// VM constructions that hit the guest stack budget clamp.
    pub ivm_stack_budget_hit_total: IntCounter,
    /// Confidential Merkle-tree commitment counts per asset.
    pub confidential_tree_commitments: GenericGaugeVec<AtomicU64>,
    /// Confidential Merkle-tree depth per asset.
    pub confidential_tree_depth: GenericGaugeVec<AtomicU64>,
    /// Confidential Merkle-tree root history entries per asset.
    pub confidential_root_history_entries: GenericGaugeVec<AtomicU64>,
    /// Confidential frontier checkpoints per asset.
    pub confidential_frontier_checkpoints: GenericGaugeVec<AtomicU64>,
    /// Height of the latest recorded frontier checkpoint per asset.
    pub confidential_frontier_last_height: GenericGaugeVec<AtomicU64>,
    /// Commitment count captured at the latest frontier checkpoint per asset.
    pub confidential_frontier_last_commitments: GenericGaugeVec<AtomicU64>,
    /// Confidential root eviction counter per asset.
    pub confidential_root_evictions_total: IntCounterVec,
    /// Confidential frontier eviction counter per asset.
    pub confidential_frontier_evictions_total: IntCounterVec,
    /// Latest TWAP price exported by the oracle (local per XOR).
    pub oracle_price_local_per_xor: Gauge,
    /// TWAP window length used by the oracle (seconds).
    pub oracle_twap_window_seconds: GenericGauge<AtomicU64>,
    /// Effective haircut basis points applied by the oracle.
    pub oracle_haircut_basis_points: GenericGauge<AtomicU64>,
    /// Oracle staleness (seconds) at the time of the last settlement quote.
    pub oracle_staleness_seconds: Gauge,
    /// Count of observations aggregated per feed/slot.
    pub oracle_observations_total: IntCounterVec,
    /// Aggregation wall-clock duration (milliseconds) grouped by feed.
    pub oracle_aggregation_duration_ms: HistogramVec,
    /// Total oracle rewards emitted per feed (mantissa units).
    pub oracle_rewards_total: IntCounterVec,
    /// Total oracle penalties applied per feed (mantissa units).
    pub oracle_penalties_total: IntCounterVec,
    /// Total feed events aggregated per feed (regardless of evidence).
    pub oracle_feed_events_total: IntCounterVec,
    /// Feed events that carried at least one evidence hash.
    pub oracle_feed_events_with_evidence_total: IntCounterVec,
    /// Count of evidence hashes attached to feed events per feed.
    pub oracle_evidence_hashes_total: IntCounterVec,
    /// FASTPQ execution mode resolutions grouped by requested/resolved/backend/device labels.
    pub fastpq_execution_mode_total: IntCounterVec,
    /// FASTPQ Poseidon pipeline resolutions grouped by requested/resolved/path/device labels.
    pub fastpq_poseidon_pipeline_total: IntCounterVec,
    /// FASTPQ Metal queue duty-cycle ratios grouped by device/queue/metric.
    pub fastpq_metal_queue_ratio: GaugeVec,
    /// FASTPQ Metal queue depth snapshots grouped by device/metric.
    pub fastpq_metal_queue_depth: GaugeVec,
    /// FASTPQ host zero-fill duration samples grouped by device class (milliseconds).
    pub fastpq_zero_fill_duration_ms: GaugeVec,
    /// FASTPQ host zero-fill bandwidth grouped by device class (gigabits per second).
    pub fastpq_zero_fill_bandwidth_gbps: GaugeVec,
    /// Settlement events grouped by kind/outcome/reason.
    pub settlement_events_total: IntCounterVec,
    /// Settlement finality outcomes grouped by kind/outcome/state.
    pub settlement_finality_events_total: IntCounterVec,
    /// PvP FX window observations (milliseconds between committed legs).
    pub settlement_fx_window_ms: HistogramVec,
    /// Per-lane/dataspace settlement buffer level recorded in micro XOR.
    pub settlement_buffer_xor: GaugeVec,
    /// Configured settlement buffer capacity per lane/dataspace (micro XOR).
    pub settlement_buffer_capacity_xor: GaugeVec,
    /// Encoded settlement buffer status (0 = normal, 1 = alert, 2 = throttle, 3 = XOR-only, 4 = halt).
    pub settlement_buffer_status: GaugeVec,
    /// Per-lane/dataspace realised haircut variance recorded in micro XOR.
    pub settlement_pnl_xor: GaugeVec,
    /// Effective haircut basis points applied per lane/dataspace in the latest block.
    pub settlement_haircut_bp: GaugeVec,
    /// Swap-line utilisation snapshots (micro XOR) grouped by lane/dataspace/profile.
    pub settlement_swapline_utilisation: GaugeVec,
    /// Settlement conversion counters grouped by lane/dataspace/source token.
    pub settlement_conversion_total: IntCounterVec,
    /// Cumulative settlement haircut totals grouped by lane/dataspace (XOR units).
    pub settlement_haircut_total: CounterVec,
    /// Subscription billing attempts grouped by pricing kind.
    pub subscription_billing_attempts_total: IntCounterVec,
    /// Subscription billing outcomes grouped by pricing kind and result.
    pub subscription_billing_outcomes_total: IntCounterVec,
    /// Offline-to-online transfer lifecycle events grouped by event kind.
    pub offline_transfer_events_total: IntCounterVec,
    /// Aggregate offline receipt counters grouped by event kind.
    pub offline_transfer_receipts_total: IntCounterVec,
    /// Distribution of settled offline bundle amounts.
    pub offline_transfer_settled_amount: Histogram,
    /// Offline transfer validation rejections grouped by platform and reason.
    pub offline_transfer_rejections_total: IntCounterVec,
    /// Offline transfer bundles pruned from hot storage.
    pub offline_transfer_pruned_total: IntCounter,
    /// Offline attestation tokens processed grouped by integrity policy.
    pub offline_attestation_policy_total: IntCounterVec,
    /// iOS App Attest assertions accepted through the compatibility signature path.
    pub offline_app_attest_signature_compat_total: IntCounter,
    /// Viral incentive lifecycle events grouped by event kind.
    pub social_events_total: IntCounterVec,
    /// Latest viral reward budget spend for the active day.
    pub social_budget_spent: Gauge,
    /// Campaign spend across the full promotion window.
    pub social_campaign_spent: Gauge,
    /// Configured campaign cap (0 = unlimited).
    pub social_campaign_cap: Gauge,
    /// Remaining campaign budget (0 when cap is unlimited).
    pub social_campaign_remaining: Gauge,
    /// Whether the promotion window is active (1 = active, 0 = inactive).
    pub social_campaign_active: Gauge,
    /// Whether the viral flows are halted (1 = halted, 0 = flowing).
    pub social_halted: Gauge,
    /// Viral incentive rejections grouped by failure reason.
    pub social_rejections_total: IntCounterVec,
    /// Multisig direct-sign validation rejections.
    pub multisig_direct_sign_reject_total: IntCounter,
    /// Open viral escrows currently tracked on-ledger.
    pub social_open_escrows: GenericGauge<AtomicU64>,
    /// Transactions currently queued as observed by consensus.
    pub sumeragi_tx_queue_depth: GenericGauge<AtomicU64>,
    /// Transaction queue capacity observed by consensus.
    pub sumeragi_tx_queue_capacity: GenericGauge<AtomicU64>,
    /// Queue saturation flag observed by consensus (0 = healthy, 1 = saturated).
    pub sumeragi_tx_queue_saturated: GenericGauge<AtomicU64>,
    /// Total pending blocks tracked by consensus.
    pub sumeragi_pending_blocks_total: GenericGauge<AtomicU64>,
    /// Pending blocks that currently gate proposals/view changes.
    pub sumeragi_pending_blocks_blocking: GenericGauge<AtomicU64>,
    /// Commit inflight queue depth (inflight + queued commit work).
    pub sumeragi_commit_inflight_queue_depth: GenericGauge<AtomicU64>,
    /// Outstanding missing-block requests observed locally.
    pub sumeragi_missing_block_requests: GenericGauge<AtomicU64>,
    /// Age in milliseconds of the oldest missing-block request.
    pub sumeragi_missing_block_oldest_ms: GenericGauge<AtomicU64>,
    /// Retry window for missing-block fetches in milliseconds.
    pub sumeragi_missing_block_retry_window_ms: GenericGauge<AtomicU64>,
    /// Dwell time from first QC arrival until payload observation (milliseconds).
    pub sumeragi_missing_block_dwell_ms: Histogram,
    /// Epoch length in blocks for NPoS scheduling (0 when not applicable).
    pub sumeragi_epoch_length_blocks: GenericGauge<AtomicU64>,
    /// Commit window deadline offset from epoch start, in blocks.
    pub sumeragi_epoch_commit_deadline_offset: GenericGauge<AtomicU64>,
    /// Reveal window deadline offset from epoch start, in blocks.
    pub sumeragi_epoch_reveal_deadline_offset: GenericGauge<AtomicU64>,
    /// Tiered state: entries retained in the hot tier after the latest snapshot.
    pub state_tiered_hot_entries: GenericGauge<AtomicU64>,
    /// Tiered state: bytes retained in the hot tier after the latest snapshot.
    pub state_tiered_hot_bytes: GenericGauge<AtomicU64>,
    /// Tiered state: entries spilled to the cold tier after the latest snapshot.
    pub state_tiered_cold_entries: GenericGauge<AtomicU64>,
    /// Tiered state: total bytes written to the cold tier in the latest snapshot.
    pub state_tiered_cold_bytes: GenericGauge<AtomicU64>,
    /// Tiered state: cold entries reused without re-encoding in the latest snapshot.
    pub state_tiered_cold_reused_entries: GenericGauge<AtomicU64>,
    /// Tiered state: total bytes reused from cold payloads in the latest snapshot.
    pub state_tiered_cold_reused_bytes: GenericGauge<AtomicU64>,
    /// Tiered state: entries promoted into the hot tier in the latest snapshot.
    pub state_tiered_hot_promotions: GenericGauge<AtomicU64>,
    /// Tiered state: entries demoted into the cold tier in the latest snapshot.
    pub state_tiered_hot_demotions: GenericGauge<AtomicU64>,
    /// Tiered state: hot-tier key budget overflow caused by grace retention.
    pub state_tiered_hot_grace_overflow_keys: GenericGauge<AtomicU64>,
    /// Tiered state: hot-tier byte budget overflow caused by grace retention.
    pub state_tiered_hot_grace_overflow_bytes: GenericGauge<AtomicU64>,
    /// Tiered state: last recorded snapshot index.
    pub state_tiered_last_snapshot_index: GenericGauge<AtomicU64>,
    /// Storage budget: bytes used per component.
    pub storage_budget_bytes_used: GenericGaugeVec<AtomicU64>,
    /// Storage budget: configured cap per component.
    pub storage_budget_bytes_limit: GenericGaugeVec<AtomicU64>,
    /// Storage budget: cap exceed events per component.
    pub storage_budget_exceeded_total: IntCounterVec,
    /// DA storage: cache outcomes per component.
    pub storage_da_cache_total: IntCounterVec,
    /// DA storage: churn bytes per component and direction.
    pub storage_da_churn_bytes_total: IntCounterVec,
    /// Governance: proposal counts grouped by status
    pub governance_proposals_status: GenericGaugeVec<AtomicU64>,
    /// Governance: latest council members count.
    pub governance_council_members: GenericGauge<AtomicU64>,
    /// Governance: latest council alternates count.
    pub governance_council_alternates: GenericGauge<AtomicU64>,
    /// Governance: total candidates considered in the latest draw.
    pub governance_council_candidates: GenericGauge<AtomicU64>,
    /// Governance: verified VRF proofs in the latest draw.
    pub governance_council_verified: GenericGauge<AtomicU64>,
    /// Governance: epoch index of the latest persisted council.
    pub governance_council_epoch: GenericGauge<AtomicU64>,
    /// Governance: total registered citizens.
    pub governance_citizens_total: GenericGauge<AtomicU64>,
    /// Governance: citizen service discipline events (decline|no_show|misconduct).
    pub governance_citizen_service_events_total: IntCounterVec,
    /// Governance: protected-namespace enforcement counters (outcome = allowed|rejected)
    pub governance_protected_namespace_total: IntCounterVec,
    /// Governance: manifest admission outcomes (result label)
    pub governance_manifest_admission_total: IntCounterVec,
    /// Governance: manifest quorum enforcement counters (outcome = satisfied|rejected)
    pub governance_manifest_quorum_total: IntCounterVec,
    /// Governance: manifest hook enforcement counters (hook, outcome)
    pub governance_manifest_hook_total: IntCounterVec,
    /// Governance: manifest activation events (event = manifest_inserted|instance_bound)
    pub governance_manifest_activations_total: IntCounterVec,
    /// Governance: recent manifest activations kept for status snapshots
    pub governance_manifest_recent: Arc<RwLock<VecDeque<GovernanceManifestActivation>>>,
    /// Governance: bond lifecycle events (lock_created|lock_extended|lock_unlocked).
    pub governance_bond_events_total: IntCounterVec,
    /// Cached Taikai ingest telemetry per (cluster, stream) for status snapshots.
    taikai_ingest_snapshots: Arc<RwLock<BTreeMap<(String, String), TaikaiIngestSnapshotInternal>>>,
    /// Insertion order for Taikai ingest snapshots (bounded).
    taikai_ingest_snapshot_order: Arc<RwLock<VecDeque<(String, String)>>>,
    /// Cached DA receipt cursors keyed by (lane, epoch) for status snapshots.
    da_receipt_cursors: Arc<RwLock<BTreeMap<DaReceiptCursorKey, u64>>>,
    taikai_alias_rotation_snapshots: TaikaiAliasRotationSnapshots,
    /// Alias service usage grouped by lane and event kind.
    pub alias_usage_total: IntCounterVec,
    /// PSP fraud: accepted assessments by tenant/band/lane/subnet
    pub fraud_psp_assessments_total: IntCounterVec,
    /// PSP fraud: transactions missing assessments (labeled by cause)
    pub fraud_psp_missing_assessment_total: IntCounterVec,
    /// PSP fraud: invalid metadata fields encountered during admission
    pub fraud_psp_invalid_metadata_total: IntCounterVec,
    /// PSP fraud: attestation verification outcomes (tenant/engine/lane/status)
    pub fraud_psp_attestation_total: IntCounterVec,
    /// PSP fraud: latency histogram as reported by PSPs (milliseconds)
    pub fraud_psp_latency_ms: HistogramVec,
    /// PSP fraud: risk score distribution (basis points)
    pub fraud_psp_score_bps: HistogramVec,
    /// PSP fraud: outcome mismatches between scoring and PSP disposition
    pub fraud_psp_outcome_mismatch_total: IntCounterVec,
    /// Streaming HPKE rekeys accepted grouped by suite identifier.
    pub streaming_hpke_rekeys_total: IntCounterVec,
    /// Streaming content key rotations processed.
    pub streaming_gck_rotations_total: IntCounter,
    /// Streaming QUIC datagrams sent.
    pub streaming_quic_datagrams_sent_total: IntCounter,
    /// Streaming QUIC datagrams dropped.
    pub streaming_quic_datagrams_dropped_total: IntCounter,
    /// Streaming FEC parity bucket occupancy.
    pub streaming_fec_parity_current: GenericGaugeVec<AtomicU64>,
    /// Streaming feedback timeout events.
    pub streaming_feedback_timeout_total: IntCounter,
    /// Streaming SoraNet privacy-route provisioning failures.
    pub streaming_soranet_provision_fail_total: IntCounter,
    /// Streaming SoraNet provisioning queue drops grouped by reason.
    pub streaming_soranet_provision_queue_drop_total: IntCounterVec,
    /// Telemetry redaction events grouped by reason.
    pub telemetry_redaction_total: IntCounterVec,
    /// Telemetry redaction skips grouped by reason.
    pub telemetry_redaction_skipped_total: IntCounterVec,
    /// Telemetry field truncations.
    pub telemetry_truncation_total: IntCounter,
    /// Streaming privacy telemetry redaction failures.
    pub streaming_privacy_redaction_fail_total: IntCounter,
    /// Streaming encode latency (milliseconds).
    pub streaming_encode_latency_ms: Histogram,
    /// Streaming encode audio jitter (milliseconds).
    pub streaming_encode_audio_jitter_ms: Histogram,
    /// Streaming encode maximum audio jitter (milliseconds).
    pub streaming_encode_audio_max_jitter_ms: GenericGauge<AtomicU64>,
    /// ISO bridge reference-data status (-1 failed, 0 missing, 1 loaded).
    pub iso_reference_status: IntGaugeVec,
    /// ISO bridge reference-data age in seconds (per dataset).
    pub iso_reference_age_seconds: IntGaugeVec,
    /// ISO bridge reference-data record counts.
    pub iso_reference_records: IntGaugeVec,
    /// ISO bridge reference-data refresh interval (seconds).
    pub iso_reference_refresh_interval_secs: IntGaugeVec,
    /// Streaming encode dropped layers.
    pub streaming_encode_dropped_layers_total: IntCounter,
    /// Streaming decode buffer size (milliseconds).
    pub streaming_decode_buffer_ms: Histogram,
    /// Streaming decode dropped frames.
    pub streaming_decode_dropped_frames_total: IntCounter,
    /// Streaming decode maximum queue depth (milliseconds).
    pub streaming_decode_max_queue_ms: Histogram,
    /// Streaming decode audio/video drift (milliseconds, absolute average).
    pub streaming_decode_av_drift_ms: Histogram,
    /// Streaming decode maximum audio/video drift (milliseconds).
    pub streaming_decode_max_drift_ms: GenericGauge<AtomicU64>,
    /// Viewer-reported audio jitter (milliseconds).
    pub streaming_audio_jitter_ms: Histogram,
    /// Viewer-reported maximum audio jitter (milliseconds).
    pub streaming_audio_max_jitter_ms: GenericGauge<AtomicU64>,
    /// Viewer-reported audio/video drift (milliseconds, absolute average).
    pub streaming_av_drift_ms: Histogram,
    /// Viewer-reported maximum audio/video drift (milliseconds).
    pub streaming_av_max_drift_ms: GenericGauge<AtomicU64>,
    /// Viewer-reported EWMA audio/video drift (milliseconds, signed).
    pub streaming_av_drift_ewma_ms: IntGauge,
    /// Aggregation window for viewer sync diagnostics (milliseconds).
    pub streaming_av_sync_window_ms: GenericGauge<AtomicU64>,
    /// Viewer sync violations observed (count).
    pub streaming_av_sync_violation_total: IntCounter,
    /// Streaming network round-trip time (milliseconds).
    pub streaming_network_rtt_ms: Histogram,
    /// Streaming network packet loss percentage (basis points).
    pub streaming_network_loss_percent_x100: Histogram,
    /// Streaming network FEC repairs performed.
    pub streaming_network_fec_repairs_total: IntCounter,
    /// Streaming network FEC failures encountered.
    pub streaming_network_fec_failures_total: IntCounter,
    /// Streaming network datagram reinjects issued.
    pub streaming_network_datagram_reinjects_total: IntCounter,
    /// Streaming energy consumption at encoder (milliwatts).
    pub streaming_energy_encoder_mw: Histogram,
    /// Streaming energy consumption at decoder (milliwatts).
    pub streaming_energy_decoder_mw: Histogram,
    /// Routed-trace audit outcomes grouped by trace identifier and status.
    pub nexus_audit_outcome_total: IntCounterVec,
    /// UNIX timestamp (seconds) of the most recent routed-trace audit outcome per trace.
    pub nexus_audit_outcome_last_timestamp: GenericGaugeVec<AtomicU64>,
    /// Space Directory manifest revisions observed per dataspace.
    pub nexus_space_directory_revision_total: IntCounterVec,
    /// Active UAID capability manifests per dataspace/profile.
    pub nexus_space_directory_active_manifests: GenericGaugeVec<AtomicU64>,
    /// Capability manifest revocations grouped by dataspace and reason.
    pub nexus_space_directory_revocations_total: IntCounterVec,
    /// Kaigi: relay registrations grouped by domain.
    pub kaigi_relay_registered_total: IntCounterVec,
    /// Kaigi: bandwidth class distribution for relay registrations.
    pub kaigi_relay_registration_bandwidth: HistogramVec,
    /// Kaigi: relay manifest updates grouped by domain and action.
    pub kaigi_relay_manifest_updates_total: IntCounterVec,
    /// Kaigi: relay manifest hop-count distribution per domain.
    pub kaigi_relay_manifest_hop_count: HistogramVec,
    /// Kaigi: relay failovers grouped by domain and call.
    pub kaigi_relay_failover_total: IntCounterVec,
    /// Kaigi: relay failover hop-count distribution per domain.
    pub kaigi_relay_failover_hop_count: HistogramVec,
    /// Kaigi: relay health reports grouped by domain and status.
    pub kaigi_relay_health_reports_total: IntCounterVec,
    /// Kaigi: current relay health state labelled by domain and relay.
    pub kaigi_relay_health_state: IntGaugeVec,
    /// Number of sumeragi dropped messages
    pub dropped_messages: DroppedMessagesCounter,
    /// Number of dropped Sumeragi block messages due to full channel (consensus path)
    pub sumeragi_dropped_block_messages_total: IntCounter,
    /// Number of dropped Sumeragi control messages due to full channel (control path)
    pub sumeragi_dropped_control_messages_total: IntCounter,
    /// Sumeragi: votes accepted at proxy tail (cumulative)
    pub sumeragi_tail_votes_total: IntCounter,
    /// Sumeragi: votes sent grouped by phase (prevote, precommit, available)
    pub sumeragi_votes_sent_total: IntCounterVec,
    /// Sumeragi: votes received grouped by phase (prevote, precommit, available)
    pub sumeragi_votes_received_total: IntCounterVec,
    /// Sumeragi: quorum certificates sent grouped by kind (prevote, precommit, available)
    pub sumeragi_qc_sent_total: IntCounterVec,
    /// Sumeragi: quorum certificates received grouped by kind (prevote, precommit, available)
    pub sumeragi_qc_received_total: IntCounterVec,
    /// Sumeragi: QC validation errors grouped by reason.
    pub sumeragi_qc_validation_errors_total: IntCounterVec,
    /// Sumeragi: validation rejects before voting grouped by reason.
    pub sumeragi_validation_reject_total: IntCounterVec,
    /// Sumeragi: validation gate last reject reason code (0=none, 1=stateless, 2=execution, 3=prev_hash, 4=prev_height, 5=topology).
    pub sumeragi_validation_reject_last_reason: GenericGauge<AtomicU64>,
    /// Sumeragi: block height of the last validation gate reject (0 when unset).
    pub sumeragi_validation_reject_last_height: GenericGauge<AtomicU64>,
    /// Sumeragi: view of the last validation gate reject (0 when unset).
    pub sumeragi_validation_reject_last_view: GenericGauge<AtomicU64>,
    /// Sumeragi: unix timestamp (ms) of the last validation gate reject (0 when unset).
    pub sumeragi_validation_reject_last_timestamp_ms: GenericGauge<AtomicU64>,
    /// Sumeragi: block-sync roster selection grouped by source.
    pub sumeragi_block_sync_roster_source_total: IntCounterVec,
    /// Sumeragi: block-sync roster drops grouped by reason.
    pub sumeragi_block_sync_roster_drop_total: IntCounterVec,
    /// Sumeragi: block-sync ShareBlocks dropped because no request was tracked.
    pub sumeragi_block_sync_share_blocks_unsolicited_total: IntCounter,
    /// Sumeragi: consensus message drops/deferrals grouped by kind, outcome, and reason.
    pub sumeragi_consensus_message_handling_total: IntCounterVec,
    /// Sumeragi: commit-conflict detections (cumulative).
    pub sumeragi_commit_conflict_detected_total: IntCounter,
    /// Sumeragi: view-change triggers grouped by cause.
    pub sumeragi_view_change_cause_total: IntCounterVec,
    /// Sumeragi: unix timestamp (ms) of the last view-change trigger grouped by cause.
    pub sumeragi_view_change_cause_last_timestamp_ms: GenericGaugeVec<AtomicU64>,
    /// Sumeragi: QC signer tallies grouped by phase and whether the signer was counted for quorum.
    pub sumeragi_qc_signer_counts: HistogramVec,
    /// Sumeragi: invalid-signature drops grouped by message kind and throttle outcome.
    pub sumeragi_invalid_signature_total: IntCounterVec,
    /// Sumeragi: widen-before-rotate events (cumulative)
    pub sumeragi_widen_before_rotate_total: IntCounter,
    /// Sumeragi: view-change suggestions emitted (cumulative)
    pub sumeragi_view_change_suggest_total: IntCounter,
    /// Sumeragi: view-change installs observed (cumulative)
    pub sumeragi_view_change_install_total: IntCounter,
    /// Sumeragi: view-change rotations after no proposal observed before cutoff (cumulative).
    pub sumeragi_proposal_gap_total: IntCounter,
    /// Sumeragi: view-change proof counters grouped by outcome (accepted|stale|rejected)
    pub sumeragi_view_change_proof_total: GenericGaugeVec<AtomicU64>,
    /// Sumeragi: Witness-availability QC assembled (cumulative)
    pub sumeragi_wa_qc_assembled_total: IntCounter,
    /// Sumeragi: certificate size histogram (signatures per committed block)
    pub sumeragi_cert_size: Histogram,
    /// Sumeragi: signatures present on the block during commit validation (all roles).
    pub sumeragi_commit_signatures_present: GenericGauge<AtomicU64>,
    /// Sumeragi: signatures counted toward the commit quorum (leader + validators in Set A/B).
    pub sumeragi_commit_signatures_counted: GenericGauge<AtomicU64>,
    /// Sumeragi: Set B validator signatures present on the block during commit validation.
    pub sumeragi_commit_signatures_set_b: GenericGauge<AtomicU64>,
    /// Sumeragi: required commit quorum size for the active topology.
    pub sumeragi_commit_signatures_required: GenericGauge<AtomicU64>,
    /// Sumeragi: latest commit certificate height (best-effort).
    pub sumeragi_commit_qc_height: GenericGauge<AtomicU64>,
    /// Sumeragi: latest commit certificate view (best-effort).
    pub sumeragi_commit_qc_view: GenericGauge<AtomicU64>,
    /// Sumeragi: latest commit certificate epoch (best-effort).
    pub sumeragi_commit_qc_epoch: GenericGauge<AtomicU64>,
    /// Sumeragi: signatures attached to the latest commit certificate.
    pub sumeragi_commit_qc_signatures_total: GenericGauge<AtomicU64>,
    /// Sumeragi: validator-set size for the latest commit certificate.
    pub sumeragi_commit_qc_validator_set_len: GenericGauge<AtomicU64>,
    /// Sumeragi: redundant vote sends (cumulative)
    pub sumeragi_redundant_sends_total: IntCounter,
    /// Sumeragi: redundant vote sends by collector index (labeled by `idx`)
    pub sumeragi_redundant_sends_by_collector: IntCounterVec,
    /// Sumeragi: redundant vote sends by collector peer id (labeled by `peer`)
    pub sumeragi_redundant_sends_by_peer: IntCounterVec,
    /// Sumeragi: current collectors_k (gauge)
    pub sumeragi_collectors_k: GenericGauge<AtomicU64>,
    /// Sumeragi: current redundant_send_r (gauge)
    pub sumeragi_redundant_send_r: GenericGauge<AtomicU64>,
    /// Sumeragi: gossip fallback invocations (collectors exhausted).
    pub sumeragi_gossip_fallback_total: IntCounter,
    /// Sumeragi: BlockCreated drops due to locked QC gate (sanity check failures).
    pub sumeragi_block_created_dropped_by_lock_total: IntCounter,
    /// Sumeragi: BlockCreated rejects due to hint mismatch (height/view/parent).
    pub sumeragi_block_created_hint_mismatch_total: IntCounter,
    /// Sumeragi: BlockCreated rejects due to proposal mismatch (header/payload).
    pub sumeragi_block_created_proposal_mismatch_total: IntCounter,
    /// Nexus: lane relay envelopes rejected during validation (grouped by error kind).
    pub lane_relay_invalid_total: IntCounterVec,
    /// Nexus: emergency validator override usage for lane relay (grouped by outcome).
    pub lane_relay_emergency_override_total: IntCounterVec,
    /// Sumeragi: number of collectors targeted for the current voting block (gauge)
    pub sumeragi_collectors_targeted_current: GenericGauge<AtomicU64>,
    /// Sumeragi: histogram of collectors targeted per block (observed at commit)
    pub sumeragi_collectors_targeted_per_block: Histogram,
    /// Sumeragi: latest PRF epoch seed (hex) observed for collector selection.
    pub sumeragi_prf_epoch_seed_hex: Arc<RwLock<Option<String>>>,
    /// Snapshot of Halo2 verifier configuration for status endpoints.
    pub halo2_status: Arc<RwLock<Halo2Status>>,
    /// Sumeragi: height associated with the current PRF context.
    pub sumeragi_prf_height: GenericGauge<AtomicU64>,
    /// Sumeragi: view associated with the current PRF context.
    pub sumeragi_prf_view: GenericGauge<AtomicU64>,
    /// Sumeragi: deterministic membership view hash (truncated to u64).
    pub sumeragi_membership_view_hash: GenericGauge<AtomicU64>,
    /// Sumeragi: height associated with the membership view hash snapshot.
    pub sumeragi_membership_height: GenericGauge<AtomicU64>,
    /// Sumeragi: view associated with the membership view hash snapshot.
    pub sumeragi_membership_view: GenericGauge<AtomicU64>,
    /// Sumeragi: epoch associated with the membership view hash snapshot.
    pub sumeragi_membership_epoch: GenericGauge<AtomicU64>,
    /// NPoS: number of times this node was selected as a PRF collector (cumulative)
    pub sumeragi_npos_collector_selected_total: IntCounter,
    /// NPoS: PRF collector assignments by topology index (labeled by `idx`)
    pub sumeragi_npos_collector_assignments_by_idx: IntCounterVec,
    /// VRF: commits broadcast by this validator (cumulative)
    pub sumeragi_vrf_commits_emitted_total: IntCounter,
    /// VRF: reveals broadcast by this validator (cumulative)
    pub sumeragi_vrf_reveals_emitted_total: IntCounter,
    /// VRF: reveals accepted after the reveal window (cumulative)
    pub sumeragi_vrf_reveals_late_total: IntCounter,
    /// VRF: total non-reveal penalties applied in last rollover (cumulative)
    pub sumeragi_vrf_non_reveal_penalties_total: IntCounter,
    /// VRF: non-reveal penalties by signer index (labeled by `idx`)
    pub sumeragi_vrf_non_reveal_by_signer: IntCounterVec,
    /// VRF: total no-participation penalties applied in last rollover (cumulative)
    pub sumeragi_vrf_no_participation_total: IntCounter,
    /// VRF: no-participation penalties by signer (labeled by `idx`)
    pub sumeragi_vrf_no_participation_by_signer: IntCounterVec,
    /// VRF: commit/reveal rejects by reason (epoch_mismatch | out_of_window | invalid_reveal)
    pub sumeragi_vrf_rejects_total_by_reason: IntCounterVec,
    /// Sumeragi: redundant vote sends by local validator index (labeled by `role_idx`)
    pub sumeragi_redundant_sends_by_role_idx: IntCounterVec,
    /// Sumeragi: current runtime mode tag.
    pub sumeragi_mode_tag: Arc<RwLock<String>>,
    /// Sumeragi: staged mode tag if activation is pending.
    pub sumeragi_staged_mode_tag: Arc<RwLock<Option<String>>>,
    /// Sumeragi: activation height for staged mode (if any).
    pub sumeragi_staged_mode_activation_height: Arc<RwLock<Option<u64>>>,
    /// Sumeragi: blocks elapsed since a staged mode activation height passed without flipping.
    pub sumeragi_mode_activation_lag_blocks: IntGauge,
    /// Sumeragi: optional lag snapshot for status payloads.
    pub sumeragi_mode_activation_lag_blocks_opt: Arc<RwLock<Option<u64>>>,
    /// Sumeragi: whether runtime mode flips are enabled (1) or disabled (0).
    pub sumeragi_mode_flip_kill_switch: IntGauge,
    /// Sumeragi: successful runtime mode flips (labeled by `mode_tag`).
    pub sumeragi_mode_flip_success_total: IntCounterVec,
    /// Sumeragi: failed runtime mode flip attempts (labeled by `mode_tag`).
    pub sumeragi_mode_flip_failure_total: IntCounterVec,
    /// Sumeragi: blocked runtime mode flip attempts (labeled by `mode_tag`).
    pub sumeragi_mode_flip_blocked_total: IntCounterVec,
    /// Sumeragi: timestamp of the last flip attempt (ms since UNIX epoch).
    pub sumeragi_last_mode_flip_timestamp_ms: IntGauge,
    /// Sumeragi: current leader index (gauge)
    pub sumeragi_leader_index: GenericGauge<AtomicU64>,
    /// Sumeragi: highest QC height (gauge)
    pub sumeragi_highest_qc_height: GenericGauge<AtomicU64>,
    /// Sumeragi: locked QC height (gauge)
    pub sumeragi_locked_qc_height: GenericGauge<AtomicU64>,
    /// Sumeragi: locked QC view (gauge)
    pub sumeragi_locked_qc_view: GenericGauge<AtomicU64>,
    /// Sumeragi: NEW_VIEW receipts per (height, view)
    pub sumeragi_new_view_receipts_by_hv: GenericGaugeVec<AtomicU64>,
    /// Sumeragi: NEW_VIEW messages published (cumulative)
    pub sumeragi_new_view_publish_total: IntCounter,
    /// Sumeragi: NEW_VIEW messages received and accepted (cumulative)
    pub sumeragi_new_view_recv_total: IntCounter,
    /// Sumeragi: NEW_VIEW messages dropped because HighestQC is behind the locked QC
    pub sumeragi_new_view_dropped_by_lock_total: IntCounter,
    /// Sumeragi: missing-block fetch planning outcomes (labels: outcome=requested|backoff|no_targets)
    pub sumeragi_missing_block_fetch_total: IntCounterVec,
    /// Sumeragi: missing-block fetch target kind (labels: target=signers|topology)
    pub sumeragi_missing_block_fetch_target_total: IntCounterVec,
    /// Sumeragi: elapsed milliseconds from first-seen certificate to missing-block fetch request
    pub sumeragi_missing_block_fetch_dwell_ms: Histogram,
    /// Sumeragi: number of peers targeted when requesting a missing block payload
    pub sumeragi_missing_block_fetch_targets: Histogram,
    /// Block-sync QCs quarantined because local context was missing.
    pub blocksync_qc_quarantine_total: IntCounter,
    /// Quarantined block-sync QCs that were revalidated successfully.
    pub blocksync_qc_revalidated_total: IntCounter,
    /// Block-sync QCs dropped permanently after bounded revalidation.
    pub blocksync_qc_final_drop_total: IntCounterVec,
    /// QCs deferred due to missing payload.
    pub qc_deferred_missing_payload_total: IntCounter,
    /// Deferred QCs resolved after payload arrival.
    pub qc_deferred_resolved_total: IntCounter,
    /// Deferred QCs expired after bounded retries.
    pub qc_deferred_expired_total: IntCounter,
    /// Consensus deferrals caused by empty commit topology.
    pub consensus_empty_commit_topology_defer_total: IntCounter,
    /// Empty-topology recoveries escalated to forced view changes.
    pub consensus_empty_commit_topology_escalation_total: IntCounter,
    /// Recovery state-machine transitions labeled by state.
    pub consensus_recovery_state_transitions_total: IntCounterVec,
    /// Height-scoped missing-block recoveries escalated via deterministic hard cap.
    pub consensus_missing_block_height_escalation_total: IntCounter,
    /// Sidecar mismatches quarantined in fail-closed mode.
    pub consensus_sidecar_quarantine_total: IntCounter,
    /// Sidecar mismatches final-dropped after retry/TTL bounds.
    pub consensus_sidecar_final_drop_total: IntCounter,
    /// Range-pull escalation attempts triggered by dependency recovery.
    pub blocksync_range_pull_escalation_total: IntCounter,
    /// Successful range-pull recoveries.
    pub blocksync_range_pull_success_total: IntCounter,
    /// Range-pull recoveries that expired without progress.
    pub blocksync_range_pull_failure_total: IntCounter,
    /// Stuck-round duration observed while recovery waits for dependencies.
    pub consensus_recovery_stuck_round_seconds: Histogram,
    /// Sumeragi DA availability: missing availability artifacts (labeled by reason)
    pub sumeragi_da_gate_block_total: IntCounterVec,
    /// Sumeragi DA availability: last recorded reason code (0=none,1=missing_local_data,3=manifest_missing,4=manifest_hash_mismatch,5=manifest_read_failed,6=manifest_spool_scan)
    pub sumeragi_da_gate_last_reason: GenericGauge<AtomicU64>,
    /// Sumeragi DA availability: last satisfaction code (0=none,1=missing_data_recovered)
    pub sumeragi_da_gate_last_satisfied: GenericGauge<AtomicU64>,
    /// Sumeragi DA availability: satisfaction transitions (labeled by gate)
    pub sumeragi_da_gate_satisfied_total: IntCounterVec,
    /// Sumeragi DA manifest guard: outcomes labeled by result/reason.
    pub sumeragi_da_manifest_guard_total: IntCounterVec,
    /// Sumeragi DA manifest cache: outcomes labeled by result.
    pub sumeragi_da_manifest_cache_total: IntCounterVec,
    /// Sumeragi DA spool cache: outcomes labeled by kind/result.
    pub sumeragi_da_spool_cache_total: IntCounterVec,
    /// Sumeragi DA pin intent spool: outcomes labeled by result/reason.
    pub sumeragi_da_pin_intent_spool_total: IntCounterVec,
    /// Sumeragi RBC: active sessions (gauge)
    pub sumeragi_rbc_sessions_active: GenericGauge<AtomicU64>,
    /// Sumeragi RBC: sessions pruned due to TTL (cumulative)
    pub sumeragi_rbc_sessions_pruned_total: IntCounter,
    /// Sumeragi RBC: READY broadcasts sent (cumulative)
    pub sumeragi_rbc_ready_broadcasts_total: IntCounter,
    /// Sumeragi RBC: rebroadcasts skipped (kind=payload|ready)
    pub sumeragi_rbc_rebroadcast_skipped_total: IntCounterVec,
    /// Sumeragi RBC: DELIVER broadcasts sent (cumulative)
    pub sumeragi_rbc_deliver_broadcasts_total: IntCounter,
    /// Sumeragi RBC: total payload bytes delivered and cached (gauge)
    pub sumeragi_rbc_payload_bytes_delivered_total: GenericGauge<AtomicU64>,
    /// Pending RBC backlog aggregated per lane (tx count).
    pub sumeragi_rbc_lane_tx_count: GenericGaugeVec<AtomicU64>,
    /// Total RBC chunks aggregated per lane.
    pub sumeragi_rbc_lane_total_chunks: GenericGaugeVec<AtomicU64>,
    /// Pending RBC chunks aggregated per lane.
    pub sumeragi_rbc_lane_pending_chunks: GenericGaugeVec<AtomicU64>,
    /// Total RBC payload bytes aggregated per lane.
    pub sumeragi_rbc_lane_bytes_total: GenericGaugeVec<AtomicU64>,
    /// Pending RBC backlog aggregated per dataspace (tx count).
    pub sumeragi_rbc_dataspace_tx_count: GenericGaugeVec<AtomicU64>,
    /// Total RBC chunks aggregated per dataspace.
    pub sumeragi_rbc_dataspace_total_chunks: GenericGaugeVec<AtomicU64>,
    /// Pending RBC chunks aggregated per dataspace.
    pub sumeragi_rbc_dataspace_pending_chunks: GenericGaugeVec<AtomicU64>,
    /// Total RBC payload bytes aggregated per dataspace.
    pub sumeragi_rbc_dataspace_bytes_total: GenericGaugeVec<AtomicU64>,
    /// Sumeragi availability: votes ingested by this collector (cumulative)
    pub sumeragi_da_votes_ingested_total: IntCounter,
    /// Sumeragi availability: votes ingested labeled by collector topology index
    pub sumeragi_da_votes_ingested_by_collector: IntCounterVec,
    /// Sumeragi availability: votes ingested labeled by peer id
    pub sumeragi_da_votes_ingested_by_peer: IntCounterVec,
    /// Sumeragi QC assembly latency histogram (milliseconds) labeled by `kind`
    pub sumeragi_qc_assembly_latency_ms: HistogramVec,
    /// Sumeragi QC last observed latency gauge (milliseconds) labeled by `kind`
    pub sumeragi_qc_last_latency_ms: GenericGaugeVec<AtomicU64>,
    /// Sumeragi RBC: persisted store sessions (gauge)
    pub sumeragi_rbc_store_sessions: GenericGauge<AtomicU64>,
    /// Sumeragi RBC: persisted store payload bytes (gauge)
    pub sumeragi_rbc_store_bytes: GenericGauge<AtomicU64>,
    /// Sumeragi RBC: current store pressure level (0=normal,1=soft,2=hard)
    pub sumeragi_rbc_store_pressure: GenericGauge<AtomicU64>,
    /// Sumeragi RBC: session evictions due to TTL/capacity enforcement (cumulative)
    pub sumeragi_rbc_store_evictions_total: IntCounter,
    /// Sumeragi RBC: persist requests dropped due to a full async queue (cumulative)
    pub sumeragi_rbc_persist_drops_total: IntCounter,
    /// Sumeragi RBC: proposals deferred due to store back-pressure (cumulative)
    pub sumeragi_rbc_backpressure_deferrals_total: IntCounter,
    /// Sumeragi RBC: DELIVER deferrals waiting on READY quorum (cumulative)
    pub sumeragi_rbc_deliver_defer_ready_total: IntCounter,
    /// Sumeragi RBC: DELIVER deferrals waiting on missing chunks (cumulative)
    pub sumeragi_rbc_deliver_defer_chunks_total: IntCounter,
    /// Sumeragi RBC: DA deadline reschedules triggered (cumulative)
    pub sumeragi_rbc_da_reschedule_total: IntCounter,
    /// Sumeragi RBC: DA deadline reschedules triggered (cumulative) labeled by consensus mode
    pub sumeragi_rbc_da_reschedule_by_mode_total: IntCounterVec,
    /// Sumeragi RBC: pending blocks aborted due to missing/mismatched/invalid RBC payload (labeled by consensus mode)
    pub sumeragi_rbc_abort_total: IntCounterVec,
    /// Sumeragi RBC: payload mismatches attributed to peers (labels: peer, kind)
    pub sumeragi_rbc_mismatch_total: IntCounterVec,
    /// Sumeragi: kura persistence failures grouped by outcome (retry|abort)
    pub sumeragi_kura_store_failures_total: IntCounterVec,
    /// Sumeragi: last recorded kura persistence retry attempt (gauge)
    pub sumeragi_kura_store_last_retry_attempt: GenericGauge<AtomicU64>,
    /// Sumeragi: last recorded kura persistence retry backoff in milliseconds (gauge)
    pub sumeragi_kura_store_last_retry_backoff_ms: GenericGauge<AtomicU64>,
    /// Sumeragi pacemaker: proposals deferred due to transaction-queue back-pressure (cumulative)
    pub sumeragi_pacemaker_backpressure_deferrals_total: IntCounter,
    /// Sumeragi pacemaker: backpressure deferrals grouped by reason (cumulative)
    pub sumeragi_pacemaker_backpressure_deferrals_by_reason_total: IntCounterVec,
    /// Sumeragi pacemaker: backpressure deferral durations (ms) grouped by reason
    pub sumeragi_pacemaker_backpressure_deferral_duration_ms: HistogramVec,
    /// Sumeragi pacemaker: backpressure deferral active state (0/1) grouped by reason
    pub sumeragi_pacemaker_backpressure_deferral_active: GenericGaugeVec<AtomicU64>,
    /// Sumeragi pacemaker: backpressure deferral age (ms) grouped by reason
    pub sumeragi_pacemaker_backpressure_deferral_age_ms: GenericGaugeVec<AtomicU64>,
    /// Sumeragi pacemaker: evaluation duration in the tick loop (ms)
    pub sumeragi_pacemaker_eval_ms: Histogram,
    /// Sumeragi pacemaker: proposal attempt duration in the tick loop (ms)
    pub sumeragi_pacemaker_propose_ms: Histogram,
    /// Sumeragi commit pipeline stage durations (ms) labeled by stage.
    pub sumeragi_commit_stage_ms: HistogramVec,
    /// State commit: view_lock wait duration (ms) during block commit.
    pub state_commit_view_lock_wait_ms: Histogram,
    /// State commit: view_lock hold duration (ms) during block commit.
    pub state_commit_view_lock_hold_ms: Histogram,
    /// Sumeragi pacemaker: commit pipeline executions triggered by timer tick (cumulative, labeled by mode/outcome)
    pub sumeragi_commit_pipeline_tick_total: IntCounterVec,
    /// Sumeragi pacemaker: prevote-quorum timeouts (cumulative, labeled by mode)
    pub sumeragi_prevote_timeout_total: IntCounterVec,
    /// Sumeragi RBC: total missing chunks across active sessions (gauge)
    pub sumeragi_rbc_backlog_chunks_total: GenericGauge<AtomicU64>,
    /// Sumeragi RBC: maximum missing chunks in a single session (gauge)
    pub sumeragi_rbc_backlog_chunks_max: GenericGauge<AtomicU64>,
    /// Sumeragi RBC: sessions pending delivery (gauge)
    pub sumeragi_rbc_backlog_sessions_pending: GenericGauge<AtomicU64>,
    /// Sumeragi RBC: pending sessions awaiting INIT (gauge)
    pub sumeragi_rbc_pending_sessions: GenericGauge<AtomicU64>,
    /// Sumeragi RBC: pending chunk frames buffered before INIT (gauge)
    pub sumeragi_rbc_pending_chunks: GenericGauge<AtomicU64>,
    /// Sumeragi RBC: pending chunk/aux bytes buffered before INIT (gauge)
    pub sumeragi_rbc_pending_bytes: GenericGauge<AtomicU64>,
    /// Sumeragi RBC: pending-frame drops by reason (cap/session_cap/ttl) (counter)
    pub sumeragi_rbc_pending_drops_total: IntCounterVec,
    /// Sumeragi RBC: pending-byte drops by reason (counter)
    pub sumeragi_rbc_pending_dropped_bytes_total: IntCounterVec,
    /// Sumeragi RBC: pending sessions evicted due to TTL or stash limits (counter)
    pub sumeragi_rbc_pending_evicted_total: IntCounter,
    /// Sumeragi: membership mismatches detected (labeled by peer, height, view)
    pub sumeragi_membership_mismatch_total: IntCounterVec,
    /// Sumeragi: peers currently flagged for membership mismatch (0/1 gauge)
    pub sumeragi_membership_mismatch_active: GenericGaugeVec<AtomicU64>,
    /// Sumeragi: post attempts to peers (cumulative), labeled by peer id
    pub sumeragi_post_to_peer_total: IntCounterVec,
    /// Sumeragi: background-post enqueued tasks (cumulative), labeled by kind {Post,Broadcast}
    pub sumeragi_bg_post_enqueued_total: IntCounterVec,
    /// Sumeragi: background-post queue full events (cumulative), labeled by kind
    pub sumeragi_bg_post_overflow_total: IntCounterVec,
    /// Sumeragi: background-post drops when the worker queue is unavailable (cumulative), labeled by kind
    pub sumeragi_bg_post_drop_total: IntCounterVec,
    /// Sumeragi: background-post queue depth (approximate, global)
    pub sumeragi_bg_post_queue_depth: GenericGauge<AtomicU64>,
    /// Sumeragi: background-post queue depth by peer (collector), labeled by peer id
    pub sumeragi_bg_post_queue_depth_by_peer: GenericGaugeVec<AtomicU64>,
    /// Sumeragi: background-post age histogram (milliseconds) labeled by kind {Post,Broadcast}
    pub sumeragi_bg_post_age_ms: HistogramVec,
    /// Sumeragi: pacemaker current backoff window (ms)
    pub sumeragi_pacemaker_backoff_ms: GenericGauge<AtomicU64>,
    /// Sumeragi: pacemaker RTT floor (ms)
    pub sumeragi_pacemaker_rtt_floor_ms: GenericGauge<AtomicU64>,
    /// Sumeragi: pacemaker backoff multiplier (gauge)
    pub sumeragi_pacemaker_backoff_multiplier: GenericGauge<AtomicU64>,
    /// Sumeragi: pacemaker RTT floor multiplier (gauge)
    pub sumeragi_pacemaker_rtt_floor_multiplier: GenericGauge<AtomicU64>,
    /// Sumeragi: pacemaker maximum backoff cap (ms)
    pub sumeragi_pacemaker_max_backoff_ms: GenericGauge<AtomicU64>,
    /// Sumeragi: pacemaker jitter band applied to window (ms, signed magnitude)
    pub sumeragi_pacemaker_jitter_ms: GenericGauge<AtomicU64>,
    /// Sumeragi: pacemaker jitter config as permille of window (0..=1000)
    pub sumeragi_pacemaker_jitter_frac_permille: GenericGauge<AtomicU64>,
    /// Sumeragi: elapsed time in the current round (ms)
    pub sumeragi_pacemaker_round_elapsed_ms: GenericGauge<AtomicU64>,
    /// Sumeragi: current view timeout target window (ms)
    pub sumeragi_pacemaker_view_timeout_target_ms: GenericGauge<AtomicU64>,
    /// Sumeragi: remaining time until current view timeout (ms)
    pub sumeragi_pacemaker_view_timeout_remaining_ms: GenericGauge<AtomicU64>,
    /// Sumeragi: per-phase latency histogram (ms), labeled by `phase` (propose|collect|commit)
    pub sumeragi_phase_latency_ms: HistogramVec,
    /// Sumeragi: per-phase latency EMA (ms), labeled by `phase`
    pub sumeragi_phase_latency_ema_ms: GenericGaugeVec<AtomicU64>,
    /// Sumeragi: aggregate pipeline EMA latency (ms) across pacemaker-controlled phases.
    pub sumeragi_phase_total_ema_ms: GenericGauge<AtomicU64>,
    /// Number of p2p dropped post messages (bounded mode)
    pub p2p_dropped_posts: GenericGauge<AtomicU64>,
    /// Number of p2p dropped broadcast messages (bounded mode)
    pub p2p_dropped_broadcasts: GenericGauge<AtomicU64>,
    /// Number of inbound messages dropped because subscriber queues were full.
    pub p2p_subscriber_queue_full_total: GenericGauge<AtomicU64>,
    /// Per-topic inbound drops caused by subscriber queues being full.
    pub p2p_subscriber_queue_full_by_topic_total: GenericGaugeVec<AtomicU64>,
    /// Number of inbound messages dropped because no subscriber matches the topic.
    pub p2p_subscriber_unrouted_total: GenericGauge<AtomicU64>,
    /// Per-topic inbound drops caused by no subscriber matches.
    pub p2p_subscriber_unrouted_by_topic_total: GenericGaugeVec<AtomicU64>,
    /// Number of p2p handshake failures
    pub p2p_handshake_failures: GenericGauge<AtomicU64>,
    /// Number of low-priority post messages throttled
    pub p2p_low_post_throttled_total: GenericGauge<AtomicU64>,
    /// Number of low-priority broadcast deliveries throttled
    pub p2p_low_broadcast_throttled_total: GenericGauge<AtomicU64>,
    /// Number of per-peer post channel overflows (bounded per-topic channels)
    pub p2p_post_overflow_total: GenericGauge<AtomicU64>,
    /// Per-topic breakdown for post channel overflows
    pub p2p_post_overflow_by_topic: GenericGaugeVec<AtomicU64>,
    /// Consensus ingress drops grouped by topic and reason.
    pub consensus_ingress_drop_total: IntCounterVec,
    /// Number of DNS interval-based refresh cycles performed.
    pub p2p_dns_refresh_total: GenericGauge<AtomicU64>,
    /// Number of DNS TTL-based refresh cycles performed.
    pub p2p_dns_ttl_refresh_total: GenericGauge<AtomicU64>,
    /// Number of DNS resolution/connection failures for hostname peers.
    pub p2p_dns_resolution_fail_total: GenericGauge<AtomicU64>,
    /// Number of DNS reconnect successes after refresh cycles.
    pub p2p_dns_reconnect_success_total: GenericGauge<AtomicU64>,
    /// Number of scheduled per-address connect backoffs
    pub p2p_backoff_scheduled_total: GenericGauge<AtomicU64>,
    /// Number of deferred outbound frames enqueued while peer sessions were unavailable.
    pub p2p_deferred_send_enqueued_total: GenericGauge<AtomicU64>,
    /// Number of deferred outbound frames dropped (expiry, overflow, stale generation).
    pub p2p_deferred_send_dropped_total: GenericGauge<AtomicU64>,
    /// Number of reconnect attempts triggered while deferring outbound frames.
    pub p2p_session_reconnect_total: GenericGauge<AtomicU64>,
    /// Cumulative reconnect retry delay (seconds, rounded up from milliseconds).
    pub p2p_connect_retry_seconds: GenericGauge<AtomicU64>,
    /// Number of incoming connections rejected by per-IP throttle
    pub p2p_accept_throttled_total: GenericGauge<AtomicU64>,
    /// Number of accept throttle bucket evictions (idle/capacity).
    pub p2p_accept_bucket_evictions_total: GenericGauge<AtomicU64>,
    /// Current number of active accept throttle buckets.
    pub p2p_accept_buckets_current: GenericGauge<AtomicU64>,
    /// Prefix cache hits/misses for accept throttle (label `result`).
    pub p2p_accept_prefix_cache_total: GenericGaugeVec<AtomicU64>,
    /// Accept throttle decisions (label `scope` = prefix|ip, `decision` = allowed|throttled).
    pub p2p_accept_throttle_decisions_total: GenericGaugeVec<AtomicU64>,
    /// Number of incoming connections rejected due to incoming cap
    pub p2p_incoming_cap_reject_total: GenericGauge<AtomicU64>,
    /// Number of incoming connections rejected due to total cap
    pub p2p_total_cap_reject_total: GenericGauge<AtomicU64>,
    /// Trust score per peer (label `peer_id`).
    pub p2p_trust_score: IntGaugeVec,
    /// Trust penalties applied (label `reason`).
    pub p2p_trust_penalties_total: IntCounterVec,
    /// Trust decay ticks applied (label `peer_id`).
    pub p2p_trust_decay_ticks_total: IntCounterVec,
    /// Trust gossip frames skipped grouped by direction and reason.
    pub p2p_trust_gossip_skipped_total: IntCounterVec,
    /// Transaction gossip batches sent (labels: plane, dataspace).
    pub tx_gossip_sent_total: IntCounterVec,
    /// Transaction gossip batches dropped (labels: plane, dataspace, reason).
    pub tx_gossip_dropped_total: IntCounterVec,
    /// Latest transaction gossip target count (labels: plane, dataspace).
    pub tx_gossip_targets: GenericGaugeVec<AtomicU64>,
    /// Fallback attempts for restricted gossip (labels: plane, dataspace, surface).
    pub tx_gossip_fallback_total: IntCounterVec,
    /// Configured frame cap for transaction gossip (bytes).
    pub tx_gossip_frame_cap_bytes: GenericGauge<AtomicU64>,
    /// Configured cap for public gossip targets (0 = broadcast).
    pub tx_gossip_public_target_cap: GenericGauge<AtomicU64>,
    /// Configured cap for restricted gossip targets (0 = commit topology).
    pub tx_gossip_restricted_target_cap: GenericGauge<AtomicU64>,
    /// Public-plane target reshuffle interval in milliseconds.
    pub tx_gossip_public_target_reshuffle_ms: GenericGauge<AtomicU64>,
    /// Restricted-plane target reshuffle interval in milliseconds.
    pub tx_gossip_restricted_target_reshuffle_ms: GenericGauge<AtomicU64>,
    /// Whether unknown dataspaces are dropped (1) or routed via the restricted plane (0).
    pub tx_gossip_drop_unknown_dataspace: GenericGauge<AtomicU64>,
    /// Restricted gossip fallback policy (0 = drop, 1 = public overlay).
    pub tx_gossip_restricted_fallback: GenericGauge<AtomicU64>,
    /// Configured policy for restricted payloads when only the public overlay is available (0 = refuse, 1 = forward).
    pub tx_gossip_restricted_public_policy: GenericGauge<AtomicU64>,
    /// Cached status snapshot for the latest gossip target selections.
    pub tx_gossip_status: Arc<RwLock<Vec<TxGossipStatus>>>,
    /// Cached configured caps for status exports.
    pub tx_gossip_caps: Arc<RwLock<TxGossipCaps>>,
    /// Accepted inbound WebSocket P2P connections
    pub p2p_ws_inbound_total: GenericGauge<AtomicU64>,
    /// Successful outbound WebSocket P2P connections
    pub p2p_ws_outbound_total: GenericGauge<AtomicU64>,
    /// Accepted inbound SCION P2P connections
    pub p2p_scion_inbound_total: GenericGauge<AtomicU64>,
    /// Successful outbound SCION P2P connections
    pub p2p_scion_outbound_total: GenericGauge<AtomicU64>,
    /// Network message queue depth by priority (High/Low).
    pub p2p_queue_depth: GenericGaugeVec<AtomicU64>,
    /// Bounded network message queue drops split by priority and kind
    pub p2p_queue_dropped_total: GenericGaugeVec<AtomicU64>,
    /// Handshake latency histogram emulation (buckets by `le` in ms)
    pub p2p_handshake_ms_bucket: GenericGaugeVec<AtomicU64>,
    /// Sum of observed handshake latencies in milliseconds
    pub p2p_handshake_ms_sum: GenericGauge<AtomicU64>,
    /// Count of observed handshakes
    pub p2p_handshake_ms_count: GenericGauge<AtomicU64>,
    /// Handshake error taxonomy
    pub p2p_handshake_error_total: GenericGaugeVec<AtomicU64>,
    /// Topic frame cap violations
    pub p2p_frame_cap_violations_total: GenericGaugeVec<AtomicU64>,
    /// Runtime: upgrade lifecycle events (labeled by kind: proposed|activated|canceled)
    pub runtime_upgrade_events_total: IntCounterVec,
    /// Runtime: provenance rejection events (labeled by reason)
    pub runtime_upgrade_provenance_rejections_total: IntCounterVec,
    /// Runtime: ABI version accepted by this node.
    pub runtime_abi_version: GenericGauge<AtomicU64>,
    /// IVM opcode pre-decode cache hits (cumulative)
    pub ivm_cache_hits: GenericGauge<AtomicU64>,
    /// IVM opcode pre-decode cache misses (cumulative)
    pub ivm_cache_misses: GenericGauge<AtomicU64>,
    /// IVM opcode pre-decode cache evictions (cumulative)
    pub ivm_cache_evictions: GenericGauge<AtomicU64>,
    /// IVM opcode pre-decode decoded streams (cumulative)
    pub ivm_cache_decoded_streams: GenericGauge<AtomicU64>,
    /// IVM opcode pre-decode decoded operations (cumulative)
    pub ivm_cache_decoded_ops_total: GenericGauge<AtomicU64>,
    /// IVM opcode pre-decode decode failures (cumulative)
    pub ivm_cache_decode_failures: GenericGauge<AtomicU64>,
    /// IVM opcode pre-decode total decode time in nanoseconds (cumulative)
    pub ivm_cache_decode_time_ns_total: GenericGauge<AtomicU64>,
    /// IVM: histogram of highest general-purpose register index touched per execution.
    pub ivm_register_max_index: Histogram,
    /// IVM: histogram of unique general-purpose registers touched per execution.
    pub ivm_register_unique_count: Histogram,
    /// Merkle root computations using GPU acceleration (cumulative)
    pub merkle_root_gpu_total: IntCounter,
    /// Merkle root computations using CPU (cumulative)
    pub merkle_root_cpu_total: IntCounter,
    /// Number of DAG vertices (transactions) in the latest validated block
    pub pipeline_dag_vertices: GenericGauge<AtomicU64>,
    /// Number of DAG edges (conflicts) in the latest validated block
    pub pipeline_dag_edges: GenericGauge<AtomicU64>,
    /// Conflict rate of DAG edges in basis points for the latest validated block
    pub pipeline_conflict_rate_bps: GenericGauge<AtomicU64>,
    /// Cumulative access-set source counts used by the scheduler (labels: source)
    pub pipeline_access_set_source_total: IntCounterVec,
    /// Number of independent components (DSF partitions) in the latest validated block
    pub pipeline_comp_count: GenericGauge<AtomicU64>,
    /// Size of the largest independent component in the latest validated block
    pub pipeline_comp_max: GenericGauge<AtomicU64>,
    /// Component-size histogram buckets labeled by `le` (component count per bucket)
    pub pipeline_comp_hist_bucket: GenericGaugeVec<AtomicU64>,
    /// Peak layer width (max transactions in any layer) for the latest validated block
    pub pipeline_peak_layer_width: GenericGauge<AtomicU64>,
    /// Average layer width (rounded) for the latest validated block
    pub pipeline_layer_avg_width: GenericGauge<AtomicU64>,
    /// Median layer width for the latest validated block
    pub pipeline_layer_median_width: GenericGauge<AtomicU64>,
    /// Nexus: cumulative count of config diffs applied per knob/profile.
    pub nexus_config_diff_total: IntCounterVec,
    /// Number of Nexus lane catalog entries configured on this node.
    pub nexus_lane_configured_total: GenericGauge<AtomicU64>,
    /// Placeholder lane identifier recorded during single-lane operation
    pub nexus_lane_id_placeholder: GenericGauge<AtomicU64>,
    /// Placeholder data-space identifier recorded during single-lane operation
    pub nexus_dataspace_id_placeholder: GenericGauge<AtomicU64>,
    /// Nexus: per-lane governance seal status (1 = sealed, 0 = ready).
    pub nexus_lane_governance_sealed: GenericGaugeVec<AtomicU64>,
    /// Nexus: total number of lanes still sealed (missing manifest).
    pub nexus_lane_governance_sealed_total: GenericGauge<AtomicU64>,
    /// Nexus: aliases of lanes still sealed (for status snapshots).
    pub nexus_lane_governance_sealed_aliases: Arc<RwLock<Vec<String>>>,
    /// Nexus: lifecycle plan applications grouped by outcome.
    pub nexus_lane_lifecycle_applied_total: IntCounterVec,
    /// Nexus: latest block height observed per lane.
    pub nexus_lane_block_height: GenericGaugeVec<AtomicU64>,
    /// Nexus: finality lag in slots per lane (head height − lane height).
    pub nexus_lane_finality_lag_slots: GenericGaugeVec<AtomicU64>,
    /// Nexus: settlement backlog (XOR) per lane/dataspace pair.
    pub nexus_lane_settlement_backlog_xor: GaugeVec,
    /// Nexus scheduler: configured TEU capacity for the current slot per lane.
    pub nexus_scheduler_lane_teu_capacity: GenericGaugeVec<AtomicU64>,
    /// Nexus scheduler: TEU committed in the current slot per lane.
    pub nexus_scheduler_lane_teu_slot_committed: GenericGaugeVec<AtomicU64>,
    /// Nexus scheduler: active circuit-breaker trigger level (0 = normal) per lane.
    pub nexus_scheduler_lane_trigger_level: GenericGaugeVec<AtomicU64>,
    /// Nexus scheduler: starvation bound in slots per lane.
    pub nexus_scheduler_starvation_bound_slots: GenericGaugeVec<AtomicU64>,
    /// Nexus scheduler: committed TEU bucket breakdown per lane (floor/headroom/etc.).
    pub nexus_scheduler_lane_teu_slot_breakdown: GenericGaugeVec<AtomicU64>,
    /// Nexus scheduler: cumulative TEU deferrals by reason per lane.
    pub nexus_scheduler_lane_teu_deferral_total: IntCounterVec,
    /// Nexus scheduler: structured headroom telemetry events per lane.
    pub nexus_scheduler_lane_headroom_events_total: IntCounterVec,
    /// Nexus scheduler: cumulative must-serve truncations per lane.
    pub nexus_scheduler_must_serve_truncations_total: IntCounterVec,
    /// Nexus scheduler: per-lane TEU snapshots exposed via `/status`.
    pub nexus_scheduler_lane_teu_status: Arc<RwLock<BTreeMap<u32, NexusLaneTeuStatus>>>,
    /// Nexus scheduler: TEU backlog per dataspace (labeled by lane).
    pub nexus_scheduler_dataspace_teu_backlog: GenericGaugeVec<AtomicU64>,
    /// Nexus scheduler: dataspace age (slots since service) labeled by lane.
    pub nexus_scheduler_dataspace_age_slots: GenericGaugeVec<AtomicU64>,
    /// Nexus scheduler: dataspace SFQ virtual finish tag labeled by lane.
    pub nexus_scheduler_dataspace_virtual_finish: GenericGaugeVec<AtomicU64>,
    /// Nexus scheduler: per-dataspace TEU snapshots exposed via `/status`.
    pub nexus_scheduler_dataspace_teu_status:
        Arc<RwLock<BTreeMap<(u32, u64), NexusDataspaceTeuStatus>>>,
    /// Nexus public-lane validator counts grouped by lifecycle status (pending, active, jailed, exiting, exited, slashed).
    pub nexus_public_lane_validator_total: IntGaugeVec,
    /// Nexus public-lane validator activations grouped by lane.
    pub nexus_public_lane_validator_activation_total: IntCounterVec,
    /// Nexus public-lane validator registration rejects grouped by reason.
    pub nexus_public_lane_validator_reject_total: IntCounterVec,
    /// Nexus public-lane bonded stake per lane (Numeric rendered as float).
    pub nexus_public_lane_stake_bonded: GaugeVec,
    /// Nexus public-lane pending-unbond amount per lane.
    pub nexus_public_lane_unbond_pending: GaugeVec,
    /// Nexus public-lane cumulative rewards recorded per lane.
    pub nexus_public_lane_reward_total: GaugeVec,
    /// Nexus public-lane slash event counter per lane.
    pub nexus_public_lane_slash_total: IntCounterVec,
    /// Number of scheduler layers in the latest validated block
    pub pipeline_layer_count: GenericGauge<AtomicU64>,
    /// Average parallelism utilization in percent (0..100) for the latest validated block
    pub pipeline_scheduler_utilization_pct: GenericGauge<AtomicU64>,
    /// Layer-width histogram buckets labeled by `le` (layer count per bucket)
    pub pipeline_layer_width_hist_bucket: GenericGaugeVec<AtomicU64>,
    /// Number of per-transaction overlays built in the latest validated block
    pub pipeline_overlay_count: GenericGauge<AtomicU64>,
    /// Total number of instructions across overlays in the latest validated block
    pub pipeline_overlay_instructions: GenericGauge<AtomicU64>,
    /// Total Norito-encoded bytes across overlays in the latest validated block
    pub pipeline_overlay_bytes: GenericGauge<AtomicU64>,
    /// Number of transactions classified into the quarantine lane in the latest validated block
    pub pipeline_quarantine_classified: GenericGauge<AtomicU64>,
    /// Number of transactions rejected due to quarantine overflow in the latest validated block
    pub pipeline_quarantine_overflow: GenericGauge<AtomicU64>,
    /// Number of transactions executed in the quarantine lane in the latest validated block
    pub pipeline_quarantine_executed: GenericGauge<AtomicU64>,
    /// Detached pipeline: number of txs prepared for detached execution in latest validated block
    pub pipeline_detached_prepared: GenericGauge<AtomicU64>,
    /// Detached pipeline: number of txs whose detached delta merged successfully
    pub pipeline_detached_merged: GenericGauge<AtomicU64>,
    /// Detached pipeline: number of txs that fell back to direct apply
    pub pipeline_detached_fallback: GenericGauge<AtomicU64>,
    /// BLS signature micro-batches verified via aggregate (same-message) in latest block
    pub pipeline_sig_bls_agg_same: GenericGauge<AtomicU64>,
    /// BLS signature micro-batches verified via aggregate (multi-message) in latest block
    pub pipeline_sig_bls_agg_multi: GenericGauge<AtomicU64>,
    /// BLS signature micro-batches verified via deterministic per-signature path in latest block
    pub pipeline_sig_bls_deterministic: GenericGauge<AtomicU64>,
    /// Cumulative same-message BLS aggregate verification attempts labeled by lane and result.
    pub pipeline_sig_bls_agg_same_total: IntCounterVec,
    /// Cumulative multi-message BLS aggregate verification attempts labeled by lane and result.
    pub pipeline_sig_bls_agg_multi_total: IntCounterVec,
    /// Pipeline stage durations (ms) labeled by stage name
    pub pipeline_stage_ms: HistogramVec,
    /// Total gas used by the latest validated block
    pub block_gas_used: GenericGauge<AtomicU64>,
    /// Confidential gas charged to the latest transaction.
    pub confidential_gas_tx_used: GenericGauge<AtomicU64>,
    /// Confidential gas charged in the current block.
    pub confidential_gas_block_used: GenericGauge<AtomicU64>,
    /// Monotonic counter of confidential gas units consumed.
    pub confidential_gas_total: IntCounter,
    /// Total fee units charged in the latest validated block
    pub block_fee_total_units: GenericGauge<AtomicU64>,
    /// Merge ledger: total entries appended (cumulative)
    pub merge_ledger_entries_total: IntCounter,
    /// Merge ledger: latest committed epoch id
    pub merge_ledger_latest_epoch: GenericGauge<AtomicU64>,
    /// Merge ledger: latest global state root hex snapshot
    pub merge_ledger_latest_root_hex: Arc<RwLock<Option<String>>>,
    /// Torii: filter expression depth by endpoint
    pub torii_filter_depth: HistogramVec,
    /// Torii: match count (items) by endpoint
    pub torii_filter_match_count: HistogramVec,
    /// Torii: scan latency (milliseconds) by endpoint
    pub torii_scan_ms: HistogramVec,
    /// Torii: stream row count (number of serialized items) by endpoint
    pub torii_stream_rows: HistogramVec,
    /// Torii: transaction admission latency (seconds) by lane and endpoint
    pub torii_lane_admission_latency_seconds: HistogramVec,
    /// Torii: attachment rejects grouped by reason.
    pub torii_attachment_reject_total: IntCounterVec,
    /// Torii: attachment sanitization latency (milliseconds).
    pub torii_attachment_sanitize_ms: HistogramVec,
    /// Torii: background prover attachment size distribution by status/content type
    pub torii_zk_prover_attachment_bytes: HistogramVec,
    /// Torii: background prover processing latency by status
    pub torii_zk_prover_latency_ms: HistogramVec,
    /// Torii: background prover garbage-collection counter
    pub torii_zk_prover_gc_total: IntCounter,
    /// Torii: background prover in-flight attachment gauge
    pub torii_zk_prover_inflight: GenericGauge<AtomicU64>,
    /// Torii: background prover pending attachment gauge
    pub torii_zk_prover_pending: GenericGauge<AtomicU64>,
    /// Torii: IVM prove helper in-flight job gauge
    pub torii_zk_ivm_prove_inflight: GenericGauge<AtomicU64>,
    /// Torii: IVM prove helper queued job gauge
    pub torii_zk_ivm_prove_queued: GenericGauge<AtomicU64>,
    /// Torii: background prover last-scan processed bytes gauge
    pub torii_zk_prover_last_scan_bytes: GenericGauge<AtomicU64>,
    /// Torii: background prover last-scan wall-clock duration gauge
    pub torii_zk_prover_last_scan_ms: GenericGauge<AtomicU64>,
    /// Torii: background prover budget exhaustion counter (labeled by reason)
    pub torii_zk_prover_budget_exhausted_total: IntCounterVec,
    /// Torii: snapshot-lane query requests total, labeled by mode (ephemeral|stored)
    pub torii_query_snapshot_requests: IntCounterVec,
    /// Torii: snapshot-lane first-batch latency (ms), labeled by mode
    pub torii_query_snapshot_first_batch_ms: HistogramVec,
    /// Torii: snapshot-lane gas consumed units total, labeled by mode
    pub torii_query_snapshot_gas_consumed_units_total: IntCounterVec,
    /// Snapshot query lane: first-batch latency (ms) by cursor mode
    pub query_snapshot_lane_first_batch_ms: HistogramVec,
    /// Snapshot query lane: first-batch item counts by cursor mode
    pub query_snapshot_lane_first_batch_items: HistogramVec,
    /// Snapshot query lane: remaining items gauge by cursor mode
    pub query_snapshot_lane_remaining_items: GenericGaugeVec<AtomicU64>,
    /// Snapshot query lane: cursors emitted total by cursor mode
    pub query_snapshot_lane_cursors_total: IntCounterVec,
    // Torii Connect (Iroha Connect) metrics
    /// Torii Connect: total WS sessions (gauge)
    pub torii_connect_sessions_total: GenericGauge<AtomicU64>,
    /// Torii Connect: active session objects (gauge)
    pub torii_connect_sessions_active: GenericGauge<AtomicU64>,
    /// Torii pre-auth: rejected connections before authentication, labeled by reason
    pub torii_pre_auth_reject_total: IntCounterVec,
    /// Torii operator auth events (action, result, reason).
    pub torii_operator_auth_total: IntCounterVec,
    /// Torii operator auth lockouts (action, reason).
    pub torii_operator_auth_lockout_total: IntCounterVec,
    /// Torii admission rejects due to exceeding signature-count limits.
    pub torii_signature_limit_total: IntCounter,
    /// Torii admission rejects due to signature-count limits, labeled by authority type.
    pub torii_signature_limit_by_authority_total: IntCounterVec,
    /// Last observed signature count when enforcing Torii signature limits.
    pub torii_signature_limit_last_count: GenericGauge<AtomicU64>,
    /// Configured signature cap recorded during the last signature-limit enforcement.
    pub torii_signature_limit_max: GenericGauge<AtomicU64>,
    /// Torii admission rejects when NTS is unhealthy for time-sensitive transactions.
    pub torii_nts_unhealthy_reject_total: IntCounter,
    /// Torii admission rejects for direct multisig signing attempts.
    pub torii_multisig_direct_sign_reject_total: IntCounter,
    /// Torii SoraFS provider admission counters (result, reason).
    pub torii_sorafs_admission_total: IntCounterVec,
    /// Torii SoraFS capacity telemetry rejections (provider, reason).
    pub torii_sorafs_capacity_telemetry_rejections_total: IntCounterVec,
    /// Torii SoraFS declared capacity gauge (GiB) per provider.
    pub torii_sorafs_capacity_declared_gib: GenericGaugeVec<AtomicU64>,
    /// Torii SoraFS effective capacity gauge (GiB) per provider.
    pub torii_sorafs_capacity_effective_gib: GenericGaugeVec<AtomicU64>,
    /// Torii SoraFS utilised capacity gauge (GiB) per provider.
    pub torii_sorafs_capacity_utilised_gib: GenericGaugeVec<AtomicU64>,
    /// Torii SoraFS outstanding capacity gauge (GiB) per provider.
    pub torii_sorafs_capacity_outstanding_gib: GenericGaugeVec<AtomicU64>,
    /// Torii SoraFS accumulated GiB·hours per provider.
    pub torii_sorafs_capacity_gibhours_total: GaugeVec,
    /// Torii SoraFS fee projection (nano units) per provider.
    pub torii_sorafs_fee_projection_nanos: GaugeVec,
    /// Torii SoraFS dispute submissions labelled by result.
    pub torii_sorafs_disputes_total: IntCounterVec,
    /// Torii SoraFS replication orders issued per provider.
    pub torii_sorafs_orders_issued_total: GenericGaugeVec<AtomicU64>,
    /// Torii SoraFS replication orders completed per provider.
    pub torii_sorafs_orders_completed_total: GenericGaugeVec<AtomicU64>,
    /// Torii SoraFS replication orders failed per provider.
    pub torii_sorafs_orders_failed_total: GenericGaugeVec<AtomicU64>,
    /// Torii SoraFS outstanding order count per provider.
    pub torii_sorafs_outstanding_orders: GenericGaugeVec<AtomicU64>,
    /// Torii SoraFS uptime success (basis points) per provider.
    pub torii_sorafs_uptime_bps: IntGaugeVec,
    /// Torii SoraFS PoR success (basis points) per provider.
    pub torii_sorafs_por_bps: IntGaugeVec,
    /// Torii SoraFS PoR ingestion backlog per manifest/provider pair.
    pub torii_sorafs_por_ingest_backlog: GenericGaugeVec<AtomicU64>,
    /// Torii SoraFS PoR ingestion failures per manifest/provider pair.
    pub torii_sorafs_por_ingest_failures_total: GenericGaugeVec<AtomicU64>,
    /// Torii SoraFS repair task transitions by status.
    pub torii_sorafs_repair_tasks_total: IntCounterVec,
    /// Torii SoraFS repair latency histogram (minutes) grouped by outcome.
    pub torii_sorafs_repair_latency_minutes: HistogramVec,
    /// Torii SoraFS repair queue depth per provider.
    pub torii_sorafs_repair_queue_depth: GenericGaugeVec<AtomicU64>,
    /// Torii SoraFS oldest queued repair age (seconds).
    pub torii_sorafs_repair_backlog_oldest_age_seconds: GenericGauge<AtomicU64>,
    /// Torii SoraFS repair lease expirations grouped by outcome.
    pub torii_sorafs_repair_lease_expired_total: IntCounterVec,
    /// Torii SoraFS slash proposals submitted grouped by outcome.
    pub torii_sorafs_slash_proposals_total: IntCounterVec,
    /// Torii SoraFS reconciliation runs grouped by result.
    pub torii_sorafs_reconciliation_runs_total: IntCounterVec,
    /// Torii SoraFS reconciliation divergence count from the latest snapshot.
    pub torii_sorafs_reconciliation_divergence_count: GenericGauge<AtomicU64>,
    /// Torii SoraFS GC runs grouped by result.
    pub torii_sorafs_gc_runs_total: IntCounterVec,
    /// Torii SoraFS GC evictions grouped by reason.
    pub torii_sorafs_gc_evictions_total: IntCounterVec,
    /// Torii SoraFS GC bytes freed grouped by reason.
    pub torii_sorafs_gc_bytes_freed_total: IntCounterVec,
    /// Torii SoraFS GC blocked evictions grouped by reason.
    pub torii_sorafs_gc_blocked_total: IntCounterVec,
    /// Torii SoraFS expired manifests observed by GC sweeps.
    pub torii_sorafs_gc_expired_manifests: GenericGauge<AtomicU64>,
    /// Torii SoraFS age of the oldest expired manifest (seconds).
    pub torii_sorafs_gc_oldest_expired_age_seconds: GenericGauge<AtomicU64>,
    /// Torii SoraFS storage bytes used per provider.
    pub torii_sorafs_storage_bytes_used: GenericGaugeVec<AtomicU64>,
    /// Torii SoraFS storage capacity bytes per provider.
    pub torii_sorafs_storage_bytes_capacity: GenericGaugeVec<AtomicU64>,
    /// Torii SoraFS pin queue depth per provider.
    pub torii_sorafs_storage_pin_queue_depth: GenericGaugeVec<AtomicU64>,
    /// Torii SoraFS fetch workers in flight per provider.
    pub torii_sorafs_storage_fetch_inflight: GenericGaugeVec<AtomicU64>,
    /// Torii SoraFS fetch throughput (bytes/sec) per provider.
    pub torii_sorafs_storage_fetch_bytes_per_sec: GenericGaugeVec<AtomicU64>,
    /// Torii SoraFS PoR workers in flight per provider.
    pub torii_sorafs_storage_por_inflight: GenericGaugeVec<AtomicU64>,
    /// Torii SoraFS PoR samples marked successful per provider.
    pub torii_sorafs_storage_por_samples_success_total: GenericGaugeVec<AtomicU64>,
    /// Torii SoraFS PoR samples marked failed per provider.
    pub torii_sorafs_storage_por_samples_failed_total: GenericGaugeVec<AtomicU64>,
    /// Torii SoraFS chunk-range request counters (endpoint, result).
    pub torii_sorafs_chunk_range_requests_total: IntCounterVec,
    /// Torii SoraFS chunk-range bytes served per endpoint.
    pub torii_sorafs_chunk_range_bytes_total: IntCounterVec,
    /// Count of providers advertising SoraFS range fetch capability grouped by feature.
    pub torii_sorafs_provider_range_capability_total: IntGaugeVec,
    /// SoraFS range fetch throttle events grouped by reason.
    pub torii_sorafs_range_fetch_throttle_events_total: IntCounterVec,
    /// Active SoraFS range fetch streams guarded by tokens (node-wide).
    pub torii_sorafs_range_fetch_concurrency_current: IntGauge,
    /// Torii SoraFS proof streams currently active (per proof kind).
    pub torii_sorafs_proof_stream_inflight: IntGaugeVec,
    /// Torii SoraFS proof stream outcomes grouped by result and reason.
    pub torii_sorafs_proof_stream_events_total: IntCounterVec,
    /// Torii SoraFS proof stream latency histogram in milliseconds.
    pub torii_sorafs_proof_stream_latency_ms: HistogramVec,
    /// Torii SoraFS proof-health alerts grouped by provider, trigger, and penalty outcome.
    pub torii_sorafs_proof_health_alerts_total: IntCounterVec,
    /// Torii SoraFS proof-health PDP failure counts captured at the last alert per provider.
    pub torii_sorafs_proof_health_pdp_failures: IntGaugeVec,
    /// Torii SoraFS proof-health PoTR breach counts captured at the last alert per provider.
    pub torii_sorafs_proof_health_potr_breaches: IntGaugeVec,
    /// Torii SoraFS proof-health penalty amount (nano-XOR) observed at the last alert per provider.
    pub torii_sorafs_proof_health_penalty_nano: GenericGaugeVec<AtomicU64>,
    /// Torii SoraFS proof-health telemetry window end epoch recorded at the last alert per provider.
    pub torii_sorafs_proof_health_window_end_epoch: GenericGaugeVec<AtomicU64>,
    /// Torii SoraFS proof-health cooldown flag recorded at the last alert per provider.
    pub torii_sorafs_proof_health_cooldown: IntGaugeVec,
    /// GAR policy violations grouped by reason/detail.
    pub torii_sorafs_gar_violations_total: IntCounterVec,
    /// Gateway refusal counters grouped by reason/profile/provider/scope.
    pub torii_sorafs_gateway_refusals_total: IntCounterVec,
    /// Canonical SoraFS gateway fixture metadata (value = release timestamp, labels = version/profile/digest).
    pub torii_sorafs_gateway_fixture_info: IntGaugeVec,
    /// SoraFS pin registry manifest counts grouped by status.
    pub torii_sorafs_registry_manifests_total: GenericGaugeVec<AtomicU64>,
    /// SoraFS manifest alias total (active entries tracked on-chain).
    pub torii_sorafs_registry_aliases_total: GenericGauge<AtomicU64>,
    /// Alias cache evaluation outcomes (fresh/refresh/expired/hard-expired).
    pub torii_sorafs_alias_cache_refresh_total: IntCounterVec,
    /// Observed alias proof age when served (seconds).
    pub torii_sorafs_alias_cache_age_seconds: Histogram,
    /// Seconds remaining until the active gateway TLS certificate expires.
    pub torii_sorafs_tls_cert_expiry_seconds: Gauge,
    /// Gateway TLS renewal attempts grouped by result.
    pub torii_sorafs_tls_renewal_total: IntCounterVec,
    /// Whether ECH is currently enabled for the gateway (0 = disabled, 1 = enabled).
    pub torii_sorafs_tls_ech_enabled: IntGauge,
    /// Gauge exposing the canonical SoraFS gateway fixture version (label = version).
    pub torii_sorafs_gateway_fixture_version: IntGaugeVec,
    /// SoraFS replication order counts grouped by status.
    pub torii_sorafs_registry_orders_total: GenericGaugeVec<AtomicU64>,
    /// SoraFS replication SLA outcomes (met, missed, pending).
    pub torii_sorafs_replication_sla_total: GenericGaugeVec<AtomicU64>,
    /// Outstanding SoraFS replication backlog (pending order count).
    pub torii_sorafs_replication_backlog_total: GenericGauge<AtomicU64>,
    /// Completion latency aggregates for SoraFS replication orders (epochs).
    pub torii_sorafs_replication_completion_latency_epochs: GaugeVec,
    /// Deadline slack aggregates for pending SoraFS replication orders (epochs).
    pub torii_sorafs_replication_deadline_slack_epochs: GaugeVec,
    /// Rejections at the SoraNet privacy ingest endpoints grouped by endpoint/reason.
    pub soranet_privacy_ingest_reject_total: IntCounterVec,
    /// Aggregated SoraNet circuit outcomes keyed by relay mode and bucket start.
    pub soranet_privacy_circuit_events_total: IntCounterVec,
    /// PoW validation failures grouped by relay mode, bucket start, and reason.
    pub soranet_privacy_pow_rejects_total: IntCounterVec,
    /// Count of SoraNet PoW revocation store fallbacks grouped by reason.
    pub soranet_pow_revocation_store_total: IntCounterVec,
    /// Aggregated SoraNet throttling events keyed by relay mode and bucket start.
    pub soranet_privacy_throttles_total: IntCounterVec,
    /// Aggregated verified byte totals emitted per relay mode and bucket start.
    pub soranet_privacy_verified_bytes_total: IntCounterVec,
    /// Average active circuits per bucket.
    pub soranet_privacy_active_circuits_avg: GaugeVec,
    /// Maximum active circuits observed per bucket.
    pub soranet_privacy_active_circuits_max: GaugeVec,
    /// Open privacy buckets still accumulating contributors (per relay mode).
    pub soranet_privacy_open_buckets: GaugeVec,
    /// Pending collector share accumulators grouped by relay mode.
    pub soranet_privacy_pending_collectors: GaugeVec,
    /// Suppressed bucket counts recorded during the latest drain, grouped by reason.
    pub soranet_privacy_snapshot_suppressed: GaugeVec,
    /// Suppressed bucket counts recorded during the latest drain, grouped by mode and reason.
    pub soranet_privacy_snapshot_suppressed_by_mode: GaugeVec,
    /// Buckets drained during the latest collector flush.
    pub soranet_privacy_snapshot_drained: IntGauge,
    /// Ratio of suppressed to drained buckets observed in the latest flush.
    pub soranet_privacy_snapshot_suppression_ratio: Gauge,
    /// Completed privacy buckets evicted due to retention.
    pub soranet_privacy_evicted_buckets_total: IntCounter,
    /// Suppression indicator for buckets that failed the contributor threshold.
    pub soranet_privacy_bucket_suppressed: GaugeVec,
    /// Suppressed bucket counters grouped by relay mode and suppression reason.
    pub soranet_privacy_suppression_total: IntCounterVec,
    /// RTT percentile gauges per bucket and relay mode.
    pub soranet_privacy_rtt_millis: GaugeVec,
    /// Aggregated GAR abuse counters keyed by hashed category.
    pub soranet_privacy_gar_reports_total: IntCounterVec,
    /// UNIX timestamp of the last successful privacy poll.
    pub soranet_privacy_last_poll_unixtime: IntGauge,
    /// Privacy polling failures grouped by provider alias.
    pub soranet_privacy_poll_errors_total: IntCounterVec,
    /// Privacy collector enabled flag (0 = disabled, 1 = active).
    pub soranet_privacy_collector_enabled: IntGauge,
    /// Active multi-source orchestrator fetches per manifest/region.
    pub sorafs_orchestrator_active_fetches: IntGaugeVec,
    /// Multi-source orchestrator fetch duration histogram (milliseconds).
    pub sorafs_orchestrator_fetch_duration_ms: HistogramVec,
    /// Multi-source orchestrator failures grouped by reason.
    pub sorafs_orchestrator_fetch_failures_total: IntCounterVec,
    /// Multi-source orchestrator retries aggregated per provider.
    pub sorafs_orchestrator_retries_total: IntCounterVec,
    /// Multi-source orchestrator provider failures aggregated per provider.
    pub sorafs_orchestrator_provider_failures_total: IntCounterVec,
    /// Multi-source orchestrator per-chunk latency histogram (milliseconds).
    pub sorafs_orchestrator_chunk_latency_ms: HistogramVec,
    /// Multi-source orchestrator byte counter aggregated per manifest/provider.
    pub sorafs_orchestrator_bytes_total: IntCounterVec,
    /// Multi-source orchestrator stall counter (chunks exceeding latency cap).
    pub sorafs_orchestrator_stalls_total: IntCounterVec,
    /// Transport-layer events emitted by the multi-source orchestrator.
    pub sorafs_orchestrator_transport_events_total: IntCounterVec,
    /// SoraFS anonymity policy events grouped by stage/outcome/reason/region.
    pub sorafs_orchestrator_policy_events_total: IntCounterVec,
    /// Distribution of SoraFS PQ-capable relay ratios grouped by stage/region.
    pub sorafs_orchestrator_pq_ratio: HistogramVec,
    /// Distribution of SoraFS PQ-capable candidate ratios grouped by stage/region.
    pub sorafs_orchestrator_pq_candidate_ratio: HistogramVec,
    /// Distribution of PQ policy shortfalls grouped by stage/region.
    pub sorafs_orchestrator_pq_deficit_ratio: HistogramVec,
    /// Distribution of classical relay ratios grouped by stage/region.
    pub sorafs_orchestrator_classical_ratio: HistogramVec,
    /// Distribution of classical relay selections grouped by stage/region.
    pub sorafs_orchestrator_classical_selected: HistogramVec,
    /// Aggregate GiB-month usage derived from DA rent quotes grouped by cluster/storage class.
    pub torii_da_rent_gib_months_total: IntCounterVec,
    /// Aggregate base rent (micro XOR) derived from DA rent quotes.
    pub torii_da_rent_base_micro_total: CounterVec,
    /// Aggregate protocol reserve contributions (micro XOR) derived from DA rent quotes.
    pub torii_da_protocol_reserve_micro_total: CounterVec,
    /// Aggregate provider reward payouts (micro XOR) derived from DA rent quotes.
    pub torii_da_provider_reward_micro_total: CounterVec,
    /// Aggregate PDP bonus payouts (micro XOR) derived from DA rent quotes.
    pub torii_da_pdp_bonus_micro_total: CounterVec,
    /// Aggregate PoTR bonus payouts (micro XOR) derived from DA rent quotes.
    pub torii_da_potr_bonus_micro_total: CounterVec,
    /// DA receipt ingest outcomes grouped by lane/epoch.
    pub torii_da_receipts_total: IntCounterVec,
    /// Highest DA receipt sequence observed per (lane, epoch).
    pub torii_da_receipt_highest_sequence: GenericGaugeVec<AtomicU64>,
    /// DA chunking + erasure coding duration (seconds).
    pub torii_da_chunking_seconds: Histogram,
    /// DA shard cursor events grouped by outcome/lane/shard.
    pub da_shard_cursor_events_total: IntCounterVec,
    /// Latest block height recorded for each shard cursor advance.
    pub da_shard_cursor_height: IntGaugeVec,
    /// Lag in blocks between the validated height and the last shard cursor advance.
    pub da_shard_cursor_lag_blocks: IntGaugeVec,
    /// Taikai ingest latency histogram grouped by cluster/stream.
    pub taikai_ingest_segment_latency_ms: HistogramVec,
    /// Taikai live-edge drift histogram grouped by cluster/stream (absolute value).
    pub taikai_ingest_live_edge_drift_ms: HistogramVec,
    /// Signed live-edge drift gauge grouped by cluster/stream (negative = ahead).
    pub taikai_ingest_live_edge_drift_signed_ms: GaugeVec,
    /// Taikai ingest failures grouped by cluster/stream/reason.
    pub taikai_ingest_errors_total: IntCounterVec,
    /// Taikai routing manifest alias rotations grouped by cluster/event/stream/alias.
    pub taikai_trm_alias_rotations_total: IntCounterVec,
    /// Taikai viewer rebuffer events grouped by cluster/stream.
    pub taikai_viewer_rebuffer_events_total: IntCounterVec,
    /// Taikai viewer playback segments grouped by cluster/stream.
    pub taikai_viewer_playback_segments_total: IntCounterVec,
    /// Taikai viewer CEK fetch duration histogram grouped by cluster/lane.
    pub taikai_viewer_cek_fetch_duration_ms: HistogramVec,
    /// Taikai viewer PQ circuit health gauge grouped by cluster.
    pub taikai_viewer_pq_circuit_health: GaugeVec,
    /// Taikai viewer CEK rotation age in seconds grouped by lane.
    pub taikai_viewer_cek_rotation_seconds_ago: GenericGaugeVec<AtomicU64>,
    /// Taikai viewer alerts firing counter grouped by cluster/alertname.
    pub taikai_viewer_alerts_firing_total: IntCounterVec,
    /// Taikai cache query outcomes grouped by result/tier.
    pub sorafs_taikai_cache_query_total: IntCounterVec,
    /// Taikai cache insert events grouped by tier.
    pub sorafs_taikai_cache_insert_total: IntCounterVec,
    /// Taikai cache eviction counters grouped by tier/reason.
    pub sorafs_taikai_cache_evictions_total: IntCounterVec,
    /// Taikai cache promotion counters grouped by source/target tiers.
    pub sorafs_taikai_cache_promotions_total: IntCounterVec,
    /// Taikai cache byte counters grouped by event/tier.
    pub sorafs_taikai_cache_bytes_total: IntCounterVec,
    /// Taikai QoS denials grouped by class.
    pub sorafs_taikai_qos_denied_total: IntCounterVec,
    /// Taikai queue events grouped by event/class.
    pub sorafs_taikai_queue_events_total: IntCounterVec,
    /// Taikai queue depth grouped by state.
    pub sorafs_taikai_queue_depth: IntGaugeVec,
    /// Taikai shard failovers grouped by preferred/selected shard.
    pub sorafs_taikai_shard_failovers_total: IntCounterVec,
    /// Gauge tracking open shard circuits in the Taikai queue.
    pub sorafs_taikai_shard_circuits_open: IntGaugeVec,
    /// Count of SoraFS anonymity policy brownouts grouped by stage/reason/region.
    pub sorafs_orchestrator_brownouts_total: IntCounterVec,
    /// Configured SoraNet base payout (nano XOR) applied per epoch.
    pub soranet_reward_base_payout_nanos: GenericGauge<AtomicU64>,
    /// SoraNet reward events grouped by relay/result label.
    pub soranet_reward_events_total: IntCounterVec,
    /// Aggregated XOR payouts (nano units) grouped by relay/result.
    pub soranet_reward_payout_nanos_total: IntCounterVec,
    /// Count of SoraNet reward skips grouped by relay/reason.
    pub soranet_reward_skips_total: IntCounterVec,
    /// Aggregated XOR adjustments (nano units) grouped by relay/kind.
    pub soranet_reward_adjustment_nanos_total: IntCounterVec,
    /// Dispute lifecycle counters grouped by action label.
    pub soranet_reward_disputes_total: IntCounterVec,
    /// Torii HTTP requests grouped by content type, method, and status.
    pub torii_http_requests_total: IntCounterVec,
    /// Torii HTTP request latency in seconds grouped by content type and method.
    pub torii_http_request_duration_seconds: HistogramVec,
    /// Torii HTTP response payload size (bytes) grouped by content type, method, and status.
    pub torii_http_response_bytes_total: IntCounterVec,
    /// Torii API version negotiation grouped by outcome and requested version.
    pub torii_api_version_negotiated_total: IntCounterVec,
    /// Content gateway requests grouped by outcome label.
    pub torii_content_requests_total: IntCounterVec,
    /// Content gateway response latency in seconds grouped by outcome.
    pub torii_content_request_duration_seconds: HistogramVec,
    /// Content gateway bytes served grouped by outcome label.
    pub torii_content_response_bytes_total: IntCounterVec,
    /// Proof endpoint requests grouped by endpoint/outcome.
    pub torii_proof_requests_total: IntCounterVec,
    /// Proof endpoint latency in seconds grouped by endpoint/outcome.
    pub torii_proof_request_duration_seconds: HistogramVec,
    /// Proof endpoint bytes served grouped by endpoint/outcome.
    pub torii_proof_response_bytes_total: IntCounterVec,
    /// Proof endpoint cache hits grouped by endpoint.
    pub torii_proof_cache_hits_total: IntCounterVec,
    /// Torii request latency in seconds grouped by connection scheme.
    pub torii_request_duration_seconds: HistogramVec,
    /// Torii request failures grouped by connection scheme and status code.
    pub torii_request_failures_total: IntCounterVec,
    /// Explorer endpoint requests grouped by endpoint and outcome.
    pub torii_explorer_requests_total: IntCounterVec,
    /// Explorer endpoint latency in seconds grouped by endpoint and outcome.
    pub torii_explorer_request_duration_seconds: HistogramVec,
    /// Norito-RPC gate decisions grouped by rollout stage and outcome.
    pub torii_norito_rpc_gate_total: IntCounterVec,
    /// Proof endpoints throttled by rate limiter (labeled by endpoint).
    pub torii_proof_throttled_total: IntCounterVec,
    /// Torii contract endpoints throttled by rate limiter (labeled by endpoint).
    pub torii_contract_throttled_total: IntCounterVec,
    /// Torii contract endpoints returning errors (labeled by endpoint).
    pub torii_contract_errors_total: IntCounterVec,
    /// SNS registrar outcomes grouped by result and suffix.
    pub sns_registrar_status_total: IntCounterVec,
    /// Torii account address rejects grouped by endpoint/reason.
    pub torii_address_invalid_total: IntCounterVec,
    /// Torii account-domain selections grouped by endpoint/domain kind.
    pub torii_address_domain_total: IntCounterVec,
    /// Torii Local-12 collision detections grouped by endpoint/kind.
    pub torii_address_collision_total: IntCounterVec,
    /// Torii Local-12 collision detections grouped by endpoint/domain label.
    pub torii_address_collision_domain_total: IntCounterVec,
    /// Torii account literal selections grouped by endpoint/format.
    pub torii_account_literal_total: IntCounterVec,
    /// Torii Norito RPC decode failures grouped by payload kind/reason.
    pub torii_norito_decode_failures_total: IntCounterVec,
    /// Torii pre-auth: active connections tracked by scheme (http/ws)
    pub torii_active_connections_total: GenericGaugeVec<AtomicU64>,
    /// Torii Connect: sessions with buffered frames (gauge)
    pub torii_connect_buffered_sessions: GenericGauge<AtomicU64>,
    /// Torii Connect: total buffered bytes across sessions (gauge)
    pub torii_connect_total_buffer_bytes: GenericGauge<AtomicU64>,
    /// Torii Connect: dedupe cache size (gauge)
    pub torii_connect_dedupe_size: GenericGauge<AtomicU64>,
    /// Torii Connect: per-IP session counts (gauge vec labeled by ip)
    pub torii_connect_per_ip_sessions: GenericGaugeVec<AtomicU64>,
    /// NTS: network time offset vs local clock (signed, milliseconds)
    pub nts_offset_ms: IntGauge,
    /// NTS: confidence (MAD) in milliseconds
    pub nts_confidence_ms: GenericGauge<AtomicU64>,
    /// NTS: number of peers currently contributing samples
    pub nts_peers_sampled: GenericGauge<AtomicU64>,
    /// NTS: number of samples used in aggregation (post-filter)
    pub nts_samples_used: GenericGauge<AtomicU64>,
    /// NTS: health status (1 = healthy, 0 = unhealthy)
    pub nts_healthy: IntGauge,
    /// NTS: fallback indicator (1 = local time fallback, 0 = NTS offset)
    pub nts_fallback: IntGauge,
    /// NTS: minimum sample threshold check (1 = ok, 0 = fail)
    pub nts_min_samples_ok: IntGauge,
    /// NTS: offset bound check (1 = ok, 0 = fail)
    pub nts_offset_ok: IntGauge,
    /// NTS: confidence bound check (1 = ok, 0 = fail)
    pub nts_confidence_ok: IntGauge,
    /// NTS: RTT histogram buckets labeled by `le` (ms)
    pub nts_rtt_ms_bucket: GenericGaugeVec<AtomicU64>,
    /// NTS: RTT histogram sum of ms
    pub nts_rtt_ms_sum: GenericGauge<AtomicU64>,
    /// NTS: RTT histogram count of observations
    pub nts_rtt_ms_count: GenericGauge<AtomicU64>,
    /// Aggregate verification latency (ms) by event kind.
    pub zk_verify_latency_ms: HistogramVec,
    /// Aggregate verification proof size (bytes) by event kind.
    pub zk_verify_proof_bytes: HistogramVec,
    /// Internal use only. Needed for generating the response.
    registry: Registry,
}

impl Default for Metrics {
    #[allow(
        clippy::too_many_lines,
        clippy::similar_names,
        clippy::inconsistent_struct_constructor
    )]
    fn default() -> Self {
        // Helper: guarded registration (panics on duplicates in debug, infallible in release)
        fn register_guarded<C: Collector + Clone + 'static>(reg: &Registry, metric: &C) {
            if let Err(err) = reg.register(Box::new(metric.clone())) {
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
        macro_rules! register {
            ($reg:expr, $metric:expr)=> {
                register_guarded(&$reg, &$metric);
            };
            ($reg:expr, $metric:expr,$($metrics:expr),+)=>{
                register!($reg, $metric);
                register!($reg, $($metrics),+);
            }
        }
        // NOTE(telemetry): Metric registration below is guarded via `register_guarded`,
        // which panics in debug builds on duplicate/invalid metric names. This catches
        // collisions early during development and keeps release builds lean.
        let txs = IntCounterVec::new(Opts::new("txs", "Transactions committed"), &["type"])
            .expect("Infallible");
        let isi = IntCounterVec::new(
            Opts::new("isi", "Iroha special instructions handled by this peer"),
            &["type", "success_status"],
        )
        .expect("Infallible");
        let isi_times = HistogramVec::new(
            HistogramOpts::new("isi_times", "Time to handle isi in this peer"),
            &["type"],
        )
        .expect("Infallible");
        let tx_amounts = Histogram::with_opts(
            HistogramOpts::new(
                "tx_amount",
                "average amount involved in a transaction on this peer",
            )
            .buckets(
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
            ),
        )
        .expect("Infallible");
        let block_height =
            IntCounter::new("block_height", "Current block height").expect("Infallible");
        let block_height_non_empty = IntCounter::new(
            "block_height_non_empty",
            "Current count of non-empty blocks",
        )
        .expect("Infallible");
        let last_commit_time_ms = GenericGauge::new(
            "last_commit_time_ms",
            "Time (since block creation) it took for the latest block to be committed by this peer",
        )
        .expect("Infallible");
        let commit_time_ms = Histogram::with_opts(
            HistogramOpts::new("commit_time_ms", "Average block commit time on this peer")
                .buckets(prometheus::exponential_buckets(100.0, 4.0, 5).expect("inputs are valid")),
        )
        .expect("Infallible");
        let slot_duration_ms = Histogram::with_opts(
            HistogramOpts::new(
                "iroha_slot_duration_ms",
                "Slot duration distribution in milliseconds (NX-18 finality SLO).",
            )
            .buckets(vec![
                250.0, 500.0, 750.0, 1_000.0, 1_250.0, 1_500.0, 2_000.0, 3_000.0,
            ]),
        )
        .expect("Infallible");
        let slot_duration_ms_latest = GenericGauge::new(
            "iroha_slot_duration_ms_latest",
            "Latest observed slot duration (ms) for NX-18 finality tracking.",
        )
        .expect("Infallible");
        let da_quorum_ratio = Gauge::new(
            "iroha_da_quorum_ratio",
            "Rolling fraction of slots that satisfied the DA quorum (0-1).",
        )
        .expect("Infallible");
        let sm_syscall_total = IntCounterVec::new(
            Opts::new(
                "iroha_sm_syscall_total",
                "SM helper syscalls observed (labels: kind, mode)",
            ),
            &["kind", "mode"],
        )
        .expect("Infallible");
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
        let sm_openssl_preview = GenericGauge::new(
            "iroha_sm_openssl_preview",
            "Whether the OpenSSL-backed SM preview helpers are enabled (0/1).",
        )
        .expect("Infallible");
        let zk_halo2_enabled = GenericGauge::new(
            "iroha_zk_halo2_enabled",
            "Whether Halo2 verification is enabled for the host (0/1).",
        )
        .expect("Infallible");
        let zk_halo2_curve_id = GenericGauge::new(
            "iroha_zk_halo2_curve_id",
            "Active Halo2 curve identifier (0=Pallas, 1=Pasta, 2=Goldilocks, 3=Bn254).",
        )
        .expect("Infallible");
        let zk_halo2_backend_id = GenericGauge::new(
            "iroha_zk_halo2_backend_id",
            "Active Halo2 backend identifier (0=IPA, 1=Unsupported).",
        )
        .expect("Infallible");
        let zk_halo2_max_k = GenericGauge::new(
            "iroha_zk_halo2_max_k",
            "Maximum supported Halo2 circuit exponent (k).",
        )
        .expect("Infallible");
        let zk_halo2_verifier_budget_ms = GenericGauge::new(
            "iroha_zk_halo2_verifier_budget_ms",
            "Halo2 verifier soft budget in milliseconds.",
        )
        .expect("Infallible");
        let zk_halo2_verifier_max_batch = GenericGauge::new(
            "iroha_zk_halo2_verifier_max_batch",
            "Maximum proofs allowed in a Halo2 batch verification.",
        )
        .expect("Infallible");
        let zk_halo2_verifier_worker_threads = GenericGauge::new(
            "iroha_zk_halo2_verifier_worker_threads",
            "Number of worker threads serving ZK lane verification.",
        )
        .expect("Infallible");
        let zk_halo2_verifier_queue_cap = GenericGauge::new(
            "iroha_zk_halo2_verifier_queue_cap",
            "Effective ZK lane queue capacity.",
        )
        .expect("Infallible");
        let zk_lane_enqueue_wait_total = IntCounter::new(
            "iroha_zk_lane_enqueue_wait_total",
            "Count of ZK lane admissions that required a bounded wait.",
        )
        .expect("Infallible");
        let zk_lane_enqueue_timeout_total = IntCounter::new(
            "iroha_zk_lane_enqueue_timeout_total",
            "Count of ZK lane admissions that timed out under saturation.",
        )
        .expect("Infallible");
        let zk_lane_drop_total = IntCounterVec::new(
            Opts::new(
                "iroha_zk_lane_drop_total",
                "Terminal ZK lane drops grouped by reason.",
            ),
            &["reason"],
        )
        .expect("Infallible");
        let zk_lane_retry_enqueued_total = IntCounter::new(
            "iroha_zk_lane_retry_enqueued_total",
            "Count of important tasks enqueued into the ZK lane retry ring.",
        )
        .expect("Infallible");
        let zk_lane_retry_replayed_total = IntCounter::new(
            "iroha_zk_lane_retry_replayed_total",
            "Count of tasks replayed from the ZK lane retry ring.",
        )
        .expect("Infallible");
        let zk_lane_retry_exhausted_total = IntCounter::new(
            "iroha_zk_lane_retry_exhausted_total",
            "Count of tasks dropped after exhausting ZK lane retry attempts.",
        )
        .expect("Infallible");
        let zk_lane_pending_depth = GenericGauge::new(
            "iroha_zk_lane_pending_depth",
            "Current number of tasks buffered in ZK lane dispatch backlog.",
        )
        .expect("Infallible");
        let zk_lane_retry_ring_depth = GenericGauge::new(
            "iroha_zk_lane_retry_ring_depth",
            "Current number of tasks buffered in the ZK lane retry ring.",
        )
        .expect("Infallible");
        let zk_verifier_cache_events_total = IntCounterVec::new(
            Opts::new(
                "iroha_zk_verifier_cache_events_total",
                "Verifier cache events grouped by cache name and event (hit|miss|insert|evict).",
            ),
            &["cache", "event"],
        )
        .expect("Infallible");
        let confidential_gas_base_verify = GenericGauge::new(
            "iroha_confidential_gas_base_verify",
            "Base gas charged when verifying a confidential proof.",
        )
        .expect("Infallible");
        let confidential_gas_per_public_input = GenericGauge::new(
            "iroha_confidential_gas_per_public_input",
            "Gas multiplier per confidential proof public input.",
        )
        .expect("Infallible");
        let confidential_gas_per_proof_byte = GenericGauge::new(
            "iroha_confidential_gas_per_proof_byte",
            "Gas multiplier per confidential proof byte.",
        )
        .expect("Infallible");
        let confidential_gas_per_nullifier = GenericGauge::new(
            "iroha_confidential_gas_per_nullifier",
            "Gas multiplier per confidential nullifier.",
        )
        .expect("Infallible");
        let confidential_gas_per_commitment = GenericGauge::new(
            "iroha_confidential_gas_per_commitment",
            "Gas multiplier per confidential commitment.",
        )
        .expect("Infallible");
        let ivm_gas_schedule_hash_lo = GenericGauge::new(
            "iroha_ivm_gas_schedule_hash_lo",
            "Lower 64 bits of the canonical IVM gas schedule hash.",
        )
        .expect("Infallible");
        let ivm_gas_schedule_hash_hi = GenericGauge::new(
            "iroha_ivm_gas_schedule_hash_hi",
            "Upper 64 bits of the canonical IVM gas schedule hash.",
        )
        .expect("Infallible");
        let confidential_tree_commitments = GenericGaugeVec::new(
            Opts::new(
                "iroha_confidential_tree_commitments",
                "Current number of confidential commitments per asset.",
            ),
            &["asset_id"],
        )
        .expect("Infallible");
        let confidential_tree_depth = GenericGaugeVec::new(
            Opts::new(
                "iroha_confidential_tree_depth",
                "Current Merkle depth for the confidential tree per asset.",
            ),
            &["asset_id"],
        )
        .expect("Infallible");
        let confidential_root_history_entries = GenericGaugeVec::new(
            Opts::new(
                "iroha_confidential_root_history_entries",
                "Root history entries retained for the confidential tree per asset.",
            ),
            &["asset_id"],
        )
        .expect("Infallible");
        let confidential_frontier_checkpoints = GenericGaugeVec::new(
            Opts::new(
                "iroha_confidential_frontier_checkpoints",
                "Frontier checkpoints tracked for the confidential tree per asset.",
            ),
            &["asset_id"],
        )
        .expect("Infallible");
        let confidential_frontier_last_height = GenericGaugeVec::new(
            Opts::new(
                "iroha_confidential_frontier_last_checkpoint_height",
                "Height of the most recent frontier checkpoint per asset.",
            ),
            &["asset_id"],
        )
        .expect("Infallible");
        let confidential_frontier_last_commitments = GenericGaugeVec::new(
            Opts::new(
                "iroha_confidential_frontier_last_checkpoint_commitments",
                "Commitment count captured at the latest frontier checkpoint per asset.",
            ),
            &["asset_id"],
        )
        .expect("Infallible");
        let confidential_root_evictions_total = IntCounterVec::new(
            Opts::new(
                "iroha_confidential_root_evictions_total",
                "Confidential root history eviction counter per asset.",
            ),
            &["asset_id"],
        )
        .expect("Infallible");
        let confidential_frontier_evictions_total = IntCounterVec::new(
            Opts::new(
                "iroha_confidential_frontier_evictions_total",
                "Confidential frontier eviction counter per asset.",
            ),
            &["asset_id"],
        )
        .expect("Infallible");
        let oracle_price_local_per_xor = Gauge::new(
            "iroha_oracle_price_local_per_xor",
            "Latest oracle TWAP quoted as local tokens per XOR.",
        )
        .expect("Infallible");
        let oracle_twap_window_seconds = GenericGauge::new(
            "iroha_oracle_twap_window_seconds",
            "TWAP window length used by the oracle in seconds.",
        )
        .expect("Infallible");
        let oracle_haircut_basis_points = GenericGauge::new(
            "iroha_oracle_haircut_basis_points",
            "Effective haircut applied by the oracle (basis points).",
        )
        .expect("Infallible");
        let oracle_staleness_seconds = Gauge::new(
            "iroha_oracle_staleness_seconds",
            "Oracle staleness in seconds at the time of the last settlement.",
        )
        .expect("Infallible");
        let oracle_observations_total = IntCounterVec::new(
            Opts::new(
                "iroha_oracle_observations_total",
                "Number of oracle observations aggregated per feed slot.",
            ),
            &["feed_id"],
        )
        .expect("Infallible");
        let oracle_aggregation_duration_ms = HistogramVec::new(
            HistogramOpts::new(
                "iroha_oracle_aggregation_duration_ms",
                "Oracle aggregation wall-clock duration in milliseconds.",
            )
            .buckets(vec![1.0, 2.5, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0]),
            &["feed_id"],
        )
        .expect("Infallible");
        let oracle_rewards_total = IntCounterVec::new(
            Opts::new(
                "iroha_oracle_rewards_total",
                "Total mantissa units rewarded to oracle providers per feed.",
            ),
            &["feed_id"],
        )
        .expect("Infallible");
        let oracle_penalties_total = IntCounterVec::new(
            Opts::new(
                "iroha_oracle_penalties_total",
                "Total mantissa units slashed from oracle providers per feed.",
            ),
            &["feed_id"],
        )
        .expect("Infallible");
        let oracle_feed_events_total = IntCounterVec::new(
            Opts::new(
                "iroha_oracle_feed_events_total",
                "Total oracle feed events aggregated per feed.",
            ),
            &["feed_id"],
        )
        .expect("Infallible");
        let oracle_feed_events_with_evidence_total = IntCounterVec::new(
            Opts::new(
                "iroha_oracle_feed_events_with_evidence_total",
                "Oracle feed events that carried at least one evidence hash.",
            ),
            &["feed_id"],
        )
        .expect("Infallible");
        let oracle_evidence_hashes_total = IntCounterVec::new(
            Opts::new(
                "iroha_oracle_evidence_hashes_total",
                "Total oracle evidence hashes attached to feed events per feed.",
            ),
            &["feed_id"],
        )
        .expect("Infallible");

        let fastpq_execution_mode_total = IntCounterVec::new(
            Opts::new(
                "fastpq_execution_mode_total",
                "FASTPQ execution mode resolutions grouped by requested/resolved/backend and per-device labels.",
            ),
            &[
                "requested",
                "resolved",
                "backend",
                "device_class",
                "chip_family",
                "gpu_kind",
            ],
        )
        .expect("Infallible");
        let fastpq_poseidon_pipeline_total = IntCounterVec::new(
            Opts::new(
                "fastpq_poseidon_pipeline_total",
                "FASTPQ Poseidon pipeline resolutions grouped by requested/resolved/path and per-device labels.",
            ),
            &[
                "requested",
                "resolved",
                "path",
                "device_class",
                "chip_family",
                "gpu_kind",
            ],
        )
        .expect("Infallible");
        let fastpq_metal_queue_ratio = GaugeVec::new(
            Opts::new(
                "fastpq_metal_queue_ratio",
                "FASTPQ Metal queue duty-cycle ratios (labels: device_class, chip_family, gpu_kind, queue, metric=busy|overlap).",
            ),
            &["device_class", "chip_family", "gpu_kind", "queue", "metric"],
        )
        .expect("Infallible");
        let fastpq_metal_queue_depth = GaugeVec::new(
            Opts::new(
                "fastpq_metal_queue_depth",
                "FASTPQ Metal queue depth snapshot (labels: device_class, chip_family, gpu_kind, metric=limit|max_in_flight|dispatch_count|window_seconds).",
            ),
            &["device_class", "chip_family", "gpu_kind", "metric"],
        )
        .expect("Infallible");
        let fastpq_zero_fill_duration_ms = GaugeVec::new(
            Opts::new(
                "fastpq_zero_fill_duration_ms",
                "FASTPQ host zero-fill duration samples in milliseconds (labels: device_class, chip_family, gpu_kind).",
            ),
            &["device_class", "chip_family", "gpu_kind"],
        )
        .expect("Infallible");
        let fastpq_zero_fill_bandwidth_gbps = GaugeVec::new(
            Opts::new(
                "fastpq_zero_fill_bandwidth_gbps",
                "FASTPQ host zero-fill bandwidth derived from runtime telemetry (labels: device_class, chip_family, gpu_kind).",
            ),
            &["device_class", "chip_family", "gpu_kind"],
        )
        .expect("Infallible");

        let sm_syscall_failures_total = IntCounterVec::new(
            Opts::new(
                "iroha_sm_syscall_failures_total",
                "SM helper syscall failures (labels: kind, mode, reason)",
            ),
            &["kind", "mode", "reason"],
        )
        .expect("Infallible");
        let settlement_events_total = IntCounterVec::new(
            Opts::new(
                "iroha_settlement_events_total",
                "Settlement events grouped by kind/outcome/reason",
            ),
            &["kind", "outcome", "reason"],
        )
        .expect("Infallible");
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
        let settlement_finality_events_total = IntCounterVec::new(
            Opts::new(
                "iroha_settlement_finality_events_total",
                "Settlement finality outcomes grouped by kind/outcome/final_state",
            ),
            &["kind", "outcome", "final_state"],
        )
        .expect("Infallible");
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
        let settlement_fx_window_ms = HistogramVec::new(
            HistogramOpts::new(
                "iroha_settlement_fx_window_ms",
                "PvP FX window observations (milliseconds between committed legs)",
            ),
            &["kind", "order", "atomicity"],
        )
        .expect("Infallible");
        for kind in ["pvp"] {
            for order in ["delivery_then_payment", "payment_then_delivery"] {
                for atomicity in ["all_or_nothing", "commit_first_leg", "commit_second_leg"] {
                    let _ = settlement_fx_window_ms.with_label_values(&[kind, order, atomicity]);
                }
            }
        }
        let settlement_buffer_xor = GaugeVec::new(
            Opts::new(
                "iroha_settlement_buffer_xor",
                "Per-lane/dataspace settlement buffer debits recorded in micro XOR.",
            ),
            &["lane_id", "dataspace_id"],
        )
        .expect("Infallible");
        let settlement_buffer_capacity_xor = GaugeVec::new(
            Opts::new(
                "iroha_settlement_buffer_capacity_xor",
                "Configured settlement buffer capacity in micro XOR per lane/dataspace.",
            ),
            &["lane_id", "dataspace_id"],
        )
        .expect("Infallible");
        let settlement_buffer_status = GaugeVec::new(
            Opts::new(
                "iroha_settlement_buffer_status",
                "Settlement buffer status code per lane/dataspace (0 = normal, 1 = alert, 2 = throttle, 3 = XOR-only, 4 = halt).",
            ),
            &["lane_id", "dataspace_id"],
        )
        .expect("Infallible");
        let settlement_pnl_xor = GaugeVec::new(
            Opts::new(
                "iroha_settlement_pnl_xor",
                "Per-lane/dataspace realised haircut variance recorded in micro XOR.",
            ),
            &["lane_id", "dataspace_id"],
        )
        .expect("Infallible");
        let settlement_haircut_bp = GaugeVec::new(
            Opts::new(
                "iroha_settlement_haircut_bp",
                "Effective haircut basis points applied per lane/dataspace for the latest block.",
            ),
            &["lane_id", "dataspace_id"],
        )
        .expect("Infallible");
        let settlement_swapline_utilisation = GaugeVec::new(
            Opts::new(
                "iroha_settlement_swapline_utilisation",
                "Swap-line utilisation snapshots (micro XOR) grouped by lane, dataspace, and liquidity profile.",
            ),
            &["lane_id", "dataspace_id", "profile"],
        )
        .expect("Infallible");
        let settlement_conversion_total = IntCounterVec::new(
            Opts::new(
                "settlement_router_conversion_total",
                "Settlement conversion counter grouped by lane, dataspace, and source token.",
            ),
            &["lane_id", "dataspace_id", "source_token"],
        )
        .expect("Infallible");
        let settlement_haircut_total = CounterVec::new(
            Opts::new(
                "settlement_router_haircut_total",
                "Cumulative settlement haircut totals (in XOR units) grouped by lane and dataspace.",
            ),
            &["lane_id", "dataspace_id"],
        )
        .expect("Infallible");
        let subscription_billing_attempts_total = IntCounterVec::new(
            Opts::new(
                "iroha_subscription_billing_attempts_total",
                "Subscription billing attempts grouped by pricing kind.",
            ),
            &["pricing"],
        )
        .expect("Infallible");
        let subscription_billing_outcomes_total = IntCounterVec::new(
            Opts::new(
                "iroha_subscription_billing_outcomes_total",
                "Subscription billing outcomes grouped by pricing kind and result.",
            ),
            &["pricing", "result"],
        )
        .expect("Infallible");
        for pricing in ["fixed", "usage"] {
            let _ = subscription_billing_attempts_total.with_label_values(&[pricing]);
            for result in ["paid", "failed", "suspended", "skipped"] {
                let _ = subscription_billing_outcomes_total.with_label_values(&[pricing, result]);
            }
        }
        let offline_transfer_events_total = IntCounterVec::new(
            Opts::new(
                "iroha_offline_transfer_events_total",
                "Offline transfer lifecycle events grouped by event kind",
            ),
            &["event"],
        )
        .expect("Infallible");
        for event in ["settled", "archived", "pruned"] {
            let _ = offline_transfer_events_total.with_label_values(&[event]);
        }
        let offline_transfer_receipts_total = IntCounterVec::new(
            Opts::new(
                "iroha_offline_transfer_receipts_total",
                "Offline transfer receipt aggregates grouped by event kind",
            ),
            &["event"],
        )
        .expect("Infallible");
        let offline_transfer_pruned_total = IntCounter::new(
            "iroha_offline_transfer_pruned_total",
            "Offline transfer bundles pruned from hot storage",
        )
        .expect("Infallible");
        let offline_attestation_policy_total = IntCounterVec::new(
            Opts::new(
                "iroha_offline_attestation_policy_total",
                "Offline attestation tokens processed grouped by Android integrity policy",
            ),
            &["policy"],
        )
        .expect("Infallible");
        let offline_app_attest_signature_compat_total = IntCounter::new(
            "iroha_offline_app_attest_signature_compat_total",
            "iOS App Attest assertions accepted via SHA256(clientDataHash) compatibility path",
        )
        .expect("Infallible");
        let social_events_total = IntCounterVec::new(
            Opts::new(
                "iroha_social_events_total",
                "Viral incentive lifecycle events grouped by event kind",
            ),
            &["event"],
        )
        .expect("Infallible");
        for event in [
            "reward_paid",
            "escrow_created",
            "escrow_released",
            "escrow_cancelled",
        ] {
            let _ = social_events_total.with_label_values(&[event]);
        }
        let social_budget_spent = Gauge::with_opts(Opts::new(
            "iroha_social_budget_spent",
            "Latest viral reward budget spend for the active day",
        ))
        .expect("Infallible");
        let social_campaign_spent = Gauge::with_opts(Opts::new(
            "iroha_social_campaign_spent",
            "Cumulative viral promotion spend across the full campaign window",
        ))
        .expect("Infallible");
        let social_campaign_cap = Gauge::with_opts(Opts::new(
            "iroha_social_campaign_cap",
            "Configured viral campaign cap (0 = unlimited)",
        ))
        .expect("Infallible");
        let social_campaign_remaining = Gauge::with_opts(Opts::new(
            "iroha_social_campaign_remaining",
            "Remaining viral campaign budget (0 when cap is unlimited)",
        ))
        .expect("Infallible");
        let social_campaign_active = Gauge::with_opts(Opts::new(
            "iroha_social_campaign_active",
            "Whether the viral promotion window is active (1 = active, 0 = inactive)",
        ))
        .expect("Infallible");
        let social_halted = Gauge::with_opts(Opts::new(
            "iroha_social_halted",
            "Whether viral incentives are currently halted (1 = halted, 0 = flowing)",
        ))
        .expect("Infallible");
        let social_rejections_total = IntCounterVec::new(
            Opts::new(
                "iroha_social_rejections_total",
                "Viral incentive rejections grouped by reason",
            ),
            &["reason"],
        )
        .expect("Infallible");
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
        let multisig_direct_sign_reject_total = IntCounter::new(
            "multisig_direct_sign_reject_total",
            "Transactions rejected for direct multisig signatures",
        )
        .expect("Infallible");
        let social_open_escrows = GenericGauge::new(
            "iroha_social_open_escrows",
            "Open viral escrows currently tracked on-ledger",
        )
        .expect("Infallible");
        for label in [
            AndroidIntegrityPolicy::MarkerKey.as_str(),
            AndroidIntegrityPolicy::PlayIntegrity.as_str(),
            AndroidIntegrityPolicy::HmsSafetyDetect.as_str(),
            AndroidIntegrityPolicy::Provisioned.as_str(),
        ] {
            let _ = offline_attestation_policy_total.with_label_values(&[label]);
        }
        let _ = offline_transfer_receipts_total.with_label_values(&["settled"]);
        let offline_transfer_settled_amount = Histogram::with_opts(
            HistogramOpts::new(
                "iroha_offline_transfer_settled_amount",
                "Distribution of offline settlement bundle amounts (asset units)",
            )
            .buckets(
                prometheus::exponential_buckets(0.01, 2.0, 18)
                    .expect("bucket inputs are valid for offline settlement histogram"),
            ),
        )
        .expect("Infallible");
        let offline_transfer_rejections_total = IntCounterVec::new(
            Opts::new(
                "iroha_offline_transfer_rejections_total",
                "Offline transfer validation rejections grouped by platform and reason",
            ),
            &["platform", "reason"],
        )
        .expect("Infallible");
        let connected_peers = GenericGauge::new(
            "connected_peers",
            "Total number of currently connected peers",
        )
        .expect("Infallible");
        let p2p_peer_churn_total = IntCounterVec::new(
            Opts::new(
                "p2p_peer_churn_total",
                "Peer churn events observed by this node",
            ),
            &["event"],
        )
        .expect("Infallible");
        for event in ["connected", "disconnected"] {
            let _ = p2p_peer_churn_total.with_label_values(&[event]);
        }
        let uptime_since_genesis_ms = GenericGauge::new(
            "uptime_since_genesis_ms",
            "Network up-time, from creation of the genesis block",
        )
        .expect("Infallible");
        let domains = GenericGauge::new("domains", "Total number of domains").expect("Infallible");
        let accounts = GenericGaugeVec::new(
            Opts::new("accounts", "User accounts registered at this time"),
            &["domain"],
        )
        .expect("Infallible");
        let view_changes = GenericGauge::new(
            "view_changes",
            "Number of view changes in the current round",
        )
        .expect("Infallible");
        let queue_size = GenericGauge::new(
            "queue_size",
            "Transactions tracked by the queue (queued + in-flight)",
        )
        .expect("Infallible");
        let queue_queued =
            GenericGauge::new("queue_queued", "Transactions still queued for selection")
                .expect("Infallible");
        let queue_inflight =
            GenericGauge::new("queue_inflight", "Transactions in-flight after selection")
                .expect("Infallible");
        let kura_fsync_enabled = GenericGauge::new(
            "kura_fsync_enabled",
            "Kura fsync policy state (0=off, 1=on, 2=batched).",
        )
        .expect("Infallible");
        let kura_fsync_failures_total = IntCounterVec::new(
            Opts::new(
                "kura_fsync_failures_total",
                "Kura fsync failures grouped by target (data/index/hashes).",
            ),
            &["target"],
        )
        .expect("Infallible");
        let kura_fsync_latency_buckets =
            prometheus::exponential_buckets(1.0, 2.0, 12).expect("valid fsync latency buckets");
        let kura_fsync_latency_ms = HistogramVec::new(
            HistogramOpts::new(
                "kura_fsync_latency_ms",
                "Kura fsync latency (milliseconds) grouped by target.",
            )
            .buckets(kura_fsync_latency_buckets.clone()),
            &["target"],
        )
        .expect("Infallible");
        let amx_latency_buckets =
            prometheus::exponential_buckets(1.0, 2.0, 12).expect("valid AMX latency buckets");
        let amx_prepare_ms = HistogramVec::new(
            HistogramOpts::new(
                "iroha_amx_prepare_ms",
                "AMX prepare phase latency in milliseconds grouped by lane id.",
            )
            .buckets(amx_latency_buckets.clone()),
            &["lane"],
        )
        .expect("Infallible");
        let amx_commit_ms = HistogramVec::new(
            HistogramOpts::new(
                "iroha_amx_commit_ms",
                "AMX commit/merge phase latency in milliseconds grouped by lane id.",
            )
            .buckets(amx_latency_buckets.clone()),
            &["lane"],
        )
        .expect("Infallible");
        let amx_abort_total = IntCounterVec::new(
            Opts::new(
                "iroha_amx_abort_total",
                "AMX abort counter grouped by lane id and abort stage.",
            ),
            &["lane", "stage"],
        )
        .expect("Infallible");
        let axt_policy_reject_total = IntCounterVec::new(
            Opts::new(
                "iroha_axt_policy_reject_total",
                "AXT policy validation failures grouped by lane id and reason.",
            ),
            &["lane", "reason"],
        )
        .expect("Infallible");
        let axt_policy_snapshot_version = GenericGauge::new(
            "iroha_axt_policy_snapshot_version",
            "Stable hash-derived version of the active AXT policy snapshot (u64 truncated).",
        )
        .expect("Infallible");
        let axt_policy_snapshot_cache_events_total = IntCounterVec::new(
            Opts::new(
                "iroha_axt_policy_snapshot_cache_events_total",
                "AXT policy snapshot hydration events grouped by cache outcome.",
            ),
            &["event"],
        )
        .expect("Infallible");
        let axt_proof_cache_events_total = IntCounterVec::new(
            Opts::new(
                "iroha_axt_proof_cache_events_total",
                "Dataspace proof cache events grouped by event label.",
            ),
            &["event"],
        )
        .expect("Infallible");
        let axt_proof_cache_state = IntGaugeVec::new(
            Opts::new(
                "iroha_axt_proof_cache_state",
                "Dataspace proof cache state (value=expiry_slot_with_skew) grouped by dsid/status/manifest_root_hex/verified_slot.",
            ),
            &["dsid", "status", "manifest_root_hex", "verified_slot"],
        )
        .expect("Infallible");
        let ivm_exec_ms = HistogramVec::new(
            HistogramOpts::new(
                "iroha_ivm_exec_ms",
                "IVM execution latency in milliseconds grouped by lane id.",
            )
            .buckets(amx_latency_buckets.clone()),
            &["lane"],
        )
        .expect("Infallible");
        let ivm_stack_bytes = GenericGaugeVec::new(
            Opts::new(
                "iroha_ivm_stack_bytes",
                "Requested/applied stack sizes for scheduler/prover/guest (bytes).",
            ),
            &["kind", "state"],
        )
        .expect("Infallible");
        let ivm_stack_clamped = GenericGaugeVec::new(
            Opts::new(
                "iroha_ivm_stack_clamped",
                "Stack clamp flags for scheduler/prover/guest (0 = no clamp, 1 = clamped).",
            ),
            &["kind"],
        )
        .expect("Infallible");
        let ivm_stack_gas_multiplier = GenericGauge::new(
            "iroha_ivm_stack_gas_multiplier",
            "Gas→stack multiplier currently in effect for guest stack limits.",
        )
        .expect("Infallible");
        let ivm_stack_pool_fallback_total = IntCounter::new(
            "iroha_ivm_stack_pool_fallback_total",
            "Number of times a pre-existing global Rayon pool forced a stack-size fallback.",
        )
        .expect("Infallible");
        let ivm_stack_budget_hit_total = IntCounter::new(
            "iroha_ivm_stack_budget_hit_total",
            "VM constructions that hit the guest stack budget clamp.",
        )
        .expect("Infallible");
        let sumeragi_tx_queue_depth = GenericGauge::new(
            "sumeragi_tx_queue_depth",
            "Transactions currently queued as observed by consensus",
        )
        .expect("Infallible");
        let sumeragi_tx_queue_capacity = GenericGauge::new(
            "sumeragi_tx_queue_capacity",
            "Transaction queue capacity observed by consensus",
        )
        .expect("Infallible");
        let sumeragi_tx_queue_saturated = GenericGauge::new(
            "sumeragi_tx_queue_saturated",
            "Transaction queue saturation flag observed by consensus (0 = healthy, 1 = saturated)",
        )
        .expect("Infallible");
        let sumeragi_pending_blocks_total = GenericGauge::new(
            "sumeragi_pending_blocks_total",
            "Total pending blocks tracked by consensus",
        )
        .expect("Infallible");
        let sumeragi_pending_blocks_blocking = GenericGauge::new(
            "sumeragi_pending_blocks_blocking",
            "Pending blocks currently gating proposals/view changes",
        )
        .expect("Infallible");
        let sumeragi_commit_inflight_queue_depth = GenericGauge::new(
            "sumeragi_commit_inflight_queue_depth",
            "Commit inflight queue depth (inflight + queued commit work)",
        )
        .expect("Infallible");
        let missing_block_dwell_buckets = vec![
            50.0, 100.0, 250.0, 500.0, 1_000.0, 2_500.0, 5_000.0, 10_000.0, 20_000.0, 60_000.0,
        ];
        let sumeragi_missing_block_requests = GenericGauge::new(
            "sumeragi_missing_block_requests",
            "Outstanding missing-block requests observed locally",
        )
        .expect("Infallible");
        let sumeragi_missing_block_oldest_ms = GenericGauge::new(
            "sumeragi_missing_block_oldest_ms",
            "Age in milliseconds of the oldest missing-block request",
        )
        .expect("Infallible");
        let sumeragi_missing_block_retry_window_ms = GenericGauge::new(
            "sumeragi_missing_block_retry_window_ms",
            "Retry window for missing-block fetches in milliseconds",
        )
        .expect("Infallible");
        let sumeragi_missing_block_dwell_ms = Histogram::with_opts(
            HistogramOpts::new(
                "sumeragi_missing_block_dwell_ms",
                "Dwell time from first QC arrival until block payload observation (milliseconds)",
            )
            .buckets(missing_block_dwell_buckets),
        )
        .expect("Infallible");
        let sumeragi_epoch_length_blocks = GenericGauge::new(
            "sumeragi_epoch_length_blocks",
            "Epoch length in blocks for NPoS scheduling (0 when not applicable)",
        )
        .expect("Infallible");
        let sumeragi_epoch_commit_deadline_offset = GenericGauge::new(
            "sumeragi_epoch_commit_deadline_offset",
            "Commit window deadline offset from epoch start (blocks)",
        )
        .expect("Infallible");
        let sumeragi_epoch_reveal_deadline_offset = GenericGauge::new(
            "sumeragi_epoch_reveal_deadline_offset",
            "Reveal window deadline offset from epoch start (blocks)",
        )
        .expect("Infallible");
        let state_tiered_hot_entries = GenericGauge::new(
            "state_tiered_hot_entries",
            "Entries retained in the tiered-state hot tier after the latest snapshot",
        )
        .expect("Infallible");
        let state_tiered_hot_bytes = GenericGauge::new(
            "state_tiered_hot_bytes",
            "Bytes retained in the tiered-state hot tier after the latest snapshot",
        )
        .expect("Infallible");
        let state_tiered_cold_entries = GenericGauge::new(
            "state_tiered_cold_entries",
            "Entries spilled to the tiered-state cold tier after the latest snapshot",
        )
        .expect("Infallible");
        let state_tiered_cold_bytes = GenericGauge::new(
            "state_tiered_cold_bytes",
            "Total bytes written to the tiered-state cold tier in the latest snapshot",
        )
        .expect("Infallible");
        let state_tiered_cold_reused_entries = GenericGauge::new(
            "state_tiered_cold_reused_entries",
            "Cold entries reused without re-encoding in the latest tiered-state snapshot",
        )
        .expect("Infallible");
        let state_tiered_cold_reused_bytes = GenericGauge::new(
            "state_tiered_cold_reused_bytes",
            "Total bytes reused from cold payloads in the latest tiered-state snapshot",
        )
        .expect("Infallible");
        let state_tiered_hot_promotions = GenericGauge::new(
            "state_tiered_hot_promotions",
            "Entries promoted into the tiered-state hot tier in the latest snapshot",
        )
        .expect("Infallible");
        let state_tiered_hot_demotions = GenericGauge::new(
            "state_tiered_hot_demotions",
            "Entries demoted into the tiered-state cold tier in the latest snapshot",
        )
        .expect("Infallible");
        let state_tiered_hot_grace_overflow_keys = GenericGauge::new(
            "state_tiered_hot_grace_overflow_keys",
            "Hot-tier key budget overflow caused by grace retention in the latest snapshot",
        )
        .expect("Infallible");
        let state_tiered_hot_grace_overflow_bytes = GenericGauge::new(
            "state_tiered_hot_grace_overflow_bytes",
            "Hot-tier byte budget overflow caused by grace retention in the latest snapshot",
        )
        .expect("Infallible");
        let state_tiered_last_snapshot_index = GenericGauge::new(
            "state_tiered_last_snapshot_index",
            "Latest tiered-state snapshot index recorded by this peer",
        )
        .expect("Infallible");
        let storage_budget_bytes_used = GenericGaugeVec::new(
            Opts::new(
                "storage_budget_bytes_used",
                "Storage budget bytes used per component",
            ),
            &["component"],
        )
        .expect("Infallible");
        let storage_budget_bytes_limit = GenericGaugeVec::new(
            Opts::new(
                "storage_budget_bytes_limit",
                "Storage budget limit bytes per component",
            ),
            &["component"],
        )
        .expect("Infallible");
        let storage_budget_exceeded_total = IntCounterVec::new(
            Opts::new(
                "storage_budget_exceeded_total",
                "Storage budget exceed events per component",
            ),
            &["component"],
        )
        .expect("Infallible");
        let storage_da_cache_total = IntCounterVec::new(
            Opts::new(
                "storage_da_cache_total",
                "DA storage cache outcomes per component",
            ),
            &["component", "result"],
        )
        .expect("Infallible");
        let storage_da_churn_bytes_total = IntCounterVec::new(
            Opts::new(
                "storage_da_churn_bytes_total",
                "DA storage churn bytes per component and direction",
            ),
            &["component", "direction"],
        )
        .expect("Infallible");
        let governance_proposals_status = GenericGaugeVec::new(
            Opts::new(
                "governance_proposals_status",
                "Governance proposals grouped by status",
            ),
            &["status"],
        )
        .expect("Infallible");
        for status in ["proposed", "approved", "rejected", "enacted"] {
            governance_proposals_status
                .with_label_values(&[status])
                .set(0);
        }
        let governance_council_members = GenericGauge::new(
            "governance_council_members",
            "Latest persisted council member count",
        )
        .expect("Infallible");
        let governance_council_alternates = GenericGauge::new(
            "governance_council_alternates",
            "Latest persisted council alternate count",
        )
        .expect("Infallible");
        let governance_council_candidates = GenericGauge::new(
            "governance_council_candidates",
            "Total candidates considered in the latest council draw",
        )
        .expect("Infallible");
        let governance_council_verified = GenericGauge::new(
            "governance_council_verified",
            "Verified VRF proofs in the latest council draw",
        )
        .expect("Infallible");
        let governance_council_epoch = GenericGauge::new(
            "governance_council_epoch",
            "Epoch index for the latest persisted council draw",
        )
        .expect("Infallible");
        let governance_citizens_total = GenericGauge::new(
            "governance_citizens_total",
            "Total registered citizens bonded for governance",
        )
        .expect("Infallible");
        let governance_citizen_service_events_total = IntCounterVec::new(
            Opts::new(
                "governance_citizen_service_events_total",
                "Citizen service discipline events grouped by outcome",
            ),
            &["event"],
        )
        .expect("Infallible");
        for event in ["decline", "no_show", "misconduct"] {
            let _ = governance_citizen_service_events_total.with_label_values(&[event]);
        }
        let governance_protected_namespace_total = IntCounterVec::new(
            Opts::new(
                "governance_protected_namespace_total",
                "Protected namespace enforcement outcomes",
            ),
            &["outcome"],
        )
        .expect("Infallible");
        for outcome in ["allowed", "rejected"] {
            let _ = governance_protected_namespace_total.with_label_values(&[outcome]);
        }
        let governance_manifest_admission_total = IntCounterVec::new(
            Opts::new(
                "governance_manifest_admission_total",
                "Governance manifest admission outcomes (allowed, rejected by reason)",
            ),
            &["result"],
        )
        .expect("Infallible");
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
        let governance_manifest_quorum_total = IntCounterVec::new(
            Opts::new(
                "governance_manifest_quorum_total",
                "Manifest validator quorum enforcement outcomes",
            ),
            &["outcome"],
        )
        .expect("Infallible");
        for outcome in ["satisfied", "rejected"] {
            let _ = governance_manifest_quorum_total.with_label_values(&[outcome]);
        }
        let governance_manifest_hook_total = IntCounterVec::new(
            Opts::new(
                "governance_manifest_hook_total",
                "Governance manifest hook enforcement outcomes",
            ),
            &["hook", "outcome"],
        )
        .expect("Infallible");
        for hook in ["runtime_upgrade"] {
            for outcome in ["allowed", "rejected"] {
                let _ = governance_manifest_hook_total.with_label_values(&[hook, outcome]);
            }
        }
        let governance_manifest_activations_total = IntCounterVec::new(
            Opts::new(
                "governance_manifest_activations_total",
                "Manifest activation events emitted by governance enactment",
            ),
            &["event"],
        )
        .expect("Infallible");
        for event in ["manifest_inserted", "instance_bound"] {
            let _ = governance_manifest_activations_total.with_label_values(&[event]);
        }
        let governance_bond_events_total = IntCounterVec::new(
            Opts::new(
                "governance_bond_events_total",
                "Governance bond lifecycle events (lock_created|lock_extended|lock_unlocked)",
            ),
            &["event"],
        )
        .expect("Infallible");
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
        let da_receipt_cursors: Arc<RwLock<BTreeMap<DaReceiptCursorKey, u64>>> =
            Arc::new(RwLock::new(BTreeMap::new()));
        let alias_usage_total = IntCounterVec::new(
            Opts::new(
                "alias_usage_total",
                "Alias service events grouped by lane and event kind",
            ),
            &["lane", "event"],
        )
        .expect("Infallible");
        let iso_reference_status = IntGaugeVec::new(
            Opts::new(
                "iso_reference_status",
                "ISO bridge reference-data status (-1 failed, 0 missing, 1 loaded)",
            ),
            &["dataset"],
        )
        .expect("Infallible");
        let iso_reference_age_seconds = IntGaugeVec::new(
            Opts::new(
                "iso_reference_age_seconds",
                "ISO bridge reference-data age (seconds)",
            ),
            &["dataset"],
        )
        .expect("Infallible");
        let iso_reference_records = IntGaugeVec::new(
            Opts::new(
                "iso_reference_records",
                "ISO bridge reference-data record counts",
            ),
            &["dataset"],
        )
        .expect("Infallible");
        let iso_reference_refresh_interval_secs = IntGaugeVec::new(
            Opts::new(
                "iso_reference_refresh_interval_secs",
                "ISO bridge reference-data refresh interval (seconds)",
            ),
            &["dataset"],
        )
        .expect("Infallible");
        for dataset in ["isin_cusip", "bic_lei", "mic_directory"] {
            let _ = iso_reference_status.with_label_values(&[dataset]);
            let _ = iso_reference_age_seconds.with_label_values(&[dataset]);
            let _ = iso_reference_records.with_label_values(&[dataset]);
            let _ = iso_reference_refresh_interval_secs.with_label_values(&[dataset]);
        }
        let fraud_psp_assessments_total = IntCounterVec::new(
            Opts::new(
                "fraud_psp_assessments_total",
                "PSP fraud assessments accepted by the host",
            ),
            &["tenant", "band", "lane", "subnet"],
        )
        .expect("Infallible");
        let fraud_psp_missing_assessment_total = IntCounterVec::new(
            Opts::new(
                "fraud_psp_missing_assessment_total",
                "Transactions missing PSP fraud assessment metadata",
            ),
            &["tenant", "lane", "subnet", "cause"],
        )
        .expect("Infallible");
        let fraud_psp_invalid_metadata_total = IntCounterVec::new(
            Opts::new(
                "fraud_psp_invalid_metadata_total",
                "Invalid PSP fraud assessment metadata fields",
            ),
            &["tenant", "field", "lane", "subnet"],
        )
        .expect("Infallible");
        let fraud_psp_attestation_total = IntCounterVec::new(
            Opts::new(
                "fraud_psp_attestation_total",
                "Fraud assessment attestation verification outcomes",
            ),
            &["tenant", "engine", "lane", "subnet", "status"],
        )
        .expect("Infallible");
        let fraud_psp_latency_ms = HistogramVec::new(
            HistogramOpts::new(
                "fraud_psp_latency_ms",
                "Latency reported by PSP fraud scoring (milliseconds)",
            )
            .buckets(prometheus::exponential_buckets(5.0, 1.8, 12).expect("inputs are valid")),
            &["tenant", "lane", "subnet"],
        )
        .expect("Infallible");
        let fraud_psp_score_bps = HistogramVec::new(
            HistogramOpts::new(
                "fraud_psp_score_bps",
                "Fraud risk score distribution (basis points)",
            )
            .buckets(prometheus::linear_buckets(0.0, 500.0, 21).expect("inputs are valid")),
            &["tenant", "band", "lane", "subnet"],
        )
        .expect("Infallible");
        let fraud_psp_outcome_mismatch_total = IntCounterVec::new(
            Opts::new(
                "fraud_psp_outcome_mismatch_total",
                "Mismatch between PSP disposition and fraud scoring",
            ),
            &["tenant", "direction", "lane", "subnet"],
        )
        .expect("Infallible");
        let streaming_hpke_rekeys_total = IntCounterVec::new(
            Opts::new(
                "streaming_hpke_rekeys_total",
                "Streaming HPKE rekeys accepted grouped by suite",
            ),
            &["suite"],
        )
        .expect("Infallible");
        for suite in ["x25519", "kyber768"] {
            let _ = streaming_hpke_rekeys_total.with_label_values(&[suite]);
        }
        let streaming_gck_rotations_total = IntCounter::new(
            "streaming_gck_rotations_total",
            "Streaming content key rotations processed",
        )
        .expect("Infallible");
        let streaming_quic_datagrams_sent_total = IntCounter::new(
            "streaming_quic_datagrams_sent_total",
            "Streaming QUIC datagrams sent",
        )
        .expect("Infallible");
        let streaming_quic_datagrams_dropped_total = IntCounter::new(
            "streaming_quic_datagrams_dropped_total",
            "Streaming QUIC datagrams dropped",
        )
        .expect("Infallible");
        let streaming_fec_parity_current = GenericGaugeVec::new(
            Opts::new(
                "streaming_fec_parity_current",
                "Streaming parity bucket occupancy (active sessions per bucket)",
            ),
            &["bucket"],
        )
        .expect("Infallible");
        for bucket in ["0", "1", "2", "3", "4", "ge5"] {
            streaming_fec_parity_current
                .with_label_values(&[bucket])
                .set(0);
        }
        let streaming_feedback_timeout_total = IntCounter::new(
            "streaming_feedback_timeout_total",
            "Streaming feedback timeout events",
        )
        .expect("Infallible");
        let streaming_soranet_provision_fail_total = IntCounter::new(
            "streaming_soranet_provision_fail_total",
            "Streaming SoraNet privacy-route provisioning failures",
        )
        .expect("Infallible");
        let streaming_soranet_provision_queue_drop_total = IntCounterVec::new(
            Opts::new(
                "streaming_soranet_provision_queue_drop_total",
                "Streaming SoraNet provisioning queue drops grouped by reason",
            ),
            &["reason"],
        )
        .expect("Infallible");
        for reason in ["full", "disconnected"] {
            let _ = streaming_soranet_provision_queue_drop_total.with_label_values(&[reason]);
        }
        let telemetry_redaction_total = IntCounterVec::new(
            Opts::new(
                "telemetry_redaction_total",
                "Telemetry redaction events grouped by reason",
            ),
            &["reason"],
        )
        .expect("Infallible");
        for reason in ["keyword", "explicit"] {
            let _ = telemetry_redaction_total.with_label_values(&[reason]);
        }
        let telemetry_redaction_skipped_total = IntCounterVec::new(
            Opts::new(
                "telemetry_redaction_skipped_total",
                "Telemetry redaction skips grouped by reason",
            ),
            &["reason"],
        )
        .expect("Infallible");
        for reason in ["allowlist", "disabled", "unsupported"] {
            let _ = telemetry_redaction_skipped_total.with_label_values(&[reason]);
        }
        let telemetry_truncation_total =
            IntCounter::new("telemetry_truncation_total", "Telemetry field truncations")
                .expect("Infallible");
        let streaming_privacy_redaction_fail_total = IntCounter::new(
            "streaming_privacy_redaction_fail_total",
            "Streaming telemetry redaction failures",
        )
        .expect("Infallible");
        let streaming_encode_latency_ms = Histogram::with_opts(
            HistogramOpts::new(
                "streaming_encode_latency_ms",
                "Streaming encode latency in milliseconds",
            )
            .buckets(prometheus::exponential_buckets(1.0, 2.0, 10).expect("inputs are valid")),
        )
        .expect("Infallible");
        let streaming_encode_audio_jitter_ms = Histogram::with_opts(
            HistogramOpts::new(
                "streaming_encode_audio_jitter_ms",
                "Streaming audio jitter (EWMA) in milliseconds",
            )
            .buckets(prometheus::exponential_buckets(0.5, 2.0, 10).expect("inputs are valid")),
        )
        .expect("Infallible");
        let streaming_encode_audio_max_jitter_ms = GenericGauge::new(
            "streaming_encode_audio_max_jitter_ms",
            "Streaming maximum audio jitter observed in milliseconds",
        )
        .expect("Infallible");
        streaming_encode_audio_max_jitter_ms.set(0);
        let streaming_encode_dropped_layers_total = IntCounter::new(
            "streaming_encode_dropped_layers_total",
            "Streaming encode dropped layers",
        )
        .expect("Infallible");
        let streaming_decode_buffer_ms = Histogram::with_opts(
            HistogramOpts::new(
                "streaming_decode_buffer_ms",
                "Streaming decode buffer depth in milliseconds",
            )
            .buckets(prometheus::exponential_buckets(10.0, 1.8, 10).expect("inputs are valid")),
        )
        .expect("Infallible");
        let streaming_decode_dropped_frames_total = IntCounter::new(
            "streaming_decode_dropped_frames_total",
            "Streaming decode dropped frames",
        )
        .expect("Infallible");
        let streaming_decode_max_queue_ms = Histogram::with_opts(
            HistogramOpts::new(
                "streaming_decode_max_queue_ms",
                "Maximum streaming decode queue depth in milliseconds",
            )
            .buckets(prometheus::exponential_buckets(10.0, 1.8, 10).expect("inputs are valid")),
        )
        .expect("Infallible");
        let streaming_decode_av_drift_ms = Histogram::with_opts(
            HistogramOpts::new(
                "streaming_decode_av_drift_ms",
                "Streaming audio/video drift (absolute average, milliseconds)",
            )
            .buckets(prometheus::exponential_buckets(0.5, 2.0, 10).expect("inputs are valid")),
        )
        .expect("Infallible");
        let streaming_decode_max_drift_ms = GenericGauge::new(
            "streaming_decode_max_drift_ms",
            "Streaming maximum audio/video drift observed in milliseconds",
        )
        .expect("Infallible");
        streaming_decode_max_drift_ms.set(0);
        let streaming_audio_jitter_ms = Histogram::with_opts(
            HistogramOpts::new(
                "streaming_audio_jitter_ms",
                "Viewer-reported audio jitter in milliseconds",
            )
            .buckets(prometheus::exponential_buckets(0.5, 2.0, 10).expect("inputs are valid")),
        )
        .expect("Infallible");
        let streaming_audio_max_jitter_ms = GenericGauge::new(
            "streaming_audio_max_jitter_ms",
            "Viewer maximum audio jitter observed in milliseconds",
        )
        .expect("Infallible");
        streaming_audio_max_jitter_ms.set(0);
        let streaming_av_drift_ms = Histogram::with_opts(
            HistogramOpts::new(
                "streaming_av_drift_ms",
                "Viewer-reported audio/video drift (absolute average, milliseconds)",
            )
            .buckets(prometheus::exponential_buckets(0.5, 2.0, 10).expect("inputs are valid")),
        )
        .expect("Infallible");
        let streaming_av_max_drift_ms = GenericGauge::new(
            "streaming_av_max_drift_ms",
            "Viewer maximum audio/video drift observed in milliseconds",
        )
        .expect("Infallible");
        streaming_av_max_drift_ms.set(0);
        let streaming_av_drift_ewma_ms = IntGauge::new(
            "streaming_av_drift_ewma_ms",
            "Viewer-reported EWMA audio/video drift in milliseconds (signed)",
        )
        .expect("Infallible");
        streaming_av_drift_ewma_ms.set(0);
        let streaming_av_sync_window_ms = GenericGauge::new(
            "streaming_av_sync_window_ms",
            "Viewer sync diagnostics aggregation window in milliseconds",
        )
        .expect("Infallible");
        streaming_av_sync_window_ms.set(0);
        let streaming_av_sync_violation_total = IntCounter::new(
            "streaming_av_sync_violation_total",
            "Viewer sync violations observed",
        )
        .expect("Infallible");
        let streaming_network_rtt_ms = Histogram::with_opts(
            HistogramOpts::new(
                "streaming_network_rtt_ms",
                "Streaming network RTT in milliseconds",
            )
            .buckets(prometheus::exponential_buckets(1.0, 1.8, 12).expect("inputs are valid")),
        )
        .expect("Infallible");
        let streaming_network_loss_percent_x100 = Histogram::with_opts(
            HistogramOpts::new(
                "streaming_network_loss_percent_x100",
                "Streaming network packet loss (basis points)",
            )
            .buckets(prometheus::linear_buckets(0.0, 50.0, 21).expect("inputs are valid")),
        )
        .expect("Infallible");
        let streaming_network_fec_repairs_total = IntCounter::new(
            "streaming_network_fec_repairs_total",
            "Streaming network FEC repairs performed",
        )
        .expect("Infallible");
        let streaming_network_fec_failures_total = IntCounter::new(
            "streaming_network_fec_failures_total",
            "Streaming network FEC failures encountered",
        )
        .expect("Infallible");
        let streaming_network_datagram_reinjects_total = IntCounter::new(
            "streaming_network_datagram_reinjects_total",
            "Streaming network datagram reinjects issued",
        )
        .expect("Infallible");
        let streaming_energy_encoder_mw = Histogram::with_opts(
            HistogramOpts::new(
                "streaming_energy_encoder_mw",
                "Streaming encoder energy consumption (milliwatts)",
            )
            .buckets(prometheus::exponential_buckets(10.0, 1.8, 12).expect("inputs are valid")),
        )
        .expect("Infallible");
        let streaming_energy_decoder_mw = Histogram::with_opts(
            HistogramOpts::new(
                "streaming_energy_decoder_mw",
                "Streaming decoder energy consumption (milliwatts)",
            )
            .buckets(prometheus::exponential_buckets(10.0, 1.8, 12).expect("inputs are valid")),
        )
        .expect("Infallible");
        let nexus_audit_outcome_total = IntCounterVec::new(
            Opts::new(
                "nexus_audit_outcome_total",
                "Routed-trace audit outcomes grouped by trace identifier and status",
            ),
            &["trace_id", "status"],
        )
        .expect("Infallible");
        let nexus_audit_outcome_last_timestamp = GenericGaugeVec::new(
            Opts::new(
                "nexus_audit_outcome_last_timestamp_seconds",
                "Unix timestamp of the most recent routed-trace audit outcome per trace",
            ),
            &["trace_id"],
        )
        .expect("Infallible");
        let nexus_space_directory_revision_total = IntCounterVec::new(
            Opts::new(
                "nexus_space_directory_revision_total",
                "Space Directory manifest revisions observed per dataspace",
            ),
            &["dataspace", "dataspace_id"],
        )
        .expect("Infallible");
        let nexus_space_directory_active_manifests = GenericGaugeVec::new(
            Opts::new(
                "nexus_space_directory_active_manifests",
                "Active UAID capability manifests per dataspace/profile",
            ),
            &["dataspace", "dataspace_id", "profile"],
        )
        .expect("Infallible");
        let nexus_space_directory_revocations_total = IntCounterVec::new(
            Opts::new(
                "nexus_space_directory_revocations_total",
                "Capability manifest revocations grouped by dataspace and reason",
            ),
            &["dataspace", "dataspace_id", "reason"],
        )
        .expect("Infallible");
        let kaigi_relay_registered_total = IntCounterVec::new(
            Opts::new(
                "kaigi_relay_registered_total",
                "Kaigi relay registrations grouped by domain",
            ),
            &["domain"],
        )
        .expect("Infallible");
        let kaigi_relay_registration_bandwidth = HistogramVec::new(
            HistogramOpts::new(
                "kaigi_relay_registration_bandwidth_class",
                "Kaigi relay registration bandwidth class distribution",
            )
            .buckets(prometheus::linear_buckets(1.0, 1.0, 8).expect("inputs are valid")),
            &["domain"],
        )
        .expect("Infallible");
        let kaigi_relay_manifest_updates_total = IntCounterVec::new(
            Opts::new(
                "kaigi_relay_manifest_updates_total",
                "Kaigi relay manifest updates grouped by domain and action",
            ),
            &["domain", "action"],
        )
        .expect("Infallible");
        let kaigi_relay_manifest_hop_count = HistogramVec::new(
            HistogramOpts::new(
                "kaigi_relay_manifest_hop_count",
                "Kaigi relay manifest hop-count distribution",
            )
            .buckets(prometheus::linear_buckets(0.0, 1.0, 9).expect("inputs are valid")),
            &["domain"],
        )
        .expect("Infallible");
        let kaigi_relay_failover_total = IntCounterVec::new(
            Opts::new(
                "kaigi_relay_failover_total",
                "Kaigi relay failovers grouped by domain and call",
            ),
            &["domain", "call"],
        )
        .expect("Infallible");
        let kaigi_relay_failover_hop_count = HistogramVec::new(
            HistogramOpts::new(
                "kaigi_relay_failover_hop_count",
                "Kaigi relay failover hop-count distribution",
            )
            .buckets(prometheus::linear_buckets(0.0, 1.0, 9).expect("inputs are valid")),
            &["domain"],
        )
        .expect("Infallible");
        let kaigi_relay_health_reports_total = IntCounterVec::new(
            Opts::new(
                "kaigi_relay_health_reports_total",
                "Kaigi relay health reports grouped by domain and status",
            ),
            &["domain", "status"],
        )
        .expect("Infallible");
        let kaigi_relay_health_state = IntGaugeVec::new(
            Opts::new(
                "kaigi_relay_health_state",
                "Kaigi relay health status (0=healthy,1=degraded,2=unavailable)",
            ),
            &["domain", "relay"],
        )
        .expect("Infallible");
        let dropped_messages =
            IntCounter::new("dropped_messages", "Sumeragi dropped messages").expect("Infallible");
        let sumeragi_dropped_block_messages_total = IntCounter::new(
            "sumeragi_dropped_block_messages_total",
            "Dropped Sumeragi block messages due to full channel",
        )
        .expect("Infallible");
        let sumeragi_dropped_control_messages_total = IntCounter::new(
            "sumeragi_dropped_control_messages_total",
            "Dropped Sumeragi control messages due to full channel",
        )
        .expect("Infallible");
        let registry = Registry::new();
        register_guarded(&registry, &streaming_hpke_rekeys_total);
        register_guarded(&registry, &streaming_fec_parity_current);
        register_guarded(&registry, &streaming_soranet_provision_queue_drop_total);
        register_guarded(&registry, &streaming_encode_latency_ms);
        register_guarded(&registry, &streaming_encode_audio_jitter_ms);
        register_guarded(&registry, &streaming_encode_audio_max_jitter_ms);
        register_guarded(&registry, &streaming_decode_buffer_ms);
        register_guarded(&registry, &streaming_decode_max_queue_ms);
        register_guarded(&registry, &streaming_decode_av_drift_ms);
        register_guarded(&registry, &streaming_decode_max_drift_ms);
        register_guarded(&registry, &streaming_audio_jitter_ms);
        register_guarded(&registry, &streaming_audio_max_jitter_ms);
        register_guarded(&registry, &streaming_av_drift_ms);
        register_guarded(&registry, &streaming_av_max_drift_ms);
        register_guarded(&registry, &streaming_av_drift_ewma_ms);
        register_guarded(&registry, &streaming_av_sync_window_ms);
        register_guarded(&registry, &streaming_av_sync_violation_total);
        register_guarded(&registry, &streaming_network_rtt_ms);
        register_guarded(&registry, &streaming_network_loss_percent_x100);
        register_guarded(&registry, &streaming_energy_encoder_mw);
        register_guarded(&registry, &streaming_energy_decoder_mw);
        register_guarded(&registry, &nexus_audit_outcome_total);
        register_guarded(&registry, &nexus_audit_outcome_last_timestamp);
        register_guarded(&registry, &nexus_space_directory_revision_total);
        register_guarded(&registry, &nexus_space_directory_active_manifests);
        register_guarded(&registry, &nexus_space_directory_revocations_total);
        register!(
            registry,
            streaming_gck_rotations_total,
            streaming_quic_datagrams_sent_total,
            streaming_quic_datagrams_dropped_total,
            streaming_feedback_timeout_total,
            streaming_soranet_provision_fail_total,
            telemetry_redaction_total,
            telemetry_redaction_skipped_total,
            telemetry_truncation_total,
            streaming_privacy_redaction_fail_total,
            streaming_encode_dropped_layers_total,
            streaming_decode_dropped_frames_total,
            streaming_network_fec_repairs_total,
            streaming_network_fec_failures_total,
            streaming_network_datagram_reinjects_total
        );
        let sumeragi_npos_collector_selected_total = IntCounter::new(
            "sumeragi_npos_collector_selected_total",
            "NPoS PRF: node selected as collector",
        )
        .expect("Infallible");
        let sumeragi_npos_collector_assignments_by_idx = IntCounterVec::new(
            Opts::new(
                "sumeragi_npos_collector_assignments_by_idx",
                "NPoS PRF collector assignments by topology index",
            ),
            &["idx"],
        )
        .expect("Infallible");
        let sumeragi_vrf_commits_emitted_total = IntCounter::new(
            "sumeragi_vrf_commits_emitted_total",
            "VRF commit messages broadcast by this validator",
        )
        .expect("Infallible");
        let sumeragi_vrf_reveals_emitted_total = IntCounter::new(
            "sumeragi_vrf_reveals_emitted_total",
            "VRF reveal messages broadcast by this validator",
        )
        .expect("Infallible");
        let sumeragi_vrf_reveals_late_total = IntCounter::new(
            "sumeragi_vrf_reveals_late_total",
            "VRF reveals accepted after the reveal window",
        )
        .expect("Infallible");
        let sumeragi_vrf_non_reveal_penalties_total = IntCounter::new(
            "sumeragi_vrf_non_reveal_penalties_total",
            "VRF non-reveal penalties applied",
        )
        .expect("Infallible");
        let sumeragi_vrf_non_reveal_by_signer = IntCounterVec::new(
            Opts::new(
                "sumeragi_vrf_non_reveal_by_signer",
                "VRF non-reveal penalties by signer index",
            ),
            &["idx"],
        )
        .expect("Infallible");
        let sumeragi_vrf_no_participation_total = IntCounter::new(
            "sumeragi_vrf_no_participation_total",
            "VRF no-participation penalties applied",
        )
        .expect("Infallible");
        let sumeragi_vrf_no_participation_by_signer = IntCounterVec::new(
            Opts::new(
                "sumeragi_vrf_no_participation_by_signer",
                "VRF no-participation penalties by signer index",
            ),
            &["idx"],
        )
        .expect("Infallible");
        let sumeragi_vrf_rejects_total_by_reason = IntCounterVec::new(
            Opts::new(
                "sumeragi_vrf_rejects_total_by_reason",
                "VRF commit/reveal rejects by reason",
            ),
            &["reason"],
        )
        .expect("Infallible");
        let p2p_dropped_posts = GenericGauge::new(
            "p2p_dropped_posts",
            "Number of p2p post messages dropped due to backpressure",
        )
        .expect("Infallible");
        let p2p_dropped_broadcasts = GenericGauge::new(
            "p2p_dropped_broadcasts",
            "Number of p2p broadcast messages dropped due to backpressure",
        )
        .expect("Infallible");
        let p2p_subscriber_queue_full_total = GenericGauge::new(
            "p2p_subscriber_queue_full_total",
            "Number of inbound messages dropped because subscriber queues were full",
        )
        .expect("Infallible");
        let p2p_subscriber_queue_full_by_topic_total = GenericGaugeVec::new(
            Opts::new(
                "p2p_subscriber_queue_full_by_topic_total",
                "Per-topic inbound drops caused by full subscriber queues",
            ),
            &["topic"],
        )
        .expect("Infallible");
        let p2p_subscriber_unrouted_total = GenericGauge::new(
            "p2p_subscriber_unrouted_total",
            "Number of inbound messages dropped because no subscriber matches the topic",
        )
        .expect("Infallible");
        let p2p_subscriber_unrouted_by_topic_total = GenericGaugeVec::new(
            Opts::new(
                "p2p_subscriber_unrouted_by_topic_total",
                "Per-topic inbound drops caused by no matching subscriber",
            ),
            &["topic"],
        )
        .expect("Infallible");
        let p2p_handshake_failures =
            GenericGauge::new("p2p_handshake_failures", "Number of p2p handshake failures")
                .expect("Infallible");
        let p2p_low_post_throttled_total = GenericGauge::new(
            "p2p_low_post_throttled_total",
            "Number of low-priority post messages throttled",
        )
        .expect("Infallible");
        let p2p_low_broadcast_throttled_total = GenericGauge::new(
            "p2p_low_broadcast_throttled_total",
            "Number of low-priority broadcast deliveries throttled",
        )
        .expect("Infallible");
        let p2p_post_overflow_total = GenericGauge::new(
            "p2p_post_overflow_total",
            "Number of per-peer post channel overflows (bounded per-topic)",
        )
        .expect("Infallible");
        let p2p_post_overflow_by_topic = GenericGaugeVec::new(
            Opts::new(
                "p2p_post_overflow_by_topic",
                "Per-topic per-peer post channel overflows (bounded per-topic)",
            ),
            &["priority", "topic"],
        )
        .expect("Infallible");
        let consensus_ingress_drop_total = IntCounterVec::new(
            Opts::new(
                "consensus_ingress_drop_total",
                "Consensus ingress drops grouped by topic and reason",
            ),
            &["topic", "reason"],
        )
        .expect("Infallible");
        let p2p_dns_refresh_total = GenericGauge::new(
            "p2p_dns_refresh_total",
            "Number of DNS interval-based refresh cycles performed",
        )
        .expect("Infallible");
        let p2p_dns_ttl_refresh_total = GenericGauge::new(
            "p2p_dns_ttl_refresh_total",
            "Number of DNS TTL-based refresh cycles performed",
        )
        .expect("Infallible");
        let p2p_dns_resolution_fail_total = GenericGauge::new(
            "p2p_dns_resolution_fail_total",
            "Number of DNS resolution/connection failures for hostname peers",
        )
        .expect("Infallible");
        let p2p_dns_reconnect_success_total = GenericGauge::new(
            "p2p_dns_reconnect_success_total",
            "Number of DNS reconnect successes after refresh cycles",
        )
        .expect("Infallible");
        let p2p_backoff_scheduled_total = GenericGauge::new(
            "p2p_backoff_scheduled_total",
            "Number of per-address connect backoffs scheduled",
        )
        .expect("Infallible");
        let p2p_deferred_send_enqueued_total = GenericGauge::new(
            "p2p_deferred_send_enqueued_total",
            "Number of deferred outbound frames enqueued while peer sessions were unavailable",
        )
        .expect("Infallible");
        let p2p_deferred_send_dropped_total = GenericGauge::new(
            "p2p_deferred_send_dropped_total",
            "Number of deferred outbound frames dropped (expiry, overflow, stale generation)",
        )
        .expect("Infallible");
        let p2p_session_reconnect_total = GenericGauge::new(
            "p2p_session_reconnect_total",
            "Number of reconnect attempts triggered while deferring outbound frames",
        )
        .expect("Infallible");
        let p2p_connect_retry_seconds = GenericGauge::new(
            "p2p_connect_retry_seconds",
            "Cumulative reconnect retry delay in seconds (rounded up from milliseconds)",
        )
        .expect("Infallible");
        let p2p_accept_throttled_total = GenericGauge::new(
            "p2p_accept_throttled_total",
            "Number of incoming connections rejected by per-IP throttle",
        )
        .expect("Infallible");
        let p2p_accept_bucket_evictions_total = GenericGauge::new(
            "p2p_accept_bucket_evictions_total",
            "Number of accept throttle bucket evictions (idle or capacity)",
        )
        .expect("Infallible");
        let p2p_accept_buckets_current = GenericGauge::new(
            "p2p_accept_buckets_current",
            "Current number of active accept throttle buckets (prefix + per-IP)",
        )
        .expect("Infallible");
        let p2p_accept_prefix_cache_total = GenericGaugeVec::new(
            Opts::new(
                "p2p_accept_prefix_cache_total",
                "Prefix bucket cache hits/misses for accept throttle (label `result` = hit|miss)",
            ),
            &["result"],
        )
        .expect("Infallible");
        let p2p_accept_throttle_decisions_total = GenericGaugeVec::new(
            Opts::new(
                "p2p_accept_throttle_decisions_total",
                "Accept throttle decisions (label `scope` = prefix|ip, `decision` = allowed|throttled)",
            ),
            &["scope", "decision"],
        )
        .expect("Infallible");
        let p2p_incoming_cap_reject_total = GenericGauge::new(
            "p2p_incoming_cap_reject_total",
            "Number of incoming connections rejected due to incoming cap",
        )
        .expect("Infallible");
        let p2p_total_cap_reject_total = GenericGauge::new(
            "p2p_total_cap_reject_total",
            "Number of incoming connections rejected due to total cap",
        )
        .expect("Infallible");
        let p2p_trust_score = IntGaugeVec::new(
            Opts::new(
                "p2p_trust_score",
                "Per-peer trust score maintained by the gossip layer (decays toward zero)",
            ),
            &["peer_id"],
        )
        .expect("Infallible");
        let p2p_trust_penalties_total = IntCounterVec::new(
            Opts::new(
                "p2p_trust_penalties_total",
                "Applied trust penalties labeled by reason",
            ),
            &["reason"],
        )
        .expect("Infallible");
        let p2p_trust_decay_ticks_total = IntCounterVec::new(
            Opts::new(
                "p2p_trust_decay_ticks_total",
                "Half-life decay ticks applied to peer trust scores (label `peer_id`)",
            ),
            &["peer_id"],
        )
        .expect("Infallible");
        let p2p_trust_gossip_skipped_total = IntCounterVec::new(
            Opts::new(
                "p2p_trust_gossip_skipped_total",
                "Trust gossip frames skipped grouped by direction and reason",
            ),
            &["direction", "reason"],
        )
        .expect("Infallible");
        for direction in ["send", "recv"] {
            for reason in ["peer_capability_off", "local_capability_off"] {
                let _ = p2p_trust_gossip_skipped_total.with_label_values(&[direction, reason]);
            }
        }
        let p2p_ws_inbound_total = GenericGauge::new(
            "p2p_ws_inbound_total",
            "Accepted inbound WebSocket P2P connections",
        )
        .expect("Infallible");
        let p2p_ws_outbound_total = GenericGauge::new(
            "p2p_ws_outbound_total",
            "Successful outbound WebSocket P2P connections",
        )
        .expect("Infallible");
        let p2p_scion_inbound_total = GenericGauge::new(
            "p2p_scion_inbound_total",
            "Accepted inbound SCION P2P connections",
        )
        .expect("Infallible");
        let p2p_scion_outbound_total = GenericGauge::new(
            "p2p_scion_outbound_total",
            "Successful outbound SCION P2P connections",
        )
        .expect("Infallible");
        let tx_gossip_sent_total = IntCounterVec::new(
            Opts::new(
                "tx_gossip_sent_total",
                "Transaction gossip batches sent grouped by plane and dataspace",
            ),
            &["plane", "dataspace"],
        )
        .expect("Infallible");
        let tx_gossip_dropped_total = IntCounterVec::new(
            Opts::new(
                "tx_gossip_dropped_total",
                "Transaction gossip batches dropped grouped by plane, dataspace, and reason",
            ),
            &["plane", "dataspace", "reason"],
        )
        .expect("Infallible");
        let tx_gossip_targets = GenericGaugeVec::new(
            Opts::new(
                "tx_gossip_targets",
                "Latest transaction gossip target count grouped by plane and dataspace",
            ),
            &["plane", "dataspace"],
        )
        .expect("Infallible");
        let tx_gossip_fallback_total = IntCounterVec::new(
            Opts::new(
                "tx_gossip_fallback_total",
                "Restricted transaction gossip fallback attempts grouped by plane, dataspace, and surface",
            ),
            &["plane", "dataspace", "surface"],
        )
        .expect("Infallible");
        let tx_gossip_frame_cap_bytes = GenericGauge::new(
            "tx_gossip_frame_cap_bytes",
            "Configured maximum frame size for transaction gossip payloads (bytes)",
        )
        .expect("Infallible");
        let tx_gossip_public_target_cap = GenericGauge::new(
            "tx_gossip_public_target_cap",
            "Configured cap on public transaction gossip targets (0 = broadcast)",
        )
        .expect("Infallible");
        let tx_gossip_restricted_target_cap = GenericGauge::new(
            "tx_gossip_restricted_target_cap",
            "Configured cap on restricted transaction gossip targets (0 = commit topology)",
        )
        .expect("Infallible");
        let tx_gossip_public_target_reshuffle_ms = GenericGauge::new(
            "tx_gossip_public_target_reshuffle_ms",
            "Public-plane target reshuffle interval in milliseconds",
        )
        .expect("Infallible");
        let tx_gossip_restricted_target_reshuffle_ms = GenericGauge::new(
            "tx_gossip_restricted_target_reshuffle_ms",
            "Restricted-plane target reshuffle interval in milliseconds",
        )
        .expect("Infallible");
        let tx_gossip_drop_unknown_dataspace = GenericGauge::new(
            "tx_gossip_drop_unknown_dataspace",
            "Whether transaction gossip drops unknown dataspaces (1 = drop, 0 = route via restricted plane)",
        )
        .expect("Infallible");
        let tx_gossip_restricted_fallback = GenericGauge::new(
            "tx_gossip_restricted_fallback",
            "Restricted gossip fallback policy (0 = drop, 1 = public overlay)",
        )
        .expect("Infallible");
        let tx_gossip_restricted_public_policy = GenericGauge::new(
            "tx_gossip_restricted_public_policy",
            "Policy for restricted gossip when only the public overlay is available (0 = refuse, 1 = forward)",
        )
        .expect("Infallible");
        let tx_gossip_status = Arc::new(RwLock::new(Vec::new()));
        let tx_gossip_caps = Arc::new(RwLock::new(TxGossipCaps::default()));
        let sumeragi_new_view_receipts_by_hv = GenericGaugeVec::new(
            Opts::new(
                "sumeragi_new_view_receipts_by_hv",
                "NEW_VIEW receipt counts labeled by (height, view)",
            ),
            &["height", "view"],
        )
        .expect("Infallible");
        let sumeragi_post_to_peer_total = IntCounterVec::new(
            Opts::new(
                "sumeragi_post_to_peer_total",
                "Sumeragi post attempts to peers (collector routing; cumulative)",
            ),
            &["peer"],
        )
        .expect("Infallible");
        let sumeragi_bg_post_enqueued_total = IntCounterVec::new(
            Opts::new(
                "sumeragi_bg_post_enqueued_total",
                "Sumeragi background-post enqueued tasks (cumulative)",
            ),
            &["kind"],
        )
        .expect("Infallible");
        let sumeragi_bg_post_overflow_total = IntCounterVec::new(
            Opts::new(
                "sumeragi_bg_post_overflow_total",
                "Sumeragi background-post queue full events (backpressure applied)",
            ),
            &["kind"],
        )
        .expect("Infallible");
        let sumeragi_bg_post_drop_total = IntCounterVec::new(
            Opts::new(
                "sumeragi_bg_post_drop_total",
                "Sumeragi background-post drops when the worker queue is unavailable",
            ),
            &["kind"],
        )
        .expect("Infallible");
        let sumeragi_bg_post_queue_depth = GenericGauge::new(
            "sumeragi_bg_post_queue_depth",
            "Sumeragi background-post queue depth (global)",
        )
        .expect("Infallible");
        let sumeragi_bg_post_queue_depth_by_peer = GenericGaugeVec::new(
            Opts::new(
                "sumeragi_bg_post_queue_depth_by_peer",
                "Sumeragi background-post queue depth by peer id",
            ),
            &["peer"],
        )
        .expect("Infallible");
        let sumeragi_bg_post_age_ms = HistogramVec::new(
            HistogramOpts::new(
                "sumeragi_bg_post_age_ms",
                "Sumeragi background-post age in milliseconds by kind",
            )
            .buckets(prometheus::exponential_buckets(1.0, 2.0, 12).expect("inputs are valid")),
            &["kind"],
        )
        .expect("Infallible");
        let sumeragi_new_view_publish_total = IntCounter::new(
            "sumeragi_new_view_publish_total",
            "Sumeragi NEW_VIEW messages published (cumulative)",
        )
        .expect("Infallible");
        let sumeragi_new_view_recv_total = IntCounter::new(
            "sumeragi_new_view_recv_total",
            "Sumeragi NEW_VIEW messages received and accepted (cumulative)",
        )
        .expect("Infallible");
        let sumeragi_new_view_dropped_by_lock_total = IntCounter::new(
            "sumeragi_new_view_dropped_by_lock_total",
            "Sumeragi NEW_VIEW messages rejected because HighestQC is behind the locked QC",
        )
        .expect("Infallible");
        let sumeragi_commit_conflict_detected_total = IntCounter::new(
            "sumeragi_commit_conflict_detected_total",
            "Sumeragi commit-conflict detections (cumulative)",
        )
        .expect("Infallible");
        let sumeragi_missing_block_fetch_total = IntCounterVec::new(
            Opts::new(
                "sumeragi_missing_block_fetch_total",
                "Sumeragi missing-block fetch planning outcomes (requested|backoff|no_targets)",
            ),
            &["outcome"],
        )
        .expect("Infallible");
        let sumeragi_missing_block_fetch_target_total = IntCounterVec::new(
            Opts::new(
                "sumeragi_missing_block_fetch_target_total",
                "Sumeragi missing-block fetch target kind (signers|topology|none)",
            ),
            &["target"],
        )
        .expect("Infallible");
        let sumeragi_missing_block_fetch_dwell_ms = Histogram::with_opts(
            HistogramOpts::new(
                "sumeragi_missing_block_fetch_dwell_ms",
                "Elapsed milliseconds between first-seen certificate and missing-block fetch request",
            )
            .buckets(prometheus::exponential_buckets(10.0, 2.0, 8).expect("inputs are valid")),
        )
        .expect("Infallible");
        let sumeragi_missing_block_fetch_targets = Histogram::with_opts(
            HistogramOpts::new(
                "sumeragi_missing_block_fetch_targets",
                "Target peers selected when requesting a missing block payload",
            )
            .buckets(prometheus::exponential_buckets(1.0, 2.0, 6).expect("inputs are valid")),
        )
        .expect("Infallible");
        let blocksync_qc_quarantine_total = IntCounter::new(
            "blocksync_qc_quarantine_total",
            "Block-sync QCs quarantined while missing context is unresolved (cumulative)",
        )
        .expect("Infallible");
        let blocksync_qc_revalidated_total = IntCounter::new(
            "blocksync_qc_revalidated_total",
            "Quarantined block-sync QCs revalidated successfully (cumulative)",
        )
        .expect("Infallible");
        let blocksync_qc_final_drop_total = IntCounterVec::new(
            Opts::new(
                "blocksync_qc_final_drop_total",
                "Block-sync QCs dropped permanently after bounded quarantine (labeled by reason)",
            ),
            &["reason"],
        )
        .expect("Infallible");
        let qc_deferred_missing_payload_total = IntCounter::new(
            "qc_deferred_missing_payload_total",
            "QCs deferred because payload was not available locally (cumulative)",
        )
        .expect("Infallible");
        let qc_deferred_resolved_total = IntCounter::new(
            "qc_deferred_resolved_total",
            "Deferred QCs resolved after payload arrival (cumulative)",
        )
        .expect("Infallible");
        let qc_deferred_expired_total = IntCounter::new(
            "qc_deferred_expired_total",
            "Deferred QCs expired after bounded retries/escalation (cumulative)",
        )
        .expect("Infallible");
        let consensus_empty_commit_topology_defer_total = IntCounter::new(
            "consensus_empty_commit_topology_defer_total",
            "Consensus deferrals caused by empty commit topology (cumulative)",
        )
        .expect("Infallible");
        let consensus_empty_commit_topology_escalation_total = IntCounter::new(
            "consensus_empty_commit_topology_escalation_total",
            "Empty-topology recoveries escalated to forced view changes (cumulative)",
        )
        .expect("Infallible");
        let consensus_recovery_state_transitions_total = IntCounterVec::new(
            Opts::new(
                "consensus_recovery_state_transitions_total",
                "Consensus recovery state transitions (labeled by state)",
            ),
            &["state"],
        )
        .expect("Infallible");
        let consensus_missing_block_height_escalation_total = IntCounter::new(
            "consensus_missing_block_height_escalation_total",
            "Height-scoped missing-block recoveries escalated via deterministic hard cap (cumulative)",
        )
        .expect("Infallible");
        let consensus_sidecar_quarantine_total = IntCounter::new(
            "consensus_sidecar_quarantine_total",
            "Sidecar mismatches quarantined in fail-closed mode (cumulative)",
        )
        .expect("Infallible");
        let consensus_sidecar_final_drop_total = IntCounter::new(
            "consensus_sidecar_final_drop_total",
            "Sidecar mismatch entries final-dropped after retry/TTL bounds (cumulative)",
        )
        .expect("Infallible");
        let blocksync_range_pull_escalation_total = IntCounter::new(
            "blocksync_range_pull_escalation_total",
            "Block-sync range-pull escalations requested by recovery logic (cumulative)",
        )
        .expect("Infallible");
        let blocksync_range_pull_success_total = IntCounter::new(
            "blocksync_range_pull_success_total",
            "Block-sync range-pull recoveries that succeeded (cumulative)",
        )
        .expect("Infallible");
        let blocksync_range_pull_failure_total = IntCounter::new(
            "blocksync_range_pull_failure_total",
            "Block-sync range-pull recoveries that expired without progress (cumulative)",
        )
        .expect("Infallible");
        let consensus_recovery_stuck_round_seconds = Histogram::with_opts(
            HistogramOpts::new(
                "consensus_recovery_stuck_round_seconds",
                "Observed seconds spent in empty-topology/missing-dependency recovery rounds",
            )
            .buckets(prometheus::exponential_buckets(0.1, 2.0, 10).expect("inputs are valid")),
        )
        .expect("Infallible");
        let sumeragi_da_gate_block_total = IntCounterVec::new(
            Opts::new(
                "sumeragi_da_gate_block_total",
                "Sumeragi commits gated by missing data-availability artifacts (missing_local_data|manifest_*)",
            ),
            &["reason"],
        )
        .expect("Infallible");
        let sumeragi_da_gate_last_reason = GenericGauge::new(
            "sumeragi_da_gate_last_reason",
            "Sumeragi DA availability last reason code (0=none,1=missing_local_data,3=manifest_missing,4=manifest_hash_mismatch,5=manifest_read_failed,6=manifest_spool_scan)",
        )
        .expect("Infallible");
        let sumeragi_da_gate_last_satisfied = GenericGauge::new(
            "sumeragi_da_gate_last_satisfied",
            "Sumeragi DA availability last satisfaction code (0=none,1=missing_data_recovered)",
        )
        .expect("Infallible");
        let sumeragi_da_gate_satisfied_total = IntCounterVec::new(
            Opts::new(
                "sumeragi_da_gate_satisfied_total",
                "Sumeragi DA availability satisfactions (missing_data_recovered)",
            ),
            &["gate"],
        )
        .expect("Infallible");
        let sumeragi_da_manifest_guard_total = IntCounterVec::new(
            Opts::new(
                "sumeragi_da_manifest_guard_total",
                "Sumeragi DA manifest guard outcomes (allowed|rejected by reason)",
            ),
            &["result", "reason"],
        )
        .expect("Infallible");
        let sumeragi_da_manifest_cache_total = IntCounterVec::new(
            Opts::new(
                "sumeragi_da_manifest_cache_total",
                "Sumeragi DA manifest cache outcomes (hit|miss)",
            ),
            &["result"],
        )
        .expect("Infallible");
        let sumeragi_da_spool_cache_total = IntCounterVec::new(
            Opts::new(
                "sumeragi_da_spool_cache_total",
                "Sumeragi DA spool cache outcomes (hit|miss by kind)",
            ),
            &["kind", "result"],
        )
        .expect("Infallible");
        let sumeragi_da_pin_intent_spool_total = IntCounterVec::new(
            Opts::new(
                "sumeragi_da_pin_intent_spool_total",
                "Sumeragi DA pin intent spool handling outcomes (kept|dropped by reason)",
            ),
            &["result", "reason"],
        )
        .expect("Infallible");
        // RBC metrics
        let sumeragi_rbc_sessions_active = GenericGauge::new(
            "sumeragi_rbc_sessions_active",
            "Sumeragi RBC active sessions (gauge)",
        )
        .expect("Infallible");
        let sumeragi_rbc_sessions_pruned_total = IntCounter::new(
            "sumeragi_rbc_sessions_pruned_total",
            "Sumeragi RBC sessions pruned due to TTL",
        )
        .expect("Infallible");
        let sumeragi_rbc_ready_broadcasts_total = IntCounter::new(
            "sumeragi_rbc_ready_broadcasts_total",
            "Sumeragi RBC READY broadcasts sent",
        )
        .expect("Infallible");
        let sumeragi_rbc_rebroadcast_skipped_total = IntCounterVec::new(
            Opts::new(
                "sumeragi_rbc_rebroadcast_skipped_total",
                "Sumeragi RBC rebroadcasts skipped (kind=payload|ready)",
            ),
            &["kind"],
        )
        .expect("Infallible");
        let sumeragi_rbc_deliver_broadcasts_total = IntCounter::new(
            "sumeragi_rbc_deliver_broadcasts_total",
            "Sumeragi RBC DELIVER broadcasts sent",
        )
        .expect("Infallible");
        let sumeragi_rbc_payload_bytes_delivered_total = GenericGauge::new(
            "sumeragi_rbc_payload_bytes_delivered_total",
            "Sumeragi RBC payload bytes delivered and cached (cumulative)",
        )
        .expect("Infallible");
        let sumeragi_rbc_lane_tx_count = GenericGaugeVec::new(
            Opts::new(
                "sumeragi_rbc_lane_tx_count",
                "Sumeragi RBC backlog transactions served per lane",
            ),
            &["lane_id"],
        )
        .expect("Infallible");
        let sumeragi_rbc_lane_total_chunks = GenericGaugeVec::new(
            Opts::new(
                "sumeragi_rbc_lane_total_chunks",
                "Sumeragi RBC total chunks attributed to a lane",
            ),
            &["lane_id"],
        )
        .expect("Infallible");
        let sumeragi_rbc_lane_pending_chunks = GenericGaugeVec::new(
            Opts::new(
                "sumeragi_rbc_lane_pending_chunks",
                "Sumeragi RBC pending chunks attributed to a lane",
            ),
            &["lane_id"],
        )
        .expect("Infallible");
        let sumeragi_rbc_lane_bytes_total = GenericGaugeVec::new(
            Opts::new(
                "sumeragi_rbc_lane_bytes_total",
                "Sumeragi RBC payload bytes attributed to a lane",
            ),
            &["lane_id"],
        )
        .expect("Infallible");
        let sumeragi_rbc_dataspace_tx_count = GenericGaugeVec::new(
            Opts::new(
                "sumeragi_rbc_dataspace_tx_count",
                "Sumeragi RBC backlog transactions served per dataspace",
            ),
            &["lane_id", "dataspace_id"],
        )
        .expect("Infallible");
        let sumeragi_rbc_dataspace_total_chunks = GenericGaugeVec::new(
            Opts::new(
                "sumeragi_rbc_dataspace_total_chunks",
                "Sumeragi RBC total chunks attributed to a dataspace",
            ),
            &["lane_id", "dataspace_id"],
        )
        .expect("Infallible");
        let sumeragi_rbc_dataspace_pending_chunks = GenericGaugeVec::new(
            Opts::new(
                "sumeragi_rbc_dataspace_pending_chunks",
                "Sumeragi RBC pending chunks attributed to a dataspace",
            ),
            &["lane_id", "dataspace_id"],
        )
        .expect("Infallible");
        let sumeragi_rbc_dataspace_bytes_total = GenericGaugeVec::new(
            Opts::new(
                "sumeragi_rbc_dataspace_bytes_total",
                "Sumeragi RBC payload bytes attributed to a dataspace",
            ),
            &["lane_id", "dataspace_id"],
        )
        .expect("Infallible");
        let sumeragi_da_votes_ingested_total = IntCounter::new(
            "sumeragi_da_votes_ingested_total",
            "Availability votes ingested by this collector (cumulative)",
        )
        .expect("Infallible");
        let sumeragi_da_votes_ingested_by_collector = IntCounterVec::new(
            Opts::new(
                "sumeragi_da_votes_ingested_by_collector",
                "Availability votes ingested labeled by collector topology index",
            ),
            &["collector_idx"],
        )
        .expect("Infallible");
        let sumeragi_da_votes_ingested_by_peer = IntCounterVec::new(
            Opts::new(
                "sumeragi_da_votes_ingested_by_peer",
                "Availability votes ingested labeled by collector peer id",
            ),
            &["peer"],
        )
        .expect("Infallible");
        let sumeragi_qc_assembly_latency_ms = HistogramVec::new(
            HistogramOpts::new(
                "sumeragi_qc_assembly_latency_ms",
                "Latency from first vote to QC assembly (ms) labeled by kind",
            )
            .buckets(vec![
                5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2000.0, 5000.0,
            ]),
            &["kind"],
        )
        .expect("Infallible");
        let sumeragi_qc_last_latency_ms = GenericGaugeVec::new(
            Opts::new(
                "sumeragi_qc_last_latency_ms",
                "Last observed QC assembly latency (ms) labeled by kind",
            ),
            &["kind"],
        )
        .expect("Infallible");
        let sumeragi_rbc_store_sessions = GenericGauge::new(
            "sumeragi_rbc_store_sessions",
            "Sumeragi RBC persisted store sessions (gauge)",
        )
        .expect("Infallible");
        let sumeragi_rbc_store_bytes = GenericGauge::new(
            "sumeragi_rbc_store_bytes",
            "Sumeragi RBC persisted store payload bytes (gauge)",
        )
        .expect("Infallible");
        let sumeragi_rbc_store_pressure = GenericGauge::new(
            "sumeragi_rbc_store_pressure",
            "Sumeragi RBC store pressure level (0=normal,1=soft,2=hard)",
        )
        .expect("Infallible");
        let sumeragi_rbc_store_evictions_total = IntCounter::new(
            "sumeragi_rbc_store_evictions_total",
            "Sumeragi RBC persisted sessions evicted due to TTL or capacity limits",
        )
        .expect("Infallible");
        let sumeragi_rbc_persist_drops_total = IntCounter::new(
            "sumeragi_rbc_persist_drops_total",
            "Sumeragi RBC persist requests dropped due to full async queue",
        )
        .expect("Infallible");
        let sumeragi_rbc_backpressure_deferrals_total = IntCounter::new(
            "sumeragi_rbc_backpressure_deferrals_total",
            "Sumeragi RBC proposal deferrals due to store back-pressure",
        )
        .expect("Infallible");
        let sumeragi_rbc_deliver_defer_ready_total = IntCounter::new(
            "sumeragi_rbc_deliver_defer_ready_total",
            "Sumeragi RBC DELIVER deferrals waiting on READY quorum",
        )
        .expect("Infallible");
        let sumeragi_rbc_deliver_defer_chunks_total = IntCounter::new(
            "sumeragi_rbc_deliver_defer_chunks_total",
            "Sumeragi RBC DELIVER deferrals waiting on missing chunks",
        )
        .expect("Infallible");
        let sumeragi_rbc_da_reschedule_total = IntCounter::new(
            "sumeragi_rbc_da_reschedule_total",
            "Sumeragi RBC DA deadline reschedules triggered",
        )
        .expect("Infallible");
        let sumeragi_rbc_da_reschedule_by_mode_total = IntCounterVec::new(
            Opts::new(
                "sumeragi_rbc_da_reschedule_by_mode_total",
                "Sumeragi RBC DA deadline reschedules triggered (labeled by consensus mode)",
            ),
            &["mode"],
        )
        .expect("Infallible");
        let sumeragi_rbc_abort_total = IntCounterVec::new(
            Opts::new(
                "sumeragi_rbc_abort_total",
                "Sumeragi pending blocks aborted due to missing/mismatched/invalid RBC payload (labeled by consensus mode)",
            ),
            &["mode"],
        )
        .expect("Infallible");
        let sumeragi_rbc_mismatch_total = IntCounterVec::new(
            Opts::new(
                "sumeragi_rbc_mismatch_total",
                "Sumeragi RBC payload mismatches attributed to peers (labeled by peer, kind)",
            ),
            &["peer", "kind"],
        )
        .expect("Infallible");
        let sumeragi_kura_store_failures_total = IntCounterVec::new(
            Opts::new(
                "sumeragi_kura_store_failures_total",
                "Sumeragi kura persistence failures (outcome=retry|abort)",
            ),
            &["outcome"],
        )
        .expect("Infallible");
        let sumeragi_kura_store_last_retry_attempt = GenericGauge::new(
            "sumeragi_kura_store_last_retry_attempt",
            "Most recent kura persistence retry attempt (gauge)",
        )
        .expect("Infallible");
        let sumeragi_kura_store_last_retry_backoff_ms = GenericGauge::new(
            "sumeragi_kura_store_last_retry_backoff_ms",
            "Most recent kura persistence retry backoff in milliseconds (gauge)",
        )
        .expect("Infallible");
        let sumeragi_pacemaker_backpressure_deferrals_total = IntCounter::new(
            "sumeragi_pacemaker_backpressure_deferrals_total",
            "Sumeragi pacemaker proposal deferrals due to transaction-queue back-pressure",
        )
        .expect("Infallible");
        let sumeragi_pacemaker_backpressure_deferrals_by_reason_total = IntCounterVec::new(
            Opts::new(
                "sumeragi_pacemaker_backpressure_deferrals_by_reason_total",
                "Sumeragi pacemaker backpressure deferrals grouped by reason",
            ),
            &["reason"],
        )
        .expect("Infallible");
        let sumeragi_pacemaker_backpressure_deferral_duration_ms = HistogramVec::new(
            HistogramOpts::new(
                "sumeragi_pacemaker_backpressure_deferral_duration_ms",
                "Sumeragi pacemaker backpressure deferral duration histogram (ms) grouped by reason",
            )
            .buckets(vec![
                5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0,
                20000.0,
            ]),
            &["reason"],
        )
        .expect("Infallible");
        let sumeragi_pacemaker_backpressure_deferral_active = GenericGaugeVec::new(
            Opts::new(
                "sumeragi_pacemaker_backpressure_deferral_active",
                "Sumeragi pacemaker backpressure deferral active state (0/1) grouped by reason",
            ),
            &["reason"],
        )
        .expect("Infallible");
        let sumeragi_pacemaker_backpressure_deferral_age_ms = GenericGaugeVec::new(
            Opts::new(
                "sumeragi_pacemaker_backpressure_deferral_age_ms",
                "Sumeragi pacemaker backpressure deferral age in milliseconds grouped by reason",
            ),
            &["reason"],
        )
        .expect("Infallible");
        let sumeragi_pacemaker_eval_ms = Histogram::with_opts(
            HistogramOpts::new(
                "sumeragi_pacemaker_eval_ms",
                "Sumeragi pacemaker evaluation duration histogram (ms)",
            )
            .buckets(vec![
                1.0, 2.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0,
            ]),
        )
        .expect("Infallible");
        let sumeragi_pacemaker_propose_ms = Histogram::with_opts(
            HistogramOpts::new(
                "sumeragi_pacemaker_propose_ms",
                "Sumeragi pacemaker proposal attempt duration histogram (ms)",
            )
            .buckets(vec![
                1.0, 2.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0,
            ]),
        )
        .expect("Infallible");
        let sumeragi_commit_stage_ms = HistogramVec::new(
            HistogramOpts::new(
                "sumeragi_commit_stage_ms",
                "Sumeragi commit pipeline stage duration histogram (ms) labeled by stage",
            )
            .buckets(vec![
                1.0, 2.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2000.0, 5000.0,
                10000.0, 20000.0,
            ]),
            &["stage"],
        )
        .expect("Infallible");
        let state_commit_view_lock_wait_ms = Histogram::with_opts(
            HistogramOpts::new(
                "state_commit_view_lock_wait_ms",
                "State commit view_lock wait duration histogram (ms)",
            )
            .buckets(vec![
                1.0, 2.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0,
                10000.0,
            ]),
        )
        .expect("Infallible");
        let state_commit_view_lock_hold_ms = Histogram::with_opts(
            HistogramOpts::new(
                "state_commit_view_lock_hold_ms",
                "State commit view_lock hold duration histogram (ms)",
            )
            .buckets(vec![
                1.0, 2.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0,
                10000.0,
            ]),
        )
        .expect("Infallible");
        let sumeragi_commit_pipeline_tick_total = IntCounterVec::new(
            Opts::new(
                "sumeragi_commit_pipeline_tick_total",
                "Sumeragi commit pipeline executions triggered by the pacemaker tick loop",
            ),
            &["mode", "outcome"],
        )
        .expect("Infallible");
        let sumeragi_prevote_timeout_total = IntCounterVec::new(
            Opts::new(
                "sumeragi_prevote_timeout_total",
                "Sumeragi prevote-quorum timeouts that triggered rebroadcast + view change",
            ),
            &["mode"],
        )
        .expect("Infallible");
        let sumeragi_rbc_backlog_chunks_total = GenericGauge::new(
            "sumeragi_rbc_backlog_chunks_total",
            "Total missing RBC chunks across active sessions",
        )
        .expect("Infallible");
        let sumeragi_rbc_backlog_chunks_max = GenericGauge::new(
            "sumeragi_rbc_backlog_chunks_max",
            "Maximum missing RBC chunks within a single session",
        )
        .expect("Infallible");
        let sumeragi_rbc_backlog_sessions_pending = GenericGauge::new(
            "sumeragi_rbc_backlog_sessions_pending",
            "Number of RBC sessions pending delivery",
        )
        .expect("Infallible");
        let sumeragi_rbc_pending_sessions: GenericGauge<AtomicU64> = GenericGauge::new(
            "sumeragi_rbc_pending_sessions",
            "Pending RBC stashes awaiting INIT (count)",
        )
        .expect("Infallible");
        let sumeragi_rbc_pending_chunks: GenericGauge<AtomicU64> = GenericGauge::new(
            "sumeragi_rbc_pending_chunks",
            "Pending RBC chunk frames buffered before INIT",
        )
        .expect("Infallible");
        let sumeragi_rbc_pending_bytes: GenericGauge<AtomicU64> = GenericGauge::new(
            "sumeragi_rbc_pending_bytes",
            "Pending RBC payload/signature bytes buffered before INIT",
        )
        .expect("Infallible");
        let sumeragi_rbc_pending_drops_total = IntCounterVec::new(
            Opts::new(
                "sumeragi_rbc_pending_drops_total",
                "Pending RBC frames dropped before INIT (labeled by reason)",
            ),
            &["reason"],
        )
        .expect("Infallible");
        let sumeragi_rbc_pending_dropped_bytes_total = IntCounterVec::new(
            Opts::new(
                "sumeragi_rbc_pending_dropped_bytes_total",
                "Pending RBC bytes dropped before INIT (labeled by reason)",
            ),
            &["reason"],
        )
        .expect("Infallible");
        let sumeragi_rbc_pending_evicted_total = IntCounter::new(
            "sumeragi_rbc_pending_evicted_total",
            "Pending RBC stashes evicted due to TTL or stash cap (sessions)",
        )
        .expect("Infallible");
        let sumeragi_membership_mismatch_total = IntCounterVec::new(
            Opts::new(
                "sumeragi_membership_mismatch_total",
                "Sumeragi consensus membership mismatches detected",
            ),
            &["peer", "height", "view"],
        )
        .expect("Infallible");
        let sumeragi_membership_mismatch_active = GenericGaugeVec::new(
            Opts::new(
                "sumeragi_membership_mismatch_active",
                "Peers currently flagged for consensus membership mismatches",
            ),
            &["peer"],
        )
        .expect("Infallible");
        let sumeragi_highest_qc_height = GenericGauge::new(
            "sumeragi_highest_qc_height",
            "Sumeragi highest QC height (gauge)",
        )
        .expect("Infallible");
        let sumeragi_locked_qc_height = GenericGauge::new(
            "sumeragi_locked_qc_height",
            "Sumeragi locked QC height (gauge)",
        )
        .expect("Infallible");
        let sumeragi_locked_qc_view =
            GenericGauge::new("sumeragi_locked_qc_view", "Sumeragi locked QC view (gauge)")
                .expect("Infallible");
        // Sumeragi pacemaker gauges
        let sumeragi_pacemaker_backoff_ms = GenericGauge::new(
            "sumeragi_pacemaker_backoff_ms",
            "Pacemaker current backoff window (ms) for view-change timeout",
        )
        .expect("Infallible");
        let sumeragi_pacemaker_rtt_floor_ms = GenericGauge::new(
            "sumeragi_pacemaker_rtt_floor_ms",
            "Pacemaker RTT-based floor (ms) for backoff window",
        )
        .expect("Infallible");
        let sumeragi_pacemaker_backoff_multiplier = GenericGauge::new(
            "sumeragi_pacemaker_backoff_multiplier",
            "Pacemaker backoff multiplier (dimensionless)",
        )
        .expect("Infallible");
        let sumeragi_pacemaker_rtt_floor_multiplier = GenericGauge::new(
            "sumeragi_pacemaker_rtt_floor_multiplier",
            "Pacemaker RTT floor multiplier (dimensionless)",
        )
        .expect("Infallible");
        let sumeragi_pacemaker_max_backoff_ms = GenericGauge::new(
            "sumeragi_pacemaker_max_backoff_ms",
            "Pacemaker maximum backoff cap (ms)",
        )
        .expect("Infallible");
        let sumeragi_pacemaker_jitter_ms = GenericGauge::new(
            "sumeragi_pacemaker_jitter_ms",
            "Pacemaker jitter offset magnitude (ms) applied to window",
        )
        .expect("Infallible");
        let sumeragi_pacemaker_jitter_frac_permille = GenericGauge::new(
            "sumeragi_pacemaker_jitter_frac_permille",
            "Pacemaker jitter band config (permille of window)",
        )
        .expect("Infallible");
        let sumeragi_pacemaker_round_elapsed_ms = GenericGauge::new(
            "sumeragi_pacemaker_round_elapsed_ms",
            "Elapsed time in the current consensus round (ms)",
        )
        .expect("Infallible");
        let sumeragi_pacemaker_view_timeout_target_ms = GenericGauge::new(
            "sumeragi_pacemaker_view_timeout_target_ms",
            "Current view timeout target window (ms) before backoff/jitter adjustments",
        )
        .expect("Infallible");
        let sumeragi_pacemaker_view_timeout_remaining_ms = GenericGauge::new(
            "sumeragi_pacemaker_view_timeout_remaining_ms",
            "Remaining time until current view timeout elapses (ms)",
        )
        .expect("Infallible");
        let sumeragi_phase_latency_ms = HistogramVec::new(
            HistogramOpts::new(
                "sumeragi_phase_latency_ms",
                "Sumeragi per-phase latency histogram (ms) labeled by phase",
            )
            .buckets(vec![
                5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2000.0, 5000.0,
            ]),
            &["phase"],
        )
        .expect("Infallible");
        let sumeragi_phase_latency_ema_ms = GenericGaugeVec::new(
            Opts::new(
                "sumeragi_phase_latency_ema_ms",
                "Sumeragi per-phase EMA latency (ms) labeled by phase",
            ),
            &["phase"],
        )
        .expect("Infallible");
        let sumeragi_phase_total_ema_ms = GenericGauge::new(
            "sumeragi_phase_total_ema_ms",
            "Sumeragi aggregate pipeline EMA latency (ms) across pacemaker-controlled phases",
        )
        .expect("Infallible");
        let p2p_queue_depth = GenericGaugeVec::new(
            Opts::new(
                "p2p_queue_depth",
                "Network message queue depth by priority (High/Low)",
            ),
            &["priority"],
        )
        .expect("Infallible");
        let p2p_queue_dropped_total = GenericGaugeVec::new(
            Opts::new(
                "p2p_queue_dropped_total",
                "Network message queue drops by priority (High/Low) and kind (Post/Broadcast)",
            ),
            &["priority", "kind"],
        )
        .expect("Infallible");
        let p2p_handshake_ms_bucket = GenericGaugeVec::new(
            Opts::new(
                "p2p_handshake_ms_bucket",
                "P2P handshake latency histogram buckets (ms)",
            ),
            &["le"],
        )
        .expect("Infallible");
        let p2p_handshake_ms_sum = GenericGauge::new(
            "p2p_handshake_ms_sum",
            "Sum of handshake latencies in milliseconds",
        )
        .expect("Infallible");
        let p2p_handshake_ms_count =
            GenericGauge::new("p2p_handshake_ms_count", "Count of observed handshakes")
                .expect("Infallible");
        let p2p_handshake_error_total = GenericGaugeVec::new(
            Opts::new(
                "p2p_handshake_error_total",
                "Handshake error counts by kind",
            ),
            &["kind"],
        )
        .expect("Infallible");
        let p2p_frame_cap_violations_total = GenericGaugeVec::new(
            Opts::new(
                "p2p_frame_cap_violations_total",
                "Inbound per-topic frame-cap violations (dropped oversized messages)",
            ),
            &["topic"],
        )
        .expect("Infallible");
        // Runtime upgrade metrics
        let runtime_upgrade_events_total = IntCounterVec::new(
            Opts::new(
                "runtime_upgrade_events_total",
                "Runtime upgrade lifecycle events total labeled by kind",
            ),
            &["kind"],
        )
        .expect("Infallible");
        let runtime_upgrade_provenance_rejections_total = IntCounterVec::new(
            Opts::new(
                "runtime_upgrade_provenance_rejections_total",
                "Runtime upgrade provenance rejections total labeled by reason",
            ),
            &["reason"],
        )
        .expect("Infallible");
        let runtime_abi_version =
            GenericGauge::new("runtime_abi_version", "ABI version allowed by runtime")
                .expect("Infallible");
        // Sumeragi consensus counters/histogram
        let sumeragi_tail_votes_total =
            IntCounter::new("sumeragi_tail_votes_total", "Votes accepted at proxy tail")
                .expect("Infallible");
        let sumeragi_votes_sent_total = IntCounterVec::new(
            Opts::new(
                "sumeragi_votes_sent_total",
                "Consensus votes sent grouped by phase",
            ),
            &["phase"],
        )
        .expect("Infallible");
        let sumeragi_votes_received_total = IntCounterVec::new(
            Opts::new(
                "sumeragi_votes_received_total",
                "Consensus votes received grouped by phase",
            ),
            &["phase"],
        )
        .expect("Infallible");
        let sumeragi_qc_sent_total = IntCounterVec::new(
            Opts::new(
                "sumeragi_qc_sent_total",
                "Quorum certificates sent grouped by kind",
            ),
            &["kind"],
        )
        .expect("Infallible");
        let sumeragi_qc_received_total = IntCounterVec::new(
            Opts::new(
                "sumeragi_qc_received_total",
                "Quorum certificates received grouped by kind",
            ),
            &["kind"],
        )
        .expect("Infallible");
        let sumeragi_qc_validation_errors_total = IntCounterVec::new(
            Opts::new(
                "sumeragi_qc_validation_errors_total",
                "QC validation errors grouped by reason",
            ),
            &["reason"],
        )
        .expect("Infallible");
        let sumeragi_qc_signer_counts = HistogramVec::new(
            HistogramOpts::new(
                "sumeragi_qc_signer_counts",
                "QC signer tallies grouped by phase (prevote/precommit/available) and kind (present|counted)",
            )
            .buckets(prometheus::linear_buckets(0.0, 1.0, 64).expect("valid signer buckets")),
            &["phase", "kind"],
        )
        .expect("Infallible");
        let sumeragi_invalid_signature_total = IntCounterVec::new(
            Opts::new(
                "sumeragi_invalid_signature_total",
                "Invalid signatures dropped grouped by message kind and throttle outcome",
            ),
            &["kind", "outcome"],
        )
        .expect("Infallible");
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
        let sumeragi_validation_reject_total = IntCounterVec::new(
            Opts::new(
                "sumeragi_validation_reject_total",
                "Blocks rejected by validation gate before voting, grouped by reason",
            ),
            &["reason"],
        )
        .expect("Infallible");
        for label in [
            "stateless",
            "execution",
            "prev_hash",
            "prev_height",
            "topology",
        ] {
            let _ = sumeragi_validation_reject_total.with_label_values(&[label]);
        }
        let sumeragi_validation_reject_last_reason = GenericGauge::new(
            "sumeragi_validation_reject_last_reason",
            "Sumeragi validation gate last reject reason code (0=none,1=stateless,2=execution,3=prev_hash,4=prev_height,5=topology)",
        )
        .expect("Infallible");
        let sumeragi_validation_reject_last_height = GenericGauge::new(
            "sumeragi_validation_reject_last_height",
            "Sumeragi validation gate last reject block height (0 when unset)",
        )
        .expect("Infallible");
        let sumeragi_validation_reject_last_view = GenericGauge::new(
            "sumeragi_validation_reject_last_view",
            "Sumeragi validation gate last reject block view (0 when unset)",
        )
        .expect("Infallible");
        let sumeragi_validation_reject_last_timestamp_ms = GenericGauge::new(
            "sumeragi_validation_reject_last_timestamp_ms",
            "Sumeragi validation gate last reject timestamp in milliseconds since unix epoch (0 when unset)",
        )
        .expect("Infallible");
        let sumeragi_block_sync_roster_source_total = IntCounterVec::new(
            Opts::new(
                "sumeragi_block_sync_roster_source_total",
                "Block-sync roster selection grouped by source",
            ),
            &["source"],
        )
        .expect("Infallible");
        for label in [
            "commit_qc_hint",
            "commit_checkpoint_pair_hint",
            "validator_checkpoint_hint",
            "commit_qc_history",
            "validator_checkpoint_history",
            "roster_sidecar",
            "commit_roster_journal",
        ] {
            let _ = sumeragi_block_sync_roster_source_total.with_label_values(&[label]);
        }
        let sumeragi_block_sync_roster_drop_total = IntCounterVec::new(
            Opts::new(
                "sumeragi_block_sync_roster_drop_total",
                "Block-sync drops grouped by roster validation reason",
            ),
            &["reason"],
        )
        .expect("Infallible");
        let _ = sumeragi_block_sync_roster_drop_total.with_label_values(&["missing"]);
        let sumeragi_block_sync_share_blocks_unsolicited_total = IntCounter::new(
            "sumeragi_block_sync_share_blocks_unsolicited_total",
            "Block-sync ShareBlocks dropped because no matching request was tracked",
        )
        .expect("Infallible");
        let sumeragi_consensus_message_handling_total = IntCounterVec::new(
            Opts::new(
                "sumeragi_consensus_message_handling_total",
                "Consensus message drops/deferrals grouped by kind, outcome, and reason",
            ),
            &["kind", "outcome", "reason"],
        )
        .expect("Infallible");
        let sumeragi_view_change_cause_total = IntCounterVec::new(
            Opts::new(
                "sumeragi_view_change_cause_total",
                "View-change triggers grouped by cause",
            ),
            &["cause"],
        )
        .expect("Infallible");
        let sumeragi_view_change_cause_last_timestamp_ms = GenericGaugeVec::new(
            Opts::new(
                "sumeragi_view_change_cause_last_timestamp_ms",
                "Unix timestamp (ms) of the last view-change trigger grouped by cause",
            ),
            &["cause"],
        )
        .expect("Infallible");
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
        for kind in ["vote", "rbc_ready", "rbc_deliver"] {
            for outcome in ["logged", "throttled"] {
                let _ = sumeragi_invalid_signature_total.with_label_values(&[kind, outcome]);
            }
        }
        let sumeragi_widen_before_rotate_total = IntCounter::new(
            "sumeragi_widen_before_rotate_total",
            "Widen-before-rotate events",
        )
        .expect("Infallible");
        let sumeragi_view_change_suggest_total = IntCounter::new(
            "sumeragi_view_change_suggest_total",
            "View-change suggestions emitted",
        )
        .expect("Infallible");
        let sumeragi_view_change_install_total = IntCounter::new(
            "sumeragi_view_change_install_total",
            "View-change installs observed",
        )
        .expect("Infallible");
        let sumeragi_proposal_gap_total = IntCounter::new(
            "sumeragi_proposal_gap_total",
            "View-change rotations after no proposal observed before cutoff",
        )
        .expect("Infallible");
        let sumeragi_view_change_proof_total = GenericGaugeVec::new(
            Opts::new(
                "sumeragi_view_change_proof_total",
                "View-change proof outcomes observed locally",
            ),
            &["outcome"],
        )
        .expect("Infallible");
        for label in ["accepted", "stale", "rejected"] {
            let _ = sumeragi_view_change_proof_total.with_label_values(&[label]);
        }
        let sumeragi_wa_qc_assembled_total = IntCounter::new(
            "sumeragi_wa_qc_assembled_total",
            "Witness-availability QC assembled (cumulative)",
        )
        .expect("Infallible");
        let sumeragi_cert_size = Histogram::with_opts(
            HistogramOpts::new(
                "sumeragi_cert_size",
                "Certificate size (signatures) per committed block",
            )
            .buckets(prometheus::exponential_buckets(1.0, 1.8, 10).expect("valid")),
        )
        .expect("Infallible");
        let sumeragi_commit_signatures_present = GenericGauge::<AtomicU64>::new(
            "sumeragi_commit_signatures_present",
            "Block signatures observed during commit validation (all roles)",
        )
        .expect("Infallible");
        let sumeragi_commit_signatures_counted = GenericGauge::<AtomicU64>::new(
            "sumeragi_commit_signatures_counted",
            "Commit-quorum signatures counted toward the threshold",
        )
        .expect("Infallible");
        let sumeragi_commit_signatures_set_b = GenericGauge::<AtomicU64>::new(
            "sumeragi_commit_signatures_set_b",
            "Set B validator signatures present during commit validation",
        )
        .expect("Infallible");
        let sumeragi_commit_signatures_required = GenericGauge::<AtomicU64>::new(
            "sumeragi_commit_signatures_required",
            "Commit-quorum size required by the active topology",
        )
        .expect("Infallible");
        let sumeragi_commit_qc_height = GenericGauge::<AtomicU64>::new(
            "sumeragi_commit_qc_height",
            "Latest commit certificate height (best-effort)",
        )
        .expect("Infallible");
        let sumeragi_commit_qc_view = GenericGauge::<AtomicU64>::new(
            "sumeragi_commit_qc_view",
            "Latest commit certificate view (best-effort)",
        )
        .expect("Infallible");
        let sumeragi_commit_qc_epoch = GenericGauge::<AtomicU64>::new(
            "sumeragi_commit_qc_epoch",
            "Latest commit certificate epoch (best-effort)",
        )
        .expect("Infallible");
        let sumeragi_commit_qc_signatures_total = GenericGauge::<AtomicU64>::new(
            "sumeragi_commit_qc_signatures_total",
            "Signatures attached to the latest commit certificate",
        )
        .expect("Infallible");
        let sumeragi_commit_qc_validator_set_len = GenericGauge::<AtomicU64>::new(
            "sumeragi_commit_qc_validator_set_len",
            "Validator-set size for the latest commit certificate",
        )
        .expect("Infallible");
        let sumeragi_redundant_sends_total = IntCounter::new(
            "sumeragi_redundant_sends_total",
            "Redundant vote sends to collectors due to timeouts",
        )
        .expect("Infallible");
        let sumeragi_redundant_sends_by_collector = IntCounterVec::new(
            Opts::new(
                "sumeragi_redundant_sends_by_collector",
                "Redundant vote sends by collector index",
            ),
            &["idx"],
        )
        .expect("Infallible");
        let sumeragi_redundant_sends_by_peer = IntCounterVec::new(
            Opts::new(
                "sumeragi_redundant_sends_by_peer",
                "Redundant vote sends by collector peer id",
            ),
            &["peer"],
        )
        .expect("Infallible");
        let sumeragi_collectors_k =
            GenericGauge::new("sumeragi_collectors_k", "Current collectors_k parameter")
                .expect("Infallible");
        let sumeragi_redundant_send_r = GenericGauge::new(
            "sumeragi_redundant_send_r",
            "Current redundant_send_r parameter",
        )
        .expect("Infallible");
        let sumeragi_gossip_fallback_total = IntCounter::new(
            "sumeragi_gossip_fallback_total",
            "Gossip fallback broadcasts triggered after redundant collectors exhausted",
        )
        .expect("Infallible");
        let sumeragi_block_created_dropped_by_lock_total = IntCounter::new(
            "sumeragi_block_created_dropped_by_lock_total",
            "BlockCreated messages dropped because they violate the locked QC gate",
        )
        .expect("Infallible");
        let sumeragi_block_created_hint_mismatch_total = IntCounter::new(
            "sumeragi_block_created_hint_mismatch_total",
            "BlockCreated messages rejected due to hint mismatch (height/view/parent)",
        )
        .expect("Infallible");
        let sumeragi_block_created_proposal_mismatch_total = IntCounter::new(
            "sumeragi_block_created_proposal_mismatch_total",
            "BlockCreated messages rejected due to proposal mismatch (header/payload)",
        )
        .expect("Infallible");
        let lane_relay_invalid_total = IntCounterVec::new(
            Opts::new(
                "lane_relay_invalid_total",
                "Lane relay envelopes rejected during validation (labeled by error kind)",
            ),
            &["error"],
        )
        .expect("Infallible");
        let lane_relay_emergency_override_total = IntCounterVec::new(
            Opts::new(
                "lane_relay_emergency_override_total",
                "Lane relay emergency validator override usage (labeled by lane, dataspace, outcome)",
            ),
            &["lane", "dataspace", "outcome"],
        )
        .expect("Infallible");
        register!(
            registry,
            lane_relay_invalid_total,
            lane_relay_emergency_override_total
        );
        let sumeragi_collectors_targeted_current = GenericGauge::new(
            "sumeragi_collectors_targeted_current",
            "Number of collectors targeted for the current voting block",
        )
        .expect("Infallible");
        let sumeragi_collectors_targeted_per_block = Histogram::with_opts(
            HistogramOpts::new(
                "sumeragi_collectors_targeted_per_block",
                "Collectors targeted per block (observed at commit)",
            )
            .buckets(prometheus::linear_buckets(0.0, 1.0, 16).expect("valid")),
        )
        .expect("Infallible");
        let sumeragi_prf_epoch_seed_hex: Arc<RwLock<Option<String>>> = Arc::new(RwLock::new(None));
        let sumeragi_mode_tag: Arc<RwLock<String>> =
            Arc::new(RwLock::new(PERMISSIONED_TAG.to_string()));
        let sumeragi_staged_mode_tag: Arc<RwLock<Option<String>>> = Arc::new(RwLock::new(None));
        let sumeragi_staged_mode_activation_height: Arc<RwLock<Option<u64>>> =
            Arc::new(RwLock::new(None));
        let sumeragi_mode_activation_lag_blocks = IntGauge::new(
            "sumeragi_mode_activation_lag_blocks",
            "Blocks since staged mode activation height elapsed without flipping",
        )
        .expect("Infallible");
        let sumeragi_mode_activation_lag_blocks_opt: Arc<RwLock<Option<u64>>> =
            Arc::new(RwLock::new(None));
        let sumeragi_mode_flip_kill_switch = IntGauge::new(
            "sumeragi_mode_flip_kill_switch",
            "Runtime consensus mode flip enable switch (1 = enabled, 0 = disabled)",
        )
        .expect("Infallible");
        let sumeragi_mode_flip_success_total = IntCounterVec::new(
            Opts::new(
                "sumeragi_mode_flip_success_total",
                "Successful runtime consensus mode flips",
            ),
            &["mode_tag"],
        )
        .expect("Infallible");
        let sumeragi_mode_flip_failure_total = IntCounterVec::new(
            Opts::new(
                "sumeragi_mode_flip_failure_total",
                "Failed runtime consensus mode flip attempts",
            ),
            &["mode_tag"],
        )
        .expect("Infallible");
        let sumeragi_mode_flip_blocked_total = IntCounterVec::new(
            Opts::new(
                "sumeragi_mode_flip_blocked_total",
                "Blocked runtime consensus mode flip attempts",
            ),
            &["mode_tag"],
        )
        .expect("Infallible");
        let sumeragi_last_mode_flip_timestamp_ms = IntGauge::new(
            "sumeragi_last_mode_flip_timestamp_ms",
            "Timestamp (ms since UNIX epoch) of the last mode flip attempt",
        )
        .expect("Infallible");
        register!(
            registry,
            sumeragi_mode_flip_kill_switch,
            sumeragi_mode_flip_success_total,
            sumeragi_mode_flip_failure_total,
            sumeragi_mode_flip_blocked_total,
            sumeragi_last_mode_flip_timestamp_ms
        );
        let halo2_status: Arc<RwLock<Halo2Status>> = Arc::new(RwLock::new(Halo2Status::default()));
        let sumeragi_prf_height = GenericGauge::new(
            "sumeragi_prf_height",
            "Height associated with the PRF context",
        )
        .expect("Infallible");
        let sumeragi_prf_view =
            GenericGauge::new("sumeragi_prf_view", "View associated with the PRF context")
                .expect("Infallible");
        let sumeragi_membership_view_hash = GenericGauge::new(
            "sumeragi_membership_view_hash",
            "Deterministic membership view hash (truncated to u64)",
        )
        .expect("Infallible");
        let sumeragi_membership_height = GenericGauge::new(
            "sumeragi_membership_height",
            "Height associated with the latest membership view hash snapshot",
        )
        .expect("Infallible");
        let sumeragi_membership_view = GenericGauge::new(
            "sumeragi_membership_view",
            "View associated with the latest membership view hash snapshot",
        )
        .expect("Infallible");
        let sumeragi_membership_epoch = GenericGauge::new(
            "sumeragi_membership_epoch",
            "Epoch associated with the latest membership view hash snapshot",
        )
        .expect("Infallible");
        let sumeragi_redundant_sends_by_role_idx = IntCounterVec::new(
            Opts::new(
                "sumeragi_redundant_sends_by_role_idx",
                "Redundant vote sends by local validator index",
            ),
            &["role_idx"],
        )
        .expect("Infallible");
        let sumeragi_leader_index =
            GenericGauge::new("sumeragi_leader_index", "Current leader index").expect("Infallible");
        let ivm_cache_hits = GenericGauge::new(
            "ivm_cache_hits",
            "IVM opcode pre-decode cache hits (cumulative)",
        )
        .expect("Infallible");
        let ivm_cache_misses = GenericGauge::new(
            "ivm_cache_misses",
            "IVM opcode pre-decode cache misses (cumulative)",
        )
        .expect("Infallible");
        let ivm_cache_evictions = GenericGauge::new(
            "ivm_cache_evictions",
            "IVM opcode pre-decode cache evictions (cumulative)",
        )
        .expect("Infallible");
        let ivm_cache_decoded_streams = GenericGauge::new(
            "ivm_cache_decoded_streams",
            "IVM opcode pre-decode decoded streams (cumulative)",
        )
        .expect("Infallible");
        let ivm_cache_decoded_ops_total = GenericGauge::new(
            "ivm_cache_decoded_ops_total",
            "IVM opcode pre-decode decoded operations (cumulative)",
        )
        .expect("Infallible");
        let ivm_cache_decode_failures = GenericGauge::new(
            "ivm_cache_decode_failures",
            "IVM opcode pre-decode decode failures (cumulative)",
        )
        .expect("Infallible");
        let ivm_cache_decode_time_ns_total = GenericGauge::new(
            "ivm_cache_decode_time_ns_total",
            "IVM opcode pre-decode total decode time in nanoseconds (cumulative)",
        )
        .expect("Infallible");
        let ivm_register_max_index = Histogram::with_opts(
            HistogramOpts::new(
                "ivm_register_max_index",
                "Maximum general-purpose register index touched per VM execution",
            )
            .buckets(vec![
                16.0, 32.0, 48.0, 64.0, 96.0, 128.0, 160.0, 192.0, 224.0, 256.0, 320.0, 384.0,
                448.0, 512.0,
            ]),
        )
        .expect("Infallible");
        let ivm_register_unique_count = Histogram::with_opts(
            HistogramOpts::new(
                "ivm_register_unique_count",
                "Unique general-purpose registers touched per VM execution",
            )
            .buckets(vec![
                8.0, 16.0, 24.0, 32.0, 64.0, 96.0, 128.0, 160.0, 192.0, 224.0, 256.0, 320.0, 384.0,
                448.0, 512.0,
            ]),
        )
        .expect("Infallible");
        let merkle_root_gpu_total = IntCounter::new(
            "merkle_root_gpu_total",
            "Merkle root computations using GPU acceleration (cumulative)",
        )
        .expect("Infallible");
        let merkle_root_cpu_total = IntCounter::new(
            "merkle_root_cpu_total",
            "Merkle root computations using CPU (cumulative)",
        )
        .expect("Infallible");
        let pipeline_dag_vertices = GenericGauge::new(
            "pipeline_dag_vertices",
            "DAG vertices (transactions) in the latest validated block",
        )
        .expect("Infallible");
        let pipeline_dag_edges = GenericGauge::new(
            "pipeline_dag_edges",
            "DAG edges (conflicts) in the latest validated block",
        )
        .expect("Infallible");
        let pipeline_conflict_rate_bps = GenericGauge::new(
            "pipeline_conflict_rate_bps",
            "DAG conflict rate (basis points) for the latest validated block",
        )
        .expect("Infallible");
        let pipeline_access_set_source_total = IntCounterVec::new(
            Opts::new(
                "pipeline_access_set_source_total",
                "Cumulative access-set source counts used by the scheduler",
            ),
            &["source"],
        )
        .expect("Infallible");
        let pipeline_comp_count = GenericGauge::new(
            "pipeline_comp_count",
            "Number of independent components (DSF partitions) in the latest validated block",
        )
        .expect("Infallible");
        let pipeline_comp_max = GenericGauge::new(
            "pipeline_comp_max",
            "Size of the largest independent component in the latest validated block",
        )
        .expect("Infallible");
        let pipeline_comp_hist_bucket = GenericGaugeVec::new(
            Opts::new(
                "pipeline_comp_hist_bucket",
                "Component-size histogram buckets (component count per bucket)",
            ),
            &["le"],
        )
        .expect("Infallible");
        let pipeline_peak_layer_width = GenericGauge::new(
            "pipeline_peak_layer_width",
            "Peak layer width (max txs in any layer) for the latest validated block",
        )
        .expect("Infallible");
        let pipeline_layer_avg_width = GenericGauge::new(
            "pipeline_layer_avg_width",
            "Average layer width (rounded) for the latest validated block",
        )
        .expect("Infallible");
        let pipeline_layer_median_width = GenericGauge::new(
            "pipeline_layer_median_width",
            "Median layer width for the latest validated block",
        )
        .expect("Infallible");
        let nexus_lane_id_placeholder = GenericGauge::new(
            "nexus_lane_id_placeholder",
            "Placeholder lane identifier while nexus routing is in single-lane mode",
        )
        .expect("Infallible");
        let nexus_dataspace_id_placeholder = GenericGauge::new(
            "nexus_dataspace_id_placeholder",
            "Placeholder data-space identifier while nexus routing is in single-lane mode",
        )
        .expect("Infallible");
        let nexus_config_diff_total = IntCounterVec::new(
            Opts::new(
                "nexus_config_diff_total",
                "Cumulative count of Nexus config diffs applied, grouped by knob/profile",
            ),
            &["knob", "profile"],
        )
        .expect("Infallible");
        let nexus_lane_configured_total = GenericGauge::new(
            "nexus_lane_configured_total",
            "Number of Nexus lane catalog entries configured on this node",
        )
        .expect("Infallible");
        let nexus_lane_governance_sealed = GenericGaugeVec::new(
            Opts::new(
                "nexus_lane_governance_sealed",
                "Per-lane governance seal status (1 = sealed, 0 = ready)",
            ),
            &["lane"],
        )
        .expect("Infallible");
        let nexus_lane_governance_sealed_total = GenericGauge::new(
            "nexus_lane_governance_sealed_total",
            "Total number of lanes still sealed (missing manifest)",
        )
        .expect("Infallible");
        let nexus_lane_lifecycle_applied_total = IntCounterVec::new(
            Opts::new(
                "nexus_lane_lifecycle_applied_total",
                "Lifecycle plan applications grouped by outcome",
            ),
            &["result"],
        )
        .expect("Infallible");
        let nexus_lane_governance_sealed_aliases = Arc::new(RwLock::new(Vec::new()));
        let nexus_lane_block_height = GenericGaugeVec::new(
            Opts::new(
                "nexus_lane_block_height",
                "Latest block height recorded per lane/dataspace pair",
            ),
            &["lane", "dataspace"],
        )
        .expect("Infallible");
        let nexus_lane_finality_lag_slots = GenericGaugeVec::new(
            Opts::new(
                "nexus_lane_finality_lag_slots",
                "Finality lag (in slots) between the global head height and each lane",
            ),
            &["lane", "dataspace"],
        )
        .expect("Infallible");
        let nexus_lane_settlement_backlog_xor = GaugeVec::new(
            Opts::new(
                "nexus_lane_settlement_backlog_xor",
                "Settlement backlog per lane/dataspace pair expressed in XOR",
            ),
            &["lane", "dataspace"],
        )
        .expect("Infallible");
        let nexus_public_lane_validator_total = IntGaugeVec::new(
            Opts::new(
                "nexus_public_lane_validator_total",
                "Public-lane validator counts grouped by lifecycle status",
            ),
            &["lane", "status"],
        )
        .expect("Infallible");
        let nexus_public_lane_validator_activation_total = IntCounterVec::new(
            Opts::new(
                "nexus_public_lane_validator_activation_total",
                "Public-lane validator activations grouped by lane",
            ),
            &["lane"],
        )
        .expect("Infallible");
        let nexus_public_lane_validator_reject_total = IntCounterVec::new(
            Opts::new(
                "nexus_public_lane_validator_reject_total",
                "Rejected public-lane validator registrations grouped by reason",
            ),
            &["reason"],
        )
        .expect("Infallible");
        let nexus_public_lane_stake_bonded = GaugeVec::new(
            Opts::new(
                "nexus_public_lane_stake_bonded",
                "Total bonded stake per public lane (Numeric rendered as float)",
            ),
            &["lane"],
        )
        .expect("Infallible");
        let nexus_public_lane_unbond_pending = GaugeVec::new(
            Opts::new(
                "nexus_public_lane_unbond_pending",
                "Pending unbond amount per public lane",
            ),
            &["lane"],
        )
        .expect("Infallible");
        let nexus_public_lane_reward_total = GaugeVec::new(
            Opts::new(
                "nexus_public_lane_reward_total",
                "Cumulative rewards recorded per public lane",
            ),
            &["lane"],
        )
        .expect("Infallible");
        let nexus_public_lane_slash_total = IntCounterVec::new(
            Opts::new(
                "nexus_public_lane_slash_total",
                "Slash events recorded per public lane",
            ),
            &["lane"],
        )
        .expect("Infallible");
        let nexus_scheduler_lane_teu_capacity = GenericGaugeVec::new(
            Opts::new(
                "nexus_scheduler_lane_teu_capacity",
                "Configured TEU capacity for the current slot per lane",
            ),
            &["lane"],
        )
        .expect("Infallible");
        let nexus_scheduler_lane_teu_slot_committed = GenericGaugeVec::new(
            Opts::new(
                "nexus_scheduler_lane_teu_slot_committed",
                "TEU committed in the current slot per lane",
            ),
            &["lane"],
        )
        .expect("Infallible");
        let nexus_scheduler_lane_trigger_level = GenericGaugeVec::new(
            Opts::new(
                "nexus_scheduler_lane_trigger_level",
                "Active circuit-breaker trigger level (0 = normal) per lane",
            ),
            &["lane"],
        )
        .expect("Infallible");
        let nexus_scheduler_starvation_bound_slots = GenericGaugeVec::new(
            Opts::new(
                "nexus_scheduler_starvation_bound_slots",
                "Starvation bound in slots per lane",
            ),
            &["lane"],
        )
        .expect("Infallible");
        let nexus_scheduler_lane_teu_slot_breakdown = GenericGaugeVec::new(
            Opts::new(
                "nexus_scheduler_lane_teu_slot_breakdown",
                "Committed TEU bucket breakdown per lane",
            ),
            &["lane", "bucket"],
        )
        .expect("Infallible");
        let nexus_scheduler_lane_teu_deferral_total = IntCounterVec::new(
            Opts::new(
                "nexus_scheduler_lane_teu_deferral_total",
                "Cumulative TEU deferrals by reason per lane",
            ),
            &["lane", "reason"],
        )
        .expect("Infallible");
        let nexus_scheduler_lane_headroom_events_total = IntCounterVec::new(
            Opts::new(
                "nexus_scheduler_lane_headroom_events_total",
                "Structured headroom telemetry events per lane",
            ),
            &["lane"],
        )
        .expect("Infallible");
        let nexus_scheduler_must_serve_truncations_total = IntCounterVec::new(
            Opts::new(
                "nexus_scheduler_must_serve_truncations_total",
                "Cumulative must-serve truncations per lane",
            ),
            &["lane"],
        )
        .expect("Infallible");
        let nexus_scheduler_dataspace_teu_backlog = GenericGaugeVec::new(
            Opts::new(
                "nexus_scheduler_dataspace_teu_backlog",
                "Pending TEU backlog per dataspace (labeled by lane)",
            ),
            &["lane", "dataspace"],
        )
        .expect("Infallible");
        let nexus_scheduler_dataspace_age_slots = GenericGaugeVec::new(
            Opts::new(
                "nexus_scheduler_dataspace_age_slots",
                "Slots since dataspace was last served (labeled by lane)",
            ),
            &["lane", "dataspace"],
        )
        .expect("Infallible");
        let nexus_scheduler_dataspace_virtual_finish = GenericGaugeVec::new(
            Opts::new(
                "nexus_scheduler_dataspace_virtual_finish",
                "SFQ virtual finish tag per dataspace (labeled by lane)",
            ),
            &["lane", "dataspace"],
        )
        .expect("Infallible");
        let nexus_scheduler_lane_teu_status =
            Arc::new(RwLock::new(BTreeMap::<u32, NexusLaneTeuStatus>::new()));
        let nexus_scheduler_dataspace_teu_status = Arc::new(RwLock::new(BTreeMap::<
            (u32, u64),
            NexusDataspaceTeuStatus,
        >::new()));
        let pipeline_layer_count = GenericGauge::new(
            "pipeline_layer_count",
            "Number of scheduler layers in the latest validated block",
        )
        .expect("Infallible");
        let pipeline_scheduler_utilization_pct = GenericGauge::new(
            "pipeline_scheduler_utilization_pct",
            "Average parallelism utilization in percent (0..100) for the latest validated block",
        )
        .expect("Infallible");
        let pipeline_layer_width_hist_bucket = GenericGaugeVec::new(
            Opts::new(
                "pipeline_layer_width_hist_bucket",
                "Layer-width histogram buckets (layer count per bucket)",
            ),
            &["le"],
        )
        .expect("Infallible");
        let pipeline_overlay_count = GenericGauge::new(
            "pipeline_overlay_count",
            "Number of overlays built in the latest validated block",
        )
        .expect("Infallible");
        let pipeline_overlay_instructions = GenericGauge::new(
            "pipeline_overlay_instructions",
            "Total instruction count across overlays in the latest validated block",
        )
        .expect("Infallible");
        let pipeline_overlay_bytes = GenericGauge::new(
            "pipeline_overlay_bytes",
            "Total Norito-encoded byte size across overlays in the latest validated block",
        )
        .expect("Infallible");
        let pipeline_quarantine_classified = GenericGauge::new(
            "pipeline_quarantine_classified",
            "Transactions classified into quarantine lane in the latest validated block",
        )
        .expect("Infallible");
        let pipeline_quarantine_overflow = GenericGauge::new(
            "pipeline_quarantine_overflow",
            "Transactions rejected due to quarantine overflow in the latest validated block",
        )
        .expect("Infallible");
        let pipeline_quarantine_executed = GenericGauge::new(
            "pipeline_quarantine_executed",
            "Transactions successfully executed in quarantine lane in the latest validated block",
        )
        .expect("Infallible");
        let pipeline_stage_ms = HistogramVec::new(
            HistogramOpts::new(
                "pipeline_stage_ms",
                "Pipeline stage duration (milliseconds) by lane and stage",
            )
            .buckets(prometheus::exponential_buckets(1.0, 2.0, 12).expect("inputs are valid")),
            &["lane", "stage"],
        )
        .expect("Infallible");
        let pipeline_detached_prepared = GenericGauge::new(
            "pipeline_detached_prepared",
            "Detached pipeline: prepared tx deltas in the latest validated block",
        )
        .expect("Infallible");
        let pipeline_detached_merged = GenericGauge::new(
            "pipeline_detached_merged",
            "Detached pipeline: successfully merged tx deltas in the latest validated block",
        )
        .expect("Infallible");
        let pipeline_detached_fallback = GenericGauge::new(
            "pipeline_detached_fallback",
            "Detached pipeline: txs that fell back to direct apply in the latest validated block",
        )
        .expect("Infallible");
        let merge_ledger_entries_total = IntCounter::new(
            "merge_ledger_entries_total",
            "Total merge-ledger entries appended on this node",
        )
        .expect("Infallible");
        let merge_ledger_latest_epoch = GenericGauge::new(
            "merge_ledger_latest_epoch",
            "Latest merge-ledger epoch committed on this node",
        )
        .expect("Infallible");
        let merge_ledger_latest_root_hex: Arc<RwLock<Option<String>>> = Arc::new(RwLock::new(None));

        // Torii metrics (app-facing): record filter complexity, match counts,
        // scan latencies, and approximate stream sizes. Labeled by endpoint.
        let torii_filter_depth = HistogramVec::new(
            HistogramOpts::new(
                "torii_filter_depth",
                "Torii filter expression depth by endpoint",
            )
            .buckets(vec![1.0, 2.0, 3.0, 5.0, 8.0, 13.0]),
            &["endpoint"],
        )
        .expect("Infallible");
        let torii_filter_match_count = HistogramVec::new(
            HistogramOpts::new(
                "torii_filter_match_count",
                "Torii filter match count (items) by endpoint",
            )
            .buckets(prometheus::exponential_buckets(1.0, 2.0, 12).expect("inputs are valid")),
            &["endpoint"],
        )
        .expect("Infallible");
        let torii_scan_ms = HistogramVec::new(
            HistogramOpts::new(
                "torii_scan_ms",
                "Torii scan time in milliseconds by endpoint",
            )
            .buckets(prometheus::exponential_buckets(0.5, 2.0, 14).expect("inputs are valid")),
            &["endpoint"],
        )
        .expect("Infallible");
        let torii_stream_rows = HistogramVec::new(
            HistogramOpts::new("torii_stream_rows", "Torii stream row count by endpoint")
                .buckets(prometheus::exponential_buckets(1.0, 2.0, 16).expect("inputs are valid")),
            &["endpoint"],
        )
        .expect("Infallible");
        let torii_lane_admission_latency_seconds = HistogramVec::new(
            HistogramOpts::new(
                "torii_lane_admission_latency_seconds",
                "Torii transaction admission latency (seconds) by lane and endpoint",
            )
            .buckets(prometheus::exponential_buckets(0.001, 2.0, 16).expect("inputs are valid")),
            &["lane_id", "endpoint"],
        )
        .expect("Infallible");
        let torii_attachment_reject_total = IntCounterVec::new(
            Opts::new(
                "torii_attachment_reject_total",
                "Torii attachment rejects grouped by reason",
            ),
            &["reason"],
        )
        .expect("Infallible");
        let torii_attachment_sanitize_ms = HistogramVec::new(
            HistogramOpts::new(
                "torii_attachment_sanitize_ms",
                "Torii attachment sanitization latency in milliseconds",
            )
            .buckets(prometheus::exponential_buckets(0.5, 2.0, 14).expect("inputs are valid")),
            &[],
        )
        .expect("Infallible");
        let torii_zk_prover_attachment_bytes = HistogramVec::new(
            HistogramOpts::new(
                "torii_zk_prover_attachment_bytes",
                "Background prover attachment size (bytes) grouped by status and content type",
            )
            .buckets(prometheus::exponential_buckets(256.0, 2.0, 12).expect("inputs are valid")),
            &["status", "content_type"],
        )
        .expect("Infallible");
        let torii_zk_prover_latency_ms = HistogramVec::new(
            HistogramOpts::new(
                "torii_zk_prover_latency_ms",
                "Background prover processing latency (milliseconds) grouped by status",
            )
            .buckets(prometheus::exponential_buckets(5.0, 2.0, 12).expect("inputs are valid")),
            &["status"],
        )
        .expect("Infallible");
        let torii_zk_prover_gc_total = IntCounter::new(
            "torii_zk_prover_gc_total",
            "Background prover reports deleted by TTL",
        )
        .expect("Infallible");
        let torii_zk_prover_inflight = GenericGauge::new(
            "torii_zk_prover_inflight",
            "Background prover attachments currently being processed",
        )
        .expect("Infallible");
        let torii_zk_prover_pending = GenericGauge::new(
            "torii_zk_prover_pending",
            "Background prover attachments pending processing",
        )
        .expect("Infallible");
        let torii_zk_ivm_prove_inflight = GenericGauge::new(
            "torii_zk_ivm_prove_inflight",
            "Torii IVM prove helper jobs currently proving",
        )
        .expect("Infallible");
        let torii_zk_ivm_prove_queued = GenericGauge::new(
            "torii_zk_ivm_prove_queued",
            "Torii IVM prove helper jobs queued (waiting for inflight slot)",
        )
        .expect("Infallible");
        let torii_zk_prover_last_scan_bytes = GenericGauge::new(
            "torii_zk_prover_last_scan_bytes",
            "Background prover bytes processed during the most recent scan",
        )
        .expect("Infallible");
        let torii_zk_prover_last_scan_ms = GenericGauge::new(
            "torii_zk_prover_last_scan_ms",
            "Background prover wall-clock duration (ms) of the most recent scan",
        )
        .expect("Infallible");
        let torii_zk_prover_budget_exhausted_total = IntCounterVec::new(
            Opts::new(
                "torii_zk_prover_budget_exhausted_total",
                "Background prover budget exhaustion events, labelled by reason",
            ),
            &["reason"],
        )
        .expect("Infallible");

        // Snapshot-lane counters
        let torii_query_snapshot_requests = IntCounterVec::new(
            Opts::new(
                "torii_query_snapshot_requests_total",
                "Torii snapshot-lane query requests total by mode",
            ),
            &["mode"],
        )
        .expect("Infallible");
        let torii_query_snapshot_first_batch_ms = HistogramVec::new(
            HistogramOpts::new(
                "torii_query_snapshot_first_batch_ms",
                "Torii snapshot-lane first-batch latency in milliseconds by mode",
            )
            .buckets(prometheus::exponential_buckets(0.5, 2.0, 14).expect("inputs are valid")),
            &["mode"],
        )
        .expect("Infallible");
        let torii_query_snapshot_gas_consumed_units_total = IntCounterVec::new(
            Opts::new(
                "torii_query_snapshot_gas_consumed_units_total",
                "Torii snapshot-lane gas units consumed by mode",
            ),
            &["mode"],
        )
        .expect("Infallible");
        let query_snapshot_lane_first_batch_ms = HistogramVec::new(
            HistogramOpts::new(
                "query_snapshot_lane_first_batch_ms",
                "Snapshot query lane first-batch latency in milliseconds by mode",
            )
            .buckets(prometheus::exponential_buckets(0.5, 2.0, 14).expect("inputs are valid")),
            &["mode"],
        )
        .expect("Infallible");
        let query_snapshot_lane_first_batch_items = HistogramVec::new(
            HistogramOpts::new(
                "query_snapshot_lane_first_batch_items",
                "Snapshot query lane first-batch item count by mode",
            )
            .buckets(vec![
                1.0, 2.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1_000.0,
            ]),
            &["mode"],
        )
        .expect("Infallible");
        let query_snapshot_lane_remaining_items = GenericGaugeVec::new(
            Opts::new(
                "query_snapshot_lane_remaining_items",
                "Snapshot query lane remaining items by mode",
            ),
            &["mode"],
        )
        .expect("Infallible");
        let query_snapshot_lane_cursors_total = IntCounterVec::new(
            Opts::new(
                "query_snapshot_lane_cursors_total",
                "Snapshot query lane cursors emitted by mode",
            ),
            &["mode"],
        )
        .expect("Infallible");

        // Torii Connect (Iroha Connect) metrics
        let torii_connect_sessions_total = GenericGauge::new(
            "torii_connect_sessions_total",
            "Torii Connect total WS sessions",
        )
        .expect("Infallible");
        let torii_connect_sessions_active = GenericGauge::new(
            "torii_connect_sessions_active",
            "Torii Connect active session objects",
        )
        .expect("Infallible");
        let torii_pre_auth_reject_total = IntCounterVec::new(
            Opts::new(
                "torii_pre_auth_reject_total",
                "Torii pre-auth rejected connections",
            ),
            &["reason"],
        )
        .expect("Infallible");
        let torii_operator_auth_total = IntCounterVec::new(
            Opts::new(
                "torii_operator_auth_total",
                "Torii operator auth events by action, result, and reason",
            ),
            &["action", "result", "reason"],
        )
        .expect("Infallible");
        let torii_operator_auth_lockout_total = IntCounterVec::new(
            Opts::new(
                "torii_operator_auth_lockout_total",
                "Torii operator auth lockouts by action and reason",
            ),
            &["action", "reason"],
        )
        .expect("Infallible");
        let torii_signature_limit_total = IntCounter::new(
            "torii_signature_limit_total",
            "Transactions rejected during admission for exceeding the configured signature limit",
        )
        .expect("Infallible");
        let torii_signature_limit_by_authority_total = IntCounterVec::new(
            Opts::new(
                "torii_signature_limit_by_authority_total",
                "Transactions rejected during admission for exceeding the signature limit, labeled by authority type",
            ),
            &["authority"],
        )
        .expect("Infallible");
        let torii_signature_limit_last_count = GenericGauge::new(
            "torii_signature_limit_last_count",
            "Last observed transaction signature count during signature-limit enforcement",
        )
        .expect("Infallible");
        let torii_signature_limit_max = GenericGauge::new(
            "torii_signature_limit_max",
            "Configured transaction signature cap recorded during signature-limit enforcement",
        )
        .expect("Infallible");
        let torii_nts_unhealthy_reject_total = IntCounter::new(
            "torii_nts_unhealthy_reject_total",
            "Transactions rejected during admission due to unhealthy network time service",
        )
        .expect("Infallible");
        let torii_multisig_direct_sign_reject_total = IntCounter::new(
            "torii_multisig_direct_sign_reject_total",
            "Transactions rejected during admission for being signed directly by a multisig account",
        )
        .expect("Infallible");
        let torii_sorafs_admission_total = IntCounterVec::new(
            Opts::new(
                "torii_sorafs_admission_total",
                "Torii SoraFS provider admission results by outcome and reason",
            ),
            &["result", "reason"],
        )
        .expect("Infallible");
        let torii_sorafs_capacity_telemetry_rejections_total = IntCounterVec::new(
            Opts::new(
                "torii_sorafs_capacity_telemetry_rejections_total",
                "Rejected SoraFS capacity telemetry windows grouped by provider and reason",
            ),
            &["provider", "reason"],
        )
        .expect("Infallible");
        let torii_sorafs_capacity_declared_gib = GenericGaugeVec::new(
            Opts::new(
                "torii_sorafs_capacity_declared_gib",
                "Declared SoraFS capacity (GiB) per provider",
            ),
            &["provider"],
        )
        .expect("Infallible");
        let torii_sorafs_capacity_effective_gib = GenericGaugeVec::new(
            Opts::new(
                "torii_sorafs_capacity_effective_gib",
                "Effective SoraFS capacity (GiB) per provider",
            ),
            &["provider"],
        )
        .expect("Infallible");
        let torii_sorafs_capacity_utilised_gib = GenericGaugeVec::new(
            Opts::new(
                "torii_sorafs_capacity_utilised_gib",
                "Utilised SoraFS capacity (GiB) per provider",
            ),
            &["provider"],
        )
        .expect("Infallible");
        let torii_sorafs_capacity_outstanding_gib = GenericGaugeVec::new(
            Opts::new(
                "torii_sorafs_capacity_outstanding_gib",
                "Outstanding SoraFS capacity reservations (GiB) per provider",
            ),
            &["provider"],
        )
        .expect("Infallible");
        let torii_sorafs_capacity_gibhours_total = GaugeVec::new(
            Opts::new(
                "torii_sorafs_capacity_gibhours_total",
                "Accumulated SoraFS provider GiB-hours",
            ),
            &["provider"],
        )
        .expect("Infallible");
        let torii_sorafs_fee_projection_nanos = GaugeVec::new(
            Opts::new(
                "torii_sorafs_fee_projection_nanos",
                "SoraFS fee projection (nano units) per provider",
            ),
            &["provider"],
        )
        .expect("Infallible");
        let torii_sorafs_disputes_total = IntCounterVec::new(
            Opts::new(
                "torii_sorafs_disputes_total",
                "Total SoraFS capacity disputes submitted via Torii",
            ),
            &["result"],
        )
        .expect("Infallible");
        let torii_sorafs_orders_issued_total = GenericGaugeVec::new(
            Opts::new(
                "torii_sorafs_orders_issued_total",
                "SoraFS replication orders issued per provider",
            ),
            &["provider"],
        )
        .expect("Infallible");
        let torii_sorafs_orders_completed_total = GenericGaugeVec::new(
            Opts::new(
                "torii_sorafs_orders_completed_total",
                "SoraFS replication orders completed per provider",
            ),
            &["provider"],
        )
        .expect("Infallible");
        let torii_sorafs_orders_failed_total = GenericGaugeVec::new(
            Opts::new(
                "torii_sorafs_orders_failed_total",
                "SoraFS replication orders failed per provider",
            ),
            &["provider"],
        )
        .expect("Infallible");
        let torii_sorafs_outstanding_orders = GenericGaugeVec::new(
            Opts::new(
                "torii_sorafs_outstanding_orders",
                "Outstanding SoraFS replication orders per provider",
            ),
            &["provider"],
        )
        .expect("Infallible");
        let torii_sorafs_uptime_bps = IntGaugeVec::new(
            Opts::new(
                "torii_sorafs_uptime_bps",
                "SoraFS uptime success rate (basis points) per provider",
            ),
            &["provider"],
        )
        .expect("Infallible");
        let torii_sorafs_por_bps = IntGaugeVec::new(
            Opts::new(
                "torii_sorafs_por_bps",
                "SoraFS PoR success rate (basis points) per provider",
            ),
            &["provider"],
        )
        .expect("Infallible");
        let torii_sorafs_por_ingest_backlog = GenericGaugeVec::new(
            Opts::new(
                "torii_sorafs_por_ingest_backlog",
                "SoraFS PoR ingestion backlog per manifest/provider pair",
            ),
            &["manifest", "provider"],
        )
        .expect("Infallible");
        let torii_sorafs_por_ingest_failures_total = GenericGaugeVec::new(
            Opts::new(
                "torii_sorafs_por_ingest_failures_total",
                "Total SoraFS PoR ingestion failures per manifest/provider pair",
            ),
            &["manifest", "provider"],
        )
        .expect("Infallible");
        let torii_sorafs_repair_tasks_total = IntCounterVec::new(
            Opts::new(
                "torii_sorafs_repair_tasks_total",
                "SoraFS repair task transitions grouped by status",
            ),
            &["status"],
        )
        .expect("Infallible");
        let torii_sorafs_repair_latency_minutes = HistogramVec::new(
            HistogramOpts::new(
                "torii_sorafs_repair_latency_minutes",
                "SoraFS repair lifecycle latency distribution (minutes)",
            )
            .buckets(vec![
                1.0, 2.0, 5.0, 10.0, 15.0, 30.0, 60.0, 120.0, 240.0, 480.0,
            ]),
            &["outcome"],
        )
        .expect("Infallible");
        let torii_sorafs_repair_queue_depth = GenericGaugeVec::new(
            Opts::new(
                "torii_sorafs_repair_queue_depth",
                "SoraFS repair queue depth per provider",
            ),
            &["provider"],
        )
        .expect("Infallible");
        let torii_sorafs_repair_backlog_oldest_age_seconds = GenericGauge::new(
            "torii_sorafs_repair_backlog_oldest_age_seconds",
            "Age of the oldest queued SoraFS repair task (seconds)",
        )
        .expect("Infallible");
        let torii_sorafs_repair_lease_expired_total = IntCounterVec::new(
            Opts::new(
                "torii_sorafs_repair_lease_expired_total",
                "SoraFS repair lease expirations grouped by outcome",
            ),
            &["outcome"],
        )
        .expect("Infallible");
        register_guarded(&registry, &torii_sorafs_repair_tasks_total);
        register_guarded(&registry, &torii_sorafs_repair_latency_minutes);
        register_guarded(&registry, &torii_sorafs_repair_queue_depth);
        register_guarded(&registry, &torii_sorafs_repair_backlog_oldest_age_seconds);
        register_guarded(&registry, &torii_sorafs_repair_lease_expired_total);
        let torii_sorafs_slash_proposals_total = IntCounterVec::new(
            Opts::new(
                "torii_sorafs_slash_proposals_total",
                "SoraFS repair slash proposals grouped by outcome",
            ),
            &["outcome"],
        )
        .expect("Infallible");
        register_guarded(&registry, &torii_sorafs_slash_proposals_total);
        let torii_sorafs_reconciliation_runs_total = IntCounterVec::new(
            Opts::new(
                "torii_sorafs_reconciliation_runs_total",
                "SoraFS reconciliation runs grouped by result",
            ),
            &["result"],
        )
        .expect("Infallible");
        let torii_sorafs_reconciliation_divergence_count = GenericGauge::new(
            "torii_sorafs_reconciliation_divergence_count",
            "SoraFS reconciliation divergence count for the latest snapshot",
        )
        .expect("Infallible");
        register_guarded(&registry, &torii_sorafs_reconciliation_runs_total);
        register_guarded(&registry, &torii_sorafs_reconciliation_divergence_count);
        let torii_sorafs_gc_runs_total = IntCounterVec::new(
            Opts::new(
                "torii_sorafs_gc_runs_total",
                "SoraFS GC runs grouped by result",
            ),
            &["result"],
        )
        .expect("Infallible");
        let torii_sorafs_gc_evictions_total = IntCounterVec::new(
            Opts::new(
                "torii_sorafs_gc_evictions_total",
                "SoraFS GC evictions grouped by reason",
            ),
            &["reason"],
        )
        .expect("Infallible");
        let torii_sorafs_gc_bytes_freed_total = IntCounterVec::new(
            Opts::new(
                "torii_sorafs_gc_bytes_freed_total",
                "Total SoraFS GC bytes freed grouped by reason",
            ),
            &["reason"],
        )
        .expect("Infallible");
        let torii_sorafs_gc_blocked_total = IntCounterVec::new(
            Opts::new(
                "torii_sorafs_gc_blocked_total",
                "SoraFS GC evictions blocked grouped by reason",
            ),
            &["reason"],
        )
        .expect("Infallible");
        let torii_sorafs_gc_expired_manifests = GenericGauge::new(
            "torii_sorafs_gc_expired_manifests",
            "Expired manifests observed by SoraFS GC sweeps",
        )
        .expect("Infallible");
        let torii_sorafs_gc_oldest_expired_age_seconds = GenericGauge::new(
            "torii_sorafs_gc_oldest_expired_age_seconds",
            "Age of the oldest expired manifest observed by SoraFS GC (seconds)",
        )
        .expect("Infallible");
        let torii_sorafs_storage_bytes_used = GenericGaugeVec::new(
            Opts::new(
                "torii_sorafs_storage_bytes_used",
                "SoraFS storage bytes used per provider",
            ),
            &["provider"],
        )
        .expect("Infallible");
        let torii_sorafs_storage_bytes_capacity = GenericGaugeVec::new(
            Opts::new(
                "torii_sorafs_storage_bytes_capacity",
                "SoraFS storage capacity bytes per provider",
            ),
            &["provider"],
        )
        .expect("Infallible");
        let torii_sorafs_storage_pin_queue_depth = GenericGaugeVec::new(
            Opts::new(
                "torii_sorafs_storage_pin_queue_depth",
                "SoraFS pin queue depth per provider",
            ),
            &["provider"],
        )
        .expect("Infallible");
        let torii_sorafs_storage_fetch_inflight = GenericGaugeVec::new(
            Opts::new(
                "torii_sorafs_storage_fetch_inflight",
                "SoraFS fetch workers in flight per provider",
            ),
            &["provider"],
        )
        .expect("Infallible");
        let torii_sorafs_storage_fetch_bytes_per_sec = GenericGaugeVec::new(
            Opts::new(
                "torii_sorafs_storage_fetch_bytes_per_sec",
                "SoraFS fetch throughput (bytes/sec) per provider",
            ),
            &["provider"],
        )
        .expect("Infallible");
        let torii_sorafs_storage_por_inflight = GenericGaugeVec::new(
            Opts::new(
                "torii_sorafs_storage_por_inflight",
                "SoraFS PoR workers in flight per provider",
            ),
            &["provider"],
        )
        .expect("Infallible");
        let torii_sorafs_storage_por_samples_success_total = GenericGaugeVec::new(
            Opts::new(
                "torii_sorafs_storage_por_samples_success_total",
                "SoraFS PoR samples marked successful per provider",
            ),
            &["provider"],
        )
        .expect("Infallible");
        let torii_sorafs_storage_por_samples_failed_total = GenericGaugeVec::new(
            Opts::new(
                "torii_sorafs_storage_por_samples_failed_total",
                "SoraFS PoR samples marked failed per provider",
            ),
            &["provider"],
        )
        .expect("Infallible");
        let torii_sorafs_chunk_range_requests_total = IntCounterVec::new(
            Opts::new(
                "torii_sorafs_chunk_range_requests_total",
                "SoraFS chunk-range requests grouped by endpoint and status",
            ),
            &["endpoint", "status"],
        )
        .expect("Infallible");
        let torii_sorafs_chunk_range_bytes_total = IntCounterVec::new(
            Opts::new(
                "torii_sorafs_chunk_range_bytes_total",
                "Total bytes served by SoraFS chunk-range endpoints",
            ),
            &["endpoint"],
        )
        .expect("Infallible");
        let torii_sorafs_provider_range_capability_total = IntGaugeVec::new(
            Opts::new(
                "torii_sorafs_provider_range_capability_total",
                "Providers advertising SoraFS range capability grouped by feature",
            ),
            &["feature"],
        )
        .expect("Infallible");
        let torii_sorafs_range_fetch_throttle_events_total = IntCounterVec::new(
            Opts::new(
                "torii_sorafs_range_fetch_throttle_events_total",
                "SoraFS range fetch throttle events grouped by reason",
            ),
            &["reason"],
        )
        .expect("Infallible");
        let torii_sorafs_range_fetch_concurrency_current = IntGauge::with_opts(Opts::new(
            "torii_sorafs_range_fetch_concurrency_current",
            "Active SoraFS range fetch streams guarded by stream tokens",
        ))
        .expect("Infallible");
        let torii_sorafs_proof_stream_inflight = IntGaugeVec::new(
            Opts::new(
                "torii_sorafs_proof_stream_inflight",
                "Active SoraFS proof streams grouped by proof kind",
            ),
            &["kind"],
        )
        .expect("Infallible");
        let torii_sorafs_proof_stream_events_total = IntCounterVec::new(
            Opts::new(
                "torii_sorafs_proof_stream_events_total",
                "SoraFS proof stream outcomes grouped by kind, result, and reason",
            ),
            &["kind", "result", "reason"],
        )
        .expect("Infallible");
        let torii_sorafs_proof_stream_latency_ms = HistogramVec::new(
            HistogramOpts::new(
                "torii_sorafs_proof_stream_latency_ms",
                "Latency in milliseconds for SoraFS proof stream items",
            )
            .buckets(vec![
                5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0,
            ]),
            &["kind"],
        )
        .expect("Infallible");
        let torii_sorafs_proof_health_alerts_total = IntCounterVec::new(
            Opts::new(
                "torii_sorafs_proof_health_alerts_total",
                "Proof-health alerts grouped by provider, trigger, and penalty outcome",
            ),
            &["provider_id", "trigger", "penalty"],
        )
        .expect("Infallible");
        let torii_sorafs_proof_health_pdp_failures = IntGaugeVec::new(
            Opts::new(
                "torii_sorafs_proof_health_pdp_failures",
                "PDP failures recorded on the latest proof-health alert per provider",
            ),
            &["provider_id"],
        )
        .expect("Infallible");
        let torii_sorafs_proof_health_potr_breaches = IntGaugeVec::new(
            Opts::new(
                "torii_sorafs_proof_health_potr_breaches",
                "PoTR breaches recorded on the latest proof-health alert per provider",
            ),
            &["provider_id"],
        )
        .expect("Infallible");
        let torii_sorafs_proof_health_penalty_nano = GenericGaugeVec::new(
            Opts::new(
                "torii_sorafs_proof_health_penalty_nano",
                "Penalty amount applied (nano-XOR) on the latest proof-health alert per provider",
            ),
            &["provider_id"],
        )
        .expect("Infallible");
        let torii_sorafs_proof_health_window_end_epoch = GenericGaugeVec::new(
            Opts::new(
                "torii_sorafs_proof_health_window_end_epoch",
                "Telemetry window end epoch recorded on the latest proof-health alert per provider",
            ),
            &["provider_id"],
        )
        .expect("Infallible");
        let torii_sorafs_proof_health_cooldown = IntGaugeVec::new(
            Opts::new(
                "torii_sorafs_proof_health_cooldown",
                "Cooldown flag captured on the latest proof-health alert per provider (0 = inactive, 1 = active)",
            ),
            &["provider_id"],
        )
        .expect("Infallible");
        register_guarded(&registry, &torii_sorafs_chunk_range_requests_total);
        register_guarded(&registry, &torii_sorafs_chunk_range_bytes_total);
        register_guarded(&registry, &torii_sorafs_provider_range_capability_total);
        register_guarded(&registry, &torii_sorafs_range_fetch_throttle_events_total);
        register_guarded(&registry, &torii_sorafs_range_fetch_concurrency_current);
        register_guarded(&registry, &torii_sorafs_proof_stream_inflight);
        register_guarded(&registry, &torii_sorafs_proof_stream_events_total);
        register_guarded(&registry, &torii_sorafs_proof_stream_latency_ms);
        register_guarded(&registry, &torii_sorafs_proof_health_alerts_total);
        register_guarded(&registry, &torii_sorafs_proof_health_pdp_failures);
        register_guarded(&registry, &torii_sorafs_proof_health_potr_breaches);
        register_guarded(&registry, &torii_sorafs_proof_health_penalty_nano);
        register_guarded(&registry, &torii_sorafs_proof_health_window_end_epoch);
        register_guarded(&registry, &torii_sorafs_proof_health_cooldown);
        let torii_sorafs_gar_violations_total = IntCounterVec::new(
            Opts::new(
                "torii_sorafs_gar_violations_total",
                "GAR policy violations grouped by reason and detail",
            ),
            &["reason", "detail"],
        )
        .expect("Infallible");
        let torii_sorafs_gateway_refusals_total = IntCounterVec::new(
            Opts::new(
                "torii_sorafs_gateway_refusals_total",
                "Gateway refusals grouped by reason, profile, provider, and scope",
            ),
            &["reason", "profile", "provider_id", "scope"],
        )
        .expect("Infallible");
        let torii_sorafs_gateway_fixture_info = IntGaugeVec::new(
            Opts::new(
                "torii_sorafs_gateway_fixture_info",
                "Canonical SoraFS gateway fixture metadata (value = release timestamp)",
            ),
            &["version", "profile", "fixtures_digest"],
        )
        .expect("Infallible");
        let torii_sorafs_registry_manifests_total = GenericGaugeVec::new(
            Opts::new(
                "torii_sorafs_registry_manifests_total",
                "SoraFS pin registry manifests grouped by status",
            ),
            &["status"],
        )
        .expect("Infallible");
        let torii_sorafs_registry_aliases_total = GenericGauge::new(
            "torii_sorafs_registry_aliases_total",
            "Total manifest aliases recorded in the SoraFS pin registry",
        )
        .expect("Infallible");
        let torii_sorafs_alias_cache_refresh_total = IntCounterVec::new(
            Opts::new(
                "torii_sorafs_alias_cache_refresh_total",
                "Alias cache refresh outcomes grouped by result and reason",
            ),
            &["result", "reason"],
        )
        .expect("Infallible");
        let torii_sorafs_alias_cache_age_seconds = Histogram::with_opts(
            HistogramOpts::new(
                "torii_sorafs_alias_cache_age_seconds",
                "Observed age of alias proofs when served (seconds)",
            )
            .buckets(vec![
                30.0, 60.0, 120.0, 300.0, 600.0, 900.0, 1_200.0, 1_800.0, 3_600.0, 7_200.0,
            ]),
        )
        .expect("Infallible");
        let torii_sorafs_tls_cert_expiry_seconds = Gauge::with_opts(Opts::new(
            "torii_sorafs_tls_cert_expiry_seconds",
            "Seconds remaining until the active gateway TLS certificate expires",
        ))
        .expect("Infallible");
        let torii_sorafs_tls_renewal_total = IntCounterVec::new(
            Opts::new(
                "torii_sorafs_tls_renewal_total",
                "Gateway TLS renewal attempts grouped by outcome",
            ),
            &["result"],
        )
        .expect("Infallible");
        let torii_sorafs_tls_ech_enabled = IntGauge::with_opts(Opts::new(
            "torii_sorafs_tls_ech_enabled",
            "Whether ECH is enabled for the gateway (0 disabled, 1 enabled)",
        ))
        .expect("Infallible");
        let torii_sorafs_gateway_fixture_version = IntGaugeVec::new(
            Opts::new(
                "torii_sorafs_gateway_fixture_version",
                "Active SoraFS gateway fixture version",
            ),
            &["version"],
        )
        .expect("Infallible");
        register_guarded(&registry, &torii_sorafs_tls_cert_expiry_seconds);
        register_guarded(&registry, &torii_sorafs_tls_renewal_total);
        register_guarded(&registry, &torii_sorafs_tls_ech_enabled);
        register_guarded(&registry, &torii_sorafs_gateway_fixture_version);
        register_guarded(&registry, &torii_sorafs_gateway_fixture_info);
        register_guarded(&registry, &torii_sorafs_gar_violations_total);
        register_guarded(&registry, &torii_sorafs_gateway_refusals_total);
        let torii_sorafs_registry_orders_total = GenericGaugeVec::new(
            Opts::new(
                "torii_sorafs_registry_orders_total",
                "SoraFS replication orders grouped by status",
            ),
            &["status"],
        )
        .expect("Infallible");
        let torii_sorafs_replication_sla_total = GenericGaugeVec::new(
            Opts::new(
                "torii_sorafs_replication_sla_total",
                "SoraFS replication SLA outcomes (met, missed, pending)",
            ),
            &["outcome"],
        )
        .expect("Infallible");
        let torii_sorafs_replication_backlog_total = GenericGauge::new(
            "torii_sorafs_replication_backlog_total",
            "Outstanding SoraFS replication backlog (pending orders)",
        )
        .expect("Infallible");
        let torii_sorafs_replication_completion_latency_epochs = GaugeVec::new(
            Opts::new(
                "torii_sorafs_replication_completion_latency_epochs",
                "Completion latency aggregates for SoraFS replication orders (epochs)",
            ),
            &["stat"],
        )
        .expect("Infallible");
        let torii_sorafs_replication_deadline_slack_epochs = GaugeVec::new(
            Opts::new(
                "torii_sorafs_replication_deadline_slack_epochs",
                "Deadline slack aggregates for pending SoraFS replication orders (epochs)",
            ),
            &["stat"],
        )
        .expect("Infallible");
        let soranet_privacy_circuit_events_total = IntCounterVec::new(
            Opts::new(
                "soranet_privacy_circuit_events_total",
                "Aggregated SoraNet circuit outcomes keyed by relay mode and bucket start",
            ),
            &["mode", "bucket_start", "kind"],
        )
        .expect("Infallible");
        let soranet_privacy_ingest_reject_total = IntCounterVec::new(
            Opts::new(
                "soranet_privacy_ingest_reject_total",
                "Rejected SoraNet privacy ingest attempts grouped by endpoint/reason",
            ),
            &["endpoint", "reason"],
        )
        .expect("Infallible");
        let soranet_privacy_pow_rejects_total = IntCounterVec::new(
            Opts::new(
                "soranet_privacy_pow_rejects_total",
                "PoW validation failures grouped by relay mode, bucket start, and reason",
            ),
            &["mode", "bucket_start", "reason"],
        )
        .expect("Infallible");
        let soranet_pow_revocation_store_total = IntCounterVec::new(
            Opts::new(
                "soranet_pow_revocation_store_total",
                "SoraNet PoW revocation store fallbacks grouped by reason",
            ),
            &["reason"],
        )
        .expect("Infallible");
        let soranet_privacy_throttles_total = IntCounterVec::new(
            Opts::new(
                "soranet_privacy_throttles_total",
                "Aggregated SoraNet throttling events keyed by relay mode and bucket start",
            ),
            &["mode", "bucket_start", "scope"],
        )
        .expect("Infallible");
        let soranet_privacy_verified_bytes_total = IntCounterVec::new(
            Opts::new(
                "soranet_privacy_verified_bytes_total",
                "Aggregated verified byte totals emitted per relay mode and bucket start",
            ),
            &["mode", "bucket_start"],
        )
        .expect("Infallible");
        let soranet_privacy_active_circuits_avg = GaugeVec::new(
            Opts::new(
                "soranet_privacy_active_circuits_avg",
                "Average concurrent SoraNet circuits per bucket",
            ),
            &["mode", "bucket_start"],
        )
        .expect("Infallible");
        let soranet_privacy_active_circuits_max = GaugeVec::new(
            Opts::new(
                "soranet_privacy_active_circuits_max",
                "Maximum concurrent SoraNet circuits per bucket",
            ),
            &["mode", "bucket_start"],
        )
        .expect("Infallible");
        let soranet_privacy_open_buckets = GaugeVec::new(
            Opts::new(
                "soranet_privacy_open_buckets",
                "Open SoraNet privacy buckets still accumulating contributors",
            ),
            &["mode"],
        )
        .expect("Infallible");
        let soranet_privacy_pending_collectors = GaugeVec::new(
            Opts::new(
                "soranet_privacy_pending_collectors",
                "Pending SoraNet privacy collector share accumulators per mode",
            ),
            &["mode"],
        )
        .expect("Infallible");
        let soranet_privacy_snapshot_suppressed = GaugeVec::new(
            Opts::new(
                "soranet_privacy_snapshot_suppressed",
                "Suppressed SoraNet privacy buckets observed in the latest drain (per reason)",
            ),
            &["reason"],
        )
        .expect("Infallible");
        let soranet_privacy_snapshot_suppressed_by_mode = GaugeVec::new(
            Opts::new(
                "soranet_privacy_snapshot_suppressed_by_mode",
                "Suppressed SoraNet privacy buckets observed in the latest drain (per mode and reason)",
            ),
            &["mode", "reason"],
        )
        .expect("Infallible");
        let soranet_privacy_snapshot_drained = IntGauge::new(
            "soranet_privacy_snapshot_drained",
            "Buckets drained during the latest privacy collector flush",
        )
        .expect("Infallible");
        let soranet_privacy_snapshot_suppression_ratio = Gauge::new(
            "soranet_privacy_snapshot_suppression_ratio",
            "Ratio of suppressed to drained buckets observed in the latest privacy flush",
        )
        .expect("Infallible");
        let soranet_privacy_evicted_buckets_total = IntCounter::new(
            "soranet_privacy_evicted_buckets_total",
            "Completed SoraNet privacy buckets evicted due to retention limits",
        )
        .expect("Infallible");
        let soranet_privacy_bucket_suppressed = GaugeVec::new(
            Opts::new(
                "soranet_privacy_bucket_suppressed",
                "Buckets withheld due to insufficient contributors",
            ),
            &["mode", "bucket_start"],
        )
        .expect("Infallible");
        let soranet_privacy_suppression_total = IntCounterVec::new(
            Opts::new(
                "soranet_privacy_suppression_total",
                "Suppressed bucket counts grouped by mode and reason",
            ),
            &["mode", "reason"],
        )
        .expect("Infallible");
        let soranet_privacy_rtt_millis = GaugeVec::new(
            Opts::new(
                "soranet_privacy_rtt_millis",
                "SoraNet RTT percentile estimates per bucket",
            ),
            &["mode", "bucket_start", "percentile"],
        )
        .expect("Infallible");
        let soranet_privacy_gar_reports_total = IntCounterVec::new(
            Opts::new(
                "soranet_privacy_gar_reports_total",
                "Aggregated SoraNet GAR abuse reports keyed by hashed category",
            ),
            &["mode", "bucket_start", "category_hash"],
        )
        .expect("Infallible");
        let soranet_privacy_last_poll_unixtime = IntGauge::new(
            "soranet_privacy_last_poll_unixtime",
            "Unix timestamp of the last successful SoraNet privacy poll",
        )
        .expect("Infallible");
        let soranet_privacy_poll_errors_total = IntCounterVec::new(
            Opts::new(
                "soranet_privacy_poll_errors_total",
                "Privacy polling failures grouped by provider alias",
            ),
            &["provider"],
        )
        .expect("Infallible");
        let soranet_privacy_collector_enabled = IntGauge::new(
            "soranet_privacy_collector_enabled",
            "Flag indicating whether the privacy collector is active (1) or disabled (0)",
        )
        .expect("Infallible");
        let sorafs_orchestrator_active_fetches = IntGaugeVec::new(
            Opts::new(
                "sorafs_orchestrator_active_fetches",
                "Active multi-source orchestrator fetches grouped by manifest and region",
            ),
            &["manifest_id", "region"],
        )
        .expect("Infallible");
        let sorafs_orchestrator_fetch_duration_ms = HistogramVec::new(
            HistogramOpts::new(
                "sorafs_orchestrator_fetch_duration_ms",
                "Multi-source orchestrator fetch duration (milliseconds)",
            )
            .buckets(prometheus::exponential_buckets(10.0, 1.8, 12).expect("valid buckets")),
            &["manifest_id", "region"],
        )
        .expect("Infallible");
        let sorafs_orchestrator_fetch_failures_total = IntCounterVec::new(
            Opts::new(
                "sorafs_orchestrator_fetch_failures_total",
                "Multi-source orchestrator fetch failures grouped by manifest, region, and reason",
            ),
            &["manifest_id", "region", "reason"],
        )
        .expect("Infallible");
        let sorafs_orchestrator_retries_total = IntCounterVec::new(
            Opts::new(
                "sorafs_orchestrator_retries_total",
                "Multi-source orchestrator retries grouped by manifest, provider, and reason",
            ),
            &["manifest_id", "provider_id", "reason"],
        )
        .expect("Infallible");
        let sorafs_orchestrator_provider_failures_total = IntCounterVec::new(
            Opts::new(
                "sorafs_orchestrator_provider_failures_total",
                "Multi-source orchestrator provider failures grouped by manifest, provider, and reason",
            ),
            &["manifest_id", "provider_id", "reason"],
        )
        .expect("Infallible");
        let sorafs_orchestrator_chunk_latency_ms = HistogramVec::new(
            HistogramOpts::new(
                "sorafs_orchestrator_chunk_latency_ms",
                "Latency per chunk fetch served by the orchestrator (milliseconds)",
            )
            .buckets(prometheus::exponential_buckets(5.0, 1.7, 16).expect("valid buckets")),
            &["manifest_id", "provider_id"],
        )
        .expect("Infallible");
        let sorafs_orchestrator_bytes_total = IntCounterVec::new(
            Opts::new(
                "sorafs_orchestrator_bytes_total",
                "Total bytes delivered by the orchestrator grouped by manifest and provider",
            ),
            &["manifest_id", "provider_id"],
        )
        .expect("Infallible");
        let sorafs_orchestrator_stalls_total = IntCounterVec::new(
            Opts::new(
                "sorafs_orchestrator_stalls_total",
                "Count of orchestrator chunks exceeding the configured latency cap",
            ),
            &["manifest_id", "provider_id"],
        )
        .expect("Infallible");
        let sorafs_orchestrator_transport_events_total = IntCounterVec::new(
            Opts::new(
                "sorafs_orchestrator_transport_events_total",
                "Transport events emitted by the orchestrator grouped by region, protocol, event, and reason",
            ),
            &["region", "protocol", "event", "reason"],
        )
        .expect("Infallible");
        let sorafs_orchestrator_policy_events_total = IntCounterVec::new(
            Opts::new(
                "sorafs_orchestrator_policy_events_total",
                "SoraFS anonymity policy outcomes grouped by region, stage, outcome, and reason",
            ),
            &["region", "stage", "outcome", "reason"],
        )
        .expect("Infallible");
        let sorafs_orchestrator_pq_ratio = HistogramVec::new(
            HistogramOpts::new(
                "sorafs_orchestrator_pq_ratio",
                "Distribution of PQ-capable relay selection ratios per region and stage",
            )
            .buckets(vec![0.0, 0.25, 0.5, 0.66, 0.75, 1.0]),
            &["region", "stage"],
        )
        .expect("Infallible");
        let sorafs_orchestrator_pq_candidate_ratio = HistogramVec::new(
            HistogramOpts::new(
                "sorafs_orchestrator_pq_candidate_ratio",
                "Distribution of PQ-capable relay supply ratios per region and stage",
            )
            .buckets(vec![0.0, 0.25, 0.5, 0.75, 1.0]),
            &["region", "stage"],
        )
        .expect("Infallible");
        let sorafs_orchestrator_pq_deficit_ratio = HistogramVec::new(
            HistogramOpts::new(
                "sorafs_orchestrator_pq_deficit_ratio",
                "Distribution of PQ policy shortfall ratios per region and stage",
            )
            .buckets(vec![0.0, 0.1, 0.25, 0.5, 0.75, 1.0]),
            &["region", "stage"],
        )
        .expect("Infallible");
        let sorafs_orchestrator_classical_ratio = HistogramVec::new(
            HistogramOpts::new(
                "sorafs_orchestrator_classical_ratio",
                "Distribution of classical relay selection ratios per region and stage",
            )
            .buckets(vec![0.0, 0.25, 0.5, 0.75, 1.0]),
            &["region", "stage"],
        )
        .expect("Infallible");
        let sorafs_orchestrator_classical_selected = HistogramVec::new(
            HistogramOpts::new(
                "sorafs_orchestrator_classical_selected",
                "Distribution of classical relay counts selected per region and stage",
            )
            .buckets(vec![0.0, 1.0, 2.0, 3.0, 4.0, 8.0, 16.0]),
            &["region", "stage"],
        )
        .expect("Infallible");
        let torii_da_rent_gib_months_total = IntCounterVec::new(
            Opts::new(
                "torii_da_rent_gib_months_total",
                "Aggregate GiB-month usage quoted by DA ingest grouped by cluster and storage class",
            ),
            &["cluster", "storage_class"],
        )
        .expect("Infallible");
        let torii_da_rent_base_micro_total = CounterVec::new(
            Opts::new(
                "torii_da_rent_base_micro_total",
                "Aggregate base rent (micro XOR) quoted by DA ingest grouped by cluster and storage class",
            ),
            &["cluster", "storage_class"],
        )
        .expect("Infallible");
        let torii_da_protocol_reserve_micro_total = CounterVec::new(
            Opts::new(
                "torii_da_protocol_reserve_micro_total",
                "Aggregate protocol reserve (micro XOR) quoted by DA ingest grouped by cluster and storage class",
            ),
            &["cluster", "storage_class"],
        )
        .expect("Infallible");
        let torii_da_provider_reward_micro_total = CounterVec::new(
            Opts::new(
                "torii_da_provider_reward_micro_total",
                "Aggregate provider rewards (micro XOR) quoted by DA ingest grouped by cluster and storage class",
            ),
            &["cluster", "storage_class"],
        )
        .expect("Infallible");
        let torii_da_pdp_bonus_micro_total = CounterVec::new(
            Opts::new(
                "torii_da_pdp_bonus_micro_total",
                "Aggregate PDP bonuses (micro XOR) quoted by DA ingest grouped by cluster and storage class",
            ),
            &["cluster", "storage_class"],
        )
        .expect("Infallible");
        let torii_da_potr_bonus_micro_total = CounterVec::new(
            Opts::new(
                "torii_da_potr_bonus_micro_total",
                "Aggregate PoTR bonuses (micro XOR) quoted by DA ingest grouped by cluster and storage class",
            ),
            &["cluster", "storage_class"],
        )
        .expect("Infallible");
        let torii_da_receipts_total = IntCounterVec::new(
            Opts::new(
                "torii_da_receipts_total",
                "DA receipt ingest outcomes grouped by lane and epoch",
            ),
            &["outcome", "lane", "epoch"],
        )
        .expect("Infallible");
        let torii_da_receipt_highest_sequence = GenericGaugeVec::new(
            Opts::new(
                "torii_da_receipt_highest_sequence",
                "Highest DA receipt sequence observed per lane and epoch",
            ),
            &["lane", "epoch"],
        )
        .expect("Infallible");
        let torii_da_chunking_seconds = Histogram::with_opts(
            HistogramOpts::new(
                "torii_da_chunking_seconds",
                "DA chunking and erasure coding duration (seconds)",
            )
            .buckets(vec![
                0.001, 0.005, 0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
            ]),
        )
        .expect("Infallible");
        let da_shard_cursor_events_total = IntCounterVec::new(
            Opts::new(
                "da_shard_cursor_events_total",
                "DA shard cursor events grouped by outcome, lane, and shard",
            ),
            &["event", "lane", "shard"],
        )
        .expect("Infallible");
        let da_shard_cursor_height = IntGaugeVec::new(
            Opts::new(
                "da_shard_cursor_height",
                "Latest block height recorded per DA shard cursor entry",
            ),
            &["lane", "shard"],
        )
        .expect("Infallible");
        let da_shard_cursor_lag_blocks = IntGaugeVec::new(
            Opts::new(
                "da_shard_cursor_lag_blocks",
                "Lag in blocks between the validated height and the last DA shard cursor advance",
            ),
            &["lane", "shard"],
        )
        .expect("Infallible");
        let taikai_ingest_segment_latency_ms = HistogramVec::new(
            HistogramOpts::new(
                "taikai_ingest_segment_latency_ms",
                "Encoder-to-ingest latency per Taikai stream (milliseconds)",
            )
            .buckets(vec![
                10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1_000.0, 2_000.0, 4_000.0,
            ]),
            &["cluster", "stream"],
        )
        .expect("Infallible");
        let taikai_ingest_live_edge_drift_ms = HistogramVec::new(
            HistogramOpts::new(
                "taikai_ingest_live_edge_drift_ms",
                "Absolute live-edge drift per Taikai stream (milliseconds)",
            )
            .buckets(vec![
                50.0, 100.0, 250.0, 500.0, 1_000.0, 1_500.0, 2_000.0, 3_000.0,
            ]),
            &["cluster", "stream"],
        )
        .expect("Infallible");
        let taikai_ingest_live_edge_drift_signed_ms = GaugeVec::new(
            Opts::new(
                "taikai_ingest_live_edge_drift_signed_ms",
                "Signed live-edge drift per Taikai stream (milliseconds; negative = ahead)",
            ),
            &["cluster", "stream"],
        )
        .expect("Infallible");
        let taikai_ingest_errors_total = IntCounterVec::new(
            Opts::new(
                "taikai_ingest_errors_total",
                "Taikai ingest failures grouped by reason",
            ),
            &["cluster", "stream", "reason"],
        )
        .expect("Infallible");
        let taikai_trm_alias_rotations_total = IntCounterVec::new(
            Opts::new(
                "taikai_trm_alias_rotations_total",
                "Taikai routing manifest alias rotations grouped by cluster/event/stream/alias",
            ),
            &[
                "cluster",
                "event",
                "stream",
                "alias_namespace",
                "alias_name",
            ],
        )
        .expect("Infallible");
        let taikai_viewer_rebuffer_events_total = IntCounterVec::new(
            Opts::new(
                "taikai_viewer_rebuffer_events_total",
                "Taikai viewer rebuffer events grouped by cluster and stream",
            ),
            &["cluster", "stream"],
        )
        .expect("Infallible");
        let taikai_viewer_playback_segments_total = IntCounterVec::new(
            Opts::new(
                "taikai_viewer_playback_segments_total",
                "Taikai viewer playback segments grouped by cluster and stream",
            ),
            &["cluster", "stream"],
        )
        .expect("Infallible");
        let taikai_viewer_cek_fetch_duration_ms = HistogramVec::new(
            HistogramOpts::new(
                "taikai_viewer_cek_fetch_duration_ms",
                "CEK fetch duration per cluster and lane (milliseconds)",
            )
            .buckets(vec![5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0]),
            &["cluster", "lane"],
        )
        .expect("Infallible");
        let taikai_viewer_pq_circuit_health = GaugeVec::new(
            Opts::new(
                "taikai_viewer_pq_circuit_health",
                "PQ circuit health percentage per cluster",
            ),
            &["cluster"],
        )
        .expect("Infallible");
        let taikai_viewer_cek_rotation_seconds_ago = GenericGaugeVec::new(
            Opts::new(
                "taikai_viewer_cek_rotation_seconds_ago",
                "Seconds since last CEK rotation per lane",
            ),
            &["lane"],
        )
        .expect("Infallible");
        let taikai_viewer_alerts_firing_total = IntCounterVec::new(
            Opts::new(
                "taikai_viewer_alerts_firing_total",
                "Counts of Taikai viewer alerts firing grouped by cluster/alert name",
            ),
            &["cluster", "alertname"],
        )
        .expect("Infallible");
        let sorafs_taikai_cache_query_total = IntCounterVec::new(
            Opts::new(
                "sorafs_taikai_cache_query_total",
                "Taikai cache query outcomes grouped by result and tier",
            ),
            &["result", "tier"],
        )
        .expect("Infallible");
        let sorafs_taikai_cache_insert_total = IntCounterVec::new(
            Opts::new(
                "sorafs_taikai_cache_insert_total",
                "Taikai cache insert events grouped by tier",
            ),
            &["tier"],
        )
        .expect("Infallible");
        let sorafs_taikai_cache_evictions_total = IntCounterVec::new(
            Opts::new(
                "sorafs_taikai_cache_evictions_total",
                "Taikai cache evictions grouped by tier and reason",
            ),
            &["tier", "reason"],
        )
        .expect("Infallible");
        let sorafs_taikai_cache_promotions_total = IntCounterVec::new(
            Opts::new(
                "sorafs_taikai_cache_promotions_total",
                "Taikai cache promotions grouped by source and target tiers",
            ),
            &["from_tier", "to_tier"],
        )
        .expect("Infallible");
        let sorafs_taikai_cache_bytes_total = IntCounterVec::new(
            Opts::new(
                "sorafs_taikai_cache_bytes_total",
                "Bytes processed by the Taikai cache grouped by event and tier",
            ),
            &["event", "tier"],
        )
        .expect("Infallible");
        let sorafs_taikai_qos_denied_total = IntCounterVec::new(
            Opts::new(
                "sorafs_taikai_qos_denied_total",
                "Taikai QoS denials grouped by class",
            ),
            &["class"],
        )
        .expect("Infallible");
        let sorafs_taikai_queue_events_total = IntCounterVec::new(
            Opts::new(
                "sorafs_taikai_queue_events_total",
                "Taikai queue events grouped by event/class",
            ),
            &["event", "class"],
        )
        .expect("Infallible");
        let sorafs_taikai_queue_depth = IntGaugeVec::new(
            Opts::new(
                "sorafs_taikai_queue_depth",
                "Current Taikai queue depth grouped by state",
            ),
            &["state"],
        )
        .expect("Infallible");
        let sorafs_taikai_shard_failovers_total = IntCounterVec::new(
            Opts::new(
                "sorafs_taikai_shard_failovers_total",
                "Taikai shard failovers grouped by preferred and selected shard",
            ),
            &["preferred_shard", "selected_shard"],
        )
        .expect("Infallible");
        let sorafs_taikai_shard_circuits_open = IntGaugeVec::new(
            Opts::new(
                "sorafs_taikai_shard_circuits_open",
                "Open circuit breakers per Taikai shard in the Taikai pull queue",
            ),
            &["shard"],
        )
        .expect("Infallible");
        let sorafs_orchestrator_brownouts_total = IntCounterVec::new(
            Opts::new(
                "sorafs_orchestrator_brownouts_total",
                "Anonymity policy brownout events grouped by region, stage, and reason",
            ),
            &["region", "stage", "reason"],
        )
        .expect("Infallible");
        let soranet_reward_base_payout_nanos = GenericGauge::new(
            "soranet_reward_base_payout_nanos",
            "Configured SoraNet base payout (nano XOR) applied per epoch",
        )
        .expect("Infallible");
        soranet_reward_base_payout_nanos.set(0);
        let soranet_reward_events_total = IntCounterVec::new(
            Opts::new(
                "soranet_reward_events_total",
                "SoraNet relay reward events grouped by relay and result",
            ),
            &["relay", "result"],
        )
        .expect("Infallible");
        let soranet_reward_payout_nanos_total = IntCounterVec::new(
            Opts::new(
                "soranet_reward_payout_nanos_total",
                "Aggregated SoraNet payout volume (nano XOR) grouped by relay and result",
            ),
            &["relay", "result"],
        )
        .expect("Infallible");
        let soranet_reward_skips_total = IntCounterVec::new(
            Opts::new(
                "soranet_reward_skips_total",
                "SoraNet relay reward skips grouped by relay and reason",
            ),
            &["relay", "reason"],
        )
        .expect("Infallible");
        let soranet_reward_adjustment_nanos_total = IntCounterVec::new(
            Opts::new(
                "soranet_reward_adjustment_nanos_total",
                "Aggregated SoraNet dispute adjustments (nano XOR) grouped by relay and kind",
            ),
            &["relay", "kind"],
        )
        .expect("Infallible");
        let soranet_reward_disputes_total = IntCounterVec::new(
            Opts::new(
                "soranet_reward_disputes_total",
                "SoraNet reward dispute lifecycle counters grouped by action",
            ),
            &["action"],
        )
        .expect("Infallible");
        register!(
            registry,
            soranet_privacy_ingest_reject_total,
            soranet_privacy_circuit_events_total,
            soranet_privacy_pow_rejects_total,
            soranet_pow_revocation_store_total,
            soranet_privacy_throttles_total,
            soranet_privacy_verified_bytes_total,
            soranet_privacy_evicted_buckets_total,
            soranet_privacy_suppression_total,
            soranet_privacy_gar_reports_total,
            soranet_privacy_poll_errors_total
        );
        register_guarded(&registry, &soranet_privacy_last_poll_unixtime);
        register_guarded(&registry, &soranet_privacy_active_circuits_avg);
        register_guarded(&registry, &soranet_privacy_active_circuits_max);
        register_guarded(&registry, &soranet_privacy_open_buckets);
        register_guarded(&registry, &soranet_privacy_pending_collectors);
        register_guarded(&registry, &soranet_privacy_snapshot_suppressed);
        register_guarded(&registry, &soranet_privacy_snapshot_suppressed_by_mode);
        register_guarded(&registry, &soranet_privacy_snapshot_drained);
        register_guarded(&registry, &soranet_privacy_snapshot_suppression_ratio);
        register_guarded(&registry, &soranet_privacy_bucket_suppressed);
        register_guarded(&registry, &soranet_privacy_rtt_millis);
        register_guarded(&registry, &soranet_privacy_collector_enabled);
        register_guarded(&registry, &sorafs_orchestrator_active_fetches);
        register_guarded(&registry, &sorafs_orchestrator_fetch_duration_ms);
        register_guarded(&registry, &sorafs_orchestrator_fetch_failures_total);
        register_guarded(&registry, &sorafs_orchestrator_retries_total);
        register_guarded(&registry, &sorafs_orchestrator_provider_failures_total);
        register_guarded(&registry, &sorafs_orchestrator_chunk_latency_ms);
        register_guarded(&registry, &sorafs_orchestrator_bytes_total);
        register_guarded(&registry, &sorafs_orchestrator_stalls_total);
        register_guarded(&registry, &sorafs_orchestrator_transport_events_total);
        register_guarded(&registry, &sorafs_orchestrator_policy_events_total);
        register_guarded(&registry, &sorafs_orchestrator_pq_ratio);
        register_guarded(&registry, &sorafs_orchestrator_pq_candidate_ratio);
        register_guarded(&registry, &sorafs_orchestrator_pq_deficit_ratio);
        register_guarded(&registry, &sorafs_orchestrator_classical_ratio);
        register_guarded(&registry, &sorafs_orchestrator_classical_selected);
        register_guarded(&registry, &torii_da_rent_gib_months_total);
        register_guarded(&registry, &torii_da_rent_base_micro_total);
        register_guarded(&registry, &torii_da_protocol_reserve_micro_total);
        register_guarded(&registry, &torii_da_provider_reward_micro_total);
        register_guarded(&registry, &torii_da_pdp_bonus_micro_total);
        register_guarded(&registry, &torii_da_potr_bonus_micro_total);
        register_guarded(&registry, &torii_da_receipts_total);
        register_guarded(&registry, &torii_da_receipt_highest_sequence);
        register_guarded(&registry, &torii_da_chunking_seconds);
        register_guarded(&registry, &da_shard_cursor_events_total);
        register_guarded(&registry, &da_shard_cursor_height);
        register_guarded(&registry, &da_shard_cursor_lag_blocks);
        register!(
            registry,
            subscription_billing_attempts_total,
            subscription_billing_outcomes_total,
            offline_transfer_events_total,
            offline_transfer_receipts_total,
            offline_transfer_settled_amount,
            offline_transfer_rejections_total,
            offline_transfer_pruned_total,
            offline_attestation_policy_total,
            offline_app_attest_signature_compat_total,
            social_events_total,
            social_budget_spent,
            social_campaign_spent,
            social_campaign_cap,
            social_campaign_remaining,
            social_campaign_active,
            social_halted,
            social_rejections_total,
            multisig_direct_sign_reject_total,
            social_open_escrows
        );
        register_guarded(&registry, &taikai_ingest_segment_latency_ms);
        register_guarded(&registry, &taikai_ingest_live_edge_drift_ms);
        register_guarded(&registry, &taikai_ingest_live_edge_drift_signed_ms);
        register_guarded(&registry, &taikai_ingest_errors_total);
        register_guarded(&registry, &taikai_trm_alias_rotations_total);
        register_guarded(&registry, &taikai_viewer_rebuffer_events_total);
        register_guarded(&registry, &taikai_viewer_playback_segments_total);
        register_guarded(&registry, &taikai_viewer_cek_fetch_duration_ms);
        register_guarded(&registry, &taikai_viewer_pq_circuit_health);
        register_guarded(&registry, &taikai_viewer_cek_rotation_seconds_ago);
        register_guarded(&registry, &taikai_viewer_alerts_firing_total);
        register_guarded(&registry, &sorafs_taikai_cache_query_total);
        register_guarded(&registry, &sorafs_taikai_cache_insert_total);
        register_guarded(&registry, &sorafs_taikai_cache_evictions_total);
        register_guarded(&registry, &sorafs_taikai_cache_promotions_total);
        register_guarded(&registry, &sorafs_taikai_cache_bytes_total);
        register_guarded(&registry, &sorafs_taikai_qos_denied_total);
        register_guarded(&registry, &sorafs_taikai_queue_events_total);
        register_guarded(&registry, &sorafs_taikai_queue_depth);
        register_guarded(&registry, &sorafs_taikai_shard_failovers_total);
        register_guarded(&registry, &sorafs_taikai_shard_circuits_open);
        register_guarded(&registry, &sorafs_orchestrator_brownouts_total);
        register_guarded(&registry, &soranet_reward_base_payout_nanos);
        register_guarded(&registry, &soranet_reward_events_total);
        register_guarded(&registry, &soranet_reward_payout_nanos_total);
        register_guarded(&registry, &soranet_reward_skips_total);
        register_guarded(&registry, &soranet_reward_adjustment_nanos_total);
        register_guarded(&registry, &soranet_reward_disputes_total);
        let torii_http_requests_total = IntCounterVec::new(
            Opts::new(
                "torii_http_requests_total",
                "Torii HTTP requests grouped by response content type, method, and status",
            ),
            &["content_type", "method", "status"],
        )
        .expect("Infallible");
        let torii_http_request_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "torii_http_request_duration_seconds",
                "Torii HTTP request latency (seconds) grouped by content type and method",
            )
            .buckets(prometheus::exponential_buckets(0.005, 2.0, 13).expect("inputs are valid")),
            &["content_type", "method"],
        )
        .expect("Infallible");
        let torii_http_response_bytes_total = IntCounterVec::new(
            Opts::new(
                "torii_http_response_bytes_total",
                "Torii HTTP response payload size (bytes) grouped by content type, method, and status",
            ),
            &["content_type", "method", "status"],
        )
        .expect("Infallible");
        let torii_api_version_negotiated_total = IntCounterVec::new(
            Opts::new(
                "torii_api_version_negotiated_total",
                "Torii API version negotiation outcomes grouped by result and version",
            ),
            &["result", "version"],
        )
        .expect("Infallible");
        let torii_content_requests_total = IntCounterVec::new(
            Opts::new(
                "torii_content_requests_total",
                "Content gateway requests grouped by outcome",
            ),
            &["outcome"],
        )
        .expect("Infallible");
        let torii_content_request_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "torii_content_request_duration_seconds",
                "Content gateway latency (seconds) grouped by outcome",
            )
            .buckets(prometheus::exponential_buckets(0.005, 2.0, 13).expect("inputs are valid")),
            &["outcome"],
        )
        .expect("Infallible");
        let torii_content_response_bytes_total = IntCounterVec::new(
            Opts::new(
                "torii_content_response_bytes_total",
                "Content gateway response payload size (bytes) grouped by outcome",
            ),
            &["outcome"],
        )
        .expect("Infallible");
        let torii_proof_requests_total = IntCounterVec::new(
            Opts::new(
                "torii_proof_requests_total",
                "Proof endpoint requests grouped by endpoint and outcome",
            ),
            &["endpoint", "outcome"],
        )
        .expect("Infallible");
        let torii_proof_request_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "torii_proof_request_duration_seconds",
                "Proof endpoint latency (seconds) grouped by endpoint and outcome",
            )
            .buckets(prometheus::exponential_buckets(0.001, 2.0, 12).expect("inputs are valid")),
            &["endpoint", "outcome"],
        )
        .expect("Infallible");
        let torii_proof_response_bytes_total = IntCounterVec::new(
            Opts::new(
                "torii_proof_response_bytes_total",
                "Proof endpoint response payload size (bytes) grouped by endpoint and outcome",
            ),
            &["endpoint", "outcome"],
        )
        .expect("Infallible");
        let torii_proof_cache_hits_total = IntCounterVec::new(
            Opts::new(
                "torii_proof_cache_hits_total",
                "Proof endpoint cache hits grouped by endpoint",
            ),
            &["endpoint"],
        )
        .expect("Infallible");
        let torii_request_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "torii_request_duration_seconds",
                "Torii request latency (seconds) grouped by connection scheme",
            )
            .buckets(prometheus::exponential_buckets(0.005, 2.0, 13).expect("inputs are valid")),
            &["scheme"],
        )
        .expect("Infallible");
        let torii_request_failures_total = IntCounterVec::new(
            Opts::new(
                "torii_request_failures_total",
                "Torii request failures grouped by connection scheme and status code",
            ),
            &["scheme", "code"],
        )
        .expect("Infallible");
        let torii_explorer_requests_total = IntCounterVec::new(
            Opts::new(
                "torii_explorer_requests_total",
                "Explorer endpoint requests grouped by endpoint and outcome",
            ),
            &["endpoint", "outcome"],
        )
        .expect("Infallible");
        let torii_explorer_request_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "torii_explorer_request_duration_seconds",
                "Explorer endpoint latency (seconds) grouped by endpoint and outcome",
            )
            .buckets(prometheus::exponential_buckets(0.001, 2.0, 14).expect("inputs are valid")),
            &["endpoint", "outcome"],
        )
        .expect("Infallible");
        let torii_norito_rpc_gate_total = IntCounterVec::new(
            Opts::new(
                "torii_norito_rpc_gate_total",
                "Torii Norito-RPC gate decisions grouped by rollout stage and outcome",
            ),
            &["stage", "outcome"],
        )
        .expect("Infallible");
        let torii_address_invalid_total = IntCounterVec::new(
            Opts::new(
                "torii_address_invalid_total",
                "Torii account identifier rejects grouped by endpoint and reason",
            ),
            &["endpoint", "reason"],
        )
        .expect("Infallible");
        let torii_address_domain_total = IntCounterVec::new(
            Opts::new(
                "torii_address_domain_total",
                "Torii account domain kinds grouped by endpoint and selector type",
            ),
            &["endpoint", "domain_kind"],
        )
        .expect("Infallible");
        let torii_address_collision_total = IntCounterVec::new(
            Opts::new(
                "torii_address_collision_total",
                "Torii Local-12 selector collisions grouped by endpoint and kind",
            ),
            &["endpoint", "kind"],
        )
        .expect("Infallible");
        let torii_address_collision_domain_total = IntCounterVec::new(
            Opts::new(
                "torii_address_collision_domain_total",
                "Torii Local-12 selector collisions grouped by endpoint and domain label",
            ),
            &["endpoint", "domain"],
        )
        .expect("Infallible");
        let torii_account_literal_total = IntCounterVec::new(
            Opts::new(
                "torii_account_literal_total",
                "Torii account literal selections grouped by endpoint and format",
            ),
            &["endpoint", "format"],
        )
        .expect("Infallible");
        let torii_norito_decode_failures_total = IntCounterVec::new(
            Opts::new(
                "torii_norito_decode_failures_total",
                "Torii Norito RPC decode failures grouped by payload kind and reason",
            ),
            &["payload_kind", "reason"],
        )
        .expect("Infallible");
        register_guarded(&registry, &torii_http_requests_total);
        register_guarded(&registry, &torii_http_request_duration_seconds);
        register_guarded(&registry, &torii_http_response_bytes_total);
        register_guarded(&registry, &torii_api_version_negotiated_total);
        register_guarded(&registry, &torii_content_requests_total);
        register_guarded(&registry, &torii_content_request_duration_seconds);
        register_guarded(&registry, &torii_content_response_bytes_total);
        register_guarded(&registry, &torii_proof_requests_total);
        register_guarded(&registry, &torii_proof_request_duration_seconds);
        register_guarded(&registry, &torii_proof_response_bytes_total);
        register_guarded(&registry, &torii_proof_cache_hits_total);
        register_guarded(&registry, &torii_request_duration_seconds);
        register_guarded(&registry, &torii_request_failures_total);
        register_guarded(&registry, &torii_explorer_requests_total);
        register_guarded(&registry, &torii_explorer_request_duration_seconds);
        register_guarded(&registry, &torii_norito_rpc_gate_total);
        register_guarded(&registry, &torii_address_invalid_total);
        register_guarded(&registry, &torii_address_domain_total);
        register_guarded(&registry, &torii_address_collision_total);
        register_guarded(&registry, &torii_address_collision_domain_total);
        register_guarded(&registry, &torii_account_literal_total);
        register_guarded(&registry, &torii_norito_decode_failures_total);
        register_guarded(&registry, &torii_connect_sessions_total);
        register_guarded(&registry, &torii_connect_sessions_active);
        register_guarded(&registry, &torii_pre_auth_reject_total);
        register_guarded(&registry, &torii_operator_auth_total);
        register_guarded(&registry, &torii_operator_auth_lockout_total);
        register_guarded(&registry, &torii_signature_limit_total);
        register_guarded(&registry, &torii_signature_limit_by_authority_total);
        register_guarded(&registry, &torii_signature_limit_last_count);
        register_guarded(&registry, &torii_signature_limit_max);
        register_guarded(&registry, &torii_nts_unhealthy_reject_total);
        register_guarded(&registry, &torii_multisig_direct_sign_reject_total);
        let torii_proof_throttled_total = IntCounterVec::new(
            Opts::new(
                "torii_proof_throttled_total",
                "Torii proof endpoints throttled by rate limiter",
            ),
            &["endpoint"],
        )
        .expect("Infallible");
        let torii_contract_throttled_total = IntCounterVec::new(
            Opts::new(
                "torii_contract_throttled_total",
                "Torii contract endpoints throttled by rate limiter",
            ),
            &["endpoint"],
        )
        .expect("Infallible");
        let torii_contract_errors_total = IntCounterVec::new(
            Opts::new(
                "torii_contract_errors_total",
                "Torii contract endpoints returning errors",
            ),
            &["endpoint"],
        )
        .expect("Infallible");
        let sns_registrar_status_total = IntCounterVec::new(
            Opts::new(
                "sns_registrar_status_total",
                "SNS registrar operation outcomes grouped by result and suffix",
            ),
            &["result", "suffix"],
        )
        .expect("Infallible");
        register_guarded(&registry, &torii_proof_throttled_total);
        register_guarded(&registry, &torii_contract_throttled_total);
        register_guarded(&registry, &torii_contract_errors_total);
        register_guarded(&registry, &sns_registrar_status_total);
        let torii_active_connections_total = GenericGaugeVec::new(
            Opts::new(
                "torii_active_connections_total",
                "Torii active connections by scheme",
            ),
            &["scheme"],
        )
        .expect("Infallible");
        let torii_connect_buffered_sessions = GenericGauge::new(
            "torii_connect_buffered_sessions",
            "Torii Connect sessions with buffered frames",
        )
        .expect("Infallible");
        let torii_connect_total_buffer_bytes = GenericGauge::new(
            "torii_connect_total_buffer_bytes",
            "Torii Connect total buffered bytes across sessions",
        )
        .expect("Infallible");
        let torii_connect_dedupe_size = GenericGauge::new(
            "torii_connect_dedupe_size",
            "Torii Connect dedupe cache size",
        )
        .expect("Infallible");
        let torii_connect_per_ip_sessions = GenericGaugeVec::new(
            Opts::new(
                "torii_connect_per_ip_sessions",
                "Torii Connect per-IP session counts (gauge)",
            ),
            &["ip"],
        )
        .expect("Infallible");
        let zk_verify_latency_ms = HistogramVec::new(
            HistogramOpts::new(
                "zk_verify_latency_ms",
                "Proof verification latency (milliseconds) grouped by backend and status",
            )
            .buckets(prometheus::exponential_buckets(1.0, 2.0, 15).expect("inputs are valid")),
            &["backend", "status"],
        )
        .expect("Infallible");
        let zk_verify_proof_bytes = HistogramVec::new(
            HistogramOpts::new(
                "zk_verify_proof_bytes",
                "Proof payload size (bytes) grouped by backend and status",
            )
            .buckets(prometheus::exponential_buckets(256.0, 2.0, 12).expect("inputs are valid")),
            &["backend", "status"],
        )
        .expect("Infallible");

        // Block-level gas and fees (latest block)
        let block_gas_used = GenericGauge::new(
            "block_gas_used",
            "Total gas used by the latest validated block",
        )
        .expect("Infallible");
        let confidential_gas_tx_used = GenericGauge::new(
            "iroha_confidential_gas_tx_used",
            "Confidential gas used by the most recent transaction",
        )
        .expect("Infallible");
        let confidential_gas_block_used = GenericGauge::new(
            "iroha_confidential_gas_block_used",
            "Confidential gas used by transactions in the current block",
        )
        .expect("Infallible");
        let confidential_gas_total = IntCounter::new(
            "iroha_confidential_gas_total",
            "Total confidential gas consumed since the node started",
        )
        .expect("Infallible");
        let block_fee_total_units = GenericGauge::new(
            "block_fee_total_units",
            "Total fee units charged in the latest validated block",
        )
        .expect("Infallible");

        // Network Time Service (basic gauges)
        let nts_offset_ms = IntGauge::new(
            "nts_offset_ms",
            "Network time offset vs local system time (ms, signed)",
        )
        .expect("Infallible");
        let nts_confidence_ms = GenericGauge::new(
            "nts_confidence_ms",
            "Network time confidence bound (MAD) in ms",
        )
        .expect("Infallible");
        let nts_peers_sampled = GenericGauge::new(
            "nts_peers_sampled",
            "Number of peers contributing NTS samples",
        )
        .expect("Infallible");
        let nts_samples_used = GenericGauge::new(
            "nts_samples_used",
            "Number of samples used in NTS aggregation after filtering",
        )
        .expect("Infallible");
        let nts_healthy = IntGauge::new(
            "nts_healthy",
            "NTS health status (1 = healthy, 0 = unhealthy)",
        )
        .expect("Infallible");
        let nts_fallback = IntGauge::new(
            "nts_fallback",
            "NTS fallback indicator (1 = local time fallback, 0 = network time)",
        )
        .expect("Infallible");
        let nts_min_samples_ok = IntGauge::new(
            "nts_min_samples_ok",
            "NTS minimum-sample health check (1 = ok, 0 = fail)",
        )
        .expect("Infallible");
        let nts_offset_ok = IntGauge::new(
            "nts_offset_ok",
            "NTS offset bound health check (1 = ok, 0 = fail)",
        )
        .expect("Infallible");
        let nts_confidence_ok = IntGauge::new(
            "nts_confidence_ok",
            "NTS confidence bound health check (1 = ok, 0 = fail)",
        )
        .expect("Infallible");
        let nts_rtt_ms_bucket = GenericGaugeVec::new(
            Opts::new(
                "nts_rtt_ms_bucket",
                "NTS RTT histogram bucket counts labeled by le (ms)",
            ),
            &["le"],
        )
        .expect("Infallible");
        let nts_rtt_ms_sum =
            GenericGauge::new("nts_rtt_ms_sum", "NTS RTT histogram sum of milliseconds")
                .expect("Infallible");
        let nts_rtt_ms_count = GenericGauge::new(
            "nts_rtt_ms_count",
            "NTS RTT histogram count of observations",
        )
        .expect("Infallible");
        register!(
            registry,
            nts_offset_ms,
            nts_confidence_ms,
            nts_peers_sampled,
            nts_samples_used,
            nts_healthy,
            nts_fallback,
            nts_min_samples_ok,
            nts_offset_ok,
            nts_confidence_ok,
            nts_rtt_ms_bucket,
            nts_rtt_ms_sum,
            nts_rtt_ms_count
        );

        // BLS signature verification counters per latest block
        let pipeline_sig_bls_agg_same = GenericGauge::new(
            "pipeline_sig_bls_agg_same",
            "BLS signature micro-batches verified via aggregate (same-message) in latest block",
        )
        .expect("Infallible");
        let pipeline_sig_bls_agg_multi = GenericGauge::new(
            "pipeline_sig_bls_agg_multi",
            "BLS signature micro-batches verified via aggregate (multi-message) in latest block",
        )
        .expect("Infallible");
        let pipeline_sig_bls_deterministic = GenericGauge::new(
            "pipeline_sig_bls_deterministic",
            "BLS signature micro-batches verified via deterministic per-signature path in latest block",
        )
        .expect("Infallible");
        let pipeline_sig_bls_agg_same_total = IntCounterVec::new(
            Opts::new(
                "pipeline_sig_bls_agg_same_total",
                "Cumulative BLS same-message aggregate verification attempts labeled by lane and result",
            ),
            &["lane", "result"],
        )
        .expect("Infallible");
        let pipeline_sig_bls_agg_multi_total = IntCounterVec::new(
            Opts::new(
                "pipeline_sig_bls_agg_multi_total",
                "Cumulative BLS multi-message aggregate verification attempts labeled by lane and result",
            ),
            &["lane", "result"],
        )
        .expect("Infallible");

        register!(
            registry,
            txs,
            tx_amounts,
            block_height,
            block_height_non_empty,
            last_commit_time_ms,
            commit_time_ms,
            slot_duration_ms,
            slot_duration_ms_latest,
            da_quorum_ratio,
            connected_peers,
            p2p_peer_churn_total,
            uptime_since_genesis_ms,
            domains,
            accounts,
            isi,
            isi_times,
            view_changes,
            queue_size,
            queue_queued,
            queue_inflight,
            kura_fsync_enabled,
            kura_fsync_failures_total,
            kura_fsync_latency_ms,
            sm_syscall_total,
            sm_syscall_failures_total,
            sm_openssl_preview,
            zk_halo2_enabled,
            zk_halo2_curve_id,
            zk_halo2_backend_id,
            zk_halo2_max_k,
            zk_halo2_verifier_budget_ms,
            zk_halo2_verifier_max_batch,
            zk_halo2_verifier_worker_threads,
            zk_halo2_verifier_queue_cap,
            zk_lane_enqueue_wait_total,
            zk_lane_enqueue_timeout_total,
            zk_lane_drop_total,
            zk_lane_retry_enqueued_total,
            zk_lane_retry_replayed_total,
            zk_lane_retry_exhausted_total,
            zk_lane_pending_depth,
            zk_lane_retry_ring_depth,
            zk_verifier_cache_events_total,
            axt_proof_cache_events_total,
            axt_proof_cache_state,
            confidential_gas_base_verify,
            confidential_gas_per_public_input,
            confidential_gas_per_proof_byte,
            confidential_gas_per_nullifier,
            confidential_gas_per_commitment,
            ivm_gas_schedule_hash_lo,
            ivm_gas_schedule_hash_hi,
            confidential_tree_commitments,
            confidential_tree_depth,
            confidential_root_history_entries,
            confidential_frontier_checkpoints,
            confidential_frontier_last_height,
            confidential_frontier_last_commitments,
            confidential_root_evictions_total,
            confidential_frontier_evictions_total,
            oracle_price_local_per_xor,
            oracle_twap_window_seconds,
            oracle_haircut_basis_points,
            oracle_staleness_seconds,
            oracle_observations_total,
            oracle_aggregation_duration_ms,
            oracle_rewards_total,
            oracle_penalties_total,
            oracle_feed_events_total,
            oracle_feed_events_with_evidence_total,
            oracle_evidence_hashes_total,
            fastpq_execution_mode_total,
            fastpq_poseidon_pipeline_total,
            fastpq_metal_queue_ratio,
            fastpq_metal_queue_depth,
            fastpq_zero_fill_duration_ms,
            fastpq_zero_fill_bandwidth_gbps,
            settlement_events_total,
            settlement_finality_events_total,
            settlement_fx_window_ms,
            settlement_buffer_xor,
            settlement_buffer_capacity_xor,
            settlement_buffer_status,
            settlement_pnl_xor,
            settlement_haircut_bp,
            settlement_swapline_utilisation,
            settlement_conversion_total,
            settlement_haircut_total,
            sumeragi_tx_queue_depth,
            sumeragi_tx_queue_capacity,
            sumeragi_tx_queue_saturated,
            sumeragi_pending_blocks_total,
            sumeragi_pending_blocks_blocking,
            sumeragi_commit_inflight_queue_depth,
            sumeragi_missing_block_requests,
            sumeragi_missing_block_oldest_ms,
            sumeragi_missing_block_retry_window_ms,
            sumeragi_missing_block_dwell_ms,
            sumeragi_epoch_length_blocks,
            sumeragi_epoch_commit_deadline_offset,
            sumeragi_epoch_reveal_deadline_offset,
            state_tiered_hot_entries,
            state_tiered_hot_bytes,
            state_tiered_cold_entries,
            state_tiered_cold_bytes,
            state_tiered_cold_reused_entries,
            state_tiered_cold_reused_bytes,
            state_tiered_hot_promotions,
            state_tiered_hot_demotions,
            state_tiered_hot_grace_overflow_keys,
            state_tiered_hot_grace_overflow_bytes,
            state_tiered_last_snapshot_index,
            storage_budget_bytes_used,
            storage_budget_bytes_limit,
            storage_budget_exceeded_total,
            storage_da_cache_total,
            storage_da_churn_bytes_total,
            alias_usage_total,
            iso_reference_status,
            iso_reference_age_seconds,
            iso_reference_records,
            iso_reference_refresh_interval_secs,
            fraud_psp_assessments_total,
            fraud_psp_missing_assessment_total,
            fraud_psp_invalid_metadata_total,
            fraud_psp_attestation_total,
            fraud_psp_latency_ms,
            fraud_psp_score_bps,
            fraud_psp_outcome_mismatch_total,
            dropped_messages,
            sumeragi_dropped_block_messages_total,
            sumeragi_dropped_control_messages_total,
            sumeragi_npos_collector_selected_total,
            sumeragi_npos_collector_assignments_by_idx,
            sumeragi_vrf_commits_emitted_total,
            sumeragi_vrf_reveals_emitted_total,
            sumeragi_vrf_reveals_late_total,
            sumeragi_vrf_non_reveal_penalties_total,
            sumeragi_vrf_non_reveal_by_signer,
            sumeragi_vrf_no_participation_total,
            sumeragi_vrf_no_participation_by_signer,
            sumeragi_vrf_rejects_total_by_reason,
            p2p_dropped_posts,
            p2p_dropped_broadcasts,
            p2p_subscriber_queue_full_total,
            p2p_subscriber_queue_full_by_topic_total,
            p2p_subscriber_unrouted_total,
            p2p_subscriber_unrouted_by_topic_total,
            p2p_handshake_failures,
            p2p_low_post_throttled_total,
            p2p_low_broadcast_throttled_total,
            p2p_post_overflow_total,
            consensus_ingress_drop_total,
            p2p_dns_refresh_total,
            p2p_dns_ttl_refresh_total,
            p2p_dns_resolution_fail_total,
            p2p_dns_reconnect_success_total,
            p2p_backoff_scheduled_total,
            p2p_deferred_send_enqueued_total,
            p2p_deferred_send_dropped_total,
            p2p_session_reconnect_total,
            p2p_connect_retry_seconds,
            p2p_accept_throttled_total,
            p2p_accept_bucket_evictions_total,
            p2p_accept_buckets_current,
            p2p_accept_prefix_cache_total,
            p2p_accept_throttle_decisions_total,
            p2p_incoming_cap_reject_total,
            p2p_total_cap_reject_total,
            p2p_trust_score,
            p2p_trust_penalties_total,
            p2p_trust_decay_ticks_total,
            p2p_trust_gossip_skipped_total,
            tx_gossip_sent_total,
            tx_gossip_dropped_total,
            tx_gossip_targets,
            tx_gossip_fallback_total,
            tx_gossip_frame_cap_bytes,
            tx_gossip_public_target_cap,
            tx_gossip_restricted_target_cap,
            tx_gossip_public_target_reshuffle_ms,
            tx_gossip_restricted_target_reshuffle_ms,
            tx_gossip_drop_unknown_dataspace,
            tx_gossip_restricted_fallback,
            tx_gossip_restricted_public_policy,
            p2p_ws_inbound_total,
            p2p_ws_outbound_total,
            p2p_scion_inbound_total,
            p2p_scion_outbound_total,
            p2p_queue_depth,
            p2p_queue_dropped_total,
            p2p_handshake_ms_bucket,
            p2p_handshake_ms_sum,
            p2p_handshake_ms_count,
            p2p_handshake_error_total,
            p2p_post_overflow_by_topic,
            p2p_frame_cap_violations_total,
            runtime_upgrade_events_total,
            runtime_upgrade_provenance_rejections_total,
            runtime_abi_version,
            sumeragi_tail_votes_total,
            sumeragi_votes_sent_total,
            sumeragi_votes_received_total,
            sumeragi_qc_sent_total,
            sumeragi_qc_received_total,
            sumeragi_qc_validation_errors_total,
            sumeragi_validation_reject_total,
            sumeragi_validation_reject_last_reason,
            sumeragi_validation_reject_last_height,
            sumeragi_validation_reject_last_view,
            sumeragi_validation_reject_last_timestamp_ms,
            sumeragi_block_sync_roster_source_total,
            sumeragi_block_sync_roster_drop_total,
            sumeragi_block_sync_share_blocks_unsolicited_total,
            sumeragi_consensus_message_handling_total,
            sumeragi_view_change_cause_total,
            sumeragi_view_change_cause_last_timestamp_ms,
            sumeragi_qc_signer_counts,
            sumeragi_invalid_signature_total,
            sumeragi_widen_before_rotate_total,
            sumeragi_view_change_suggest_total,
            sumeragi_view_change_install_total,
            sumeragi_proposal_gap_total,
            sumeragi_view_change_proof_total,
            sumeragi_cert_size,
            sumeragi_commit_signatures_present,
            sumeragi_commit_signatures_counted,
            sumeragi_commit_signatures_set_b,
            sumeragi_commit_signatures_required,
            sumeragi_commit_qc_height,
            sumeragi_commit_qc_view,
            sumeragi_commit_qc_epoch,
            sumeragi_commit_qc_signatures_total,
            sumeragi_commit_qc_validator_set_len,
            sumeragi_leader_index,
            sumeragi_highest_qc_height,
            sumeragi_locked_qc_height,
            sumeragi_locked_qc_view,
            sumeragi_new_view_receipts_by_hv,
            sumeragi_new_view_publish_total,
            sumeragi_new_view_recv_total,
            sumeragi_new_view_dropped_by_lock_total,
            sumeragi_commit_conflict_detected_total,
            sumeragi_missing_block_fetch_total,
            sumeragi_missing_block_fetch_target_total,
            sumeragi_missing_block_fetch_dwell_ms,
            sumeragi_missing_block_fetch_targets,
            blocksync_qc_quarantine_total,
            blocksync_qc_revalidated_total,
            blocksync_qc_final_drop_total,
            qc_deferred_missing_payload_total,
            qc_deferred_resolved_total,
            qc_deferred_expired_total,
            consensus_empty_commit_topology_defer_total,
            consensus_empty_commit_topology_escalation_total,
            consensus_recovery_state_transitions_total,
            consensus_missing_block_height_escalation_total,
            consensus_sidecar_quarantine_total,
            consensus_sidecar_final_drop_total,
            blocksync_range_pull_escalation_total,
            blocksync_range_pull_success_total,
            blocksync_range_pull_failure_total,
            consensus_recovery_stuck_round_seconds,
            sumeragi_da_gate_block_total,
            sumeragi_da_gate_last_reason,
            sumeragi_da_gate_last_satisfied,
            sumeragi_da_gate_satisfied_total,
            sumeragi_da_manifest_guard_total,
            sumeragi_da_manifest_cache_total,
            sumeragi_da_spool_cache_total,
            sumeragi_da_pin_intent_spool_total,
            sumeragi_post_to_peer_total,
            sumeragi_bg_post_enqueued_total,
            sumeragi_bg_post_overflow_total,
            sumeragi_bg_post_drop_total,
            sumeragi_bg_post_queue_depth,
            sumeragi_bg_post_queue_depth_by_peer,
            sumeragi_bg_post_age_ms,
            // Per-peer queue depth gauge carries dynamic labels; registering once prevents
            // duplicate collector panics when tests construct multiple registries.
            ivm_cache_hits,
            ivm_cache_misses,
            ivm_cache_evictions,
            ivm_cache_decoded_streams,
            ivm_cache_decoded_ops_total,
            ivm_cache_decode_failures,
            ivm_cache_decode_time_ns_total,
            ivm_register_max_index,
            ivm_register_unique_count,
            merkle_root_gpu_total,
            merkle_root_cpu_total,
            pipeline_dag_vertices,
            pipeline_dag_edges,
            pipeline_conflict_rate_bps,
            pipeline_access_set_source_total,
            pipeline_overlay_count,
            pipeline_overlay_instructions
        );
        register!(
            registry,
            kaigi_relay_registered_total,
            kaigi_relay_registration_bandwidth,
            kaigi_relay_manifest_updates_total,
            kaigi_relay_manifest_hop_count,
            kaigi_relay_failover_total,
            kaigi_relay_failover_hop_count,
            kaigi_relay_health_reports_total,
            kaigi_relay_health_state
        );
        register!(registry, pipeline_overlay_bytes);
        register!(
            registry,
            pipeline_peak_layer_width,
            pipeline_layer_avg_width,
            pipeline_layer_median_width,
            nexus_config_diff_total,
            nexus_lane_configured_total,
            nexus_lane_id_placeholder,
            nexus_dataspace_id_placeholder,
            nexus_lane_governance_sealed,
            nexus_lane_governance_sealed_total,
            nexus_lane_lifecycle_applied_total,
            nexus_lane_block_height,
            nexus_lane_finality_lag_slots,
            nexus_scheduler_lane_teu_capacity,
            nexus_scheduler_lane_teu_slot_committed,
            nexus_scheduler_lane_trigger_level,
            nexus_scheduler_starvation_bound_slots,
            pipeline_layer_count,
            pipeline_scheduler_utilization_pct,
            pipeline_layer_width_hist_bucket
        );
        register!(
            registry,
            nexus_scheduler_lane_teu_slot_breakdown,
            nexus_scheduler_lane_teu_deferral_total,
            nexus_scheduler_lane_headroom_events_total,
            nexus_scheduler_must_serve_truncations_total,
            nexus_lane_settlement_backlog_xor,
            nexus_public_lane_validator_total,
            nexus_public_lane_validator_activation_total,
            nexus_public_lane_validator_reject_total,
            nexus_public_lane_stake_bonded,
            nexus_public_lane_unbond_pending,
            nexus_public_lane_reward_total,
            nexus_public_lane_slash_total,
            nexus_scheduler_dataspace_teu_backlog,
            nexus_scheduler_dataspace_age_slots,
            nexus_scheduler_dataspace_virtual_finish
        );
        register!(registry, pipeline_quarantine_classified);
        register!(registry, pipeline_quarantine_overflow);
        register!(registry, pipeline_quarantine_executed);
        register!(registry, pipeline_stage_ms);
        register!(registry, amx_prepare_ms);
        register!(registry, amx_commit_ms);
        register!(registry, amx_abort_total);
        register!(
            registry,
            axt_policy_reject_total,
            axt_policy_snapshot_version,
            axt_policy_snapshot_cache_events_total
        );
        register!(
            registry,
            ivm_exec_ms,
            ivm_stack_bytes,
            ivm_stack_clamped,
            ivm_stack_gas_multiplier,
            ivm_stack_pool_fallback_total,
            ivm_stack_budget_hit_total
        );
        register!(
            registry,
            pipeline_detached_prepared,
            pipeline_detached_merged,
            pipeline_detached_fallback
        );
        register!(
            registry,
            merge_ledger_entries_total,
            merge_ledger_latest_epoch
        );
        // RBC metrics registration
        register!(registry, sumeragi_rbc_sessions_active);
        register!(
            registry,
            sumeragi_rbc_sessions_pruned_total,
            sumeragi_rbc_ready_broadcasts_total,
            sumeragi_rbc_rebroadcast_skipped_total,
            sumeragi_rbc_deliver_broadcasts_total,
            sumeragi_da_votes_ingested_total
        );
        register!(
            registry,
            sumeragi_da_votes_ingested_by_collector,
            sumeragi_da_votes_ingested_by_peer
        );
        register!(
            registry,
            sumeragi_rbc_payload_bytes_delivered_total,
            sumeragi_rbc_lane_tx_count,
            sumeragi_rbc_lane_total_chunks,
            sumeragi_rbc_lane_pending_chunks,
            sumeragi_rbc_lane_bytes_total
        );
        register!(
            registry,
            sumeragi_rbc_dataspace_tx_count,
            sumeragi_rbc_dataspace_total_chunks,
            sumeragi_rbc_dataspace_pending_chunks,
            sumeragi_rbc_dataspace_bytes_total,
            sumeragi_qc_assembly_latency_ms,
            sumeragi_qc_last_latency_ms
        );
        register!(
            registry,
            sumeragi_rbc_store_sessions,
            sumeragi_rbc_store_bytes,
            sumeragi_rbc_store_pressure,
            sumeragi_rbc_store_evictions_total,
            sumeragi_rbc_persist_drops_total
        );
        register!(
            registry,
            sumeragi_rbc_backpressure_deferrals_total,
            sumeragi_rbc_deliver_defer_ready_total,
            sumeragi_rbc_deliver_defer_chunks_total,
            sumeragi_rbc_da_reschedule_total,
            sumeragi_rbc_da_reschedule_by_mode_total,
            sumeragi_rbc_abort_total,
            sumeragi_rbc_mismatch_total,
            sumeragi_kura_store_failures_total,
            sumeragi_kura_store_last_retry_attempt,
            sumeragi_kura_store_last_retry_backoff_ms,
            sumeragi_pacemaker_backpressure_deferrals_total,
            sumeragi_pacemaker_backpressure_deferrals_by_reason_total,
            sumeragi_pacemaker_backpressure_deferral_duration_ms,
            sumeragi_pacemaker_backpressure_deferral_active,
            sumeragi_pacemaker_backpressure_deferral_age_ms,
            sumeragi_pacemaker_eval_ms,
            sumeragi_pacemaker_propose_ms,
            sumeragi_pacemaker_backoff_ms,
            sumeragi_pacemaker_rtt_floor_ms,
            sumeragi_pacemaker_backoff_multiplier,
            sumeragi_pacemaker_rtt_floor_multiplier,
            sumeragi_pacemaker_max_backoff_ms,
            sumeragi_pacemaker_jitter_ms,
            sumeragi_pacemaker_jitter_frac_permille,
            sumeragi_pacemaker_round_elapsed_ms,
            sumeragi_pacemaker_view_timeout_target_ms,
            sumeragi_pacemaker_view_timeout_remaining_ms,
            sumeragi_commit_stage_ms,
            state_commit_view_lock_wait_ms,
            state_commit_view_lock_hold_ms,
            sumeragi_commit_pipeline_tick_total,
            sumeragi_prevote_timeout_total,
            sumeragi_rbc_backlog_chunks_total,
            sumeragi_rbc_backlog_chunks_max,
            sumeragi_rbc_backlog_sessions_pending,
            sumeragi_rbc_pending_sessions,
            sumeragi_rbc_pending_chunks,
            sumeragi_rbc_pending_bytes,
            sumeragi_rbc_pending_drops_total,
            sumeragi_rbc_pending_dropped_bytes_total,
            sumeragi_rbc_pending_evicted_total
        );
        register!(
            registry,
            sumeragi_membership_mismatch_total,
            sumeragi_membership_mismatch_active
        );
        register!(
            registry,
            pipeline_sig_bls_agg_same,
            pipeline_sig_bls_agg_multi,
            pipeline_sig_bls_deterministic,
            pipeline_sig_bls_agg_same_total,
            pipeline_sig_bls_agg_multi_total
        );
        register!(registry, block_gas_used);
        register!(
            registry,
            confidential_gas_tx_used,
            confidential_gas_block_used
        );
        register!(registry, confidential_gas_total);
        register!(registry, block_fee_total_units);
        register!(
            registry,
            torii_filter_depth,
            torii_filter_match_count,
            torii_scan_ms,
            torii_stream_rows,
            torii_lane_admission_latency_seconds,
            torii_attachment_reject_total,
            torii_attachment_sanitize_ms
        );
        register!(
            registry,
            torii_zk_prover_attachment_bytes,
            torii_zk_prover_latency_ms,
            torii_zk_prover_gc_total,
            torii_zk_prover_inflight,
            torii_zk_prover_pending,
            torii_zk_ivm_prove_inflight,
            torii_zk_ivm_prove_queued,
            torii_zk_prover_last_scan_bytes,
            torii_zk_prover_last_scan_ms,
            torii_zk_prover_budget_exhausted_total
        );
        register!(
            registry,
            governance_proposals_status,
            governance_council_members,
            governance_council_alternates,
            governance_council_candidates,
            governance_council_verified,
            governance_council_epoch,
            governance_citizens_total,
            governance_citizen_service_events_total,
            governance_protected_namespace_total,
            governance_manifest_admission_total,
            governance_manifest_quorum_total,
            governance_manifest_hook_total,
            governance_manifest_activations_total,
            governance_bond_events_total
        );
        register_guarded(&registry, &sumeragi_phase_latency_ms);
        register_guarded(&registry, &sumeragi_phase_latency_ema_ms);
        let metrics = Self {
            txs,
            block_height,
            block_height_non_empty,
            last_commit_time_ms,
            commit_time_ms,
            slot_duration_ms,
            slot_duration_ms_latest,
            da_quorum_ratio,
            connected_peers,
            p2p_peer_churn_total,
            uptime_since_genesis_ms,
            domains,
            accounts,
            tx_amounts,
            isi,
            isi_times,
            view_changes,
            queue_size,
            queue_queued,
            queue_inflight,
            kura_fsync_enabled,
            kura_fsync_failures_total,
            kura_fsync_latency_ms,
            sm_syscall_total,
            sm_syscall_failures_total,
            sm_openssl_preview,
            zk_halo2_enabled,
            zk_halo2_curve_id,
            zk_halo2_backend_id,
            zk_halo2_max_k,
            zk_halo2_verifier_budget_ms,
            zk_halo2_verifier_max_batch,
            zk_halo2_verifier_worker_threads,
            zk_halo2_verifier_queue_cap,
            zk_lane_enqueue_wait_total,
            zk_lane_enqueue_timeout_total,
            zk_lane_drop_total,
            zk_lane_retry_enqueued_total,
            zk_lane_retry_replayed_total,
            zk_lane_retry_exhausted_total,
            zk_lane_pending_depth,
            zk_lane_retry_ring_depth,
            zk_verifier_cache_events_total,
            confidential_gas_base_verify,
            confidential_gas_per_public_input,
            confidential_gas_per_proof_byte,
            confidential_gas_per_nullifier,
            confidential_gas_per_commitment,
            ivm_gas_schedule_hash_lo,
            ivm_gas_schedule_hash_hi,
            ivm_stack_bytes,
            ivm_stack_clamped,
            ivm_stack_gas_multiplier,
            ivm_stack_pool_fallback_total,
            ivm_stack_budget_hit_total,
            confidential_tree_commitments,
            confidential_tree_depth,
            confidential_root_history_entries,
            confidential_frontier_checkpoints,
            confidential_frontier_last_height,
            confidential_frontier_last_commitments,
            confidential_root_evictions_total,
            confidential_frontier_evictions_total,
            oracle_price_local_per_xor,
            oracle_twap_window_seconds,
            oracle_haircut_basis_points,
            oracle_staleness_seconds,
            oracle_observations_total,
            oracle_aggregation_duration_ms,
            oracle_rewards_total,
            oracle_penalties_total,
            oracle_feed_events_total,
            oracle_feed_events_with_evidence_total,
            oracle_evidence_hashes_total,
            fastpq_execution_mode_total,
            fastpq_poseidon_pipeline_total,
            fastpq_metal_queue_ratio,
            fastpq_metal_queue_depth,
            fastpq_zero_fill_duration_ms,
            fastpq_zero_fill_bandwidth_gbps,
            settlement_events_total,
            settlement_finality_events_total,
            settlement_fx_window_ms,
            settlement_buffer_xor,
            settlement_buffer_capacity_xor,
            settlement_buffer_status,
            settlement_pnl_xor,
            settlement_haircut_bp,
            settlement_swapline_utilisation,
            settlement_conversion_total,
            settlement_haircut_total,
            subscription_billing_attempts_total,
            subscription_billing_outcomes_total,
            offline_transfer_events_total,
            offline_transfer_receipts_total,
            offline_transfer_settled_amount,
            offline_transfer_rejections_total,
            offline_transfer_pruned_total,
            offline_attestation_policy_total,
            offline_app_attest_signature_compat_total,
            social_events_total,
            social_budget_spent,
            social_campaign_spent,
            social_campaign_cap,
            social_campaign_remaining,
            social_campaign_active,
            social_halted,
            social_rejections_total,
            multisig_direct_sign_reject_total,
            social_open_escrows,
            sumeragi_tx_queue_depth,
            sumeragi_tx_queue_capacity,
            sumeragi_tx_queue_saturated,
            sumeragi_pending_blocks_total,
            sumeragi_pending_blocks_blocking,
            sumeragi_commit_inflight_queue_depth,
            sumeragi_missing_block_requests,
            sumeragi_missing_block_oldest_ms,
            sumeragi_missing_block_retry_window_ms,
            sumeragi_missing_block_dwell_ms,
            sumeragi_epoch_length_blocks,
            sumeragi_epoch_commit_deadline_offset,
            sumeragi_epoch_reveal_deadline_offset,
            state_tiered_hot_entries,
            state_tiered_hot_bytes,
            state_tiered_cold_entries,
            state_tiered_cold_bytes,
            state_tiered_cold_reused_entries,
            state_tiered_cold_reused_bytes,
            state_tiered_hot_promotions,
            state_tiered_hot_demotions,
            state_tiered_hot_grace_overflow_keys,
            state_tiered_hot_grace_overflow_bytes,
            state_tiered_last_snapshot_index,
            storage_budget_bytes_used,
            storage_budget_bytes_limit,
            storage_budget_exceeded_total,
            storage_da_cache_total,
            storage_da_churn_bytes_total,
            governance_proposals_status,
            governance_council_members,
            governance_council_alternates,
            governance_council_candidates,
            governance_council_verified,
            governance_council_epoch,
            governance_citizens_total,
            governance_citizen_service_events_total,
            governance_protected_namespace_total,
            governance_manifest_admission_total,
            governance_manifest_quorum_total,
            governance_manifest_hook_total,
            governance_manifest_activations_total,
            governance_bond_events_total,
            governance_manifest_recent,
            taikai_ingest_snapshots,
            taikai_ingest_snapshot_order,
            da_receipt_cursors,
            taikai_alias_rotation_snapshots,
            alias_usage_total,
            iso_reference_status,
            iso_reference_age_seconds,
            iso_reference_records,
            iso_reference_refresh_interval_secs,
            fraud_psp_assessments_total,
            fraud_psp_missing_assessment_total,
            fraud_psp_invalid_metadata_total,
            fraud_psp_attestation_total,
            fraud_psp_latency_ms,
            fraud_psp_score_bps,
            fraud_psp_outcome_mismatch_total,
            streaming_hpke_rekeys_total,
            streaming_gck_rotations_total,
            streaming_quic_datagrams_sent_total,
            streaming_quic_datagrams_dropped_total,
            streaming_fec_parity_current,
            streaming_feedback_timeout_total,
            streaming_soranet_provision_fail_total,
            streaming_soranet_provision_queue_drop_total,
            telemetry_redaction_total,
            telemetry_redaction_skipped_total,
            telemetry_truncation_total,
            streaming_privacy_redaction_fail_total,
            streaming_encode_latency_ms,
            streaming_encode_audio_jitter_ms,
            streaming_encode_audio_max_jitter_ms,
            streaming_encode_dropped_layers_total,
            streaming_decode_buffer_ms,
            streaming_decode_dropped_frames_total,
            streaming_decode_max_queue_ms,
            streaming_decode_av_drift_ms,
            streaming_decode_max_drift_ms,
            streaming_audio_jitter_ms,
            streaming_audio_max_jitter_ms,
            streaming_av_drift_ms,
            streaming_av_max_drift_ms,
            streaming_av_drift_ewma_ms,
            streaming_av_sync_window_ms,
            streaming_av_sync_violation_total,
            streaming_network_rtt_ms,
            streaming_network_loss_percent_x100,
            streaming_network_fec_repairs_total,
            streaming_network_fec_failures_total,
            streaming_network_datagram_reinjects_total,
            streaming_energy_encoder_mw,
            streaming_energy_decoder_mw,
            nexus_audit_outcome_total,
            nexus_audit_outcome_last_timestamp,
            nexus_space_directory_revision_total,
            nexus_space_directory_active_manifests,
            nexus_space_directory_revocations_total,
            kaigi_relay_registered_total,
            kaigi_relay_registration_bandwidth,
            kaigi_relay_manifest_updates_total,
            kaigi_relay_manifest_hop_count,
            kaigi_relay_failover_total,
            kaigi_relay_failover_hop_count,
            kaigi_relay_health_reports_total,
            kaigi_relay_health_state,
            dropped_messages,
            // Sumeragi dropped message counters (consensus and control paths)
            sumeragi_dropped_block_messages_total,
            sumeragi_dropped_control_messages_total,
            p2p_dropped_posts,
            p2p_dropped_broadcasts,
            p2p_subscriber_queue_full_total,
            p2p_subscriber_queue_full_by_topic_total,
            p2p_subscriber_unrouted_total,
            p2p_subscriber_unrouted_by_topic_total,
            p2p_handshake_failures,
            p2p_low_post_throttled_total,
            p2p_low_broadcast_throttled_total,
            p2p_post_overflow_total,
            p2p_post_overflow_by_topic,
            consensus_ingress_drop_total,
            p2p_dns_refresh_total,
            p2p_dns_ttl_refresh_total,
            p2p_dns_resolution_fail_total,
            p2p_dns_reconnect_success_total,
            p2p_backoff_scheduled_total,
            p2p_deferred_send_enqueued_total,
            p2p_deferred_send_dropped_total,
            p2p_session_reconnect_total,
            p2p_connect_retry_seconds,
            p2p_accept_throttled_total,
            p2p_accept_bucket_evictions_total,
            p2p_accept_buckets_current,
            p2p_accept_prefix_cache_total,
            p2p_accept_throttle_decisions_total,
            p2p_incoming_cap_reject_total,
            p2p_total_cap_reject_total,
            p2p_trust_score,
            p2p_trust_penalties_total,
            p2p_trust_decay_ticks_total,
            p2p_trust_gossip_skipped_total,
            tx_gossip_sent_total,
            tx_gossip_dropped_total,
            tx_gossip_targets,
            tx_gossip_fallback_total,
            tx_gossip_frame_cap_bytes,
            tx_gossip_public_target_cap,
            tx_gossip_restricted_target_cap,
            tx_gossip_public_target_reshuffle_ms,
            tx_gossip_restricted_target_reshuffle_ms,
            tx_gossip_drop_unknown_dataspace,
            tx_gossip_restricted_fallback,
            tx_gossip_restricted_public_policy,
            tx_gossip_status,
            tx_gossip_caps,
            p2p_ws_inbound_total,
            p2p_ws_outbound_total,
            p2p_scion_inbound_total,
            p2p_scion_outbound_total,
            p2p_queue_depth,
            p2p_queue_dropped_total,
            p2p_handshake_ms_bucket,
            p2p_handshake_ms_sum,
            p2p_handshake_ms_count,
            p2p_handshake_error_total,
            p2p_frame_cap_violations_total,
            runtime_upgrade_events_total,
            runtime_upgrade_provenance_rejections_total,
            runtime_abi_version,
            sumeragi_tail_votes_total,
            sumeragi_votes_sent_total,
            sumeragi_votes_received_total,
            sumeragi_qc_sent_total,
            sumeragi_qc_received_total,
            sumeragi_qc_validation_errors_total,
            sumeragi_validation_reject_total,
            sumeragi_validation_reject_last_reason,
            sumeragi_validation_reject_last_height,
            sumeragi_validation_reject_last_view,
            sumeragi_validation_reject_last_timestamp_ms,
            sumeragi_block_sync_roster_source_total,
            sumeragi_block_sync_roster_drop_total,
            sumeragi_block_sync_share_blocks_unsolicited_total,
            sumeragi_consensus_message_handling_total,
            sumeragi_view_change_cause_total,
            sumeragi_view_change_cause_last_timestamp_ms,
            sumeragi_qc_signer_counts,
            sumeragi_invalid_signature_total,
            sumeragi_widen_before_rotate_total,
            sumeragi_view_change_suggest_total,
            sumeragi_view_change_install_total,
            sumeragi_proposal_gap_total,
            sumeragi_view_change_proof_total,
            sumeragi_wa_qc_assembled_total,
            sumeragi_cert_size,
            sumeragi_commit_signatures_present,
            sumeragi_commit_signatures_counted,
            sumeragi_commit_signatures_set_b,
            sumeragi_commit_signatures_required,
            sumeragi_commit_qc_height,
            sumeragi_commit_qc_view,
            sumeragi_commit_qc_epoch,
            sumeragi_commit_qc_signatures_total,
            sumeragi_commit_qc_validator_set_len,
            sumeragi_redundant_sends_total,
            sumeragi_redundant_sends_by_collector,
            sumeragi_redundant_sends_by_peer,
            sumeragi_collectors_k,
            sumeragi_redundant_send_r,
            sumeragi_gossip_fallback_total,
            sumeragi_block_created_dropped_by_lock_total,
            sumeragi_block_created_hint_mismatch_total,
            sumeragi_block_created_proposal_mismatch_total,
            lane_relay_invalid_total,
            lane_relay_emergency_override_total,
            sumeragi_collectors_targeted_current,
            sumeragi_collectors_targeted_per_block,
            sumeragi_prf_epoch_seed_hex,
            halo2_status,
            sumeragi_prf_height,
            sumeragi_prf_view,
            sumeragi_membership_view_hash,
            sumeragi_membership_height,
            sumeragi_membership_view,
            sumeragi_membership_epoch,
            sumeragi_redundant_sends_by_role_idx,
            sumeragi_mode_tag,
            sumeragi_staged_mode_tag,
            sumeragi_staged_mode_activation_height,
            sumeragi_mode_activation_lag_blocks,
            sumeragi_mode_activation_lag_blocks_opt,
            sumeragi_mode_flip_kill_switch,
            sumeragi_mode_flip_success_total,
            sumeragi_mode_flip_failure_total,
            sumeragi_mode_flip_blocked_total,
            sumeragi_last_mode_flip_timestamp_ms,
            sumeragi_leader_index,
            sumeragi_highest_qc_height,
            sumeragi_locked_qc_height,
            sumeragi_locked_qc_view,
            sumeragi_new_view_receipts_by_hv,
            sumeragi_new_view_publish_total,
            sumeragi_new_view_recv_total,
            sumeragi_new_view_dropped_by_lock_total,
            sumeragi_commit_conflict_detected_total,
            sumeragi_missing_block_fetch_total,
            sumeragi_missing_block_fetch_target_total,
            sumeragi_missing_block_fetch_dwell_ms,
            sumeragi_missing_block_fetch_targets,
            blocksync_qc_quarantine_total,
            blocksync_qc_revalidated_total,
            blocksync_qc_final_drop_total,
            qc_deferred_missing_payload_total,
            qc_deferred_resolved_total,
            qc_deferred_expired_total,
            consensus_empty_commit_topology_defer_total,
            consensus_empty_commit_topology_escalation_total,
            consensus_recovery_state_transitions_total,
            consensus_missing_block_height_escalation_total,
            consensus_sidecar_quarantine_total,
            consensus_sidecar_final_drop_total,
            blocksync_range_pull_escalation_total,
            blocksync_range_pull_success_total,
            blocksync_range_pull_failure_total,
            consensus_recovery_stuck_round_seconds,
            sumeragi_da_gate_block_total,
            sumeragi_da_gate_last_reason,
            sumeragi_da_gate_last_satisfied,
            sumeragi_da_gate_satisfied_total,
            sumeragi_da_manifest_guard_total,
            sumeragi_da_manifest_cache_total,
            sumeragi_da_spool_cache_total,
            sumeragi_da_pin_intent_spool_total,
            sumeragi_rbc_sessions_active,
            sumeragi_rbc_sessions_pruned_total,
            sumeragi_rbc_ready_broadcasts_total,
            sumeragi_rbc_rebroadcast_skipped_total,
            sumeragi_rbc_deliver_broadcasts_total,
            sumeragi_rbc_payload_bytes_delivered_total,
            sumeragi_rbc_lane_tx_count,
            sumeragi_rbc_lane_total_chunks,
            sumeragi_rbc_lane_pending_chunks,
            sumeragi_rbc_lane_bytes_total,
            sumeragi_rbc_dataspace_tx_count,
            sumeragi_rbc_dataspace_total_chunks,
            sumeragi_rbc_dataspace_pending_chunks,
            sumeragi_rbc_dataspace_bytes_total,
            sumeragi_da_votes_ingested_total,
            sumeragi_da_votes_ingested_by_collector,
            sumeragi_da_votes_ingested_by_peer,
            sumeragi_qc_assembly_latency_ms,
            sumeragi_qc_last_latency_ms,
            sumeragi_rbc_store_sessions,
            sumeragi_rbc_store_bytes,
            sumeragi_rbc_store_pressure,
            sumeragi_rbc_store_evictions_total,
            sumeragi_rbc_persist_drops_total,
            sumeragi_rbc_backpressure_deferrals_total,
            sumeragi_rbc_deliver_defer_ready_total,
            sumeragi_rbc_deliver_defer_chunks_total,
            sumeragi_rbc_da_reschedule_total,
            sumeragi_rbc_da_reschedule_by_mode_total,
            sumeragi_rbc_abort_total,
            sumeragi_rbc_mismatch_total,
            sumeragi_kura_store_failures_total,
            sumeragi_kura_store_last_retry_attempt,
            sumeragi_kura_store_last_retry_backoff_ms,
            sumeragi_pacemaker_backpressure_deferrals_total,
            sumeragi_pacemaker_backpressure_deferrals_by_reason_total,
            sumeragi_pacemaker_backpressure_deferral_duration_ms,
            sumeragi_pacemaker_backpressure_deferral_active,
            sumeragi_pacemaker_backpressure_deferral_age_ms,
            sumeragi_pacemaker_eval_ms,
            sumeragi_pacemaker_propose_ms,
            sumeragi_commit_stage_ms,
            state_commit_view_lock_wait_ms,
            state_commit_view_lock_hold_ms,
            sumeragi_commit_pipeline_tick_total,
            sumeragi_prevote_timeout_total,
            sumeragi_rbc_backlog_chunks_total,
            sumeragi_rbc_backlog_chunks_max,
            sumeragi_rbc_backlog_sessions_pending,
            sumeragi_rbc_pending_sessions,
            sumeragi_rbc_pending_chunks,
            sumeragi_rbc_pending_bytes,
            sumeragi_rbc_pending_drops_total,
            sumeragi_rbc_pending_dropped_bytes_total,
            sumeragi_rbc_pending_evicted_total,
            sumeragi_membership_mismatch_total,
            sumeragi_membership_mismatch_active,
            sumeragi_post_to_peer_total,
            sumeragi_bg_post_enqueued_total,
            sumeragi_bg_post_overflow_total,
            sumeragi_bg_post_drop_total,
            sumeragi_bg_post_queue_depth,
            sumeragi_bg_post_queue_depth_by_peer,
            sumeragi_bg_post_age_ms,
            sumeragi_pacemaker_backoff_ms,
            sumeragi_pacemaker_rtt_floor_ms,
            sumeragi_pacemaker_backoff_multiplier,
            sumeragi_pacemaker_rtt_floor_multiplier,
            sumeragi_pacemaker_max_backoff_ms,
            sumeragi_pacemaker_jitter_ms,
            sumeragi_pacemaker_jitter_frac_permille,
            sumeragi_pacemaker_round_elapsed_ms,
            sumeragi_pacemaker_view_timeout_target_ms,
            sumeragi_pacemaker_view_timeout_remaining_ms,
            sumeragi_phase_latency_ms,
            sumeragi_phase_latency_ema_ms,
            sumeragi_phase_total_ema_ms,
            // IVM cache counters
            ivm_cache_hits,
            ivm_cache_misses,
            ivm_cache_evictions,
            ivm_cache_decoded_streams,
            ivm_cache_decoded_ops_total,
            ivm_cache_decode_failures,
            ivm_cache_decode_time_ns_total,
            ivm_register_max_index,
            ivm_register_unique_count,
            // Merkle root computation counters
            merkle_root_gpu_total,
            merkle_root_cpu_total,
            pipeline_dag_vertices,
            pipeline_dag_edges,
            pipeline_conflict_rate_bps,
            pipeline_access_set_source_total,
            pipeline_comp_count,
            pipeline_comp_max,
            pipeline_comp_hist_bucket,
            pipeline_peak_layer_width,
            pipeline_layer_avg_width,
            pipeline_layer_median_width,
            nexus_config_diff_total,
            nexus_lane_configured_total,
            nexus_lane_id_placeholder,
            nexus_dataspace_id_placeholder,
            nexus_lane_governance_sealed,
            nexus_lane_governance_sealed_total,
            nexus_lane_governance_sealed_aliases,
            nexus_lane_lifecycle_applied_total,
            nexus_lane_block_height,
            nexus_lane_finality_lag_slots,
            nexus_lane_settlement_backlog_xor,
            nexus_public_lane_validator_total,
            nexus_public_lane_validator_activation_total,
            nexus_public_lane_validator_reject_total,
            nexus_public_lane_stake_bonded,
            nexus_public_lane_unbond_pending,
            nexus_public_lane_reward_total,
            nexus_public_lane_slash_total,
            nexus_scheduler_lane_teu_capacity,
            nexus_scheduler_lane_teu_slot_committed,
            nexus_scheduler_lane_trigger_level,
            nexus_scheduler_starvation_bound_slots,
            nexus_scheduler_lane_teu_slot_breakdown,
            nexus_scheduler_lane_teu_deferral_total,
            nexus_scheduler_lane_headroom_events_total,
            nexus_scheduler_must_serve_truncations_total,
            nexus_scheduler_lane_teu_status,
            nexus_scheduler_dataspace_teu_backlog,
            nexus_scheduler_dataspace_age_slots,
            nexus_scheduler_dataspace_virtual_finish,
            nexus_scheduler_dataspace_teu_status,
            pipeline_layer_count,
            pipeline_scheduler_utilization_pct,
            pipeline_layer_width_hist_bucket,
            pipeline_overlay_count,
            pipeline_overlay_instructions,
            pipeline_overlay_bytes,
            pipeline_quarantine_classified,
            pipeline_quarantine_overflow,
            pipeline_quarantine_executed,
            pipeline_stage_ms,
            amx_prepare_ms,
            amx_commit_ms,
            amx_abort_total,
            axt_policy_reject_total,
            axt_policy_snapshot_version,
            axt_policy_snapshot_cache_events_total,
            axt_proof_cache_events_total,
            axt_proof_cache_state,
            ivm_exec_ms,
            pipeline_detached_prepared,
            pipeline_detached_merged,
            pipeline_detached_fallback,
            merge_ledger_entries_total,
            merge_ledger_latest_epoch,
            merge_ledger_latest_root_hex,
            pipeline_sig_bls_agg_same,
            pipeline_sig_bls_agg_multi,
            pipeline_sig_bls_deterministic,
            pipeline_sig_bls_agg_same_total,
            pipeline_sig_bls_agg_multi_total,
            block_gas_used,
            confidential_gas_tx_used,
            confidential_gas_block_used,
            confidential_gas_total,
            block_fee_total_units,
            torii_filter_depth,
            torii_filter_match_count,
            torii_scan_ms,
            torii_stream_rows,
            torii_lane_admission_latency_seconds,
            torii_attachment_reject_total,
            torii_attachment_sanitize_ms,
            torii_zk_prover_attachment_bytes,
            torii_zk_prover_latency_ms,
            torii_zk_prover_gc_total,
            torii_zk_prover_inflight,
            torii_zk_prover_pending,
            torii_zk_ivm_prove_inflight,
            torii_zk_ivm_prove_queued,
            torii_zk_prover_last_scan_bytes,
            torii_zk_prover_last_scan_ms,
            torii_zk_prover_budget_exhausted_total,
            torii_query_snapshot_requests,
            torii_query_snapshot_first_batch_ms,
            torii_query_snapshot_gas_consumed_units_total,
            query_snapshot_lane_first_batch_ms,
            query_snapshot_lane_first_batch_items,
            query_snapshot_lane_remaining_items,
            query_snapshot_lane_cursors_total,
            torii_connect_sessions_total,
            torii_connect_sessions_active,
            torii_pre_auth_reject_total,
            torii_operator_auth_total,
            torii_operator_auth_lockout_total,
            torii_signature_limit_total,
            torii_signature_limit_by_authority_total,
            torii_signature_limit_last_count,
            torii_signature_limit_max,
            torii_nts_unhealthy_reject_total,
            torii_multisig_direct_sign_reject_total,
            torii_sorafs_admission_total,
            torii_sorafs_capacity_telemetry_rejections_total,
            torii_sorafs_capacity_declared_gib,
            torii_sorafs_capacity_effective_gib,
            torii_sorafs_capacity_utilised_gib,
            torii_sorafs_capacity_outstanding_gib,
            torii_sorafs_capacity_gibhours_total,
            torii_sorafs_fee_projection_nanos,
            torii_sorafs_disputes_total,
            torii_sorafs_orders_issued_total,
            torii_sorafs_orders_completed_total,
            torii_sorafs_orders_failed_total,
            torii_sorafs_outstanding_orders,
            torii_sorafs_uptime_bps,
            torii_sorafs_por_bps,
            torii_sorafs_por_ingest_backlog,
            torii_sorafs_por_ingest_failures_total,
            torii_sorafs_repair_tasks_total,
            torii_sorafs_repair_latency_minutes,
            torii_sorafs_repair_queue_depth,
            torii_sorafs_repair_backlog_oldest_age_seconds,
            torii_sorafs_repair_lease_expired_total,
            torii_sorafs_slash_proposals_total,
            torii_sorafs_reconciliation_runs_total,
            torii_sorafs_reconciliation_divergence_count,
            torii_sorafs_gc_runs_total,
            torii_sorafs_gc_evictions_total,
            torii_sorafs_gc_bytes_freed_total,
            torii_sorafs_gc_blocked_total,
            torii_sorafs_gc_expired_manifests,
            torii_sorafs_gc_oldest_expired_age_seconds,
            torii_sorafs_storage_bytes_used,
            torii_sorafs_storage_bytes_capacity,
            torii_sorafs_storage_pin_queue_depth,
            torii_sorafs_storage_fetch_inflight,
            torii_sorafs_storage_fetch_bytes_per_sec,
            torii_sorafs_storage_por_inflight,
            torii_sorafs_storage_por_samples_success_total,
            torii_sorafs_storage_por_samples_failed_total,
            torii_sorafs_chunk_range_requests_total,
            torii_sorafs_chunk_range_bytes_total,
            torii_sorafs_provider_range_capability_total,
            torii_sorafs_range_fetch_throttle_events_total,
            torii_sorafs_range_fetch_concurrency_current,
            torii_sorafs_proof_stream_inflight,
            torii_sorafs_proof_stream_events_total,
            torii_sorafs_proof_stream_latency_ms,
            torii_sorafs_proof_health_alerts_total,
            torii_sorafs_proof_health_pdp_failures,
            torii_sorafs_proof_health_potr_breaches,
            torii_sorafs_proof_health_penalty_nano,
            torii_sorafs_proof_health_window_end_epoch,
            torii_sorafs_proof_health_cooldown,
            torii_sorafs_gar_violations_total,
            torii_sorafs_gateway_refusals_total,
            torii_sorafs_gateway_fixture_info,
            torii_sorafs_registry_manifests_total,
            torii_sorafs_registry_aliases_total,
            torii_sorafs_alias_cache_refresh_total,
            torii_sorafs_alias_cache_age_seconds,
            torii_sorafs_tls_cert_expiry_seconds,
            torii_sorafs_tls_renewal_total,
            torii_sorafs_tls_ech_enabled,
            torii_sorafs_gateway_fixture_version,
            torii_sorafs_registry_orders_total,
            torii_sorafs_replication_sla_total,
            torii_sorafs_replication_backlog_total,
            torii_sorafs_replication_completion_latency_epochs,
            torii_sorafs_replication_deadline_slack_epochs,
            soranet_privacy_ingest_reject_total,
            soranet_privacy_circuit_events_total,
            soranet_privacy_pow_rejects_total,
            soranet_pow_revocation_store_total,
            soranet_privacy_throttles_total,
            soranet_privacy_verified_bytes_total,
            soranet_privacy_active_circuits_avg,
            soranet_privacy_active_circuits_max,
            soranet_privacy_open_buckets,
            soranet_privacy_pending_collectors,
            soranet_privacy_snapshot_suppressed,
            soranet_privacy_snapshot_suppressed_by_mode,
            soranet_privacy_snapshot_drained,
            soranet_privacy_snapshot_suppression_ratio,
            soranet_privacy_evicted_buckets_total,
            soranet_privacy_bucket_suppressed,
            soranet_privacy_suppression_total,
            soranet_privacy_rtt_millis,
            soranet_privacy_gar_reports_total,
            soranet_privacy_last_poll_unixtime,
            soranet_privacy_poll_errors_total,
            soranet_privacy_collector_enabled,
            sorafs_orchestrator_active_fetches,
            sorafs_orchestrator_fetch_duration_ms,
            sorafs_orchestrator_fetch_failures_total,
            sorafs_orchestrator_retries_total,
            sorafs_orchestrator_provider_failures_total,
            sorafs_orchestrator_chunk_latency_ms,
            sorafs_orchestrator_bytes_total,
            sorafs_orchestrator_stalls_total,
            sorafs_orchestrator_transport_events_total,
            sorafs_orchestrator_policy_events_total,
            sorafs_orchestrator_pq_ratio,
            sorafs_orchestrator_pq_candidate_ratio,
            sorafs_orchestrator_pq_deficit_ratio,
            sorafs_orchestrator_classical_ratio,
            sorafs_orchestrator_classical_selected,
            torii_da_rent_gib_months_total,
            torii_da_rent_base_micro_total,
            torii_da_protocol_reserve_micro_total,
            torii_da_provider_reward_micro_total,
            torii_da_pdp_bonus_micro_total,
            torii_da_potr_bonus_micro_total,
            torii_da_receipts_total,
            torii_da_receipt_highest_sequence,
            torii_da_chunking_seconds,
            da_shard_cursor_events_total,
            da_shard_cursor_height,
            da_shard_cursor_lag_blocks,
            taikai_ingest_segment_latency_ms,
            taikai_ingest_live_edge_drift_ms,
            taikai_ingest_live_edge_drift_signed_ms,
            taikai_ingest_errors_total,
            taikai_trm_alias_rotations_total,
            taikai_viewer_rebuffer_events_total,
            taikai_viewer_playback_segments_total,
            taikai_viewer_cek_fetch_duration_ms,
            taikai_viewer_pq_circuit_health,
            taikai_viewer_cek_rotation_seconds_ago,
            taikai_viewer_alerts_firing_total,
            sorafs_taikai_cache_query_total,
            sorafs_taikai_cache_insert_total,
            sorafs_taikai_cache_evictions_total,
            sorafs_taikai_cache_promotions_total,
            sorafs_taikai_cache_bytes_total,
            sorafs_taikai_qos_denied_total,
            sorafs_taikai_queue_events_total,
            sorafs_taikai_queue_depth,
            sorafs_taikai_shard_failovers_total,
            sorafs_taikai_shard_circuits_open,
            sorafs_orchestrator_brownouts_total,
            soranet_reward_base_payout_nanos,
            soranet_reward_events_total,
            soranet_reward_payout_nanos_total,
            soranet_reward_skips_total,
            soranet_reward_adjustment_nanos_total,
            soranet_reward_disputes_total,
            torii_http_requests_total,
            torii_http_request_duration_seconds,
            torii_http_response_bytes_total,
            torii_api_version_negotiated_total,
            torii_content_requests_total,
            torii_content_request_duration_seconds,
            torii_content_response_bytes_total,
            torii_proof_requests_total,
            torii_proof_request_duration_seconds,
            torii_proof_response_bytes_total,
            torii_proof_cache_hits_total,
            torii_request_duration_seconds,
            torii_request_failures_total,
            torii_explorer_requests_total,
            torii_explorer_request_duration_seconds,
            torii_norito_rpc_gate_total,
            torii_address_invalid_total,
            torii_address_domain_total,
            torii_address_collision_total,
            torii_address_collision_domain_total,
            torii_account_literal_total,
            torii_norito_decode_failures_total,
            torii_proof_throttled_total,
            torii_contract_throttled_total,
            torii_contract_errors_total,
            sns_registrar_status_total,
            torii_active_connections_total,
            torii_connect_buffered_sessions,
            torii_connect_total_buffer_bytes,
            torii_connect_dedupe_size,
            torii_connect_per_ip_sessions,
            zk_verify_latency_ms,
            zk_verify_proof_bytes,
            nts_offset_ms,
            nts_confidence_ms,
            nts_peers_sampled,
            nts_samples_used,
            nts_healthy,
            nts_fallback,
            nts_min_samples_ok,
            nts_offset_ok,
            nts_confidence_ok,
            nts_rtt_ms_bucket,
            nts_rtt_ms_sum,
            nts_rtt_ms_count,
            registry,
            // Newly added NPoS PRF metrics
            sumeragi_npos_collector_selected_total,
            sumeragi_npos_collector_assignments_by_idx,
            sumeragi_vrf_commits_emitted_total,
            sumeragi_vrf_reveals_emitted_total,
            sumeragi_vrf_reveals_late_total,
            sumeragi_vrf_non_reveal_penalties_total,
            sumeragi_vrf_non_reveal_by_signer,
            sumeragi_vrf_no_participation_total,
            sumeragi_vrf_no_participation_by_signer,
            sumeragi_vrf_rejects_total_by_reason,
        };
        metrics.apply_stack_snapshot(&stack_settings_snapshot());
        metrics
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

impl Metrics {
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
            FsyncMode::Off => 0,
            FsyncMode::On => 1,
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
            .inc_by(u128_to_f64(quote.base_rent.as_micro()));
        self.torii_da_protocol_reserve_micro_total
            .with_label_values(&labels)
            .inc_by(u128_to_f64(quote.protocol_reserve.as_micro()));
        self.torii_da_provider_reward_micro_total
            .with_label_values(&labels)
            .inc_by(u128_to_f64(quote.provider_reward.as_micro()));
        self.torii_da_pdp_bonus_micro_total
            .with_label_values(&labels)
            .inc_by(u128_to_f64(quote.pdp_bonus.as_micro()));
        self.torii_da_potr_bonus_micro_total
            .with_label_values(&labels)
            .inc_by(u128_to_f64(quote.potr_bonus.as_micro()));
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
        let epoch_label = epoch.to_string();
        self.torii_da_receipts_total
            .with_label_values(&[outcome, &lane_label, &epoch_label])
            .inc();
        if cursor_advanced {
            self.set_da_receipt_cursor(lane_id, epoch, sequence);
        }
    }

    /// Observe DA chunking + erasure coding duration in seconds.
    pub fn observe_da_chunking_seconds(&self, seconds: f64) {
        self.torii_da_chunking_seconds.observe(seconds);
    }

    /// Update the highest-seen DA receipt sequence for a lane/epoch.
    pub fn set_da_receipt_cursor(&self, lane_id: u32, epoch: u64, sequence: u64) {
        let lane_label = lane_id.to_string();
        let epoch_label = epoch.to_string();
        self.torii_da_receipt_highest_sequence
            .with_label_values(&[&lane_label, &epoch_label])
            .set(sequence);

        let key = DaReceiptCursorKey { lane_id, epoch };
        let mut guard = self
            .da_receipt_cursors
            .write()
            .expect("DA receipt cursor cache poisoned");
        guard
            .entry(key)
            .and_modify(|current| *current = (*current).max(sequence))
            .or_insert(sequence);
    }

    /// Snapshot DA receipt cursors for status responses.
    pub fn da_receipt_cursor_status(&self) -> Vec<DaReceiptCursorStatus> {
        self.da_receipt_cursors
            .read()
            .expect("DA receipt cursor cache poisoned")
            .iter()
            .map(
                |(DaReceiptCursorKey { lane_id, epoch }, highest_sequence)| DaReceiptCursorStatus {
                    lane_id: *lane_id,
                    epoch: *epoch,
                    highest_sequence: *highest_sequence,
                },
            )
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

    /// Cache the staged consensus mode (if any) and its activation height.
    pub fn set_sumeragi_staged_mode(
        &self,
        staged_tag: Option<String>,
        activation_height: Option<u64>,
    ) {
        if let Ok(mut guard) = self.sumeragi_staged_mode_tag.write() {
            *guard = staged_tag;
        }
        if let Ok(mut guard) = self.sumeragi_staged_mode_activation_height.write() {
            *guard = activation_height;
        }
    }

    /// Snapshot the cached consensus mode tag.
    pub fn sumeragi_mode_tag(&self) -> String {
        self.sumeragi_mode_tag
            .read()
            .map_or_else(|_| PERMISSIONED_TAG.to_string(), |guard| guard.clone())
    }

    /// Snapshot the staged consensus mode tag (if any).
    pub fn sumeragi_staged_mode_tag(&self) -> Option<String> {
        self.sumeragi_staged_mode_tag
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    /// Snapshot the staged consensus mode activation height (if any).
    pub fn sumeragi_staged_mode_activation_height(&self) -> Option<u64> {
        self.sumeragi_staged_mode_activation_height
            .read()
            .map(|guard| *guard)
            .unwrap_or_default()
    }

    /// Snapshot the observed activation lag (blocks) after a staged mode height passes.
    pub fn sumeragi_mode_activation_lag_blocks(&self) -> Option<u64> {
        self.sumeragi_mode_activation_lag_blocks_opt
            .read()
            .map(|guard| *guard)
            .unwrap_or_default()
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

    /// Record the domain classification (implicit vs explicit, SNS suffix, etc.) emitted by Torii’s address handler.
    pub fn inc_torii_address_domain(&self, endpoint: &str, domain_kind: &str) {
        self.torii_address_domain_total
            .with_label_values(&[endpoint, domain_kind])
            .inc();
    }

    /// Record a Local-12 selector collision detected by Torii.
    pub fn inc_torii_address_collision(&self, endpoint: &str, kind: &str) {
        self.torii_address_collision_total
            .with_label_values(&[endpoint, kind])
            .inc();
    }

    /// Record a Local-12 selector collision grouped by endpoint + domain label.
    pub fn inc_torii_address_collision_domain(&self, endpoint: &str, domain: &str) {
        self.torii_address_collision_domain_total
            .with_label_values(&[endpoint, domain])
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

    /// Record a settled offline-to-online transfer bundle.
    pub fn record_offline_transfer_settlement(&self, amount: f64, receipt_count: u32) {
        self.offline_transfer_events_total
            .with_label_values(&["settled"])
            .inc();
        self.offline_transfer_receipts_total
            .with_label_values(&["settled"])
            .inc_by(u64::from(receipt_count));
        let observed = if amount.is_finite() {
            amount.max(0.0)
        } else {
            0.0
        };
        self.offline_transfer_settled_amount.observe(observed);
    }

    /// Record an offline transfer archival transition.
    pub fn inc_offline_transfer_archived(&self) {
        self.offline_transfer_events_total
            .with_label_values(&["archived"])
            .inc();
    }

    /// Record an offline bundle being pruned from hot storage.
    pub fn inc_offline_transfer_pruned(&self) {
        self.offline_transfer_events_total
            .with_label_values(&["pruned"])
            .inc();
        self.offline_transfer_pruned_total.inc();
    }

    /// Record a validation rejection for an offline transfer bundle.
    pub fn record_offline_transfer_rejection(
        &self,
        platform: OfflineTransferRejectionPlatform,
        reason: OfflineTransferRejectionReason,
    ) {
        self.offline_transfer_rejections_total
            .with_label_values(&[platform.as_label(), reason.as_label()])
            .inc();
    }

    /// Record which Android integrity policy produced the attestation token.
    pub fn record_offline_attestation_policy(&self, policy: &str) {
        self.offline_attestation_policy_total
            .with_label_values(&[policy])
            .inc();
    }

    /// Record an iOS App Attest signature accepted by the compatibility fallback.
    pub fn inc_offline_app_attest_signature_compat(&self) {
        self.offline_app_attest_signature_compat_total.inc();
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
    pub fn record_soranet_privacy_bucket(&self, bucket: &SoranetPrivacyBucketMetricsV1) {
        let mode_label = bucket.mode.as_label();
        let bucket_label_string = bucket.bucket_start_unix.to_string();
        let bucket_label = bucket_label_string.as_str();

        if bucket.is_suppressed() {
            let reason_label = bucket
                .suppression_reason
                .map_or("unknown", SoranetPrivacySuppressionReasonV1::as_label);
            self.soranet_privacy_bucket_suppressed
                .with_label_values(&[mode_label, bucket_label])
                .set(1.0);
            self.soranet_privacy_suppression_total
                .with_label_values(&[mode_label, reason_label])
                .inc();
            return;
        }

        self.soranet_privacy_bucket_suppressed
            .with_label_values(&[mode_label, bucket_label])
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
                    .with_label_values(&[mode_label, bucket_label, kind])
                    .inc_by(value);
            }
        }

        for entry in &bucket.pow_rejects_by_reason {
            if entry.count > 0 {
                self.soranet_privacy_pow_rejects_total
                    .with_label_values(&[mode_label, bucket_label, entry.reason.as_label()])
                    .inc_by(entry.count);
            }
        }

        for (scope, value) in [
            ("congestion", bucket.throttle_congestion_total),
            ("cooldown", bucket.throttle_cooldown_total),
            ("emergency", bucket.throttle_emergency_total),
            ("remote_quota", bucket.throttle_remote_total),
            ("descriptor_quota", bucket.throttle_descriptor_total),
            ("descriptor_replay", bucket.throttle_descriptor_replay_total),
            ("emergency", bucket.throttle_emergency_total),
        ] {
            if value > 0 {
                self.soranet_privacy_throttles_total
                    .with_label_values(&[mode_label, bucket_label, scope])
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
                .with_label_values(&[mode_label, bucket_label])
                .inc_by(bytes);
        }

        let avg_value = bucket.active_circuits_mean.map_or(0.0, Self::to_f64);
        self.soranet_privacy_active_circuits_avg
            .with_label_values(&[mode_label, bucket_label])
            .set(avg_value);

        let max_value = bucket.active_circuits_max.map_or(0.0, Self::to_f64);
        self.soranet_privacy_active_circuits_max
            .with_label_values(&[mode_label, bucket_label])
            .set(max_value);

        for percentile in &bucket.rtt_percentiles_ms {
            self.soranet_privacy_rtt_millis
                .with_label_values(&[mode_label, bucket_label, percentile.label.as_str()])
                .set(Self::to_f64(percentile.value_ms));
        }

        for entry in &bucket.gar_abuse_counts {
            if entry.count == 0 {
                continue;
            }
            let category_hex = encode_hex_lower(&entry.category_hash);
            self.soranet_privacy_gar_reports_total
                .with_label_values(&[mode_label, bucket_label, category_hex.as_str()])
                .inc_by(entry.count);
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

    /// Record a rejected SoraFS capacity telemetry window.
    pub fn record_sorafs_capacity_telemetry_reject(&self, provider: &str, reason: &str) {
        self.torii_sorafs_capacity_telemetry_rejections_total
            .with_label_values(&[provider, reason])
            .inc();
    }

    /// Record the latest SoraFS fee projection for `provider`.
    pub fn record_sorafs_fee_projection(&self, provider: &str, fee_nanos: u128) {
        let gauge_value = u128_to_f64(fee_nanos);
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
        pin_queue_depth: u64,
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
        self.torii_sorafs_storage_pin_queue_depth
            .with_label_values(&[provider])
            .set(pin_queue_depth);
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
        if let (Ok(mut snapshots), Ok(mut order)) = (
            self.taikai_ingest_snapshots.write(),
            self.taikai_ingest_snapshot_order.write(),
        ) {
            if !snapshots.contains_key(&key) {
                if snapshots.len() >= TAIKAI_INGEST_SNAPSHOT_CAP
                    && let Some(evicted) = order.pop_front()
                {
                    snapshots.remove(&evicted);
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
        self.update_taikai_snapshot(cluster, stream, |snapshot| {
            if snapshot.error_totals.contains_key(&reason)
                || snapshot.error_totals.len() < TAIKAI_INGEST_ERROR_REASON_CAP
            {
                *snapshot.error_totals.entry(reason).or_insert(0) += 1;
                return;
            }
            // Evict the lexicographically earliest entry to bound memory usage.
            if snapshot.error_totals.pop_first().is_some() {
                snapshot.error_totals.insert(reason, 1);
            }
        });
    }

    fn record_taikai_alias_rotation_snapshot(&self, snapshot: TaikaiAliasRotationSnapshotArgs<'_>) {
        if let Ok(mut guard) = self.taikai_alias_rotation_snapshots.write() {
            let entry = guard
                .entry((
                    snapshot.cluster.to_owned(),
                    snapshot.event.to_owned(),
                    snapshot.stream.to_owned(),
                ))
                .or_insert_with(TaikaiAliasRotationSnapshotInternal::default);
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

    /// Snapshot the current alias rotation telemetry for status payloads.
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

    /// Record Taikai cache query outcomes.
    pub fn record_taikai_cache_query(&self, result: &str, tier: &str) {
        self.sorafs_taikai_cache_query_total
            .with_label_values(&[result, tier])
            .inc();
    }

    /// Record Taikai cache insert events (also increments byte counters).
    pub fn record_taikai_cache_insert(&self, tier: &str, bytes: u64) {
        self.sorafs_taikai_cache_insert_total
            .with_label_values(&[tier])
            .inc();
        self.record_taikai_cache_bytes("insert", tier, bytes);
    }

    /// Record Taikai cache evictions.
    pub fn record_taikai_cache_eviction(&self, tier: &str, reason: &str) {
        self.sorafs_taikai_cache_evictions_total
            .with_label_values(&[tier, reason])
            .inc();
    }

    /// Record Taikai cache promotions between tiers.
    pub fn record_taikai_cache_promotion(&self, from: &str, to: &str) {
        self.sorafs_taikai_cache_promotions_total
            .with_label_values(&[from, to])
            .inc();
    }

    /// Record Taikai cache byte totals for the provided event and tier.
    pub fn record_taikai_cache_bytes(&self, event: &str, tier: &str, bytes: u64) {
        if bytes == 0 {
            return;
        }
        self.sorafs_taikai_cache_bytes_total
            .with_label_values(&[event, tier])
            .inc_by(bytes);
    }

    /// Record Taikai QoS denials grouped by class.
    pub fn inc_taikai_qos_denied(&self, class: &str) {
        self.sorafs_taikai_qos_denied_total
            .with_label_values(&[class])
            .inc();
    }

    /// Record Taikai queue events grouped by event/class.
    pub fn inc_taikai_queue_event(&self, event: &str, class: &str) {
        self.sorafs_taikai_queue_events_total
            .with_label_values(&[event, class])
            .inc();
    }

    /// Set Taikai queue depth gauges grouped by state.
    pub fn set_taikai_queue_depth(&self, state: &str, value: i64) {
        self.sorafs_taikai_queue_depth
            .with_label_values(&[state])
            .set(value);
    }

    /// Increment the shard failover counter for the preferred → selected pair.
    pub fn inc_taikai_shard_failover(&self, preferred: &str, selected: &str) {
        self.sorafs_taikai_shard_failovers_total
            .with_label_values(&[preferred, selected])
            .inc();
    }

    /// Set the open/closed state gauge for a specific Taikai shard circuit.
    pub fn set_taikai_shard_circuit_open(&self, shard: &str, open: bool) {
        self.sorafs_taikai_shard_circuits_open
            .with_label_values(&[shard])
            .set(i64::from(open));
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

    /// Record a SoraNet PoW revocation store fallback.
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
        #[cfg(feature = "otel-exporter")]
        {
            let otel = global_sorafs_gateway_otel();
            otel.record_proof_event(kind, result, reason, provider_id, tier, latency_ms);
        }
        #[cfg(not(feature = "otel-exporter"))]
        {
            let _ = (provider_id, tier);
        }
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
        #[cfg(feature = "otel-exporter")]
        {
            let otel = global_sorafs_gateway_otel();
            otel.record_request(
                endpoint,
                "success",
                Some("ok"),
                chunker,
                profile,
                provider_id,
                tier,
                Some(status),
            );
            if let Some(latency_ms) = latency_ms {
                otel.observe_ttfb(endpoint, chunker, profile, provider_id, tier, latency_ms);
            }
        }
        #[cfg(not(feature = "otel-exporter"))]
        {
            let _ = (chunker, profile, provider_id, tier, latency_ms);
        }
    }

    /// Set the provider range capability counters for the supplied feature label.
    pub fn set_sorafs_provider_range_capability(&self, feature: &str, count: i64) {
        self.torii_sorafs_provider_range_capability_total
            .with_label_values(&[feature])
            .set(count);
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
        #[cfg(feature = "otel-exporter")]
        {
            let provider_attr = if provider_id.is_empty() {
                None
            } else {
                Some(provider_id)
            };
            let otel = global_sorafs_gateway_otel();
            otel.record_request(
                scope,
                "refused",
                Some(reason),
                None,
                Some(profile),
                provider_attr,
                None,
                Some(status),
            );
        }
        #[cfg(not(feature = "otel-exporter"))]
        {
            let _ = status;
        }
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
        let mut buffer = Vec::new();
        let encoder = prometheus::TextEncoder::new();
        let metric_families = self.registry.gather();
        Encoder::encode(&encoder, &metric_families, &mut buffer)?;
        Ok(String::from_utf8(buffer)?)
    }

    /// Convert metrics to Prometheus format, optionally stripping lane/dataspace-labelled series
    /// when Nexus is disabled.
    ///
    /// # Errors
    /// - If [`Encoder`] fails to encode the data
    /// - If the buffer produced by [`Encoder`] causes [`String::from_utf8`] to fail.
    pub fn try_to_string_with_nexus_gate(&self, nexus_enabled: bool) -> eyre::Result<String> {
        if nexus_enabled {
            return self.try_to_string();
        }

        let mut buffer = Vec::new();
        let encoder = prometheus::TextEncoder::new();
        let metric_families = self.registry.gather();
        let filtered: Vec<_> = metric_families
            .into_iter()
            .filter(|family| !family_has_lane_labels(family))
            .collect();
        Encoder::encode(&encoder, &filtered, &mut buffer)?;
        Ok(String::from_utf8(buffer)?)
    }
}

fn record_gauge_stats(gauge: &GaugeVec, samples: &[f64]) {
    const AVG_LABEL: [&str; 1] = ["avg"];
    const P95_LABEL: [&str; 1] = ["p95"];
    const MAX_LABEL: [&str; 1] = ["max"];
    const COUNT_LABEL: [&str; 1] = ["count"];

    if samples.is_empty() {
        gauge.with_label_values(&AVG_LABEL).set(0.0);
        gauge.with_label_values(&P95_LABEL).set(0.0);
        gauge.with_label_values(&MAX_LABEL).set(0.0);
        gauge.with_label_values(&COUNT_LABEL).set(0.0);
        return;
    }

    let len = samples.len();
    let count = u64::try_from(len).map_or_else(|_| u64_to_f64(u64::MAX), u64_to_f64);
    let sum: f64 = samples.iter().copied().sum();
    let avg = sum / count.max(1.0);

    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);

    let max = *sorted.last().expect("non-empty after guard");
    let rank = ((len as u128) * 95).div_ceil(100);
    let p95_index = rank
        .saturating_sub(1)
        .try_into()
        .map_or(len - 1, |idx: usize| idx.min(len - 1));
    let p95 = sorted[p95_index];

    gauge.with_label_values(&AVG_LABEL).set(avg);
    gauge.with_label_values(&P95_LABEL).set(p95);
    gauge.with_label_values(&MAX_LABEL).set(max);
    gauge.with_label_values(&COUNT_LABEL).set(count);
}

#[allow(clippy::cast_precision_loss)]
fn u64_to_f64(value: u64) -> f64 {
    value as f64
}

fn clamp_u32_to_i64(value: u32) -> i64 {
    i64::from(value)
}

fn u128_to_f64(value: u128) -> f64 {
    u64::try_from(value).map_or(f64::MAX, u64_to_f64)
}

fn family_has_lane_labels(family: &prometheus::proto::MetricFamily) -> bool {
    family
        .get_metric()
        .iter()
        .flat_map(prometheus::proto::Metric::get_label)
        .any(|label| {
            matches!(
                label.name(),
                "lane" | "lane_id" | "dataspace" | "dataspace_id"
            )
        })
}

#[cfg(test)]
mod test {
    #![allow(clippy::restriction)]

    use std::time::Duration;

    use norito::json::{self, Value};

    use super::*;

    #[test]
    fn metrics_lifecycle() {
        let metrics = Metrics::default();
        println!(
            "{:?}",
            metrics
                .try_to_string()
                .expect("Should not fail for default")
        );
        println!("{:?}", Status::from(&metrics));
        println!("{:?}", Status::default());
    }

    #[test]
    fn pacemaker_metrics_are_exported() {
        let metrics = Metrics::default();
        let dump = metrics.try_to_string().expect("metrics text");
        assert!(
            dump.contains("sumeragi_pacemaker_view_timeout_target_ms"),
            "metrics export missing pacemaker view timeout target"
        );
    }

    #[test]
    fn p2p_queue_depth_metric_accepts_updates() {
        let metrics = Metrics::default();
        metrics.p2p_queue_depth.with_label_values(&["High"]).set(12);
        metrics.p2p_queue_depth.with_label_values(&["Low"]).set(7);
        assert_eq!(
            metrics.p2p_queue_depth.with_label_values(&["High"]).get(),
            12
        );
        assert_eq!(metrics.p2p_queue_depth.with_label_values(&["Low"]).get(), 7);
    }

    #[test]
    fn soranet_reward_metrics_record_without_exporter() {
        let metrics = Metrics::default();
        metrics.record_soranet_reward("relay_hex", 0, "rewarded");
        metrics.record_soranet_reward_skip("relay_hex", "insufficient_bond");
        metrics.record_soranet_adjustment("relay_hex", 0, "credit");
        metrics.inc_soranet_dispute("filed");
    }

    #[test]
    fn records_norito_decode_failures() {
        let metrics = Metrics::default();
        metrics.inc_torii_norito_decode_failure("transaction", "checksum_mismatch");
        let counter = metrics
            .torii_norito_decode_failures_total
            .with_label_values(&["transaction", "checksum_mismatch"])
            .get();
        assert_eq!(counter, 1, "decode failure counter increments");
    }

    #[test]
    fn records_norito_rpc_gate_outcomes() {
        let metrics = Metrics::default();
        metrics.inc_torii_norito_rpc_gate("canary", "allowed");
        metrics.inc_torii_norito_rpc_gate("canary", "canary_denied");
        assert_eq!(
            metrics
                .torii_norito_rpc_gate_total
                .with_label_values(&["canary", "allowed"])
                .get(),
            1
        );
        assert_eq!(
            metrics
                .torii_norito_rpc_gate_total
                .with_label_values(&["canary", "canary_denied"])
                .get(),
            1
        );
    }

    #[test]
    fn records_attachment_sanitizer_metrics() {
        let metrics = Metrics::default();
        metrics.inc_torii_attachment_reject("type");
        let counter = metrics
            .torii_attachment_reject_total
            .with_label_values(&["type"])
            .get();
        assert_eq!(counter, 1);
        metrics.observe_torii_attachment_sanitize_ms(12);
        let samples = metrics
            .torii_attachment_sanitize_ms
            .with_label_values::<&str>(&[])
            .get_sample_count();
        assert_eq!(samples, 1);
    }

    #[test]
    fn records_da_chunking_latency() {
        let metrics = Metrics::default();
        metrics.observe_da_chunking_seconds(0.125);
        let samples = metrics.torii_da_chunking_seconds.get_sample_count();
        assert_eq!(samples, 1);
    }

    #[test]
    fn records_operator_auth_metrics() {
        let metrics = Metrics::default();
        metrics.inc_torii_operator_auth("gate", "allowed", "session");
        metrics.inc_torii_operator_auth_lockout("gate", "invalid_session");
        assert_eq!(
            metrics
                .torii_operator_auth_total
                .with_label_values(&["gate", "allowed", "session"])
                .get(),
            1
        );
        assert_eq!(
            metrics
                .torii_operator_auth_lockout_total
                .with_label_values(&["gate", "invalid_session"])
                .get(),
            1
        );
    }

    #[test]
    fn records_sns_registrar_status_metrics() {
        let metrics = Metrics::default();
        metrics.inc_sns_registrar_status("ok", "sora");
        metrics.inc_sns_registrar_status("error", "sora");
        assert_eq!(
            metrics
                .sns_registrar_status_total
                .with_label_values(&["ok", "sora"])
                .get(),
            1
        );
        assert_eq!(
            metrics
                .sns_registrar_status_total
                .with_label_values(&["error", "sora"])
                .get(),
            1
        );
    }

    #[test]
    fn records_rbc_rebroadcast_skips_by_kind() {
        let metrics = Metrics::default();
        metrics
            .sumeragi_rbc_rebroadcast_skipped_total
            .with_label_values(&["payload"])
            .inc();
        metrics
            .sumeragi_rbc_rebroadcast_skipped_total
            .with_label_values(&["ready"])
            .inc();
        assert_eq!(
            metrics
                .sumeragi_rbc_rebroadcast_skipped_total
                .with_label_values(&["payload"])
                .get(),
            1
        );
        assert_eq!(
            metrics
                .sumeragi_rbc_rebroadcast_skipped_total
                .with_label_values(&["ready"])
                .get(),
            1
        );
    }

    #[test]
    fn records_alias_cache_metrics() {
        let metrics = Metrics::default();
        metrics.record_sorafs_alias_cache("success", "fresh", 42.0);

        let refresh_counter = metrics
            .torii_sorafs_alias_cache_refresh_total
            .with_label_values(&["success", "fresh"])
            .get();
        assert_eq!(refresh_counter, 1, "alias cache counter increments");

        let age_samples = metrics
            .torii_sorafs_alias_cache_age_seconds
            .get_sample_count();
        assert_eq!(age_samples, 1, "alias cache age histogram records sample");
    }

    #[test]
    fn records_privacy_suppression_reason_metrics() {
        let metrics = Metrics::default();
        let bucket = SoranetPrivacyBucketMetricsV1::suppressed_with_reason(
            SoranetPrivacyModeV1::Entry,
            1,
            60,
            SoranetPrivacySuppressionReasonV1::CollectorSuppressed,
        );

        metrics.record_soranet_privacy_bucket(&bucket);

        let suppression_gauge = metrics
            .soranet_privacy_bucket_suppressed
            .with_label_values(&["entry", "1"])
            .get();
        assert!(
            (suppression_gauge - 1.0).abs() < f64::EPSILON,
            "suppression gauge toggles"
        );

        let reason_counter = metrics
            .soranet_privacy_suppression_total
            .with_label_values(&["entry", "collector_suppressed"])
            .get();
        assert_eq!(
            reason_counter, 1,
            "suppression counter increments for the reason"
        );
    }

    fn sample_privacy_snapshot() -> PrivacyDrainSnapshot {
        let mut snapshot = PrivacyDrainSnapshot {
            drained_buckets: 5,
            evicted_completed: 2,
            ..PrivacyDrainSnapshot::default()
        };
        snapshot.open_buckets.insert(SoranetPrivacyModeV1::Entry, 3);
        snapshot
            .collector_backlog
            .insert(SoranetPrivacyModeV1::Entry, 7);
        snapshot
            .suppressed_counts
            .insert(SoranetPrivacySuppressionReasonV1::CollectorSuppressed, 4);
        snapshot
            .suppressed_by_mode
            .entry(SoranetPrivacyModeV1::Entry)
            .or_default()
            .insert(SoranetPrivacySuppressionReasonV1::CollectorSuppressed, 2);
        snapshot
    }

    #[test]
    fn records_privacy_queue_snapshot_metrics() {
        let metrics = Metrics::default();
        let snapshot = sample_privacy_snapshot();

        metrics.record_soranet_privacy_queue_snapshot(&snapshot);
        let open_entry = metrics
            .soranet_privacy_open_buckets
            .with_label_values(&["entry"])
            .get();
        assert!(
            (open_entry - 3.0).abs() < f64::EPSILON,
            "entry bucket count recorded"
        );
        assert!(
            metrics
                .soranet_privacy_open_buckets
                .with_label_values(&["middle"])
                .get()
                .abs()
                < f64::EPSILON,
            "missing modes should be reset to zero"
        );
        assert_eq!(
            metrics.soranet_privacy_evicted_buckets_total.get(),
            2,
            "evicted counter increments"
        );
        assert_eq!(
            metrics.soranet_privacy_snapshot_drained.get(),
            5,
            "drained gauge reflects snapshot"
        );
        let collector_gauge = metrics
            .soranet_privacy_snapshot_suppressed
            .with_label_values(&["collector_suppressed"])
            .get();
        assert!(
            (collector_gauge - 4.0).abs() < f64::EPSILON,
            "snapshot gauge reflects supplied suppression counts"
        );
        let pending_collectors = metrics
            .soranet_privacy_pending_collectors
            .with_label_values(&["entry"])
            .get();
        assert!(
            (pending_collectors - 7.0).abs() < f64::EPSILON,
            "pending collector gauge reflects backlog"
        );
        assert!(
            metrics
                .soranet_privacy_pending_collectors
                .with_label_values(&["middle"])
                .get()
                .abs()
                < f64::EPSILON,
            "unspecified collector backlog resets to zero"
        );
        assert!(
            (metrics.soranet_privacy_snapshot_suppression_ratio.get() - 0.8).abs() < f64::EPSILON,
            "suppression ratio reflects suppressed/share of drained buckets"
        );
        assert!(
            metrics
                .soranet_privacy_snapshot_suppressed
                .with_label_values(&["forced_flush_window_elapsed"])
                .get()
                .abs()
                < f64::EPSILON,
            "unspecified reasons reset to zero"
        );
        let collector_by_mode = metrics
            .soranet_privacy_snapshot_suppressed_by_mode
            .with_label_values(&["entry", "collector_suppressed"])
            .get();
        assert!(
            (collector_by_mode - 2.0).abs() < f64::EPSILON,
            "per-mode suppression gauge tracks counts"
        );
        assert!(
            metrics
                .soranet_privacy_snapshot_suppressed_by_mode
                .with_label_values(&["middle", "insufficient_contributors"])
                .get()
                .abs()
                < f64::EPSILON,
            "unspecified per-mode suppression resets to zero"
        );
    }

    #[test]
    fn records_tls_metrics() {
        let metrics = Metrics::default();
        metrics.set_sorafs_tls_state(true, Some(Duration::from_secs(90)));
        metrics.record_sorafs_tls_renewal("success");

        let expiry = metrics.torii_sorafs_tls_cert_expiry_seconds.get();
        assert!(
            (expiry - 90.0).abs() < f64::EPSILON,
            "TLS expiry gauge records seconds remaining"
        );

        let ech_enabled = metrics.torii_sorafs_tls_ech_enabled.get();
        assert_eq!(ech_enabled, 1, "ECH gauge reflects enabled state");

        let renewal_total = metrics
            .torii_sorafs_tls_renewal_total
            .with_label_values(&["success"])
            .get();
        assert_eq!(renewal_total, 1, "TLS renewal counter increments");

        metrics.set_sorafs_tls_state(false, None);
        assert_eq!(
            metrics.torii_sorafs_tls_ech_enabled.get(),
            0,
            "ECH gauge resets when disabled"
        );
        assert!(
            metrics.torii_sorafs_tls_cert_expiry_seconds.get().abs() < f64::EPSILON,
            "TLS expiry gauge resets when expiry is unknown"
        );
    }

    #[test]
    fn records_proof_stream_metrics() {
        let metrics = Metrics::default();
        metrics.inc_sorafs_proof_stream_inflight("por");
        metrics.record_sorafs_proof_stream_event("por", "success", None, None, None, Some(42.0));
        metrics.dec_sorafs_proof_stream_inflight("por");

        let inflight = metrics
            .torii_sorafs_proof_stream_inflight
            .with_label_values(&["por"])
            .get();
        assert_eq!(inflight, 0, "inflight gauge returns to zero after dec");

        let total = metrics
            .torii_sorafs_proof_stream_events_total
            .with_label_values(&["por", "success", "ok"])
            .get();
        assert_eq!(total, 1, "proof stream counter increments");

        let samples = metrics
            .torii_sorafs_proof_stream_latency_ms
            .with_label_values(&["por"])
            .get_sample_count();
        assert_eq!(samples, 1, "latency histogram records observation");
    }

    #[test]
    fn records_torii_proof_metrics() {
        let metrics = Metrics::default();
        metrics.record_torii_proof_request("v1/zk/proof", "ok", 128, Duration::from_millis(5));
        metrics.inc_torii_proof_cache_hit("v1/zk/proof");
        metrics.inc_torii_proof_throttled("v1/zk/proof");

        assert_eq!(
            metrics
                .torii_proof_requests_total
                .with_label_values(&["v1/zk/proof", "ok"])
                .get(),
            1,
            "proof request counter increments"
        );
        assert_eq!(
            metrics
                .torii_proof_response_bytes_total
                .with_label_values(&["v1/zk/proof", "ok"])
                .get(),
            128,
            "proof response bytes counter increments"
        );
        assert_eq!(
            metrics
                .torii_proof_request_duration_seconds
                .with_label_values(&["v1/zk/proof", "ok"])
                .get_sample_count(),
            1,
            "proof request latency histogram records observation"
        );
        assert_eq!(
            metrics
                .torii_proof_cache_hits_total
                .with_label_values(&["v1/zk/proof"])
                .get(),
            1,
            "proof cache hits counter increments"
        );
        assert_eq!(
            metrics
                .torii_proof_throttled_total
                .with_label_values(&["v1/zk/proof"])
                .get(),
            1,
            "proof throttle counter increments"
        );
    }

    #[test]
    fn records_torii_explorer_metrics() {
        let metrics = Metrics::default();
        metrics.record_torii_explorer_request(
            "/v1/explorer/transactions",
            "ok",
            Duration::from_millis(4),
        );
        metrics.record_torii_explorer_request(
            "/v1/explorer/transactions",
            "error",
            Duration::from_millis(7),
        );

        assert_eq!(
            metrics
                .torii_explorer_requests_total
                .with_label_values(&["/v1/explorer/transactions", "ok"])
                .get(),
            1,
            "explorer request counter increments for ok outcomes"
        );
        assert_eq!(
            metrics
                .torii_explorer_requests_total
                .with_label_values(&["/v1/explorer/transactions", "error"])
                .get(),
            1,
            "explorer request counter increments for error outcomes"
        );
        assert_eq!(
            metrics
                .torii_explorer_request_duration_seconds
                .with_label_values(&["/v1/explorer/transactions", "ok"])
                .get_sample_count(),
            1,
            "explorer request latency histogram records ok outcomes"
        );
        assert_eq!(
            metrics
                .torii_explorer_request_duration_seconds
                .with_label_values(&["/v1/explorer/transactions", "error"])
                .get_sample_count(),
            1,
            "explorer request latency histogram records error outcomes"
        );
    }

    #[test]
    fn records_gateway_fixture_version() {
        let metrics = Metrics::default();
        metrics.set_sorafs_gateway_fixture_version("1.0.0");

        let initial = metrics
            .torii_sorafs_gateway_fixture_version
            .with_label_values(&["1.0.0"])
            .get();
        assert_eq!(initial, 1, "fixture version gauge set for current version");

        metrics.set_sorafs_gateway_fixture_version("1.0.1");
        let updated = metrics
            .torii_sorafs_gateway_fixture_version
            .with_label_values(&["1.0.1"])
            .get();
        assert_eq!(updated, 1, "fixture version gauge switches to new version");

        let previous = metrics
            .torii_sorafs_gateway_fixture_version
            .with_label_values(&["1.0.0"])
            .get();
        assert_eq!(previous, 0, "previous version gauge resets");
    }

    #[test]
    fn records_lane_settlement_snapshot_metrics() {
        let metrics = Metrics::default();
        metrics.record_lane_settlement_snapshot(LaneSettlementSnapshot {
            lane_id: "lane-1",
            dataspace_id: "ds-42",
            xor_due_micro: 1_000,
            variance_micro: 250,
            haircut_bps: 25,
            swapline: Some(LaneSwaplineSnapshot {
                profile: "tier1-deep",
                utilisation_micro: 1_000,
            }),
            buffer: Some(LaneSettlementBuffer {
                remaining: 500.0,
                capacity: 1_500.0,
                status: 1.0,
            }),
        });
        let buffer = metrics
            .settlement_buffer_xor
            .with_label_values(&["lane-1", "ds-42"])
            .get();
        assert!(
            (buffer - 500.0).abs() < f64::EPSILON,
            "buffer gauge captures remaining headroom"
        );
        let capacity = metrics
            .settlement_buffer_capacity_xor
            .with_label_values(&["lane-1", "ds-42"])
            .get();
        assert!(
            (capacity - 1_500.0).abs() < f64::EPSILON,
            "buffer capacity gauge records configured capacity"
        );
        let status = metrics
            .settlement_buffer_status
            .with_label_values(&["lane-1", "ds-42"])
            .get();
        assert!(
            (status - 1.0).abs() < f64::EPSILON,
            "buffer status gauge encodes alert/throttle/xor-only/halt states"
        );
        let pnl = metrics
            .settlement_pnl_xor
            .with_label_values(&["lane-1", "ds-42"])
            .get();
        assert!(
            (pnl - u128_to_f64(250)).abs() < f64::EPSILON,
            "pnl gauge captures variance"
        );
        let haircut = metrics
            .settlement_haircut_bp
            .with_label_values(&["lane-1", "ds-42"])
            .get();
        assert!(
            (haircut - 25.0).abs() < f64::EPSILON,
            "haircut gauge captures epsilon bps"
        );
        let swapline = metrics
            .settlement_swapline_utilisation
            .with_label_values(&["lane-1", "ds-42", "tier1-deep"])
            .get();
        assert!(
            (swapline - u128_to_f64(1_000)).abs() < f64::EPSILON,
            "swapline gauge records utilisation"
        );
    }

    #[test]
    fn settlement_conversion_and_haircut_totals_increment() {
        let metrics = Metrics::default();
        metrics.inc_settlement_conversion_total(
            "lane-1",
            "ds-7",
            "61CtjvNd9T3THAR65GsMVHr82Bjc",
            4,
        );
        metrics.inc_settlement_haircut_total("lane-1", "ds-7", 3_500_000);

        let conversions = metrics
            .settlement_conversion_total
            .with_label_values(&["lane-1", "ds-7", "61CtjvNd9T3THAR65GsMVHr82Bjc"])
            .get();
        assert_eq!(conversions, 4);

        let haircut = metrics
            .settlement_haircut_total
            .with_label_values(&["lane-1", "ds-7"])
            .get();
        assert!(
            (haircut - (3_500_000_f64 / 1_000_000.0)).abs() < f64::EPSILON,
            "haircut counter tracks XOR totals"
        );
    }

    #[test]
    fn records_chunk_range_metrics() {
        let metrics = Metrics::default();
        metrics.record_sorafs_chunk_range("car_range", 206, 4_096, None, None, None, None, None);

        let request_counter = metrics
            .torii_sorafs_chunk_range_requests_total
            .with_label_values(&["car_range", "206"])
            .get();
        assert_eq!(request_counter, 1, "chunk-range request counter increments");

        let bytes_counter = metrics
            .torii_sorafs_chunk_range_bytes_total
            .with_label_values(&["car_range"])
            .get();
        assert_eq!(
            bytes_counter, 4_096,
            "chunk-range bytes counter tracks payload"
        );

        metrics.set_sorafs_provider_range_capability("providers", 2);
        let provider_total = metrics
            .torii_sorafs_provider_range_capability_total
            .with_label_values(&["providers"])
            .get();
        assert_eq!(provider_total, 2, "provider capability gauge updates");

        metrics.inc_sorafs_range_fetch_throttle("concurrency");
        let throttle_total = metrics
            .torii_sorafs_range_fetch_throttle_events_total
            .with_label_values(&["concurrency"])
            .get();
        assert_eq!(throttle_total, 1, "throttle counter increments");

        metrics.inc_sorafs_range_fetch_concurrency();
        assert_eq!(
            metrics.torii_sorafs_range_fetch_concurrency_current.get(),
            1,
            "concurrency gauge increments"
        );
        metrics.dec_sorafs_range_fetch_concurrency();
        assert_eq!(
            metrics.torii_sorafs_range_fetch_concurrency_current.get(),
            0,
            "concurrency gauge decrements"
        );
    }

    #[test]
    fn records_sorafs_gc_metrics() {
        let metrics = Metrics::default();
        metrics.inc_sorafs_gc_runs("success");
        metrics.inc_sorafs_gc_evictions("retention_expired");
        metrics.add_sorafs_gc_freed_bytes("retention_expired", 2_048);
        metrics.inc_sorafs_gc_blocked("repair_active");
        metrics.set_sorafs_gc_expired_snapshot(3, 120);

        assert_eq!(
            metrics
                .torii_sorafs_gc_runs_total
                .with_label_values(&["success"])
                .get(),
            1
        );
        assert_eq!(
            metrics
                .torii_sorafs_gc_evictions_total
                .with_label_values(&["retention_expired"])
                .get(),
            1
        );
        assert_eq!(
            metrics
                .torii_sorafs_gc_bytes_freed_total
                .with_label_values(&["retention_expired"])
                .get(),
            2_048
        );
        assert_eq!(
            metrics
                .torii_sorafs_gc_blocked_total
                .with_label_values(&["repair_active"])
                .get(),
            1
        );
        assert_eq!(metrics.torii_sorafs_gc_expired_manifests.get(), 3);
        assert_eq!(
            metrics.torii_sorafs_gc_oldest_expired_age_seconds.get(),
            120
        );
    }

    #[test]
    fn records_sorafs_reconciliation_metrics() {
        let metrics = Metrics::default();
        metrics.inc_sorafs_reconciliation_runs("success");
        metrics.set_sorafs_reconciliation_divergence_count(7);

        assert_eq!(
            metrics
                .torii_sorafs_reconciliation_runs_total
                .with_label_values(&["success"])
                .get(),
            1
        );
        assert_eq!(
            metrics.torii_sorafs_reconciliation_divergence_count.get(),
            7
        );
    }

    #[test]
    fn records_sorafs_repair_metrics() {
        let metrics = Metrics::default();
        metrics.inc_sorafs_repair_tasks("queued");
        metrics.observe_sorafs_repair_latency("completed", 12.5);
        metrics.record_sorafs_repair_queue_depths(&[
            ("provider-a".to_string(), 2),
            ("provider-b".to_string(), 1),
        ]);
        metrics.set_sorafs_repair_backlog_oldest_age_seconds(300);
        metrics.inc_sorafs_repair_lease_expired("requeued");
        metrics.inc_sorafs_slash_proposals("submitted");

        assert_eq!(
            metrics
                .torii_sorafs_repair_tasks_total
                .with_label_values(&["queued"])
                .get(),
            1
        );
        assert_eq!(
            metrics
                .torii_sorafs_repair_queue_depth
                .with_label_values(&["provider-a"])
                .get(),
            2
        );
        assert_eq!(
            metrics
                .torii_sorafs_repair_queue_depth
                .with_label_values(&["provider-b"])
                .get(),
            1
        );
        assert_eq!(
            metrics.torii_sorafs_repair_backlog_oldest_age_seconds.get(),
            300
        );
        assert_eq!(
            metrics
                .torii_sorafs_repair_lease_expired_total
                .with_label_values(&["requeued"])
                .get(),
            1
        );
        assert_eq!(
            metrics
                .torii_sorafs_slash_proposals_total
                .with_label_values(&["submitted"])
                .get(),
            1
        );
    }

    #[test]
    fn repair_otel_handles_noop_without_exporter() {
        let otel = SorafsRepairOtel::new();
        otel.record_task_transition("queued");
        otel.record_latency(1.0, "completed");
        otel.record_backlog_oldest_age_seconds(10.0);
        otel.record_queue_depth(2, "provider-a");
        otel.record_lease_expired("requeued");
        otel.record_slash_proposal("submitted");
        let _ = global_sorafs_repair_otel();
    }

    #[test]
    fn gc_otel_handles_noop_without_exporter() {
        let otel = SorafsGcOtel::new();
        otel.record_run("success");
        otel.record_eviction("retention_expired", 512);
        otel.record_blocked("repair_active");
        let _ = global_sorafs_gc_otel();
    }

    #[test]
    fn records_gar_violation_metrics() {
        let metrics = Metrics::default();
        metrics.record_sorafs_gar_violation("provider", "missing_id");

        let total = metrics
            .torii_sorafs_gar_violations_total
            .with_label_values(&["provider", "missing_id"])
            .get();
        assert_eq!(total, 1, "GAR violation counter increments");
    }

    #[test]
    fn records_gateway_refusal_metrics() {
        let metrics = Metrics::default();
        metrics.record_sorafs_gateway_refusal(
            406,
            "unsupported_chunker",
            "sorafs.sf1@1.0.0",
            "provider123",
            "/v1/sorafs/storage/car/range",
        );

        let total = metrics
            .torii_sorafs_gateway_refusals_total
            .with_label_values(&[
                "unsupported_chunker",
                "sorafs.sf1@1.0.0",
                "provider123",
                "/v1/sorafs/storage/car/range",
            ])
            .get();
        assert_eq!(total, 1, "gateway refusal counter increments");
    }

    #[test]
    fn gateway_otel_handles_noop_without_exporter() {
        let otel = SorafsGatewayOtel::new();
        otel.record_request(
            "chunk",
            "success",
            None,
            Some("sorafs.sf1@1.0.0"),
            Some("sorafs.sf1@1.0.0"),
            Some("provider123"),
            Some("hot"),
            Some(206),
        );
        otel.observe_ttfb(
            "chunk",
            Some("sorafs.sf1@1.0.0"),
            Some("sorafs.sf1@1.0.0"),
            Some("provider123"),
            Some("hot"),
            42.0,
        );
        otel.record_proof_event(
            "por",
            "success",
            None,
            Some("provider123"),
            Some("hot"),
            Some(12.0),
        );
        let _ = global_sorafs_gateway_otel();
    }

    #[test]
    fn node_otel_handles_noop_without_exporter() {
        let otel = SorafsNodeOtel::new();
        otel.record_storage("provider123", 512, 1_024, 10, 1);
        otel.record_storage("provider123", 768, 1_024, 12, 2);
        otel.record_deal_settlement("provider123", "completed", 850_000_000, 600_000_000, 0, 0);
        let _ = global_sorafs_node_otel();
    }

    #[test]
    fn records_gateway_fixture_metadata() {
        let metrics = Metrics::default();
        metrics.set_sorafs_gateway_fixture_metadata("v1", "sf1", "deadbeef", 123);

        let gauge = metrics
            .torii_sorafs_gateway_fixture_info
            .with_label_values(&["v1", "sf1", "deadbeef"])
            .get();
        assert_eq!(
            gauge, 123,
            "fixture metadata gauge stores release timestamp"
        );
    }

    #[allow(clippy::too_many_lines)]
    fn sample_status() -> Status {
        Status {
            peers: 4,
            blocks: 5,
            blocks_non_empty: 3,
            commit_time_ms: 130,
            da_reschedule_total: 7,
            txs_approved: 31,
            txs_rejected: 3,
            uptime: Uptime(Duration::new(5, 937_000_000)),
            view_changes: 2,
            queue_size: 18,
            crypto: CryptoStatus {
                sm_helpers_available: true,
                sm_openssl_preview_enabled: false,
                halo2: Halo2Status {
                    enabled: true,
                    curve: "pasta".to_string(),
                    backend: "ipa".to_string(),
                    max_k: 21,
                    verifier_budget_ms: 350,
                    verifier_max_batch: 8,
                },
            },
            stack: StackStatus {
                requested_scheduler_bytes: 1_048_576,
                requested_prover_bytes: 1_048_576,
                requested_guest_bytes: 1_048_576,
                scheduler_bytes: 1_048_576,
                prover_bytes: 1_048_576,
                guest_bytes: 1_048_576,
                gas_to_stack_multiplier: 4,
                scheduler_clamped: false,
                prover_clamped: false,
                guest_clamped: false,
                pool_fallback_total: 0,
                budget_hit_total: 0,
            },
            sumeragi: Some(SumeragiConsensusStatus {
                mode_tag: PERMISSIONED_TAG.to_string(),
                staged_mode_tag: None,
                staged_mode_activation_height: None,
                mode_activation_lag_blocks: None,
                leader_index: 1,
                highest_qc_height: 10,
                locked_qc_height: 9,
                locked_qc_view: 3,
                commit_signatures_present: 6,
                commit_signatures_counted: 5,
                commit_signatures_set_b: 2,
                commit_signatures_required: 5,
                commit_qc_height: 12,
                commit_qc_view: 4,
                commit_qc_epoch: 1,
                commit_qc_signatures_total: 5,
                commit_qc_validator_set_len: 7,
                gossip_fallback_total: 2,
                block_created_dropped_by_lock_total: 1,
                block_created_hint_mismatch_total: 0,
                block_created_proposal_mismatch_total: 0,
                tx_queue_depth: 5,
                tx_queue_capacity: 20,
                tx_queue_saturated: false,
                epoch_length_blocks: 0,
                epoch_commit_deadline_offset: 0,
                epoch_reveal_deadline_offset: 0,
                view_change_proof_accepted_total: 4,
                view_change_proof_stale_total: 1,
                view_change_proof_rejected_total: 0,
                view_change_install_total: 0,
                view_change_suggest_total: 0,
                da_reschedule_total: 0,
                rbc_store_sessions: 3,
                rbc_store_bytes: 8192,
                rbc_store_pressure_level: 1,
                rbc_store_backpressure_deferrals_total: 4,
                rbc_store_persist_drops_total: 3,
                rbc_store_evictions_total: 2,
                prf_epoch_seed: Some("cafebabe42".to_string()),
                prf_height: 11,
                prf_view: 2,
                lane_governance_sealed_total: 0,
                lane_governance_sealed_aliases: Vec::new(),
                ..SumeragiConsensusStatus::default()
            }),
            governance: GovernanceStatus {
                proposals: GovernanceProposalCounters {
                    proposed: 2,
                    approved: 1,
                    rejected: 0,
                    enacted: 1,
                },
                protected_namespace: GovernanceProtectedNamespaceCounters {
                    total_checks: 5,
                    allowed: 4,
                    rejected: 1,
                },
                manifest_admission: GovernanceManifestAdmissionCounters {
                    total_checks: 6,
                    allowed: 4,
                    missing_manifest: 1,
                    non_validator_authority: 0,
                    quorum_rejected: 1,
                    protected_namespace_rejected: 0,
                    runtime_hook_rejected: 0,
                },
                manifest_quorum: GovernanceManifestQuorumCounters {
                    total_checks: 4,
                    satisfied: 3,
                    rejected: 1,
                },
                recent_manifest_activations: vec![GovernanceManifestActivation {
                    namespace: "apps".to_string(),
                    contract_id: "demo.contract".to_string(),
                    code_hash_hex: "deadbeef".to_string(),
                    abi_hash_hex: Some("cafebabe".to_string()),
                    height: 42,
                    activated_at_ms: 1_234_567,
                }],
                sealed_lanes_total: 0,
                sealed_lane_aliases: Vec::new(),
                citizens_total: 0,
            },
            teu_lane_commit: Vec::new(),
            teu_dataspace_backlog: Vec::new(),
            tx_gossip: TxGossipSnapshot {
                caps: TxGossipCaps {
                    frame_cap_bytes: 0,
                    public_target_cap: None,
                    restricted_target_cap: None,
                    public_target_reshuffle_ms: None,
                    restricted_target_reshuffle_ms: None,
                    drop_unknown_dataspace: false,
                    restricted_fallback: "drop".to_string(),
                    restricted_public_policy: "refuse".to_string(),
                },
                targets: Vec::new(),
            },
            sorafs_micropayments: Vec::new(),
            taikai_alias_rotations: Vec::new(),
            taikai_ingest: Vec::new(),
            da_receipt_cursors: Vec::new(),
        }
    }

    #[test]
    fn build_sumeragi_status_uses_cached_mode_fields() {
        let metrics = Metrics::default();
        metrics.set_sumeragi_mode_tag("custom-mode");
        metrics.set_sumeragi_staged_mode(Some("next-mode".to_string()), Some(42));
        if let Ok(mut guard) = metrics.sumeragi_mode_activation_lag_blocks_opt.write() {
            *guard = Some(3);
        }

        let status = build_sumeragi_status(&metrics);
        assert_eq!(status.mode_tag, "custom-mode");
        assert_eq!(status.staged_mode_tag.as_deref(), Some("next-mode"));
        assert_eq!(status.staged_mode_activation_height, Some(42));
        assert_eq!(status.mode_activation_lag_blocks, Some(3));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn serialize_status_json() {
        let value = sample_status();
        let actual = json::to_json_pretty(&value).expect("Sample is valid");
        let actual_value: Value = json::from_json(&actual).expect("pretty JSON should parse");
        let expected_value = norito::json!({
            "peers": 4,
            "blocks": 5,
            "blocks_non_empty": 3,
            "commit_time_ms": 130,
            "da_reschedule_total": 7,
            "txs_approved": 31,
            "txs_rejected": 3,
            "uptime": {
                "secs": 5,
                "nanos": 937_000_000
            },
            "view_changes": 2,
            "queue_size": 18,
            "crypto": {
                "sm_helpers_available": true,
                "sm_openssl_preview_enabled": false,
                "halo2": {
                    "enabled": true,
                    "curve": "pasta",
                    "backend": "ipa",
                    "max_k": 21,
                    "verifier_budget_ms": 350,
                    "verifier_max_batch": 8
                }
            },
            "stack": {
                "requested_scheduler_bytes": 1_048_576,
                "requested_prover_bytes": 1_048_576,
                "requested_guest_bytes": 1_048_576,
                "scheduler_bytes": 1_048_576,
                "prover_bytes": 1_048_576,
                "guest_bytes": 1_048_576,
                "gas_to_stack_multiplier": 4,
                "scheduler_clamped": false,
                "prover_clamped": false,
                "guest_clamped": false,
                "pool_fallback_total": 0,
                "budget_hit_total": 0
            },
            "sumeragi": {
                "mode_tag": "iroha2-consensus::permissioned-sumeragi@v1",
                "staged_mode_tag": null,
                "staged_mode_activation_height": null,
                "mode_activation_lag_blocks": null,
                "leader_index": 1,
                "highest_qc_height": 10,
                "locked_qc_height": 9,
                "locked_qc_view": 3,
                "commit_signatures_present": 6,
                "commit_signatures_counted": 5,
                "commit_signatures_set_b": 2,
                "commit_signatures_required": 5,
                "commit_qc_height": 12,
                "commit_qc_view": 4,
                "commit_qc_epoch": 1,
                "commit_qc_signatures_total": 5,
                "commit_qc_validator_set_len": 7,
                "gossip_fallback_total": 2,
                "block_created_dropped_by_lock_total": 1,
                "block_created_hint_mismatch_total": 0,
                "block_created_proposal_mismatch_total": 0,
                "tx_queue_depth": 5,
                "tx_queue_capacity": 20,
                "tx_queue_saturated": false,
                "epoch_length_blocks": 0,
                "epoch_commit_deadline_offset": 0,
                "epoch_reveal_deadline_offset": 0,
                "prf_epoch_seed": "cafebabe42",
                "prf_height": 11,
                "prf_view": 2,
                "da_reschedule_total": 0,
                "rbc_deliver_defer_ready_total": 0,
                "rbc_deliver_defer_chunks_total": 0,
                "rbc_store_sessions": 3,
                "rbc_store_bytes": 8192,
                "rbc_store_pressure_level": 1,
                "rbc_store_backpressure_deferrals_total": 4,
                "rbc_store_persist_drops_total": 3,
                "rbc_store_evictions_total": 2,
                "view_change_proof_accepted_total": 4,
                "view_change_proof_stale_total": 1,
                "view_change_proof_rejected_total": 0,
                "view_change_suggest_total": 0,
                "view_change_install_total": 0,
                "lane_governance_sealed_total": 0,
                "lane_governance_sealed_aliases": []
            },
            "governance": {
                "proposals": {
                    "proposed": 2,
                    "approved": 1,
                    "rejected": 0,
                    "enacted": 1
                },
                "protected_namespace": {
                    "total_checks": 5,
                    "allowed": 4,
                    "rejected": 1
                },
                "manifest_admission": {
                    "total_checks": 6,
                    "allowed": 4,
                    "missing_manifest": 1,
                    "non_validator_authority": 0,
                    "quorum_rejected": 1,
                    "protected_namespace_rejected": 0,
                    "runtime_hook_rejected": 0
                },
                "manifest_quorum": {
                    "total_checks": 4,
                    "satisfied": 3,
                    "rejected": 1
                },
                "recent_manifest_activations": [{
                    "namespace": "apps",
                    "contract_id": "demo.contract",
                    "code_hash_hex": "deadbeef",
                    "abi_hash_hex": "cafebabe",
                    "height": 42,
                    "activated_at_ms": 1_234_567
                }],
                "sealed_lanes_total": 0,
                "sealed_lane_aliases": [],
                "citizens_total": 0
            },
            "teu_lane_commit": [],
            "teu_dataspace_backlog": [],
            "tx_gossip": {
                "caps": {
                    "frame_cap_bytes": 0,
                    "drop_unknown_dataspace": false,
                    "restricted_fallback": "drop",
                    "restricted_public_policy": "refuse"
                },
                "targets": []
            },
            "sorafs_micropayments": [],
            "taikai_alias_rotations": [],
            "taikai_ingest": [],
            "da_receipt_cursors": []
        });
        let actual_da = actual_value
            .as_object()
            .and_then(|map| map.get("da_reschedule_total"))
            .cloned()
            .expect("status payload should include da_reschedule_total");
        let expected_da = expected_value
            .as_object()
            .and_then(|map| map.get("da_reschedule_total"))
            .cloned()
            .expect("expected payload should include da_reschedule_total");
        assert_eq!(actual_da, expected_da, "da_reschedule_total mismatch");
        let actual = actual
            .replace("[\n    \n  ]", "[\n  ]")
            .replace("[\n      \n    ]", "[\n  ]");
        // CAUTION: if this is outdated, make sure to update the documentation:
        // https://docs.iroha.tech/reference/torii-endpoints.html#status
        let expected = expect_test::expect![[r#"
            {
              "peers": 4,
              "blocks": 5,
              "blocks_non_empty": 3,
              "commit_time_ms": 130,
              "txs_approved": 31,
              "txs_rejected": 3,
              "uptime": {
                "secs": 5,
                "nanos": 937000000
              },
              "view_changes": 2,
              "queue_size": 18,
              "da_reschedule_total": 7,
              "crypto": {
                "sm_helpers_available": true,
                "sm_openssl_preview_enabled": false,
                "halo2": {
                  "enabled": true,
                  "curve": "pasta",
                  "backend": "ipa",
                  "max_k": 21,
                  "verifier_budget_ms": 350,
                  "verifier_max_batch": 8
                }
              },
              "stack": {
                "requested_scheduler_bytes": 1048576,
                "requested_prover_bytes": 1048576,
                "requested_guest_bytes": 1048576,
                "scheduler_bytes": 1048576,
                "prover_bytes": 1048576,
                "guest_bytes": 1048576,
                "gas_to_stack_multiplier": 4,
                "scheduler_clamped": false,
                "prover_clamped": false,
                "guest_clamped": false,
                "pool_fallback_total": 0,
                "budget_hit_total": 0
              },
              "sumeragi": {
                "mode_tag": "iroha2-consensus::permissioned-sumeragi@v1",
                "leader_index": 1,
                "highest_qc_height": 10,
                "locked_qc_height": 9,
                "locked_qc_view": 3,
                "commit_signatures_present": 6,
                "commit_signatures_counted": 5,
                "commit_signatures_set_b": 2,
                "commit_signatures_required": 5,
                "commit_qc_height": 12,
                "commit_qc_view": 4,
                "commit_qc_epoch": 1,
                "commit_qc_signatures_total": 5,
                "commit_qc_validator_set_len": 7,
                "gossip_fallback_total": 2,
                "block_created_dropped_by_lock_total": 1,
                "block_created_hint_mismatch_total": 0,
                "block_created_proposal_mismatch_total": 0,
                "tx_queue_depth": 5,
                "tx_queue_capacity": 20,
                "tx_queue_saturated": false,
                "epoch_length_blocks": 0,
                "epoch_commit_deadline_offset": 0,
                "epoch_reveal_deadline_offset": 0,
                "prf_epoch_seed": "cafebabe42",
                "prf_height": 11,
                "prf_view": 2,
                "da_reschedule_total": 0,
                "rbc_deliver_defer_ready_total": 0,
                "rbc_deliver_defer_chunks_total": 0,
                "rbc_store_sessions": 3,
                "rbc_store_bytes": 8192,
                "rbc_store_pressure_level": 1,
                "rbc_store_backpressure_deferrals_total": 4,
                "rbc_store_persist_drops_total": 3,
                "rbc_store_evictions_total": 2,
                "view_change_proof_accepted_total": 4,
                "view_change_proof_stale_total": 1,
                "view_change_proof_rejected_total": 0,
                "view_change_suggest_total": 0,
                "view_change_install_total": 0,
                "lane_governance_sealed_total": 0,
                "lane_governance_sealed_aliases": [
              ]
              },
              "governance": {
                "proposals": {
                  "proposed": 2,
                  "approved": 1,
                  "rejected": 0,
                  "enacted": 1
                },
                "protected_namespace": {
                  "total_checks": 5,
                  "allowed": 4,
                  "rejected": 1
                },
                "manifest_admission": {
                  "total_checks": 6,
                  "allowed": 4,
                  "missing_manifest": 1,
                  "non_validator_authority": 0,
                  "quorum_rejected": 1,
                  "protected_namespace_rejected": 0,
                  "runtime_hook_rejected": 0
                },
                "manifest_quorum": {
                  "total_checks": 4,
                  "satisfied": 3,
                  "rejected": 1
                },
                "recent_manifest_activations": [
                  {
                    "namespace": "apps",
                    "contract_id": "demo.contract",
                    "code_hash_hex": "deadbeef",
                    "abi_hash_hex": "cafebabe",
                    "height": 42,
                    "activated_at_ms": 1234567
                  }
                ],
                "sealed_lanes_total": 0,
                "sealed_lane_aliases": [
              ],
                "citizens_total": 0
              },
              "teu_lane_commit": [
              ],
              "teu_dataspace_backlog": [
              ],
              "tx_gossip": {
                "caps": {
                  "frame_cap_bytes": 0,
                  "drop_unknown_dataspace": false,
                  "restricted_fallback": "drop",
                  "restricted_public_policy": "refuse"
                }
              }
            }"#]];
        expected.assert_eq(&actual);
    }
}
