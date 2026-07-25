//! Storage configuration helpers for the embedded SoraFS worker.

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use iroha_config::parameters::actual;
use iroha_data_model::{
    prelude::Quantity,
    sorafs::transparency::{
        MODERATION_PRIVACY_PARAMETERS_VERSION_V1, ModerationPrivacyModeV1,
        ModerationPrivacyParametersV1,
    },
};
use sorafs_manifest::deal::XorQuantity;

use crate::{
    metering::SmoothingConfig,
    pdp_provider::{PDP_PROVIDER_POLICY_VERSION_V1, PdpProviderProtocolPolicyV1},
    transparency::{
        PrivacyAggregateCycleConfig, PrivacyAggregateScheduleConfig,
        PrivacyCompositionBudgetPolicyV1,
    },
};

/// Convenience wrapper around the Torii-level SoraFS storage configuration.
#[derive(Debug, Clone)]
pub struct StorageConfig {
    enabled: bool,
    data_dir: PathBuf,
    max_capacity_bytes: iroha_config::base::util::Bytes<u64>,
    max_parallel_fetches: usize,
    max_pins: usize,
    por_sample_interval_secs: u64,
    pdp_sample_window: u16,
    pdp_tree_memory_limit_bytes: iroha_config::base::util::Bytes<u64>,
    moderation_screening_enabled: bool,
    moderation_screening_authority_bundle_path: Option<PathBuf>,
    moderation_screening_authority_bundle_digest: Option<[u8; 32]>,
    pdp_provider: PdpProviderProtocolPolicyV1,
    runtime_retention: RuntimeRetentionPolicy,
    alias: Option<String>,
    adverts: AdvertOverrides,
    metering_smoothing: MeteringSmoothingConfig,
    stream_token_signing_key_path: Option<PathBuf>,
    orderbook: OrderbookAdmissionPolicy,
    reputation_trust_policy_path: Option<PathBuf>,
    pricing_trust_policy_path: Option<PathBuf>,
    hedging_feed_trust_policy_path: Option<PathBuf>,
    privacy_aggregate_schedule: Option<PrivacyAggregateScheduleConfig>,
    privacy_aggregate_policy: Option<PrivacyAggregatePolicyConfig>,
    evidence_viewer_audit_schedule: Option<PrivacyAggregateScheduleConfig>,
    reserve_lifecycle_schedule: Option<ReserveLifecycleScheduleConfig>,
    governance_dir: Option<PathBuf>,
    governance_dag_publisher_peer_id: Option<String>,
    governance_dag_signing_key_path: Option<PathBuf>,
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

    /// Maximum PDP segments that one governed challenge may sample.
    #[must_use]
    pub fn pdp_sample_window(&self) -> u16 {
        self.pdp_sample_window
    }

    /// Aggregate in-memory budget for canonical PDP tree indexes.
    #[must_use]
    pub fn pdp_tree_memory_limit_bytes(&self) -> iroha_config::base::util::Bytes<u64> {
        self.pdp_tree_memory_limit_bytes
    }

    /// Whether authenticated moderation-screening admission is enabled.
    #[must_use]
    pub fn moderation_screening_enabled(&self) -> bool {
        self.moderation_screening_enabled
    }

    /// Canonical non-secret authority bundle configured for screening admission.
    #[must_use]
    pub fn moderation_screening_authority_bundle_path(&self) -> Option<&PathBuf> {
        self.moderation_screening_authority_bundle_path.as_ref()
    }

    /// Reviewed BLAKE3 digest of the exact canonical authority bundle bytes.
    #[must_use]
    pub fn moderation_screening_authority_bundle_digest(&self) -> Option<[u8; 32]> {
        self.moderation_screening_authority_bundle_digest
    }

    /// Durable admission-bound PDP provider protocol policy.
    #[must_use]
    pub fn pdp_provider_policy(&self) -> PdpProviderProtocolPolicyV1 {
        self.pdp_provider
    }

