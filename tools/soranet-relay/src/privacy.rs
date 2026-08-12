//! Privacy-preserving telemetry aggregation for the SoraNet relay runtime.
//!
//! The relay accumulates handshake, throttling, and capacity events into
//! coarse-grained buckets so operators can observe health without retaining
//! per-client metadata. Buckets are emitted once they satisfy the configured
//! contribution thresholds; otherwise they surface as
//! `soranet_privacy_bucket_suppressed` markers.

use std::{
    collections::{BTreeMap, VecDeque},
    fmt::{self, Write as _},
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use blake3::Hasher as Blake3Hasher;
use hex::ToHex;
use iroha_data_model::soranet::privacy_metrics::{
    SoranetPrivacyEventActiveSampleV1, SoranetPrivacyEventGarAbuseCategoryV1,
    SoranetPrivacyEventHandshakeFailureV1, SoranetPrivacyEventHandshakeSuccessV1,
    SoranetPrivacyEventKindV1, SoranetPrivacyEventThrottleV1, SoranetPrivacyEventV1,
    SoranetPrivacyEventVerifiedBytesV1, SoranetPrivacyHandshakeFailureV1, SoranetPrivacyModeV1,
    SoranetPrivacyThrottleScopeV1,
};
use norito::json;

use crate::config::{
    PRIVACY_EVENT_BUFFER_MAX_CAPACITY_V1, PRIVACY_MAX_COMPLETED_BUCKETS_V1,
    PRIVACY_MAX_OPEN_BUCKETS_V1, PrivacyTelemetryConfig, RelayMode,
};

/// Percentiles captured in RTT exports.
const RTT_PERCENTILES: &[f64] = &[0.5, 0.9, 0.99];
/// Latency histogram bucket bounds (inclusive, milliseconds).
const RTT_BUCKET_BOUNDS_MS: &[u64] = &[
    10, 25, 50, 75, 100, 150, 200, 300, 500, 750, 1_000, 1_500, 2_000, 2_500, 3_000,
];
/// Maximum copied detail retained in one first-release privacy event.
const PRIVACY_EVENT_DETAIL_MAX_BYTES_V1: usize = 256;
/// Maximum encoded JSON retained transiently for one privacy event.
const PRIVACY_EVENT_JSON_MAX_BYTES_V1: usize = 2 * 1024;
/// Maximum distinct privacy-preserving GAR hashes retained in one bucket.
const PRIVACY_GAR_CATEGORIES_PER_BUCKET_MAX_V1: usize = 256;
/// Conservative Prometheus output allowance for one completed bucket.
const PRIVACY_PROMETHEUS_MAX_BYTES_PER_BUCKET_V1: usize = 128 * 1024;

struct BoundedText {
    inner: String,
    maximum: usize,
    failed: bool,
}

impl BoundedText {
    fn new(maximum: usize) -> Self {
        Self {
            inner: String::new(),
            maximum,
            failed: false,
        }
    }

    fn into_string(self) -> String {
        if self.failed {
            String::new()
        } else {
            self.inner
        }
    }
}

impl fmt::Write for BoundedText {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let Some(next) = self.inner.len().checked_add(value.len()) else {
            self.failed = true;
            return Err(fmt::Error);
        };
        if next > self.maximum || self.inner.try_reserve(value.len()).is_err() {
            self.failed = true;
            return Err(fmt::Error);
        }
        self.inner.push_str(value);
        Ok(())
    }
}

fn bounded_event_queue(requested: usize) -> (usize, VecDeque<SoranetPrivacyEventV1>) {
    let capacity = requested.max(1).min(PRIVACY_EVENT_BUFFER_MAX_CAPACITY_V1);
    let mut events = VecDeque::new();
    if events.try_reserve_exact(capacity).is_err() {
        return (0, events);
    }
    (capacity, events)
}

fn drain_event_ndjson(events: &mut VecDeque<SoranetPrivacyEventV1>) -> String {
    let mut body = String::new();
    while let Some(event) = events.pop_front() {
        let line = match json::to_json_bounded(&event, PRIVACY_EVENT_JSON_MAX_BYTES_V1) {
            Ok(line) => line,
            Err(error) => {
                eprintln!("failed to serialise bounded privacy event: {error}");
                continue;
            }
        };
        let additional = line.len().saturating_add(1);
        if body.try_reserve(additional).is_err() {
            events.clear();
            break;
        }
        body.push_str(&line);
        body.push('\n');
    }
    body
}

/// Aggregator configuration knobs used by the privacy telemetry layer.
#[derive(Debug, Clone, Copy)]
pub struct PrivacyConfig {
    /// Bucket duration in seconds.
    pub bucket_secs: u64,
    /// Minimum handshakes required before flushing a bucket.
    pub min_handshakes: u64,
    /// Maximum completed buckets retained in memory.
    pub max_completed_buckets: usize,
    /// Buckets to delay before flushing partially complete buckets.
    pub flush_delay_buckets: u64,
    /// Forced flush interval even when handshakes are below threshold.
    pub force_flush_buckets: u64,
    /// Expected number of shares contributed by relays.
    pub expected_shares: u16,
    /// Capacity of the event buffer.
    pub event_buffer_capacity: usize,
}

impl Default for PrivacyConfig {
    fn default() -> Self {
        Self {
            bucket_secs: 60,
            min_handshakes: 12,
            max_completed_buckets: 60,
            flush_delay_buckets: 1,
            force_flush_buckets: 6,
            expected_shares: 2,
            event_buffer_capacity: 4_096,
        }
    }
}

/// Reasons why a handshake was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectReason {
    Pow,
    Timeout,
    Downgrade,
    Other,
}

/// Throttle scopes tracked across buckets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThrottleScope {
    Congestion,
    Cooldown,
    Emergency,
    RemoteQuota,
    DescriptorQuota,
    DescriptorReplay,
}

