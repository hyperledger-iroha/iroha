//! Privacy-preserving telemetry aggregation for the `SoraNet` anonymity layer.
//!
//! The SNNet-8 roadmap item introduces a secure aggregation pipeline that
//! collects relay handshake telemetry while preserving end-user anonymity.
//! Runtime components feed circuit successes/failures, throttling decisions,
//! RTT samples, verified byte counters, and GAR abuse reports into this
//! aggregator. Statistics are only emitted when enough contributors populate a
//! bucket; otherwise the bucket is surfaced as suppressed so downstream
//! dashboards can reason about withheld windows without leaking per-relay
//! details.

use std::{
    collections::{BTreeMap, VecDeque, btree_map::Entry},
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use blake3::Hasher as Blake3Hasher;
use iroha_data_model::soranet::privacy_metrics::{
    SoranetGarAbuseCountV1, SoranetGarAbuseShareV1, SoranetLatencyPercentileV1,
    SoranetPowFailureCountV1, SoranetPowFailureReasonV1, SoranetPrivacyBucketMetricsV1,
    SoranetPrivacyEventKindV1, SoranetPrivacyEventV1, SoranetPrivacyHandshakeFailureV1,
    SoranetPrivacyModeV1, SoranetPrivacyPrioShareV1, SoranetPrivacySuppressionReasonV1,
    SoranetPrivacyThrottleScopeV1,
};
use norito::json;
use thiserror::Error;

/// Percentiles exposed for RTT measurements.
/// Percentile configuration expressed as a rational ratio.
#[derive(Clone, Copy)]
struct PercentileSpec {
    label: &'static str,
    numerator: u64,
    denominator: u64,
}

const RTT_PERCENTILES: &[PercentileSpec] = &[
    PercentileSpec {
        label: "p50",
        numerator: 1,
        denominator: 2,
    },
    PercentileSpec {
        label: "p90",
        numerator: 9,
        denominator: 10,
    },
    PercentileSpec {
        label: "p99",
        numerator: 99,
        denominator: 100,
    },
];

/// Histogram bucket bounds (inclusive) for RTT observations, measured in milliseconds.
const RTT_BUCKET_BOUNDS_MS: &[u64] = &[
    10, 25, 50, 75, 100, 150, 200, 300, 500, 750, 1000, 1500, 2000, 2500, 3000,
];

/// Number of histogram buckets derived from [`RTT_BUCKET_BOUNDS_MS`].
const RTT_BUCKET_COUNT: usize = RTT_BUCKET_BOUNDS_MS.len() + 1;

fn pow_reason_from_detail(detail: Option<&str>) -> SoranetPowFailureReasonV1 {
    let Some(raw) = detail else {
        return SoranetPowFailureReasonV1::InvalidSolution;
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return SoranetPowFailureReasonV1::InvalidSolution;
    }
    match trimmed {
        label
            if label
                .eq_ignore_ascii_case(SoranetPowFailureReasonV1::DifficultyMismatch.as_label()) =>
        {
            SoranetPowFailureReasonV1::DifficultyMismatch
        }
        label if label.eq_ignore_ascii_case(SoranetPowFailureReasonV1::Expired.as_label()) => {
            SoranetPowFailureReasonV1::Expired
        }
        label
            if label
                .eq_ignore_ascii_case(SoranetPowFailureReasonV1::FutureSkewExceeded.as_label()) =>
        {
            SoranetPowFailureReasonV1::FutureSkewExceeded
        }
        label if label.eq_ignore_ascii_case(SoranetPowFailureReasonV1::TtlTooShort.as_label()) => {
            SoranetPowFailureReasonV1::TtlTooShort
        }
        label
            if label
                .eq_ignore_ascii_case(SoranetPowFailureReasonV1::UnsupportedVersion.as_label()) =>
        {
            SoranetPowFailureReasonV1::UnsupportedVersion
        }
        label
            if label
                .eq_ignore_ascii_case(SoranetPowFailureReasonV1::SignatureInvalid.as_label()) =>
        {
            SoranetPowFailureReasonV1::SignatureInvalid
        }
        label
            if label
                .eq_ignore_ascii_case(SoranetPowFailureReasonV1::PostQuantumError.as_label()) =>
        {
            SoranetPowFailureReasonV1::PostQuantumError
        }
        label if label.eq_ignore_ascii_case(SoranetPowFailureReasonV1::ClockError.as_label()) => {
            SoranetPowFailureReasonV1::ClockError
        }
        label
            if label.eq_ignore_ascii_case(SoranetPowFailureReasonV1::RelayMismatch.as_label()) =>
        {
            SoranetPowFailureReasonV1::RelayMismatch
        }
        label if label.eq_ignore_ascii_case(SoranetPowFailureReasonV1::Replay.as_label()) => {
            SoranetPowFailureReasonV1::Replay
        }
        label if label.eq_ignore_ascii_case(SoranetPowFailureReasonV1::StoreError.as_label()) => {
            SoranetPowFailureReasonV1::StoreError
        }
        _ => SoranetPowFailureReasonV1::InvalidSolution,
    }
}

/// Configuration for the privacy-preserving aggregation pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrivacyBucketConfig {
    /// Width of a bucket in seconds.
    pub bucket_secs: u64,
    /// Minimum number of contributing handshake events required before a bucket is emitted.
    pub min_contributors: u64,
    /// Number of completed bucket intervals to wait before flushing a bucket that meets the threshold.
    pub flush_delay_buckets: u64,
    /// Hard limit after which a bucket is emitted as suppressed even if it did not meet the threshold.
    pub force_flush_buckets: u64,
    /// Maximum number of completed buckets retained before draining.
    pub max_completed_buckets: usize,
    /// Number of Prio collector shares required before emitting a combined bucket.
    pub expected_shares: u16,
    /// Maximum bucket lag allowed for collector shares before the bucket is suppressed.
    pub max_share_lag_buckets: u64,
}

