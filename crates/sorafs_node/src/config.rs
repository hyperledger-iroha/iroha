//! Storage configuration helpers for the embedded SoraFS worker.

use std::path::{Path, PathBuf};

use iroha_config::parameters::actual;

use crate::metering::SmoothingConfig;

/// Convenience wrapper around the Torii-level SoraFS storage configuration.
#[derive(Debug, Clone)]
pub struct StorageConfig {
    enabled: bool,
    data_dir: PathBuf,
    max_capacity_bytes: iroha_config::base::util::Bytes<u64>,
    max_parallel_fetches: usize,
    max_pins: usize,
    por_sample_interval_secs: u64,
    alias: Option<String>,
    adverts: AdvertOverrides,
    metering_smoothing: MeteringSmoothingConfig,
    stream_token_signing_key_path: Option<PathBuf>,
    governance_dir: Option<PathBuf>,
    penalty: PenaltySettings,
}

impl StorageConfig {
    /// Returns a builder initialised with the Torii defaults.
    #[must_use]
    pub fn builder() -> StorageConfigBuilder {
        StorageConfigBuilder::new()
    }

    /// Whether the storage worker should be active.
    #[must_use]
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Directory where chunk data and metadata are stored.
    #[must_use]
    pub fn data_dir(&self) -> &PathBuf {
        &self.data_dir
    }

    /// Maximum allowed on-disk footprint (bytes).
    #[must_use]
    pub fn max_capacity_bytes(&self) -> iroha_config::base::util::Bytes<u64> {
        self.max_capacity_bytes
    }

    /// Maximum number of concurrent fetch streams.
    #[must_use]
    pub fn max_parallel_fetches(&self) -> usize {
        self.max_parallel_fetches
    }

    /// Maximum number of manifests the node accepts before back-pressure.
    #[must_use]
    pub fn max_pins(&self) -> usize {
        self.max_pins
    }

    /// Cadence for Proof-of-Retrievability sampling (seconds).
    #[must_use]
    pub fn por_sample_interval_secs(&self) -> u64 {
        self.por_sample_interval_secs
    }

    /// Optional human-friendly alias reported in telemetry.
    #[must_use]
    pub fn alias(&self) -> Option<&String> {
        self.alias.as_ref()
    }

    /// Advert telemetry overrides emitted by the storage worker.
    #[must_use]
    pub fn adverts(&self) -> &AdvertOverrides {
        &self.adverts
    }

    /// Smoothing configuration for metering snapshots.
    #[must_use]
    pub fn metering_smoothing(&self) -> &MeteringSmoothingConfig {
        &self.metering_smoothing
    }

    /// Optional filesystem path to the gateway signing key (Ed25519).
    #[must_use]
    pub fn stream_token_signing_key_path(&self) -> Option<&PathBuf> {
        self.stream_token_signing_key_path.as_ref()
    }

    /// Optional directory used to materialise governance artefacts.
    #[must_use]
    pub fn governance_dir(&self) -> Option<&PathBuf> {
        self.governance_dir.as_ref()
    }

    /// Penalty policy applied to PoR failures.
    #[must_use]
    pub fn penalty(&self) -> &PenaltySettings {
        &self.penalty
    }

    /// Convenience helper that converts the stored smoothing parameters
    /// into the runtime [`SmoothingConfig`].
    #[must_use]
    pub fn smoothing_config(&self) -> SmoothingConfig {
        self.metering_smoothing.to_metering_config()
    }
}

impl From<actual::SorafsStorage> for StorageConfig {
    fn from(value: actual::SorafsStorage) -> Self {
        Self::from(&value)
    }
}

impl From<&actual::SorafsStorage> for StorageConfig {
    fn from(value: &actual::SorafsStorage) -> Self {
        Self::from_storage_and_penalty(value, &actual::SorafsPenaltyPolicy::default())
    }
}