impl From<RejectReason> for SoranetPrivacyHandshakeFailureV1 {
    fn from(reason: RejectReason) -> Self {
        match reason {
            RejectReason::Pow => Self::Pow,
            RejectReason::Timeout => Self::Timeout,
            RejectReason::Downgrade => Self::Downgrade,
            RejectReason::Other => Self::Other,
        }
    }
}

impl From<ThrottleScope> for SoranetPrivacyThrottleScopeV1 {
    fn from(scope: ThrottleScope) -> Self {
        match scope {
            ThrottleScope::Congestion => Self::Congestion,
            ThrottleScope::Cooldown => Self::Cooldown,
            ThrottleScope::Emergency => Self::Emergency,
            ThrottleScope::RemoteQuota => Self::RemoteQuota,
            ThrottleScope::DescriptorQuota => Self::DescriptorQuota,
            ThrottleScope::DescriptorReplay => Self::DescriptorReplay,
        }
    }
}

/// Bounded ring buffer of privacy events for downstream collectors.
pub struct PrivacyEventBuffer {
    max_events: usize,
    events: Mutex<VecDeque<SoranetPrivacyEventV1>>,
}

/// Bounded downgrade buffer used for orchestrator proxy remediation hooks.
pub struct ProxyPolicyEventBuffer {
    max_events: usize,
    events: Mutex<VecDeque<SoranetPrivacyEventV1>>,
}

impl PrivacyEventBuffer {
    /// Construct a new buffer retaining up to `max_events` entries.
    pub fn new(max_events: usize) -> Self {
        let (capacity, events) = bounded_event_queue(max_events);
        Self {
            max_events: capacity,
            events: Mutex::new(events),
        }
    }

    pub fn record_handshake_success(
        &self,
        mode: SoranetPrivacyModeV1,
        when: SystemTime,
        rtt_ms: Option<u64>,
        active_after: Option<u64>,
    ) {
        let event = SoranetPrivacyEventV1 {
            timestamp_unix: unix_seconds(when),
            kind: SoranetPrivacyEventKindV1::HandshakeSuccess(
                SoranetPrivacyEventHandshakeSuccessV1 {
                    rtt_ms,
                    active_circuits_after: active_after,
                },
            ),
            mode,
        };
        self.push(event);
    }

    pub fn record_handshake_failure(
        &self,
        mode: SoranetPrivacyModeV1,
        when: SystemTime,
        reason: SoranetPrivacyHandshakeFailureV1,
        detail: Option<&str>,
        rtt_ms: Option<u64>,
    ) {
        let event = SoranetPrivacyEventV1 {
            timestamp_unix: unix_seconds(when),
            kind: SoranetPrivacyEventKindV1::HandshakeFailure(
                SoranetPrivacyEventHandshakeFailureV1 {
                    reason,
                    detail: detail_to_string(detail),
                    rtt_ms,
                },
            ),
            mode,
        };
        self.push(event);
    }

    pub fn record_throttle(
        &self,
        mode: SoranetPrivacyModeV1,
        when: SystemTime,
        scope: SoranetPrivacyThrottleScopeV1,
    ) {
        let event = SoranetPrivacyEventV1 {
            timestamp_unix: unix_seconds(when),
            kind: SoranetPrivacyEventKindV1::Throttle(SoranetPrivacyEventThrottleV1 { scope }),
            mode,
        };
        self.push(event);
    }

    pub fn record_active_sample(
        &self,
        mode: SoranetPrivacyModeV1,
        when: SystemTime,
        active_circuits: u64,
    ) {
        let event = SoranetPrivacyEventV1 {
            timestamp_unix: unix_seconds(when),
            kind: SoranetPrivacyEventKindV1::ActiveSample(SoranetPrivacyEventActiveSampleV1 {
                active_circuits,
            }),
            mode,
        };
        self.push(event);
    }

    pub fn record_verified_bytes(&self, mode: SoranetPrivacyModeV1, when: SystemTime, bytes: u128) {
        if bytes == 0 {
            return;
        }
        let event = SoranetPrivacyEventV1 {
            timestamp_unix: unix_seconds(when),
            kind: SoranetPrivacyEventKindV1::VerifiedBytes(SoranetPrivacyEventVerifiedBytesV1 {
                bytes,
            }),
            mode,
        };
        self.push(event);
    }

    pub fn record_gar_category(&self, mode: SoranetPrivacyModeV1, when: SystemTime, label: &str) {
        if label.is_empty() {
            return;
        }
        let event = SoranetPrivacyEventV1 {
            timestamp_unix: unix_seconds(when),
            kind: SoranetPrivacyEventKindV1::GarAbuseCategory(
                SoranetPrivacyEventGarAbuseCategoryV1 {
                    label: label.to_string(),
                },
            ),
            mode,
        };
        self.push(event);
    }

    /// Drain buffered events, serialising them as newline-delimited JSON.
    pub fn drain_ndjson(&self) -> String {
        let mut guard = self
            .events
            .lock()
            .expect("privacy event buffer mutex poisoned");
        drain_event_ndjson(&mut guard)
    }

    /// Return the number of buffered privacy events without draining them.
    pub fn queue_depth(&self) -> usize {
        let guard = self
            .events
            .lock()
            .expect("privacy event buffer mutex poisoned");
        guard.len()
    }

    fn push(&self, event: SoranetPrivacyEventV1) {
        let mut guard = self
            .events
            .lock()
            .expect("privacy event buffer mutex poisoned");
        if self.max_events == 0 {
            return;
        }
        if guard.len() == self.max_events {
            guard.pop_front();
        }
        guard.push_back(event);
    }
}

impl ProxyPolicyEventBuffer {
    /// Construct a downgrade buffer retaining up to `max_events` entries.
    pub fn new(max_events: usize) -> Self {
        let (capacity, events) = bounded_event_queue(max_events);
        Self {
            max_events: capacity,
            events: Mutex::new(events),
        }
    }

