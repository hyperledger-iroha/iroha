//! Privacy-preserving telemetry summaries for the `SoraNet` anonymity layer.
//!
//! These types model the aggregated metrics emitted by the SNNet-8 secure
//! telemetry pipeline. Buckets are materialised at minute granularity (or any
//! configured interval) once the secure aggregation collector validates that
//! enough circuit events contributed to the window. Individual handshake
//! events, GAR abuse reports, and RTT measurements remain unlinkable; only the
//! aggregated statistics described here are persisted or forwarded to
//! dashboards. This mirrors the behaviour implemented by the reference relay
//! privacy aggregator while providing schema-stable Norito payloads for
//! long-term storage and operator dashboards.

use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
#[cfg(feature = "json")]
use norito::json;

#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};

/// Aggregated GAR abuse report counts keyed by the truncated category hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SoranetGarAbuseCountV1 {
    /// First eight bytes of the BLAKE3 hash for the GAR abuse category label.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub category_hash: [u8; 8],
    /// Number of reports recorded for the category within the bucket.
    pub count: u64,
}

impl SoranetGarAbuseCountV1 {
    /// Construct a new GAR abuse counter entry.
    #[must_use]
    pub const fn new(category_hash: [u8; 8], count: u64) -> Self {
        Self {
            category_hash,
            count,
        }
    }
}

/// Secret-shared GAR abuse counter contribution emitted by a Prio collector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SoranetGarAbuseShareV1 {
    /// First eight bytes of the BLAKE3 hash for the GAR abuse category label.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub category_hash: [u8; 8],
    /// Signed share for the number of reports recorded for the category.
    pub count_share: i64,
}

impl SoranetGarAbuseShareV1 {
    /// Construct a new GAR abuse share entry.
    #[must_use]
    pub const fn new(category_hash: [u8; 8], count_share: i64) -> Self {
        Self {
            category_hash,
            count_share,
        }
    }
}

/// Relay mode associated with privacy telemetry buckets.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
pub enum SoranetPrivacyModeV1 {
    /// Entry relay handling client ingress.
    Entry,
    /// Middle relay forwarding anonymous circuits.
    Middle,
    /// Exit relay delivering traffic to Torii.
    Exit,
}

impl SoranetPrivacyModeV1 {
    /// Human-readable label reused across telemetry outputs.
    #[must_use]
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::Entry => "entry",
            Self::Middle => "middle",
            Self::Exit => "exit",
        }
    }
}

impl core::fmt::Display for SoranetPrivacyModeV1 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_label())
    }
}

#[cfg(feature = "json")]
impl json::JsonSerialize for SoranetPrivacyModeV1 {
    fn json_serialize(&self, out: &mut String) {
        json::write_json_string(self.as_label(), out);
    }
}

#[cfg(feature = "json")]
impl json::JsonDeserialize for SoranetPrivacyModeV1 {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = parser.parse_string()?;
        match value.as_str() {
            "entry" => Ok(Self::Entry),
            "middle" => Ok(Self::Middle),
            "exit" => Ok(Self::Exit),
            other => Err(json::Error::unknown_field(other)),
        }
    }
}

/// Secret-shared Prio contribution covering a privacy telemetry bucket.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SoranetPrivacyPrioShareV1 {
    /// Identifier for the collector emitting the share (stable across restarts).
    pub collector_id: u16,
    /// Bucket start timestamp (seconds since UNIX epoch).
    pub bucket_start_unix: u64,
    /// Bucket width in seconds.
    pub bucket_duration_secs: u32,
    /// Relay mode represented by this share.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::privacy_mode"))]
    pub mode: SoranetPrivacyModeV1,
    /// Share for successfully established anonymous circuits.
    pub handshake_accept_share: i64,
    /// Share for proof-of-work rejected handshakes.
    pub handshake_pow_reject_share: i64,
    /// Share for downgrade-attempt rejections.
    pub handshake_downgrade_share: i64,
    /// Share for handshake timeouts.
    pub handshake_timeout_share: i64,
    /// Share for miscellaneous handshake failures.
    pub handshake_other_failure_share: i64,
    /// Share for congestion throttles.
    pub throttle_congestion_share: i64,
    /// Share for cooldown throttles.
    pub throttle_cooldown_share: i64,
    /// Share for emergency throttles.
    pub throttle_emergency_share: i64,
    /// Share for remote-quota throttles.
    pub throttle_remote_share: i64,
    /// Share for descriptor quota throttles.
    pub throttle_descriptor_share: i64,
    /// Share for descriptor replay throttles.
    pub throttle_descriptor_replay_share: i64,
    /// Share for the sum of active circuit samples within the bucket.
    pub active_circuits_sum_share: i64,
    /// Share for the number of active circuit samples contributing to `active_circuits_sum_share`.
    pub active_circuits_sample_share: i64,
    /// Maximum active circuits observed by the collector (non secret-shared).
    #[norito(default)]
    pub active_circuits_max_observed: Option<u64>,
    /// Share for verified bytes relayed.
    pub verified_bytes_share: i64,
    /// Shares for RTT histogram buckets. Length must match the histogram layout.
    #[norito(default)]
    pub rtt_bucket_shares: Vec<i64>,
    /// Secret-shared GAR abuse counters.
    #[norito(default)]
    pub gar_abuse_shares: Vec<SoranetGarAbuseShareV1>,
    /// Collector-level suppression hint; final suppression is computed after shares combine.
    pub suppressed: bool,
}