impl StorageConfig {
    /// Construct a storage configuration using storage + penalty policy inputs.
    #[must_use]
    pub fn from_storage_and_penalty(
        storage: &actual::SorafsStorage,
        penalty: &actual::SorafsPenaltyPolicy,
    ) -> Self {
        Self {
            enabled: storage.enabled,
            data_dir: storage.data_dir.clone(),
            max_capacity_bytes: storage.max_capacity_bytes,
            max_parallel_fetches: storage.max_parallel_fetches,
            max_pins: storage.max_pins,
            por_sample_interval_secs: storage.por_sample_interval_secs,
            alias: storage.alias.clone(),
            adverts: AdvertOverrides::from(&storage.adverts),
            metering_smoothing: MeteringSmoothingConfig::from(&storage.metering_smoothing),
            stream_token_signing_key_path: storage.stream_tokens.signing_key_path.clone(),
            governance_dir: storage.governance_dag_dir.clone(),
            penalty: PenaltySettings::from_policy(penalty),
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self::from(actual::SorafsStorage::default())
    }
}

/// Builder for [`StorageConfig`].
#[derive(Debug, Clone)]
pub struct StorageConfigBuilder {
    inner: StorageConfig,
}

impl StorageConfigBuilder {
    fn new() -> Self {
        Self {
            inner: StorageConfig::default(),
        }
    }

    /// Enable or disable the storage worker.
    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.inner.enabled = enabled;
        self
    }

    /// Override the storage data directory.
    #[must_use]
    pub fn data_dir(mut self, data_dir: PathBuf) -> Self {
        self.inner.data_dir = data_dir;
        self
    }

    /// Set the capacity ceiling (bytes).
    #[must_use]
    pub fn max_capacity_bytes(mut self, bytes: iroha_config::base::util::Bytes<u64>) -> Self {
        self.inner.max_capacity_bytes = bytes;
        self
    }

    /// Set the fetch concurrency budget.
    #[must_use]
    pub fn max_parallel_fetches(mut self, fetches: usize) -> Self {
        self.inner.max_parallel_fetches = fetches;
        self
    }

    /// Set the pin limit before back-pressure.
    #[must_use]
    pub fn max_pins(mut self, pins: usize) -> Self {
        self.inner.max_pins = pins;
        self
    }

    /// Set the PoR sampling cadence (seconds).
    #[must_use]
    pub fn por_sample_interval_secs(mut self, interval: u64) -> Self {
        self.inner.por_sample_interval_secs = interval;
        self
    }

    /// Override the telemetry alias.
    #[must_use]
    pub fn alias<S: Into<Option<String>>>(mut self, alias: S) -> Self {
        self.inner.alias = alias.into();
        self
    }

    /// Override advert telemetry data.
    #[must_use]
    pub fn adverts(mut self, adverts: AdvertOverrides) -> Self {
        self.inner.adverts = adverts;
        self
    }

    /// Override the gateway signing key path used for stream tokens and PoR proofs.
    #[must_use]
    pub fn stream_token_signing_key_path(mut self, path: Option<PathBuf>) -> Self {
        self.inner.stream_token_signing_key_path = path;
        self
    }

    /// Override the metering smoothing parameters.
    #[must_use]
    pub fn metering_smoothing(mut self, smoothing: MeteringSmoothingConfig) -> Self {
        self.inner.metering_smoothing = smoothing;
        self
    }

    /// Override the governance artefact directory.
    #[must_use]
    pub fn governance_dir<P: Into<Option<PathBuf>>>(mut self, dir: P) -> Self {
        self.inner.governance_dir = dir.into();
        self
    }

    /// Override the strike threshold applied to consecutive PoR failures before slashing.
    #[must_use]
    pub fn penalty_strike_threshold(mut self, threshold: u32) -> Self {
        self.inner.penalty.strike_threshold = threshold;
        self
    }

    /// Override the bond percentage slashed when the strike threshold is exceeded (basis points).
    #[must_use]
    pub fn penalty_bond_bps(mut self, bps: u16) -> Self {
        self.inner.penalty.penalty_bond_bps = bps.min(10_000);
        self
    }