    /// Record a downgrade event for downstream remediation hooks.
    pub fn record_downgrade(
        &self,
        mode: SoranetPrivacyModeV1,
        when: SystemTime,
        detail: Option<&str>,
    ) {
        let event = SoranetPrivacyEventV1 {
            timestamp_unix: unix_seconds(when),
            kind: SoranetPrivacyEventKindV1::HandshakeFailure(
                SoranetPrivacyEventHandshakeFailureV1 {
                    reason: SoranetPrivacyHandshakeFailureV1::Downgrade,
                    detail: detail_to_string(detail),
                    rtt_ms: None,
                },
            ),
            mode,
        };

        let mut guard = self
            .events
            .lock()
            .expect("proxy policy buffer mutex poisoned");
        if self.max_events == 0 {
            return;
        }
        if guard.len() == self.max_events {
            guard.pop_front();
        }
        guard.push_back(event);
    }

    /// Drain buffered downgrade events as NDJSON body.
    pub fn drain_ndjson(&self) -> String {
        let mut guard = self
            .events
            .lock()
            .expect("proxy policy buffer mutex poisoned");
        drain_event_ndjson(&mut guard)
    }

    /// Current number of downgrade events awaiting proxy remediation.
    pub fn queue_depth(&self) -> usize {
        let guard = self
            .events
            .lock()
            .expect("proxy policy buffer mutex poisoned");
        guard.len()
    }
}

fn detail_to_string(detail: Option<&str>) -> Option<String> {
    detail.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            let mut end = trimmed.len().min(PRIVACY_EVENT_DETAIL_MAX_BYTES_V1);
            while !trimmed.is_char_boundary(end) {
                end = end.saturating_sub(1);
            }
            let mut retained = String::new();
            retained.try_reserve_exact(end).ok()?;
            retained.push_str(&trimmed[..end]);
            Some(retained)
        }
    })
}

/// Aggregates privacy-aware counters for a relay instance.
pub struct PrivacyAggregator {
    config: PrivacyConfig,
    state: Mutex<PrivacyState>,
}

/// Tracks open and completed buckets for the privacy aggregator.
#[derive(Debug, Default)]
struct PrivacyState {
    open: BTreeMap<u64, BucketStats>,
    completed: VecDeque<CompletedBucket>,
}

/// Completed bucket ready for export.
#[derive(Debug, Clone)]
struct CompletedBucket {
    start_bucket: u64,
    stats: BucketSummary,
}

/// Summarised stats recorded for a completed bucket.
#[derive(Debug, Clone)]
struct BucketSummary {
    handshake_success: u64,
    handshake_pow_rejects: u64,
    handshake_downgrades: u64,
    handshake_timeouts: u64,
    handshake_other_failures: u64,
    capacity_rejects: u64,
    throttle_congestion: u64,
    throttle_cooldown: u64,
    throttle_emergency: u64,
    throttle_remote: u64,
    throttle_descriptor: u64,
    throttle_descriptor_replay: u64,
    cooldown_millis_sum: u128,
    cooldown_count: u64,
    active_avg: Option<f64>,
    active_max: Option<u64>,
    bytes_verified: u128,
    rtt_percentiles: Vec<(String, u64)>,
    gar_counts: BTreeMap<String, u64>,
    suppressed: bool,
}

/// Running bucket statistics before completion.
#[derive(Debug, Default)]
struct BucketStats {
    handshake_success: u64,
    handshake_pow_rejects: u64,
    handshake_downgrades: u64,
    handshake_timeouts: u64,
    handshake_other_failures: u64,
    capacity_rejects: u64,
    throttle_congestion: u64,
    throttle_cooldown: u64,
    throttle_emergency: u64,
    throttle_remote: u64,
    throttle_descriptor: u64,
    throttle_descriptor_replay: u64,
    throttle_cooldown_sum_millis: u128,
    throttle_cooldown_count: u64,
    rtt: LatencyHistogram,
    active: ActiveAccumulator,
    bytes_verified: u128,
    gar_counts: BTreeMap<String, u64>,
}

/// Histogram accumulator for RTT measurements.
#[derive(Debug, Default)]
struct LatencyHistogram {
    buckets: [u64; RTT_BUCKET_BOUNDS_MS.len() + 1],
    total: u64,
}

impl LatencyHistogram {
    fn observe(&mut self, millis: u64) {
        let idx = RTT_BUCKET_BOUNDS_MS
            .iter()
            .position(|bound| millis <= *bound)
            .unwrap_or(RTT_BUCKET_BOUNDS_MS.len());
        self.buckets[idx] = self.buckets[idx].saturating_add(1);
        self.total = self.total.saturating_add(1);
    }

    fn percentiles(&self) -> Vec<(String, u64)> {
        if self.total == 0 {
            return Vec::new();
        }
        let mut result = Vec::with_capacity(RTT_PERCENTILES.len());
        for percentile in RTT_PERCENTILES {
            let rank = ((*percentile * self.total as f64).ceil() as u64).max(1);
            let mut cumulative = 0u64;
            let mut value = RTT_BUCKET_BOUNDS_MS
                .last()
                .copied()
                .unwrap_or_default()
                .max(1);
            for (idx, count) in self.buckets.iter().copied().enumerate() {
                cumulative = cumulative.saturating_add(count);
                if cumulative >= rank {
                    value = if idx < RTT_BUCKET_BOUNDS_MS.len() {
                        RTT_BUCKET_BOUNDS_MS[idx]
                    } else {
                        RTT_BUCKET_BOUNDS_MS
                            .last()
                            .map(|bound| bound.saturating_add(1_000))
                            .unwrap_or(1_000)
                    };
                    break;
                }
            }
            let label = format!("p{}", (percentile * 100.0) as u32);
            result.push((label, value));
        }
        result
    }
}