impl SoranetPrivacyPrioShareV1 {
    /// Construct a new Prio share with empty histogram and GAR counters.
    #[must_use]
    pub fn new(collector_id: u16, bucket_start_unix: u64, bucket_duration_secs: u32) -> Self {
        Self {
            collector_id,
            bucket_start_unix,
            bucket_duration_secs,
            mode: SoranetPrivacyModeV1::Entry,
            handshake_accept_share: 0,
            handshake_pow_reject_share: 0,
            handshake_downgrade_share: 0,
            handshake_timeout_share: 0,
            handshake_other_failure_share: 0,
            throttle_congestion_share: 0,
            throttle_cooldown_share: 0,
            throttle_emergency_share: 0,
            throttle_remote_share: 0,
            throttle_descriptor_share: 0,
            throttle_descriptor_replay_share: 0,
            active_circuits_sum_share: 0,
            active_circuits_sample_share: 0,
            active_circuits_max_observed: None,
            verified_bytes_share: 0,
            rtt_bucket_shares: Vec::new(),
            gar_abuse_shares: Vec::new(),
            suppressed: false,
        }
    }
}

/// Percentile estimate for RTT observations collected during the bucket window.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SoranetLatencyPercentileV1 {
    /// Label describing the percentile (e.g., `"p50"`, `"p95"`, `"p99"`).
    pub label: String,
    /// Estimated RTT in milliseconds for the percentile.
    pub value_ms: u64,
}

impl SoranetLatencyPercentileV1 {
    /// Construct a new percentile entry.
    #[must_use]
    pub fn new(label: String, value_ms: u64) -> Self {
        Self { label, value_ms }
    }
}

/// Aggregated metrics for a privacy-preserving `SoraNet` telemetry bucket.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SoranetPrivacyBucketMetricsV1 {
    /// Relay mode associated with the aggregated bucket.
    pub mode: SoranetPrivacyModeV1,
    /// Inclusive UNIX timestamp for the start of the bucket window (seconds).
    pub bucket_start_unix: u64,
    /// Width of the bucket window in seconds.
    pub bucket_duration_secs: u32,
    /// Total number of contributing handshake events counted towards the window.
    pub contributor_count: u32,
    /// Successfully established anonymous circuits.
    pub handshake_accept_total: u64,
    /// Handshake attempts rejected due to proof-of-work failures.
    pub handshake_pow_reject_total: u64,
    /// Breakdown of `PoW` failures by reason (best-effort; may be empty).
    #[norito(default)]
    pub pow_rejects_by_reason: Vec<SoranetPowFailureCountV1>,
    /// Handshake attempts rejected due to downgrade attempts.
    pub handshake_downgrade_total: u64,
    /// Handshake attempts that timed out before completion.
    pub handshake_timeout_total: u64,
    /// Handshake attempts rejected for other reasons.
    pub handshake_other_failure_total: u64,
    /// Throttling decisions attributed to congestion controllers.
    pub throttle_congestion_total: u64,
    /// Throttling decisions triggered by cooldown limits.
    pub throttle_cooldown_total: u64,
    /// Throttling decisions triggered by emergency override controls.
    pub throttle_emergency_total: u64,
    /// Throttling decisions attributed to remote quota limits.
    pub throttle_remote_total: u64,
    /// Throttling decisions attributed to descriptor quotas.
    pub throttle_descriptor_total: u64,
    /// Throttling decisions triggered by descriptor replay protection.
    pub throttle_descriptor_replay_total: u64,
    /// Average number of concurrently active circuits observed during the window.
    #[norito(default)]
    pub active_circuits_mean: Option<u64>,
    /// Maximum concurrently active circuits observed during the window.
    #[norito(default)]
    pub active_circuits_max: Option<u64>,
    /// Total verified bytes relayed within the bucket.
    pub verified_bytes_total: u128,
    /// RTT percentile estimates for handshake flows captured during the bucket.
    #[norito(default)]
    pub rtt_percentiles_ms: Vec<SoranetLatencyPercentileV1>,
    /// Aggregated GAR abuse report counters surfaced by relays.
    #[norito(default)]
    pub gar_abuse_counts: Vec<SoranetGarAbuseCountV1>,
    /// Indicates that the bucket was withheld due to insufficient contributions.
    #[norito(default)]
    pub suppressed: bool,
    /// Reason describing why the bucket was suppressed (when `suppressed = true`).
    #[norito(default)]
    pub suppression_reason: Option<SoranetPrivacySuppressionReasonV1>,
}