impl Default for PrivacyBucketConfig {
    fn default() -> Self {
        Self {
            bucket_secs: 60,
            min_contributors: 12,
            flush_delay_buckets: 1,
            force_flush_buckets: 6,
            max_completed_buckets: 120,
            expected_shares: 2,
            max_share_lag_buckets: 12,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, time::Duration};

    use iroha_data_model::soranet::privacy_metrics::SoranetPrivacyModeV1;

    use super::*;

    fn base_config() -> PrivacyBucketConfig {
        PrivacyBucketConfig {
            bucket_secs: 1,
            min_contributors: 2,
            flush_delay_buckets: 1,
            force_flush_buckets: 3,
            max_completed_buckets: 8,
            expected_shares: 2,
            max_share_lag_buckets: 3,
        }
    }

    #[test]
    fn snapshot_reports_open_buckets() {
        let aggregator =
            SoranetSecureAggregator::new(base_config()).expect("config should be valid");
        let start = SystemTime::UNIX_EPOCH + Duration::from_secs(1);
        aggregator.record_handshake_success(SoranetPrivacyModeV1::Entry, start, None, None);

        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(2);
        let (drained, snapshot) = aggregator.drain_ready_with_snapshot(now);
        assert!(
            drained.is_empty(),
            "no buckets should drain before reaching min contributors"
        );
        assert_eq!(snapshot.drained_buckets, 0);
        assert_eq!(snapshot.evicted_completed, 0);
        assert!(
            snapshot.suppressed_counts.is_empty(),
            "no suppressed buckets expected with empty drain"
        );
        assert_eq!(
            snapshot
                .open_buckets
                .get(&SoranetPrivacyModeV1::Entry)
                .copied()
                .unwrap_or(0),
            1,
            "entry mode open bucket count should be tracked"
        );
    }

    #[test]
    fn snapshot_tracks_evicted_completed_buckets() {
        let mut config = base_config();
        config.min_contributors = 1;
        config.max_completed_buckets = 1;
        config.force_flush_buckets = 2;
        let aggregator = SoranetSecureAggregator::new(config).expect("config should be valid");

        let first = SystemTime::UNIX_EPOCH + Duration::from_secs(1);
        let second = SystemTime::UNIX_EPOCH + Duration::from_secs(2);
        let third = SystemTime::UNIX_EPOCH + Duration::from_secs(3);
        for when in [first, second, third] {
            aggregator.record_handshake_success(SoranetPrivacyModeV1::Entry, when, None, None);
        }

        let (drained, snapshot) = aggregator.drain_ready_with_snapshot(third);
        assert_eq!(
            drained.len(),
            1,
            "only the newest completed bucket should remain"
        );
        assert_eq!(
            snapshot.evicted_completed, 1,
            "retention policy should evict the oldest completed bucket"
        );
        assert!(
            snapshot.suppressed_counts.is_empty(),
            "force flush not triggered so no suppressed buckets"
        );
        assert_eq!(
            snapshot
                .open_buckets
                .get(&SoranetPrivacyModeV1::Entry)
                .copied()
                .unwrap_or(0),
            1,
            "the newest bucket still collecting contributors should stay open"
        );
    }

    #[test]
    fn snapshot_reports_suppressed_counts() {
        let mut config = base_config();
        config.min_contributors = 2;
        config.flush_delay_buckets = 1;
        config.force_flush_buckets = 2;
        let aggregator = SoranetSecureAggregator::new(config).expect("config should be valid");

        let bucket_time = SystemTime::UNIX_EPOCH + Duration::from_secs(1);
        aggregator.record_handshake_success(SoranetPrivacyModeV1::Exit, bucket_time, None, None);

        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(4);
        let (drained, snapshot) = aggregator.drain_ready_with_snapshot(now);
        assert_eq!(drained.len(), 1, "forced flush should drain the bucket");
        assert_eq!(snapshot.drained_buckets, 1);
        let forced_flush_count = snapshot
            .suppressed_counts
            .get(&SoranetPrivacySuppressionReasonV1::ForcedFlushWindowElapsed)
            .copied()
            .unwrap_or(0);
        assert_eq!(
            forced_flush_count, 1,
            "suppressed map tracks forced flushes"
        );
        let forced_flush_by_mode = snapshot
            .suppressed_by_mode
            .get(&SoranetPrivacyModeV1::Exit)
            .and_then(|reason_map| {
                reason_map
                    .get(&SoranetPrivacySuppressionReasonV1::ForcedFlushWindowElapsed)
                    .copied()
            })
            .unwrap_or(0);
        assert_eq!(
            forced_flush_by_mode, 1,
            "per-mode suppressed map tracks forced flushes"
        );
    }

    #[test]
    fn verified_bytes_accumulate_in_bucket_metrics() {
        let aggregator =
            SoranetSecureAggregator::new(base_config()).expect("config should be valid");
        let bucket_time = SystemTime::UNIX_EPOCH + Duration::from_secs(1);
        // Meet the contributor threshold so the bucket drains normally.
        aggregator.record_handshake_success(SoranetPrivacyModeV1::Entry, bucket_time, None, None);
        aggregator.record_handshake_success(SoranetPrivacyModeV1::Entry, bucket_time, None, None);
        // Track verified bytes, ensuring zero-byte samples are ignored.
        aggregator.record_verified_bytes(SoranetPrivacyModeV1::Entry, bucket_time, 2_048);
        aggregator.record_verified_bytes(SoranetPrivacyModeV1::Entry, bucket_time, 512);
        aggregator.record_verified_bytes(SoranetPrivacyModeV1::Entry, bucket_time, 0);

        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(3);
        let (drained, snapshot) = aggregator.drain_ready_with_snapshot(now);
        assert_eq!(drained.len(), 1, "bucket should drain once delay elapses");
        assert_eq!(snapshot.drained_buckets, 1);
        assert!(
            snapshot.suppressed_counts.is_empty(),
            "bucket met the contributor threshold"
        );

        let metrics = &drained[0];
        assert_eq!(metrics.mode, SoranetPrivacyModeV1::Entry);
        assert!(!metrics.suppressed, "bucket was emitted, not suppressed");
        assert_eq!(
            metrics.verified_bytes_total, 2_560,
            "verified byte totals should saturate within the bucket"
        );
    }

    #[test]
    fn drain_ready_flushes_across_modes() {
        let mut config = base_config();
        config.min_contributors = 1;
        config.flush_delay_buckets = 1;
        config.force_flush_buckets = 2;
        let aggregator = SoranetSecureAggregator::new(config).expect("config should be valid");

        let ready_time = SystemTime::UNIX_EPOCH + Duration::from_secs(8);
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
        aggregator.record_handshake_success(SoranetPrivacyModeV1::Entry, now, None, None);
        aggregator.record_handshake_success(SoranetPrivacyModeV1::Exit, ready_time, None, None);

        let (drained, snapshot) = aggregator.drain_ready_with_snapshot(now);
        assert_eq!(
            drained.len(),
            1,
            "ready buckets must drain even when earlier modes are not ready"
        );
        assert_eq!(drained[0].mode, SoranetPrivacyModeV1::Exit);
        assert_eq!(snapshot.drained_buckets, 1);
        assert_eq!(
            snapshot
                .open_buckets
                .get(&SoranetPrivacyModeV1::Entry)
                .copied()
                .unwrap_or(0),
            1
        );
    }

    #[test]
    fn gar_categories_are_hashed_and_counted() {
        let aggregator =
            SoranetSecureAggregator::new(base_config()).expect("config should be valid");
        let bucket_time = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
        aggregator.record_handshake_success(SoranetPrivacyModeV1::Exit, bucket_time, None, None);
        aggregator.record_handshake_success(SoranetPrivacyModeV1::Exit, bucket_time, None, None);
        aggregator.record_gar_category(SoranetPrivacyModeV1::Exit, bucket_time, "fraud");
        aggregator.record_gar_category(SoranetPrivacyModeV1::Exit, bucket_time, "fraud");
        aggregator.record_gar_category(SoranetPrivacyModeV1::Exit, bucket_time, "spam");
        aggregator.record_gar_category(SoranetPrivacyModeV1::Exit, bucket_time, "");
        aggregator.record_gar_category(SoranetPrivacyModeV1::Exit, bucket_time, "   ");

        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(15);
        let (drained, _) = aggregator.drain_ready_with_snapshot(now);
        assert_eq!(drained.len(), 1, "bucket should drain once delay elapses");

        let metrics = &drained[0];
        assert_eq!(metrics.mode, SoranetPrivacyModeV1::Exit);
        assert!(!metrics.suppressed);

        let fraud_hash = gar_category_hash("fraud");
        let spam_hash = gar_category_hash("spam");
        let mut counts = BTreeMap::new();
        for entry in &metrics.gar_abuse_counts {
            counts.insert(entry.category_hash, entry.count);
        }
        assert_eq!(
            counts.get(&fraud_hash),
            Some(&2),
            "identical labels should increment the same counter"
        );
        assert_eq!(
            counts.get(&spam_hash),
            Some(&1),
            "distinct labels should receive separate counters"
        );
        assert!(
            !counts.contains_key(&gar_category_hash("")),
            "empty labels must be ignored"
        );
    }
}

impl PrivacyBucketConfig {
    /// Validate the supplied configuration.
    ///
    /// # Errors
    ///
    /// Returns [`PrivacyConfigError`] if the configuration violates bucket constraints.
    pub fn validate(&self) -> Result<(), PrivacyConfigError> {
        if self.bucket_secs == 0 {
            return Err(PrivacyConfigError::ZeroBucketWidth);
        }
        if self.bucket_secs > u64::from(u32::MAX) {
            return Err(PrivacyConfigError::BucketWidthExceeds(self.bucket_secs));
        }
        if self.min_contributors == 0 {
            return Err(PrivacyConfigError::ZeroContributors);
        }
        if self.force_flush_buckets < self.flush_delay_buckets {
            return Err(PrivacyConfigError::ForceFlushLessThanDelay {
                flush_delay: self.flush_delay_buckets,
                force_flush: self.force_flush_buckets,
            });
        }
        if self.max_completed_buckets == 0 {
            return Err(PrivacyConfigError::ZeroCompletedCapacity);
        }
        if self.expected_shares == 0 {
            return Err(PrivacyConfigError::ZeroExpectedShares);
        }
        if self.max_share_lag_buckets == 0 {
            return Err(PrivacyConfigError::ZeroShareLagWindow);
        }
        Ok(())
    }
}

/// Errors surfaced when the privacy bucket configuration is invalid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PrivacyConfigError {
    /// Bucket width must be non-zero.
    #[error("bucket width must be non-zero")]
    ZeroBucketWidth,
    /// Bucket width exceeds the maximum representable duration.
    #[error("bucket width {0} seconds exceeds u32::MAX seconds")]
    BucketWidthExceeds(u64),
    /// At least one contributor is required before a bucket may be emitted.
    #[error("minimum contributors must be non-zero")]
    ZeroContributors,
    /// Force-flush configuration must not fire before the standard flush delay.
    #[error(
        "force flush ({force_flush} buckets) must not be smaller than flush delay ({flush_delay} buckets)"
    )]
    ForceFlushLessThanDelay {
        /// Number of buckets to wait before attempting a standard flush.
        flush_delay: u64,
        /// Number of buckets after which a force-flush is triggered.
        force_flush: u64,
    },
    /// Completed bucket retention must be non-zero.
    #[error("max_completed_buckets must be non-zero")]
    ZeroCompletedCapacity,
    /// Expected share count must be non-zero.
    #[error("expected_shares must be non-zero")]
    ZeroExpectedShares,
    /// Retention window for collector shares must be non-zero.
    #[error("max_share_lag_buckets must be non-zero")]
    ZeroShareLagWindow,
}