/// Accumulator for active circuit counts.
#[derive(Debug, Default)]
struct ActiveAccumulator {
    total: u128,
    samples: u64,
    max: u64,
}

impl ActiveAccumulator {
    fn record(&mut self, value: u64) {
        self.total = self.total.saturating_add(u128::from(value));
        self.samples = self.samples.saturating_add(1);
        if value > self.max {
            self.max = value;
        }
    }

    fn summary(&self) -> (Option<f64>, Option<u64>) {
        if self.samples == 0 {
            return (None, None);
        }
        let avg = (self.total as f64) / (self.samples as f64);
        (Some(avg), Some(self.max))
    }
}
impl CompletedBucket {
    fn render_prometheus(&self, output: &mut impl fmt::Write, mode: RelayMode, bucket_secs: u64) {
        let bucket_start_secs = self.start_bucket.saturating_mul(bucket_secs);
        let bucket_label = bucket_start_secs.to_string();
        if self.stats.suppressed {
            let _ = writeln!(
                output,
                "soranet_privacy_bucket_suppressed{{mode=\"{mode}\",bucket_start=\"{bucket}\"}} 1",
                mode = mode.as_label(),
                bucket = bucket_label,
            );
            return;
        }

        let mut emit_event = |kind: &str, value: u64| {
            if value == 0 {
                return;
            }
            let _ = writeln!(
                output,
                "soranet_privacy_circuit_events_total{{mode=\"{mode}\",bucket_start=\"{bucket}\",kind=\"{kind}\"}} {value}",
                mode = mode.as_label(),
                bucket = bucket_label,
                kind = kind,
                value = value,
            );
        };

        emit_event("accepted", self.stats.handshake_success);
        emit_event("pow_rejected", self.stats.handshake_pow_rejects);
        emit_event("downgrade", self.stats.handshake_downgrades);
        emit_event("timeout", self.stats.handshake_timeouts);
        emit_event("other_failure", self.stats.handshake_other_failures);
        emit_event("capacity_reject", self.stats.capacity_rejects);

        let throttles = [
            ("congestion", self.stats.throttle_congestion),
            ("cooldown", self.stats.throttle_cooldown),
            ("emergency", self.stats.throttle_emergency),
            ("remote_quota", self.stats.throttle_remote),
            ("descriptor_quota", self.stats.throttle_descriptor),
            ("descriptor_replay", self.stats.throttle_descriptor_replay),
        ];
        for (scope, value) in throttles {
            if value == 0 {
                continue;
            }
            let _ = writeln!(
                output,
                "soranet_privacy_throttles_total{{mode=\"{mode}\",bucket_start=\"{bucket}\",scope=\"{scope}\"}} {value}",
                mode = mode.as_label(),
                bucket = bucket_label,
                scope = scope,
                value = value,
            );
        }

        if self.stats.cooldown_count > 0 {
            let _ = writeln!(
                output,
                "soranet_privacy_throttle_cooldown_millis_sum{{mode=\"{mode}\",bucket_start=\"{bucket}\"}} {sum}",
                mode = mode.as_label(),
                bucket = bucket_label,
                sum = self.stats.cooldown_millis_sum,
            );
            let _ = writeln!(
                output,
                "soranet_privacy_throttle_cooldown_millis_count{{mode=\"{mode}\",bucket_start=\"{bucket}\"}} {count}",
                mode = mode.as_label(),
                bucket = bucket_label,
                count = self.stats.cooldown_count,
            );
        }

        if let Some(avg) = self.stats.active_avg {
            let _ = writeln!(
                output,
                "soranet_privacy_active_circuits_avg{{mode=\"{mode}\",bucket_start=\"{bucket}\"}} {avg}",
                mode = mode.as_label(),
                bucket = bucket_label,
                avg = avg,
            );
        }
        if let Some(max) = self.stats.active_max {
            let _ = writeln!(
                output,
                "soranet_privacy_active_circuits_max{{mode=\"{mode}\",bucket_start=\"{bucket}\"}} {max}",
                mode = mode.as_label(),
                bucket = bucket_label,
                max = max,
            );
        }

        if self.stats.bytes_verified > 0 {
            let _ = writeln!(
                output,
                "soranet_privacy_verified_bytes_total{{mode=\"{mode}\",bucket_start=\"{bucket}\"}} {value}",
                mode = mode.as_label(),
                bucket = bucket_label,
                value = self.stats.bytes_verified,
            );
        }

        for (percentile, value) in &self.stats.rtt_percentiles {
            let _ = writeln!(
                output,
                "soranet_privacy_rtt_millis{{mode=\"{mode}\",bucket_start=\"{bucket}\",percentile=\"{percentile}\"}} {value}",
                mode = mode.as_label(),
                bucket = bucket_label,
                percentile = percentile,
                value = value,
            );
        }

        for (hash, count) in &self.stats.gar_counts {
            let _ = writeln!(
                output,
                "soranet_privacy_gar_reports_total{{mode=\"{mode}\",bucket_start=\"{bucket}\",category_hash=\"{hash}\"}} {count}",
                mode = mode.as_label(),
                bucket = bucket_label,
                hash = hash,
                count = count,
            );
        }
    }
}

impl PrivacyAggregator {
    /// Create a new privacy aggregator using the supplied configuration.
    pub fn new(config: PrivacyConfig) -> Self {
        let mut config = normalize_config(config);
        let mut state = PrivacyState::default();
        if state
            .completed
            .try_reserve_exact(config.max_completed_buckets)
            .is_err()
        {
            config.max_completed_buckets = 0;
        }
        Self {
            config,
            state: Mutex::new(state),
        }
    }