    /// Safety ceilings for auxiliary runtime state and replay histories.
    #[must_use]
    pub fn runtime_retention(&self) -> RuntimeRetentionPolicy {
        self.runtime_retention
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

    /// Local orderbook admission policy.
    #[must_use]
    pub fn orderbook_admission_policy(&self) -> &OrderbookAdmissionPolicy {
        &self.orderbook
    }

    /// Canonical external trust-policy file used for reputation snapshot admission.
    #[must_use]
    pub fn reputation_trust_policy_path(&self) -> Option<&PathBuf> {
        self.reputation_trust_policy_path.as_ref()
    }

    /// Canonical external trust-policy file used for governed pricing admission.
    #[must_use]
    pub fn pricing_trust_policy_path(&self) -> Option<&PathBuf> {
        self.pricing_trust_policy_path.as_ref()
    }

    /// Canonical external trust-policy file used for signed hedging-feed admission.
    #[must_use]
    pub fn hedging_feed_trust_policy_path(&self) -> Option<&PathBuf> {
        self.hedging_feed_trust_policy_path.as_ref()
    }

    /// Optional config-backed privacy aggregate due-cycle scheduler.
    #[must_use]
    pub fn privacy_aggregate_schedule(&self) -> Option<PrivacyAggregateScheduleConfig> {
        self.privacy_aggregate_schedule
    }

    /// Governed config-backed privacy and composition-budget policy.
    #[must_use]
    pub fn privacy_aggregate_policy(&self) -> Option<&PrivacyAggregatePolicyConfig> {
        self.privacy_aggregate_policy.as_ref()
    }

    /// Optional config-backed evidence-viewer audit-report due-cycle scheduler.
    #[must_use]
    pub fn evidence_viewer_audit_schedule(&self) -> Option<PrivacyAggregateScheduleConfig> {
        self.evidence_viewer_audit_schedule
    }

    /// Optional config-backed reserve lifecycle advancement scheduler.
    #[must_use]
    pub fn reserve_lifecycle_schedule(&self) -> Option<ReserveLifecycleScheduleConfig> {
        self.reserve_lifecycle_schedule
    }

    /// Optional directory used to materialise governance artefacts.
    #[must_use]
    pub fn governance_dir(&self) -> Option<&PathBuf> {
        self.governance_dir.as_ref()
    }

    /// Optional publisher peer identifier for signed runtime Governance DAG blocks.
    #[must_use]
    pub fn governance_dag_publisher_peer_id(&self) -> Option<&String> {
        self.governance_dag_publisher_peer_id.as_ref()
    }

    /// Optional Ed25519 signing-key path for signed runtime Governance DAG blocks.
    #[must_use]
    pub fn governance_dag_signing_key_path(&self) -> Option<&PathBuf> {
        self.governance_dag_signing_key_path.as_ref()
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
            pdp_sample_window: storage.pdp_sample_window,
            pdp_tree_memory_limit_bytes: storage.pdp_tree_memory_limit_bytes,
            moderation_screening_enabled: storage.moderation_screening_enabled,
            moderation_screening_authority_bundle_path: storage
                .moderation_screening_authority_bundle_path
                .clone(),
            moderation_screening_authority_bundle_digest: storage
                .moderation_screening_authority_bundle_digest,
            pdp_provider: PdpProviderProtocolPolicyV1 {
                version: PDP_PROVIDER_POLICY_VERSION_V1,
                max_pending_records: storage.pdp_provider.max_pending_records,
                max_terminal_records: storage.pdp_provider.max_terminal_records,
                checkpoint_max_bytes: storage.pdp_provider.checkpoint_max_bytes.0,
                challenge_max_bytes: u32::try_from(storage.pdp_provider.challenge_max_bytes.0)
                    .unwrap_or(u32::MAX),
                proof_max_bytes: u32::try_from(storage.pdp_provider.proof_max_bytes.0)
                    .unwrap_or(u32::MAX),
                min_response_window_secs: storage.pdp_provider.min_response_window_secs,
                max_response_window_secs: storage.pdp_provider.max_response_window_secs,
                max_future_skew_secs: storage.pdp_provider.max_future_skew_secs,
                terminal_retention_secs: storage.pdp_provider.terminal_retention_secs,
            },
            runtime_retention: RuntimeRetentionPolicy::from(storage.runtime),
            alias: storage.alias.clone(),
            adverts: AdvertOverrides::from(&storage.adverts),
            metering_smoothing: MeteringSmoothingConfig::from(&storage.metering_smoothing),
            stream_token_signing_key_path: storage.stream_tokens.signing_key_path.clone(),
            orderbook: OrderbookAdmissionPolicy::from(storage.orderbook.clone()),
            reputation_trust_policy_path: storage.reputation_trust_policy_path.clone(),
            pricing_trust_policy_path: storage.pricing_trust_policy_path.clone(),
            hedging_feed_trust_policy_path: storage.hedging_feed_trust_policy_path.clone(),
            privacy_aggregate_schedule: storage.privacy_aggregates.clone().into_schedule_config(),
            privacy_aggregate_policy: storage.privacy_aggregates.clone().into_policy_config(),
            evidence_viewer_audit_schedule: storage.evidence_viewer_audits.into_schedule_config(),
            reserve_lifecycle_schedule: storage.reserve_lifecycle.into_schedule_config(),
            governance_dir: storage.governance_dag_dir.clone(),
            governance_dag_publisher_peer_id: storage.governance_dag_publisher_peer_id.clone(),
            governance_dag_signing_key_path: storage.governance_dag_signing_key_path.clone(),
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

    /// Set the maximum number of PDP segment samples in one challenge.
    #[must_use]
    pub fn pdp_sample_window(mut self, sample_window: u16) -> Self {
        self.inner.pdp_sample_window = sample_window;
        self
    }

    /// Set the aggregate memory budget for canonical PDP tree indexes.
    #[must_use]
    pub fn pdp_tree_memory_limit_bytes(
        mut self,
        bytes: iroha_config::base::util::Bytes<u64>,
    ) -> Self {
        self.inner.pdp_tree_memory_limit_bytes = bytes;
        self
    }

    /// Enable or disable authenticated moderation-screening admission.
    #[must_use]
    pub fn moderation_screening_enabled(mut self, enabled: bool) -> Self {
        self.inner.moderation_screening_enabled = enabled;
        self
    }

    /// Set the canonical non-secret screening authority bundle path.
    #[must_use]
    pub fn moderation_screening_authority_bundle_path(mut self, path: Option<PathBuf>) -> Self {
        self.inner.moderation_screening_authority_bundle_path = path;
        self
    }

    /// Set the reviewed digest of the exact screening authority bundle bytes.
    #[must_use]
    pub fn moderation_screening_authority_bundle_digest(
        mut self,
        digest: Option<[u8; 32]>,
    ) -> Self {
        self.inner.moderation_screening_authority_bundle_digest = digest;
        self
    }

    /// Override the durable admission-bound PDP provider protocol policy.
    #[must_use]
    pub fn pdp_provider_policy(mut self, policy: PdpProviderProtocolPolicyV1) -> Self {
        self.inner.pdp_provider = policy;
        self
    }

    /// Override auxiliary runtime retention and checkpoint safety ceilings.
    #[must_use]
    pub fn runtime_retention(mut self, policy: RuntimeRetentionPolicy) -> Self {
        self.inner.runtime_retention = policy;
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

    /// Override the local orderbook admission policy.
    #[must_use]
    pub fn orderbook_admission_policy(mut self, policy: OrderbookAdmissionPolicy) -> Self {
        self.inner.orderbook = policy;
        self
    }

    /// Override the canonical reputation trust-policy path.
    #[must_use]
    pub fn reputation_trust_policy_path(mut self, path: Option<PathBuf>) -> Self {
        self.inner.reputation_trust_policy_path = path;
        self
    }

    /// Set the canonical external governed-pricing trust-policy file.
    #[must_use]
    pub fn pricing_trust_policy_path(mut self, path: Option<PathBuf>) -> Self {
        self.inner.pricing_trust_policy_path = path;
        self
    }

    /// Set the canonical external signed hedging-feed trust-policy file.
    #[must_use]
    pub fn hedging_feed_trust_policy_path(mut self, path: Option<PathBuf>) -> Self {
        self.inner.hedging_feed_trust_policy_path = path;
        self
    }

    /// Override the optional config-backed privacy aggregate scheduler.
    #[must_use]
    pub fn privacy_aggregate_schedule(
        mut self,
        schedule: Option<PrivacyAggregateScheduleConfig>,
    ) -> Self {
        self.inner.privacy_aggregate_schedule = schedule;
        self
    }

    /// Override the governed config-backed privacy aggregate policy.
    #[must_use]
    pub fn privacy_aggregate_policy(
        mut self,
        policy: Option<PrivacyAggregatePolicyConfig>,
    ) -> Self {
        self.inner.privacy_aggregate_policy = policy;
        self
    }

    /// Override the optional config-backed evidence-viewer audit-report scheduler.
    #[must_use]
    pub fn evidence_viewer_audit_schedule(
        mut self,
        schedule: Option<PrivacyAggregateScheduleConfig>,
    ) -> Self {
        self.inner.evidence_viewer_audit_schedule = schedule;
        self
    }

    /// Override the optional config-backed reserve lifecycle scheduler.
    #[must_use]
    pub fn reserve_lifecycle_schedule(
        mut self,
        schedule: Option<ReserveLifecycleScheduleConfig>,
    ) -> Self {
        self.inner.reserve_lifecycle_schedule = schedule;
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

    /// Override the signed runtime Governance DAG publisher peer identifier.
    #[must_use]
    pub fn governance_dag_publisher_peer_id<S: Into<Option<String>>>(mut self, peer_id: S) -> Self {
        self.inner.governance_dag_publisher_peer_id = peer_id.into();
        self
    }

    /// Override the signed runtime Governance DAG signing key path.
    #[must_use]
    pub fn governance_dag_signing_key_path(mut self, path: Option<PathBuf>) -> Self {
        self.inner.governance_dag_signing_key_path = path;
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

/// Config-backed safety ceilings for auxiliary embedded runtime state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeRetentionPolicy {
    event_history_limit: usize,
    state_entry_limit: usize,
    checkpoint_max_bytes: u64,
    proof_outcome_forwarder_interval: Duration,
    proof_outcome_max_attempts: u32,
}

impl RuntimeRetentionPolicy {
    /// Construct a policy, clamping every safety ceiling to at least one.
    #[must_use]
    pub fn new(
        event_history_limit: usize,
        state_entry_limit: usize,
        checkpoint_max_bytes: u64,
    ) -> Self {
        let event_history_limit = event_history_limit.max(1);
        let defaults = actual::SorafsRuntimeRetention::default();
        Self {
            event_history_limit,
            state_entry_limit: state_entry_limit.max(event_history_limit),
            checkpoint_max_bytes: checkpoint_max_bytes.max(1),
            proof_outcome_forwarder_interval: defaults.proof_outcome_forwarder_interval,
            proof_outcome_max_attempts: defaults.proof_outcome_max_attempts.max(1),
        }
    }

    /// Maximum replay events retained for each event stream.
    #[must_use]
    pub fn event_history_limit(self) -> usize {
        self.event_history_limit
    }

    /// Maximum entries retained in each auxiliary state index.
    #[must_use]
    pub fn state_entry_limit(self) -> usize {
        self.state_entry_limit
    }

    /// Maximum encoded bytes accepted for one auxiliary runtime checkpoint.
    #[must_use]
    pub fn checkpoint_max_bytes(self) -> u64 {
        self.checkpoint_max_bytes
    }

    /// Finalized reconciliation cadence for durable proof-outcome delivery.
    #[must_use]
    pub fn proof_outcome_forwarder_interval(self) -> Duration {
        self.proof_outcome_forwarder_interval
    }

    /// Submission attempts allowed for one exact proof-outcome transaction.
    #[must_use]
    pub fn proof_outcome_max_attempts(self) -> u32 {
        self.proof_outcome_max_attempts
    }
}

impl From<actual::SorafsRuntimeRetention> for RuntimeRetentionPolicy {
    fn from(policy: actual::SorafsRuntimeRetention) -> Self {
        let mut resolved = Self::new(
            policy.event_history_limit,
            policy.state_entry_limit,
            policy.checkpoint_max_bytes.0,
        );
        resolved.proof_outcome_forwarder_interval = policy.proof_outcome_forwarder_interval;
        resolved.proof_outcome_max_attempts = policy.proof_outcome_max_attempts.max(1);
        resolved
    }
}

/// Config-backed local orderbook admission policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderbookAdmissionPolicy {
    min_order_gib: u64,
    price_tick: Quantity,
}

impl OrderbookAdmissionPolicy {
    /// Construct a local orderbook admission policy.
    #[must_use]
    pub fn new(min_order_gib: u64, price_tick: Quantity) -> Self {
        Self {
            min_order_gib: min_order_gib.max(1),
            price_tick,
        }
    }

    /// Minimum accepted order quantity in GiB.
    #[must_use]
    pub fn min_order_gib(&self) -> u64 {
        self.min_order_gib
    }

    /// Exact accepted XOR price tick per GiB.
    #[must_use]
    pub fn price_tick(&self) -> &Quantity {
        &self.price_tick
    }
}

impl From<actual::SorafsOrderbook> for OrderbookAdmissionPolicy {
    fn from(policy: actual::SorafsOrderbook) -> Self {
        Self::new(policy.min_order_gib, policy.price_tick)
    }
}

/// Config-backed local reserve lifecycle advancement schedule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReserveLifecycleScheduleConfig {
    interval_seconds: u64,
    initial_delay_seconds: u64,
}

impl ReserveLifecycleScheduleConfig {
    /// Construct a reserve lifecycle advancement schedule, clamping zero cadence to one second.
    #[must_use]
    pub fn new(interval_seconds: u64, initial_delay_seconds: u64) -> Self {
        Self {
            interval_seconds: interval_seconds.max(1),
            initial_delay_seconds,
        }
    }

    /// Interval between lifecycle advancement ticks, in seconds.
    #[must_use]
    pub fn interval_seconds(&self) -> u64 {
        self.interval_seconds
    }

    /// Delay before the first lifecycle advancement tick, in seconds.
    #[must_use]
    pub fn initial_delay_seconds(&self) -> u64 {
        self.initial_delay_seconds
    }
}

trait ReserveLifecycleScheduleConfigExt {
    fn into_schedule_config(self) -> Option<ReserveLifecycleScheduleConfig>;
}

impl ReserveLifecycleScheduleConfigExt for actual::SorafsReserveLifecycleSchedule {
    fn into_schedule_config(self) -> Option<ReserveLifecycleScheduleConfig> {
        self.enabled.then(|| {
            ReserveLifecycleScheduleConfig::new(self.interval_seconds, self.initial_delay_seconds)
        })
    }
}

trait PrivacyAggregateScheduleConfigExt {
    fn into_schedule_config(self) -> Option<PrivacyAggregateScheduleConfig>;
}

/// Governed V1 privacy aggregate policy sourced exclusively from `iroha_config`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyAggregatePolicyConfig {
    aggregate_id_prefix: String,
    privacy: ModerationPrivacyParametersV1,
    policy_digest: [u8; 32],
    composition_budget: PrivacyCompositionBudgetPolicyV1,
}

impl PrivacyAggregatePolicyConfig {
    /// Construct and validate one governed privacy aggregate policy.
    ///
    /// # Errors
    ///
    /// Returns an error when the public identifier, privacy parameters, policy
    /// digest, or durable composition-budget policy is not canonical.
    pub fn new(
        aggregate_id_prefix: String,
        privacy: ModerationPrivacyParametersV1,
        policy_digest: [u8; 32],
        composition_budget: PrivacyCompositionBudgetPolicyV1,
    ) -> Result<Self, String> {
        if aggregate_id_prefix.trim() != aggregate_id_prefix
            || aggregate_id_prefix.is_empty()
            || aggregate_id_prefix.len() > 128
            || aggregate_id_prefix.chars().any(char::is_control)
        {
            return Err("privacy aggregate identifier prefix is invalid".to_string());
        }
        privacy
            .validate()
            .map_err(|error| format!("privacy aggregate parameters are invalid: {error}"))?;
        if policy_digest.iter().all(|byte| *byte == 0) {
            return Err("privacy aggregate policy digest must be nonzero".to_string());
        }
        composition_budget
            .validate()
            .map_err(|error| format!("privacy composition budget is invalid: {error}"))?;
        if composition_budget.budget_id != policy_digest {
            return Err(
                "privacy composition budget must be bound to the policy digest".to_string(),
            );
        }
        Ok(Self {
            aggregate_id_prefix,
            privacy,
            policy_digest,
            composition_budget,
        })
    }

    /// Build one cycle config from the governed public policy.
    #[must_use]
    pub fn cycle_config(&self) -> PrivacyAggregateCycleConfig {
        PrivacyAggregateCycleConfig {
            aggregate_id_prefix: self.aggregate_id_prefix.clone(),
            privacy: self.privacy,
            policy_digest: Some(self.policy_digest),
            metadata: Vec::new(),
        }
    }

    /// Return the governed digest bound into threshold-PRF cycle requests.
    #[must_use]
    pub const fn policy_digest(&self) -> [u8; 32] {
        self.policy_digest
    }

    /// Return whether this policy requires differential-privacy randomness.
    #[must_use]
    pub const fn requires_cycle_prf(&self) -> bool {
        self.privacy.per_subject_metric_cap.is_some()
    }

    /// Durable composition-budget policy bound to the same governed digest.
    #[must_use]
    pub const fn composition_budget(&self) -> PrivacyCompositionBudgetPolicyV1 {
        self.composition_budget
    }
}

trait PrivacyAggregatePolicyConfigExt {
    fn into_policy_config(self) -> Option<PrivacyAggregatePolicyConfig>;
}

impl PrivacyAggregateScheduleConfigExt for actual::SorafsPrivacyAggregateSchedule {
    fn into_schedule_config(self) -> Option<PrivacyAggregateScheduleConfig> {
        if !self.enabled {
            return None;
        }
        Some(PrivacyAggregateScheduleConfig {
            cycle_seconds: self.cycle_seconds.max(1),
            publish_delay_seconds: self.publish_delay_seconds,
        })
    }
}

impl PrivacyAggregatePolicyConfigExt for actual::SorafsPrivacyAggregateSchedule {
    fn into_policy_config(self) -> Option<PrivacyAggregatePolicyConfig> {
        if !self.enabled {
            return None;
        }
        let mode = match self.privacy_mode.as_str() {
            "differential_privacy" => ModerationPrivacyModeV1::DifferentialPrivacy,
            "suppression" => ModerationPrivacyModeV1::Suppression,
            "differential_privacy_with_suppression" => {
                ModerationPrivacyModeV1::DifferentialPrivacyWithSuppression
            }
            invalid => {
                panic!("validated SoraFS privacy aggregate config has unsupported mode `{invalid}`")
            }
        };
        let policy_digest = self.policy_digest.unwrap_or_else(|| {
            panic!("enabled SoraFS privacy aggregate config is missing its policy digest")
        });
        let uses_dp = !matches!(mode, ModerationPrivacyModeV1::Suppression);
        let uses_suppression = !matches!(mode, ModerationPrivacyModeV1::DifferentialPrivacy);
        let privacy = ModerationPrivacyParametersV1 {
            version: MODERATION_PRIVACY_PARAMETERS_VERSION_V1,
            mode,
            epsilon_numerator: uses_dp.then_some(self.epsilon_numerator),
            epsilon_denominator: uses_dp.then_some(self.epsilon_denominator),
            delta_ppb: uses_dp.then_some(0),
            per_subject_metric_cap: uses_dp.then_some(self.per_subject_metric_cap),
            suppression_threshold: uses_suppression.then_some(self.suppression_threshold),
            suppressed_count: 0,
        };
        let composition_budget = PrivacyCompositionBudgetPolicyV1 {
            budget_id: policy_digest,
            epsilon_limit_numerator: self.composition_budget_epsilon_numerator,
            epsilon_limit_denominator: self.composition_budget_epsilon_denominator,
            max_publications: self.composition_budget_max_publications,
        };
        Some(
            PrivacyAggregatePolicyConfig::new(
                self.aggregate_id_prefix,
                privacy,
                policy_digest,
                composition_budget,
            )
            .unwrap_or_else(|error| {
                panic!("validated SoraFS privacy aggregate policy is invalid: {error}")
            }),
        )
    }
}

impl PrivacyAggregateScheduleConfigExt for actual::SorafsEvidenceViewerAuditSchedule {
    fn into_schedule_config(self) -> Option<PrivacyAggregateScheduleConfig> {
        if !self.enabled {
            return None;
        }
        Some(PrivacyAggregateScheduleConfig {
            cycle_seconds: self.cycle_seconds.max(1),
            publish_delay_seconds: self.publish_delay_seconds,
        })
    }
}

/// Governance policy controlling repair escalation decisions.
#[derive(Debug, Clone)]
pub struct RepairEscalationPolicy {
    quorum_bps: u16,
    minimum_voters: u32,
    dispute_window_secs: u64,
    appeal_window_secs: u64,
    max_penalty: XorQuantity,
}

impl RepairEscalationPolicy {
    /// Construct a policy from the governance configuration.
    pub fn from_policy(policy: &actual::RepairEscalationPolicyV1) -> Self {
        Self {
            quorum_bps: policy.quorum_bps.min(10_000),
            minimum_voters: policy.minimum_voters.max(1),
            dispute_window_secs: policy.dispute_window_secs,
            appeal_window_secs: policy.appeal_window_secs,
            max_penalty: policy.max_penalty.clone(),
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

    /// Maximum slash penalty allowed for repair escalation proposals.
    #[must_use]
    pub fn max_penalty(&self) -> &XorQuantity {
        &self.max_penalty
    }

    /// Clamp a proposed penalty to the configured maximum.
    #[must_use]
    pub fn cap_penalty(&self, penalty: &XorQuantity) -> XorQuantity {
        penalty.min(&self.max_penalty)
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
    default_slash_penalty: XorQuantity,
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

    /// Default penalty used for scheduler-generated slash proposals.
    #[must_use]
    pub fn default_slash_penalty(&self) -> &XorQuantity {
        &self.default_slash_penalty
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
            default_slash_penalty: repair.default_slash_penalty.clone(),
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
    fn pdp_config_protocol_ceiling_matches_manifest_v1() {
        const {
            assert!(
                iroha_config::parameters::defaults::sorafs::storage::PDP_SAMPLE_WINDOW_MAX as usize
                    == sorafs_manifest::PDP_MAX_SEGMENT_SAMPLES_V1
            );
            assert!(iroha_config::parameters::defaults::sorafs::storage::PDP_SAMPLE_WINDOW > 0);
            assert!(
                iroha_config::parameters::defaults::sorafs::storage::PDP_SAMPLE_WINDOW
                    <= iroha_config::parameters::defaults::sorafs::storage::PDP_SAMPLE_WINDOW_MAX
            );
            assert!(
                iroha_config::parameters::defaults::sorafs::storage::pdp_provider::CHALLENGE_MAX_BYTES
                    .0 as usize
                    == sorafs_manifest::PDP_CHALLENGE_MAX_CANONICAL_BYTES_V1
            );
            assert!(
                iroha_config::parameters::defaults::sorafs::storage::pdp_provider::PROOF_MAX_BYTES.0
                    as usize
                    == sorafs_manifest::PDP_PROOF_MAX_CANONICAL_BYTES_V1
            );
        }
    }

    #[test]
    fn conversion_from_actual_preserves_fields() {
        let mut actual = actual::SorafsStorage::default();
        actual.enabled = true;
        actual.data_dir = PathBuf::from("/tmp/sorafs");
        actual.max_capacity_bytes = iroha_config::base::util::Bytes(1_024);
        actual.max_parallel_fetches = 99;
        actual.max_pins = 1_001;
        actual.por_sample_interval_secs = 42;
        actual.pdp_sample_window = 37;
        actual.pdp_tree_memory_limit_bytes = iroha_config::base::util::Bytes(8_388_608);
        actual.pdp_provider = actual::SorafsPdpProviderPolicy {
            max_pending_records: 31,
            max_terminal_records: 47,
            checkpoint_max_bytes: iroha_config::base::util::Bytes(33_554_432),
            challenge_max_bytes: iroha_config::base::util::Bytes(262_144),
            proof_max_bytes: iroha_config::base::util::Bytes(8_388_608),
            min_response_window_secs: 120,
            max_response_window_secs: 480,
            max_future_skew_secs: 3,
            terminal_retention_secs: 7_200,
        };
        actual.runtime = actual::SorafsRuntimeRetention {
            event_history_limit: 17,
            state_entry_limit: 23,
            checkpoint_max_bytes: iroha_config::base::util::Bytes(4_096),
            proof_outcome_forwarder_interval: Duration::from_millis(250),
            proof_outcome_max_attempts: 5,
        };
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
        actual.orderbook = actual::SorafsOrderbook {
            min_order_gib: 8,
            price_tick: "0.025".parse().expect("exact orderbook price tick"),
        };
        actual.reputation_trust_policy_path =
            Some(PathBuf::from("/tmp/sorafs-reputation-policy.to"));
        actual.pricing_trust_policy_path = Some(PathBuf::from("/tmp/sorafs-pricing-policy.to"));
        actual.hedging_feed_trust_policy_path =
            Some(PathBuf::from("/tmp/sorafs-hedging-policy.to"));
        actual.privacy_aggregates = actual::SorafsPrivacyAggregateSchedule {
            enabled: true,
            cycle_seconds: 12,
            publish_delay_seconds: 3,
            policy_digest: Some([0xC0; 32]),
            ..actual::SorafsPrivacyAggregateSchedule::default()
        };
        actual.evidence_viewer_audits = actual::SorafsEvidenceViewerAuditSchedule {
            enabled: true,
            cycle_seconds: 24,
            publish_delay_seconds: 6,
        };
        actual.reserve_lifecycle = actual::SorafsReserveLifecycleSchedule {
            enabled: true,
            interval_seconds: 30,
            initial_delay_seconds: 7,
        };

        let cfg = StorageConfig::from(&actual);
        assert!(cfg.enabled());
        assert_eq!(cfg.data_dir(), &PathBuf::from("/tmp/sorafs"));
        assert_eq!(cfg.max_capacity_bytes().0, 1_024);
        assert_eq!(cfg.max_parallel_fetches(), 99);
        assert_eq!(cfg.max_pins(), 1_001);
        assert_eq!(cfg.por_sample_interval_secs(), 42);
        assert_eq!(cfg.pdp_sample_window(), 37);
        assert_eq!(cfg.pdp_tree_memory_limit_bytes().0, 8_388_608);
        assert_eq!(
            cfg.pdp_provider_policy(),
            PdpProviderProtocolPolicyV1 {
                version: PDP_PROVIDER_POLICY_VERSION_V1,
                max_pending_records: 31,
                max_terminal_records: 47,
                checkpoint_max_bytes: 33_554_432,
                challenge_max_bytes: 262_144,
                proof_max_bytes: 8_388_608,
                min_response_window_secs: 120,
                max_response_window_secs: 480,
                max_future_skew_secs: 3,
                terminal_retention_secs: 7_200,
            }
        );
        assert_eq!(cfg.runtime_retention().event_history_limit(), 17);
        assert_eq!(cfg.runtime_retention().state_entry_limit(), 23);
        assert_eq!(cfg.runtime_retention().checkpoint_max_bytes(), 4_096);
        assert_eq!(
            cfg.runtime_retention().proof_outcome_forwarder_interval(),
            Duration::from_millis(250)
        );
        assert_eq!(cfg.runtime_retention().proof_outcome_max_attempts(), 5);
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
        let orderbook = cfg.orderbook_admission_policy();
        assert_eq!(orderbook.min_order_gib(), 8);
        assert_eq!(orderbook.price_tick().to_string(), "0.025");
        assert_eq!(
            cfg.reputation_trust_policy_path(),
            Some(&PathBuf::from("/tmp/sorafs-reputation-policy.to"))
        );
        assert_eq!(
            cfg.pricing_trust_policy_path(),
            Some(&PathBuf::from("/tmp/sorafs-pricing-policy.to"))
        );
        assert_eq!(
            cfg.hedging_feed_trust_policy_path(),
            Some(&PathBuf::from("/tmp/sorafs-hedging-policy.to"))
        );
        assert_eq!(
            cfg.privacy_aggregate_schedule(),
            Some(PrivacyAggregateScheduleConfig {
                cycle_seconds: 12,
                publish_delay_seconds: 3,
            })
        );
        let privacy_policy = cfg
            .privacy_aggregate_policy()
            .expect("enabled privacy aggregate policy");
        let cycle_policy = privacy_policy.cycle_config();
        assert_eq!(cycle_policy.aggregate_id_prefix, "sfm4c-cycle");
        assert_eq!(cycle_policy.policy_digest, Some([0xC0; 32]));
        assert_eq!(privacy_policy.policy_digest(), [0xC0; 32]);
        assert!(privacy_policy.requires_cycle_prf());
        assert_eq!(cycle_policy.privacy.epsilon_numerator, Some(4));
        assert_eq!(cycle_policy.privacy.epsilon_denominator, Some(5));
        assert_eq!(cycle_policy.privacy.delta_ppb, Some(0));
        assert_eq!(cycle_policy.privacy.per_subject_metric_cap, Some(1));
        assert_eq!(
            privacy_policy.composition_budget(),
            PrivacyCompositionBudgetPolicyV1 {
                budget_id: [0xC0; 32],
                epsilon_limit_numerator: 12,
                epsilon_limit_denominator: 1,
                max_publications: 52,
            }
        );
        assert_eq!(
            cfg.evidence_viewer_audit_schedule(),
            Some(PrivacyAggregateScheduleConfig {
                cycle_seconds: 24,
                publish_delay_seconds: 6,
            })
        );
        assert_eq!(
            cfg.reserve_lifecycle_schedule(),
            Some(ReserveLifecycleScheduleConfig::new(30, 7))
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
    fn runtime_retention_clamps_zero_safety_ceilings() {
        let policy = RuntimeRetentionPolicy::new(0, 0, 0);
        assert_eq!(policy, RuntimeRetentionPolicy::new(1, 1, 1));
        assert_eq!(
            policy.proof_outcome_forwarder_interval(),
            Duration::from_secs(1)
        );
        assert_eq!(policy.proof_outcome_max_attempts(), 8);
    }

    #[test]
    fn privacy_aggregate_schedule_is_none_when_disabled() {
        let mut actual = actual::SorafsStorage::default();
        actual.privacy_aggregates = actual::SorafsPrivacyAggregateSchedule {
            enabled: false,
            cycle_seconds: 0,
            publish_delay_seconds: 5,
            ..actual::SorafsPrivacyAggregateSchedule::default()
        };

        let cfg = StorageConfig::from(&actual);
        assert_eq!(cfg.privacy_aggregate_schedule(), None);
        assert_eq!(cfg.privacy_aggregate_policy(), None);
    }

    #[test]
    fn evidence_viewer_audit_schedule_is_none_when_disabled() {
        let mut actual = actual::SorafsStorage::default();
        actual.evidence_viewer_audits = actual::SorafsEvidenceViewerAuditSchedule {
            enabled: false,
            cycle_seconds: 0,
            publish_delay_seconds: 5,
        };

        let cfg = StorageConfig::from(&actual);
        assert_eq!(cfg.evidence_viewer_audit_schedule(), None);
    }

    #[test]
    fn reserve_lifecycle_schedule_is_none_when_disabled_and_clamps_zero_interval() {
        let mut actual = actual::SorafsStorage::default();
        actual.reserve_lifecycle = actual::SorafsReserveLifecycleSchedule {
            enabled: false,
            interval_seconds: 0,
            initial_delay_seconds: 5,
        };

        let cfg = StorageConfig::from(&actual);
        assert_eq!(cfg.reserve_lifecycle_schedule(), None);

        actual.reserve_lifecycle.enabled = true;
        let cfg = StorageConfig::from(&actual);
        assert_eq!(
            cfg.reserve_lifecycle_schedule(),
            Some(ReserveLifecycleScheduleConfig::new(1, 5))
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
            default_slash_penalty: "0.000005".parse().expect("valid exact quantity"),
            auditor_rate_per_sec: std::num::NonZeroU32::new(5),
            auditor_burst: std::num::NonZeroU32::new(10),
        };

        let policy = actual::RepairEscalationPolicyV1 {
            quorum_bps: 7_000,
            minimum_voters: 4,
            dispute_window_secs: 12_000,
            appeal_window_secs: 24_000,
            max_penalty: "0.000009".parse().expect("valid exact quantity"),
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
        assert_eq!(
            cfg.default_slash_penalty(),
            &"0.000005"
                .parse::<XorQuantity>()
                .expect("valid exact quantity")
        );
        assert_eq!(cfg.escalation_policy().quorum_bps(), 7_000);
        assert_eq!(cfg.escalation_policy().minimum_voters(), 4);
        assert_eq!(cfg.escalation_policy().dispute_window_secs(), 12_000);
        assert_eq!(cfg.escalation_policy().appeal_window_secs(), 24_000);
        assert_eq!(
            cfg.escalation_policy().max_penalty(),
            &"0.000009"
                .parse::<XorQuantity>()
                .expect("valid exact quantity")
        );

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