    /// Override the cooldown window (seconds) enforced between slashes.
    #[must_use]
    pub fn penalty_cooldown_secs(mut self, cooldown_secs: u64) -> Self {
        self.inner.penalty.cooldown_secs = cooldown_secs;
        self
    }

    /// Finalise the builder into a configuration.
    #[must_use]
    pub fn build(self) -> StorageConfig {
        self.inner
    }
}

/// Governance policy controlling repair escalation decisions.
#[derive(Debug, Clone, Copy)]
pub struct RepairEscalationPolicy {
    quorum_bps: u16,
    minimum_voters: u32,
    dispute_window_secs: u64,
    appeal_window_secs: u64,
    max_penalty_nano: u128,
}

impl RepairEscalationPolicy {
    /// Construct a policy from the governance configuration.
    pub fn from_policy(policy: &actual::RepairEscalationPolicyV1) -> Self {
        Self {
            quorum_bps: policy.quorum_bps.min(10_000),
            minimum_voters: policy.minimum_voters.max(1),
            dispute_window_secs: policy.dispute_window_secs,
            appeal_window_secs: policy.appeal_window_secs,
            max_penalty_nano: policy.max_penalty_nano,
        }
    }

    /// Approval quorum (basis points) required to approve a decision.
    #[must_use]
    pub fn quorum_bps(&self) -> u16 {
        self.quorum_bps
    }

    /// Minimum number of distinct voters required to resolve a decision.
    #[must_use]
    pub fn minimum_voters(&self) -> u32 {
        self.minimum_voters
    }

    /// Dispute window in seconds after escalation before governance finalizes.
    #[must_use]
    pub fn dispute_window_secs(&self) -> u64 {
        self.dispute_window_secs
    }

    /// Appeal window in seconds after approval before a decision is final.
    #[must_use]
    pub fn appeal_window_secs(&self) -> u64 {
        self.appeal_window_secs
    }

    /// Maximum slash penalty allowed for repair escalation proposals (nano-XOR).
    #[must_use]
    pub fn max_penalty_nano(&self) -> u128 {
        self.max_penalty_nano
    }

    /// Clamp a proposed penalty to the configured maximum.
    #[must_use]
    pub fn cap_penalty(&self, penalty: u128) -> u128 {
        penalty.min(self.max_penalty_nano)
    }
}

impl Default for RepairEscalationPolicy {
    fn default() -> Self {
        Self::from_policy(&actual::RepairEscalationPolicyV1::default())
    }
}

/// Repair scheduler configuration resolved from the runtime config.
#[derive(Debug, Clone)]
pub struct RepairConfig {
    enabled: bool,
    state_dir: Option<PathBuf>,
    claim_ttl_secs: u64,
    heartbeat_interval_secs: u64,
    max_attempts: u32,
    worker_concurrency: usize,
    backoff_initial_secs: u64,
    backoff_max_secs: u64,
    default_slash_penalty_nano: u128,
    escalation_policy: RepairEscalationPolicy,
}

impl RepairConfig {
    /// Whether the repair scheduler is enabled.
    #[must_use]
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Optional directory for durable repair state.
    #[must_use]
    pub fn state_dir(&self) -> Option<&PathBuf> {
        self.state_dir.as_ref()
    }

    /// Claim TTL for repair tickets (seconds).
    #[must_use]
    pub fn claim_ttl_secs(&self) -> u64 {
        self.claim_ttl_secs
    }

    /// Heartbeat interval/TTL for active claims (seconds).
    #[must_use]
    pub fn heartbeat_interval_secs(&self) -> u64 {
        self.heartbeat_interval_secs
    }

    /// Maximum number of attempts before escalation.
    #[must_use]
    pub fn max_attempts(&self) -> u32 {
        self.max_attempts
    }

    /// Concurrent repair workers per node.
    #[must_use]
    pub fn worker_concurrency(&self) -> usize {
        self.worker_concurrency
    }

    /// Initial retry backoff for failed repairs (seconds).
    #[must_use]
    pub fn backoff_initial_secs(&self) -> u64 {
        self.backoff_initial_secs
    }