    /// Record an accepted circuit handshake.
    pub fn record_circuit_accepted(
        &self,
        when: SystemTime,
        rtt_millis: Option<u64>,
        active_after: Option<u64>,
    ) {
        self.with_bucket(when, |bucket| {
            bucket.record_handshake_success(rtt_millis, active_after);
        });
    }

    /// Record a rejected circuit handshake.
    pub fn record_circuit_rejected(
        &self,
        when: SystemTime,
        reason: RejectReason,
        rtt_millis: Option<u64>,
    ) {
        self.with_bucket(when, |bucket| {
            bucket.record_handshake_failure(reason, rtt_millis);
        });
    }

    /// Record a throttling decision scoped to the supplied category.
    pub fn record_throttle(&self, when: SystemTime, scope: ThrottleScope) {
        self.with_bucket(when, |bucket| bucket.record_throttle(scope));
    }

    /// Record the cooldown associated with a throttle.
    pub fn record_throttle_cooldown(&self, when: SystemTime, cooldown: Duration) {
        if cooldown.is_zero() {
            return;
        }
        self.with_bucket(when, |bucket| bucket.record_throttle_cooldown(cooldown));
    }

    /// Record a capacity rejection originating from congestion limits.
    pub fn record_capacity_reject(&self, when: SystemTime) {
        self.with_bucket(when, BucketStats::record_capacity_reject);
    }

    /// Record an instantaneous snapshot of active circuits outside handshake paths.
    pub fn record_active_sample(&self, when: SystemTime, active_circuits: u64) {
        self.with_bucket(when, |bucket| bucket.record_active_sample(active_circuits));
    }

    /// Record the amount of verified bytes relayed by anonymity circuits.
    pub fn record_verified_bytes(&self, when: SystemTime, bytes: u128) {
        if bytes == 0 {
            return;
        }
        self.with_bucket(when, |bucket| bucket.record_verified_bytes(bytes));
    }

    /// Record a GAR abuse category using a privacy-preserving hash.
    pub fn record_gar_category(&self, when: SystemTime, category: &str) {
        if let Some(hash) = gar_category_hash(category) {
            self.with_bucket(when, move |bucket| bucket.record_gar_category(hash.clone()));
        }
    }

    /// Render Prometheus metrics for completed buckets as of the supplied timestamp.
    pub fn render_prometheus(&self, mode: RelayMode, now: SystemTime) -> String {
        let bucket_secs = self.config.bucket_secs;
        let mut state = self
            .state
            .lock()
            .expect("soranet privacy aggregator mutex poisoned");
        let current_idx = bucket_index(now, bucket_secs);
        state.flush_ready(current_idx, &self.config);
        let maximum = match state
            .completed
            .len()
            .checked_mul(PRIVACY_PROMETHEUS_MAX_BYTES_PER_BUCKET_V1)
        {
            Some(maximum) => maximum,
            None => return String::new(),
        };
        let mut output = BoundedText::new(maximum);
        for bucket in &state.completed {
            bucket.render_prometheus(&mut output, mode, bucket_secs);
        }
        output.into_string()
    }

    fn with_bucket<F>(&self, when: SystemTime, mut update: F)
    where
        F: FnMut(&mut BucketStats),
    {
        let mut state = self
            .state
            .lock()
            .expect("soranet privacy aggregator mutex poisoned");
        let bucket_idx = bucket_index(when, self.config.bucket_secs);
        if !state.open.contains_key(&bucket_idx)
            && state.open.len()
                >= usize::try_from(PRIVACY_MAX_OPEN_BUCKETS_V1).unwrap_or(usize::MAX)
        {
            return;
        }
        let bucket = state.open.entry(bucket_idx).or_default();
        update(bucket);
        state.flush_ready(bucket_idx, &self.config);
    }
}

impl PrivacyState {
    fn flush_ready(&mut self, current_idx: u64, config: &PrivacyConfig) {
        if config.bucket_secs == 0 {
            return;
        }
        let mut ready = Vec::new();
        if ready.try_reserve_exact(self.open.len()).is_err() {
            return;
        }
        for (&bucket_idx, stats) in self.open.iter() {
            let age = current_idx.saturating_sub(bucket_idx);
            let meets_delay = age >= config.flush_delay_buckets;
            let force_flush = age >= config.force_flush_buckets;
            if !meets_delay && !force_flush {
                break;
            }
            let contributors = stats.handshake_events();
            if meets_delay && contributors >= config.min_handshakes {
                ready.push((bucket_idx, false));
            } else if force_flush {
                ready.push((bucket_idx, true));
            }
        }

        for (bucket_idx, suppressed) in ready {
            if let Some(stats) = self.open.remove(&bucket_idx) {
                let summary = stats.into_summary(suppressed);
                let completed = CompletedBucket {
                    start_bucket: bucket_idx,
                    stats: summary,
                };
                self.push_completed(completed, config.max_completed_buckets);
            }
        }
    }

    fn push_completed(&mut self, bucket: CompletedBucket, max_completed: usize) {
        if max_completed == 0 {
            return;
        }
        while self.completed.len() >= max_completed {
            self.completed.pop_front();
        }
        if self.completed.len() == self.completed.capacity()
            && self.completed.try_reserve_exact(1).is_err()
        {
            return;
        }
        self.completed.push_back(bucket);
    }
}

impl BucketStats {
    fn record_handshake_success(&mut self, rtt_millis: Option<u64>, active_after: Option<u64>) {
        self.handshake_success = self.handshake_success.saturating_add(1);
        if let Some(millis) = rtt_millis {
            self.rtt.observe(millis);
        }
        if let Some(active) = active_after {
            self.active.record(active);
        }
    }