/// Errors surfaced when Prio collector shares are invalid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PrivacyShareError {
    /// Share referenced a bucket duration that differs from the configured width.
    #[error("bucket duration mismatch: expected {expected} seconds, received {received} seconds")]
    BucketDurationMismatch {
        /// Expected duration derived from the aggregator configuration.
        expected: u32,
        /// Duration advertised by the share.
        received: u32,
    },
    /// Share referenced a start timestamp that is not aligned to the configured bucket width.
    #[error("bucket start {bucket_start} is not aligned to {bucket_secs}-second buckets")]
    BucketAlignmentMismatch {
        /// Bucket start timestamp from the share.
        bucket_start: u64,
        /// Configured bucket width (seconds).
        bucket_secs: u64,
    },
    /// Share provided an RTT histogram with an unexpected number of buckets.
    #[error("RTT histogram length mismatch: expected {expected}, received {received}")]
    HistogramLengthMismatch {
        /// Expected number of histogram buckets.
        expected: usize,
        /// Histogram buckets provided by the share.
        received: usize,
    },
    /// Share aggregation yielded a negative result where only unsigned totals are allowed.
    #[error("negative aggregate in field `{field}`: {value}")]
    NegativeAggregate {
        /// Field name associated with the invalid aggregate.
        field: &'static str,
        /// Negative value encountered while combining shares.
        value: i128,
    },
    /// Share aggregation overflowed the supported numeric range.
    #[error("aggregate in field `{field}` exceeds supported range")]
    AggregateOverflow {
        /// Field that overflowed.
        field: &'static str,
    },
    /// Share disagreed on the relay mode for the bucket.
    #[error("share mode mismatch: expected {expected}, received {received}")]
    ModeMismatch {
        /// Mode observed in earlier shares for the bucket.
        expected: SoranetPrivacyModeV1,
        /// Mode advertised by the conflicting share.
        received: SoranetPrivacyModeV1,
    },
}

/// Errors encountered while ingesting relay privacy events.
#[derive(Debug, Error)]
pub enum PrivacyEventError {
    /// A line within the NDJSON payload failed to deserialize.
    #[error("failed to parse NDJSON line {line}: {source}")]
    InvalidNdjsonLine {
        /// 1-based line index.
        line: usize,
        /// Underlying JSON decode error.
        #[source]
        source: json::Error,
    },
}

/// Observed handshake failure reasons surfaced by the aggregator inputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::exhaustive_enums)]
pub enum HandshakeFailure {
    /// Proof-of-work validation rejected the ticket.
    Pow {
        /// Reason reported for the `PoW` rejection (best-effort).
        reason: SoranetPowFailureReasonV1,
    },
    /// The handshake exchange timed out.
    Timeout,
    /// Clients attempted to downgrade negotiated capabilities.
    Downgrade,
    /// Any other failure reason.
    Other,
}

/// Scope for throttling decisions surfaced to the aggregator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::exhaustive_enums)]
pub enum PrivacyThrottleScope {
    /// Congestion controller rejected the circuit.
    Congestion,
    /// Cooldown windows prevented new circuits from starting.
    Cooldown,
    /// Adaptive limits throttled a remote origin.
    RemoteQuota,
    /// Descriptor-level quota triggered the throttle.
    DescriptorQuota,
    /// Descriptor replay protection rejected the circuit.
    DescriptorReplay,
    /// Operator emergency stop throttled the circuit.
    Emergency,
}

/// Privacy-preserving aggregation pipeline that produces SNNet-8 metrics buckets.
#[derive(Debug)]
pub struct SoranetSecureAggregator {
    config: PrivacyBucketConfig,
    state: Mutex<PrivacyState>,
}

/// Snapshot of queue state returned alongside drained privacy buckets.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PrivacyDrainSnapshot {
    /// Number of buckets drained in this flush.
    pub drained_buckets: usize,
    /// Open buckets still accumulating contributors, keyed by relay mode.
    pub open_buckets: BTreeMap<SoranetPrivacyModeV1, usize>,
    /// Collector share accumulators still pending, keyed by relay mode.
    pub collector_backlog: BTreeMap<SoranetPrivacyModeV1, usize>,
    /// Buckets evicted due to completed-queue retention since the previous flush.
    pub evicted_completed: u64,
    /// Suppressed buckets drained in the last flush, keyed by suppression reason.
    pub suppressed_counts: BTreeMap<SoranetPrivacySuppressionReasonV1, u64>,
    /// Suppressed buckets keyed by relay mode and suppression reason.
    pub suppressed_by_mode:
        BTreeMap<SoranetPrivacyModeV1, BTreeMap<SoranetPrivacySuppressionReasonV1, u64>>,
}