    /// Maximum retry backoff for failed repairs (seconds).
    #[must_use]
    pub fn backoff_max_secs(&self) -> u64 {
        self.backoff_max_secs
    }

    /// Default penalty used for scheduler-generated slash proposals (nano-XOR).
    #[must_use]
    pub fn default_slash_penalty_nano(&self) -> u128 {
        self.default_slash_penalty_nano
    }

    /// Governance policy for escalation/quorum enforcement.
    #[must_use]
    pub fn escalation_policy(&self) -> &RepairEscalationPolicy {
        &self.escalation_policy
    }

    /// Override the escalation governance policy.
    #[must_use]
    pub fn with_escalation_policy(mut self, policy: RepairEscalationPolicy) -> Self {
        self.escalation_policy = policy;
        self
    }

    /// Apply a default state directory when one is not provided.
    #[must_use]
    pub fn with_default_state_dir(mut self, data_dir: &Path) -> Self {
        if self.state_dir.is_none() {
            self.state_dir = Some(data_dir.join("repair"));
        }
        self
    }
}

impl Default for RepairConfig {
    fn default() -> Self {
        Self::from(&actual::SorafsRepair::default())
    }
}

impl From<actual::SorafsRepair> for RepairConfig {
    fn from(value: actual::SorafsRepair) -> Self {
        Self::from(&value)
    }
}

impl From<&actual::SorafsRepair> for RepairConfig {
    fn from(value: &actual::SorafsRepair) -> Self {
        Self::from_repair_and_policy(value, &actual::RepairEscalationPolicyV1::default())
    }
}

impl RepairConfig {
    /// Build a repair config from runtime settings and the governance escalation policy.
    #[must_use]
    pub fn from_repair_and_policy(
        repair: &actual::SorafsRepair,
        policy: &actual::RepairEscalationPolicyV1,
    ) -> Self {
        Self {
            enabled: repair.enabled,
            state_dir: repair.state_dir.clone(),
            claim_ttl_secs: repair.claim_ttl_secs,
            heartbeat_interval_secs: repair.heartbeat_interval_secs,
            max_attempts: repair.max_attempts,
            worker_concurrency: repair.worker_concurrency,
            backoff_initial_secs: repair.backoff_initial_secs,
            backoff_max_secs: repair.backoff_max_secs,
            default_slash_penalty_nano: repair.default_slash_penalty_nano,
            escalation_policy: RepairEscalationPolicy::from_policy(policy),
        }
    }
}

/// GC scheduler configuration resolved from the runtime config.
#[derive(Debug, Clone)]
pub struct GcConfig {
    enabled: bool,
    state_dir: Option<PathBuf>,
    interval_secs: u64,
    max_deletions_per_run: u32,
    retention_grace_secs: u64,
    pre_admission_sweep: bool,
}

impl GcConfig {
    /// Whether the GC worker is enabled.
    #[must_use]
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Optional directory for durable GC state.
    #[must_use]
    pub fn state_dir(&self) -> Option<&PathBuf> {
        self.state_dir.as_ref()
    }

    /// GC cadence (seconds).
    #[must_use]
    pub fn interval_secs(&self) -> u64 {
        self.interval_secs
    }

    /// Maximum deletions per GC run.
    #[must_use]
    pub fn max_deletions_per_run(&self) -> u32 {
        self.max_deletions_per_run
    }

    /// Grace window for retention expiry (seconds).
    #[must_use]
    pub fn retention_grace_secs(&self) -> u64 {
        self.retention_grace_secs
    }

    /// Whether a GC sweep is attempted before rejecting new pins.
    #[must_use]
    pub fn pre_admission_sweep(&self) -> bool {
        self.pre_admission_sweep
    }

    /// Apply a default state directory when one is not provided.
    #[must_use]
    pub fn with_default_state_dir(mut self, data_dir: &Path) -> Self {
        if self.state_dir.is_none() {
            self.state_dir = Some(data_dir.join("gc"));
        }
        self
    }
}