    fn record_handshake_failure(&mut self, reason: RejectReason, rtt_millis: Option<u64>) {
        match reason {
            RejectReason::Pow => {
                self.handshake_pow_rejects = self.handshake_pow_rejects.saturating_add(1);
            }
            RejectReason::Timeout => {
                self.handshake_timeouts = self.handshake_timeouts.saturating_add(1);
            }
            RejectReason::Downgrade => {
                self.handshake_downgrades = self.handshake_downgrades.saturating_add(1);
            }
            RejectReason::Other => {
                self.handshake_other_failures = self.handshake_other_failures.saturating_add(1);
            }
        }
        if let Some(millis) = rtt_millis {
            self.rtt.observe(millis);
        }
    }

    fn record_throttle(&mut self, scope: ThrottleScope) {
        match scope {
            ThrottleScope::Congestion => {
                self.throttle_congestion = self.throttle_congestion.saturating_add(1);
            }
            ThrottleScope::Cooldown => {
                self.throttle_cooldown = self.throttle_cooldown.saturating_add(1);
            }
            ThrottleScope::Emergency => {
                self.throttle_emergency = self.throttle_emergency.saturating_add(1);
            }
            ThrottleScope::RemoteQuota => {
                self.throttle_remote = self.throttle_remote.saturating_add(1);
            }
            ThrottleScope::DescriptorQuota => {
                self.throttle_descriptor = self.throttle_descriptor.saturating_add(1);
            }
            ThrottleScope::DescriptorReplay => {
                self.throttle_descriptor_replay = self.throttle_descriptor_replay.saturating_add(1);
            }
        }
    }

    fn record_throttle_cooldown(&mut self, cooldown: Duration) {
        let millis = cooldown.as_millis();
        if millis == 0 {
            return;
        }
        self.throttle_cooldown_sum_millis =
            self.throttle_cooldown_sum_millis.saturating_add(millis);
        self.throttle_cooldown_count = self.throttle_cooldown_count.saturating_add(1);
    }

    fn record_capacity_reject(&mut self) {
        self.capacity_rejects = self.capacity_rejects.saturating_add(1);
    }

    fn record_active_sample(&mut self, sample: u64) {
        self.active.record(sample);
    }

    fn record_verified_bytes(&mut self, bytes: u128) {
        self.bytes_verified = self.bytes_verified.saturating_add(bytes);
    }

    fn record_gar_category(&mut self, hash: String) {
        if let Some(entry) = self.gar_counts.get_mut(&hash) {
            *entry = entry.saturating_add(1);
        } else if self.gar_counts.len() < PRIVACY_GAR_CATEGORIES_PER_BUCKET_MAX_V1 {
            self.gar_counts.insert(hash, 1);
        }
    }

    fn handshake_events(&self) -> u64 {
        self.handshake_success
            .saturating_add(self.handshake_pow_rejects)
            .saturating_add(self.handshake_downgrades)
            .saturating_add(self.handshake_timeouts)
            .saturating_add(self.handshake_other_failures)
            .saturating_add(self.capacity_rejects)
    }

    fn into_summary(self, suppressed: bool) -> BucketSummary {
        let Self {
            handshake_success,
            handshake_pow_rejects,
            handshake_downgrades,
            handshake_timeouts,
            handshake_other_failures,
            capacity_rejects,
            throttle_congestion,
            throttle_cooldown,
            throttle_emergency,
            throttle_remote,
            throttle_descriptor,
            throttle_descriptor_replay,
            throttle_cooldown_sum_millis,
            throttle_cooldown_count,
            rtt,
            active,
            bytes_verified,
            gar_counts,
        } = self;

        let rtt_percentiles = rtt.percentiles();
        let (active_avg, active_max) = active.summary();

        BucketSummary {
            handshake_success,
            handshake_pow_rejects,
            handshake_downgrades,
            handshake_timeouts,
            handshake_other_failures,
            capacity_rejects,
            throttle_congestion,
            throttle_cooldown,
            throttle_emergency,
            throttle_remote,
            throttle_descriptor,
            throttle_descriptor_replay,
            cooldown_millis_sum: throttle_cooldown_sum_millis,
            cooldown_count: throttle_cooldown_count,
            active_avg,
            active_max,
            bytes_verified,
            rtt_percentiles,
            gar_counts,
            suppressed,
        }
    }
}

fn bucket_index(timestamp: SystemTime, bucket_secs: u64) -> u64 {
    if bucket_secs == 0 {
        return 0;
    }
    timestamp
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() / bucket_secs)
        .unwrap_or(0)
}

fn gar_category_hash(category: &str) -> Option<String> {
    let trimmed = category.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut hasher = Blake3Hasher::new();
    hasher.update(trimmed.as_bytes());
    let digest = hasher.finalize();
    let mut truncated = [0u8; 8];
    truncated.copy_from_slice(&digest.as_bytes()[..8]);
    Some(truncated.encode_hex::<String>())
}