impl SoranetPrivacyBucketMetricsV1 {
    /// Construct an empty bucket flagged as suppressed.
    #[must_use]
    pub fn suppressed(
        mode: SoranetPrivacyModeV1,
        bucket_start_unix: u64,
        bucket_duration_secs: u32,
    ) -> Self {
        Self::suppressed_with_reason(
            mode,
            bucket_start_unix,
            bucket_duration_secs,
            SoranetPrivacySuppressionReasonV1::InsufficientContributors,
        )
    }

    /// Construct an empty bucket flagged as suppressed with an explicit reason.
    #[must_use]
    pub fn suppressed_with_reason(
        mode: SoranetPrivacyModeV1,
        bucket_start_unix: u64,
        bucket_duration_secs: u32,
        reason: SoranetPrivacySuppressionReasonV1,
    ) -> Self {
        Self {
            mode,
            bucket_start_unix,
            bucket_duration_secs,
            contributor_count: 0,
            handshake_accept_total: 0,
            handshake_pow_reject_total: 0,
            pow_rejects_by_reason: Vec::new(),
            handshake_downgrade_total: 0,
            handshake_timeout_total: 0,
            handshake_other_failure_total: 0,
            throttle_congestion_total: 0,
            throttle_cooldown_total: 0,
            throttle_emergency_total: 0,
            throttle_remote_total: 0,
            throttle_descriptor_total: 0,
            throttle_descriptor_replay_total: 0,
            active_circuits_mean: None,
            active_circuits_max: None,
            verified_bytes_total: 0,
            rtt_percentiles_ms: Vec::new(),
            gar_abuse_counts: Vec::new(),
            suppressed: true,
            suppression_reason: Some(reason),
        }
    }

    /// Returns `true` when the bucket is marked as suppressed.
    #[must_use]
    pub const fn is_suppressed(&self) -> bool {
        self.suppressed
    }

    /// Returns the total number of handshake events recorded within the bucket.
    #[must_use]
    pub fn handshake_events_total(&self) -> u64 {
        self.handshake_accept_total
            .saturating_add(self.handshake_pow_reject_total)
            .saturating_add(self.handshake_downgrade_total)
            .saturating_add(self.handshake_timeout_total)
            .saturating_add(self.handshake_other_failure_total)
    }
}

/// Enumerates the reasons a bucket may be suppressed.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
pub enum SoranetPrivacySuppressionReasonV1 {
    /// Not enough handshake contributors populated the bucket.
    InsufficientContributors,
    /// Every collector share flagged suppression, preventing release.
    CollectorSuppressed,
    /// Collector shares failed to arrive before the retention window elapsed.
    CollectorWindowElapsed,
    /// The bucket exceeded the force-flush window without reaching the threshold.
    ForcedFlushWindowElapsed,
}

impl SoranetPrivacySuppressionReasonV1 {
    /// Stable label used across telemetry exports and dashboards.
    #[must_use]
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::InsufficientContributors => "insufficient_contributors",
            Self::CollectorSuppressed => "collector_suppressed",
            Self::CollectorWindowElapsed => "collector_window_elapsed",
            Self::ForcedFlushWindowElapsed => "forced_flush_window_elapsed",
        }
    }
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for SoranetPrivacySuppressionReasonV1 {
    fn write_json(&self, out: &mut String) {
        norito::json::write_json_string(self.as_label(), out);
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for SoranetPrivacySuppressionReasonV1 {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = parser.parse_string()?;
        match value.as_str() {
            "insufficient_contributors" => Ok(Self::InsufficientContributors),
            "collector_suppressed" => Ok(Self::CollectorSuppressed),
            "collector_window_elapsed" => Ok(Self::CollectorWindowElapsed),
            "forced_flush_window_elapsed" => Ok(Self::ForcedFlushWindowElapsed),
            other => Err(norito::json::Error::unknown_field(other)),
        }
    }
}

/// Privacy-preserving telemetry event ingested by the secure aggregator.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SoranetPrivacyEventV1 {
    /// UNIX timestamp (seconds) indicating when the observation occurred.
    pub timestamp_unix: u64,
    /// Relay mode that authored the observation.
    pub mode: SoranetPrivacyModeV1,
    /// Event payload describing the observation.
    pub kind: SoranetPrivacyEventKindV1,
}