impl Default for GcConfig {
    fn default() -> Self {
        Self::from(&actual::SorafsGc::default())
    }
}

impl From<actual::SorafsGc> for GcConfig {
    fn from(value: actual::SorafsGc) -> Self {
        Self::from(&value)
    }
}

impl From<&actual::SorafsGc> for GcConfig {
    fn from(value: &actual::SorafsGc) -> Self {
        Self {
            enabled: value.enabled,
            state_dir: value.state_dir.clone(),
            interval_secs: value.interval_secs,
            max_deletions_per_run: value.max_deletions_per_run,
            retention_grace_secs: value.retention_grace_secs,
            pre_admission_sweep: value.pre_admission_sweep,
        }
    }
}

/// Optional overrides for provider advert telemetry.
#[derive(Debug, Clone)]
pub struct AdvertOverrides {
    stake_pointer: Option<String>,
    availability: String,
    max_latency_ms: u32,
    topics: Vec<String>,
}

impl AdvertOverrides {
    /// Construct overrides from individual fields.
    #[must_use]
    pub fn new(
        stake_pointer: Option<String>,
        availability: impl Into<String>,
        max_latency_ms: u32,
        topics: Vec<String>,
    ) -> Self {
        Self {
            stake_pointer,
            availability: availability.into(),
            max_latency_ms,
            topics,
        }
    }

    /// Stake pointer advertised alongside provider metadata.
    #[must_use]
    pub fn stake_pointer(&self) -> Option<&String> {
        self.stake_pointer.as_ref()
    }

    /// Availability tier advertised by the provider.
    #[must_use]
    pub fn availability(&self) -> &str {
        &self.availability
    }

    /// Maximum advertised retrieval latency (milliseconds).
    #[must_use]
    pub fn max_latency_ms(&self) -> u32 {
        self.max_latency_ms
    }

    /// Rendezvous topics published for discovery.
    #[must_use]
    pub fn topics(&self) -> &[String] {
        &self.topics
    }
}

/// Per-metric smoothing configuration used by the embedded capacity meter.
#[derive(Debug, Clone, Default)]
pub struct MeteringSmoothingConfig {
    gib_hours_alpha: Option<f64>,
    por_success_alpha: Option<f64>,
}

impl MeteringSmoothingConfig {
    /// Construct a configuration from optional alpha values.
    #[must_use]
    pub fn new(gib_hours_alpha: Option<f64>, por_success_alpha: Option<f64>) -> Self {
        Self {
            gib_hours_alpha: Self::sanitize_alpha(gib_hours_alpha),
            por_success_alpha: Self::sanitize_alpha(por_success_alpha),
        }
    }

    /// Set the GiB·hour smoothing alpha (values <= 0 disable smoothing).
    #[must_use]
    pub fn with_gib_hours_alpha(mut self, alpha: f64) -> Self {
        self.gib_hours_alpha = Self::sanitize_alpha(Some(alpha));
        self
    }

    /// Set the PoR success smoothing alpha (values <= 0 disable smoothing).
    #[must_use]
    pub fn with_por_success_alpha(mut self, alpha: f64) -> Self {
        self.por_success_alpha = Self::sanitize_alpha(Some(alpha));
        self
    }

    /// Return the configured GiB·hour smoothing alpha, if any.
    #[must_use]
    pub fn gib_hours_alpha(&self) -> Option<f64> {
        self.gib_hours_alpha
    }

    /// Return the configured PoR success smoothing alpha, if any.
    #[must_use]
    pub fn por_success_alpha(&self) -> Option<f64> {
        self.por_success_alpha
    }

    /// Convert into the runtime smoothing configuration used by the meter.
    #[must_use]
    pub fn to_metering_config(&self) -> SmoothingConfig {
        SmoothingConfig::from_optional_alphas(self.gib_hours_alpha, self.por_success_alpha)
    }