impl SoranetSecureAggregator {
    /// Construct a new aggregator using the provided configuration.
    ///
    /// # Errors
    ///
    /// Returns [`PrivacyConfigError`] if the configuration fails validation.
    pub fn new(config: PrivacyBucketConfig) -> Result<Self, PrivacyConfigError> {
        config.validate()?;
        Ok(Self {
            config,
            state: Mutex::new(PrivacyState::default()),
        })
    }

    /// Access the current configuration.
    #[must_use]
    pub fn config(&self) -> PrivacyBucketConfig {
        self.config
    }

    /// Record a successful anonymous circuit establishment.
    pub fn record_handshake_success(
        &self,
        mode: SoranetPrivacyModeV1,
        when: SystemTime,
        rtt_millis: Option<u64>,
        active_circuits: Option<u64>,
    ) {
        self.with_bucket(mode, when, |bucket| {
            bucket.record_success(rtt_millis, active_circuits)
        });
    }

    /// Record a failed handshake and its failure reason.
    pub fn record_handshake_failure(
        &self,
        mode: SoranetPrivacyModeV1,
        when: SystemTime,
        reason: HandshakeFailure,
        rtt_millis: Option<u64>,
    ) {
        self.with_bucket(mode, when, |bucket| {
            bucket.record_failure(reason, rtt_millis)
        });
    }

    /// Record a throttling decision.
    pub fn record_throttle(
        &self,
        mode: SoranetPrivacyModeV1,
        when: SystemTime,
        scope: PrivacyThrottleScope,
    ) {
        self.with_bucket(mode, when, |bucket| bucket.record_throttle(scope));
    }

    /// Record an instantaneous sample of active circuits outside handshake events.
    pub fn record_active_sample(
        &self,
        mode: SoranetPrivacyModeV1,
        when: SystemTime,
        active_circuits: u64,
    ) {
        self.with_bucket(mode, when, |bucket| {
            bucket.record_active_sample(active_circuits)
        });
    }

    /// Record verified bytes relayed within the anonymised circuit.
    pub fn record_verified_bytes(&self, mode: SoranetPrivacyModeV1, when: SystemTime, bytes: u128) {
        if bytes == 0 {
            return;
        }
        self.with_bucket(mode, when, |bucket| bucket.record_verified_bytes(bytes));
    }

    /// Record an anonymised GAR abuse report category.
    pub fn record_gar_category(
        &self,
        mode: SoranetPrivacyModeV1,
        when: SystemTime,
        category: &str,
    ) {
        let trimmed = category.trim();
        if trimmed.is_empty() {
            return;
        }
        let hash = gar_category_hash(trimmed);
        self.with_bucket(mode, when, |bucket| bucket.record_gar_category(hash));
    }

    /// Record a telemetry event expressed via the `SoranetPrivacyEventV1` payload.
    pub fn record_event(&self, event: &SoranetPrivacyEventV1) {
        let when = unix_seconds_to_system_time(event.timestamp_unix);
        match &event.kind {
            SoranetPrivacyEventKindV1::HandshakeSuccess(payload) => {
                self.record_handshake_success(
                    event.mode,
                    when,
                    payload.rtt_ms,
                    payload.active_circuits_after,
                );
            }
            SoranetPrivacyEventKindV1::HandshakeFailure(payload) => {
                let reason = match payload.reason {
                    SoranetPrivacyHandshakeFailureV1::Pow => HandshakeFailure::Pow {
                        reason: pow_reason_from_detail(payload.detail.as_deref()),
                    },
                    SoranetPrivacyHandshakeFailureV1::Timeout => HandshakeFailure::Timeout,
                    SoranetPrivacyHandshakeFailureV1::Downgrade => HandshakeFailure::Downgrade,
                    SoranetPrivacyHandshakeFailureV1::Other => HandshakeFailure::Other,
                };
                self.record_handshake_failure(event.mode, when, reason, payload.rtt_ms);
            }
            SoranetPrivacyEventKindV1::Throttle(payload) => {
                self.record_throttle(event.mode, when, payload.scope.into());
            }
            SoranetPrivacyEventKindV1::ActiveSample(payload) => {
                self.record_active_sample(event.mode, when, payload.active_circuits);
            }
            SoranetPrivacyEventKindV1::VerifiedBytes(payload) => {
                self.record_verified_bytes(event.mode, when, payload.bytes);
            }
            SoranetPrivacyEventKindV1::GarAbuseCategory(payload) => {
                self.record_gar_category(event.mode, when, &payload.label);
            }
        }
    }

    /// Ingest a newline-delimited JSON payload emitted by relay admin endpoints.
    ///
    /// # Errors
    /// Returns an error when any line fails to parse as a `SoranetPrivacyEventV1`.
    pub fn ingest_ndjson(&self, payload: &str) -> Result<usize, PrivacyEventError> {
        let mut count = 0usize;
        for (idx, line) in payload.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let event: SoranetPrivacyEventV1 =
                json::from_str(trimmed).map_err(|source| PrivacyEventError::InvalidNdjsonLine {
                    line: idx + 1,
                    source,
                })?;
            self.record_event(&event);
            count = count.saturating_add(1);
        }
        Ok(count)
    }

    /// Ingest a Prio share emitted by a distributed privacy collector.
    ///
    /// Shares are accumulated per-bucket until [`PrivacyBucketConfig::expected_shares`]
    /// are present. Once the threshold is satisfied the combined bucket is added to
    /// the completed queue and becomes visible to [`drain_ready`](Self::drain_ready).
    ///
    /// # Errors
    ///
    /// Returns [`PrivacyShareError`] when share metadata is inconsistent with the
    /// current configuration (e.g., bucket width mismatch) or when combining shares
    /// yields invalid aggregates (negative totals or values exceeding supported
    /// ranges).
    pub fn ingest_prio_share(
        &self,
        share: SoranetPrivacyPrioShareV1,
    ) -> Result<(), PrivacyShareError> {
        let mut state = self
            .state
            .lock()
            .expect("soranet secure aggregator mutex poisoned");
        if let Some(metrics) = state.ingest_prio_share(share, &self.config)? {
            state.push_completed(metrics, &self.config);
        }
        Ok(())
    }

    /// Drain ready buckets using the supplied timestamp to evaluate flush delays.
    pub fn drain_ready_with_snapshot(
        &self,
        now: SystemTime,
    ) -> (Vec<SoranetPrivacyBucketMetricsV1>, PrivacyDrainSnapshot) {
        let mut state = self
            .state
            .lock()
            .expect("soranet secure aggregator mutex poisoned");
        let current_idx = bucket_index(now, self.config.bucket_secs);
        state.flush_ready(current_idx, &self.config);
        let updated_open = state.open_bucket_counts();
        let collector_backlog = state.collector_backlog_by_mode();
        let evicted_completed = state.take_evicted_completed();
        let mut drained: Vec<_> = state.completed.drain(..).collect();
        drained.sort_by_key(|bucket| (bucket.bucket_start_unix, bucket.mode));
        let mut suppressed_counts = BTreeMap::new();
        let mut suppressed_by_mode: BTreeMap<
            SoranetPrivacyModeV1,
            BTreeMap<SoranetPrivacySuppressionReasonV1, u64>,
        > = BTreeMap::new();
        for bucket in &drained {
            if let Some(reason) = bucket.suppression_reason {
                *suppressed_counts.entry(reason).or_default() += 1;
                *suppressed_by_mode
                    .entry(bucket.mode)
                    .or_default()
                    .entry(reason)
                    .or_default() += 1;
            }
        }
        let snapshot = PrivacyDrainSnapshot {
            drained_buckets: drained.len(),
            open_buckets: updated_open,
            collector_backlog,
            evicted_completed,
            suppressed_counts,
            suppressed_by_mode,
        };
        (drained, snapshot)
    }

    /// Drain ready buckets using the supplied timestamp and discard the snapshot metadata.
    pub fn drain_ready(&self, now: SystemTime) -> Vec<SoranetPrivacyBucketMetricsV1> {
        let (drained, _) = self.drain_ready_with_snapshot(now);
        drained
    }

    /// Drain ready buckets as of the current wall clock.
    pub fn drain_ready_now(&self) -> Vec<SoranetPrivacyBucketMetricsV1> {
        self.drain_ready(SystemTime::now())
    }

    /// Drain ready buckets and return the accompanying queue snapshot using the current wall clock.
    pub fn drain_ready_now_with_snapshot(
        &self,
    ) -> (Vec<SoranetPrivacyBucketMetricsV1>, PrivacyDrainSnapshot) {
        self.drain_ready_with_snapshot(SystemTime::now())
    }

    fn with_bucket<F>(&self, mode: SoranetPrivacyModeV1, when: SystemTime, mut update: F)
    where
        F: FnMut(&mut BucketStats),
    {
        let mut state = self
            .state
            .lock()
            .expect("soranet secure aggregator mutex poisoned");
        let bucket_idx = bucket_index(when, self.config.bucket_secs);
        let bucket = state.open.entry((mode, bucket_idx)).or_default();
        update(bucket);
        state.flush_ready(bucket_idx, &self.config);
    }
}