/// Enumeration of privacy telemetry event kinds.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(DeriveJsonSerialize, DeriveJsonDeserialize),
    norito(tag = "kind", content = "payload")
)]
pub enum SoranetPrivacyEventKindV1 {
    /// Successful anonymous circuit establishment.
    HandshakeSuccess(SoranetPrivacyEventHandshakeSuccessV1),
    /// Failed handshake accompanied by the failure reason.
    HandshakeFailure(SoranetPrivacyEventHandshakeFailureV1),
    /// Throttling decision emitted by relay abuse controls.
    Throttle(SoranetPrivacyEventThrottleV1),
    /// Snapshot of currently active anonymous circuits.
    ActiveSample(SoranetPrivacyEventActiveSampleV1),
    /// Verified bandwidth contribution, expressed in bytes.
    VerifiedBytes(SoranetPrivacyEventVerifiedBytesV1),
    /// GAR abuse report categorised under a hashed label.
    GarAbuseCategory(SoranetPrivacyEventGarAbuseCategoryV1),
}

/// Payload describing a successful anonymous circuit establishment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SoranetPrivacyEventHandshakeSuccessV1 {
    /// Optional RTT measurement captured for the handshake (milliseconds).
    #[norito(default)]
    pub rtt_ms: Option<u64>,
    /// Optional active circuit count immediately after the handshake succeeded.
    #[norito(default)]
    pub active_circuits_after: Option<u64>,
}

/// Payload describing a failed handshake event. This struct is intentionally
/// `Clone`-only because the detail slug remains an owned `String`.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SoranetPrivacyEventHandshakeFailureV1 {
    /// Reason explaining why the handshake failed.
    pub reason: SoranetPrivacyHandshakeFailureV1,
    /// Optional downgrade detail slug (e.g., `suite_no_overlap`).
    #[norito(default)]
    pub detail: Option<String>,
    /// Optional RTT measurement captured for the failed handshake (milliseconds).
    #[norito(default)]
    pub rtt_ms: Option<u64>,
}

/// Payload describing a throttling decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SoranetPrivacyEventThrottleV1 {
    /// Scope of the throttle.
    pub scope: SoranetPrivacyThrottleScopeV1,
}

/// Payload describing an active circuits sample.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SoranetPrivacyEventActiveSampleV1 {
    /// Number of active circuits observed.
    pub active_circuits: u64,
}

/// Payload describing a verified bandwidth contribution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SoranetPrivacyEventVerifiedBytesV1 {
    /// Total verified bytes relayed during the observation window.
    pub bytes: u128,
}

/// Payload describing a GAR abuse category report.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SoranetPrivacyEventGarAbuseCategoryV1 {
    /// Raw GAR category label (hashed before aggregation).
    pub label: String,
}

/// Handshake failure classification surfaced by telemetry events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
pub enum SoranetPrivacyHandshakeFailureV1 {
    /// Proof-of-work validation rejected the ticket.
    Pow,
    /// The handshake timed out.
    Timeout,
    /// Capability negotiation detected a downgrade attempt.
    Downgrade,
    /// Any other failure category.
    Other,
}

/// Classification for `PoW` validation failures to aid telemetry and dashboards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
pub enum SoranetPowFailureReasonV1 {
    /// Ticket digest failed the predicate.
    InvalidSolution,
    /// Ticket difficulty did not match policy.
    DifficultyMismatch,
    /// Ticket expired before verification.
    Expired,
    /// Ticket expiry exceeded the allowed skew.
    FutureSkewExceeded,
    /// Ticket lifetime fell below the minimum TTL.
    TtlTooShort,
    /// Ticket version is not supported.
    UnsupportedVersion,
    /// Signature over a signed ticket failed verification.
    SignatureInvalid,
    /// Post-quantum crypto error while decoding signature data.
    PostQuantumError,
    /// System clock error while validating expiry.
    ClockError,
    /// Signed ticket was presented to the wrong relay.
    RelayMismatch,
    /// Signed ticket was replayed or revoked.
    Replay,
    /// Revocation store was unavailable or rejected persistence.
    StoreError,
}