    fn sanitize_alpha(alpha: Option<f64>) -> Option<f64> {
        alpha.and_then(|value| {
            if value <= 0.0 {
                None
            } else {
                Some(value.min(1.0))
            }
        })
    }
}

impl From<&actual::SorafsMeteringSmoothing> for MeteringSmoothingConfig {
    fn from(value: &actual::SorafsMeteringSmoothing) -> Self {
        Self::new(value.gib_hours_alpha, value.por_success_alpha)
    }
}

impl Default for AdvertOverrides {
    fn default() -> Self {
        let defaults = actual::SorafsAdvertOverrides::default();
        Self {
            stake_pointer: None,
            availability: defaults.availability,
            max_latency_ms: defaults.max_latency_ms,
            topics: defaults.topics,
        }
    }
}

impl From<actual::SorafsAdvertOverrides> for AdvertOverrides {
    fn from(value: actual::SorafsAdvertOverrides) -> Self {
        Self::from(&value)
    }
}

impl From<&actual::SorafsAdvertOverrides> for AdvertOverrides {
    fn from(value: &actual::SorafsAdvertOverrides) -> Self {
        Self {
            stake_pointer: value.stake_pointer.clone(),
            availability: value.availability.clone(),
            max_latency_ms: value.max_latency_ms,
            topics: value.topics.clone(),
        }
    }
}

/// Penalty policy controlling PoR failure escalation and slashing.
#[derive(Debug, Clone, Copy)]
pub struct PenaltySettings {
    /// Consecutive PoR failures required to trigger a slash.
    pub strike_threshold: u32,
    /// Bond percentage slashed when the strike threshold is exceeded.
    pub penalty_bond_bps: u16,
    /// Cooldown (seconds) enforced between slashes.
    pub cooldown_secs: u64,
}

impl Default for PenaltySettings {
    fn default() -> Self {
        let defaults = actual::SorafsPenaltyPolicy::default();
        Self {
            strike_threshold: defaults.strike_threshold,
            penalty_bond_bps: defaults.penalty_bond_bps,
            // Cooldown windows are expressed in settlement windows (hours); default to hourly cadence.
            cooldown_secs: u64::from(defaults.cooldown_windows).saturating_mul(60 * 60),
        }
    }
}