#[derive(Debug, Default)]
struct PrivacyState {
    open: BTreeMap<(SoranetPrivacyModeV1, u64), BucketStats>,
    completed: VecDeque<SoranetPrivacyBucketMetricsV1>,
    collector_shares: BTreeMap<(SoranetPrivacyModeV1, u64), CollectorShareAccumulator>,
    evicted_completed: u64,
}

impl PrivacyState {
    fn flush_ready(&mut self, current_idx: u64, config: &PrivacyBucketConfig) {
        self.evict_stale_collectors(current_idx, config);
        let mut ready = Vec::new();
        for (&(mode, bucket_idx), stats) in &self.open {
            let age = current_idx.saturating_sub(bucket_idx);
            let meets_delay = age > 0 && age >= config.flush_delay_buckets;
            let force_flush = age > 0 && age >= config.force_flush_buckets;
            if !meets_delay && !force_flush {
                continue;
            }
            let contributor_count = stats.handshake_events();
            if meets_delay && contributor_count >= config.min_contributors {
                ready.push(((mode, bucket_idx), None));
            } else if force_flush {
                ready.push((
                    (mode, bucket_idx),
                    Some(SoranetPrivacySuppressionReasonV1::ForcedFlushWindowElapsed),
                ));
            }
        }

        for ((mode, bucket_idx), suppression_reason) in ready {
            if let Some(stats) = self.open.remove(&(mode, bucket_idx)) {
                let metrics = stats.into_metrics(mode, bucket_idx, suppression_reason, config);
                self.push_completed(metrics, config);
            }
        }
    }

    fn push_completed(
        &mut self,
        metrics: SoranetPrivacyBucketMetricsV1,
        config: &PrivacyBucketConfig,
    ) {
        self.completed.push_back(metrics);
        while self.completed.len() > config.max_completed_buckets {
            self.completed.pop_front();
            self.evicted_completed = self.evicted_completed.saturating_add(1);
        }
    }

    fn open_bucket_counts(&self) -> BTreeMap<SoranetPrivacyModeV1, usize> {
        let mut counts = BTreeMap::new();
        for (mode, _) in self.open.keys() {
            *counts.entry(*mode).or_default() += 1;
        }
        counts
    }

    fn collector_backlog_by_mode(&self) -> BTreeMap<SoranetPrivacyModeV1, usize> {
        let mut counts = BTreeMap::new();
        for (mode, _) in self.collector_shares.keys() {
            *counts.entry(*mode).or_default() += 1;
        }
        counts
    }

    fn evict_stale_collectors(&mut self, current_idx: u64, config: &PrivacyBucketConfig) {
        let mut stale = Vec::new();
        for (&(mode, bucket_idx), accumulator) in &self.collector_shares {
            let age = current_idx.saturating_sub(bucket_idx);
            if age >= config.max_share_lag_buckets {
                stale.push((
                    mode,
                    bucket_idx,
                    accumulator.bucket_start_unix,
                    accumulator.bucket_duration_secs,
                ));
            }
        }
        for (mode, bucket_idx, bucket_start_unix, bucket_duration_secs) in stale {
            let _ = self.collector_shares.remove(&(mode, bucket_idx));
            let suppressed = SoranetPrivacyBucketMetricsV1::suppressed_with_reason(
                mode,
                bucket_start_unix,
                bucket_duration_secs,
                SoranetPrivacySuppressionReasonV1::CollectorWindowElapsed,
            );
            self.push_completed(suppressed, config);
        }
    }

    fn take_evicted_completed(&mut self) -> u64 {
        let evicted = self.evicted_completed;
        self.evicted_completed = 0;
        evicted
    }