fn unix_seconds(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn normalize_config(mut config: PrivacyConfig) -> PrivacyConfig {
    if config.bucket_secs == 0 {
        config.bucket_secs = PrivacyConfig::default().bucket_secs;
    }
    if config.max_completed_buckets == 0 {
        config.max_completed_buckets = PrivacyConfig::default().max_completed_buckets;
    }
    config.max_completed_buckets = config
        .max_completed_buckets
        .min(PRIVACY_MAX_COMPLETED_BUCKETS_V1);
    if config.expected_shares == 0 {
        config.expected_shares = PrivacyConfig::default().expected_shares;
    }
    if config.event_buffer_capacity == 0 {
        config.event_buffer_capacity = PrivacyConfig::default().event_buffer_capacity;
    }
    config.event_buffer_capacity = config
        .event_buffer_capacity
        .min(PRIVACY_EVENT_BUFFER_MAX_CAPACITY_V1);
    config.flush_delay_buckets = config.flush_delay_buckets.min(PRIVACY_MAX_OPEN_BUCKETS_V1);
    config.force_flush_buckets = config.force_flush_buckets.min(PRIVACY_MAX_OPEN_BUCKETS_V1);
    if config.force_flush_buckets < config.flush_delay_buckets {
        config.force_flush_buckets = config.flush_delay_buckets;
    }
    config
}

impl From<&PrivacyTelemetryConfig> for PrivacyConfig {
    fn from(config: &PrivacyTelemetryConfig) -> Self {
        Self {
            bucket_secs: config.bucket_secs,
            min_handshakes: config.min_handshakes,
            max_completed_buckets: config.max_completed_buckets,
            flush_delay_buckets: config.flush_delay_buckets,
            force_flush_buckets: config.force_flush_buckets,
            expected_shares: config.expected_shares,
            event_buffer_capacity: config.event_buffer_capacity,
        }
    }
}

impl From<PrivacyTelemetryConfig> for PrivacyConfig {
    fn from(config: PrivacyTelemetryConfig) -> Self {
        Self::from(&config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_time() -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(1_000)
    }

    #[test]
    fn renders_metrics_when_threshold_met() {
        let config = PrivacyConfig {
            min_handshakes: 1,
            ..PrivacyConfig::default()
        };
        let bucket_secs = config.bucket_secs.max(1);
        let aggregator = PrivacyAggregator::new(config);

        let bucket_start = base_time();
        aggregator.record_circuit_accepted(bucket_start, Some(42), Some(7));
        aggregator.record_throttle(bucket_start, ThrottleScope::Cooldown);
        aggregator.record_throttle_cooldown(bucket_start, Duration::from_millis(500));
        aggregator.record_verified_bytes(bucket_start, 1_024);
        aggregator.record_gar_category(bucket_start, "abuse.spam");
        aggregator.record_active_sample(bucket_start, 9);

        let render_time = bucket_start + Duration::from_secs(bucket_secs.saturating_mul(2));
        let output = aggregator.render_prometheus(RelayMode::Entry, render_time);
        assert!(
            output.contains("soranet_privacy_circuit_events_total"),
            "expected handshake counter in metrics: {output}"
        );
        assert!(
            output.contains("soranet_privacy_active_circuits_avg"),
            "expected active circuit average in metrics: {output}"
        );
        assert!(
            output.contains("soranet_privacy_verified_bytes_total"),
            "expected verified bytes counter in metrics: {output}"
        );
        assert!(
            output.contains("soranet_privacy_gar_reports_total"),
            "expected GAR hash counter in metrics: {output}"
        );
    }

    #[test]
    fn renders_emergency_scope_metric() {
        let config = PrivacyConfig {
            min_handshakes: 1,
            ..PrivacyConfig::default()
        };
        let bucket_secs = config.bucket_secs.max(1);
        let aggregator = PrivacyAggregator::new(config);

        let bucket_start = base_time();
        aggregator.record_circuit_accepted(bucket_start, None, None);
        aggregator.record_throttle(bucket_start, ThrottleScope::Emergency);

        let render_time = bucket_start + Duration::from_secs(bucket_secs.saturating_mul(2));
        let output = aggregator.render_prometheus(RelayMode::Middle, render_time);
        assert!(
            output.contains("scope=\"emergency\""),
            "expected emergency throttle scope in metrics: {output}"
        );
    }

    #[test]
    fn suppressed_bucket_emits_marker() {
        let config = PrivacyConfig {
            min_handshakes: 5,
            flush_delay_buckets: 1,
            force_flush_buckets: 1,
            ..PrivacyConfig::default()
        };
        let bucket_secs = config.bucket_secs.max(1);
        let aggregator = PrivacyAggregator::new(config);

        let bucket_start = base_time();
        aggregator.record_circuit_accepted(bucket_start, Some(5), None);

        let render_time = bucket_start + Duration::from_secs(bucket_secs.saturating_mul(2));
        let output = aggregator.render_prometheus(RelayMode::Exit, render_time);
        assert!(
            output.contains("soranet_privacy_bucket_suppressed"),
            "expected suppressed marker in metrics: {output}"
        );
        assert!(
            !output.contains("soranet_privacy_circuit_events_total"),
            "suppressed bucket should not expose counters: {output}"
        );
    }

    #[test]
    fn event_buffer_serialises_ndjson() {
        let buffer = PrivacyEventBuffer::new(4);
        let mode = SoranetPrivacyModeV1::Entry;
        let when = base_time();

        buffer.record_handshake_success(mode, when, Some(12), Some(3));
        buffer.record_handshake_failure(
            mode,
            when,
            SoranetPrivacyHandshakeFailureV1::Downgrade,
            Some("suite_no_overlap"),
            Some(24),
        );
        buffer.record_throttle(mode, when, SoranetPrivacyThrottleScopeV1::Congestion);
        buffer.record_throttle(mode, when, SoranetPrivacyThrottleScopeV1::Emergency);

        assert_eq!(buffer.queue_depth(), 4, "queue depth must not drain events");

        let body = buffer.drain_ndjson();
        let lines: Vec<&str> = body.trim_end().split('\n').collect();
        assert_eq!(lines.len(), 4, "expected four NDJSON entries: {body}");
        assert!(
            lines.iter().any(|line| line.contains("HandshakeSuccess")),
            "handshake success should serialise into NDJSON: {body}"
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("\"detail\":\"suite_no_overlap\"")),
            "downgrade detail should show up in NDJSON: {body}"
        );
        assert!(
            lines.iter().any(|line| line.contains("Throttle")),
            "throttle event should serialise into NDJSON: {body}"
        );
        assert!(
            lines
                .iter()
                .filter(|line| line.contains("\"Throttle\""))
                .any(|line| line.contains("\"emergency\"")),
            "emergency throttle scope should be encoded explicitly: {body}"
        );
        assert!(buffer.drain_ndjson().is_empty(), "buffer should drain");
        assert_eq!(buffer.queue_depth(), 0, "drain must empty the queue");
    }

    #[test]
    fn proxy_policy_buffer_trims_details_and_caps_queue() {
        let buffer = ProxyPolicyEventBuffer::new(2);
        let mode = SoranetPrivacyModeV1::Middle;
        let when = base_time();

        buffer.record_downgrade(mode, when, Some("  constant-rate capability missing  "));
        buffer.record_downgrade(mode, when, Some("   "));
        assert_eq!(buffer.queue_depth(), 2, "queue depth respects capacity");

        let body = buffer.drain_ndjson();
        let lines: Vec<&str> = body.trim_end().split('\n').collect();
        assert_eq!(
            lines.len(),
            2,
            "expected ndjson entries for both downgrades"
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("\"detail\":\"constant-rate capability missing\"")),
            "trimmed detail should be preserved in NDJSON: {body}"
        );
        assert!(
            lines.iter().any(|line| line.contains("\"detail\":null")),
            "blank detail should serialise as null: {body}"
        );
        assert_eq!(buffer.queue_depth(), 0, "drain must empty queue");

        buffer.record_downgrade(mode, when, Some("first"));
        buffer.record_downgrade(mode, when, Some("second"));
        buffer.record_downgrade(mode, when, Some("third"));
        assert_eq!(buffer.queue_depth(), 2, "oldest downgrade must be evicted");
        let truncated = buffer.drain_ndjson();
        assert!(
            !truncated.contains("first"),
            "oldest downgrade should be dropped when capacity exceeded: {truncated}"
        );
        assert!(
            truncated.contains("second") && truncated.contains("third"),
            "newest downgrades must remain in buffer: {truncated}"
        );
    }

    #[test]
    fn gar_hash_trims_input() {
        let hash = gar_category_hash("  Policy::Spam  ").expect("hash generated");
        let hash_again = gar_category_hash("Policy::Spam").expect("hash generated");
        assert_eq!(hash, hash_again);
    }

    #[test]
    fn converts_from_telemetry_config() {
        let telemetry = PrivacyTelemetryConfig {
            bucket_secs: 90,
            min_handshakes: 7,
            flush_delay_buckets: 2,
            force_flush_buckets: 5,
            max_completed_buckets: 20,
            expected_shares: 3,
            event_buffer_capacity: 2_048,
        };
        let config: PrivacyConfig = telemetry.into();
        assert_eq!(config.bucket_secs, 90);
        assert_eq!(config.min_handshakes, 7);
        assert_eq!(config.flush_delay_buckets, 2);
        assert_eq!(config.force_flush_buckets, 5);
        assert_eq!(config.max_completed_buckets, 20);
        assert_eq!(config.expected_shares, 3);
        assert_eq!(config.event_buffer_capacity, 2_048);
    }

    #[test]
    fn programmatic_privacy_limits_are_clamped_before_allocation() {
        let aggregator = PrivacyAggregator::new(PrivacyConfig {
            flush_delay_buckets: u64::MAX,
            force_flush_buckets: u64::MAX,
            max_completed_buckets: usize::MAX,
            event_buffer_capacity: usize::MAX,
            ..PrivacyConfig::default()
        });
        assert_eq!(
            aggregator.config.max_completed_buckets,
            PRIVACY_MAX_COMPLETED_BUCKETS_V1
        );
        assert_eq!(
            aggregator.config.event_buffer_capacity,
            PRIVACY_EVENT_BUFFER_MAX_CAPACITY_V1
        );
        assert_eq!(
            aggregator.config.flush_delay_buckets,
            PRIVACY_MAX_OPEN_BUCKETS_V1
        );
        assert_eq!(
            aggregator.config.force_flush_buckets,
            PRIVACY_MAX_OPEN_BUCKETS_V1
        );

        let buffer = PrivacyEventBuffer::new(usize::MAX);
        assert!(
            buffer.max_events == 0 || buffer.max_events == PRIVACY_EVENT_BUFFER_MAX_CAPACITY_V1
        );
        let proxy = ProxyPolicyEventBuffer::new(usize::MAX);
        assert!(proxy.max_events == 0 || proxy.max_events == PRIVACY_EVENT_BUFFER_MAX_CAPACITY_V1);
    }

    #[test]
    fn privacy_detail_and_category_retention_are_bounded() {
        let detail = "é".repeat(PRIVACY_EVENT_DETAIL_MAX_BYTES_V1);
        let retained = detail_to_string(Some(&detail)).expect("bounded detail");
        assert_eq!(retained.len(), PRIVACY_EVENT_DETAIL_MAX_BYTES_V1);
        assert!(retained.is_char_boundary(retained.len()));

        let mut bucket = BucketStats::default();
        for index in 0..=PRIVACY_GAR_CATEGORIES_PER_BUCKET_MAX_V1 {
            bucket.record_gar_category(format!("{index:016x}"));
        }
        assert_eq!(
            bucket.gar_counts.len(),
            PRIVACY_GAR_CATEGORIES_PER_BUCKET_MAX_V1
        );
    }

    #[test]
    fn open_bucket_retention_stops_at_the_first_release_limit() {
        let aggregator = PrivacyAggregator::new(PrivacyConfig {
            bucket_secs: 1,
            min_handshakes: u64::MAX,
            flush_delay_buckets: PRIVACY_MAX_OPEN_BUCKETS_V1,
            force_flush_buckets: PRIVACY_MAX_OPEN_BUCKETS_V1,
            ..PrivacyConfig::default()
        });
        for bucket in (0..=PRIVACY_MAX_OPEN_BUCKETS_V1).rev() {
            aggregator.record_capacity_reject(UNIX_EPOCH + Duration::from_secs(bucket));
        }
        let state = aggregator.state.lock().expect("privacy state");
        assert_eq!(
            state.open.len(),
            usize::try_from(PRIVACY_MAX_OPEN_BUCKETS_V1).expect("fixed limit fits usize")
        );
    }
}