impl PenaltySettings {
    /// Construct settings from the configured penalty policy.
    pub fn from_policy(policy: &actual::SorafsPenaltyPolicy) -> Self {
        Self {
            strike_threshold: policy.strike_threshold,
            penalty_bond_bps: policy.penalty_bond_bps,
            cooldown_secs: u64::from(policy.cooldown_windows).saturating_mul(60 * 60),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversion_from_actual_preserves_fields() {
        let mut actual = actual::SorafsStorage::default();
        actual.enabled = true;
        actual.data_dir = PathBuf::from("/tmp/sorafs");
        actual.max_capacity_bytes = iroha_config::base::util::Bytes(1_024);
        actual.max_parallel_fetches = 99;
        actual.max_pins = 1_001;
        actual.por_sample_interval_secs = 42;
        actual.alias = Some("tenant.alpha".into());
        actual.adverts = actual::SorafsAdvertOverrides {
            stake_pointer: Some("stake.pool:abcd".into()),
            availability: "warm".into(),
            max_latency_ms: 750,
            topics: vec![
                "sorafs.sf1.primary:global".into(),
                "sorafs.sf1.backup:eu".into(),
            ],
        };

        let cfg = StorageConfig::from(&actual);
        assert!(cfg.enabled());
        assert_eq!(cfg.data_dir(), &PathBuf::from("/tmp/sorafs"));
        assert_eq!(cfg.max_capacity_bytes().0, 1_024);
        assert_eq!(cfg.max_parallel_fetches(), 99);
        assert_eq!(cfg.max_pins(), 1_001);
        assert_eq!(cfg.por_sample_interval_secs(), 42);
        assert_eq!(cfg.alias(), Some(&"tenant.alpha".to_string()));
        let adverts = cfg.adverts();
        assert_eq!(
            adverts.stake_pointer(),
            Some(&"stake.pool:abcd".to_string())
        );
        assert_eq!(adverts.availability(), "warm");
        assert_eq!(adverts.max_latency_ms(), 750);
        assert_eq!(
            adverts.topics(),
            &[
                "sorafs.sf1.primary:global".to_string(),
                "sorafs.sf1.backup:eu".to_string()
            ]
        );
        let penalty = cfg.penalty();
        let defaults = actual::SorafsPenaltyPolicy::default();
        assert_eq!(penalty.strike_threshold, defaults.strike_threshold);
        assert_eq!(penalty.penalty_bond_bps, defaults.penalty_bond_bps);
        assert_eq!(
            penalty.cooldown_secs,
            u64::from(defaults.cooldown_windows).saturating_mul(60 * 60)
        );
    }

    #[test]
    fn repair_and_gc_configs_preserve_fields() {
        let repair = actual::SorafsRepair {
            enabled: true,
            state_dir: Some(PathBuf::from("/tmp/repair_state")),
            claim_ttl_secs: 900,
            heartbeat_interval_secs: 45,
            max_attempts: 6,
            worker_concurrency: 12,
            backoff_initial_secs: 7,
            backoff_max_secs: 120,
            default_slash_penalty_nano: 5_000,
        };

        let policy = actual::RepairEscalationPolicyV1 {
            quorum_bps: 7_000,
            minimum_voters: 4,
            dispute_window_secs: 12_000,
            appeal_window_secs: 24_000,
            max_penalty_nano: 9_000,
        };
        let cfg = RepairConfig::from_repair_and_policy(&repair, &policy);
        assert!(cfg.enabled());
        assert_eq!(cfg.state_dir(), Some(&PathBuf::from("/tmp/repair_state")));
        assert_eq!(cfg.claim_ttl_secs(), 900);
        assert_eq!(cfg.heartbeat_interval_secs(), 45);
        assert_eq!(cfg.max_attempts(), 6);
        assert_eq!(cfg.worker_concurrency(), 12);
        assert_eq!(cfg.backoff_initial_secs(), 7);
        assert_eq!(cfg.backoff_max_secs(), 120);
        assert_eq!(cfg.default_slash_penalty_nano(), 5_000);
        assert_eq!(cfg.escalation_policy().quorum_bps(), 7_000);
        assert_eq!(cfg.escalation_policy().minimum_voters(), 4);
        assert_eq!(cfg.escalation_policy().dispute_window_secs(), 12_000);
        assert_eq!(cfg.escalation_policy().appeal_window_secs(), 24_000);
        assert_eq!(cfg.escalation_policy().max_penalty_nano(), 9_000);

        let gc = actual::SorafsGc {
            enabled: true,
            state_dir: Some(PathBuf::from("/tmp/gc_state")),
            interval_secs: 300,
            max_deletions_per_run: 2_000,
            retention_grace_secs: 86_400,
            pre_admission_sweep: false,
        };

        let gc_cfg = GcConfig::from(&gc);
        assert!(gc_cfg.enabled());
        assert_eq!(gc_cfg.state_dir(), Some(&PathBuf::from("/tmp/gc_state")));
        assert_eq!(gc_cfg.interval_secs(), 300);
        assert_eq!(gc_cfg.max_deletions_per_run(), 2_000);
        assert_eq!(gc_cfg.retention_grace_secs(), 86_400);
        assert!(!gc_cfg.pre_admission_sweep());
    }

    #[test]
    fn repair_and_gc_default_state_dirs_follow_storage_root() {
        let data_dir = PathBuf::from("/var/lib/sorafs");
        let repair =
            RepairConfig::from(&actual::SorafsRepair::default()).with_default_state_dir(&data_dir);
        let gc = GcConfig::from(&actual::SorafsGc::default()).with_default_state_dir(&data_dir);

        assert_eq!(repair.state_dir(), Some(&data_dir.join("repair")));
        assert_eq!(gc.state_dir(), Some(&data_dir.join("gc")));
    }
}