    fn ingest_prio_share(
        &mut self,
        share: SoranetPrivacyPrioShareV1,
        config: &PrivacyBucketConfig,
    ) -> Result<Option<SoranetPrivacyBucketMetricsV1>, PrivacyShareError> {
        let expected_duration = u32::try_from(config.bucket_secs)
            .expect("config validation ensures bucket_secs <= u32::MAX");
        if share.bucket_duration_secs != expected_duration {
            return Err(PrivacyShareError::BucketDurationMismatch {
                expected: expected_duration,
                received: share.bucket_duration_secs,
            });
        }
        if !share.bucket_start_unix.is_multiple_of(config.bucket_secs) {
            return Err(PrivacyShareError::BucketAlignmentMismatch {
                bucket_start: share.bucket_start_unix,
                bucket_secs: config.bucket_secs,
            });
        }
        let bucket_idx = share.bucket_start_unix / config.bucket_secs;
        let bucket_key = (share.mode, bucket_idx);
        match self.collector_shares.entry(bucket_key) {
            Entry::Occupied(mut occ) => {
                occ.get_mut().insert(share)?;
                if occ.get().ready(config.expected_shares) {
                    let metrics = occ.get().combine(config)?;
                    occ.remove();
                    Ok(Some(metrics))
                } else {
                    Ok(None)
                }
            }
            Entry::Vacant(vac) => {
                let mut accumulator = CollectorShareAccumulator::new(
                    share.bucket_start_unix,
                    share.bucket_duration_secs,
                    share.mode,
                );
                accumulator.insert(share)?;
                if accumulator.ready(config.expected_shares) {
                    let metrics = accumulator.combine(config)?;
                    Ok(Some(metrics))
                } else {
                    vac.insert(accumulator);
                    Ok(None)
                }
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
struct BucketStats {
    handshake_success: u64,
    handshake_pow_rejects: u64,
    pow_rejects_by_reason: BTreeMap<SoranetPowFailureReasonV1, u64>,
    handshake_downgrades: u64,
    handshake_timeouts: u64,
    handshake_other_failures: u64,
    throttle_congestion: u64,
    throttle_cooldown: u64,
    throttle_emergency: u64,
    throttle_remote: u64,
    throttle_descriptor: u64,
    throttle_descriptor_replay: u64,
    active: ActiveAccumulator,
    rtt: LatencyHistogram,
    bytes_verified: u128,
    gar_counts: BTreeMap<[u8; 8], u64>,
}

impl BucketStats {
    fn record_success(&mut self, rtt_millis: Option<u64>, active_circuits: Option<u64>) {
        self.handshake_success = self.handshake_success.saturating_add(1);
        if let Some(latency) = rtt_millis {
            self.rtt.observe(latency);
        }
        if let Some(active) = active_circuits {
            self.active.record(active);
        }
    }

    fn record_failure(&mut self, reason: HandshakeFailure, rtt_millis: Option<u64>) {
        if let Some(latency) = rtt_millis {
            self.rtt.observe(latency);
        }
        match reason {
            HandshakeFailure::Pow { reason } => {
                self.handshake_pow_rejects = self.handshake_pow_rejects.saturating_add(1);
                let counter = self.pow_rejects_by_reason.entry(reason).or_insert(0);
                *counter = counter.saturating_add(1);
            }
            HandshakeFailure::Timeout => {
                self.handshake_timeouts = self.handshake_timeouts.saturating_add(1);
            }
            HandshakeFailure::Downgrade => {
                self.handshake_downgrades = self.handshake_downgrades.saturating_add(1);
            }
            HandshakeFailure::Other => {
                self.handshake_other_failures = self.handshake_other_failures.saturating_add(1);
            }
        }
    }

    fn record_throttle(&mut self, scope: PrivacyThrottleScope) {
        match scope {
            PrivacyThrottleScope::Congestion => {
                self.throttle_congestion = self.throttle_congestion.saturating_add(1);
            }
            PrivacyThrottleScope::Cooldown => {
                self.throttle_cooldown = self.throttle_cooldown.saturating_add(1);
            }
            PrivacyThrottleScope::Emergency => {
                self.throttle_emergency = self.throttle_emergency.saturating_add(1);
            }
            PrivacyThrottleScope::RemoteQuota => {
                self.throttle_remote = self.throttle_remote.saturating_add(1);
            }
            PrivacyThrottleScope::DescriptorQuota => {
                self.throttle_descriptor = self.throttle_descriptor.saturating_add(1);
            }
            PrivacyThrottleScope::DescriptorReplay => {
                self.throttle_descriptor_replay = self.throttle_descriptor_replay.saturating_add(1);
            }
        }
    }

    fn record_active_sample(&mut self, count: u64) {
        self.active.record(count);
    }

    fn record_verified_bytes(&mut self, bytes: u128) {
        self.bytes_verified = self.bytes_verified.saturating_add(bytes);
    }

    fn record_gar_category(&mut self, hash: [u8; 8]) {
        let counter = self.gar_counts.entry(hash).or_insert(0);
        *counter = counter.saturating_add(1);
    }

    fn handshake_events(&self) -> u64 {
        self.handshake_success
            .saturating_add(self.handshake_pow_rejects)
            .saturating_add(self.handshake_downgrades)
            .saturating_add(self.handshake_timeouts)
            .saturating_add(self.handshake_other_failures)
    }

    fn into_metrics(
        self,
        mode: SoranetPrivacyModeV1,
        bucket_idx: u64,
        suppression_reason: Option<SoranetPrivacySuppressionReasonV1>,
        config: &PrivacyBucketConfig,
    ) -> SoranetPrivacyBucketMetricsV1 {
        let bucket_start_unix = bucket_idx.saturating_mul(config.bucket_secs);
        let bucket_duration_secs = u32::try_from(config.bucket_secs).unwrap_or(u32::MAX);
        if let Some(reason) = suppression_reason {
            return SoranetPrivacyBucketMetricsV1::suppressed_with_reason(
                mode,
                bucket_start_unix,
                bucket_duration_secs,
                reason,
            );
        }

        let contributor_count = u32::try_from(self.handshake_events()).unwrap_or(u32::MAX);

        let Self {
            handshake_success,
            handshake_pow_rejects,
            pow_rejects_by_reason,
            handshake_downgrades,
            handshake_timeouts,
            handshake_other_failures,
            throttle_congestion,
            throttle_cooldown,
            throttle_emergency,
            throttle_remote,
            throttle_descriptor,
            throttle_descriptor_replay,
            active,
            rtt,
            bytes_verified,
            gar_counts,
        } = self;

        let (active_mean, active_max) = active.into_summary();
        let percentiles = rtt
            .into_percentiles()
            .into_iter()
            .map(|(label, value)| SoranetLatencyPercentileV1::new(label, value))
            .collect();
        let gar_abuse_counts = gar_counts
            .into_iter()
            .map(|(hash, count)| SoranetGarAbuseCountV1::new(hash, count))
            .collect();
        let pow_rejects_by_reason = pow_rejects_by_reason
            .into_iter()
            .map(|(reason, count)| SoranetPowFailureCountV1 { reason, count })
            .collect();

        SoranetPrivacyBucketMetricsV1 {
            mode,
            bucket_start_unix,
            bucket_duration_secs,
            contributor_count,
            handshake_accept_total: handshake_success,
            handshake_pow_reject_total: handshake_pow_rejects,
            pow_rejects_by_reason,
            handshake_downgrade_total: handshake_downgrades,
            handshake_timeout_total: handshake_timeouts,
            handshake_other_failure_total: handshake_other_failures,
            throttle_congestion_total: throttle_congestion,
            throttle_cooldown_total: throttle_cooldown,
            throttle_emergency_total: throttle_emergency,
            throttle_remote_total: throttle_remote,
            throttle_descriptor_total: throttle_descriptor,
            throttle_descriptor_replay_total: throttle_descriptor_replay,
            active_circuits_mean: active_mean,
            active_circuits_max: active_max,
            verified_bytes_total: bytes_verified,
            rtt_percentiles_ms: percentiles,
            gar_abuse_counts,
            suppressed: false,
            suppression_reason: None,
        }
    }
}

#[derive(Debug)]
struct CollectorShareAccumulator {
    bucket_start_unix: u64,
    bucket_duration_secs: u32,
    mode: SoranetPrivacyModeV1,
    shares: BTreeMap<u16, SoranetPrivacyPrioShareV1>,
}

impl CollectorShareAccumulator {
    fn new(bucket_start_unix: u64, bucket_duration_secs: u32, mode: SoranetPrivacyModeV1) -> Self {
        Self {
            bucket_start_unix,
            bucket_duration_secs,
            mode,
            shares: BTreeMap::new(),
        }
    }

    fn insert(&mut self, share: SoranetPrivacyPrioShareV1) -> Result<(), PrivacyShareError> {
        if share.bucket_start_unix != self.bucket_start_unix {
            return Err(PrivacyShareError::BucketAlignmentMismatch {
                bucket_start: share.bucket_start_unix,
                bucket_secs: u64::from(self.bucket_duration_secs),
            });
        }
        if share.bucket_duration_secs != self.bucket_duration_secs {
            return Err(PrivacyShareError::BucketDurationMismatch {
                expected: self.bucket_duration_secs,
                received: share.bucket_duration_secs,
            });
        }
        if share.mode != self.mode {
            return Err(PrivacyShareError::ModeMismatch {
                expected: self.mode,
                received: share.mode,
            });
        }
        if !share.rtt_bucket_shares.is_empty() && share.rtt_bucket_shares.len() != RTT_BUCKET_COUNT
        {
            return Err(PrivacyShareError::HistogramLengthMismatch {
                expected: RTT_BUCKET_COUNT,
                received: share.rtt_bucket_shares.len(),
            });
        }
        self.shares.insert(share.collector_id, share);
        Ok(())
    }

    fn ready(&self, expected_shares: u16) -> bool {
        u16::try_from(self.shares.len()).unwrap_or(u16::MAX) >= expected_shares.max(1)
    }

    fn combine(
        &self,
        config: &PrivacyBucketConfig,
    ) -> Result<SoranetPrivacyBucketMetricsV1, PrivacyShareError> {
        let mut totals = CombinedShareTotals::default();
        for share in self.shares.values() {
            totals.add_share(share)?;
        }
        totals.finalize(
            self.mode,
            self.bucket_start_unix,
            self.bucket_duration_secs,
            config,
        )
    }
}

#[derive(Debug)]
struct CombinedShareTotals {
    handshake_success: i128,
    handshake_pow_rejects: i128,
    handshake_downgrades: i128,
    handshake_timeouts: i128,
    handshake_other_failures: i128,
    throttle_congestion: i128,
    throttle_cooldown: i128,
    throttle_emergency: i128,
    throttle_remote: i128,
    throttle_descriptor: i128,
    throttle_descriptor_replay: i128,
    active_sum: i128,
    active_samples: i128,
    active_max: Option<u64>,
    verified_bytes: i128,
    rtt_counts: [i128; RTT_BUCKET_COUNT],
    gar_counts: BTreeMap<[u8; 8], i128>,
    suppressed_all: bool,
}

impl Default for CombinedShareTotals {
    fn default() -> Self {
        Self {
            handshake_success: 0,
            handshake_pow_rejects: 0,
            handshake_downgrades: 0,
            handshake_timeouts: 0,
            handshake_other_failures: 0,
            throttle_congestion: 0,
            throttle_cooldown: 0,
            throttle_remote: 0,
            throttle_descriptor: 0,
            throttle_descriptor_replay: 0,
            throttle_emergency: 0,
            active_sum: 0,
            active_samples: 0,
            active_max: None,
            verified_bytes: 0,
            rtt_counts: [0; RTT_BUCKET_COUNT],
            gar_counts: BTreeMap::new(),
            suppressed_all: true,
        }
    }
}

impl CombinedShareTotals {
    fn add_share(&mut self, share: &SoranetPrivacyPrioShareV1) -> Result<(), PrivacyShareError> {
        self.handshake_success = checked_add_i128(
            self.handshake_success,
            share.handshake_accept_share.into(),
            "handshake_accept",
        )?;
        self.handshake_pow_rejects = checked_add_i128(
            self.handshake_pow_rejects,
            share.handshake_pow_reject_share.into(),
            "handshake_pow_reject",
        )?;
        self.handshake_downgrades = checked_add_i128(
            self.handshake_downgrades,
            share.handshake_downgrade_share.into(),
            "handshake_downgrade",
        )?;
        self.handshake_timeouts = checked_add_i128(
            self.handshake_timeouts,
            share.handshake_timeout_share.into(),
            "handshake_timeout",
        )?;
        self.handshake_other_failures = checked_add_i128(
            self.handshake_other_failures,
            share.handshake_other_failure_share.into(),
            "handshake_other_failure",
        )?;
        self.throttle_congestion = checked_add_i128(
            self.throttle_congestion,
            share.throttle_congestion_share.into(),
            "throttle_congestion",
        )?;
        self.throttle_cooldown = checked_add_i128(
            self.throttle_cooldown,
            share.throttle_cooldown_share.into(),
            "throttle_cooldown",
        )?;
        self.throttle_emergency = checked_add_i128(
            self.throttle_emergency,
            share.throttle_emergency_share.into(),
            "throttle_emergency",
        )?;
        self.throttle_remote = checked_add_i128(
            self.throttle_remote,
            share.throttle_remote_share.into(),
            "throttle_remote",
        )?;
        self.throttle_descriptor = checked_add_i128(
            self.throttle_descriptor,
            share.throttle_descriptor_share.into(),
            "throttle_descriptor",
        )?;
        self.throttle_descriptor_replay = checked_add_i128(
            self.throttle_descriptor_replay,
            share.throttle_descriptor_replay_share.into(),
            "throttle_descriptor_replay",
        )?;
        self.active_sum = checked_add_i128(
            self.active_sum,
            share.active_circuits_sum_share.into(),
            "active_circuits_sum",
        )?;
        self.active_samples = checked_add_i128(
            self.active_samples,
            share.active_circuits_sample_share.into(),
            "active_circuits_samples",
        )?;
        if let Some(max) = share.active_circuits_max_observed {
            self.active_max = Some(self.active_max.map_or(max, |current| current.max(max)));
        }
        self.verified_bytes = checked_add_i128(
            self.verified_bytes,
            share.verified_bytes_share.into(),
            "verified_bytes",
        )?;
        if share.rtt_bucket_shares.is_empty() {
            // No observations contributed by this share.
        } else {
            for (slot, contribution) in self
                .rtt_counts
                .iter_mut()
                .zip(share.rtt_bucket_shares.iter())
            {
                *slot = checked_add_i128(*slot, (*contribution).into(), "rtt_bucket")?;
            }
        }
        for SoranetGarAbuseShareV1 {
            category_hash,
            count_share,
        } in &share.gar_abuse_shares
        {
            let entry = self.gar_counts.entry(*category_hash).or_insert(0);
            *entry = checked_add_i128(*entry, (*count_share).into(), "gar_abuse_counts")?;
        }
        self.suppressed_all &= share.suppressed;
        Ok(())
    }

    fn finalize(
        self,
        mode: SoranetPrivacyModeV1,
        bucket_start_unix: u64,
        bucket_duration_secs: u32,
        config: &PrivacyBucketConfig,
    ) -> Result<SoranetPrivacyBucketMetricsV1, PrivacyShareError> {
        let handshake_success =
            ensure_non_negative_u64(self.handshake_success, "handshake_accept")?;
        let handshake_pow =
            ensure_non_negative_u64(self.handshake_pow_rejects, "handshake_pow_reject")?;
        let handshake_downgrade =
            ensure_non_negative_u64(self.handshake_downgrades, "handshake_downgrade")?;
        let handshake_timeout =
            ensure_non_negative_u64(self.handshake_timeouts, "handshake_timeout")?;
        let handshake_other =
            ensure_non_negative_u64(self.handshake_other_failures, "handshake_other_failure")?;
        let throttle_congestion =
            ensure_non_negative_u64(self.throttle_congestion, "throttle_congestion")?;
        let throttle_cooldown =
            ensure_non_negative_u64(self.throttle_cooldown, "throttle_cooldown")?;
        let throttle_remote = ensure_non_negative_u64(self.throttle_remote, "throttle_remote")?;
        let throttle_descriptor =
            ensure_non_negative_u64(self.throttle_descriptor, "throttle_descriptor")?;
        let throttle_descriptor_replay = ensure_non_negative_u64(
            self.throttle_descriptor_replay,
            "throttle_descriptor_replay",
        )?;
        let throttle_emergency =
            ensure_non_negative_u64(self.throttle_emergency, "throttle_emergency")?;
        let active_sum = ensure_non_negative_u128(self.active_sum, "active_circuits_sum")?;
        let active_samples =
            ensure_non_negative_u64(self.active_samples, "active_circuits_samples")?;
        let verified_bytes = ensure_non_negative_u128(self.verified_bytes, "verified_bytes")?;
        let rtt_counts = ensure_non_negative_histogram(&self.rtt_counts)?;

        let mut gar_counts = BTreeMap::new();
        for (hash, count) in self.gar_counts {
            let value = ensure_non_negative_u64(count, "gar_abuse_counts")?;
            if value > 0 {
                gar_counts.insert(hash, value);
            }
        }

        let handshake_total = self.handshake_success
            + self.handshake_pow_rejects
            + self.handshake_downgrades
            + self.handshake_timeouts
            + self.handshake_other_failures;
        if handshake_total < 0 {
            return Err(PrivacyShareError::NegativeAggregate {
                field: "total_handshakes",
                value: handshake_total,
            });
        }
        let suppressed_threshold = i128::from(config.min_contributors);
        let suppressed_by_threshold = handshake_total < suppressed_threshold;
        let suppression_reason = if suppressed_by_threshold {
            Some(SoranetPrivacySuppressionReasonV1::InsufficientContributors)
        } else if self.suppressed_all {
            Some(SoranetPrivacySuppressionReasonV1::CollectorSuppressed)
        } else {
            None
        };

        if let Some(reason) = suppression_reason {
            return Ok(SoranetPrivacyBucketMetricsV1::suppressed_with_reason(
                mode,
                bucket_start_unix,
                bucket_duration_secs,
                reason,
            ));
        }
        let stats = BucketStats {
            handshake_success,
            handshake_pow_rejects: handshake_pow,
            pow_rejects_by_reason: BTreeMap::new(),
            handshake_downgrades: handshake_downgrade,
            handshake_timeouts: handshake_timeout,
            handshake_other_failures: handshake_other,
            throttle_congestion,
            throttle_cooldown,
            throttle_emergency,
            throttle_remote,
            throttle_descriptor,
            throttle_descriptor_replay,
            active: ActiveAccumulator {
                total: active_sum,
                count: active_samples,
                max: self.active_max.unwrap_or(0),
            },
            rtt: LatencyHistogram::from_counts(&rtt_counts),
            bytes_verified: verified_bytes,
            gar_counts,
        };

        let bucket_idx = bucket_start_unix / config.bucket_secs;
        Ok(stats.into_metrics(mode, bucket_idx, None, config))
    }
}

fn checked_add_i128(lhs: i128, rhs: i128, field: &'static str) -> Result<i128, PrivacyShareError> {
    lhs.checked_add(rhs)
        .ok_or(PrivacyShareError::AggregateOverflow { field })
}

fn ensure_non_negative_u64(value: i128, field: &'static str) -> Result<u64, PrivacyShareError> {
    if value < 0 {
        return Err(PrivacyShareError::NegativeAggregate { field, value });
    }
    u64::try_from(value).map_err(|_| PrivacyShareError::AggregateOverflow { field })
}

fn ensure_non_negative_u128(value: i128, field: &'static str) -> Result<u128, PrivacyShareError> {
    if value < 0 {
        return Err(PrivacyShareError::NegativeAggregate { field, value });
    }
    u128::try_from(value).map_err(|_| PrivacyShareError::AggregateOverflow { field })
}

fn ensure_non_negative_histogram(counts: &[i128]) -> Result<Vec<u64>, PrivacyShareError> {
    let mut result = Vec::with_capacity(counts.len());
    for value in counts {
        result.push(ensure_non_negative_u64(*value, "rtt_bucket")?);
    }
    Ok(result)
}

#[derive(Debug, Clone, Copy, Default)]
struct ActiveAccumulator {
    total: u128,
    count: u64,
    max: u64,
}

impl ActiveAccumulator {
    fn record(&mut self, sample: u64) {
        self.total = self.total.saturating_add(u128::from(sample));
        self.count = self.count.saturating_add(1);
        if self.count == 1 || sample > self.max {
            self.max = sample;
        }
    }

    fn into_summary(self) -> (Option<u64>, Option<u64>) {
        if self.count == 0 {
            return (None, None);
        }
        let avg_raw = self.total / u128::from(self.count);
        let avg = u64::try_from(avg_raw).unwrap_or(u64::MAX);
        (Some(avg), Some(self.max))
    }
}

#[derive(Debug, Clone)]
struct LatencyHistogram {
    buckets: [u64; RTT_BUCKET_BOUNDS_MS.len() + 1],
    total: u64,
}

impl Default for LatencyHistogram {
    fn default() -> Self {
        Self {
            buckets: [0; RTT_BUCKET_BOUNDS_MS.len() + 1],
            total: 0,
        }
    }
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

    fn merge_counts(&mut self, counts: &[u64]) {
        debug_assert_eq!(counts.len(), RTT_BUCKET_COUNT);
        let added: u64 = counts.iter().sum();
        self.total = self.total.saturating_add(added);
        for (slot, addition) in self.buckets.iter_mut().zip(counts.iter()) {
            *slot = slot.saturating_add(*addition);
        }
    }

    fn from_counts(counts: &[u64]) -> Self {
        let mut histogram = Self::default();
        if !counts.is_empty() {
            histogram.merge_counts(counts);
        }
        histogram
    }

    fn into_percentiles(self) -> Vec<(String, u64)> {
        if self.total == 0 {
            return Vec::new();
        }

        let mut result = Vec::with_capacity(RTT_PERCENTILES.len());
        for spec in RTT_PERCENTILES {
            let numerator = u128::from(spec.numerator);
            let denominator = u128::from(spec.denominator);
            let total = u128::from(self.total);
            let rank_raw = (total
                .saturating_mul(numerator)
                .saturating_add(denominator.saturating_sub(1)))
                / denominator;
            let rank = u64::try_from(rank_raw).unwrap_or(u64::MAX).max(1);
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
                            .map_or(1_000, |bound| bound.saturating_add(1_000))
                    };
                    break;
                }
            }

            result.push((spec.label.to_string(), value));
        }

        result
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

fn gar_category_hash(category: &str) -> [u8; 8] {
    let mut hasher = Blake3Hasher::new();
    hasher.update(category.trim().as_bytes());
    let digest = hasher.finalize();
    let mut truncated = [0u8; 8];
    truncated.copy_from_slice(&digest.as_bytes()[..8]);
    truncated
}

fn unix_seconds_to_system_time(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(seconds)
}

impl From<SoranetPrivacyHandshakeFailureV1> for HandshakeFailure {
    fn from(value: SoranetPrivacyHandshakeFailureV1) -> Self {
        match value {
            SoranetPrivacyHandshakeFailureV1::Pow => HandshakeFailure::Pow {
                reason: SoranetPowFailureReasonV1::InvalidSolution,
            },
            SoranetPrivacyHandshakeFailureV1::Timeout => HandshakeFailure::Timeout,
            SoranetPrivacyHandshakeFailureV1::Downgrade => HandshakeFailure::Downgrade,
            SoranetPrivacyHandshakeFailureV1::Other => HandshakeFailure::Other,
        }
    }
}

impl From<SoranetPrivacyThrottleScopeV1> for PrivacyThrottleScope {
    fn from(value: SoranetPrivacyThrottleScopeV1) -> Self {
        match value {
            SoranetPrivacyThrottleScopeV1::Congestion => PrivacyThrottleScope::Congestion,
            SoranetPrivacyThrottleScopeV1::Cooldown => PrivacyThrottleScope::Cooldown,
            SoranetPrivacyThrottleScopeV1::Emergency => PrivacyThrottleScope::Emergency,
            SoranetPrivacyThrottleScopeV1::RemoteQuota => PrivacyThrottleScope::RemoteQuota,
            SoranetPrivacyThrottleScopeV1::DescriptorQuota => PrivacyThrottleScope::DescriptorQuota,
            SoranetPrivacyThrottleScopeV1::DescriptorReplay => {
                PrivacyThrottleScope::DescriptorReplay
            }
        }
    }
}