impl SoranetPowFailureReasonV1 {
    /// Stable label used across telemetry exports and dashboards.
    #[must_use]
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::InvalidSolution => "invalid_solution",
            Self::DifficultyMismatch => "difficulty_mismatch",
            Self::Expired => "expired",
            Self::FutureSkewExceeded => "future_skew_exceeded",
            Self::TtlTooShort => "ttl_too_short",
            Self::UnsupportedVersion => "unsupported_version",
            Self::SignatureInvalid => "signature_invalid",
            Self::PostQuantumError => "post_quantum_error",
            Self::ClockError => "clock_error",
            Self::RelayMismatch => "relay_mismatch",
            Self::Replay => "replay",
            Self::StoreError => "store_error",
        }
    }
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for SoranetPowFailureReasonV1 {
    fn write_json(&self, out: &mut String) {
        norito::json::write_json_string(self.as_label(), out);
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for SoranetPowFailureReasonV1 {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = parser.parse_string()?;
        match value.as_str() {
            "invalid_solution" => Ok(Self::InvalidSolution),
            "difficulty_mismatch" => Ok(Self::DifficultyMismatch),
            "expired" => Ok(Self::Expired),
            "future_skew_exceeded" => Ok(Self::FutureSkewExceeded),
            "ttl_too_short" => Ok(Self::TtlTooShort),
            "unsupported_version" => Ok(Self::UnsupportedVersion),
            "signature_invalid" => Ok(Self::SignatureInvalid),
            "post_quantum_error" => Ok(Self::PostQuantumError),
            "clock_error" => Ok(Self::ClockError),
            "relay_mismatch" => Ok(Self::RelayMismatch),
            "replay" => Ok(Self::Replay),
            "store_error" => Ok(Self::StoreError),
            other => Err(norito::json::Error::unknown_field(other)),
        }
    }
}

/// Count of `PoW` validation failures grouped by reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SoranetPowFailureCountV1 {
    /// Reason describing why the `PoW` ticket failed verification.
    pub reason: SoranetPowFailureReasonV1,
    /// Number of failures attributed to this reason.
    pub count: u64,
}

/// Throttle scopes surfaced by telemetry events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
pub enum SoranetPrivacyThrottleScopeV1 {
    /// Congestion controller denied the request.
    Congestion,
    /// Cooldown limits prevented the handshake.
    Cooldown,
    /// Emergency consensus controls rejected the descriptor.
    Emergency,
    /// Remote-quota limits throttled the origin.
    RemoteQuota,
    /// Descriptor-level quota triggered the throttle.
    DescriptorQuota,
    /// Descriptor replay protection rejected the handshake.
    DescriptorReplay,
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for SoranetPrivacyThrottleScopeV1 {
    fn write_json(&self, out: &mut String) {
        let label = match self {
            Self::Congestion => "congestion",
            Self::Cooldown => "cooldown",
            Self::Emergency => "emergency",
            Self::RemoteQuota => "remote_quota",
            Self::DescriptorQuota => "descriptor_quota",
            Self::DescriptorReplay => "descriptor_replay",
        };
        norito::json::write_json_string(label, out);
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for SoranetPrivacyThrottleScopeV1 {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = parser.parse_string()?;
        match value.as_str() {
            "congestion" => Ok(Self::Congestion),
            "cooldown" => Ok(Self::Cooldown),
            "emergency" => Ok(Self::Emergency),
            "remote_quota" => Ok(Self::RemoteQuota),
            "descriptor_quota" => Ok(Self::DescriptorQuota),
            "descriptor_replay" => Ok(Self::DescriptorReplay),
            other => Err(norito::json::Error::unknown_field(other)),
        }
    }
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for SoranetPrivacyHandshakeFailureV1 {
    fn write_json(&self, out: &mut String) {
        let label = match self {
            Self::Pow => "pow",
            Self::Timeout => "timeout",
            Self::Downgrade => "downgrade",
            Self::Other => "other",
        };
        norito::json::write_json_string(label, out);
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for SoranetPrivacyHandshakeFailureV1 {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = parser.parse_string()?;
        match value.as_str() {
            "pow" => Ok(Self::Pow),
            "timeout" => Ok(Self::Timeout),
            "downgrade" => Ok(Self::Downgrade),
            "other" => Ok(Self::Other),
            other => Err(norito::json::Error::unknown_field(other)),
        }
    }
}
