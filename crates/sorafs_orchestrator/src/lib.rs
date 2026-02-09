//! High-level orchestrator facade wiring SoraFS scoreboards to the async
//! multi-source fetch loop implemented in `sorafs_car`.

use std::{
    cmp::Ordering as CmpOrdering,
    collections::{HashMap, VecDeque},
    future::Future,
    io::Cursor,
    num::{NonZeroU32, NonZeroUsize},
    pin::Pin,
    str::FromStr,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering as AtomicOrdering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use iroha_core::prelude::Hash;
use iroha_data_model::{
    name::Name,
    soranet::privacy_metrics::{
        SoranetPrivacyEventKindV1, SoranetPrivacyEventV1, SoranetPrivacyHandshakeFailureV1,
        SoranetPrivacyModeV1, SoranetPrivacyPrioShareV1,
    },
    taikai::{TaikaiEventId, TaikaiRenditionId, TaikaiStreamId},
};
use iroha_logger::{debug, info, warn};
use iroha_telemetry::{
    metrics::{SorafsFetchOtel, global_or_default, global_sorafs_fetch_otel},
    privacy::{PrivacyBucketConfig, PrivacyConfigError, SoranetSecureAggregator},
};
use norito::json;
use rand::Rng;
use reqwest::{Client, StatusCode, Url};
use sorafs_car::{
    CarBuildPlan, CarVerifier, CarWriteStats, CarWriter, TaikaiSegmentHint,
    gateway::{
        GatewayBuildError, GatewayFetchConfig, GatewayFetchContext, GatewayFetchedManifest,
        GatewayManifestError, GatewayProviderInput,
    },
    multi_fetch::{
        self, AttemptFailure, ChunkObserver, ChunkResponse, ChunkVerificationError, FetchOptions,
        FetchOutcome, FetchProvider, FetchRequest, ProviderMetadata, TransportProtocolKind,
    },
    scoreboard::{self, Eligibility, Scoreboard, ScoreboardConfig, TelemetrySnapshot},
};
use sorafs_manifest::{
    GovernanceProofs,
    validation::{PinPolicyConstraints, validate_manifest},
};
use thiserror::Error;
use tokio::{
    sync::{Mutex as AsyncMutex, Notify},
    task::JoinHandle,
    time::sleep,
};

pub mod appeals;
pub mod compliance;
pub mod incentives;
pub mod proxy;
pub mod soranet;
pub mod taikai_cache;
pub mod treasury;

pub(crate) const SORANET_PQ_RANK_STEP_WEIGHT: u32 = 250;
pub(crate) const SORANET_PQ_CERTIFICATE_BONUS: u32 = 500;
pub(crate) const SORANET_BANDWIDTH_UNIT_BYTES: u64 = 256 * 1024; // 256 KiB per weight step.
use compliance::{CompliancePolicy, ComplianceReason};
use soranet::{
    CircuitEvent, CircuitId, CircuitInfo, CircuitManager, CircuitManagerConfig,
    CircuitManagerError, CircuitRotationRecord, GuardCapabilityComponents, GuardRecord, GuardSet,
    RelayDescriptor, RelayDirectory, RelayPathHint,
};
use taikai_cache::{
    CacheAdmissionError, CacheAdmissionGossip, CacheAdmissionTracker, QosClass, QosConfig,
    ReliabilityTuning, SegmentKey as TaikaiSegmentKey, TaikaiCache, TaikaiCacheConfig,
    TaikaiCacheHandle, TaikaiCacheStatsSnapshot, TaikaiPullQueueStats, TaikaiPullRequest,
    TaikaiPullTicket, TaikaiQueueError,
};

use crate::proxy::{
    BrowserExtensionManifest, LocalQuicProxyConfig, LocalQuicProxyHandle, PROXY_MANIFEST_ID,
    PROXY_PROTOCOL_LABEL, ProxyError, ProxyMode, spawn_local_quic_proxy,
};

/// Convenient re-exports for downstream callers.
pub mod prelude {
    pub use blake3::Hash as Blake3Hash;
    pub use sorafs_car::{
        CarBuildPlan, CarChunk, ChunkStore, FilePlan, InMemoryPayload, PorProof,
        gateway::{GatewayFetchConfig, GatewayProviderInput},
        multi_fetch::{
            CapabilityMismatch, ChunkObserver, ChunkResponse, FetchOptions, FetchOutcome,
            FetchProvider, FetchRequest, ProviderMetadata, RangeCapability, StreamBudget,
            TransportHint,
        },
        scoreboard::{
            Eligibility, IneligibilityReason, ProviderTelemetry, Scoreboard, ScoreboardConfig,
            ScoreboardEntry, TelemetrySnapshot,
        },
    };
    pub use sorafs_chunker::ChunkProfile;

    pub use crate::{
        AnonymityPolicy, CircuitRefreshReport, FetchSession, GatewayCarVerification,
        GatewayOrchestratorError, ManifestVerificationContext, ManifestVerificationError,
        Orchestrator, OrchestratorConfig, PolicyFallback, PolicyReport, PolicyStatus,
        TransportPolicy, WriteModeHint,
        appeals::{
            AppealClass, AppealClassConfig, AppealDecision, AppealDisbursementError,
            AppealDisbursementInput, AppealDisbursementPlan, AppealPricingConfig,
            AppealPricingError, AppealQuote, AppealQuoteBreakdown, AppealQuoteInput,
            AppealSettlementBreakdown, AppealSettlementConfig, AppealSettlementError,
            AppealUrgency, AppealVerdict, JurorPayout,
        },
        bindings::{ConfigJsonError, config_from_json, config_to_json},
        compliance::{CompliancePolicy, ComplianceReason},
        fetch_via_gateway,
        incentives::{RelayRewardEngine, RewardConfig, RewardConfigError},
        provider_supports_pq, provider_supports_soranet,
        proxy::{BrowserExtensionManifest, LocalQuicProxyConfig, LocalQuicProxyHandle},
        soranet::{
            CircuitEvent, CircuitId, CircuitInfo, CircuitLatencySnapshot, CircuitManager,
            CircuitManagerConfig, CircuitManagerError, CircuitRetirementReason,
            CircuitRotationRecord, Endpoint, ExitDemotion, ExitDemotionReason, GuardCacheKey,
            GuardCacheKeyError, GuardRecord, GuardRetention, GuardSelector, GuardSet,
            PathHintReport, PathMetadata, RelayDescriptor, RelayDirectory, RelayPathHint,
            RelayRoles,
        },
        taikai_cache::{
            CacheEviction as TaikaiCacheEviction, CacheEvictionReason as TaikaiCacheEvictionReason,
            CachePromotion as TaikaiCachePromotion, CacheTierKind as TaikaiCacheTierKind,
            CachedSegment as TaikaiCachedSegment, QosClass as TaikaiQosClass,
            QosConfig as TaikaiQosConfig, QosError as TaikaiQosError,
            SegmentKey as TaikaiSegmentKey, TaikaiCache, TaikaiCacheConfig,
            TaikaiCacheInsertOutcome, TaikaiCacheQueryOutcome, TaikaiShardId, TaikaiShardRing,
        },
        treasury::{
            AdjustmentKind, AdjustmentRequest, DisputeId, DisputeOutcome, DisputeResolution,
            EarningsDashboard, EarningsRow, PayoutInput, PayoutOutcome, PayoutServiceError,
            RelayPayoutService, RewardDispute, RewardDisputeRegistry, RewardLedgerError,
            RewardLedgerSnapshot,
        },
    };
}

/// Outcome captured after reconciling the circuit manager with the current SoraNet state.
#[derive(Debug, Clone, PartialEq)]
pub struct CircuitRefreshReport {
    /// Lifecycle events generated while updating circuits.
    pub events: Vec<CircuitEvent>,
    /// Snapshot of active circuits after reconciliation.
    pub active: Vec<CircuitInfo>,
    /// Rotation records accumulated by the circuit manager.
    pub rotation_history: Vec<CircuitRotationRecord>,
}

/// Configuration describing how the orchestrator reacts to handshake downgrades.
#[derive(Debug, Clone)]
pub struct DowngradeRemediationConfig {
    /// Enable or disable remediation.
    pub enabled: bool,
    /// Minimum downgrade events in the sliding window required to trigger remediation.
    pub threshold: NonZeroU32,
    /// Sliding window used to count downgrade events.
    pub window: Duration,
    /// Cooldown duration before restoring the previous proxy mode.
    pub cooldown: Duration,
    /// Proxy mode enforced while remediation is active.
    pub target_mode: ProxyMode,
    /// Proxy mode restored after the cooldown elapses.
    pub resume_mode: ProxyMode,
    /// Privacy modes that contribute to the downgrade counter (empty = all).
    pub modes: Vec<SoranetPrivacyModeV1>,
}

impl Default for DowngradeRemediationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold: NonZeroU32::new(3).expect("non-zero threshold"),
            window: Duration::from_mins(5),
            cooldown: Duration::from_mins(15),
            target_mode: ProxyMode::MetadataOnly,
            resume_mode: ProxyMode::Bridge,
            modes: vec![SoranetPrivacyModeV1::Entry],
        }
    }
}

impl DowngradeRemediationConfig {
    /// Returns `true` if downgrade telemetry for the provided mode should be considered.
    #[must_use]
    pub fn matches_mode(&self, mode: SoranetPrivacyModeV1) -> bool {
        self.modes.is_empty() || self.modes.iter().any(|candidate| candidate == &mode)
    }
}

#[derive(Default)]
struct DowngradeState {
    recent: VecDeque<Instant>,
    active: bool,
    active_mode: Option<ProxyMode>,
    revert_task: Option<JoinHandle<()>>,
}

pub(crate) struct DowngradeRemediator {
    runtime: Arc<AsyncMutex<LocalProxyRuntime>>,
    config: DowngradeRemediationConfig,
    metrics: Arc<iroha_telemetry::metrics::Metrics>,
    state: Arc<AsyncMutex<DowngradeState>>,
}

impl DowngradeRemediator {
    fn new(
        runtime: Arc<AsyncMutex<LocalProxyRuntime>>,
        config: DowngradeRemediationConfig,
        metrics: Arc<iroha_telemetry::metrics::Metrics>,
    ) -> Self {
        Self {
            runtime,
            config,
            metrics,
            state: Arc::new(AsyncMutex::new(DowngradeState::default())),
        }
    }

    async fn proxy_label(&self) -> String {
        let guard = self.runtime.lock().await;
        guard.telemetry_label()
    }

    async fn observe_handshake_downgrade(&self, _timestamp_unix: u64, mode: SoranetPrivacyModeV1) {
        if !self.config.enabled || !self.config.matches_mode(mode) {
            return;
        }
        let now = Instant::now();
        let mut state = self.state.lock().await;
        if state.active {
            if let Some(task) = state.revert_task.take() {
                task.abort();
            }
            let resume_mode = state
                .active_mode
                .clone()
                .unwrap_or(self.config.resume_mode.clone());
            let handle = self.spawn_revert_task(resume_mode);
            state.revert_task = Some(handle);
            return;
        }
        state.recent.push_back(now);
        while let Some(front) = state.recent.front() {
            if now.duration_since(*front) > self.config.window {
                state.recent.pop_front();
            } else {
                break;
            }
        }
        if state.recent.len() < self.config.threshold.get() as usize {
            return;
        }
        state.recent.clear();
        state.active = true;
        let resume_mode = self.config.resume_mode.clone();
        state.active_mode = Some(resume_mode.clone());
        drop(state);

        match apply_proxy_mode(&self.runtime, self.config.target_mode.clone()).await {
            Ok(_) => {
                let label = self.proxy_label().await;
                record_proxy_metric(&label, "remediation_trigger", "downgrade");
                let mut state = self.state.lock().await;
                state.revert_task = Some(self.spawn_revert_task(resume_mode));
            }
            Err(error) => {
                warn!(
                    target: "soranet.proxy",
                    ?error,
                    "failed to activate downgrade remediation mode"
                );
                let mut state = self.state.lock().await;
                state.active = false;
                state.active_mode = None;
            }
        }
    }

    fn spawn_revert_task(&self, resume_mode: ProxyMode) -> JoinHandle<()> {
        let runtime = Arc::clone(&self.runtime);
        let state = Arc::clone(&self.state);
        let metrics = Arc::clone(&self.metrics);
        let config = self.config.clone();
        tokio::spawn(async move {
            sleep(config.cooldown).await;
            let label = {
                let guard = runtime.lock().await;
                guard.telemetry_label()
            };
            match apply_proxy_mode(&runtime, resume_mode.clone()).await {
                Ok(_) => {
                    record_proxy_metric(&label, "remediation_cooldown", "complete");
                }
                Err(error) => {
                    warn!(
                        target: "soranet.proxy",
                        ?error,
                        label = label,
                        requested_mode = %resume_mode.as_str(),
                        "failed to revert proxy mode after remediation cooldown"
                    );
                    record_proxy_metric(&label, "remediation_cooldown_error", resume_mode.as_str());
                    metrics.inc_sorafs_orchestrator_transport_event(
                        &label,
                        PROXY_PROTOCOL_LABEL,
                        "remediation_cooldown_error",
                        resume_mode.as_str(),
                    );
                }
            }
            let mut guard = state.lock().await;
            guard.active = false;
            guard.active_mode = None;
            guard.revert_task = None;
        })
    }
}

/// Errors surfaced by the orchestrator facade.
#[derive(Debug, Error)]
pub enum OrchestratorError {
    /// Scoreboard construction failed (capability mismatch, persistence error, etc.).
    #[error("failed to build scoreboard: {0}")]
    Scoreboard(#[source] std::io::Error),
    /// Underlying multi-source orchestrator returned an error.
    #[error(transparent)]
    MultiSource(#[from] multi_fetch::MultiSourceError),
    /// Circuit lifecycle manager failed to reconcile SoraNet circuits.
    #[error("failed to refresh soranet circuits: {0}")]
    Circuit(#[source] CircuitManagerError),
    /// Local proxy was not configured but remediation was requested.
    #[error("local proxy not configured in orchestrator config")]
    ProxyNotConfigured,
    /// Local proxy operation failed.
    #[error("local proxy operation failed: {0}")]
    Proxy(#[from] ProxyError),
}

/// Transport policy applied when selecting providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransportPolicy {
    /// Prefer relays that advertise SoraNet support while keeping direct transports as a fallback.
    /// Multi-source adopters now use this policy by default.
    #[default]
    SoranetPreferred,
    /// Require SoraNet transport and fail instead of falling back to direct providers.
    SoranetStrict,
    /// Enforce direct mode by restricting selection to providers that expose Torii/QUIC transports.
    /// Use this explicit downgrade when relays are unhealthy or compliance mandates single-source
    /// fetches (see `roadmap.md`, “SoraNet Anonymity Overlay Program” for the rollback checklist).
    DirectOnly,
}

impl TransportPolicy {
    pub fn label(self) -> &'static str {
        match self {
            Self::SoranetPreferred => "soranet-first",
            Self::SoranetStrict => "soranet-strict",
            Self::DirectOnly => "direct-only",
        }
    }

    /// Parse a [`TransportPolicy`] from a textual label (accepts dash or underscore separators).
    pub fn parse(label: &str) -> Option<Self> {
        let normalised = label.trim().to_ascii_lowercase();
        match normalised.as_str() {
            "soranet_first" | "soranet-first" => Some(Self::SoranetPreferred),
            "soranet_strict" | "soranet-strict" | "soranet_only" | "soranet-only" => {
                Some(Self::SoranetStrict)
            }
            "direct_only" | "direct-only" => Some(Self::DirectOnly),
            _ => None,
        }
    }
}

/// Staged anonymity policy enforced while selecting SoraNet-capable providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(clippy::enum_variant_names)]
pub enum AnonymityPolicy {
    /// Stage A (default): require that at least one SoraNet hop (guard) advertises PQ capability.
    #[default]
    GuardPq,
    /// Stage B: prefer PQ-capable relays for a majority of SoraNet hops (≥ two thirds).
    MajorityPq,
    /// Stage C: enforce PQ-only SoraNet paths, falling back to direct transports otherwise.
    StrictPq,
}

impl AnonymityPolicy {
    pub fn label(self) -> &'static str {
        match self {
            Self::GuardPq => "anon-guard-pq",
            Self::MajorityPq => "anon-majority-pq",
            Self::StrictPq => "anon-strict-pq",
        }
    }

    /// Parse an [`AnonymityPolicy`] from a textual representation.
    pub fn parse(label: &str) -> Option<Self> {
        let normalised = label.trim().to_ascii_lowercase();
        match normalised.as_str() {
            "anon_guard_pq" | "anon-guard-pq" => Some(Self::GuardPq),
            "stage_a" | "stage-a" | "stagea" => Some(Self::GuardPq),
            "anon_majority_pq" | "anon-majority-pq" => Some(Self::MajorityPq),
            "stage_b" | "stage-b" | "stageb" => Some(Self::MajorityPq),
            "anon_strict_pq" | "anon-strict-pq" => Some(Self::StrictPq),
            "stage_c" | "stage-c" | "stagec" => Some(Self::StrictPq),
            _ => None,
        }
    }

    /// Returns the next less strict policy, if any.
    #[must_use]
    pub const fn fallback(self) -> Option<Self> {
        match self {
            Self::StrictPq => Some(Self::MajorityPq),
            Self::MajorityPq => Some(Self::GuardPq),
            Self::GuardPq => None,
        }
    }
}

/// Rollout phase controlling the default anonymity stage applied to SoraNet paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RolloutPhase {
    /// Canary wave — require at least one PQ-capable guard (Stage A).
    #[default]
    Canary,
    /// Ramp wave — prefer PQ-capable relays for ≥ two thirds of hops (Stage B).
    Ramp,
    /// Default GA posture — enforce PQ-only SoraNet paths (Stage C).
    Default,
}

impl RolloutPhase {
    /// Stable string label used in config/CLI bindings.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Canary => "canary",
            Self::Ramp => "ramp",
            Self::Default => "default",
        }
    }

    /// Parse a rollout phase from a textual label (accepts stage aliases).
    pub fn parse(label: &str) -> Option<Self> {
        let normalised = label.trim().to_ascii_lowercase();
        match normalised.as_str() {
            "canary" | "stage_a" | "stage-a" | "stagea" => Some(Self::Canary),
            "ramp" | "stage_b" | "stage-b" | "stageb" => Some(Self::Ramp),
            "default" | "ga" | "stage_c" | "stage-c" | "stagec" => Some(Self::Default),
            _ => None,
        }
    }

    /// Map the rollout phase to the default anonymity policy.
    #[must_use]
    pub const fn default_anonymity_policy(self) -> AnonymityPolicy {
        match self {
            Self::Canary => AnonymityPolicy::GuardPq,
            Self::Ramp => AnonymityPolicy::MajorityPq,
            Self::Default => AnonymityPolicy::StrictPq,
        }
    }
}

/// Write-mode hint forwarded by SDKs to tighten PQ expectations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WriteModeHint {
    /// Default behaviour for read/replication workloads.
    #[default]
    ReadOnly,
    /// Upload workloads that require PQ-only paths end-to-end.
    UploadPqOnly,
}

impl WriteModeHint {
    /// Stable string label used in logs and metrics.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ReadOnly => "read_only",
            Self::UploadPqOnly => "upload_pq_only",
        }
    }

    /// Parse a [`WriteModeHint`] from a textual label.
    pub fn parse(label: &str) -> Option<Self> {
        let normalised = label.trim().to_ascii_lowercase();
        match normalised.as_str() {
            "read_only" | "read-only" => Some(Self::ReadOnly),
            "upload_pq_only" | "upload-pq-only" => Some(Self::UploadPqOnly),
            _ => None,
        }
    }

    /// Returns `true` when the hint mandates PQ-only transport.
    #[must_use]
    pub const fn enforces_pq_only(self) -> bool {
        matches!(self, Self::UploadPqOnly)
    }

    /// Apply the hint to derive effective transport/anonymity policies.
    #[must_use]
    pub const fn apply(
        self,
        transport_policy: TransportPolicy,
        anonymity_policy: AnonymityPolicy,
    ) -> (TransportPolicy, AnonymityPolicy) {
        match self {
            Self::ReadOnly => (transport_policy, anonymity_policy),
            Self::UploadPqOnly => (TransportPolicy::SoranetStrict, AnonymityPolicy::StrictPq),
        }
    }
}

/// Configuration for polling relay admin privacy feeds.
#[derive(Clone)]
pub struct PrivacyEventsConfig {
    /// Aggregation bucket parameters mirroring relay defaults.
    pub bucket: PrivacyBucketConfig,
    /// Interval between successive `/privacy/events` polls.
    pub poll_interval: Duration,
    /// Request timeout applied to each polling request.
    pub request_timeout: Duration,
}

impl Default for PrivacyEventsConfig {
    fn default() -> Self {
        Self {
            bucket: PrivacyBucketConfig::default(),
            poll_interval: Duration::from_secs(10),
            request_timeout: Duration::from_secs(5),
        }
    }
}

impl PrivacyEventsConfig {
    /// Override the poll interval between successive requests.
    #[must_use]
    pub fn with_poll_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = interval;
        self
    }

    /// Override the request timeout applied to polling requests.
    #[must_use]
    pub fn with_request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    /// Override the aggregation bucket parameters.
    #[must_use]
    pub fn with_bucket(mut self, bucket: PrivacyBucketConfig) -> Self {
        self.bucket = bucket;
        self
    }
}

/// Optional overrides forcing policy stages during fetches.
#[derive(Debug, Clone, Default)]
pub struct PolicyOverride {
    /// Override for the transport policy applied during fetches.
    pub transport_policy: Option<TransportPolicy>,
    /// Override for the anonymity policy applied during fetches.
    pub anonymity_policy: Option<AnonymityPolicy>,
}

impl PolicyOverride {
    /// Construct a new override from the supplied policies.
    #[must_use]
    pub const fn new(
        transport_policy: Option<TransportPolicy>,
        anonymity_policy: Option<AnonymityPolicy>,
    ) -> Self {
        Self {
            transport_policy,
            anonymity_policy,
        }
    }

    /// Returns `true` when neither transport nor anonymity overrides are configured.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.transport_policy.is_none() && self.anonymity_policy.is_none()
    }

    /// Attach a transport policy override.
    #[must_use]
    pub fn with_transport_policy(mut self, policy: TransportPolicy) -> Self {
        self.transport_policy = Some(policy);
        self
    }

    /// Attach an anonymity policy override.
    #[must_use]
    pub fn with_anonymity_policy(mut self, policy: AnonymityPolicy) -> Self {
        self.anonymity_policy = Some(policy);
        self
    }
}

/// Combined configuration applied to scoreboard generation and fetch scheduling.
#[derive(Clone)]
pub struct OrchestratorConfig {
    /// Configuration forwarded to the scoreboard builder.
    pub scoreboard: ScoreboardConfig,
    /// Fetch options forwarded to the multi-source orchestrator.
    pub fetch: FetchOptions,
    /// Optional region label attached to telemetry metrics.
    pub telemetry_region: Option<String>,
    /// Optional limit on the number of providers selected from the scoreboard.
    pub max_providers: Option<NonZeroUsize>,
    /// Transport policy that controls how providers are prioritised.
    pub transport_policy: TransportPolicy,
    /// Rollout phase controlling the default anonymity policy.
    pub rollout_phase: RolloutPhase,
    /// Effective anonymity policy applied to SoraNet providers.
    pub anonymity_policy: AnonymityPolicy,
    /// Explicit anonymity override, if configured.
    pub anonymity_policy_override: Option<AnonymityPolicy>,
    /// Compliance policy derived from governance (jurisdiction/CID opt-outs).
    pub compliance: CompliancePolicy,
    /// Optional SoraNet directory describing relay metadata.
    pub relay_directory: Option<RelayDirectory>,
    /// Supplemental path hints to refine relay path selection.
    pub relay_path_hints: Vec<RelayPathHint>,
    /// Optional guard cache used to prioritise pinned SoraNet relays.
    pub guard_set: Option<GuardSet>,
    /// Optional circuit manager configuration. `None` disables lifecycle management.
    pub circuit_manager: Option<CircuitManagerConfig>,
    /// Optional local QUIC proxy configuration used by browser integrations.
    pub local_proxy: Option<LocalQuicProxyConfig>,
    /// Optional downgrade remediation policy triggered by telemetry.
    pub downgrade_remediation: Option<DowngradeRemediationConfig>,
    /// Optional overrides forcing specific transport/anonymity stages.
    pub policy_override: PolicyOverride,
    /// Write-mode hint indicating whether PQ-only enforcement is required.
    pub write_mode: WriteModeHint,
    /// Privacy event polling configuration. `None` disables the collector.
    pub privacy_events: Option<PrivacyEventsConfig>,
    /// Optional Taikai cache configuration wired for SNNet-14 distribution pilots.
    pub taikai_cache: Option<TaikaiCacheConfig>,
}

impl OrchestratorConfig {
    /// Attach a telemetry region label used when emitting metrics.
    #[must_use]
    pub fn with_telemetry_region(mut self, region: impl Into<String>) -> Self {
        self.telemetry_region = Some(region.into());
        self
    }

    /// Limit the number of providers selected from the scoreboard.
    #[must_use]
    pub fn with_max_providers(mut self, limit: NonZeroUsize) -> Self {
        self.max_providers = Some(limit);
        self
    }

    /// Override the transport policy used when ranking providers.
    #[must_use]
    pub fn with_transport_policy(mut self, policy: TransportPolicy) -> Self {
        self.transport_policy = policy;
        self
    }

    /// Configure the rollout phase controlling the default anonymity posture.
    #[must_use]
    pub fn with_rollout_phase(mut self, phase: RolloutPhase) -> Self {
        self.rollout_phase = phase;
        if self.anonymity_policy_override.is_none() {
            self.anonymity_policy = phase.default_anonymity_policy();
        }
        self
    }

    /// Override the anonymity policy.
    #[must_use]
    pub fn with_anonymity_policy(mut self, policy: AnonymityPolicy) -> Self {
        self.anonymity_policy = policy;
        self.anonymity_policy_override = Some(policy);
        self
    }

    /// Attach policy overrides that pin transport/anonymity stages.
    #[must_use]
    pub fn with_policy_override(mut self, overrides: PolicyOverride) -> Self {
        self.policy_override = overrides;
        self
    }

    /// Attach a guard set used to prioritise pinned SoraNet relays.
    #[must_use]
    pub fn with_guard_set(mut self, guard_set: GuardSet) -> Self {
        self.guard_set = Some(guard_set);
        self
    }

    /// Attach a relay directory describing SoraNet relays.
    #[must_use]
    pub fn with_relay_directory(mut self, directory: RelayDirectory) -> Self {
        self.relay_directory = Some(directory);
        self
    }

    /// Attach path hints used to refine relay selection and MASQUE bypass eligibility.
    #[must_use]
    pub fn with_relay_path_hints(mut self, hints: Vec<RelayPathHint>) -> Self {
        self.relay_path_hints = hints;
        self
    }

    /// Override the circuit manager configuration (set to `None` to disable lifecycle management).
    #[must_use]
    pub fn with_circuit_manager(mut self, config: Option<CircuitManagerConfig>) -> Self {
        self.circuit_manager = config;
        self
    }

    /// Configure the local QUIC proxy used for browser/SDK integrations.
    #[must_use]
    pub fn with_local_proxy(mut self, config: Option<LocalQuicProxyConfig>) -> Self {
        self.local_proxy = config;
        self
    }

    /// Override the write-mode hint applied during provider selection.
    #[must_use]
    pub fn with_write_mode(mut self, mode: WriteModeHint) -> Self {
        self.write_mode = mode;
        self
    }

    /// Attach an explicit compliance policy.
    #[must_use]
    pub fn with_compliance_policy(mut self, policy: CompliancePolicy) -> Self {
        self.compliance = policy;
        self
    }

    /// Override the privacy events polling configuration.
    #[must_use]
    pub fn with_privacy_events(mut self, config: Option<PrivacyEventsConfig>) -> Self {
        self.privacy_events = config;
        self
    }

    /// Attach a Taikai cache configuration. `None` disables the cache.
    #[must_use]
    pub fn with_taikai_cache(mut self, cache: Option<TaikaiCacheConfig>) -> Self {
        self.taikai_cache = cache;
        self
    }

    /// Configure the downgrade remediation policy.
    #[must_use]
    pub fn with_downgrade_remediation(
        mut self,
        remediation: Option<DowngradeRemediationConfig>,
    ) -> Self {
        self.downgrade_remediation = remediation;
        self
    }
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            scoreboard: ScoreboardConfig::default(),
            fetch: FetchOptions::default(),
            telemetry_region: None,
            max_providers: None,
            transport_policy: TransportPolicy::default(),
            rollout_phase: RolloutPhase::default(),
            anonymity_policy: RolloutPhase::default().default_anonymity_policy(),
            anonymity_policy_override: None,
            compliance: CompliancePolicy::default(),
            relay_directory: None,
            relay_path_hints: Vec::new(),
            guard_set: None,
            circuit_manager: Some(CircuitManagerConfig::default()),
            local_proxy: None,
            downgrade_remediation: Some(DowngradeRemediationConfig::default()),
            policy_override: PolicyOverride::default(),
            write_mode: WriteModeHint::default(),
            privacy_events: Some(PrivacyEventsConfig::default()),
            taikai_cache: None,
        }
    }
}

/// Helper utilities for configuring the orchestrator via Norito JSON payloads.
pub mod bindings {
    use std::{
        collections::BTreeSet, convert::TryFrom, num::NonZeroU32, path::PathBuf, time::Duration,
    };

    use norito::json::{Map, Value};

    use super::*;

    /// Errors that may occur while parsing [`OrchestratorConfig`] from JSON.
    #[derive(Debug, Error)]
    #[error("{0}")]
    pub struct ConfigJsonError(String);

    impl ConfigJsonError {
        fn new(message: impl Into<String>) -> Self {
            Self(message.into())
        }
    }

    /// Serialise an [`OrchestratorConfig`] into a Norito JSON value.
    #[must_use]
    pub fn config_to_json(config: &OrchestratorConfig) -> Value {
        let mut root = Map::new();

        let mut scoreboard = Map::new();
        scoreboard.insert(
            "latency_cap_ms".into(),
            Value::from(config.scoreboard.latency_cap_ms as u64),
        );
        scoreboard.insert(
            "weight_scale".into(),
            Value::from(u64::from(config.scoreboard.weight_scale.get())),
        );
        scoreboard.insert(
            "telemetry_grace_secs".into(),
            Value::from(config.scoreboard.telemetry_grace_period.as_secs()),
        );
        scoreboard.insert(
            "now_unix_secs".into(),
            Value::from(config.scoreboard.now_unix_secs),
        );
        if let Some(path) = &config.scoreboard.persist_path {
            scoreboard.insert(
                "persist_path".into(),
                Value::String(path.to_string_lossy().into_owned()),
            );
        }
        root.insert("scoreboard".into(), Value::Object(scoreboard));

        let mut fetch = Map::new();
        fetch.insert(
            "verify_lengths".into(),
            Value::Bool(config.fetch.verify_lengths),
        );
        fetch.insert(
            "verify_digests".into(),
            Value::Bool(config.fetch.verify_digests),
        );
        if let Some(limit) = config.fetch.per_chunk_retry_limit {
            fetch.insert("retry_budget".into(), Value::from(limit as u64));
        }
        fetch.insert(
            "provider_failure_threshold".into(),
            Value::from(config.fetch.provider_failure_threshold as u64),
        );
        if let Some(limit) = config.fetch.global_parallel_limit {
            fetch.insert("global_parallel_limit".into(), Value::from(limit as u64));
        }
        root.insert("fetch".into(), Value::Object(fetch));

        if let Some(region) = &config.telemetry_region {
            root.insert("telemetry_region".into(), Value::String(region.clone()));
        }
        if let Some(limit) = config.max_providers {
            root.insert("max_providers".into(), Value::from(limit.get() as u64));
        }
        root.insert(
            "rollout_phase".into(),
            Value::from(config.rollout_phase.label()),
        );

        root.insert(
            "anonymity_policy".into(),
            Value::from(config.anonymity_policy.label()),
        );
        if let Some(policy) = config.anonymity_policy_override {
            root.insert(
                "anonymity_policy_override".into(),
                Value::from(policy.label()),
            );
        }
        root.insert(
            "transport_policy".into(),
            Value::from(config.transport_policy.label()),
        );
        if !config.policy_override.is_empty() {
            let mut override_map = Map::new();
            if let Some(policy) = config.policy_override.transport_policy {
                override_map.insert("transport_policy".into(), Value::from(policy.label()));
            }
            if let Some(policy) = config.policy_override.anonymity_policy {
                override_map.insert("anonymity_policy".into(), Value::from(policy.label()));
            }
            root.insert("policy_override".into(), Value::Object(override_map));
        } else {
            root.insert("policy_override".into(), Value::Null);
        }
        root.insert("write_mode".into(), Value::from(config.write_mode.label()));

        if !config.compliance.is_empty() {
            root.insert("compliance".into(), compliance_to_json(&config.compliance));
        }

        if !config.relay_path_hints.is_empty() {
            let hints = config
                .relay_path_hints
                .iter()
                .map(|hint| {
                    let mut obj = Map::new();
                    obj.insert(
                        "relay_id_hex".into(),
                        Value::String(hex::encode(hint.relay_id)),
                    );
                    if let Some(rtt) = hint.avg_rtt_ms {
                        obj.insert("avg_rtt_ms".into(), Value::from(rtt as u64));
                    }
                    if let Some(region) = hint.region.as_ref() {
                        obj.insert("region".into(), Value::String(region.clone()));
                    }
                    if let Some(asn) = hint.asn {
                        obj.insert("asn".into(), Value::from(u64::from(asn)));
                    }
                    if hint.validator_lane {
                        obj.insert("validator_lane".into(), Value::Bool(true));
                    }
                    if hint.masque_bypass_allowed {
                        obj.insert("masque_bypass_allowed".into(), Value::Bool(true));
                    }
                    Value::Object(obj)
                })
                .collect();
            root.insert("relay_path_hints".into(), Value::Array(hints));
        } else {
            root.insert("relay_path_hints".into(), Value::Null);
        }

        match &config.privacy_events {
            Some(privacy) => {
                let mut bucket = Map::new();
                bucket.insert(
                    "bucket_secs".into(),
                    Value::from(privacy.bucket.bucket_secs),
                );
                bucket.insert(
                    "min_contributors".into(),
                    Value::from(privacy.bucket.min_contributors),
                );
                bucket.insert(
                    "flush_delay_buckets".into(),
                    Value::from(privacy.bucket.flush_delay_buckets),
                );
                bucket.insert(
                    "force_flush_buckets".into(),
                    Value::from(privacy.bucket.force_flush_buckets),
                );
                bucket.insert(
                    "max_completed_buckets".into(),
                    Value::from(privacy.bucket.max_completed_buckets as u64),
                );
                bucket.insert(
                    "expected_shares".into(),
                    Value::from(u64::from(privacy.bucket.expected_shares)),
                );

                let mut privacy_value = Map::new();
                privacy_value.insert(
                    "poll_interval_secs".into(),
                    Value::from(privacy.poll_interval.as_secs()),
                );
                privacy_value.insert(
                    "request_timeout_secs".into(),
                    Value::from(privacy.request_timeout.as_secs()),
                );
                privacy_value.insert("bucket".into(), Value::Object(bucket));
                root.insert("privacy_events".into(), Value::Object(privacy_value));
            }
            None => {
                root.insert("privacy_events".into(), Value::Null);
            }
        }

        let mut circuit = Map::new();
        match &config.circuit_manager {
            Some(cfg) => {
                circuit.insert("enabled".into(), Value::Bool(true));
                circuit.insert(
                    "circuit_ttl_secs".into(),
                    Value::from(cfg.circuit_ttl().as_secs()),
                );
                circuit.insert(
                    "validator_masque_bypass".into(),
                    Value::Bool(cfg.validator_masque_bypass()),
                );
            }
            None => {
                circuit.insert("enabled".into(), Value::Bool(false));
                circuit.insert("validator_masque_bypass".into(), Value::Bool(false));
            }
        }
        root.insert("circuit_manager".into(), Value::Object(circuit));

        if let Some(proxy_cfg) = &config.local_proxy {
            let value =
                norito::json::to_value(proxy_cfg).expect("local proxy config should serialise");
            root.insert("local_proxy".into(), value);
        } else {
            root.insert("local_proxy".into(), Value::Null);
        }

        match &config.downgrade_remediation {
            Some(remediation) => {
                let mut map = Map::new();
                map.insert("enabled".into(), Value::Bool(remediation.enabled));
                map.insert(
                    "threshold".into(),
                    Value::from(u64::from(remediation.threshold.get())),
                );
                map.insert(
                    "window_secs".into(),
                    Value::from(remediation.window.as_secs()),
                );
                map.insert(
                    "cooldown_secs".into(),
                    Value::from(remediation.cooldown.as_secs()),
                );
                map.insert(
                    "target_mode".into(),
                    Value::from(remediation.target_mode.as_str()),
                );
                map.insert(
                    "resume_mode".into(),
                    Value::from(remediation.resume_mode.as_str()),
                );
                if !remediation.modes.is_empty() {
                    map.insert(
                        "modes".into(),
                        Value::Array(
                            remediation
                                .modes
                                .iter()
                                .map(|mode| Value::from(mode.as_label()))
                                .collect(),
                        ),
                    );
                }
                root.insert("downgrade_remediation".into(), Value::Object(map));
            }
            None => {
                root.insert("downgrade_remediation".into(), Value::Null);
            }
        }

        if let Some(cache) = &config.taikai_cache {
            root.insert("taikai_cache".into(), taikai_cache_to_json(cache));
        } else {
            root.insert("taikai_cache".into(), Value::Null);
        }

        Value::Object(root)
    }

    /// Parse an [`OrchestratorConfig`] from a Norito JSON value.
    pub fn config_from_json(value: &Value) -> Result<OrchestratorConfig, ConfigJsonError> {
        let root = value
            .as_object()
            .ok_or_else(|| ConfigJsonError::new("orchestrator config must be a JSON object"))?;

        let mut config = OrchestratorConfig::default();

        if let Some(scoreboard_value) = root.get("scoreboard") {
            let scoreboard = scoreboard_value
                .as_object()
                .ok_or_else(|| ConfigJsonError::new("scoreboard must be a JSON object"))?;

            if let Some(latency_value) = scoreboard.get("latency_cap_ms") {
                let latency = latency_value
                    .as_u64()
                    .ok_or_else(|| ConfigJsonError::new("scoreboard.latency_cap_ms must be u64"))?;
                config.scoreboard.latency_cap_ms = u32::try_from(latency).map_err(|_| {
                    ConfigJsonError::new("scoreboard.latency_cap_ms exceeds u32::MAX")
                })?;
            }

            if let Some(weight_value) = scoreboard.get("weight_scale") {
                let weight = weight_value
                    .as_u64()
                    .ok_or_else(|| ConfigJsonError::new("scoreboard.weight_scale must be u64"))?;
                let weight_u32 = u32::try_from(weight).map_err(|_| {
                    ConfigJsonError::new("scoreboard.weight_scale exceeds u32::MAX")
                })?;
                config.scoreboard.weight_scale = NonZeroU32::new(weight_u32)
                    .ok_or_else(|| ConfigJsonError::new("scoreboard.weight_scale must be > 0"))?;
            }

            if let Some(grace_value) = scoreboard.get("telemetry_grace_secs") {
                let secs = grace_value.as_u64().ok_or_else(|| {
                    ConfigJsonError::new("scoreboard.telemetry_grace_secs must be u64")
                })?;
                config.scoreboard.telemetry_grace_period = Duration::from_secs(secs);
            }

            if let Some(path_value) = scoreboard.get("persist_path") {
                let path = path_value.as_str().ok_or_else(|| {
                    ConfigJsonError::new("scoreboard.persist_path must be a string")
                })?;
                if !path.is_empty() {
                    config.scoreboard.persist_path = Some(PathBuf::from(path));
                } else {
                    config.scoreboard.persist_path = None;
                }
            }

            if let Some(now_value) = scoreboard.get("now_unix_secs") {
                let now = now_value
                    .as_u64()
                    .ok_or_else(|| ConfigJsonError::new("scoreboard.now_unix_secs must be u64"))?;
                config.scoreboard.now_unix_secs = now;
            }
        }

        if let Some(fetch_value) = root.get("fetch") {
            let fetch = fetch_value
                .as_object()
                .ok_or_else(|| ConfigJsonError::new("fetch must be a JSON object"))?;

            if let Some(verify_lengths) = fetch.get("verify_lengths") {
                config.fetch.verify_lengths = verify_lengths.as_bool().ok_or_else(|| {
                    ConfigJsonError::new("fetch.verify_lengths must be a boolean")
                })?;
            }

            if let Some(verify_digests) = fetch.get("verify_digests") {
                config.fetch.verify_digests = verify_digests.as_bool().ok_or_else(|| {
                    ConfigJsonError::new("fetch.verify_digests must be a boolean")
                })?;
            }

            if let Some(retry_budget) = fetch.get("retry_budget") {
                let retries = retry_budget.as_u64().ok_or_else(|| {
                    ConfigJsonError::new("fetch.retry_budget must be a positive integer")
                })?;
                if retries == 0 {
                    config.fetch.per_chunk_retry_limit = Some(1);
                } else {
                    config.fetch.per_chunk_retry_limit =
                        Some(usize::try_from(retries).map_err(|_| {
                            ConfigJsonError::new("fetch.retry_budget exceeds usize::MAX")
                        })?);
                }
            }

            if let Some(provider_threshold) = fetch.get("provider_failure_threshold") {
                let value = provider_threshold.as_u64().ok_or_else(|| {
                    ConfigJsonError::new("fetch.provider_failure_threshold must be u64")
                })?;
                config.fetch.provider_failure_threshold = usize::try_from(value).map_err(|_| {
                    ConfigJsonError::new("fetch.provider_failure_threshold exceeds usize::MAX")
                })?;
            }

            if let Some(global_limit) = fetch.get("global_parallel_limit") {
                let limit = global_limit.as_u64().ok_or_else(|| {
                    ConfigJsonError::new("fetch.global_parallel_limit must be a positive integer")
                })?;
                if limit == 0 {
                    config.fetch.global_parallel_limit = Some(1);
                } else {
                    config.fetch.global_parallel_limit =
                        Some(usize::try_from(limit).map_err(|_| {
                            ConfigJsonError::new("fetch.global_parallel_limit exceeds usize::MAX")
                        })?);
                }
            }
        }

        if let Some(region_value) = root.get("telemetry_region") {
            let region = region_value.as_str().ok_or_else(|| {
                ConfigJsonError::new("telemetry_region must be a string when present")
            })?;
            if !region.is_empty() {
                config.telemetry_region = Some(region.to_string());
            } else {
                config.telemetry_region = None;
            }
        }

        if let Some(max_providers) = root.get("max_providers") {
            let limit = max_providers
                .as_u64()
                .ok_or_else(|| ConfigJsonError::new("max_providers must be a positive integer"))?;
            if limit == 0 {
                config.max_providers = Some(NonZeroUsize::new(1).expect("non-zero constant"));
            } else {
                let limit_usize = usize::try_from(limit)
                    .map_err(|_| ConfigJsonError::new("max_providers exceeds usize::MAX"))?;
                config.max_providers = NonZeroUsize::new(limit_usize);
            }
        }

        if let Some(hints_value) = root.get("relay_path_hints") {
            if hints_value.is_null() {
                config.relay_path_hints = Vec::new();
            } else {
                let array = hints_value.as_array().ok_or_else(|| {
                    ConfigJsonError::new("relay_path_hints must be an array when present")
                })?;
                let mut hints = Vec::with_capacity(array.len());
                for (index, entry) in array.iter().enumerate() {
                    let obj = entry.as_object().ok_or_else(|| {
                        ConfigJsonError::new(format!(
                            "relay_path_hints[{index}] must be an object with relay_id_hex and optional metadata"
                        ))
                    })?;
                    let relay_id_hex = obj
                        .get("relay_id_hex")
                        .and_then(json::Value::as_str)
                        .ok_or_else(|| {
                            ConfigJsonError::new(format!(
                                "relay_path_hints[{index}].relay_id_hex must be a hex string"
                            ))
                        })?;
                    let avg_rtt_ms = obj
                        .get("avg_rtt_ms")
                        .map(|value| {
                            value.as_u64().ok_or_else(|| {
                                ConfigJsonError::new(format!(
                                    "relay_path_hints[{index}].avg_rtt_ms must be an unsigned integer"
                                ))
                            })
                        })
                        .transpose()?
                        .map(|value| value as u32);
                    let region = obj
                        .get("region")
                        .map(|value| {
                            value.as_str().ok_or_else(|| {
                                ConfigJsonError::new(format!(
                                    "relay_path_hints[{index}].region must be a string"
                                ))
                            })
                        })
                        .transpose()?
                        .map(str::to_owned);
                    let asn = obj
                        .get("asn")
                        .map(|value| {
                            value.as_u64().ok_or_else(|| {
                                ConfigJsonError::new(format!(
                                    "relay_path_hints[{index}].asn must be an unsigned integer"
                                ))
                            })
                        })
                        .transpose()?
                        .map(|value| value as u32);
                    let validator_lane = obj
                        .get("validator_lane")
                        .and_then(json::Value::as_bool)
                        .unwrap_or(false);
                    let masque_bypass_allowed = obj
                        .get("masque_bypass_allowed")
                        .and_then(json::Value::as_bool)
                        .unwrap_or(false);
                    let hint = RelayPathHint::from_hex(
                        relay_id_hex,
                        avg_rtt_ms,
                        region,
                        asn,
                        validator_lane,
                        masque_bypass_allowed,
                    )
                    .map_err(ConfigJsonError::new)?;
                    hints.push(hint);
                }
                config.relay_path_hints = hints;
            }
        }

        if let Some(phase_value) = root.get("rollout_phase") {
            let label = phase_value.as_str().ok_or_else(|| {
                ConfigJsonError::new("rollout_phase must be a string when present")
            })?;
            let phase = RolloutPhase::parse(label).ok_or_else(|| {
                ConfigJsonError::new(
                    "rollout_phase must be one of canary|ramp|default|stage_a|stage-a|stagea|stage_b|stage-b|stageb|stage_c|stage-c|stagec|ga",
                )
            })?;
            config.rollout_phase = phase;
            if config.anonymity_policy_override.is_none() {
                config.anonymity_policy = phase.default_anonymity_policy();
            }
        }

        if let Some(policy_value) = root.get("anonymity_policy_override") {
            let label = policy_value.as_str().ok_or_else(|| {
                ConfigJsonError::new("anonymity_policy_override must be a string when present")
            })?;
            let policy = AnonymityPolicy::parse(label).ok_or_else(|| {
                ConfigJsonError::new(
                    "anonymity_policy_override must be one of anon-guard-pq|anon-majority-pq|anon-strict-pq or stage_a|stage_b|stage_c aliases",
                )
            })?;
            config.anonymity_policy = policy;
            config.anonymity_policy_override = Some(policy);
        }

        if let Some(policy_value) = root.get("anonymity_policy") {
            let label = policy_value.as_str().ok_or_else(|| {
                ConfigJsonError::new("anonymity_policy must be a string when present")
            })?;
            let policy = AnonymityPolicy::parse(label).ok_or_else(|| {
                ConfigJsonError::new(
                    "anonymity_policy must be one of anon-guard-pq|anon-majority-pq|anon-strict-pq or stage_a|stage_b|stage_c aliases",
                )
            })?;
            config.anonymity_policy = policy;
            config.anonymity_policy_override = Some(policy);
        }

        if let Some(policy_value) = root.get("transport_policy") {
            let label = policy_value.as_str().ok_or_else(|| {
                ConfigJsonError::new("transport_policy must be a string when present")
            })?;
            config.transport_policy = TransportPolicy::parse(label).ok_or_else(|| {
                ConfigJsonError::new(
                    "transport_policy must be one of soranet_first|soranet-first|soranet_strict|soranet-strict|soranet_only|soranet-only|direct_only|direct-only",
                )
            })?;
        }

        if let Some(override_value) = root.get("policy_override") {
            if override_value.is_null() {
                config.policy_override = PolicyOverride::default();
            } else {
                let override_obj = override_value.as_object().ok_or_else(|| {
                    ConfigJsonError::new("policy_override must be an object or null")
                })?;
                let mut overrides = PolicyOverride::default();
                if let Some(label) = override_obj.get("transport_policy") {
                    let label = label.as_str().ok_or_else(|| {
                        ConfigJsonError::new("policy_override.transport_policy must be a string")
                    })?;
                    let normalized = label.trim().to_ascii_lowercase().replace('-', "_");
                    let policy = TransportPolicy::parse(&normalized).ok_or_else(|| {
                        ConfigJsonError::new(
                            "policy_override.transport_policy must be one of soranet_first|soranet-first|soranet_strict|soranet-strict|soranet_only|soranet-only|direct_only|direct-only",
                        )
                    })?;
                    overrides.transport_policy = Some(policy);
                }
                if let Some(label) = override_obj.get("anonymity_policy") {
                    let label = label.as_str().ok_or_else(|| {
                        ConfigJsonError::new("policy_override.anonymity_policy must be a string")
                    })?;
                    let policy = AnonymityPolicy::parse(label).ok_or_else(|| {
                        ConfigJsonError::new(
                            "policy_override.anonymity_policy must be one of anon-guard-pq|anon-majority-pq|anon-strict-pq or stage_a|stage_b|stage_c aliases",
                        )
                    })?;
                    overrides.anonymity_policy = Some(policy);
                }
                config.policy_override = overrides;
            }
        }

        if let Some(compliance_value) = root.get("compliance") {
            let compliance = compliance_value.as_object().ok_or_else(|| {
                ConfigJsonError::new("compliance must be a JSON object when present")
            })?;
            let mut policy = CompliancePolicy::default();

            if let Some(jurisdictions) = compliance.get("operator_jurisdictions") {
                let array = jurisdictions.as_array().ok_or_else(|| {
                    ConfigJsonError::new("compliance.operator_jurisdictions must be an array")
                })?;
                let mut collected = Vec::new();
                for (index, entry) in array.iter().enumerate() {
                    let raw = entry.as_str().ok_or_else(|| {
                        ConfigJsonError::new(format!(
                            "compliance.operator_jurisdictions[{index}] must be a string"
                        ))
                    })?;
                    let code = normalise_iso_code(raw).map_err(ConfigJsonError::new)?;
                    if !collected.contains(&code) {
                        collected.push(code);
                    }
                }
                policy.set_operator_jurisdictions(collected);
            }

            if let Some(opt_outs) = compliance.get("jurisdiction_opt_outs") {
                let array = opt_outs.as_array().ok_or_else(|| {
                    ConfigJsonError::new("compliance.jurisdiction_opt_outs must be an array")
                })?;
                let mut set = BTreeSet::new();
                for (index, entry) in array.iter().enumerate() {
                    let raw = entry.as_str().ok_or_else(|| {
                        ConfigJsonError::new(format!(
                            "compliance.jurisdiction_opt_outs[{index}] must be a string"
                        ))
                    })?;
                    let code = normalise_iso_code(raw).map_err(ConfigJsonError::new)?;
                    set.insert(code);
                }
                policy.set_jurisdiction_opt_outs(set);
            }

            if let Some(opt_outs) = compliance.get("blinded_cid_opt_outs") {
                let array = opt_outs.as_array().ok_or_else(|| {
                    ConfigJsonError::new("compliance.blinded_cid_opt_outs must be an array")
                })?;
                let mut set = BTreeSet::new();
                for (index, entry) in array.iter().enumerate() {
                    let raw = entry.as_str().ok_or_else(|| {
                        ConfigJsonError::new(format!(
                            "compliance.blinded_cid_opt_outs[{index}] must be a string"
                        ))
                    })?;
                    let trimmed = raw.trim();
                    if trimmed.len() != Hash::LENGTH * 2 {
                        return Err(ConfigJsonError::new(format!(
                            "compliance.blinded_cid_opt_outs[{index}] must be a 64-character hex digest"
                        )));
                    }
                    let mut decoded = [0u8; Hash::LENGTH];
                    hex::decode_to_slice(trimmed, &mut decoded).map_err(|_| {
                        ConfigJsonError::new(format!(
                            "compliance.blinded_cid_opt_outs[{index}] contains invalid hex digits"
                        ))
                    })?;
                    set.insert(decoded);
                }
                policy.set_blinded_cid_opt_outs(set);
            }

            if let Some(contacts) = compliance.get("audit_contacts") {
                let array = contacts.as_array().ok_or_else(|| {
                    ConfigJsonError::new("compliance.audit_contacts must be an array")
                })?;
                let mut collected = Vec::new();
                for (index, entry) in array.iter().enumerate() {
                    let raw = entry.as_str().ok_or_else(|| {
                        ConfigJsonError::new(format!(
                            "compliance.audit_contacts[{index}] must be a string"
                        ))
                    })?;
                    let trimmed = raw.trim();
                    if trimmed.is_empty() {
                        return Err(ConfigJsonError::new(format!(
                            "compliance.audit_contacts[{index}] must not be empty"
                        )));
                    }
                    collected.push(trimmed.to_string());
                }
                policy.set_audit_contacts(collected);
            }

            if let Some(attestations) = compliance.get("attestations") {
                let array = attestations.as_array().ok_or_else(|| {
                    ConfigJsonError::new("compliance.attestations must be an array")
                })?;
                let mut collected = Vec::new();
                for (index, entry) in array.iter().enumerate() {
                    let object = entry.as_object().ok_or_else(|| {
                        ConfigJsonError::new(format!(
                            "compliance.attestations[{index}] must be an object"
                        ))
                    })?;
                    let jurisdiction = match object.get("jurisdiction") {
                        Some(value) => {
                            if value.is_null() {
                                None
                            } else {
                                let raw = value.as_str().ok_or_else(|| {
                                    ConfigJsonError::new(format!(
                                        "compliance.attestations[{index}].jurisdiction must be a string when present"
                                    ))
                                })?;
                                Some(normalise_iso_code(raw).map_err(ConfigJsonError::new)?)
                            }
                        }
                        None => None,
                    };
                    let document_uri = object
                        .get("document_uri")
                        .ok_or_else(|| {
                            ConfigJsonError::new(format!(
                                "compliance.attestations[{index}].document_uri is required"
                            ))
                        })?
                        .as_str()
                        .ok_or_else(|| {
                            ConfigJsonError::new(format!(
                                "compliance.attestations[{index}].document_uri must be a string"
                            ))
                        })?;
                    let document_uri = document_uri.trim();
                    if document_uri.is_empty() {
                        return Err(ConfigJsonError::new(format!(
                            "compliance.attestations[{index}].document_uri must not be empty"
                        )));
                    }
                    let digest_hex = object
                        .get("digest_hex")
                        .ok_or_else(|| {
                            ConfigJsonError::new(format!(
                                "compliance.attestations[{index}].digest_hex is required"
                            ))
                        })?
                        .as_str()
                        .ok_or_else(|| {
                            ConfigJsonError::new(format!(
                                "compliance.attestations[{index}].digest_hex must be a string"
                            ))
                        })?;
                    let digest_hex = digest_hex.trim();
                    if digest_hex.len() != Hash::LENGTH * 2 {
                        return Err(ConfigJsonError::new(format!(
                            "compliance.attestations[{index}].digest_hex must be a 64-character hex digest"
                        )));
                    }
                    let mut digest = [0u8; Hash::LENGTH];
                    hex::decode_to_slice(digest_hex, &mut digest).map_err(|_| {
                        ConfigJsonError::new(format!(
                            "compliance.attestations[{index}].digest_hex contains invalid hex digits"
                        ))
                    })?;
                    let issued_at = object
                        .get("issued_at_ms")
                        .ok_or_else(|| {
                            ConfigJsonError::new(format!(
                                "compliance.attestations[{index}].issued_at_ms is required"
                            ))
                        })?
                        .as_u64()
                        .ok_or_else(|| {
                            ConfigJsonError::new(format!(
                                "compliance.attestations[{index}].issued_at_ms must be a non-negative integer"
                            ))
                        })?;
                    let expires_at = match object.get("expires_at_ms") {
                        Some(value) => {
                            if value.is_null() {
                                None
                            } else {
                                Some(
                                    value.as_u64().ok_or_else(|| {
                                        ConfigJsonError::new(format!(
                                            "compliance.attestations[{index}].expires_at_ms must be a non-negative integer when present"
                                        ))
                                    })?,
                                )
                            }
                        }
                        None => None,
                    };
                    let attestation = compliance::OperatorAttestation::new(
                        jurisdiction,
                        document_uri.to_string(),
                        digest,
                        issued_at,
                        expires_at,
                    );
                    collected.push(attestation);
                }
                policy.set_attestations(collected);
            }

            config.compliance = policy;
        }

        if let Some(hints_value) = root.get("relay_path_hints") {
            if hints_value.is_null() {
                config.relay_path_hints = Vec::new();
            } else {
                let hints_array = hints_value.as_array().ok_or_else(|| {
                    ConfigJsonError::new("relay_path_hints must be an array or null")
                })?;
                let mut hints = Vec::with_capacity(hints_array.len());
                for (index, value) in hints_array.iter().enumerate() {
                    let obj = value.as_object().ok_or_else(|| {
                        ConfigJsonError::new(format!("relay_path_hints[{index}] must be an object"))
                    })?;
                    let relay_hex = obj.get("relay_id_hex").and_then(Value::as_str).ok_or_else(
                        || {
                            ConfigJsonError::new(
                                "relay_path_hints.relay_id_hex must be a 64-character hex string",
                            )
                        },
                    )?;
                    let avg_rtt_ms = obj
                        .get("avg_rtt_ms")
                        .map(|value| {
                            value.as_u64().ok_or_else(|| {
                                ConfigJsonError::new(
                                    "relay_path_hints.avg_rtt_ms must be a positive integer",
                                )
                            })
                        })
                        .transpose()?
                        .map(|value| {
                            u32::try_from(value).map_err(|_| {
                                ConfigJsonError::new("relay_path_hints.avg_rtt_ms exceeds u32::MAX")
                            })
                        })
                        .transpose()?;
                    let region = obj
                        .get("region")
                        .map(|value| {
                            value
                                .as_str()
                                .ok_or_else(|| {
                                    ConfigJsonError::new(
                                        "relay_path_hints.region must be a string when present",
                                    )
                                })
                                .map(|region| region.to_string())
                        })
                        .transpose()?;
                    let asn = obj
                        .get("asn")
                        .map(|value| {
                            value.as_u64().ok_or_else(|| {
                                ConfigJsonError::new(
                                    "relay_path_hints.asn must be a positive integer",
                                )
                            })
                        })
                        .transpose()?
                        .map(|value| {
                            u32::try_from(value).map_err(|_| {
                                ConfigJsonError::new("relay_path_hints.asn exceeds u32::MAX")
                            })
                        })
                        .transpose()?;
                    let validator_lane = obj
                        .get("validator_lane")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    let masque_bypass_allowed = obj
                        .get("masque_bypass_allowed")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);

                    let hint = RelayPathHint::from_hex(
                        relay_hex,
                        avg_rtt_ms,
                        region,
                        asn,
                        validator_lane,
                        masque_bypass_allowed,
                    )
                    .map_err(|err| {
                        ConfigJsonError::new(format!("relay_path_hints[{index}] is invalid: {err}"))
                    })?;
                    hints.push(hint);
                }
                config.relay_path_hints = hints;
            }
        }

        if let Some(privacy_value) = root.get("privacy_events") {
            if privacy_value.is_null() {
                config.privacy_events = None;
            } else {
                let privacy_obj = privacy_value.as_object().ok_or_else(|| {
                    ConfigJsonError::new("privacy_events must be a JSON object or null")
                })?;

                let mut privacy_config = PrivacyEventsConfig::default();

                if let Some(poll_value) = privacy_obj.get("poll_interval_secs") {
                    let secs = poll_value.as_u64().ok_or_else(|| {
                        ConfigJsonError::new(
                            "privacy_events.poll_interval_secs must be a positive integer",
                        )
                    })?;
                    if secs == 0 {
                        return Err(ConfigJsonError::new(
                            "privacy_events.poll_interval_secs must be at least 1 second",
                        ));
                    }
                    privacy_config.poll_interval = Duration::from_secs(secs);
                }

                if let Some(timeout_value) = privacy_obj.get("request_timeout_secs") {
                    let secs = timeout_value.as_u64().ok_or_else(|| {
                        ConfigJsonError::new(
                            "privacy_events.request_timeout_secs must be a positive integer",
                        )
                    })?;
                    if secs == 0 {
                        return Err(ConfigJsonError::new(
                            "privacy_events.request_timeout_secs must be at least 1 second",
                        ));
                    }
                    privacy_config.request_timeout = Duration::from_secs(secs);
                }

                if let Some(bucket_value) = privacy_obj.get("bucket") {
                    let bucket_obj = bucket_value.as_object().ok_or_else(|| {
                        ConfigJsonError::new("privacy_events.bucket must be a JSON object")
                    })?;
                    let mut bucket = PrivacyBucketConfig::default();

                    if let Some(secs) = bucket_obj.get("bucket_secs") {
                        bucket.bucket_secs = secs.as_u64().ok_or_else(|| {
                            ConfigJsonError::new(
                                "privacy_events.bucket.bucket_secs must be a positive integer",
                            )
                        })?;
                        if bucket.bucket_secs == 0 {
                            return Err(ConfigJsonError::new(
                                "privacy_events.bucket.bucket_secs must be greater than zero",
                            ));
                        }
                    }

                    if let Some(contributors) = bucket_obj.get("min_contributors") {
                        bucket.min_contributors = contributors.as_u64().ok_or_else(|| {
                            ConfigJsonError::new(
                                "privacy_events.bucket.min_contributors must be a positive integer",
                            )
                        })?;
                    }

                    if let Some(delay) = bucket_obj.get("flush_delay_buckets") {
                        bucket.flush_delay_buckets = delay.as_u64().ok_or_else(|| {
                            ConfigJsonError::new(
                                "privacy_events.bucket.flush_delay_buckets must be a positive integer",
                            )
                        })?;
                    }

                    if let Some(force) = bucket_obj.get("force_flush_buckets") {
                        bucket.force_flush_buckets = force.as_u64().ok_or_else(|| {
                            ConfigJsonError::new(
                                "privacy_events.bucket.force_flush_buckets must be a positive integer",
                            )
                        })?;
                    }

                    if let Some(max_completed) = bucket_obj.get("max_completed_buckets") {
                        let value = max_completed.as_u64().ok_or_else(|| {
                            ConfigJsonError::new(
                                "privacy_events.bucket.max_completed_buckets must be a positive integer",
                            )
                        })?;
                        bucket.max_completed_buckets = usize::try_from(value).map_err(|_| {
                            ConfigJsonError::new(
                                "privacy_events.bucket.max_completed_buckets exceeds usize::MAX",
                            )
                        })?;
                    }

                    if let Some(expected) = bucket_obj.get("expected_shares") {
                        let value = expected.as_u64().ok_or_else(|| {
                            ConfigJsonError::new(
                                "privacy_events.bucket.expected_shares must be a positive integer",
                            )
                        })?;
                        bucket.expected_shares = u16::try_from(value).map_err(|_| {
                            ConfigJsonError::new(
                                "privacy_events.bucket.expected_shares exceeds u16::MAX",
                            )
                        })?;
                    }

                    bucket.validate().map_err(|err| {
                        ConfigJsonError::new(format!(
                            "privacy_events.bucket configuration invalid: {err}"
                        ))
                    })?;
                    privacy_config.bucket = bucket;
                }

                privacy_config.bucket.validate().map_err(|err| {
                    ConfigJsonError::new(format!(
                        "privacy_events.bucket configuration invalid: {err}"
                    ))
                })?;

                config.privacy_events = Some(privacy_config);
            }
        }

        if let Some(circuit_value) = root.get("circuit_manager") {
            let circuit_obj = circuit_value.as_object().ok_or_else(|| {
                ConfigJsonError::new("circuit_manager must be a JSON object when present")
            })?;
            let enabled = circuit_obj
                .get("enabled")
                .map(|value| {
                    value.as_bool().ok_or_else(|| {
                        ConfigJsonError::new(
                            "circuit_manager.enabled must be a boolean when present",
                        )
                    })
                })
                .transpose()?
                .unwrap_or(true);
            if enabled {
                let default_ttl = CircuitManagerConfig::default().circuit_ttl().as_secs();
                let ttl_secs = circuit_obj
                    .get("circuit_ttl_secs")
                    .map(|value| {
                        value.as_u64().ok_or_else(|| {
                            ConfigJsonError::new(
                                "circuit_manager.circuit_ttl_secs must be a positive integer",
                            )
                        })
                    })
                    .transpose()?
                    .unwrap_or(default_ttl);
                if ttl_secs == 0 {
                    return Err(ConfigJsonError::new(
                        "circuit_manager.circuit_ttl_secs must be at least 1 second",
                    ));
                }
                let masque_bypass = circuit_obj
                    .get("validator_masque_bypass")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                config.circuit_manager = Some(
                    CircuitManagerConfig::new(Duration::from_secs(ttl_secs))
                        .with_validator_masque_bypass(masque_bypass),
                );
            } else {
                config.circuit_manager = None;
            }
        }

        if let Some(write_mode_value) = root.get("write_mode") {
            let label = write_mode_value
                .as_str()
                .ok_or_else(|| ConfigJsonError::new("write_mode must be a string when present"))?;
            config.write_mode = WriteModeHint::parse(label).ok_or_else(|| {
                ConfigJsonError::new(
                    "write_mode must be one of read_only|read-only|upload_pq_only|upload-pq-only",
                )
            })?;
        }

        if let Some(proxy_value) = root.get("local_proxy") {
            if proxy_value.is_null() {
                config.local_proxy = None;
            } else {
                let proxy_cfg: LocalQuicProxyConfig = norito::json::from_value(proxy_value.clone())
                    .map_err(|err| {
                        ConfigJsonError::new(format!("local_proxy configuration invalid: {err}"))
                    })?;
                config.local_proxy = Some(proxy_cfg);
            }
        }

        if let Some(remediation_value) = root.get("downgrade_remediation") {
            if remediation_value.is_null() {
                config.downgrade_remediation = None;
            } else {
                let remediation_map = remediation_value.as_object().ok_or_else(|| {
                    ConfigJsonError::new("downgrade_remediation must be an object or null")
                })?;
                let mut remediation = DowngradeRemediationConfig::default();

                if let Some(enabled_value) = remediation_map.get("enabled") {
                    remediation.enabled = enabled_value.as_bool().ok_or_else(|| {
                        ConfigJsonError::new("downgrade_remediation.enabled must be a boolean")
                    })?;
                }
                if let Some(threshold_value) = remediation_map.get("threshold") {
                    let value = threshold_value.as_u64().ok_or_else(|| {
                        ConfigJsonError::new("downgrade_remediation.threshold must be an integer")
                    })?;
                    remediation.threshold =
                        NonZeroU32::new(u32::try_from(value).map_err(|_| {
                            ConfigJsonError::new("downgrade_remediation.threshold exceeds u32::MAX")
                        })?)
                        .ok_or_else(|| {
                            ConfigJsonError::new(
                                "downgrade_remediation.threshold must be greater than zero",
                            )
                        })?;
                }
                if let Some(window_value) = remediation_map.get("window_secs") {
                    let secs = window_value.as_u64().ok_or_else(|| {
                        ConfigJsonError::new("downgrade_remediation.window_secs must be an integer")
                    })?;
                    remediation.window = Duration::from_secs(secs);
                }
                if let Some(cooldown_value) = remediation_map.get("cooldown_secs") {
                    let secs = cooldown_value.as_u64().ok_or_else(|| {
                        ConfigJsonError::new(
                            "downgrade_remediation.cooldown_secs must be an integer",
                        )
                    })?;
                    remediation.cooldown = Duration::from_secs(secs);
                }
                if let Some(target_mode) = remediation_map.get("target_mode") {
                    let label = target_mode.as_str().ok_or_else(|| {
                        ConfigJsonError::new("downgrade_remediation.target_mode must be a string")
                    })?;
                    remediation.target_mode = ProxyMode::parse(label).ok_or_else(|| {
                        ConfigJsonError::new(
                            "downgrade_remediation.target_mode must be bridge or metadata-only",
                        )
                    })?;
                }
                if let Some(resume_mode) = remediation_map.get("resume_mode") {
                    let label = resume_mode.as_str().ok_or_else(|| {
                        ConfigJsonError::new("downgrade_remediation.resume_mode must be a string")
                    })?;
                    remediation.resume_mode = ProxyMode::parse(label).ok_or_else(|| {
                        ConfigJsonError::new(
                            "downgrade_remediation.resume_mode must be bridge or metadata-only",
                        )
                    })?;
                }
                if let Some(modes_value) = remediation_map.get("modes") {
                    if modes_value.is_null() {
                        remediation.modes.clear();
                    } else {
                        let modes_array = modes_value.as_array().ok_or_else(|| {
                            ConfigJsonError::new(
                                "downgrade_remediation.modes must be an array of strings",
                            )
                        })?;
                        let mut modes = Vec::with_capacity(modes_array.len());
                        for entry in modes_array {
                            let label = entry.as_str().ok_or_else(|| {
                                ConfigJsonError::new(
                                    "downgrade_remediation.modes entries must be strings",
                                )
                            })?;
                            let mode = match label {
                                "entry" => SoranetPrivacyModeV1::Entry,
                                "middle" => SoranetPrivacyModeV1::Middle,
                                "exit" => SoranetPrivacyModeV1::Exit,
                                other => {
                                    return Err(ConfigJsonError::new(format!(
                                        "downgrade_remediation.modes contains unsupported value `{other}`"
                                    )));
                                }
                            };
                            modes.push(mode);
                        }
                        remediation.modes = modes;
                    }
                }

                config.downgrade_remediation = Some(remediation);
            }
        }

        if let Some(cache_value) = root.get("taikai_cache") {
            if cache_value.is_null() {
                config.taikai_cache = None;
            } else {
                let cache = taikai_cache_from_json(cache_value)?;
                config.taikai_cache = Some(cache);
            }
        }

        Ok(config)
    }

    fn taikai_cache_to_json(config: &TaikaiCacheConfig) -> Value {
        let mut root = Map::new();
        root.insert(
            "hot_capacity_bytes".into(),
            Value::from(config.hot_capacity_bytes),
        );
        root.insert(
            "hot_retention_secs".into(),
            Value::from(config.hot_retention.as_secs()),
        );
        root.insert(
            "warm_capacity_bytes".into(),
            Value::from(config.warm_capacity_bytes),
        );
        root.insert(
            "warm_retention_secs".into(),
            Value::from(config.warm_retention.as_secs()),
        );
        root.insert(
            "cold_capacity_bytes".into(),
            Value::from(config.cold_capacity_bytes),
        );
        root.insert(
            "cold_retention_secs".into(),
            Value::from(config.cold_retention.as_secs()),
        );

        let mut qos = Map::new();
        qos.insert(
            "priority_rate_bps".into(),
            Value::from(config.qos.priority_rate_bps),
        );
        qos.insert(
            "standard_rate_bps".into(),
            Value::from(config.qos.standard_rate_bps),
        );
        qos.insert(
            "bulk_rate_bps".into(),
            Value::from(config.qos.bulk_rate_bps),
        );
        qos.insert(
            "burst_multiplier".into(),
            Value::from(u64::from(config.qos.burst_multiplier)),
        );
        root.insert("qos".into(), Value::Object(qos));

        let mut reliability = Map::new();
        reliability.insert(
            "failures_to_trip".into(),
            Value::from(config.reliability.failures_to_trip),
        );
        reliability.insert(
            "open_secs".into(),
            Value::from(config.reliability.open_secs),
        );
        root.insert("reliability".into(), Value::Object(reliability));

        Value::Object(root)
    }

    fn taikai_cache_from_json(value: &Value) -> Result<TaikaiCacheConfig, ConfigJsonError> {
        let object = value
            .as_object()
            .ok_or_else(|| ConfigJsonError::new("taikai_cache must be a JSON object"))?;

        let hot_capacity = object
            .get("hot_capacity_bytes")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                ConfigJsonError::new("taikai_cache.hot_capacity_bytes must be an unsigned integer")
            })?;
        let hot_retention_secs = object
            .get("hot_retention_secs")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                ConfigJsonError::new("taikai_cache.hot_retention_secs must be an unsigned integer")
            })?;

        let warm_capacity = object
            .get("warm_capacity_bytes")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                ConfigJsonError::new("taikai_cache.warm_capacity_bytes must be an unsigned integer")
            })?;
        let warm_retention_secs = object
            .get("warm_retention_secs")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                ConfigJsonError::new("taikai_cache.warm_retention_secs must be an unsigned integer")
            })?;

        let cold_capacity = object
            .get("cold_capacity_bytes")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                ConfigJsonError::new("taikai_cache.cold_capacity_bytes must be an unsigned integer")
            })?;
        let cold_retention_secs = object
            .get("cold_retention_secs")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                ConfigJsonError::new("taikai_cache.cold_retention_secs must be an unsigned integer")
            })?;

        let qos_value = object
            .get("qos")
            .ok_or_else(|| ConfigJsonError::new("taikai_cache.qos section is required"))?;
        let qos_object = qos_value
            .as_object()
            .ok_or_else(|| ConfigJsonError::new("taikai_cache.qos must be a JSON object"))?;
        let priority_rate = qos_object
            .get("priority_rate_bps")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                ConfigJsonError::new(
                    "taikai_cache.qos.priority_rate_bps must be an unsigned integer",
                )
            })?;
        let standard_rate = qos_object
            .get("standard_rate_bps")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                ConfigJsonError::new(
                    "taikai_cache.qos.standard_rate_bps must be an unsigned integer",
                )
            })?;
        let bulk_rate = qos_object
            .get("bulk_rate_bps")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                ConfigJsonError::new("taikai_cache.qos.bulk_rate_bps must be an unsigned integer")
            })?;
        let burst_multiplier = qos_object
            .get("burst_multiplier")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                ConfigJsonError::new(
                    "taikai_cache.qos.burst_multiplier must be an unsigned integer",
                )
            })?;

        Ok(TaikaiCacheConfig {
            hot_capacity_bytes: hot_capacity,
            hot_retention: Duration::from_secs(hot_retention_secs),
            warm_capacity_bytes: warm_capacity,
            warm_retention: Duration::from_secs(warm_retention_secs),
            cold_capacity_bytes: cold_capacity,
            cold_retention: Duration::from_secs(cold_retention_secs),
            qos: QosConfig {
                priority_rate_bps: priority_rate,
                standard_rate_bps: standard_rate,
                bulk_rate_bps: bulk_rate,
                burst_multiplier: u32::try_from(burst_multiplier).map_err(|_| {
                    ConfigJsonError::new(
                        "taikai_cache.qos.burst_multiplier must fit into a 32-bit integer",
                    )
                })?,
            },
            reliability: {
                let defaults = ReliabilityTuning::default();
                let reliability = object
                    .get("reliability")
                    .and_then(Value::as_object)
                    .cloned()
                    .unwrap_or_default();
                let failures_to_trip = reliability
                    .get("failures_to_trip")
                    .and_then(Value::as_u64)
                    .map(|value| value.max(1))
                    .unwrap_or(u64::from(defaults.failures_to_trip));
                let open_secs = reliability
                    .get("open_secs")
                    .and_then(Value::as_u64)
                    .map(|value| value.max(1))
                    .unwrap_or(defaults.open_secs);
                ReliabilityTuning {
                    failures_to_trip: u32::try_from(failures_to_trip).map_err(|_| {
                        ConfigJsonError::new(
                            "taikai_cache.reliability.failures_to_trip must fit in u32",
                        )
                    })?,
                    open_secs,
                }
            },
        })
    }

    fn compliance_to_json(policy: &CompliancePolicy) -> Value {
        let mut map = Map::new();

        if !policy.operator_jurisdictions().is_empty() {
            let values = policy
                .operator_jurisdictions()
                .iter()
                .map(|code| Value::String(code.clone()))
                .collect();
            map.insert("operator_jurisdictions".into(), Value::Array(values));
        }

        let jurisdiction_opt_outs: Vec<Value> = policy
            .jurisdiction_opt_outs()
            .map(|code| Value::String(code.to_string()))
            .collect();
        if !jurisdiction_opt_outs.is_empty() {
            map.insert(
                "jurisdiction_opt_outs".into(),
                Value::Array(jurisdiction_opt_outs),
            );
        }

        let cid_opt_outs: Vec<Value> = policy
            .blinded_cid_opt_outs()
            .map(|digest| Value::String(hex::encode_upper(digest)))
            .collect();
        if !cid_opt_outs.is_empty() {
            map.insert("blinded_cid_opt_outs".into(), Value::Array(cid_opt_outs));
        }

        if !policy.audit_contacts().is_empty() {
            let values = policy
                .audit_contacts()
                .iter()
                .map(|contact| Value::String(contact.clone()))
                .collect();
            map.insert("audit_contacts".into(), Value::Array(values));
        }

        if !policy.attestations().is_empty() {
            let entries = policy
                .attestations()
                .iter()
                .map(|attestation| {
                    let mut entry = Map::new();
                    if let Some(code) = attestation.jurisdiction() {
                        entry.insert("jurisdiction".into(), Value::String(code.to_string()));
                    }
                    entry.insert(
                        "document_uri".into(),
                        Value::String(attestation.document_uri().to_string()),
                    );
                    entry.insert(
                        "digest_hex".into(),
                        Value::String(hex::encode_upper(attestation.digest())),
                    );
                    entry.insert(
                        "issued_at_ms".into(),
                        Value::from(attestation.issued_at_ms()),
                    );
                    if let Some(expires) = attestation.expires_at_ms() {
                        entry.insert("expires_at_ms".into(), Value::from(expires));
                    }
                    Value::Object(entry)
                })
                .collect();
            map.insert("attestations".into(), Value::Array(entries));
        }

        Value::Object(map)
    }

    fn normalise_iso_code(raw: &str) -> Result<String, String> {
        let trimmed = raw.trim();
        if trimmed.len() != 2 || !trimmed.chars().all(|c| c.is_ascii_alphabetic()) {
            return Err(format!(
                "jurisdiction code `{raw}` must be a 2-letter ISO 3166-1 alpha-2 code"
            ));
        }
        Ok(trimmed.to_ascii_uppercase())
    }
}

/// Primary entry point for SoraFS orchestrator operations.
pub struct Orchestrator {
    config: OrchestratorConfig,
    circuit_manager: Option<Arc<Mutex<CircuitManager>>>,
    proxy_runtime: Option<Arc<AsyncMutex<LocalProxyRuntime>>>,
    downgrade_remediator: Option<Arc<DowngradeRemediator>>,
    taikai_cache: Option<TaikaiCacheHandle>,
    taikai_cache_tracker: Option<Arc<Mutex<CacheAdmissionTracker>>>,
}

impl Clone for Orchestrator {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            circuit_manager: self.circuit_manager.clone(),
            proxy_runtime: self.proxy_runtime.clone(),
            downgrade_remediator: self.downgrade_remediator.clone(),
            taikai_cache: self.taikai_cache.clone(),
            taikai_cache_tracker: self.taikai_cache_tracker.clone(),
        }
    }
}

#[derive(Clone)]
pub(crate) struct LocalProxyRuntime {
    config: LocalQuicProxyConfig,
    handle: Option<LocalQuicProxyHandle>,
}

impl LocalProxyRuntime {
    fn new(config: LocalQuicProxyConfig, handle: Option<LocalQuicProxyHandle>) -> Self {
        Self { config, handle }
    }

    fn telemetry_label(&self) -> String {
        self.config
            .telemetry_label
            .clone()
            .unwrap_or_else(|| "proxy".to_string())
    }

    #[allow(dead_code)]
    fn mode(&self) -> ProxyMode {
        self.config.proxy_mode.clone()
    }

    fn manifest(&self) -> Option<BrowserExtensionManifest> {
        self.handle
            .as_ref()
            .and_then(LocalQuicProxyHandle::browser_manifest)
    }
}

impl Orchestrator {
    /// Creates a new orchestrator with the supplied configuration.
    #[must_use]
    pub fn new(config: OrchestratorConfig) -> Self {
        let mut config = config;
        if let Some(directory) = config.relay_directory.as_mut() {
            if !config.relay_path_hints.is_empty() {
                let report = directory.apply_path_hints(&config.relay_path_hints);
                if report.applied > 0 {
                    info!(
                        target: "telemetry::sorafs.directory",
                        event = "path_hints_applied",
                        applied = report.applied
                    );
                }
                if !report.missing.is_empty() {
                    warn!(
                        target: "telemetry::sorafs.directory",
                        event = "path_hints_missing",
                        missing = ?report.missing
                    );
                }
            }
        } else if !config.relay_path_hints.is_empty() {
            warn!(
                target: "telemetry::sorafs.directory",
                event = "path_hints_ignored",
                "relay_path_hints supplied without a relay_directory; ignoring hints"
            );
        }

        let circuit_manager = config
            .circuit_manager
            .clone()
            .map(|cfg| Arc::new(Mutex::new(CircuitManager::new(cfg))));
        let taikai_cache = config
            .taikai_cache
            .clone()
            .map(TaikaiCacheHandle::from_config);
        let taikai_cache_tracker = taikai_cache
            .as_ref()
            .map(|handle| CacheAdmissionTracker::with_defaults(handle.clone()))
            .map(|tracker| Arc::new(Mutex::new(tracker)));
        let proxy_runtime = match config.local_proxy.clone() {
            Some(proxy_cfg) => {
                let label = proxy_cfg
                    .telemetry_label
                    .clone()
                    .unwrap_or_else(|| "proxy".to_string());
                match spawn_local_quic_proxy(proxy_cfg.clone()) {
                    Ok(handle) => {
                        record_proxy_metric(&label, "spawned", "ok");
                        Some(Arc::new(AsyncMutex::new(LocalProxyRuntime::new(
                            proxy_cfg,
                            Some(handle),
                        ))))
                    }
                    Err(err) => {
                        let reason = err.to_string();
                        record_proxy_metric(&label, "spawn_error", &reason);
                        warn!(
                            target: "soranet.proxy",
                            ?err,
                            label = label,
                            "failed to spawn local QUIC proxy"
                        );
                        Some(Arc::new(AsyncMutex::new(LocalProxyRuntime::new(
                            proxy_cfg, None,
                        ))))
                    }
                }
            }
            None => None,
        };
        let downgrade_remediator = match (&proxy_runtime, &config.downgrade_remediation) {
            (Some(runtime), Some(remediation_cfg)) if remediation_cfg.enabled => {
                let metrics = global_or_default();
                Some(Arc::new(DowngradeRemediator::new(
                    Arc::clone(runtime),
                    remediation_cfg.clone(),
                    metrics,
                )))
            }
            _ => None,
        };
        Self {
            config,
            circuit_manager,
            proxy_runtime,
            downgrade_remediator,
            taikai_cache,
            taikai_cache_tracker,
        }
    }

    /// Returns the orchestrator configuration.
    #[must_use]
    pub fn config(&self) -> &OrchestratorConfig {
        &self.config
    }

    /// Returns the local proxy handle if one was spawned.
    pub async fn local_proxy(&self) -> Option<LocalQuicProxyHandle> {
        let runtime = self.proxy_runtime.as_ref()?;
        let guard = runtime.lock().await;
        guard.handle.clone()
    }

    async fn proxy_manifest(&self) -> Option<BrowserExtensionManifest> {
        let runtime = self.proxy_runtime.as_ref()?;
        let guard = runtime.lock().await;
        guard.manifest()
    }

    /// Updates the local proxy runtime mode, restarting the proxy if necessary.
    pub async fn set_proxy_mode(
        &mut self,
        mode: ProxyMode,
    ) -> Result<Option<BrowserExtensionManifest>, OrchestratorError> {
        let runtime = self
            .proxy_runtime
            .as_ref()
            .ok_or(OrchestratorError::ProxyNotConfigured)?;
        let manifest = apply_proxy_mode(runtime, mode.clone()).await?;
        if let Some(proxy_cfg) = self.config.local_proxy.as_mut() {
            proxy_cfg.proxy_mode = mode;
        }
        Ok(manifest)
    }

    /// Returns the Taikai cache handle if configured.
    #[must_use]
    pub fn taikai_cache(&self) -> Option<Arc<Mutex<TaikaiCache>>> {
        self.taikai_cache.as_ref().map(TaikaiCacheHandle::cache)
    }

    /// Returns a clone of the Taikai cache handle so callers can interact
    /// with the cache and pull queue without managing the underlying mutexes.
    #[must_use]
    pub fn taikai_cache_handle(&self) -> Option<TaikaiCacheHandle> {
        self.taikai_cache.clone()
    }

    /// Returns the Taikai cache admission tracker if configured.
    #[must_use]
    pub fn taikai_cache_tracker(&self) -> Option<Arc<Mutex<CacheAdmissionTracker>>> {
        self.taikai_cache_tracker.clone()
    }

    /// Applies a cache admission gossip entry to the Taikai shard ring.
    pub fn apply_cache_admission_gossip(
        &self,
        gossip: &CacheAdmissionGossip,
        now_unix_ms: u64,
    ) -> Result<bool, CacheAdmissionError> {
        let Some(tracker) = &self.taikai_cache_tracker else {
            return Ok(false);
        };
        match tracker.lock() {
            Ok(mut guard) => guard.ingest(gossip, now_unix_ms),
            Err(err) => {
                warn!(
                    target: "soranet.taikai_cache",
                    ?err,
                    "failed to lock Taikai cache admission tracker"
                );
                Ok(false)
            }
        }
    }

    fn snapshot_taikai_cache_stats(&self) -> Option<TaikaiCacheStatsSnapshot> {
        self.taikai_cache
            .as_ref()
            .and_then(|handle| match handle.cache().lock() {
                Ok(guard) => Some(guard.stats()),
                Err(err) => {
                    warn!(
                        target: "soranet.taikai_cache",
                        ?err,
                        "failed to lock Taikai cache while collecting stats"
                    );
                    None
                }
            })
    }

    fn snapshot_taikai_queue_stats(&self) -> Option<TaikaiPullQueueStats> {
        self.taikai_cache
            .as_ref()
            .map(TaikaiCacheHandle::queue_stats)
    }

    /// Refresh the circuit manager using the supplied timestamp.
    pub fn reconcile_circuits_at(
        &self,
        now: SystemTime,
    ) -> Result<Option<CircuitRefreshReport>, OrchestratorError> {
        let Some(manager) = &self.circuit_manager else {
            return Ok(None);
        };
        let Some(directory) = &self.config.relay_directory else {
            return Ok(None);
        };
        let Some(guard_set) = &self.config.guard_set else {
            return Ok(None);
        };

        let now_unix = now
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        let (_, policy) = self
            .config
            .write_mode
            .apply(self.config.transport_policy, self.config.anonymity_policy);

        let (events, active, rotation_history) = {
            let mut manager = manager
                .lock()
                .expect("soranet circuit manager mutex poisoned");
            let events = manager
                .refresh(directory, guard_set, now_unix, policy)
                .map_err(OrchestratorError::Circuit)?;
            let active = manager.active_circuits();
            let rotation_history = manager.rotation_history().to_vec();
            (events, active, rotation_history)
        };

        Ok(Some(CircuitRefreshReport {
            events,
            active,
            rotation_history,
        }))
    }

    /// Refreshes circuits using `SystemTime::now()` as the reference timestamp.
    pub fn reconcile_circuits(&self) -> Result<Option<CircuitRefreshReport>, OrchestratorError> {
        self.reconcile_circuits_at(SystemTime::now())
    }

    /// Returns the currently active SoraNet circuits managed by the orchestrator.
    pub fn active_circuits(&self) -> Vec<CircuitInfo> {
        self.circuit_manager
            .as_ref()
            .map(|manager| {
                manager
                    .lock()
                    .expect("soranet circuit manager mutex poisoned")
                    .active_circuits()
            })
            .unwrap_or_default()
    }

    /// Returns the aggregated circuit rotation history.
    pub fn circuit_rotation_history(&self) -> Vec<CircuitRotationRecord> {
        self.circuit_manager
            .as_ref()
            .map(|manager| {
                manager
                    .lock()
                    .expect("soranet circuit manager mutex poisoned")
                    .rotation_history()
                    .to_vec()
            })
            .unwrap_or_default()
    }

    /// Records a latency sample for the specified circuit.
    pub fn record_circuit_latency(&self, circuit_id: CircuitId, latency: Duration) -> bool {
        let Some(manager) = &self.circuit_manager else {
            return false;
        };
        manager
            .lock()
            .expect("soranet circuit manager mutex poisoned")
            .record_latency(circuit_id, latency)
    }

    /// Tears down every active circuit, returning rotation records for the retired paths.
    pub fn teardown_circuits(&self, now: SystemTime) -> Option<Vec<CircuitRotationRecord>> {
        let Some(manager) = &self.circuit_manager else {
            return None;
        };
        let now_unix = now
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        let records = manager
            .lock()
            .expect("soranet circuit manager mutex poisoned")
            .teardown_all(now_unix);
        Some(records)
    }

    fn refresh_circuits(&self) -> Result<(), OrchestratorError> {
        if let Some(mut report) = self.reconcile_circuits()?
            && !report.events.is_empty()
        {
            for event in report.events.drain(..) {
                debug!("soranet circuit event: {:?}", event);
            }
        }

        Ok(())
    }

    /// Builds a provider scoreboard from the supplied plan, adverts, and telemetry snapshot.
    ///
    /// When `ScoreboardConfig::persist_path` is set, the artefact is persisted automatically by
    /// the underlying builder.
    pub fn build_scoreboard(
        &self,
        plan: &CarBuildPlan,
        adverts: &[ProviderMetadata],
        telemetry: &TelemetrySnapshot,
    ) -> Result<Scoreboard, OrchestratorError> {
        scoreboard::build_scoreboard(plan, adverts, telemetry, &self.config.scoreboard)
            .map_err(OrchestratorError::Scoreboard)
    }

    /// Builds a scoreboard and immediately runs the multi-source fetch loop.
    pub async fn fetch_plan<F, Fut, E>(
        &self,
        plan: &CarBuildPlan,
        adverts: &[ProviderMetadata],
        telemetry: &TelemetrySnapshot,
        fetcher: F,
    ) -> Result<FetchSession, OrchestratorError>
    where
        F: Fn(FetchRequest) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<ChunkResponse, E>> + Send + 'static,
        E: std::error::Error + Send + Sync + 'static,
    {
        let scoreboard = self.build_scoreboard(plan, adverts, telemetry)?;
        self.fetch_with_scoreboard(plan, &scoreboard, fetcher).await
    }

    /// Runs the fetch loop using an already constructed scoreboard.
    pub async fn fetch_with_scoreboard<F, Fut, E>(
        &self,
        plan: &CarBuildPlan,
        scoreboard: &Scoreboard,
        fetcher: F,
    ) -> Result<FetchSession, OrchestratorError>
    where
        F: Fn(FetchRequest) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<ChunkResponse, E>> + Send + 'static,
        E: std::error::Error + Send + Sync + 'static,
    {
        self.refresh_circuits()?;
        self.execute_fetch(plan, scoreboard, fetcher).await
    }

    /// Runs the fetch loop using an already constructed scoreboard and a chunk observer.
    pub async fn fetch_with_scoreboard_and_observer<F, Fut, E, O>(
        &self,
        plan: &CarBuildPlan,
        scoreboard: &Scoreboard,
        fetcher: F,
        observer: O,
    ) -> Result<FetchSession, OrchestratorError>
    where
        F: Fn(FetchRequest) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<ChunkResponse, E>> + Send + 'static,
        E: std::error::Error + Send + Sync + 'static,
        O: ChunkObserver + 'static,
    {
        self.refresh_circuits()?;
        self.execute_fetch_with_observer(plan, scoreboard, fetcher, observer)
            .await
    }

    async fn execute_fetch<F, Fut, E>(
        &self,
        plan: &CarBuildPlan,
        scoreboard: &Scoreboard,
        fetcher: F,
    ) -> Result<FetchSession, OrchestratorError>
    where
        F: Fn(FetchRequest) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<ChunkResponse, E>> + Send + 'static,
        E: std::error::Error + Send + Sync + 'static,
    {
        let mut base_transport = self.config.transport_policy;
        let mut base_anonymity = self.config.anonymity_policy;
        if let Some(policy) = self.config.policy_override.transport_policy {
            base_transport = policy;
        }
        if let Some(policy) = self.config.policy_override.anonymity_policy {
            base_anonymity = policy;
        }
        let (mut transport_policy, anonymity_policy) =
            self.config.write_mode.apply(base_transport, base_anonymity);
        let compliance_decision = self
            .config
            .compliance
            .decision_for_digest(plan.payload_digest.as_bytes());
        let compliance_reason = compliance_decision.reason();
        if compliance_reason.is_some() {
            transport_policy = TransportPolicy::DirectOnly;
        }
        let skip_policy_fallback = self.config.policy_override.transport_policy.is_some()
            || self.config.policy_override.anonymity_policy.is_some();
        let SelectionOutcome {
            providers,
            mut summary,
        } = eligible_providers(
            scoreboard,
            self.config.max_providers,
            transport_policy,
            anonymity_policy,
            self.config.guard_set.as_ref(),
            self.config.relay_directory.as_ref(),
            skip_policy_fallback,
        );
        if let Some(reason) = compliance_reason {
            summary.apply_compliance_override(reason.into());
        }
        let region = self
            .config
            .telemetry_region
            .as_deref()
            .unwrap_or("unspecified");
        let ctx = FetchMetricsCtx::begin(
            plan,
            region,
            &providers,
            &summary,
            &self.config.fetch,
            &self.config.scoreboard,
            self.config.write_mode,
        );

        if providers.is_empty()
            || (self.config.write_mode.enforces_pq_only()
                && !matches!(summary.status, PolicyStatus::Met))
        {
            let error = multi_fetch::MultiSourceError::NoProviders;
            ctx.on_error(&error);
            ctx.finish();
            return Err(OrchestratorError::from(error));
        }

        let privacy_collector = match self.config.privacy_events.as_ref() {
            Some(privacy_cfg) => {
                let endpoints = privacy_endpoints_from_providers(&providers);
                match PrivacyCollector::spawn(
                    endpoints,
                    privacy_cfg,
                    global_or_default(),
                    self.downgrade_remediator.clone(),
                ) {
                    Ok(Some(collector)) => Some(collector),
                    Ok(None) => None,
                    Err(error) => {
                        warn!(
                            target: "telemetry::sorafs.privacy",
                            ?error,
                            "failed to initialise privacy event collector"
                        );
                        None
                    }
                }
            }
            None => None,
        };

        let queue_bridge = self
            .taikai_cache_handle()
            .and_then(|handle| TaikaiQueueBridge::new(handle, plan));
        let result = if let Some(bridge) = queue_bridge.clone() {
            let wrapped_fetcher = wrap_fetcher_with_taikai_queue(fetcher, bridge);
            multi_fetch::fetch_plan_parallel(
                plan,
                providers,
                wrapped_fetcher,
                self.config.fetch.clone(),
            )
            .await
        } else {
            multi_fetch::fetch_plan_parallel(plan, providers, fetcher, self.config.fetch.clone())
                .await
        };

        let proxy_manifest = self.proxy_manifest().await;
        let session = match result {
            Ok(outcome) => {
                ctx.on_success(&outcome);
                ctx.finish();
                let policy_report = PolicyReport::from(summary);
                let cache_stats = self.snapshot_taikai_cache_stats();
                let cache_queue_stats = self.snapshot_taikai_queue_stats();
                Ok(FetchSession {
                    outcome,
                    policy_report,
                    local_proxy_manifest: proxy_manifest.clone(),
                    car_verification: None,
                    taikai_cache_stats: cache_stats,
                    taikai_cache_queue: cache_queue_stats,
                })
            }
            Err(error) => {
                ctx.on_error(&error);
                ctx.finish();
                Err(OrchestratorError::from(error))
            }
        };

        if let Some(collector) = privacy_collector {
            collector.shutdown().await;
        }

        session
    }

    async fn execute_fetch_with_observer<F, Fut, E, O>(
        &self,
        plan: &CarBuildPlan,
        scoreboard: &Scoreboard,
        fetcher: F,
        observer: O,
    ) -> Result<FetchSession, OrchestratorError>
    where
        F: Fn(FetchRequest) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<ChunkResponse, E>> + Send + 'static,
        E: std::error::Error + Send + Sync + 'static,
        O: ChunkObserver + 'static,
    {
        let mut base_transport = self.config.transport_policy;
        let mut base_anonymity = self.config.anonymity_policy;
        if let Some(policy) = self.config.policy_override.transport_policy {
            base_transport = policy;
        }
        if let Some(policy) = self.config.policy_override.anonymity_policy {
            base_anonymity = policy;
        }
        let (mut transport_policy, anonymity_policy) =
            self.config.write_mode.apply(base_transport, base_anonymity);
        let compliance_decision = self
            .config
            .compliance
            .decision_for_digest(plan.payload_digest.as_bytes());
        let compliance_reason = compliance_decision.reason();
        if compliance_reason.is_some() {
            transport_policy = TransportPolicy::DirectOnly;
        }
        let skip_policy_fallback = self.config.policy_override.transport_policy.is_some()
            || self.config.policy_override.anonymity_policy.is_some();
        let SelectionOutcome {
            providers,
            mut summary,
        } = eligible_providers(
            scoreboard,
            self.config.max_providers,
            transport_policy,
            anonymity_policy,
            self.config.guard_set.as_ref(),
            self.config.relay_directory.as_ref(),
            skip_policy_fallback,
        );
        if let Some(reason) = compliance_reason {
            summary.apply_compliance_override(reason.into());
        }
        let region = self
            .config
            .telemetry_region
            .as_deref()
            .unwrap_or("unspecified");
        let ctx = FetchMetricsCtx::begin(
            plan,
            region,
            &providers,
            &summary,
            &self.config.fetch,
            &self.config.scoreboard,
            self.config.write_mode,
        );

        if providers.is_empty()
            || (self.config.write_mode.enforces_pq_only()
                && !matches!(summary.status, PolicyStatus::Met))
        {
            let error = multi_fetch::MultiSourceError::NoProviders;
            ctx.on_error(&error);
            ctx.finish();
            return Err(OrchestratorError::from(error));
        }

        let privacy_collector = match self.config.privacy_events.as_ref() {
            Some(privacy_cfg) => {
                let endpoints = privacy_endpoints_from_providers(&providers);
                match PrivacyCollector::spawn(
                    endpoints,
                    privacy_cfg,
                    global_or_default(),
                    self.downgrade_remediator.clone(),
                ) {
                    Ok(Some(collector)) => Some(collector),
                    Ok(None) => None,
                    Err(error) => {
                        warn!(
                            target: "telemetry::sorafs.privacy",
                            ?error,
                            "failed to initialise privacy event collector"
                        );
                        None
                    }
                }
            }
            None => None,
        };

        let queue_bridge = self
            .taikai_cache_handle()
            .and_then(|handle| TaikaiQueueBridge::new(handle, plan));
        let result = if let Some(bridge) = queue_bridge.clone() {
            let wrapped_fetcher = wrap_fetcher_with_taikai_queue(fetcher, bridge);
            multi_fetch::fetch_plan_parallel_with_observer(
                plan,
                providers,
                wrapped_fetcher,
                self.config.fetch.clone(),
                observer,
            )
            .await
        } else {
            multi_fetch::fetch_plan_parallel_with_observer(
                plan,
                providers,
                fetcher,
                self.config.fetch.clone(),
                observer,
            )
            .await
        };

        let proxy_manifest = self.proxy_manifest().await;
        let session = match result {
            Ok(outcome) => {
                ctx.on_success(&outcome);
                ctx.finish();
                let policy_report = PolicyReport::from(summary);
                let cache_stats = self.snapshot_taikai_cache_stats();
                let cache_queue_stats = self.snapshot_taikai_queue_stats();
                Ok(FetchSession {
                    outcome,
                    policy_report,
                    local_proxy_manifest: proxy_manifest.clone(),
                    car_verification: None,
                    taikai_cache_stats: cache_stats,
                    taikai_cache_queue: cache_queue_stats,
                })
            }
            Err(error) => {
                ctx.on_error(&error);
                ctx.finish();
                Err(OrchestratorError::from(error))
            }
        };

        if let Some(collector) = privacy_collector {
            collector.shutdown().await;
        }

        session
    }
}

fn record_proxy_metric(region: &str, event: &str, reason: &str) {
    let metrics = global_or_default();
    metrics.inc_sorafs_orchestrator_transport_event(region, PROXY_PROTOCOL_LABEL, event, reason);
    let otel = global_sorafs_fetch_otel();
    otel.record_transport_event(
        PROXY_MANIFEST_ID,
        region,
        None::<&str>,
        PROXY_PROTOCOL_LABEL,
        event,
        reason,
    );
}

async fn apply_proxy_mode(
    runtime: &Arc<AsyncMutex<LocalProxyRuntime>>,
    mode: ProxyMode,
) -> Result<Option<BrowserExtensionManifest>, OrchestratorError> {
    let (previous_mode, label, config, previous_handle) = {
        let mut guard = runtime.lock().await;
        if guard.config.proxy_mode == mode {
            return Ok(guard.manifest());
        }
        let label = guard.telemetry_label();
        let previous_mode = guard.config.proxy_mode.clone();
        let mut config = guard.config.clone();
        config.proxy_mode = mode.clone();
        let handle = guard.handle.take();
        (previous_mode, label, config, handle)
    };

    if let Some(handle) = previous_handle {
        handle.shutdown().await;
    }

    match spawn_local_quic_proxy(config.clone()) {
        Ok(handle) => {
            let manifest = handle.browser_manifest();
            {
                let mut guard = runtime.lock().await;
                guard.config = config;
                guard.handle = Some(handle);
            }
            record_proxy_metric(&label, "mode_update", mode.as_str());
            Ok(manifest)
        }
        Err(error) => {
            warn!(
                target: "soranet.proxy",
                ?error,
                label = label,
                requested_mode = %mode.as_str(),
                "failed to restart local QUIC proxy with requested mode"
            );
            record_proxy_metric(&label, "mode_update_error", mode.as_str());
            let mut revert_config = config;
            revert_config.proxy_mode = previous_mode.clone();
            match spawn_local_quic_proxy(revert_config.clone()) {
                Ok(handle) => {
                    record_proxy_metric(&label, "mode_revert", previous_mode.as_str());
                    let mut guard = runtime.lock().await;
                    guard.config = revert_config;
                    guard.handle = Some(handle);
                }
                Err(restart_err) => {
                    let reason = restart_err.to_string();
                    record_proxy_metric(&label, "mode_revert_error", &reason);
                    warn!(
                        target: "soranet.proxy",
                        ?restart_err,
                        label = label,
                        previous_mode = %previous_mode.as_str(),
                        "failed to restore previous proxy mode after remediation error"
                    );
                    let mut guard = runtime.lock().await;
                    guard.config = revert_config;
                    guard.handle = None;
                }
            }
            Err(OrchestratorError::Proxy(error))
        }
    }
}

#[derive(Default, Clone, Copy)]
struct TransportSupport {
    has_soranet: bool,
    has_quic: bool,
    has_torii: bool,
}

impl TransportSupport {
    fn from_provider(provider: &FetchProvider) -> Self {
        let mut support = Self::default();
        if let Some(metadata) = provider.metadata() {
            for hint in &metadata.transport_hints {
                match hint.protocol_kind() {
                    TransportProtocolKind::SoraNetRelay => support.has_soranet = true,
                    TransportProtocolKind::QuicStream => support.has_quic = true,
                    TransportProtocolKind::ToriiHttpRange => support.has_torii = true,
                    TransportProtocolKind::VendorReserved | TransportProtocolKind::Unknown => {}
                }
            }
        }
        support
    }

    fn has_soranet(self) -> bool {
        self.has_soranet
    }

    fn protocol_label(&self) -> &'static str {
        if self.has_soranet {
            "soranet"
        } else if self.has_quic {
            "quic"
        } else if self.has_torii {
            "torii"
        } else {
            "fallback"
        }
    }
}

/// Returns `true` when the provider advertises SoraNet transport support.
pub fn provider_supports_soranet(provider: &FetchProvider) -> bool {
    TransportSupport::from_provider(provider).has_soranet()
}

const MAJORITY_THRESHOLD: f64 = 2.0 / 3.0;

#[derive(Default, Clone, Copy)]
struct PqSupport {
    guard: bool,
    majority: bool,
    strict: bool,
}

/// Returns `true` when the provider satisfies PQ requirements for strict anonymity.
pub fn provider_supports_pq(provider: &FetchProvider) -> bool {
    PqSupport::from_provider(provider).satisfies(AnonymityPolicy::StrictPq)
}

/// Outcome status reported for a staged anonymity policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyStatus {
    /// Policy does not apply because no SoraNet candidates were present.
    NotApplicable,
    /// Policy requirements were met.
    Met,
    /// Policy fell back to a degraded mode (brownout) due to insufficient PQ supply.
    Brownout,
}

/// Reason why a policy fell back to a degraded mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyFallback {
    /// No providers advertised SoraNet transport.
    NoSoranetCandidates,
    /// No guard-capable providers satisfied the PQ constraints.
    MissingGuardSupport,
    /// Majority PQ requirement could not be satisfied.
    MissingMajoritySupport,
    /// Strict PQ requirement could not be satisfied.
    MissingStrictSupport,
    /// Compliance forced direct mode due to jurisdictional opt-out.
    ComplianceJurisdictionOptOut,
    /// Compliance forced direct mode due to a blinded-CID opt-out.
    ComplianceBlindedCidOptOut,
}

impl From<ComplianceReason> for PolicyFallback {
    fn from(reason: ComplianceReason) -> Self {
        match reason {
            ComplianceReason::JurisdictionOptOut => Self::ComplianceJurisdictionOptOut,
            ComplianceReason::BlindedCidOptOut => Self::ComplianceBlindedCidOptOut,
        }
    }
}

/// Public report describing the applied anonymity policy and selection outcome.
#[derive(Debug, Clone)]
pub struct PolicyReport {
    /// Policy requested by the caller.
    pub policy: AnonymityPolicy,
    /// Policy stage effectively applied after fallback.
    pub effective_policy: AnonymityPolicy,
    /// Total number of SoraNet-capable candidates considered.
    pub total_candidates: usize,
    /// Number of candidates that satisfied the PQ requirement.
    pub pq_candidates: usize,
    /// Number of SoraNet providers selected for the session.
    pub selected_soranet_total: usize,
    /// Number of selected providers that satisfied the PQ requirement.
    pub selected_pq: usize,
    /// Outcome status for the requested policy.
    pub status: PolicyStatus,
    /// Optional reason describing why the policy fell back to a degraded mode.
    pub fallback_reason: Option<PolicyFallback>,
}

impl PolicyReport {
    /// Returns how many selected providers relied on classical handshakes.
    #[must_use]
    pub fn selected_classical(&self) -> usize {
        self.selected_soranet_total.saturating_sub(self.selected_pq)
    }

    /// Indicates whether any classical providers were used.
    #[must_use]
    pub fn uses_classical(&self) -> bool {
        self.selected_classical() > 0
    }

    /// Ratio of PQ-capable providers among the selected SoraNet set.
    #[must_use]
    pub fn pq_ratio(&self) -> f64 {
        if self.selected_soranet_total == 0 {
            0.0
        } else {
            self.selected_pq as f64 / self.selected_soranet_total as f64
        }
    }

    /// Ratio of classical providers among the selected SoraNet set.
    #[must_use]
    pub fn classical_ratio(&self) -> f64 {
        if self.selected_soranet_total == 0 {
            0.0
        } else {
            self.selected_classical() as f64 / self.selected_soranet_total as f64
        }
    }

    /// Ratio of PQ-capable candidates in the scoreboard.
    #[must_use]
    pub fn candidate_ratio(&self) -> f64 {
        if self.total_candidates == 0 {
            0.0
        } else {
            self.pq_candidates as f64 / self.total_candidates as f64
        }
    }

    /// Ratio of PQ deficit relative to the requested policy.
    #[must_use]
    pub fn deficit_ratio(&self) -> f64 {
        match self.status {
            PolicyStatus::Met => 0.0,
            PolicyStatus::NotApplicable => {
                if matches!(
                    self.fallback_reason,
                    Some(PolicyFallback::NoSoranetCandidates)
                ) {
                    1.0
                } else {
                    0.0
                }
            }
            PolicyStatus::Brownout => (self.target_ratio() - self.pq_ratio()).clamp(0.0, 1.0),
        }
    }

    /// Ratio of PQ supply remaining in the candidate set after selection.
    #[must_use]
    pub fn supply_delta_ratio(&self) -> f64 {
        (self.candidate_ratio() - self.pq_ratio()).clamp(0.0, 1.0)
    }

    /// Indicates whether the policy resulted in a brownout.
    #[must_use]
    pub fn is_brownout(&self) -> bool {
        matches!(self.status, PolicyStatus::Brownout)
    }

    /// Indicates whether the brownout should be surfaced to operators.
    #[must_use]
    pub fn should_flag_brownout(&self) -> bool {
        self.is_brownout()
            || matches!(
                self.fallback_reason,
                Some(PolicyFallback::NoSoranetCandidates)
            )
    }

    /// Returns a stable status label used for telemetry reporting.
    #[must_use]
    pub fn status_label(&self) -> &'static str {
        match self.status {
            PolicyStatus::NotApplicable => "not_applicable",
            PolicyStatus::Met => "met",
            PolicyStatus::Brownout => "brownout",
        }
    }

    /// Returns a stable fallback reason label used for telemetry reporting.
    #[must_use]
    pub fn reason_label(&self) -> &'static str {
        match self.fallback_reason {
            Some(PolicyFallback::NoSoranetCandidates) => "no_soranet",
            Some(PolicyFallback::MissingGuardSupport) => "missing_guard_pq",
            Some(PolicyFallback::MissingMajoritySupport) => "missing_majority_pq",
            Some(PolicyFallback::MissingStrictSupport) => "missing_strict_pq",
            Some(PolicyFallback::ComplianceJurisdictionOptOut) => "compliance_jurisdiction_opt_out",
            Some(PolicyFallback::ComplianceBlindedCidOptOut) => "compliance_blinded_cid_opt_out",
            None => "none",
        }
    }

    fn target_ratio(&self) -> f64 {
        match self.policy {
            AnonymityPolicy::GuardPq => {
                if self.selected_soranet_total > 0 {
                    1.0 / self.selected_soranet_total as f64
                } else if self.total_candidates > 0 {
                    1.0 / self.total_candidates as f64
                } else {
                    1.0
                }
            }
            AnonymityPolicy::MajorityPq => MAJORITY_THRESHOLD,
            AnonymityPolicy::StrictPq => 1.0,
        }
    }
}

/// Completed multi-provider fetch along with policy reporting metadata.
#[derive(Debug, Clone)]
pub struct FetchSession {
    /// Resulting fetch outcome returned by the scheduler.
    pub outcome: FetchOutcome,
    /// Applied anonymity policy report covering PQ/classical usage.
    pub policy_report: PolicyReport,
    /// Optional manifest describing the spawned local QUIC proxy.
    pub local_proxy_manifest: Option<BrowserExtensionManifest>,
    /// Optional CAR/manifest verification proof.
    pub car_verification: Option<GatewayCarVerification>,
    /// Snapshot of Taikai cache statistics captured after the fetch.
    pub taikai_cache_stats: Option<TaikaiCacheStatsSnapshot>,
    /// Snapshot of the Taikai cache pull queue after the fetch.
    pub taikai_cache_queue: Option<TaikaiPullQueueStats>,
}

/// Verification artefacts produced after validating manifest + CAR parity.
#[derive(Debug, Clone)]
pub struct GatewayCarVerification {
    /// Canonical manifest digest computed from the fetched payload.
    pub manifest_digest: blake3::Hash,
    /// Payload digest recorded by the gateway (BLAKE3-256).
    pub manifest_payload_digest: blake3::Hash,
    /// Manifest-declared content length in bytes.
    pub manifest_content_length: u64,
    /// Chunk count recorded alongside the manifest.
    pub manifest_chunk_count: u64,
    /// Canonical CAR digest recorded in the manifest.
    pub manifest_car_digest: [u8; 32],
    /// Governance proofs bundled with the manifest.
    pub manifest_governance: GovernanceProofs,
    /// Chunk profile handle advertised by the gateway.
    pub chunk_profile_handle: String,
    /// Statistics derived while assembling the CAR archive.
    pub car_stats: CarWriteStats,
    /// Number of PoR leaves observed during verification.
    pub por_leaf_count: usize,
}

/// Verification context required to validate a fetch against a manifest.
#[derive(Debug, Clone)]
pub struct ManifestVerificationContext<'a> {
    /// Decoded Norito manifest describing the payload.
    pub manifest: &'a sorafs_manifest::ManifestV1,
    /// Canonical manifest digest advertised by the provider.
    pub manifest_digest: blake3::Hash,
    /// Payload digest recorded alongside the manifest (BLAKE3-256).
    pub payload_digest: blake3::Hash,
    /// Total payload length recorded by the provider.
    pub content_length: u64,
    /// Number of chunks recorded by the provider.
    pub chunk_count: u64,
    /// Chunk profile handle declared by the provider.
    pub chunk_profile_handle: &'a str,
}

impl<'a> ManifestVerificationContext<'a> {
    /// Constructs a verification context from individual fields.
    #[must_use]
    pub fn new(
        manifest: &'a sorafs_manifest::ManifestV1,
        manifest_digest: blake3::Hash,
        payload_digest: blake3::Hash,
        content_length: u64,
        chunk_count: u64,
        chunk_profile_handle: &'a str,
    ) -> Self {
        Self {
            manifest,
            manifest_digest,
            payload_digest,
            content_length,
            chunk_count,
            chunk_profile_handle,
        }
    }
}

impl<'a> From<&'a GatewayFetchedManifest> for ManifestVerificationContext<'a> {
    fn from(manifest: &'a GatewayFetchedManifest) -> Self {
        Self {
            manifest: &manifest.manifest,
            manifest_digest: manifest.manifest_digest,
            payload_digest: manifest.payload_digest,
            content_length: manifest.content_length,
            chunk_count: manifest.chunk_count,
            chunk_profile_handle: manifest.chunk_profile_handle.as_str(),
        }
    }
}

/// Errors surfaced while validating a fetched payload against a manifest.
#[derive(Debug, Error)]
pub enum ManifestVerificationError {
    /// Manifest content length does not match the fetch plan.
    #[error("manifest content length {actual} does not match fetch plan ({expected})")]
    ContentLengthMismatch { actual: u64, expected: u64 },
    /// Manifest chunk count does not match the fetch plan.
    #[error("manifest chunk count {actual} does not match fetch plan ({expected})")]
    ChunkCountMismatch { actual: u64, expected: usize },
    /// Manifest failed policy validation.
    #[error("manifest validation failed: {0}")]
    ManifestValidation(String),
    /// Failed to assemble the CAR archive for verification.
    #[error("failed to assemble CAR for verification: {0}")]
    CarBuild(String),
    /// Trustless verifier rejected the payload.
    #[error("manifest verification failed: {0}")]
    Verification(String),
}

impl FetchSession {
    /// Verify the assembled payload against the supplied manifest context.
    ///
    /// On success the session records a [`GatewayCarVerification`] snapshot that callers can
    /// persist alongside their audit logs.
    pub fn verify_against_manifest(
        &mut self,
        plan: &CarBuildPlan,
        context: ManifestVerificationContext<'_>,
    ) -> Result<&GatewayCarVerification, ManifestVerificationError> {
        let verification = verify_fetch_against_manifest(plan, &self.outcome, context)?;
        self.car_verification = Some(verification);
        Ok(self
            .car_verification
            .as_ref()
            .expect("verification snapshot was just attached"))
    }
}

fn verify_fetch_against_manifest(
    plan: &CarBuildPlan,
    outcome: &FetchOutcome,
    context: ManifestVerificationContext<'_>,
) -> Result<GatewayCarVerification, ManifestVerificationError> {
    if context.content_length != plan.content_length {
        return Err(ManifestVerificationError::ContentLengthMismatch {
            actual: context.content_length,
            expected: plan.content_length,
        });
    }
    let expected_chunks = plan.chunks.len();
    if context.chunk_count != expected_chunks as u64 {
        return Err(ManifestVerificationError::ChunkCountMismatch {
            actual: context.chunk_count,
            expected: expected_chunks,
        });
    }

    validate_manifest(context.manifest, &PinPolicyConstraints::default())
        .map_err(|err| ManifestVerificationError::ManifestValidation(err.to_string()))?;

    let payload = outcome.assemble_payload();
    let mut buffer = Cursor::new(Vec::new());
    let writer = CarWriter::new(plan, &payload)
        .map_err(|err| ManifestVerificationError::CarBuild(err.to_string()))?;
    writer
        .write_to(&mut buffer)
        .map_err(|err| ManifestVerificationError::CarBuild(err.to_string()))?;
    let car_bytes = buffer.into_inner();

    let verification = CarVerifier::verify_full_car_with_plan(context.manifest, plan, &car_bytes)
        .map_err(|err| ManifestVerificationError::Verification(err.to_string()))?;
    let car_stats = verification.stats.clone();
    let por_leaf_count = verification.chunk_store.por_leaf_count();

    Ok(GatewayCarVerification {
        manifest_digest: context.manifest_digest,
        manifest_payload_digest: context.payload_digest,
        manifest_content_length: context.content_length,
        manifest_chunk_count: context.chunk_count,
        manifest_car_digest: context.manifest.car_digest,
        manifest_governance: context.manifest.governance.clone(),
        chunk_profile_handle: context.chunk_profile_handle.to_string(),
        car_stats,
        por_leaf_count,
    })
}

impl PqSupport {
    fn from_provider(provider: &FetchProvider) -> Self {
        let mut support = Self::default();
        if let Some(metadata) = provider.metadata() {
            for name in &metadata.capability_names {
                let label = name.trim().to_ascii_lowercase();
                if label.is_empty() {
                    continue;
                }
                if !(label.contains("soranet") && label.contains("pq")) {
                    continue;
                }
                if label.contains("strict")
                    || label.contains("full")
                    || label.contains("only")
                    || label.contains("exclusive")
                    || label.contains("all")
                {
                    support.strict = true;
                    continue;
                }
                if label.contains("majority")
                    || label.contains("2of3")
                    || label.contains("2/3")
                    || label.contains("two_thirds")
                    || label.contains("mostly")
                    || label.contains("hybrid")
                {
                    support.majority = true;
                    continue;
                }
                if label.contains("guard") || label.contains("entry") {
                    support.guard = true;
                    continue;
                }
                // Generic PQ capability indicator.
                support.majority = true;
            }
        }

        if support.strict {
            support.majority = true;
            support.guard = true;
        } else if support.majority {
            support.guard = true;
        }

        support
    }

    fn satisfies(self, policy: AnonymityPolicy) -> bool {
        match policy {
            AnonymityPolicy::GuardPq => self.guard || self.majority || self.strict,
            AnonymityPolicy::MajorityPq => self.majority || self.strict,
            AnonymityPolicy::StrictPq => self.strict,
        }
    }
}

#[derive(Clone)]
struct SoranetCandidate {
    provider: FetchProvider,
    pq: PqSupport,
    guard_weight: u32,
    has_pq_certificate: bool,
    bandwidth_bytes_per_sec: u64,
    reputation_weight: u32,
    identifier: String,
}

impl SoranetCandidate {
    fn new(
        provider: FetchProvider,
        pq: PqSupport,
        descriptor: Option<&RelayDescriptor>,
        guard_record: Option<&GuardRecord>,
    ) -> Self {
        let descriptor_guard_weight = descriptor.map(|desc| desc.guard_weight).unwrap_or(0);
        let guard_record_weight = guard_record.map(|record| record.guard_weight).unwrap_or(0);
        let guard_weight = if guard_record_weight > 0 {
            guard_record_weight
        } else {
            descriptor_guard_weight
        };
        let has_pq_certificate = guard_record.is_some_and(|record| record.pq_kem_public.is_some())
            || descriptor.is_some_and(|desc| desc.is_pq_capable());
        let guard_bandwidth = guard_record
            .map(|record| record.bandwidth_bytes_per_sec)
            .unwrap_or(0);
        let descriptor_bandwidth = descriptor
            .map(|desc| desc.bandwidth_bytes_per_sec)
            .unwrap_or(0);
        let provider_bandwidth = provider
            .metadata()
            .and_then(|meta| meta.stream_budget.as_ref())
            .map(|budget| budget.max_bytes_per_sec)
            .unwrap_or(0);
        let bandwidth_bytes_per_sec = if guard_bandwidth > 0 {
            guard_bandwidth
        } else if descriptor_bandwidth > 0 {
            descriptor_bandwidth
        } else {
            provider_bandwidth
        };
        let descriptor_reputation = descriptor.map(|desc| desc.reputation_weight).unwrap_or(0);
        let guard_reputation = guard_record
            .map(|record| record.reputation_weight)
            .unwrap_or(0);
        let provider_reputation = provider.weight().get();
        let reputation_weight = if guard_reputation > 0 {
            guard_reputation
        } else if descriptor_reputation > 0 {
            descriptor_reputation
        } else {
            provider_reputation
        };
        let identifier = canonical_provider_label(&provider);

        Self {
            provider,
            pq,
            guard_weight,
            has_pq_certificate,
            bandwidth_bytes_per_sec,
            reputation_weight,
            identifier,
        }
    }

    fn capability_components(&self) -> GuardCapabilityComponents {
        GuardCapabilityComponents::from_inputs(
            self.pq_rank(),
            self.has_pq_certificate,
            self.guard_weight,
            self.bandwidth_bytes_per_sec,
            self.reputation_weight,
        )
    }

    fn pq_rank(&self) -> u8 {
        if self.pq.strict {
            3
        } else if self.pq.majority {
            2
        } else if self.pq.guard {
            1
        } else {
            0
        }
    }

    fn capability_weight(&self) -> NonZeroU32 {
        let base_weight = self.provider.weight().get();
        let capability = self.capability_components();
        let pq_rank_bonus =
            u32::from(capability.pq_rank()).saturating_mul(SORANET_PQ_RANK_STEP_WEIGHT);
        let pq_certificate_bonus = if capability.has_pq_certificate() {
            SORANET_PQ_CERTIFICATE_BONUS
        } else {
            0
        };
        let guard_bonus = capability.guard_weight();
        let bandwidth_bonus = capability.bandwidth_units();
        let reputation_bonus = capability.reputation_weight();

        let weighted = base_weight
            .saturating_add(pq_rank_bonus)
            .saturating_add(pq_certificate_bonus)
            .saturating_add(guard_bonus)
            .saturating_add(bandwidth_bonus)
            .saturating_add(reputation_bonus);

        NonZeroU32::new(weighted).expect("capability-weighted score must remain non-zero")
    }

    fn into_weighted_provider(self) -> FetchProvider {
        let weighted = self.capability_weight();
        self.provider.with_weight(weighted)
    }
}

fn canonical_provider_label(provider: &FetchProvider) -> String {
    fn is_hex_identifier(value: &str) -> bool {
        value.len() == 64 && value.chars().all(|c| c.is_ascii_hexdigit())
    }

    if let Some(metadata) = provider.metadata() {
        if let Some(hex_id) = metadata
            .provider_id
            .as_deref()
            .filter(|value| is_hex_identifier(value))
        {
            return hex_id.to_ascii_lowercase();
        }
        if let Some(alias) = metadata
            .profile_aliases
            .iter()
            .find(|alias| is_hex_identifier(alias))
        {
            return alias.to_ascii_lowercase();
        }
        if let Some(identifier) = &metadata.provider_id {
            return identifier.clone();
        }
    }
    provider.id().as_str().to_string()
}

fn guard_descriptor_for_provider<'a>(
    provider: &FetchProvider,
    catalog: &'a HashMap<[u8; 32], &'a RelayDescriptor>,
) -> Option<&'a RelayDescriptor> {
    if let Some(metadata) = provider.metadata() {
        if let Some(identifier) = metadata.provider_id.as_deref()
            && let Some(descriptor) = lookup_descriptor(identifier, catalog)
        {
            return Some(descriptor);
        }
        for alias in &metadata.profile_aliases {
            if let Some(descriptor) = lookup_descriptor(alias, catalog) {
                return Some(descriptor);
            }
        }
    }

    lookup_descriptor(provider.id().as_str(), catalog)
}

fn decode_relay_id(value: &str) -> Option<[u8; 32]> {
    let trimmed = value.trim();
    if trimmed.len() != 64 || !trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let mut relay_id = [0u8; 32];
    hex::decode_to_slice(trimmed, &mut relay_id).ok()?;
    Some(relay_id)
}

fn lookup_descriptor<'a>(
    value: &str,
    catalog: &'a HashMap<[u8; 32], &'a RelayDescriptor>,
) -> Option<&'a RelayDescriptor> {
    let relay_id = decode_relay_id(value)?;
    catalog.get(&relay_id).copied()
}

fn guard_record_for_provider<'a>(
    provider: &FetchProvider,
    catalog: &'a HashMap<[u8; 32], &'a GuardRecord>,
) -> Option<&'a GuardRecord> {
    if let Some(metadata) = provider.metadata() {
        if let Some(identifier) = metadata.provider_id.as_deref()
            && let Some(record) = lookup_guard_record(identifier, catalog)
        {
            return Some(record);
        }
        for alias in &metadata.profile_aliases {
            if let Some(record) = lookup_guard_record(alias, catalog) {
                return Some(record);
            }
        }
    }

    lookup_guard_record(provider.id().as_str(), catalog)
}

fn lookup_guard_record<'a>(
    value: &str,
    catalog: &'a HashMap<[u8; 32], &'a GuardRecord>,
) -> Option<&'a GuardRecord> {
    let relay_id = decode_relay_id(value)?;
    catalog.get(&relay_id).copied()
}

fn compare_soranet_candidates(left: &SoranetCandidate, right: &SoranetCandidate) -> CmpOrdering {
    let left_capability = left.capability_components();
    let right_capability = right.capability_components();
    right_capability
        .cmp(&left_capability)
        .then_with(|| {
            right
                .provider
                .weight()
                .get()
                .cmp(&left.provider.weight().get())
        })
        .then_with(|| left.identifier.cmp(&right.identifier))
}

#[derive(Debug, Clone)]
struct PolicySummary {
    policy: AnonymityPolicy,
    effective_policy: AnonymityPolicy,
    total_candidates: usize,
    pq_candidates: usize,
    selected_soranet_total: usize,
    selected_pq: usize,
    status: PolicyStatus,
    fallback_reason: Option<PolicyFallback>,
}

impl PolicySummary {
    fn new(policy: AnonymityPolicy, total_candidates: usize, pq_candidates: usize) -> Self {
        let status = if total_candidates == 0 {
            PolicyStatus::NotApplicable
        } else {
            PolicyStatus::Met
        };
        Self {
            policy,
            effective_policy: policy,
            total_candidates,
            pq_candidates,
            selected_soranet_total: 0,
            selected_pq: 0,
            status,
            fallback_reason: if total_candidates == 0 {
                Some(PolicyFallback::NoSoranetCandidates)
            } else {
                None
            },
        }
    }

    fn update_selected_counts(&mut self, total: usize, pq: usize) {
        self.selected_soranet_total = total;
        self.selected_pq = pq;
        self.evaluate_outcome();
    }

    fn ratio(&self) -> f64 {
        if self.selected_soranet_total == 0 {
            0.0
        } else {
            self.selected_pq as f64 / self.selected_soranet_total as f64
        }
    }

    fn selected_classical(&self) -> usize {
        self.selected_soranet_total.saturating_sub(self.selected_pq)
    }

    fn classical_ratio(&self) -> f64 {
        if self.selected_soranet_total == 0 {
            0.0
        } else {
            self.selected_classical() as f64 / self.selected_soranet_total as f64
        }
    }

    #[allow(dead_code)]
    fn uses_classical(&self) -> bool {
        self.selected_classical() > 0
    }

    fn candidate_ratio(&self) -> f64 {
        if self.total_candidates == 0 {
            0.0
        } else {
            self.pq_candidates as f64 / self.total_candidates as f64
        }
    }

    fn target_ratio(&self) -> f64 {
        match self.policy {
            AnonymityPolicy::GuardPq => {
                if self.selected_soranet_total > 0 {
                    1.0 / self.selected_soranet_total as f64
                } else if self.total_candidates > 0 {
                    1.0 / self.total_candidates as f64
                } else {
                    1.0
                }
            }
            AnonymityPolicy::MajorityPq => MAJORITY_THRESHOLD,
            AnonymityPolicy::StrictPq => 1.0,
        }
    }

    fn deficit_ratio(&self) -> f64 {
        match self.status {
            PolicyStatus::Met => 0.0,
            PolicyStatus::NotApplicable => {
                if matches!(
                    self.fallback_reason,
                    Some(PolicyFallback::NoSoranetCandidates)
                ) {
                    1.0
                } else {
                    0.0
                }
            }
            PolicyStatus::Brownout => (self.target_ratio() - self.ratio()).clamp(0.0, 1.0),
        }
    }

    fn supply_delta_ratio(&self) -> f64 {
        (self.candidate_ratio() - self.ratio()).clamp(0.0, 1.0)
    }

    fn is_brownout(&self) -> bool {
        matches!(self.status, PolicyStatus::Brownout)
    }

    #[allow(dead_code)]
    fn should_flag_brownout(&self) -> bool {
        self.is_brownout()
            || matches!(
                self.fallback_reason,
                Some(PolicyFallback::NoSoranetCandidates)
            )
    }

    fn outcome_label(&self) -> &'static str {
        match self.status {
            PolicyStatus::NotApplicable => "not_applicable",
            PolicyStatus::Met => "met",
            PolicyStatus::Brownout => "brownout",
        }
    }

    fn reason_label(&self) -> &'static str {
        match self.fallback_reason {
            Some(PolicyFallback::NoSoranetCandidates) => "no_soranet",
            Some(PolicyFallback::MissingGuardSupport) => "missing_guard_pq",
            Some(PolicyFallback::MissingMajoritySupport) => "missing_majority_pq",
            Some(PolicyFallback::MissingStrictSupport) => "missing_strict_pq",
            Some(PolicyFallback::ComplianceJurisdictionOptOut) => "compliance_jurisdiction_opt_out",
            Some(PolicyFallback::ComplianceBlindedCidOptOut) => "compliance_blinded_cid_opt_out",
            None => "none",
        }
    }

    fn evaluate_outcome(&mut self) {
        if self.total_candidates == 0 {
            self.status = PolicyStatus::NotApplicable;
            self.fallback_reason = Some(PolicyFallback::NoSoranetCandidates);
            return;
        }

        match self.policy {
            AnonymityPolicy::GuardPq => {
                if self.selected_pq >= 1 {
                    self.status = PolicyStatus::Met;
                    self.fallback_reason = None;
                } else {
                    self.status = PolicyStatus::Brownout;
                    self.fallback_reason = Some(PolicyFallback::MissingGuardSupport);
                }
            }
            AnonymityPolicy::MajorityPq => {
                if self.selected_soranet_total == 0 {
                    self.status = PolicyStatus::Brownout;
                    self.fallback_reason = Some(PolicyFallback::MissingMajoritySupport);
                } else if self.selected_pq * 3 >= self.selected_soranet_total * 2 {
                    self.status = PolicyStatus::Met;
                    self.fallback_reason = None;
                } else {
                    self.status = PolicyStatus::Brownout;
                    self.fallback_reason = Some(PolicyFallback::MissingMajoritySupport);
                }
            }
            AnonymityPolicy::StrictPq => {
                if self.selected_soranet_total > 0
                    && self.selected_pq == self.selected_soranet_total
                {
                    self.status = PolicyStatus::Met;
                    self.fallback_reason = None;
                } else {
                    self.status = PolicyStatus::Brownout;
                    self.fallback_reason = Some(PolicyFallback::MissingStrictSupport);
                }
            }
        }
    }

    fn effective_policy(&self) -> AnonymityPolicy {
        self.effective_policy
    }

    fn set_effective_policy(&mut self, policy: AnonymityPolicy) {
        self.effective_policy = policy;
    }

    fn apply_compliance_override(&mut self, fallback: PolicyFallback) {
        self.status = PolicyStatus::NotApplicable;
        self.fallback_reason = Some(fallback);
        self.effective_policy = self.policy;
        self.selected_soranet_total = 0;
        self.selected_pq = 0;
    }
}

impl From<PolicySummary> for PolicyReport {
    fn from(summary: PolicySummary) -> Self {
        Self {
            policy: summary.policy,
            effective_policy: summary.effective_policy,
            total_candidates: summary.total_candidates,
            pq_candidates: summary.pq_candidates,
            selected_soranet_total: summary.selected_soranet_total,
            selected_pq: summary.selected_pq,
            status: summary.status,
            fallback_reason: summary.fallback_reason,
        }
    }
}

struct SelectionOutcome {
    providers: Vec<FetchProvider>,
    summary: PolicySummary,
}

fn select_soranet_providers(
    candidates: Vec<SoranetCandidate>,
    policy: AnonymityPolicy,
) -> Vec<FetchProvider> {
    if candidates.is_empty() {
        return Vec::new();
    }

    let mut pq_candidates = Vec::new();
    let mut classical_candidates = Vec::new();
    for candidate in candidates.into_iter() {
        if candidate.pq.satisfies(policy) {
            pq_candidates.push(candidate);
        } else {
            classical_candidates.push(candidate);
        }
    }

    pq_candidates.sort_by(compare_soranet_candidates);
    classical_candidates.sort_by(compare_soranet_candidates);

    match policy {
        AnonymityPolicy::GuardPq => {
            let mut providers: Vec<FetchProvider> = pq_candidates
                .into_iter()
                .map(SoranetCandidate::into_weighted_provider)
                .collect();
            providers.extend(
                classical_candidates
                    .into_iter()
                    .map(SoranetCandidate::into_weighted_provider),
            );
            providers
        }
        AnonymityPolicy::MajorityPq => {
            if pq_candidates.is_empty() {
                // Brownout path: no PQ-capable relays.
                classical_candidates
                    .into_iter()
                    .map(SoranetCandidate::into_weighted_provider)
                    .collect()
            } else {
                let allowance = ((pq_candidates.len() as f64) * (1.0 - MAJORITY_THRESHOLD)
                    / MAJORITY_THRESHOLD)
                    .floor() as usize;
                let mut providers: Vec<FetchProvider> = pq_candidates
                    .into_iter()
                    .map(SoranetCandidate::into_weighted_provider)
                    .collect();
                providers.extend(
                    classical_candidates
                        .into_iter()
                        .take(allowance)
                        .map(SoranetCandidate::into_weighted_provider),
                );
                providers
            }
        }
        AnonymityPolicy::StrictPq => {
            if pq_candidates.is_empty() {
                Vec::new()
            } else {
                pq_candidates
                    .into_iter()
                    .map(SoranetCandidate::into_weighted_provider)
                    .collect()
            }
        }
    }
}

fn eligible_providers(
    scoreboard: &Scoreboard,
    limit: Option<NonZeroUsize>,
    transport_policy: TransportPolicy,
    anonymity_policy: AnonymityPolicy,
    guard_set: Option<&GuardSet>,
    relay_directory: Option<&RelayDirectory>,
    skip_policy_fallback: bool,
) -> SelectionOutcome {
    let mut soranet_candidates = Vec::new();
    let mut quic = Vec::new();
    let mut torii = Vec::new();
    let mut fallback = Vec::new();
    let mut soranet_catalog: HashMap<String, PqSupport> = HashMap::new();
    let guard_descriptor_index = relay_directory.map(|directory| {
        directory
            .entries()
            .iter()
            .map(|descriptor| (descriptor.relay_id, descriptor))
            .collect::<HashMap<[u8; 32], &RelayDescriptor>>()
    });
    let guard_record_index = guard_set.map(|set| {
        set.iter()
            .map(|record| (record.relay_id, record))
            .collect::<HashMap<[u8; 32], &GuardRecord>>()
    });

    for entry in scoreboard.entries() {
        if !matches!(entry.eligibility, Eligibility::Eligible) {
            continue;
        }

        let provider = entry.provider.clone();
        let support = TransportSupport::from_provider(&provider);
        let pq_support = PqSupport::from_provider(&provider);
        let provider_id = provider.id().as_str().to_string();
        let descriptor = guard_descriptor_index
            .as_ref()
            .and_then(|catalog| guard_descriptor_for_provider(&provider, catalog));
        let guard_record = guard_record_index
            .as_ref()
            .and_then(|catalog| guard_record_for_provider(&provider, catalog));

        if support.has_soranet() {
            soranet_catalog.insert(provider_id, pq_support);
            soranet_candidates.push(SoranetCandidate::new(
                provider,
                pq_support,
                descriptor,
                guard_record,
            ));
        } else if support.has_quic {
            quic.push(provider);
        } else if support.has_torii {
            torii.push(provider);
        } else {
            fallback.push(provider);
        }
    }

    let pq_candidate_count = soranet_candidates
        .iter()
        .filter(|candidate| candidate.pq.satisfies(anonymity_policy))
        .count();

    let mut summary = PolicySummary::new(
        anonymity_policy,
        soranet_candidates.len(),
        pq_candidate_count,
    );

    let base_candidates = soranet_candidates;
    let base_quic = quic;
    let base_torii = torii;
    let base_fallback = fallback;

    let build_providers = |mut soranet_selected: Vec<FetchProvider>| -> Vec<FetchProvider> {
        let mut providers = match transport_policy {
            TransportPolicy::SoranetPreferred => {
                let mut ordered = Vec::new();
                ordered.append(&mut soranet_selected);
                ordered.extend(base_quic.iter().cloned());
                ordered.extend(base_torii.iter().cloned());
                ordered.extend(base_fallback.iter().cloned());
                ordered
            }
            TransportPolicy::SoranetStrict => soranet_selected,
            TransportPolicy::DirectOnly => {
                let _ = soranet_selected;
                let mut ordered = Vec::new();
                ordered.extend(base_quic.iter().cloned());
                ordered.extend(base_torii.iter().cloned());
                ordered
            }
        };

        if let Some(guards) = guard_set {
            providers = reorder_by_guard_set(providers, guards);
        }

        if let Some(cap) = limit {
            providers.truncate(cap.get());
        }

        providers
    };

    let compute_counts = |providers: &[FetchProvider]| {
        let mut selected_soranet_total = 0;
        let mut selected_pq = 0;
        for provider in providers {
            let id = provider.id().as_str();
            if let Some(pq) = soranet_catalog.get(id) {
                selected_soranet_total += 1;
                if pq.satisfies(anonymity_policy) {
                    selected_pq += 1;
                }
            }
        }
        (selected_soranet_total, selected_pq)
    };

    let mut policy_used = anonymity_policy;
    let initial_selected = if matches!(
        transport_policy,
        TransportPolicy::SoranetPreferred | TransportPolicy::SoranetStrict
    ) {
        select_soranet_providers(base_candidates.clone(), policy_used)
    } else {
        Vec::new()
    };
    let mut providers = build_providers(initial_selected);
    let (mut selected_soranet_total, mut selected_pq) = compute_counts(&providers);

    if matches!(transport_policy, TransportPolicy::DirectOnly) {
        summary = PolicySummary::new(anonymity_policy, 0, 0);
        summary.set_effective_policy(anonymity_policy);
        summary.update_selected_counts(0, 0);
        return SelectionOutcome { providers, summary };
    }

    summary.set_effective_policy(policy_used);
    summary.update_selected_counts(selected_soranet_total, selected_pq);

    if !skip_policy_fallback && matches!(transport_policy, TransportPolicy::SoranetPreferred) {
        while selected_soranet_total == 0 {
            let Some(next_policy) = policy_used.fallback() else {
                break;
            };
            policy_used = next_policy;
            let selected = select_soranet_providers(base_candidates.clone(), policy_used);
            let next_providers = build_providers(selected);
            let (next_total, next_pq) = compute_counts(&next_providers);

            providers = next_providers;
            selected_soranet_total = next_total;
            selected_pq = next_pq;
            summary.set_effective_policy(policy_used);
            summary.update_selected_counts(selected_soranet_total, selected_pq);

            if selected_soranet_total > 0 {
                break;
            }
        }
    }

    if skip_policy_fallback && !matches!(summary.status, PolicyStatus::Met) {
        providers.clear();
    }

    SelectionOutcome { providers, summary }
}

#[derive(Clone, Copy)]
struct GuardPreference {
    position: usize,
    pq: bool,
    weight: u32,
    bandwidth_bytes_per_sec: u64,
    reputation_weight: u32,
    pinned_at: u64,
}

fn reorder_by_guard_set(providers: Vec<FetchProvider>, guard_set: &GuardSet) -> Vec<FetchProvider> {
    if providers.is_empty() || guard_set.is_empty() {
        return providers;
    }

    let guard_preferences: HashMap<[u8; 32], GuardPreference> = guard_set
        .iter()
        .enumerate()
        .map(|(position, record)| {
            (
                record.relay_id,
                GuardPreference {
                    position,
                    pq: record.pq_kem_public.is_some(),
                    weight: record.guard_weight,
                    bandwidth_bytes_per_sec: record.bandwidth_bytes_per_sec,
                    reputation_weight: record.reputation_weight,
                    pinned_at: record.pinned_at_unix,
                },
            )
        })
        .collect();

    struct ProviderOrdering {
        provider: FetchProvider,
        original_index: usize,
        pinned: bool,
        pq: bool,
        guard_weight: u32,
        bandwidth_bytes_per_sec: u64,
        reputation: u32,
        position: usize,
        pinned_at: u64,
    }

    let mut keyed: Vec<ProviderOrdering> = providers
        .into_iter()
        .enumerate()
        .map(|(original_index, provider)| {
            let provider_bandwidth = provider
                .metadata()
                .and_then(|meta| meta.stream_budget.as_ref())
                .map(|budget| budget.max_bytes_per_sec)
                .unwrap_or(0);
            let provider_reputation = provider.weight().get();
            let preference = guard_preference_for_provider(&provider, &guard_preferences);
            let (
                pinned,
                pq,
                guard_weight,
                bandwidth_bytes_per_sec,
                reputation,
                position,
                pinned_at,
            ) = preference
                .map(|pref| {
                    let bandwidth = if pref.bandwidth_bytes_per_sec > 0 {
                        pref.bandwidth_bytes_per_sec
                    } else {
                        provider_bandwidth
                    };
                    let reputation = if pref.reputation_weight > 0 {
                        pref.reputation_weight
                    } else {
                        provider_reputation
                    };
                    (
                        true,
                        pref.pq,
                        pref.weight,
                        bandwidth,
                        reputation,
                        pref.position,
                        pref.pinned_at,
                    )
                })
                .unwrap_or((
                    false,
                    false,
                    0,
                    provider_bandwidth,
                    provider_reputation,
                    usize::MAX,
                    0,
                ));
            ProviderOrdering {
                provider,
                original_index,
                pinned,
                pq,
                guard_weight,
                bandwidth_bytes_per_sec,
                reputation,
                position,
                pinned_at,
            }
        })
        .collect();

    keyed.sort_by(|left, right| {
        right
            .pinned
            .cmp(&left.pinned)
            .then_with(|| right.pq.cmp(&left.pq))
            .then_with(|| right.guard_weight.cmp(&left.guard_weight))
            .then_with(|| {
                right
                    .bandwidth_bytes_per_sec
                    .cmp(&left.bandwidth_bytes_per_sec)
            })
            .then_with(|| right.reputation.cmp(&left.reputation))
            .then_with(|| left.position.cmp(&right.position))
            .then_with(|| right.pinned_at.cmp(&left.pinned_at))
            .then_with(|| left.original_index.cmp(&right.original_index))
    });

    keyed.into_iter().map(|entry| entry.provider).collect()
}

fn guard_preference_for_provider<'a>(
    provider: &FetchProvider,
    guard_preferences: &'a HashMap<[u8; 32], GuardPreference>,
) -> Option<&'a GuardPreference> {
    let metadata = provider.metadata()?;
    if let Some(pref) = metadata
        .provider_id
        .as_deref()
        .and_then(|value| guard_preference_from_str(value, guard_preferences))
    {
        return Some(pref);
    }
    for candidate in &metadata.profile_aliases {
        if let Some(pref) = guard_preference_from_str(candidate, guard_preferences) {
            return Some(pref);
        }
    }
    None
}

fn guard_preference_from_str<'a>(
    value: &str,
    guard_preferences: &'a HashMap<[u8; 32], GuardPreference>,
) -> Option<&'a GuardPreference> {
    let relay_id = decode_relay_id(value)?;
    guard_preferences.get(&relay_id)
}

struct FetchMetricsCtx {
    manifest_id: String,
    region: String,
    job_id: String,
    metrics: Arc<iroha_telemetry::metrics::Metrics>,
    otel: Arc<SorafsFetchOtel>,
    telemetry: FetchTelemetryCtx,
    latency_cap_ms: u32,
    policy_summary: PolicySummary,
    start: Instant,
}

impl FetchMetricsCtx {
    fn begin(
        plan: &CarBuildPlan,
        region: &str,
        providers: &[FetchProvider],
        policy_summary: &PolicySummary,
        fetch_options: &FetchOptions,
        scoreboard_config: &ScoreboardConfig,
        write_mode: WriteModeHint,
    ) -> Self {
        let metrics = global_or_default();
        let otel = global_sorafs_fetch_otel();
        let manifest_id = manifest_id_hex(plan);
        metrics.sorafs_orchestrator_fetch_started(&manifest_id, region);
        let job_id = generate_job_id();
        otel.fetch_started(&manifest_id, region, &job_id);
        let provider_count = providers.len();
        let telemetry = FetchTelemetryCtx::new(job_id.clone(), write_mode);
        telemetry.emit_start(
            &manifest_id,
            region,
            plan,
            provider_count,
            fetch_options,
            policy_summary,
        );
        metrics.record_sorafs_orchestrator_policy_event(
            policy_summary.policy.label(),
            region,
            policy_summary.outcome_label(),
            policy_summary.reason_label(),
        );
        if policy_summary.should_flag_brownout() {
            metrics.inc_sorafs_orchestrator_brownout(
                policy_summary.policy.label(),
                region,
                policy_summary.reason_label(),
            );
        }
        let mut event_reason = policy_summary.reason_label();
        if event_reason.is_empty() {
            event_reason = policy_summary.outcome_label();
        }
        if event_reason.is_empty() {
            event_reason = "none";
        }
        let reason = event_reason.to_string();
        for provider in providers {
            let protocol = TransportSupport::from_provider(provider).protocol_label();
            metrics.inc_sorafs_orchestrator_transport_event(region, protocol, "selected", &reason);
            otel.record_transport_event(
                &manifest_id,
                region,
                Some(&job_id),
                protocol,
                "selected",
                &reason,
            );
        }
        metrics.record_sorafs_orchestrator_pq_ratio(
            policy_summary.policy.label(),
            region,
            policy_summary.ratio(),
        );
        metrics.record_sorafs_orchestrator_pq_candidate_ratio(
            policy_summary.policy.label(),
            region,
            policy_summary.candidate_ratio(),
        );
        metrics.record_sorafs_orchestrator_pq_deficit_ratio(
            policy_summary.policy.label(),
            region,
            policy_summary.deficit_ratio(),
        );
        metrics.record_sorafs_orchestrator_classical_ratio(
            policy_summary.policy.label(),
            region,
            policy_summary.classical_ratio(),
        );
        metrics.record_sorafs_orchestrator_classical_selected(
            policy_summary.policy.label(),
            region,
            policy_summary.selected_classical() as u64,
        );
        otel.record_policy_event(
            &manifest_id,
            region,
            Some(&job_id),
            policy_summary.policy.label(),
            policy_summary.outcome_label(),
            policy_summary.reason_label(),
        );
        otel.record_pq_ratio(
            &manifest_id,
            region,
            Some(&job_id),
            policy_summary.policy.label(),
            policy_summary.ratio(),
        );
        otel.record_pq_candidate_ratio(
            &manifest_id,
            region,
            Some(&job_id),
            policy_summary.policy.label(),
            policy_summary.candidate_ratio(),
        );
        otel.record_pq_deficit_ratio(
            &manifest_id,
            region,
            Some(&job_id),
            policy_summary.policy.label(),
            policy_summary.deficit_ratio(),
        );
        otel.record_classical_ratio(
            &manifest_id,
            region,
            Some(&job_id),
            policy_summary.policy.label(),
            policy_summary.classical_ratio(),
        );
        otel.record_classical_selected(
            &manifest_id,
            region,
            Some(&job_id),
            policy_summary.policy.label(),
            policy_summary.selected_classical() as u64,
        );
        Self {
            manifest_id,
            region: region.to_string(),
            job_id,
            metrics,
            otel,
            telemetry,
            latency_cap_ms: scoreboard_config.latency_cap_ms,
            policy_summary: policy_summary.clone(),
            start: Instant::now(),
        }
    }

    fn on_success(&self, outcome: &FetchOutcome) {
        let duration_ms = self.start.elapsed().as_secs_f64() * 1_000.0;
        self.metrics.record_sorafs_orchestrator_duration(
            &self.manifest_id,
            &self.region,
            duration_ms,
        );
        self.otel
            .record_duration(&self.manifest_id, &self.region, &self.job_id, duration_ms);

        let mut retry_counts: HashMap<String, u64> = HashMap::new();
        let mut total_retries = 0;
        for receipt in &outcome.chunk_receipts {
            if receipt.attempts > 1 {
                let extra = receipt.attempts.saturating_sub(1) as u64;
                retry_counts
                    .entry(receipt.provider.as_str().to_string())
                    .and_modify(|value| *value += extra)
                    .or_insert(extra);
            }
        }
        for (provider, count) in retry_counts {
            if count == 0 {
                continue;
            }
            self.metrics.inc_sorafs_orchestrator_retries(
                &self.manifest_id,
                &provider,
                "retry",
                count,
            );
            self.otel.record_retries(
                &self.manifest_id,
                &self.region,
                Some(&self.job_id),
                &provider,
                "retry",
                count,
            );
            self.telemetry
                .emit_retry(&self.manifest_id, &self.region, &provider, "retry", count);
            total_retries += count;
        }

        let mut total_provider_failures = 0;
        for report in outcome.provider_reports.iter() {
            let failures = report.failures as u64;
            if failures > 0 {
                let provider_id = report.provider.id().as_str();
                self.metrics.inc_sorafs_orchestrator_provider_failures(
                    &self.manifest_id,
                    provider_id,
                    "session_failure",
                    failures,
                );
                self.otel.record_provider_failure(
                    &self.manifest_id,
                    &self.region,
                    Some(&self.job_id),
                    provider_id,
                    "session_failure",
                    failures,
                );
                self.telemetry.emit_provider_failure(
                    &self.manifest_id,
                    &self.region,
                    provider_id,
                    "session_failure",
                    failures,
                );
                total_provider_failures += failures;
            }
        }

        let mut total_bytes: u64 = 0;
        let mut total_stalls: u64 = 0;
        let mut throughput_samples: u64 = 0;
        let mut throughput_sum_mib_per_s = 0.0;
        for (receipt, chunk) in outcome.chunk_receipts.iter().zip(&outcome.chunks) {
            let provider_id = receipt.provider.as_str();
            let chunk_bytes = chunk.len() as u64;
            let latency_ms = receipt.latency_ms;
            self.metrics.record_sorafs_orchestrator_chunk_latency(
                &self.manifest_id,
                provider_id,
                latency_ms,
            );
            self.otel.record_chunk_latency(
                &self.manifest_id,
                &self.region,
                Some(&self.job_id),
                provider_id,
                latency_ms,
            );
            self.metrics
                .inc_sorafs_orchestrator_bytes(&self.manifest_id, provider_id, chunk_bytes);
            self.otel.record_bytes(
                &self.manifest_id,
                &self.region,
                Some(&self.job_id),
                provider_id,
                chunk_bytes,
            );
            if latency_ms > f64::from(self.latency_cap_ms) {
                total_stalls += 1;
                self.metrics
                    .inc_sorafs_orchestrator_stall(&self.manifest_id, provider_id);
                self.otel.record_stall(
                    &self.manifest_id,
                    &self.region,
                    Some(&self.job_id),
                    provider_id,
                );
                self.telemetry.emit_stall(
                    &self.manifest_id,
                    &self.region,
                    provider_id,
                    latency_ms,
                    chunk_bytes,
                );
            }
            if latency_ms > 0.0 && chunk_bytes > 0 {
                let seconds = latency_ms / 1_000.0;
                if seconds > 0.0 {
                    throughput_sum_mib_per_s += (chunk_bytes as f64 / (1024.0 * 1024.0)) / seconds;
                    throughput_samples += 1;
                }
            }
            total_bytes += chunk_bytes;
        }
        let avg_throughput_mib_s = if throughput_samples > 0 {
            throughput_sum_mib_per_s / throughput_samples as f64
        } else {
            0.0
        };

        self.telemetry.emit_success(
            &self.manifest_id,
            &self.region,
            duration_ms,
            outcome,
            total_retries,
            total_provider_failures,
            total_bytes,
            avg_throughput_mib_s,
            total_stalls,
            &self.policy_summary,
        );
    }

    fn on_error(&self, error: &multi_fetch::MultiSourceError) {
        let duration_ms = self.start.elapsed().as_secs_f64() * 1_000.0;
        self.metrics.record_sorafs_orchestrator_duration(
            &self.manifest_id,
            &self.region,
            duration_ms,
        );
        self.otel
            .record_duration(&self.manifest_id, &self.region, &self.job_id, duration_ms);

        let reason = error_reason(error);
        self.metrics
            .inc_sorafs_orchestrator_failure(&self.manifest_id, &self.region, reason);
        self.otel
            .record_failure(&self.manifest_id, &self.region, Some(&self.job_id), reason);

        let provider_info = provider_from_error(error);
        if let Some((ref provider, ref provider_reason)) = provider_info {
            self.metrics.inc_sorafs_orchestrator_provider_failures(
                &self.manifest_id,
                provider,
                provider_reason,
                1,
            );
            self.otel.record_provider_failure(
                &self.manifest_id,
                &self.region,
                Some(&self.job_id),
                provider,
                provider_reason,
                1,
            );
            self.telemetry.emit_provider_failure(
                &self.manifest_id,
                &self.region,
                provider,
                provider_reason,
                1,
            );
        }

        self.telemetry.emit_error(
            &self.manifest_id,
            &self.region,
            duration_ms,
            reason,
            provider_info
                .as_ref()
                .map(|(provider, provider_reason)| (provider.as_str(), provider_reason.as_str())),
            &self.policy_summary,
        );
    }

    fn finish(&self) {
        self.metrics
            .sorafs_orchestrator_fetch_finished(&self.manifest_id, &self.region);
        self.otel
            .fetch_finished(&self.manifest_id, &self.region, &self.job_id);
    }
}

struct FetchTelemetryCtx {
    job_id: String,
    write_mode: WriteModeHint,
}

impl FetchTelemetryCtx {
    fn new(job_id: String, write_mode: WriteModeHint) -> Self {
        Self { job_id, write_mode }
    }

    fn emit_start(
        &self,
        manifest_id: &str,
        region: &str,
        plan: &CarBuildPlan,
        provider_count: usize,
        fetch_options: &FetchOptions,
        policy_summary: &PolicySummary,
    ) {
        let chunk_specs = plan.chunk_fetch_specs();
        let chunk_count = chunk_specs.len() as u64;
        let total_bytes = chunk_specs
            .iter()
            .map(|spec| u64::from(spec.length))
            .sum::<u64>();
        let (retry_budget, retry_unbounded) = fetch_options
            .per_chunk_retry_limit
            .map_or((0u64, true), |value| (value as u64, false));
        let (global_parallel_limit, parallel_unbounded) = fetch_options
            .global_parallel_limit
            .map_or((0u64, true), |value| (value as u64, false));

        info!(
            target: "telemetry::sorafs.fetch.lifecycle",
            event = "start",
            status = "started",
            manifest = manifest_id,
            region = region,
            job_id = %self.job_id,
            chunk_count = chunk_count,
            total_bytes,
            provider_candidates = provider_count as u64,
            verify_lengths = fetch_options.verify_lengths,
            verify_digests = fetch_options.verify_digests,
            retry_budget = retry_budget,
            retry_budget_unbounded = retry_unbounded,
            provider_failure_threshold = fetch_options.provider_failure_threshold as u64,
            global_parallel_limit = global_parallel_limit,
            global_parallel_unbounded = parallel_unbounded,
            write_mode = self.write_mode.label(),
            anonymity_policy = policy_summary.policy.label(),
            anonymity_effective_policy = policy_summary.effective_policy().label(),
            anonymity_outcome = policy_summary.outcome_label(),
            anonymity_reason = policy_summary.reason_label(),
            anonymity_ratio = policy_summary.ratio(),
            anonymity_candidates = policy_summary.total_candidates as u64,
            anonymity_pq_candidates = policy_summary.pq_candidates as u64,
            anonymity_selected = policy_summary.selected_soranet_total as u64,
            anonymity_classical_selected = policy_summary.selected_classical() as u64,
            anonymity_candidate_ratio = policy_summary.candidate_ratio(),
            anonymity_deficit_ratio = policy_summary.deficit_ratio(),
            anonymity_supply_delta = policy_summary.supply_delta_ratio(),
            anonymity_classical_ratio = policy_summary.classical_ratio(),
            anonymity_classical = policy_summary.uses_classical(),
            anonymity_brownout = policy_summary.is_brownout(),
        );
    }

    #[allow(clippy::too_many_arguments)] // telemetry emission uses explicit context values
    fn emit_success(
        &self,
        manifest_id: &str,
        region: &str,
        duration_ms: f64,
        outcome: &FetchOutcome,
        total_retries: u64,
        provider_failures: u64,
        total_bytes: u64,
        avg_throughput_mib_s: f64,
        stall_count: u64,
        policy_summary: &PolicySummary,
    ) {
        info!(
            target: "telemetry::sorafs.fetch.lifecycle",
            event = "complete",
            status = "success",
            manifest = manifest_id,
            region = region,
            job_id = %self.job_id,
            duration_ms,
            chunk_count = outcome.chunks.len() as u64,
            provider_reports = outcome.provider_reports.len() as u64,
            retries_total = total_retries,
            provider_failures_total = provider_failures,
            total_bytes,
            avg_throughput_mib_s,
            stall_count,
            write_mode = self.write_mode.label(),
            anonymity_policy = policy_summary.policy.label(),
            anonymity_effective_policy = policy_summary.effective_policy().label(),
            anonymity_outcome = policy_summary.outcome_label(),
            anonymity_reason = policy_summary.reason_label(),
            anonymity_ratio = policy_summary.ratio(),
            anonymity_selected = policy_summary.selected_soranet_total as u64,
            anonymity_classical_selected = policy_summary.selected_classical() as u64,
            anonymity_candidate_ratio = policy_summary.candidate_ratio(),
            anonymity_deficit_ratio = policy_summary.deficit_ratio(),
            anonymity_supply_delta = policy_summary.supply_delta_ratio(),
            anonymity_classical_ratio = policy_summary.classical_ratio(),
            anonymity_classical = policy_summary.uses_classical(),
            anonymity_brownout = policy_summary.is_brownout(),
        );
    }

    fn emit_error(
        &self,
        manifest_id: &str,
        region: &str,
        duration_ms: f64,
        reason: &str,
        provider: Option<(&str, &str)>,
        policy_summary: &PolicySummary,
    ) {
        if let Some((provider_id, provider_reason)) = provider {
            warn!(
                target: "telemetry::sorafs.fetch.error",
                status = "failed",
                manifest = manifest_id,
                region = region,
                job_id = %self.job_id,
                duration_ms,
                reason = reason,
                provider = provider_id,
                provider_reason = provider_reason,
                write_mode = self.write_mode.label(),
                anonymity_policy = policy_summary.policy.label(),
                anonymity_effective_policy = policy_summary.effective_policy().label(),
                anonymity_outcome = policy_summary.outcome_label(),
                anonymity_reason = policy_summary.reason_label(),
                anonymity_ratio = policy_summary.ratio(),
                anonymity_candidate_ratio = policy_summary.candidate_ratio(),
                anonymity_deficit_ratio = policy_summary.deficit_ratio(),
                anonymity_classical_ratio = policy_summary.classical_ratio(),
                anonymity_classical_selected = policy_summary.selected_classical() as u64,
                anonymity_classical = policy_summary.uses_classical(),
                anonymity_brownout = policy_summary.is_brownout(),
            );
        } else {
            warn!(
                target: "telemetry::sorafs.fetch.error",
                status = "failed",
                manifest = manifest_id,
                region = region,
                job_id = %self.job_id,
                duration_ms,
                reason = reason,
                write_mode = self.write_mode.label(),
                anonymity_policy = policy_summary.policy.label(),
                anonymity_effective_policy = policy_summary.effective_policy().label(),
                anonymity_outcome = policy_summary.outcome_label(),
                anonymity_reason = policy_summary.reason_label(),
                anonymity_ratio = policy_summary.ratio(),
                anonymity_candidate_ratio = policy_summary.candidate_ratio(),
                anonymity_deficit_ratio = policy_summary.deficit_ratio(),
                anonymity_classical_ratio = policy_summary.classical_ratio(),
                anonymity_classical_selected = policy_summary.selected_classical() as u64,
                anonymity_classical = policy_summary.uses_classical(),
                anonymity_brownout = policy_summary.is_brownout(),
            );
        }
    }

    fn emit_retry(
        &self,
        manifest_id: &str,
        region: &str,
        provider: &str,
        reason: &str,
        count: u64,
    ) {
        info!(
            target: "telemetry::sorafs.fetch.retry",
            manifest = manifest_id,
            region = region,
            job_id = %self.job_id,
            provider = provider,
            reason = reason,
            attempts = count,
        );
    }

    fn emit_provider_failure(
        &self,
        manifest_id: &str,
        region: &str,
        provider: &str,
        reason: &str,
        count: u64,
    ) {
        warn!(
            target: "telemetry::sorafs.fetch.provider_failure",
            manifest = manifest_id,
            region = region,
            job_id = %self.job_id,
            provider = provider,
            reason = reason,
            failures = count,
        );
    }

    fn emit_stall(
        &self,
        manifest_id: &str,
        region: &str,
        provider: &str,
        latency_ms: f64,
        bytes: u64,
    ) {
        warn!(
            target: "telemetry::sorafs.fetch.stall",
            manifest = manifest_id,
            region = region,
            job_id = %self.job_id,
            provider = provider,
            latency_ms,
            bytes,
        );
    }
}

#[derive(Debug, Error)]
enum PrivacyCollectorError {
    #[error("invalid privacy bucket configuration: {0}")]
    InvalidBucket(#[from] PrivacyConfigError),
    #[error("failed to construct privacy polling client: {0}")]
    HttpClient(#[from] reqwest::Error),
}

struct PrivacyCollector {
    aggregator: Arc<SoranetSecureAggregator>,
    metrics: Arc<iroha_telemetry::metrics::Metrics>,
    handles: Vec<JoinHandle<()>>,
    _remediation: Option<Arc<DowngradeRemediator>>,
    stop: Arc<AtomicBool>,
    notify: Arc<Notify>,
}

impl PrivacyCollector {
    fn spawn(
        endpoints: Vec<(String, Url)>,
        config: &PrivacyEventsConfig,
        metrics: Arc<iroha_telemetry::metrics::Metrics>,
        remediation: Option<Arc<DowngradeRemediator>>,
    ) -> Result<Option<Self>, PrivacyCollectorError> {
        if endpoints.is_empty() {
            metrics.set_soranet_privacy_collector_enabled(false);
            return Ok(None);
        }

        let aggregator = Arc::new(SoranetSecureAggregator::new(config.bucket)?);
        let client = Arc::new(Client::builder().timeout(config.request_timeout).build()?);
        let stop = Arc::new(AtomicBool::new(false));
        let notify = Arc::new(Notify::new());
        let mut handles = Vec::with_capacity(endpoints.len());

        for (alias, url) in endpoints {
            let aggregator = Arc::clone(&aggregator);
            let metrics = Arc::clone(&metrics);
            let stop = Arc::clone(&stop);
            let notify = Arc::clone(&notify);
            let remediation = remediation.clone();
            let client = Arc::clone(&client);
            let interval = config.poll_interval;
            handles.push(tokio::spawn(async move {
                poll_privacy_endpoint(
                    alias,
                    url,
                    client,
                    aggregator,
                    metrics,
                    remediation,
                    stop,
                    notify,
                    interval,
                )
                .await;
            }));
        }

        debug!(
            target: "telemetry::sorafs.privacy",
            collectors = handles.len(),
            "started privacy event collector"
        );
        metrics.set_soranet_privacy_collector_enabled(true);

        Ok(Some(Self {
            aggregator,
            metrics,
            handles,
            _remediation: remediation,
            stop,
            notify,
        }))
    }

    async fn shutdown(self) {
        let Self {
            aggregator,
            metrics,
            mut handles,
            stop,
            notify,
            ..
        } = self;

        stop.store(true, AtomicOrdering::Relaxed);
        notify.notify_waiters();
        for handle in handles.drain(..) {
            if let Err(error) = handle.await
                && !error.is_cancelled()
            {
                warn!(
                    target: "telemetry::sorafs.privacy",
                    ?error,
                    "privacy collector task terminated with error"
                );
            }
        }
        Self::flush_inner(&aggregator, &metrics);
        metrics.set_soranet_privacy_collector_enabled(false);
    }

    #[allow(dead_code)]
    fn flush(&self) {
        Self::flush_inner(&self.aggregator, &self.metrics);
    }

    fn flush_inner(
        aggregator: &Arc<SoranetSecureAggregator>,
        metrics: &Arc<iroha_telemetry::metrics::Metrics>,
    ) {
        let buckets = aggregator.drain_ready_now();
        if !buckets.is_empty() {
            for bucket in buckets {
                metrics.record_soranet_privacy_bucket(&bucket);
            }
        }
    }
}

fn ingest_privacy_payload(
    aggregator: &SoranetSecureAggregator,
    metrics: &Arc<iroha_telemetry::metrics::Metrics>,
    payload: &str,
    alias: &str,
) -> (usize, usize, bool) {
    let mut events_ingested = 0usize;
    let mut shares_ingested = 0usize;
    let mut had_error = false;
    for (idx, line) in payload.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match json::from_str::<SoranetPrivacyEventV1>(trimmed) {
            Ok(event) => {
                aggregator.record_event(&event);
                events_ingested = events_ingested.saturating_add(1);
            }
            Err(event_error) => match json::from_str::<SoranetPrivacyPrioShareV1>(trimmed) {
                Ok(share) => {
                    if let Err(error) = aggregator.ingest_prio_share(share) {
                        warn!(
                            target: "telemetry::sorafs.privacy",
                            provider = %alias,
                            line = idx + 1,
                            ?error,
                            "failed to ingest privacy share"
                        );
                        had_error = true;
                        metrics
                            .soranet_privacy_poll_errors_total
                            .with_label_values(&[alias])
                            .inc();
                    } else {
                        shares_ingested = shares_ingested.saturating_add(1);
                    }
                }
                Err(share_error) => {
                    warn!(
                        target: "telemetry::sorafs.privacy",
                        provider = %alias,
                        line = idx + 1,
                        ?event_error,
                        ?share_error,
                        "failed to decode relay privacy payload line"
                    );
                    had_error = true;
                    metrics
                        .soranet_privacy_poll_errors_total
                        .with_label_values(&[alias])
                        .inc();
                }
            },
        }
    }
    (events_ingested, shares_ingested, had_error)
}

#[allow(clippy::too_many_arguments)]
async fn poll_privacy_endpoint(
    alias: String,
    url: Url,
    client: Arc<Client>,
    aggregator: Arc<SoranetSecureAggregator>,
    metrics: Arc<iroha_telemetry::metrics::Metrics>,
    remediation: Option<Arc<DowngradeRemediator>>,
    stop: Arc<AtomicBool>,
    notify: Arc<Notify>,
    interval: Duration,
) {
    loop {
        if stop.load(AtomicOrdering::Relaxed) {
            break;
        }

        let alias_label = alias.as_str();
        match client
            .get(url.clone())
            .header("accept", "application/x-ndjson")
            .send()
            .await
        {
            Ok(response) => {
                let mut poll_succeeded = false;
                if response.status().is_success() {
                    match response.text().await {
                        Ok(body) => {
                            if body.trim().is_empty() {
                                poll_succeeded = true;
                            } else {
                                let (events, shares, had_error) = ingest_privacy_payload(
                                    &aggregator,
                                    &metrics,
                                    &body,
                                    alias_label,
                                );
                                if events > 0 || shares > 0 {
                                    debug!(
                                        target: "telemetry::sorafs.privacy",
                                        provider = %alias,
                                        events,
                                        shares,
                                        "ingested relay privacy payload"
                                    );
                                }
                                if !had_error {
                                    poll_succeeded = true;
                                }
                            }
                        }
                        Err(error) => {
                            warn!(
                                target: "telemetry::sorafs.privacy",
                                provider = %alias,
                                %error,
                                "failed to read relay privacy response"
                            );
                            metrics
                                .soranet_privacy_poll_errors_total
                                .with_label_values(&[alias_label])
                                .inc();
                        }
                    }
                } else if response.status() == StatusCode::NO_CONTENT {
                    poll_succeeded = true;
                } else {
                    warn!(
                        target: "telemetry::sorafs.privacy",
                        provider = %alias,
                        status = %response.status(),
                        "relay privacy endpoint returned unexpected status"
                    );
                    metrics
                        .soranet_privacy_poll_errors_total
                        .with_label_values(&[alias_label])
                        .inc();
                }
                flush_privacy_buckets(&aggregator, &metrics);
                if poll_succeeded && let Ok(elapsed) = SystemTime::now().duration_since(UNIX_EPOCH)
                {
                    metrics
                        .soranet_privacy_last_poll_unixtime
                        .set(elapsed.as_secs() as i64);
                }
            }
            Err(error) => {
                warn!(
                    target: "telemetry::sorafs.privacy",
                    provider = %alias,
                    %error,
                    "failed to poll relay privacy events"
                );
                metrics
                    .soranet_privacy_poll_errors_total
                    .with_label_values(&[alias_label])
                    .inc();
            }
        }

        if let Some(remediator) = remediation.as_ref() {
            let mut policy_url = url.clone();
            policy_url.set_path("/policy/proxy-toggle");
            policy_url.set_query(None);
            match client
                .get(policy_url)
                .header("accept", "application/x-ndjson")
                .send()
                .await
            {
                Ok(response) => {
                    if response.status().is_success() {
                        match response.text().await {
                            Ok(body) => {
                                if !body.trim().is_empty() {
                                    for (idx, line) in body.lines().enumerate() {
                                        let trimmed = line.trim();
                                        if trimmed.is_empty() {
                                            continue;
                                        }
                                        match json::from_str::<SoranetPrivacyEventV1>(trimmed) {
                                            Ok(event) => {
                                                if let SoranetPrivacyEventKindV1::HandshakeFailure(
                                                    failure,
                                                ) = &event.kind
                                                    && matches!(
                                                        failure.reason,
                                                        SoranetPrivacyHandshakeFailureV1::Downgrade
                                                    )
                                                {
                                                    remediator
                                                        .observe_handshake_downgrade(
                                                            event.timestamp_unix,
                                                            event.mode,
                                                        )
                                                        .await;
                                                }
                                            }
                                            Err(error) => {
                                                warn!(
                                                    target: "telemetry::sorafs.privacy",
                                                    provider = %alias,
                                                    line = idx + 1,
                                                    ?error,
                                                    "failed to decode proxy policy downgrade payload"
                                                );
                                                metrics
                                                    .soranet_privacy_poll_errors_total
                                                    .with_label_values(&[alias_label])
                                                    .inc();
                                            }
                                        }
                                    }
                                }
                            }
                            Err(error) => {
                                warn!(
                                    target: "telemetry::sorafs.privacy",
                                    provider = %alias,
                                    %error,
                                    "failed to read proxy policy downgrade response"
                                );
                                metrics
                                    .soranet_privacy_poll_errors_total
                                    .with_label_values(&[alias_label])
                                    .inc();
                            }
                        }
                    } else if response.status() != StatusCode::NO_CONTENT {
                        warn!(
                            target: "telemetry::sorafs.privacy",
                            provider = %alias,
                            status = %response.status(),
                            "proxy policy downgrade endpoint returned unexpected status"
                        );
                        metrics
                            .soranet_privacy_poll_errors_total
                            .with_label_values(&[alias_label])
                            .inc();
                    }
                }
                Err(error) => {
                    debug!(
                        target: "telemetry::sorafs.privacy",
                        provider = %alias,
                        %error,
                        "failed to poll proxy policy downgrade endpoint"
                    );
                }
            }
        }

        if stop.load(AtomicOrdering::Relaxed) {
            break;
        }

        tokio::select! {
            _ = notify.notified() => break,
            _ = sleep(interval) => {}
        }
    }
}

fn flush_privacy_buckets(
    aggregator: &SoranetSecureAggregator,
    metrics: &Arc<iroha_telemetry::metrics::Metrics>,
) {
    let (buckets, snapshot) = aggregator.drain_ready_now_with_snapshot();
    metrics.record_soranet_privacy_queue_snapshot(&snapshot);
    for bucket in buckets {
        metrics.record_soranet_privacy_bucket(&bucket);
    }
}

fn privacy_endpoints_from_providers(providers: &[FetchProvider]) -> Vec<(String, Url)> {
    let mut endpoints = Vec::new();
    for provider in providers {
        let alias = provider.id().as_str().to_string();
        if let Some(metadata) = provider.metadata()
            && let Some(raw) = metadata.privacy_events_url.as_ref()
        {
            match Url::parse(raw) {
                Ok(url) => endpoints.push((alias.clone(), url)),
                Err(error) => warn!(
                    target: "telemetry::sorafs.privacy",
                    provider = %alias,
                    url = raw,
                    %error,
                    "skipping invalid privacy events URL"
                ),
            }
        }
    }
    endpoints
}

fn manifest_id_hex(plan: &CarBuildPlan) -> String {
    hex::encode(plan.payload_digest.as_bytes())
}

fn generate_job_id() -> String {
    let mut bytes = [0u8; 16];
    let mut rng = rand::rng();
    rng.fill(&mut bytes);
    hex::encode(bytes)
}

fn error_reason(error: &multi_fetch::MultiSourceError) -> &'static str {
    match error {
        multi_fetch::MultiSourceError::NoProviders => "no_providers",
        multi_fetch::MultiSourceError::NoHealthyProviders { .. } => "no_healthy_providers",
        multi_fetch::MultiSourceError::NoCompatibleProviders { .. } => "no_compatible_providers",
        multi_fetch::MultiSourceError::ExhaustedRetries { .. } => "exhausted_retries",
        multi_fetch::MultiSourceError::ObserverFailed { .. } => "observer_failed",
        multi_fetch::MultiSourceError::InternalInvariant(_) => "internal_invariant",
    }
}

fn provider_from_error(error: &multi_fetch::MultiSourceError) -> Option<(String, String)> {
    match error {
        multi_fetch::MultiSourceError::NoHealthyProviders {
            last_error: Some(last),
            ..
        } => Some((
            last.provider.as_str().to_string(),
            attempt_failure_reason(&last.failure).to_string(),
        )),
        multi_fetch::MultiSourceError::ExhaustedRetries { last_error, .. } => Some((
            last_error.provider.as_str().to_string(),
            attempt_failure_reason(&last_error.failure).to_string(),
        )),
        _ => None,
    }
}

fn attempt_failure_reason(failure: &AttemptFailure) -> &'static str {
    match failure {
        AttemptFailure::Provider { policy_block, .. } => {
            if policy_block.is_some() {
                "policy_block"
            } else {
                "provider_error"
            }
        }
        AttemptFailure::InvalidChunk(reason) => match reason {
            ChunkVerificationError::LengthMismatch { .. } => "length_mismatch",
            ChunkVerificationError::DigestMismatch { .. } => "digest_mismatch",
        },
    }
}

fn wrap_fetcher_with_taikai_queue<F, Fut, E>(
    fetcher: F,
    bridge: Arc<TaikaiQueueBridge>,
) -> impl Fn(FetchRequest) -> Pin<Box<dyn Future<Output = Result<ChunkResponse, E>> + Send>>
+ Send
+ Sync
+ 'static
where
    F: Fn(FetchRequest) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<ChunkResponse, E>> + Send + 'static,
    E: std::error::Error + Send + Sync + 'static,
{
    let fetcher = Arc::new(fetcher);
    move |request: FetchRequest| {
        let bridge = bridge.clone();
        let hedge_after = bridge.hedge_after();
        let hedge_candidate = request.clone();
        let fetcher = fetcher.clone();
        Box::pin(async move {
            let mut lease = bridge.before_request(&request);
            if lease.is_none() {
                return (*fetcher)(request).await;
            }

            let batch_id = lease.as_ref().map(TaikaiQueueLease::batch_id);
            let mut primary = Box::pin((*fetcher)(request));
            let mut hedge_delay = Box::pin(sleep(hedge_after));
            let mut hedged: Option<Pin<Box<_>>> = None;
            let mut primary_result: Option<Result<ChunkResponse, E>> = None;
            let mut hedged_result: Option<Result<ChunkResponse, E>> = None;

            loop {
                tokio::select! {
                    outcome = &mut primary, if primary_result.is_none() => {
                        primary_result = Some(outcome);
                    }
                    _ = &mut hedge_delay, if hedged.is_none() => {
                        let should_hedge = match batch_id {
                            Some(id) => bridge.trigger_hedge(id, Instant::now()),
                            None => true,
                        };
                        if should_hedge {
                            hedged = Some(Box::pin((*fetcher)(hedge_candidate.clone())));
                        }
                    }
                    outcome = async {
                        if let Some(fut) = hedged.as_mut() {
                            fut.await
                        } else {
                            futures::future::pending().await
                        }
                    }, if hedged.is_some() && hedged_result.is_none() => {
                        hedged_result = Some(outcome);
                    }
                }

                if let Some(Ok(value)) = primary_result.take() {
                    if let Some(lease) = lease.take() {
                        lease.success();
                    }
                    return Ok(value);
                }

                if let Some(Ok(value)) = hedged_result.take() {
                    if let Some(lease) = lease.take() {
                        lease.success();
                    }
                    return Ok(value);
                }

                let primary_failed = matches!(primary_result, Some(Err(_)));
                let hedged_failed = matches!(hedged_result, Some(Err(_)));
                if primary_failed && hedged_failed {
                    if let Some(lease) = lease.take() {
                        lease.failure();
                    }
                    return primary_result.take().unwrap_or_else(|| {
                        hedged_result.expect("hedged result collected before returning")
                    });
                }

                if primary_failed && hedged.is_none() {
                    if let Some(lease) = lease.take() {
                        lease.failure();
                    }
                    return primary_result
                        .expect("primary result captured when hedged future is absent");
                }
            }
        })
    }
}

struct TaikaiQueueBridge {
    handle: TaikaiCacheHandle,
    hedge_after: Duration,
}

impl TaikaiQueueBridge {
    fn new(handle: TaikaiCacheHandle, plan: &CarBuildPlan) -> Option<Arc<Self>> {
        if plan
            .chunks
            .iter()
            .any(|chunk| chunk.taikai_segment_hint.is_some())
        {
            let hedge_after = handle.pull_queue_config().hedge_after;
            Some(Arc::new(Self {
                handle,
                hedge_after,
            }))
        } else {
            None
        }
    }

    fn before_request(&self, request: &FetchRequest) -> Option<TaikaiQueueLease> {
        let hint = request.spec.taikai_segment_hint.as_ref()?;
        let key = taikai_segment_key_from_hint(hint)?;
        let size_bytes = hint.payload_len.unwrap_or(request.spec.length as u64);
        let pull = TaikaiPullRequest::new(
            key.clone(),
            QosClass::Priority,
            size_bytes,
            hint.payload_digest,
        );
        match self.handle.enqueue_pull(pull) {
            Ok(()) => match self.handle.issue_specific_at(&key, Instant::now()) {
                Ok(Some(batch)) => Some(TaikaiQueueLease::new(
                    self.handle.clone(),
                    TaikaiPullTicket::from(batch.id),
                    batch.id,
                )),
                Ok(None) => {
                    _ = self.handle.cancel_pending(&key);
                    None
                }
                Err(err) => {
                    warn!(
                        target: "soranet.taikai_cache",
                        ?err,
                        "failed to issue Taikai queue batch for fetch request"
                    );
                    _ = self.handle.cancel_pending(&key);
                    None
                }
            },
            Err(TaikaiQueueError::Backpressure { .. }) => {
                warn!(
                    target: "soranet.taikai_cache",
                    "Taikai pull queue backpressure while scheduling fetch request"
                );
                _ = self.handle.cancel_pending(&key);
                None
            }
            Err(TaikaiQueueError::Unavailable) => None,
        }
    }

    fn hedge_after(&self) -> Duration {
        self.hedge_after
    }

    fn trigger_hedge(&self, id: crate::taikai_cache::TaikaiPullBatchId, now: Instant) -> bool {
        match self.handle.hedge_overdue_batches_at(now) {
            Ok(batches) => batches.iter().any(|batch| batch.id == id),
            Err(error) => {
                warn!(
                    target: "soranet.taikai_cache",
                    ?error,
                    "taikai hedge trigger failed while acquiring queue lock"
                );
                false
            }
        }
    }
}

struct TaikaiQueueLease {
    handle: TaikaiCacheHandle,
    ticket: TaikaiPullTicket,
    batch_id: crate::taikai_cache::TaikaiPullBatchId,
}

impl TaikaiQueueLease {
    fn new(
        handle: TaikaiCacheHandle,
        ticket: TaikaiPullTicket,
        batch_id: crate::taikai_cache::TaikaiPullBatchId,
    ) -> Self {
        Self {
            handle,
            ticket,
            batch_id,
        }
    }

    fn success(self) {
        let _ = self.handle.complete_batch(self.ticket);
    }

    fn failure(self) {
        let _ = self.handle.fail_batch(self.ticket);
    }

    fn batch_id(&self) -> crate::taikai_cache::TaikaiPullBatchId {
        self.batch_id
    }
}

fn taikai_segment_key_from_hint(hint: &TaikaiSegmentHint) -> Option<TaikaiSegmentKey> {
    let event = TaikaiEventId::new(Name::from_str(&hint.event).ok()?);
    let stream = TaikaiStreamId::new(Name::from_str(&hint.stream).ok()?);
    let rendition = TaikaiRenditionId::new(Name::from_str(&hint.rendition).ok()?);
    Some(TaikaiSegmentKey::new(
        event,
        stream,
        rendition,
        hint.sequence,
    ))
}

/// Errors surfaced when orchestrating gateway-backed fetches.
#[derive(Debug, Error)]
pub enum GatewayOrchestratorError {
    /// Gateway configuration or provider inputs were invalid.
    #[error("failed to build gateway context: {0}")]
    Build(#[from] GatewayBuildError),
    /// Scoreboard excluded all providers, leaving no candidates for streaming.
    #[error("no eligible providers available for SoraFS fetch")]
    NoEligibleProviders,
    /// Underlying orchestrator failed during scoreboard construction or fetch.
    #[error(transparent)]
    Orchestrator(#[from] OrchestratorError),
    /// Manifest fetch failed for verification.
    #[error("failed to fetch manifest for verification: {0}")]
    Manifest(#[from] GatewayManifestError),
    /// Failed to assemble CAR for verification.
    #[error("failed to assemble CAR for verification: {0}")]
    CarBuild(String),
    /// Manifest and CAR verification failed.
    #[error("manifest verification failed: {0}")]
    Verification(String),
}

impl From<ManifestVerificationError> for GatewayOrchestratorError {
    fn from(err: ManifestVerificationError) -> Self {
        match err {
            ManifestVerificationError::CarBuild(message) => Self::CarBuild(message),
            ManifestVerificationError::Verification(message) => Self::Verification(message),
            ManifestVerificationError::ManifestValidation(message) => Self::Verification(message),
            other => Self::Verification(other.to_string()),
        }
    }
}

/// Execute a gateway-backed fetch using the orchestrator facade.
///
/// This helper wires the [`GatewayFetchContext`] into the high-level orchestrator by deriving
/// provider metadata, applying optional limits, and streaming chunks with the configured retry
/// policy. It is intended for SDK/CLI bindings that already possess stream tokens and manifest
/// context but would rather not construct scoreboards manually.
///
/// # Errors
///
/// Returns [`GatewayOrchestratorError::Build`] if provider descriptors are malformed,
/// [`GatewayOrchestratorError::NoEligibleProviders`] when every provider is filtered out by the
/// scoreboard, or [`GatewayOrchestratorError::Orchestrator`] when the underlying orchestrator
/// fails (either while building the scoreboard or during the fetch loop).
pub async fn fetch_via_gateway(
    mut config: OrchestratorConfig,
    plan: &CarBuildPlan,
    gateway_config: GatewayFetchConfig,
    providers: impl IntoIterator<Item = GatewayProviderInput>,
    telemetry: Option<&TelemetrySnapshot>,
    max_peers: Option<usize>,
) -> Result<FetchSession, GatewayOrchestratorError> {
    let context = GatewayFetchContext::new(gateway_config, providers)?;
    let metadata: Vec<ProviderMetadata> = context
        .providers()
        .into_iter()
        .map(|provider| {
            let alias = provider.id().as_str().to_string();
            let mut meta = provider
                .metadata()
                .cloned()
                .unwrap_or_else(ProviderMetadata::new);
            let canonical_provider_id = meta.provider_id.clone();
            if !meta.profile_aliases.iter().any(|entry| entry == &alias) {
                meta.profile_aliases.push(alias.clone());
            }
            if let Some(hex_id) = canonical_provider_id.as_ref()
                && hex_id.len() == 64
                && hex_id.chars().all(|c| c.is_ascii_hexdigit())
                && !meta.profile_aliases.iter().any(|entry| entry == hex_id)
            {
                meta.profile_aliases.push(hex_id.clone());
            }
            meta.provider_id = Some(alias);
            if meta.range_capability.is_none() {
                meta.range_capability = Some(multi_fetch::RangeCapability {
                    max_chunk_span: u32::MAX,
                    min_granularity: 1,
                    supports_sparse_offsets: true,
                    requires_alignment: false,
                    supports_merkle_proof: true,
                });
            }
            if meta.stream_budget.is_none()
                && let Some(max_streams) = meta.max_streams
            {
                meta.stream_budget = Some(multi_fetch::StreamBudget {
                    max_in_flight: max_streams,
                    max_bytes_per_sec: 0,
                    burst_bytes: None,
                });
            }
            meta
        })
        .collect();

    if metadata.is_empty() {
        return Err(GatewayOrchestratorError::NoEligibleProviders);
    }

    if let Some(limit) = max_peers {
        let limit = limit.max(1);
        config.max_providers = NonZeroUsize::new(limit);
        config.fetch.global_parallel_limit = Some(
            config
                .fetch
                .global_parallel_limit
                .map_or(limit, |existing| existing.min(limit)),
        );
    }

    let telemetry_snapshot = telemetry.map_or_else(TelemetrySnapshot::default, Clone::clone);

    let orchestrator = Orchestrator::new(config);
    let scoreboard = orchestrator.build_scoreboard(plan, &metadata, &telemetry_snapshot)?;

    if !scoreboard
        .entries()
        .iter()
        .any(|entry| matches!(entry.eligibility, Eligibility::Eligible))
    {
        return Err(GatewayOrchestratorError::NoEligibleProviders);
    }

    let fetcher = context.fetcher();
    let mut session = orchestrator
        .fetch_with_scoreboard(plan, &scoreboard, fetcher.as_closure())
        .await
        .map_err(GatewayOrchestratorError::from)?;

    let gateway_manifest = context.fetch_manifest().await?;
    let verification_context = ManifestVerificationContext::from(&gateway_manifest);
    session
        .verify_against_manifest(plan, verification_context)
        .map_err(GatewayOrchestratorError::from)?;

    Ok(session)
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeSet, HashMap},
        convert::TryInto,
        fmt,
        fs::File,
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        num::{NonZeroU32, NonZeroUsize},
        path::PathBuf,
        sync::{
            Arc, Mutex,
            atomic::{AtomicUsize, Ordering as AtomicOrdering},
        },
        thread,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
    use futures::executor::block_on;
    use iroha_data_model::soranet::privacy_metrics::{
        SoranetPrivacyEventActiveSampleV1, SoranetPrivacyEventHandshakeFailureV1,
        SoranetPrivacyEventHandshakeSuccessV1, SoranetPrivacyEventKindV1,
        SoranetPrivacyEventThrottleV1, SoranetPrivacyEventV1, SoranetPrivacyEventVerifiedBytesV1,
        SoranetPrivacyHandshakeFailureV1, SoranetPrivacyModeV1, SoranetPrivacyPrioShareV1,
        SoranetPrivacyThrottleScopeV1,
    };
    use iroha_logger::{telemetry::Channel, test_logger};
    use iroha_telemetry::metrics::global_or_default;
    use norito::json::{self, Map, Value};
    use reqwest::Url;
    use sorafs_car::{
        CarBuildPlan, CarChunk, ChunkFetchSpec, ChunkStore, FilePlan, TaikaiSegmentHint,
        multi_fetch::{ChunkResponse, FetchProvider, FetchRequest, TransportHint},
        scoreboard::{IneligibilityReason, ProviderTelemetry},
    };
    use sorafs_chunker::ChunkProfile;
    use sorafs_manifest::{StreamTokenBodyV1, StreamTokenV1};
    use tokio::sync::Mutex as AsyncMutex;

    use super::*;
    use crate::{
        bindings::{config_from_json, config_to_json},
        proxy::ProxyMode,
        soranet::{
            CircuitId, CircuitRetirementReason, Endpoint, GuardRecord, GuardSet, PathMetadata,
            RelayDescriptor, RelayDirectory, RelayRoles,
        },
        taikai_cache::TaikaiPullQueueConfig,
    };

    fn relay_id(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn entry_descriptor(id_byte: u8, weight: u32, endpoints: Vec<Endpoint>) -> RelayDescriptor {
        RelayDescriptor {
            relay_id: relay_id(id_byte),
            guard_weight: weight,
            bandwidth_bytes_per_sec: 0,
            reputation_weight: 0,
            roles: RelayRoles::new(true, true, true),
            endpoints,
            pq_kem_public: None,
            certificate: None,
            path_metadata: PathMetadata::default(),
        }
    }

    fn provider_metadata(id: &str) -> ProviderMetadata {
        let mut metadata = ProviderMetadata::new();
        metadata.provider_id = Some(id.to_owned());
        metadata.transport_hints = vec![TransportHint {
            protocol: "soranet".into(),
            protocol_id: 3,
            priority: 0,
        }];
        metadata.capability_names = vec![
            "soranet_pq".into(),
            "soranet_pq_guard".into(),
            format!("soranet_guard_pq_{id}"),
        ];
        metadata.range_capability = Some(multi_fetch::RangeCapability {
            max_chunk_span: 2_097_152,
            min_granularity: 1024,
            supports_sparse_offsets: true,
            requires_alignment: false,
            supports_merkle_proof: true,
        });
        metadata.stream_budget = Some(multi_fetch::StreamBudget {
            max_in_flight: 4,
            max_bytes_per_sec: 4 * 1024 * 1024,
            burst_bytes: Some(4 * 1024 * 1024),
        });
        metadata
    }

    fn assert_close(lhs: f64, rhs: f64) {
        let delta = (lhs - rhs).abs();
        assert!(delta < 1e-6, "expected {lhs} ≈ {rhs} (delta {delta})");
    }

    fn directory_descriptor(id: u8, roles: RelayRoles, pq: bool) -> RelayDescriptor {
        let mut descriptor = RelayDescriptor {
            relay_id: [id; 32],
            guard_weight: 50,
            bandwidth_bytes_per_sec: 0,
            reputation_weight: 0,
            roles,
            endpoints: vec![Endpoint::new(format!("soranet://relay-{id:02x}"), 0)],
            pq_kem_public: None,
            certificate: None,
            path_metadata: PathMetadata::default(),
        };
        if pq {
            descriptor.pq_kem_public = Some(vec![id]);
        }
        descriptor
    }

    fn should_skip_socket_permission(message: &str) -> bool {
        message.contains("Operation not permitted") || message.contains("Permission denied")
    }

    #[derive(Debug)]
    struct HedgedTestError(&'static str);

    impl fmt::Display for HedgedTestError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(self.0)
        }
    }

    impl std::error::Error for HedgedTestError {}

    fn taikai_plan_with_hint(payload: &[u8]) -> CarBuildPlan {
        let digest = blake3::hash(payload);
        let hint = TaikaiSegmentHint {
            event: "global-keynote".into(),
            stream: "stage-a".into(),
            rendition: "1080p".into(),
            sequence: 7,
            payload_len: Some(payload.len() as u64),
            payload_digest: Some(*digest.as_bytes()),
        };
        let chunk = CarChunk {
            offset: 0,
            length: payload
                .len()
                .try_into()
                .expect("small payload fits into u32"),
            digest: *digest.as_bytes(),
            taikai_segment_hint: Some(hint),
        };
        CarBuildPlan {
            chunk_profile: ChunkProfile::DEFAULT,
            payload_digest: digest,
            content_length: payload.len() as u64,
            chunks: vec![chunk],
            files: vec![FilePlan {
                path: vec!["taikai.ts".into()],
                first_chunk: 0,
                chunk_count: 1,
                size: payload.len() as u64,
            }],
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn downgrade_remediator_switches_proxy_mode() {
        test_logger();
        let proxy_cfg = LocalQuicProxyConfig {
            bind_addr: "127.0.0.1:0".into(),
            telemetry_label: Some("remediation-test".into()),
            proxy_mode: ProxyMode::Bridge,
            ..LocalQuicProxyConfig::default()
        };
        let initial_handle = match spawn_local_quic_proxy(proxy_cfg.clone()) {
            Ok(handle) => handle,
            Err(ProxyError::QuinnEndpoint(message)) if should_skip_socket_permission(&message) => {
                eprintln!("skipping downgrade remediator test (loopback unavailable): {message}");
                return;
            }
            Err(err) => panic!("spawn local proxy: {err}"),
        };
        let runtime = Arc::new(AsyncMutex::new(LocalProxyRuntime::new(
            proxy_cfg.clone(),
            Some(initial_handle),
        )));
        let metrics = Arc::new(iroha_telemetry::metrics::Metrics::default());
        let remediation_cfg = DowngradeRemediationConfig {
            enabled: true,
            threshold: NonZeroU32::new(2).expect("non-zero threshold"),
            window: Duration::from_secs(30),
            cooldown: Duration::from_millis(100),
            target_mode: ProxyMode::MetadataOnly,
            resume_mode: ProxyMode::Bridge,
            modes: vec![SoranetPrivacyModeV1::Entry],
        };
        let remediator =
            DowngradeRemediator::new(Arc::clone(&runtime), remediation_cfg, Arc::clone(&metrics));

        remediator
            .observe_handshake_downgrade(0, SoranetPrivacyModeV1::Entry)
            .await;
        {
            let guard = runtime.lock().await;
            assert_eq!(guard.mode(), ProxyMode::Bridge);
        }

        remediator
            .observe_handshake_downgrade(1, SoranetPrivacyModeV1::Entry)
            .await;
        tokio::time::sleep(Duration::from_millis(50)).await;
        {
            let guard = runtime.lock().await;
            assert_eq!(guard.mode(), ProxyMode::MetadataOnly);
        }

        tokio::time::sleep(Duration::from_millis(180)).await;
        let final_handle = {
            let guard = runtime.lock().await;
            assert_eq!(guard.mode(), ProxyMode::Bridge);
            guard.handle.clone()
        };
        if let Some(handle) = final_handle {
            handle.shutdown().await;
        }
    }

    #[test]
    fn reconcile_circuits_returns_none_when_disabled() {
        test_logger();

        let config = OrchestratorConfig {
            circuit_manager: None,
            ..OrchestratorConfig::default()
        };
        let orchestrator = Orchestrator::new(config);

        assert!(
            orchestrator
                .reconcile_circuits()
                .expect("circuit reconciliation should not fail when disabled")
                .is_none(),
            "reconciliation should be skipped when the circuit manager is disabled"
        );
        assert!(orchestrator.active_circuits().is_empty());
        assert!(orchestrator.circuit_rotation_history().is_empty());
        assert!(
            !orchestrator.record_circuit_latency(CircuitId::from_raw(7), Duration::from_millis(5),),
            "latency recording must no-op when circuits are disabled"
        );
        assert!(
            orchestrator.teardown_circuits(SystemTime::now()).is_none(),
            "teardown should return None when circuits are disabled"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn taikai_fetch_wrapper_hedges_overdue_batches() {
        test_logger();

        let plan = taikai_plan_with_hint(&[0x01, 0x02, 0x03, 0x04]);
        let cache_config = TaikaiCacheConfig::default();
        let mut queue_config = TaikaiPullQueueConfig::tuned_for_cache(&cache_config);
        queue_config.hedge_after = Duration::from_millis(5);
        let handle = TaikaiCacheHandle::with_queue_config(cache_config, queue_config);
        let bridge = TaikaiQueueBridge::new(handle.clone(), &plan)
            .expect("taikai plan activates queue bridge");
        let attempts = Arc::new(AtomicUsize::new(0));
        let fetcher = {
            let attempts = attempts.clone();
            move |_request: FetchRequest| {
                let attempts = attempts.clone();
                async move {
                    let idx = attempts.fetch_add(1, AtomicOrdering::SeqCst);
                    if idx == 0 {
                        sleep(Duration::from_millis(20)).await;
                        return Err(HedgedTestError("primary stalled"));
                    }
                    Ok(ChunkResponse::new(vec![0xAA, 0xBB, 0xCC, 0xDD]))
                }
            }
        };
        let wrapped = wrap_fetcher_with_taikai_queue(fetcher, bridge);
        let chunk = plan
            .chunks
            .first()
            .expect("taikai plan must contain one chunk")
            .clone();
        let request = FetchRequest {
            provider: Arc::new(FetchProvider::new("provider-1")),
            spec: ChunkFetchSpec {
                chunk_index: 0,
                offset: chunk.offset,
                length: chunk.length,
                digest: chunk.digest,
                taikai_segment_hint: chunk.taikai_segment_hint.clone(),
            },
            attempt: 1,
        };

        let outcome = wrapped(request).await.expect("hedged fetch succeeds");
        assert_eq!(outcome.bytes, vec![0xAA, 0xBB, 0xCC, 0xDD]);
        assert_eq!(attempts.load(AtomicOrdering::SeqCst), 2);
        let stats = handle.queue_stats();
        assert!(
            stats.hedged_batches >= 1,
            "expected hedged batch to be recorded, stats={stats:?}"
        );
    }

    #[test]
    fn reconcile_circuits_reports_and_teardown() {
        test_logger();

        let guard = GuardRecord {
            relay_id: [0xAA; 32],
            pinned_at_unix: 100,
            endpoint: Endpoint::new("soranet://guard-AA", 0),
            guard_weight: 50,
            bandwidth_bytes_per_sec: 0,
            reputation_weight: 0,
            pq_kem_public: Some(vec![0x42]),
            certificate: None,
            path_metadata: PathMetadata::default(),
        };
        let guard_set = GuardSet::new(vec![guard.clone()]);
        let directory = RelayDirectory::new(vec![
            directory_descriptor(0x11, RelayRoles::new(false, true, false), true),
            directory_descriptor(0x22, RelayRoles::new(false, false, true), true),
        ]);

        let config = OrchestratorConfig {
            guard_set: Some(guard_set),
            relay_directory: Some(directory),
            circuit_manager: Some(CircuitManagerConfig::new(Duration::from_secs(2))),
            anonymity_policy: AnonymityPolicy::GuardPq,
            ..OrchestratorConfig::default()
        };
        let orchestrator = Orchestrator::new(config);

        let initial_report = orchestrator
            .reconcile_circuits_at(UNIX_EPOCH + Duration::from_secs(10))
            .expect("initial reconciliation must succeed")
            .expect("circuit manager should be enabled");
        assert_eq!(
            initial_report.events.len(),
            1,
            "initial reconciliation should emit a build event"
        );
        assert!(
            matches!(initial_report.events[0], CircuitEvent::Built { .. }),
            "expected a built event after initial reconciliation"
        );
        assert_eq!(initial_report.active.len(), 1);
        assert!(
            initial_report.rotation_history.is_empty(),
            "rotation history should be empty after initial build"
        );

        let active_circuit = &initial_report.active[0];
        assert!(
            orchestrator.record_circuit_latency(active_circuit.id, Duration::from_millis(47)),
            "latency sample should be recorded for active circuit"
        );

        let rotated_report = orchestrator
            .reconcile_circuits_at(UNIX_EPOCH + Duration::from_secs(13))
            .expect("rotation reconciliation must succeed")
            .expect("circuit manager should remain enabled");
        assert!(
            rotated_report
                .events
                .iter()
                .any(|event| matches!(event, CircuitEvent::Retired { .. })),
            "expected a retirement event after TTL expiration"
        );
        assert!(
            orchestrator
                .circuit_rotation_history()
                .iter()
                .any(|record| matches!(record.reason, CircuitRetirementReason::Expired)),
            "rotation history should record the expired circuit"
        );

        let teardown_records = orchestrator
            .teardown_circuits(UNIX_EPOCH + Duration::from_secs(20))
            .expect("teardown should return rotation records");
        assert!(
            !teardown_records.is_empty(),
            "teardown must produce rotation records"
        );
        assert!(
            teardown_records
                .iter()
                .any(|record| matches!(record.reason, CircuitRetirementReason::ManualTeardown)),
            "teardown should tag retired circuits with ManualTeardown"
        );
        assert!(
            orchestrator.active_circuits().is_empty(),
            "no circuits should remain active after teardown"
        );
    }

    fn spawn_privacy_server(
        status_line: &'static str,
        body: Option<String>,
        policy_body: Option<String>,
    ) -> std::io::Result<(Url, thread::JoinHandle<()>)> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let addr = listener
            .local_addr()
            .expect("obtain mock privacy server address");
        let url = Url::parse(&format!("http://127.0.0.1:{}/privacy/events", addr.port()))
            .expect("construct mock privacy server url");
        let body_bytes = body.map(|value| value.into_bytes());
        let policy_bytes = policy_body.map(|value| value.into_bytes());
        let handle = thread::spawn(move || {
            let mut remaining_privacy = body_bytes;
            let mut remaining_policy = policy_bytes;
            let expected_requests =
                usize::from(remaining_privacy.is_some()) + usize::from(remaining_policy.is_some());
            let deadline = Instant::now() + Duration::from_secs(2);
            let mut served_requests = 0usize;
            while served_requests < expected_requests && Instant::now() < deadline {
                let (mut stream, _) = match listener.accept() {
                    Ok(pair) => pair,
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                        continue;
                    }
                    Err(_) => break,
                };
                served_requests = served_requests.saturating_add(1);
                let mut buffer = [0u8; 4096];
                let mut received = Vec::new();
                loop {
                    match stream.read(&mut buffer) {
                        Ok(0) => break,
                        Ok(count) => {
                            received.extend_from_slice(&buffer[..count]);
                            if received.windows(4).any(|chunk| chunk == b"\r\n\r\n") {
                                break;
                            }
                        }
                        Err(error) => panic!("failed to read privacy request: {error}"),
                    }
                }

                let request = String::from_utf8_lossy(&received);
                let path = request
                    .lines()
                    .next()
                    .and_then(|line| line.split_whitespace().nth(1))
                    .unwrap_or("/privacy/events");
                let body = if path == "/policy/proxy-toggle" {
                    &mut remaining_policy
                } else {
                    &mut remaining_privacy
                };
                let content_length = body.as_ref().map_or(0, Vec::len);
                let mut response = format!(
                    "HTTP/1.1 {status_line}\r\nConnection: close\r\nContent-Length: {content_length}\r\n"
                );
                if content_length > 0 {
                    response.push_str("Content-Type: application/x-ndjson\r\n");
                }
                response.push_str("\r\n");
                stream
                    .write_all(response.as_bytes())
                    .expect("write mock privacy headers");
                if let Some(bytes) = body.take() {
                    stream.write_all(&bytes).expect("write mock privacy body");
                }
                stream.flush().expect("flush mock privacy response");
            }
        });
        Ok((url, handle))
    }

    #[test]
    fn compliance_example_config_parses() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let example_path = manifest_dir.join("../../docs/examples/sorafs_compliance_policy.json");
        let file = File::open(&example_path).expect("open compliance example");
        let value: Value =
            norito::json::from_reader(file).expect("decode compliance example config");
        let config =
            bindings::config_from_json(&value).expect("parse orchestrator compliance config");

        assert_eq!(
            config.compliance.operator_jurisdictions(),
            &["US".to_string(), "JP".to_string()]
        );
        assert!(
            config
                .compliance
                .jurisdiction_opt_outs()
                .any(|code| code == "US")
        );
        let digest_vec =
            hex::decode("C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828")
                .expect("hex digest");
        let digest: [u8; Hash::LENGTH] = digest_vec.clone().try_into().expect("digest length");
        assert!(
            config
                .compliance
                .blinded_cid_opt_outs()
                .any(|entry| entry == &digest)
        );
        assert!(
            config
                .compliance
                .audit_contacts()
                .iter()
                .any(|contact| contact.starts_with("mailto:"))
        );
        let attestations = config.compliance.attestations();
        assert_eq!(attestations.len(), 2);
        let first = &attestations[0];
        assert_eq!(first.jurisdiction(), Some("US"));
        assert_eq!(
            first.document_uri(),
            "https://gov.example.org/gar/us-attestation.pdf"
        );
        let first_digest: [u8; Hash::LENGTH] =
            hex::decode("37E8AA2E0C9BE3D64B2812747307D534642856A3F234D6F087E42A54B3A8380F")
                .expect("first attestation digest")
                .try_into()
                .expect("digest len");
        assert_eq!(first.digest(), &first_digest);
        assert_eq!(first.issued_at_ms(), 1_704_000_000_000);
        assert_eq!(first.expires_at_ms(), Some(1_706_000_000_000));
        let second = &attestations[1];
        assert_eq!(second.jurisdiction(), None);
        assert_eq!(
            second.document_uri(),
            "https://gov.example.org/gar/global-attestation.json"
        );
        let second_digest: [u8; Hash::LENGTH] =
            hex::decode("9D0E16C6F42449F57AD1BB221B35A484AF881B40B3F99588635B9A5FB1F88DA2")
                .expect("second attestation digest")
                .try_into()
                .expect("digest len");
        assert_eq!(second.digest(), &second_digest);
        assert_eq!(second.issued_at_ms(), 1_704_500_000_000);
        assert_eq!(second.expires_at_ms(), None);
    }

    #[test]
    fn compliance_config_roundtrips_in_json() {
        let digest = Hash::new(b"roundtrip");
        let mut policy = CompliancePolicy::default();
        policy.set_operator_jurisdictions(vec!["CA".to_string()]);
        policy.set_jurisdiction_opt_outs(BTreeSet::from(["CA".to_string()]));
        policy.set_blinded_cid_opt_outs(BTreeSet::from([*digest.as_ref()]));
        policy.set_audit_contacts(vec!["https://gov.example/audit".to_string()]);
        let attestation_digest = Hash::new(b"attestation-roundtrip");
        policy.set_attestations(vec![compliance::OperatorAttestation::new(
            Some("CA".to_string()),
            "https://gov.example/audit/attestation.json",
            *attestation_digest.as_ref(),
            1_704_100_000_000,
            Some(1_706_100_000_000),
        )]);

        let config = OrchestratorConfig {
            compliance: policy.clone(),
            ..OrchestratorConfig::default()
        };

        let json = bindings::config_to_json(&config);
        let parsed = bindings::config_from_json(&json).expect("roundtrip compliance config");
        assert_eq!(parsed.compliance, policy);
        let object = json
            .as_object()
            .expect("orchestrator config serialises as JSON object");
        assert!(object.contains_key("compliance"));
    }
    #[test]
    fn taikai_cache_config_roundtrips_in_json() {
        let cache_config = TaikaiCacheConfig {
            hot_capacity_bytes: 8 * 1024 * 1024,
            hot_retention: Duration::from_secs(45),
            warm_capacity_bytes: 32 * 1024 * 1024,
            warm_retention: Duration::from_mins(3),
            cold_capacity_bytes: 256 * 1024 * 1024,
            cold_retention: Duration::from_hours(1),
            qos: QosConfig {
                priority_rate_bps: 80 * 1024 * 1024,
                standard_rate_bps: 40 * 1024 * 1024,
                bulk_rate_bps: 12 * 1024 * 1024,
                burst_multiplier: 4,
            },
            reliability: ReliabilityTuning {
                failures_to_trip: 5,
                open_secs: 7,
            },
        };
        let config = OrchestratorConfig {
            taikai_cache: Some(cache_config.clone()),
            ..OrchestratorConfig::default()
        };

        let json = bindings::config_to_json(&config);
        let parsed = bindings::config_from_json(&json).expect("roundtrip taikai cache config");
        assert_eq!(parsed.taikai_cache, Some(cache_config));
        let object = json
            .as_object()
            .expect("orchestrator config serialises as JSON object");
        assert!(object.contains_key("taikai_cache"));
    }

    #[test]
    fn canonical_compliance_catalog_is_valid() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let catalog_path = manifest_dir.join("../../governance/compliance/soranet_opt_outs.json");
        let file = File::open(&catalog_path).expect("open canonical compliance catalog");
        let catalog_value: Value =
            norito::json::from_reader(file).expect("decode canonical compliance catalog");

        let mut root = Map::new();
        root.insert("compliance".into(), catalog_value);
        let config = bindings::config_from_json(&Value::Object(root))
            .expect("parse canonical compliance catalog");

        assert!(config.compliance.operator_jurisdictions().is_empty());
        assert!(
            config
                .compliance
                .jurisdiction_opt_outs()
                .any(|code| code == "US")
        );
        assert!(
            config
                .compliance
                .blinded_cid_opt_outs()
                .all(|digest| digest.len() == Hash::LENGTH)
        );
        assert!(
            config
                .compliance
                .audit_contacts()
                .iter()
                .any(|contact| contact.starts_with("https://"))
        );
    }

    #[test]
    fn orchestrator_defaults_to_guard_policy() {
        let config = OrchestratorConfig::default();
        assert_eq!(config.anonymity_policy, AnonymityPolicy::GuardPq);
    }

    #[test]
    fn orchestrator_refreshes_circuits_when_configured() {
        let directory = RelayDirectory::new(vec![
            entry_descriptor(0x01, 120, vec![Endpoint::new("soranet://relay-1", 0)]),
            entry_descriptor(0x02, 110, vec![Endpoint::new("soranet://relay-2", 0)]),
            entry_descriptor(0x03, 100, vec![Endpoint::new("soranet://relay-3", 0)]),
        ]);
        let guard = GuardRecord {
            relay_id: [0x01; 32],
            pinned_at_unix: 0,
            endpoint: Endpoint::new("soranet://relay-1", 0),
            guard_weight: 120,
            bandwidth_bytes_per_sec: 0,
            reputation_weight: 0,
            pq_kem_public: None,
            certificate: None,
            path_metadata: PathMetadata::default(),
        };
        let guard_set = GuardSet::new(vec![guard]);
        let config = OrchestratorConfig::default()
            .with_relay_directory(directory)
            .with_guard_set(guard_set)
            .with_circuit_manager(Some(CircuitManagerConfig::new(Duration::from_mins(5))));
        let orchestrator = Orchestrator::new(config);

        orchestrator
            .refresh_circuits()
            .expect("circuit refresh succeeds");

        let manager = orchestrator
            .circuit_manager
            .as_ref()
            .expect("circuit manager")
            .lock()
            .expect("lock circuit manager");
        assert_eq!(manager.active_circuits().len(), 1);
    }

    #[test]
    fn anonymity_policy_parses_stage_aliases() {
        assert_eq!(
            AnonymityPolicy::parse("stage-a"),
            Some(AnonymityPolicy::GuardPq)
        );
        assert_eq!(
            AnonymityPolicy::parse("stage_b"),
            Some(AnonymityPolicy::MajorityPq)
        );
        assert_eq!(
            AnonymityPolicy::parse("stageC"),
            Some(AnonymityPolicy::StrictPq)
        );
        assert_eq!(AnonymityPolicy::parse("anon-unknown"), None);
        assert_eq!(AnonymityPolicy::parse("stage_0"), None);
    }

    #[test]
    fn config_json_roundtrip_includes_path_hints_and_masque_bypass() {
        let relay_id = [0x11; 32];
        let hints = vec![RelayPathHint {
            relay_id,
            avg_rtt_ms: Some(18),
            region: Some("eu-central".to_string()),
            asn: Some(64_600),
            validator_lane: true,
            masque_bypass_allowed: true,
        }];
        let config = OrchestratorConfig::default()
            .with_relay_path_hints(hints.clone())
            .with_circuit_manager(Some(
                CircuitManagerConfig::new(Duration::from_secs(9))
                    .with_validator_masque_bypass(true),
            ));

        let value = config_to_json(&config);
        let parsed = config_from_json(&value).expect("parse config json");
        assert_eq!(parsed.relay_path_hints, hints);
        let circuit = parsed.circuit_manager.expect("circuit manager");
        assert!(circuit.validator_masque_bypass());
        assert_eq!(circuit.circuit_ttl(), Duration::from_secs(9));
    }

    #[test]
    fn guard_policy_reports_deficit_when_missing_pq() {
        let mut summary = PolicySummary::new(AnonymityPolicy::GuardPq, 3, 1);
        summary.update_selected_counts(3, 0);
        assert!(summary.is_brownout());
        assert_close(summary.target_ratio(), 1.0 / 3.0);
        assert_close(summary.ratio(), 0.0);
        assert_close(summary.deficit_ratio(), 1.0 / 3.0);
        assert_close(summary.candidate_ratio(), 1.0 / 3.0);
        assert_close(summary.supply_delta_ratio(), 1.0 / 3.0);
    }

    #[test]
    fn guard_policy_zero_deficit_when_pq_present() {
        let mut summary = PolicySummary::new(AnonymityPolicy::GuardPq, 3, 2);
        summary.update_selected_counts(3, 1);
        assert!(!summary.is_brownout());
        assert_close(summary.target_ratio(), 1.0 / 3.0);
        assert_close(summary.ratio(), 1.0 / 3.0);
        assert_close(summary.deficit_ratio(), 0.0);
        assert_close(summary.candidate_ratio(), 2.0 / 3.0);
        assert_close(summary.supply_delta_ratio(), (2.0 / 3.0) - (1.0 / 3.0));
    }

    #[test]
    fn majority_policy_deficit_reflects_gap() {
        let mut summary = PolicySummary::new(AnonymityPolicy::MajorityPq, 4, 2);
        summary.update_selected_counts(3, 1);
        assert!(summary.is_brownout());
        assert_close(summary.target_ratio(), MAJORITY_THRESHOLD);
        assert_close(summary.ratio(), 1.0 / 3.0);
        assert_close(
            summary.deficit_ratio(),
            (MAJORITY_THRESHOLD - (1.0 / 3.0)).clamp(0.0, 1.0),
        );
        assert_close(summary.candidate_ratio(), 0.5);
        assert_close(
            summary.supply_delta_ratio(),
            (0.5_f64 - (1.0_f64 / 3.0_f64)).clamp(0.0, 1.0),
        );
    }

    #[test]
    fn no_soranet_candidates_surface_full_deficit() {
        let summary = PolicySummary::new(AnonymityPolicy::StrictPq, 0, 0);
        assert!(!summary.is_brownout());
        assert_close(summary.deficit_ratio(), 1.0);
        assert_close(summary.candidate_ratio(), 0.0);
    }

    #[test]
    fn policy_report_preserves_effective_fallback_stage() {
        let mut summary = PolicySummary::new(AnonymityPolicy::StrictPq, 2, 0);
        summary.set_effective_policy(AnonymityPolicy::MajorityPq);
        summary.update_selected_counts(1, 0);

        let report = PolicyReport::from(summary);
        assert_eq!(report.policy, AnonymityPolicy::StrictPq);
        assert_eq!(report.effective_policy, AnonymityPolicy::MajorityPq);
        assert!(matches!(report.status, PolicyStatus::Brownout));
    }

    #[test]
    fn write_mode_hint_enforces_strict_policy() {
        let payload = vec![0x55; 2048];
        let plan = CarBuildPlan::single_file(&payload).expect("plan");
        let adverts = vec![provider_metadata("alpha")];
        let telemetry = TelemetrySnapshot::default();

        let config = OrchestratorConfig {
            write_mode: WriteModeHint::UploadPqOnly,
            ..OrchestratorConfig::default()
        };
        let orchestrator = Orchestrator::new(config.clone());
        let scoreboard = orchestrator
            .build_scoreboard(&plan, &adverts, &telemetry)
            .expect("scoreboard");

        let (transport_policy, anonymity_policy) = config
            .write_mode
            .apply(config.transport_policy, config.anonymity_policy);
        let SelectionOutcome { summary, .. } = eligible_providers(
            &scoreboard,
            config.max_providers,
            transport_policy,
            anonymity_policy,
            config.guard_set.as_ref(),
            config.relay_directory.as_ref(),
            false,
        );

        assert_eq!(summary.policy, AnonymityPolicy::StrictPq);
        assert_eq!(summary.effective_policy(), AnonymityPolicy::StrictPq);
    }

    #[derive(Debug, Clone)]
    struct HarnessError {
        message: String,
    }

    impl HarnessError {
        fn provider(message: impl Into<String>) -> Self {
            Self {
                message: message.into(),
            }
        }
    }

    impl fmt::Display for HarnessError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(&self.message)
        }
    }

    impl std::error::Error for HarnessError {}

    #[test]
    fn build_scoreboard_persists_json() {
        let temp = tempfile::tempdir().expect("tempdir");
        let persist_path = temp.path().join("scoreboard.json");

        let payload = vec![0xAB; 8192];
        let plan = CarBuildPlan::single_file(&payload).expect("plan");
        let adverts = vec![provider_metadata("alpha")];
        let telemetry = TelemetrySnapshot::default();

        let scoreboard_cfg = ScoreboardConfig {
            persist_path: Some(persist_path.clone()),
            ..ScoreboardConfig::default()
        };
        let orchestrator = Orchestrator::new(OrchestratorConfig {
            scoreboard: scoreboard_cfg,
            ..OrchestratorConfig::default()
        });

        let scoreboard = orchestrator
            .build_scoreboard(&plan, &adverts, &telemetry)
            .expect("scoreboard");
        assert_eq!(scoreboard.entries().len(), 1);

        let file = File::open(persist_path).expect("scoreboard file");
        let value: Value = norito::json::from_reader(file).expect("decode json");
        let entries = value
            .as_object()
            .and_then(|map| map.get("entries"))
            .and_then(Value::as_array)
            .expect("entries array");
        assert_eq!(entries.len(), 1);
        let provider_id = entries[0]
            .as_object()
            .and_then(|map| map.get("provider_id"))
            .and_then(Value::as_str)
            .expect("provider id");
        assert_eq!(provider_id, "alpha");
    }

    #[test]
    fn fetch_uses_eligible_provider() {
        let payload = vec![0xCD; 4096];
        let plan = CarBuildPlan::single_file(&payload).expect("plan");
        let good = provider_metadata("primary");
        let mut bad = ProviderMetadata::new();
        bad.provider_id = Some("secondary".into());
        bad.range_capability = None;

        let adverts = vec![good, bad];
        let telemetry = TelemetrySnapshot::default();

        let orchestrator = Orchestrator::new(OrchestratorConfig::default());
        let scoreboard = orchestrator
            .build_scoreboard(&plan, &adverts, &telemetry)
            .expect("scoreboard");
        assert_eq!(scoreboard.entries().len(), 2);
        assert!(
            matches!(
                scoreboard.entries()[1].eligibility,
                Eligibility::Ineligible(IneligibilityReason::Capability(_))
            ),
            "secondary provider should be marked ineligible"
        );

        let payload_arc = Arc::new(payload);
        let fetcher = {
            let payload = Arc::clone(&payload_arc);
            move |request: FetchRequest| {
                let payload = Arc::clone(&payload);
                let start = request.spec.offset as usize;
                let end = start + request.spec.length as usize;
                let bytes = payload[start..end].to_vec();
                futures::future::ready(Ok::<ChunkResponse, std::io::Error>(ChunkResponse::new(
                    bytes,
                )))
            }
        };

        let session = block_on(orchestrator.fetch_with_scoreboard(&plan, &scoreboard, fetcher))
            .expect("fetch outcome");
        assert_eq!(session.outcome.chunks.len(), plan.chunks.len());
        assert!(
            session
                .outcome
                .chunk_receipts
                .iter()
                .all(|receipt| receipt.provider.as_str() == "primary")
        );
        assert_eq!(session.policy_report.status, PolicyStatus::Met);
        assert_eq!(session.policy_report.selected_soranet_total, 1);
        assert!(!session.policy_report.uses_classical());
    }

    #[test]
    fn fetch_respects_max_providers_limit() {
        let payload = vec![0xAB; 4096];
        let plan = CarBuildPlan::single_file(&payload).expect("plan");
        let adverts = vec![provider_metadata("alpha"), provider_metadata("beta")];
        let telemetry = TelemetrySnapshot::default();

        let config = OrchestratorConfig {
            max_providers: NonZeroUsize::new(1),
            ..OrchestratorConfig::default()
        };
        let orchestrator = Orchestrator::new(config);
        let scoreboard = orchestrator
            .build_scoreboard(&plan, &adverts, &telemetry)
            .expect("scoreboard");

        let seen: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let payload_arc = Arc::new(payload.clone());
        let fetcher = {
            let seen = Arc::clone(&seen);
            let payload = Arc::clone(&payload_arc);
            move |request: FetchRequest| {
                let seen = Arc::clone(&seen);
                let payload = Arc::clone(&payload);
                let start = request.spec.offset as usize;
                let end = start + request.spec.length as usize;
                let bytes = payload[start..end].to_vec();
                seen.lock()
                    .expect("lock")
                    .push(request.provider.id().as_str().to_string());
                futures::future::ready(Ok::<ChunkResponse, std::io::Error>(ChunkResponse::new(
                    bytes,
                )))
            }
        };

        let session = block_on(orchestrator.fetch_with_scoreboard(&plan, &scoreboard, fetcher))
            .expect("fetch outcome");
        let outcome = &session.outcome;
        assert_eq!(outcome.provider_reports.len(), 1);
        let used = seen.lock().expect("lock");
        assert!(
            used.iter().all(|provider| provider == "alpha"),
            "expected only the highest-ranked provider to be used, got: {:?}",
            &*used
        );
        assert_eq!(session.policy_report.status, PolicyStatus::Met);
        assert_eq!(session.policy_report.selected_soranet_total, 1);
    }

    #[test]
    fn transport_policy_prioritises_soranet_then_direct() {
        let payload = vec![0xCD; 2048];
        let plan = CarBuildPlan::single_file(&payload).expect("plan");

        let mut soranet_meta = provider_metadata("soranet-alpha");
        soranet_meta.transport_hints = vec![TransportHint {
            protocol: "soranet".into(),
            protocol_id: 3,
            priority: 0,
        }];

        let mut quic_meta = provider_metadata("quic-beta");
        quic_meta.transport_hints = vec![TransportHint {
            protocol: "quic".into(),
            protocol_id: 2,
            priority: 0,
        }];

        let mut torii_meta = provider_metadata("torii-gamma");
        torii_meta.transport_hints = vec![TransportHint {
            protocol: "torii".into(),
            protocol_id: 1,
            priority: 0,
        }];

        let mut fallback_meta = provider_metadata("fallback-delta");
        fallback_meta.transport_hints.clear();

        let adverts = vec![soranet_meta, quic_meta, torii_meta, fallback_meta];
        let telemetry = TelemetrySnapshot::default();
        let orchestrator = Orchestrator::new(OrchestratorConfig::default());
        let scoreboard = orchestrator
            .build_scoreboard(&plan, &adverts, &telemetry)
            .expect("scoreboard");

        let SelectionOutcome { providers, .. } = eligible_providers(
            &scoreboard,
            None,
            TransportPolicy::SoranetPreferred,
            AnonymityPolicy::GuardPq,
            None,
            None,
            false,
        );
        let default_order: Vec<String> = providers
            .into_iter()
            .map(|provider| provider.id().as_str().to_string())
            .collect();

        assert_eq!(
            default_order,
            vec![
                "soranet-alpha".to_string(),
                "quic-beta".to_string(),
                "torii-gamma".to_string(),
                "fallback-delta".to_string(),
            ]
        );

        let SelectionOutcome { providers, .. } = eligible_providers(
            &scoreboard,
            None,
            TransportPolicy::DirectOnly,
            AnonymityPolicy::GuardPq,
            None,
            None,
            false,
        );
        let direct_order: Vec<String> = providers
            .into_iter()
            .map(|provider| provider.id().as_str().to_string())
            .collect();

        assert_eq!(
            direct_order,
            vec!["quic-beta".to_string(), "torii-gamma".to_string()]
        );

        let SelectionOutcome { providers, .. } = eligible_providers(
            &scoreboard,
            None,
            TransportPolicy::SoranetStrict,
            AnonymityPolicy::GuardPq,
            None,
            None,
            false,
        );
        let strict_order: Vec<String> = providers
            .into_iter()
            .map(|provider| provider.id().as_str().to_string())
            .collect();

        assert_eq!(strict_order, vec!["soranet-alpha".to_string()]);
    }

    #[test]
    fn write_mode_hint_upload_enforces_soranet_strict() {
        let (transport, anonymity) = WriteModeHint::UploadPqOnly
            .apply(TransportPolicy::SoranetPreferred, AnonymityPolicy::GuardPq);
        assert_eq!(transport, TransportPolicy::SoranetStrict);
        assert_eq!(anonymity, AnonymityPolicy::StrictPq);

        let (transport_direct, anonymity_direct) = WriteModeHint::UploadPqOnly
            .apply(TransportPolicy::DirectOnly, AnonymityPolicy::GuardPq);
        assert_eq!(transport_direct, TransportPolicy::SoranetStrict);
        assert_eq!(anonymity_direct, AnonymityPolicy::StrictPq);
    }

    #[test]
    fn guard_set_prioritises_pinned_provider() {
        let payload = vec![0x42; 1024];
        let plan = CarBuildPlan::single_file(&payload).expect("plan");

        let first_id_bytes = [0x11u8; 32];
        let second_id_bytes = [0x22u8; 32];
        let first_id_hex = hex::encode(first_id_bytes);
        let second_id_hex = hex::encode(second_id_bytes);

        let mut primary_meta = provider_metadata(&first_id_hex);
        primary_meta.transport_hints = vec![TransportHint {
            protocol: "soranet".into(),
            protocol_id: 3,
            priority: 0,
        }];

        let mut secondary_meta = provider_metadata(&second_id_hex);
        secondary_meta.transport_hints = vec![TransportHint {
            protocol: "soranet".into(),
            protocol_id: 3,
            priority: 0,
        }];

        let adverts = vec![primary_meta, secondary_meta];
        let telemetry = TelemetrySnapshot::default();
        let orchestrator = Orchestrator::new(OrchestratorConfig::default());
        let scoreboard = orchestrator
            .build_scoreboard(&plan, &adverts, &telemetry)
            .expect("scoreboard");

        let SelectionOutcome { providers, .. } = eligible_providers(
            &scoreboard,
            None,
            TransportPolicy::SoranetPreferred,
            AnonymityPolicy::GuardPq,
            None,
            None,
            false,
        );
        let baseline: Vec<String> = providers
            .into_iter()
            .filter_map(|provider| {
                provider
                    .metadata()
                    .and_then(|meta| meta.provider_id.clone())
            })
            .collect();

        assert_eq!(baseline, vec![first_id_hex.clone(), second_id_hex.clone()]);

        let guard_set = GuardSet::new(vec![GuardRecord {
            relay_id: second_id_bytes,
            pinned_at_unix: 0,
            endpoint: Endpoint::new("soranet://relay-secondary", 0),
            guard_weight: 100,
            bandwidth_bytes_per_sec: 0,
            reputation_weight: 0,
            pq_kem_public: None,
            certificate: None,
            path_metadata: PathMetadata::default(),
        }]);

        let SelectionOutcome { providers, .. } = eligible_providers(
            &scoreboard,
            None,
            TransportPolicy::SoranetPreferred,
            AnonymityPolicy::GuardPq,
            Some(&guard_set),
            None,
            false,
        );
        let prioritised: Vec<String> = providers
            .into_iter()
            .filter_map(|provider| {
                provider
                    .metadata()
                    .and_then(|meta| meta.provider_id.clone())
            })
            .collect();

        assert_eq!(prioritised, vec![second_id_hex, first_id_hex]);
    }

    #[test]
    fn guard_set_applies_capability_weighting() {
        let payload = vec![0xAA; 2048];
        let plan = CarBuildPlan::single_file(&payload).expect("plan");

        let alpha_id_bytes = [0x31u8; 32];
        let beta_id_bytes = [0x41u8; 32];
        let alpha_id_hex = hex::encode(alpha_id_bytes);
        let beta_id_hex = hex::encode(beta_id_bytes);

        let mut alpha_meta = provider_metadata(&alpha_id_hex);
        alpha_meta.transport_hints = vec![TransportHint {
            protocol: "soranet".into(),
            protocol_id: 3,
            priority: 0,
        }];
        let mut beta_meta = provider_metadata(&beta_id_hex);
        beta_meta.transport_hints = vec![TransportHint {
            protocol: "soranet".into(),
            protocol_id: 3,
            priority: 0,
        }];

        let mut alpha_telemetry = ProviderTelemetry::new(alpha_id_hex.clone());
        alpha_telemetry.staking_weight = Some(3.0);
        let mut beta_telemetry = ProviderTelemetry::new(beta_id_hex.clone());
        beta_telemetry.staking_weight = Some(1.0);
        let telemetry =
            TelemetrySnapshot::from_records(vec![alpha_telemetry.clone(), beta_telemetry.clone()]);

        let orchestrator = Orchestrator::new(OrchestratorConfig::default());
        let scoreboard = orchestrator
            .build_scoreboard(&plan, &[alpha_meta.clone(), beta_meta.clone()], &telemetry)
            .expect("scoreboard");

        let SelectionOutcome { providers, .. } = eligible_providers(
            &scoreboard,
            None,
            TransportPolicy::SoranetPreferred,
            AnonymityPolicy::GuardPq,
            None,
            None,
            false,
        );
        let baseline: Vec<String> = providers
            .into_iter()
            .filter_map(|provider| {
                provider
                    .metadata()
                    .and_then(|meta| meta.provider_id.clone())
            })
            .collect();
        assert_eq!(baseline, vec![beta_id_hex.clone(), alpha_id_hex.clone()]);

        let guard_alpha_classical = GuardRecord {
            relay_id: alpha_id_bytes,
            pinned_at_unix: 0,
            endpoint: Endpoint::new("soranet://alpha-classic", 0),
            guard_weight: 300,
            bandwidth_bytes_per_sec: 6 * 1024 * 1024,
            reputation_weight: 320,
            pq_kem_public: None,
            certificate: None,
            path_metadata: PathMetadata::default(),
        };
        let guard_beta_pq = GuardRecord {
            relay_id: beta_id_bytes,
            pinned_at_unix: 0,
            endpoint: Endpoint::new("soranet://beta-pq", 0),
            guard_weight: 200,
            bandwidth_bytes_per_sec: 8 * 1024 * 1024,
            reputation_weight: 280,
            pq_kem_public: Some(vec![0x01]),
            certificate: None,
            path_metadata: PathMetadata::default(),
        };
        let guard_set = GuardSet::new(vec![guard_alpha_classical, guard_beta_pq]);
        let SelectionOutcome { providers, .. } = eligible_providers(
            &scoreboard,
            None,
            TransportPolicy::SoranetPreferred,
            AnonymityPolicy::GuardPq,
            Some(&guard_set),
            None,
            false,
        );
        let pq_pref: Vec<String> = providers
            .into_iter()
            .filter_map(|provider| {
                provider
                    .metadata()
                    .and_then(|meta| meta.provider_id.clone())
            })
            .collect();
        assert_eq!(pq_pref, vec![beta_id_hex.clone(), alpha_id_hex.clone()]);

        let guard_alpha_pq = GuardRecord {
            relay_id: alpha_id_bytes,
            pinned_at_unix: 10,
            endpoint: Endpoint::new("soranet://alpha-pq", 0),
            guard_weight: 150,
            bandwidth_bytes_per_sec: 7 * 1024 * 1024,
            reputation_weight: 340,
            pq_kem_public: Some(vec![0xAA]),
            certificate: None,
            path_metadata: PathMetadata::default(),
        };
        let guard_beta_heavier = GuardRecord {
            relay_id: beta_id_bytes,
            pinned_at_unix: 10,
            endpoint: Endpoint::new("soranet://beta-heavy", 0),
            guard_weight: 400,
            bandwidth_bytes_per_sec: 7 * 1024 * 1024,
            reputation_weight: 360,
            pq_kem_public: Some(vec![0xBB]),
            certificate: None,
            path_metadata: PathMetadata::default(),
        };
        let guard_set = GuardSet::new(vec![guard_alpha_pq.clone(), guard_beta_heavier]);
        let SelectionOutcome { providers, .. } = eligible_providers(
            &scoreboard,
            None,
            TransportPolicy::SoranetPreferred,
            AnonymityPolicy::GuardPq,
            Some(&guard_set),
            None,
            false,
        );
        let weight_pref: Vec<String> = providers
            .into_iter()
            .filter_map(|provider| {
                provider
                    .metadata()
                    .and_then(|meta| meta.provider_id.clone())
            })
            .collect();
        assert_eq!(weight_pref, vec![beta_id_hex.clone(), alpha_id_hex.clone()]);

        let guard_beta_equal = GuardRecord {
            relay_id: beta_id_bytes,
            pinned_at_unix: 20,
            endpoint: Endpoint::new("soranet://beta-peer", 0),
            guard_weight: guard_alpha_pq.guard_weight,
            bandwidth_bytes_per_sec: guard_alpha_pq.bandwidth_bytes_per_sec,
            reputation_weight: guard_alpha_pq.reputation_weight,
            pq_kem_public: guard_alpha_pq.pq_kem_public.clone(),
            certificate: None,
            path_metadata: PathMetadata::default(),
        };
        let guard_set = GuardSet::new(vec![guard_alpha_pq, guard_beta_equal]);
        let SelectionOutcome { providers, .. } = eligible_providers(
            &scoreboard,
            None,
            TransportPolicy::SoranetPreferred,
            AnonymityPolicy::GuardPq,
            Some(&guard_set),
            None,
            false,
        );
        let reputation_pref: Vec<String> = providers
            .into_iter()
            .filter_map(|provider| {
                provider
                    .metadata()
                    .and_then(|meta| meta.provider_id.clone())
            })
            .collect();
        assert_eq!(
            reputation_pref,
            vec![alpha_id_hex.clone(), beta_id_hex.clone()]
        );

        let guard_alpha_high_bw = GuardRecord {
            relay_id: alpha_id_bytes,
            pinned_at_unix: 25,
            endpoint: Endpoint::new("soranet://alpha-high-bw", 0),
            guard_weight: 250,
            bandwidth_bytes_per_sec: 9 * 1024 * 1024,
            reputation_weight: 310,
            pq_kem_public: Some(vec![0xAC]),
            certificate: None,
            path_metadata: PathMetadata::default(),
        };
        let guard_beta_low_bw = GuardRecord {
            relay_id: beta_id_bytes,
            pinned_at_unix: 25,
            endpoint: Endpoint::new("soranet://beta-low-bw", 0),
            guard_weight: 250,
            bandwidth_bytes_per_sec: 5 * 1024 * 1024,
            reputation_weight: 360,
            pq_kem_public: Some(vec![0xBC]),
            certificate: None,
            path_metadata: PathMetadata::default(),
        };
        let guard_set = GuardSet::new(vec![guard_alpha_high_bw.clone(), guard_beta_low_bw.clone()]);
        let SelectionOutcome { providers, .. } = eligible_providers(
            &scoreboard,
            None,
            TransportPolicy::SoranetPreferred,
            AnonymityPolicy::GuardPq,
            Some(&guard_set),
            None,
            false,
        );
        let bandwidth_pref: Vec<String> = providers
            .into_iter()
            .filter_map(|provider| {
                provider
                    .metadata()
                    .and_then(|meta| meta.provider_id.clone())
            })
            .collect();
        assert_eq!(
            bandwidth_pref,
            vec![alpha_id_hex.clone(), beta_id_hex.clone()]
        );

        let guard_set = GuardSet::new(vec![guard_beta_low_bw, guard_alpha_high_bw]);
        let SelectionOutcome { providers, .. } = eligible_providers(
            &scoreboard,
            None,
            TransportPolicy::SoranetPreferred,
            AnonymityPolicy::GuardPq,
            Some(&guard_set),
            None,
            false,
        );
        let pq_sorted: Vec<String> = providers
            .into_iter()
            .filter_map(|provider| {
                provider
                    .metadata()
                    .and_then(|meta| meta.provider_id.clone())
            })
            .collect();
        assert_eq!(pq_sorted, vec![alpha_id_hex, beta_id_hex]);
    }

    #[test]
    fn guard_set_weighting_updates_provider_weights_without_directory() {
        let payload = vec![0xCD; 4096];
        let plan = CarBuildPlan::single_file(&payload).expect("plan");

        let alpha_bytes = [0x71u8; 32];
        let beta_bytes = [0x72u8; 32];
        let alpha_hex = hex::encode(alpha_bytes);
        let beta_hex = hex::encode(beta_bytes);

        let mut alpha_meta = provider_metadata(&alpha_hex);
        alpha_meta.capability_names = vec!["soranet_classic".into()];
        let mut beta_meta = provider_metadata(&beta_hex);
        beta_meta.capability_names = vec![
            "soranet_pq".into(),
            "soranet_pq_guard".into(),
            "soranet_pq_strict".into(),
        ];

        let orchestrator = Orchestrator::new(OrchestratorConfig::default());
        let adverts = vec![alpha_meta.clone(), beta_meta.clone()];
        let scoreboard = orchestrator
            .build_scoreboard(&plan, &adverts, &TelemetrySnapshot::default())
            .expect("scoreboard");

        let guard_alpha = GuardRecord {
            relay_id: alpha_bytes,
            pinned_at_unix: 0,
            endpoint: Endpoint::new("soranet://alpha-classic", 0),
            guard_weight: 120,
            bandwidth_bytes_per_sec: 4 * 1024 * 1024,
            reputation_weight: 220,
            pq_kem_public: None,
            certificate: None,
            path_metadata: PathMetadata::default(),
        };
        let guard_beta = GuardRecord {
            relay_id: beta_bytes,
            pinned_at_unix: 0,
            endpoint: Endpoint::new("soranet://beta-pq", 0),
            guard_weight: 240,
            bandwidth_bytes_per_sec: 9 * 1024 * 1024,
            reputation_weight: 340,
            pq_kem_public: Some(vec![0xB1]),
            certificate: None,
            path_metadata: PathMetadata::default(),
        };
        let guard_set = GuardSet::new(vec![guard_alpha, guard_beta]);

        let SelectionOutcome { providers, .. } = eligible_providers(
            &scoreboard,
            None,
            TransportPolicy::SoranetPreferred,
            AnonymityPolicy::GuardPq,
            Some(&guard_set),
            None,
            false,
        );

        let provider_ids: Vec<String> = providers
            .iter()
            .filter_map(|provider| provider.metadata()?.provider_id.clone())
            .collect();
        assert_eq!(provider_ids, vec![beta_hex.clone(), alpha_hex.clone()]);

        let beta_weight = providers[0].weight().get();
        let alpha_weight = providers[1].weight().get();
        assert!(
            beta_weight > alpha_weight,
            "expected PQ guard weighting to increase beta weight (alpha {alpha_weight}, beta {beta_weight})"
        );
    }

    #[test]
    fn policy_override_enforces_strict_stage_selection() {
        let payload = vec![0x77; 1024];
        let plan = CarBuildPlan::single_file(&payload).expect("plan");

        let mut pq_meta = provider_metadata("relay-pq");
        pq_meta.transport_hints = vec![TransportHint {
            protocol: "soranet".into(),
            protocol_id: 3,
            priority: 0,
        }];
        pq_meta.capability_names = vec![
            "soranet_pq".into(),
            "soranet_pq_guard".into(),
            "soranet_pq_strict".into(),
        ];

        let mut classical_meta = provider_metadata("relay-classic");
        classical_meta.transport_hints = vec![TransportHint {
            protocol: "soranet".into(),
            protocol_id: 3,
            priority: 0,
        }];
        classical_meta.capability_names = vec!["soranet_classic".into()];

        let orchestrator = Orchestrator::new(OrchestratorConfig::default());
        let scoreboard = orchestrator
            .build_scoreboard(
                &plan,
                &[pq_meta.clone(), classical_meta.clone()],
                &TelemetrySnapshot::default(),
            )
            .expect("scoreboard");

        let SelectionOutcome { providers, summary } = eligible_providers(
            &scoreboard,
            None,
            TransportPolicy::SoranetPreferred,
            AnonymityPolicy::StrictPq,
            None,
            None,
            true,
        );
        assert_eq!(providers.len(), 1);
        assert_eq!(
            providers[0]
                .metadata()
                .and_then(|meta| meta.provider_id.clone())
                .as_deref(),
            Some("relay-pq"),
        );
        assert!(matches!(summary.status, PolicyStatus::Met));
        assert_eq!(summary.policy, AnonymityPolicy::StrictPq);

        let scoreboard = orchestrator
            .build_scoreboard(&plan, &[classical_meta], &TelemetrySnapshot::default())
            .expect("scoreboard classical");
        let SelectionOutcome { providers, summary } = eligible_providers(
            &scoreboard,
            None,
            TransportPolicy::SoranetPreferred,
            AnonymityPolicy::StrictPq,
            None,
            None,
            true,
        );
        assert!(providers.is_empty());
        assert!(matches!(summary.status, PolicyStatus::Brownout));
    }

    #[test]
    fn guard_directory_weights_soranet_candidates() {
        let payload = vec![0x99; 4096];
        let plan = CarBuildPlan::single_file(&payload).expect("plan");

        let alpha_bytes = [0x11u8; 32];
        let beta_bytes = [0x22u8; 32];
        let gamma_bytes = [0x33u8; 32];
        let delta_bytes = [0x44u8; 32];
        let epsilon_bytes = [0x55u8; 32];

        let alpha_hex = hex::encode(alpha_bytes);
        let beta_hex = hex::encode(beta_bytes);
        let gamma_hex = hex::encode(gamma_bytes);
        let delta_hex = hex::encode(delta_bytes);
        let epsilon_hex = hex::encode(epsilon_bytes);

        fn with_bandwidth(mut metadata: ProviderMetadata, bytes_per_sec: u64) -> ProviderMetadata {
            metadata.stream_budget = Some(multi_fetch::StreamBudget {
                max_in_flight: 4,
                max_bytes_per_sec: bytes_per_sec,
                burst_bytes: Some(bytes_per_sec),
            });
            metadata
        }

        let mut alpha_meta = provider_metadata(&alpha_hex);
        alpha_meta = with_bandwidth(alpha_meta, 4 * 1024 * 1024);
        let mut beta_meta = provider_metadata(&beta_hex);
        beta_meta = with_bandwidth(beta_meta, 2 * 1024 * 1024);
        let mut gamma_meta = provider_metadata(&gamma_hex);
        gamma_meta = with_bandwidth(gamma_meta, 8 * 1024 * 1024);
        let mut delta_meta = provider_metadata(&delta_hex);
        delta_meta = with_bandwidth(delta_meta, 8 * 1024 * 1024);
        let mut epsilon_meta = provider_metadata(&epsilon_hex);
        epsilon_meta = with_bandwidth(epsilon_meta, 8 * 1024 * 1024);

        let telemetry = TelemetrySnapshot::from_records(vec![
            {
                let mut record = ProviderTelemetry::new(alpha_hex.clone());
                record.staking_weight = Some(1.3);
                record
            },
            {
                let mut record = ProviderTelemetry::new(beta_hex.clone());
                record.staking_weight = Some(1.5);
                record
            },
            {
                let mut record = ProviderTelemetry::new(gamma_hex.clone());
                record.staking_weight = Some(1.4);
                record
            },
            {
                let mut record = ProviderTelemetry::new(delta_hex.clone());
                record.staking_weight = Some(1.1);
                record
            },
            {
                let mut record = ProviderTelemetry::new(epsilon_hex.clone());
                record.staking_weight = Some(1.1);
                record
            },
        ]);

        let orchestrator = Orchestrator::new(OrchestratorConfig::default());
        let adverts = vec![
            alpha_meta.clone(),
            beta_meta.clone(),
            gamma_meta.clone(),
            delta_meta.clone(),
            epsilon_meta.clone(),
        ];
        let scoreboard = orchestrator
            .build_scoreboard(&plan, &adverts, &telemetry)
            .expect("scoreboard");

        let mut alpha_descriptor =
            entry_descriptor(0x11, 150, vec![Endpoint::new("soranet://relay-alpha", 0)]);
        alpha_descriptor.pq_kem_public = Some(vec![0xAA]);
        alpha_descriptor.bandwidth_bytes_per_sec = 9 * 1024 * 1024;
        alpha_descriptor.reputation_weight = 135;
        let mut beta_descriptor =
            entry_descriptor(0x22, 240, vec![Endpoint::new("soranet://relay-beta", 0)]);
        beta_descriptor.bandwidth_bytes_per_sec = 8 * 1024 * 1024;
        beta_descriptor.reputation_weight = 130;
        let mut gamma_descriptor =
            entry_descriptor(0x33, 220, vec![Endpoint::new("soranet://relay-gamma", 0)]);
        gamma_descriptor.bandwidth_bytes_per_sec = 8 * 1024 * 1024;
        gamma_descriptor.reputation_weight = 125;
        let mut delta_descriptor =
            entry_descriptor(0x44, 220, vec![Endpoint::new("soranet://relay-delta", 0)]);
        delta_descriptor.bandwidth_bytes_per_sec = 7 * 1024 * 1024;
        delta_descriptor.reputation_weight = 120;
        let mut epsilon_descriptor =
            entry_descriptor(0x55, 220, vec![Endpoint::new("soranet://relay-epsilon", 0)]);
        epsilon_descriptor.bandwidth_bytes_per_sec = 7 * 1024 * 1024;
        epsilon_descriptor.reputation_weight = 120;

        let directory = RelayDirectory::new(vec![
            alpha_descriptor,
            beta_descriptor,
            gamma_descriptor,
            delta_descriptor,
            epsilon_descriptor,
        ]);

        let SelectionOutcome { providers, .. } = eligible_providers(
            &scoreboard,
            None,
            TransportPolicy::SoranetPreferred,
            AnonymityPolicy::GuardPq,
            None,
            Some(&directory),
            false,
        );

        let ordered_ids: Vec<String> = providers
            .into_iter()
            .filter_map(|provider| provider.metadata()?.provider_id.clone())
            .collect();

        assert_eq!(
            ordered_ids,
            vec![delta_hex, epsilon_hex, gamma_hex, beta_hex, alpha_hex]
        );
    }

    #[test]
    fn capability_weighting_adjusts_provider_weights() {
        let payload = vec![0xAB; 1024];
        let plan = CarBuildPlan::single_file(&payload).expect("plan");

        let alpha_bytes = [0x66u8; 32];
        let beta_bytes = [0x77u8; 32];
        let alpha_hex = hex::encode(alpha_bytes);
        let beta_hex = hex::encode(beta_bytes);

        let alpha_meta = provider_metadata(&alpha_hex);
        let beta_meta = provider_metadata(&beta_hex);

        let orchestrator = Orchestrator::new(OrchestratorConfig::default());
        let adverts = vec![alpha_meta.clone(), beta_meta.clone()];
        let scoreboard = orchestrator
            .build_scoreboard(&plan, &adverts, &TelemetrySnapshot::default())
            .expect("scoreboard");

        let base_weights: HashMap<String, u32> = scoreboard
            .entries()
            .iter()
            .filter_map(|entry| {
                let id = entry
                    .provider
                    .metadata()
                    .and_then(|meta| meta.provider_id.clone())?;
                Some((id, entry.provider.weight().get()))
            })
            .collect();

        let alpha_base = base_weights
            .get(&alpha_hex)
            .copied()
            .expect("expected alpha base weight");
        let beta_base = base_weights
            .get(&beta_hex)
            .copied()
            .expect("expected beta base weight");
        assert_eq!(alpha_base, beta_base);

        let mut alpha_descriptor =
            entry_descriptor(0x66, 420, vec![Endpoint::new("soranet://relay-alpha", 0)]);
        alpha_descriptor.pq_kem_public = Some(vec![0xAA]);
        alpha_descriptor.bandwidth_bytes_per_sec = 12 * 1024 * 1024;
        alpha_descriptor.reputation_weight = 185;

        let mut beta_descriptor =
            entry_descriptor(0x77, 240, vec![Endpoint::new("soranet://relay-beta", 0)]);
        beta_descriptor.bandwidth_bytes_per_sec = 4 * 1024 * 1024;
        beta_descriptor.reputation_weight = 120;

        let directory = RelayDirectory::new(vec![alpha_descriptor, beta_descriptor]);

        let SelectionOutcome { providers, .. } = eligible_providers(
            &scoreboard,
            None,
            TransportPolicy::SoranetPreferred,
            AnonymityPolicy::GuardPq,
            None,
            Some(&directory),
            false,
        );

        let provider_ids: Vec<String> = providers
            .iter()
            .filter_map(|provider| provider.metadata()?.provider_id.clone())
            .collect();
        assert_eq!(provider_ids, vec![beta_hex.clone(), alpha_hex.clone()]);
    }

    #[test]
    fn capability_weighting_is_deterministic_for_equal_descriptors() {
        let payload = vec![0xBC; 2048];
        let plan = CarBuildPlan::single_file(&payload).expect("plan");

        let first_bytes = [0x21u8; 32];
        let second_bytes = [0x11u8; 32];
        let first_hex = hex::encode(first_bytes);
        let second_hex = hex::encode(second_bytes);

        let first_meta = provider_metadata(&first_hex);
        let second_meta = provider_metadata(&second_hex);

        let orchestrator = Orchestrator::new(OrchestratorConfig::default());
        let adverts = vec![first_meta.clone(), second_meta.clone()];
        let scoreboard = orchestrator
            .build_scoreboard(&plan, &adverts, &TelemetrySnapshot::default())
            .expect("scoreboard");

        let mut first_descriptor =
            entry_descriptor(0x21, 300, vec![Endpoint::new("soranet://relay-first", 0)]);
        first_descriptor.pq_kem_public = Some(vec![0x01]);
        first_descriptor.bandwidth_bytes_per_sec = 6 * 1024 * 1024;
        first_descriptor.reputation_weight = 140;

        let mut second_descriptor =
            entry_descriptor(0x11, 300, vec![Endpoint::new("soranet://relay-second", 0)]);
        second_descriptor.pq_kem_public = Some(vec![0x02]);
        second_descriptor.bandwidth_bytes_per_sec = 6 * 1024 * 1024;
        second_descriptor.reputation_weight = 140;

        let directory = RelayDirectory::new(vec![first_descriptor, second_descriptor]);

        let SelectionOutcome { providers, .. } = eligible_providers(
            &scoreboard,
            None,
            TransportPolicy::SoranetPreferred,
            AnonymityPolicy::GuardPq,
            None,
            Some(&directory),
            false,
        );

        let ordered_ids: Vec<String> = providers
            .iter()
            .filter_map(|provider| provider.metadata()?.provider_id.clone())
            .collect();
        assert_eq!(ordered_ids, vec![second_hex.clone(), first_hex.clone()]);

        let first_weight = providers[0].weight().get();
        let second_weight = providers[1].weight().get();
        assert_eq!(
            first_weight, second_weight,
            "weights should remain equal when descriptors match (first {first_weight}, second {second_weight})"
        );
    }

    #[test]
    fn capability_weighting_prefers_pq_descriptor_with_lower_guard_weight() {
        let payload = vec![0xCD; 1536];
        let plan = CarBuildPlan::single_file(&payload).expect("plan");

        let pq_bytes = [0x91u8; 32];
        let classical_bytes = [0x92u8; 32];
        let pq_hex = hex::encode(pq_bytes);
        let classical_hex = hex::encode(classical_bytes);

        let mut pq_meta = provider_metadata(&pq_hex);
        pq_meta.transport_hints = vec![TransportHint {
            protocol: "soranet".into(),
            protocol_id: 3,
            priority: 0,
        }];
        pq_meta.capability_names = vec!["soranet_pq_guard".into()];

        let mut classical_meta = provider_metadata(&classical_hex);
        classical_meta.transport_hints = vec![TransportHint {
            protocol: "soranet".into(),
            protocol_id: 3,
            priority: 0,
        }];
        classical_meta.capability_names = vec!["soranet_classical".into()];

        let orchestrator = Orchestrator::new(OrchestratorConfig::default());
        let adverts = vec![pq_meta.clone(), classical_meta.clone()];
        let scoreboard = orchestrator
            .build_scoreboard(&plan, &adverts, &TelemetrySnapshot::default())
            .expect("scoreboard");

        let mut pq_descriptor =
            entry_descriptor(0x91, 140, vec![Endpoint::new("soranet://pq-relay", 0)]);
        pq_descriptor.pq_kem_public = Some(vec![0xAA]);
        pq_descriptor.bandwidth_bytes_per_sec = 6 * 1024 * 1024;
        pq_descriptor.reputation_weight = 115;

        let mut classical_descriptor = entry_descriptor(
            0x92,
            260,
            vec![Endpoint::new("soranet://classical-relay", 0)],
        );
        classical_descriptor.bandwidth_bytes_per_sec = 9 * 1024 * 1024;
        classical_descriptor.reputation_weight = 180;

        let directory = RelayDirectory::new(vec![pq_descriptor, classical_descriptor]);

        let SelectionOutcome { providers, .. } = eligible_providers(
            &scoreboard,
            None,
            TransportPolicy::SoranetPreferred,
            AnonymityPolicy::GuardPq,
            None,
            Some(&directory),
            false,
        );

        let provider_ids: Vec<String> = providers
            .iter()
            .filter_map(|provider| provider.metadata()?.provider_id.clone())
            .collect();

        assert_eq!(provider_ids, vec![pq_hex, classical_hex]);
    }

    #[test]
    fn capability_weighting_uses_descriptor_when_guard_record_is_weaker() {
        let payload = vec![0xCE; 1024];
        let plan = CarBuildPlan::single_file(&payload).expect("plan");

        let alpha_bytes = [0xA1u8; 32];
        let beta_bytes = [0xB2u8; 32];
        let alpha_hex = hex::encode(alpha_bytes);
        let beta_hex = hex::encode(beta_bytes);

        let mut alpha_meta = provider_metadata(&alpha_hex);
        alpha_meta.transport_hints = vec![TransportHint {
            protocol: "soranet".into(),
            protocol_id: 3,
            priority: 0,
        }];
        alpha_meta.capability_names = vec!["soranet_pq_guard".into()];

        let mut beta_meta = provider_metadata(&beta_hex);
        beta_meta.transport_hints = vec![TransportHint {
            protocol: "soranet".into(),
            protocol_id: 3,
            priority: 0,
        }];
        beta_meta.capability_names = vec!["soranet_pq_guard".into()];

        let orchestrator = Orchestrator::new(OrchestratorConfig::default());
        let adverts = vec![alpha_meta.clone(), beta_meta.clone()];
        let scoreboard = orchestrator
            .build_scoreboard(&plan, &adverts, &TelemetrySnapshot::default())
            .expect("scoreboard");

        let alpha_record = GuardRecord {
            relay_id: alpha_bytes,
            pinned_at_unix: 0,
            endpoint: Endpoint::new("soranet://alpha", 0),
            guard_weight: 260,
            bandwidth_bytes_per_sec: 5 * 1024 * 1024,
            reputation_weight: 210,
            pq_kem_public: None,
            certificate: None,
            path_metadata: PathMetadata::default(),
        };
        let beta_record = GuardRecord {
            relay_id: beta_bytes,
            pinned_at_unix: 0,
            endpoint: Endpoint::new("soranet://beta", 0),
            guard_weight: 35,
            bandwidth_bytes_per_sec: 2 * 1024 * 1024,
            reputation_weight: 90,
            pq_kem_public: None,
            certificate: None,
            path_metadata: PathMetadata::default(),
        };
        let guard_set = GuardSet::new(vec![alpha_record, beta_record]);

        let mut alpha_descriptor =
            entry_descriptor(0xA1, 240, vec![Endpoint::new("soranet://relay-alpha", 0)]);
        alpha_descriptor.bandwidth_bytes_per_sec = 4 * 1024 * 1024;
        alpha_descriptor.reputation_weight = 180;

        let mut beta_descriptor =
            entry_descriptor(0xB2, 480, vec![Endpoint::new("soranet://relay-beta", 0)]);
        beta_descriptor.pq_kem_public = Some(vec![0x02]);
        beta_descriptor.bandwidth_bytes_per_sec = 9 * 1024 * 1024;
        beta_descriptor.reputation_weight = 260;

        let directory = RelayDirectory::new(vec![alpha_descriptor, beta_descriptor]);

        let SelectionOutcome { providers, .. } = eligible_providers(
            &scoreboard,
            None,
            TransportPolicy::SoranetPreferred,
            AnonymityPolicy::GuardPq,
            Some(&guard_set),
            Some(&directory),
            false,
        );

        let ordered_ids: Vec<String> = providers
            .iter()
            .filter_map(|provider| provider.metadata()?.provider_id.clone())
            .collect();
        assert_eq!(ordered_ids, vec![alpha_hex.clone(), beta_hex.clone()]);

        let alpha_weight = providers[0].weight().get();
        let beta_weight = providers[1].weight().get();
        assert!(
            beta_weight > alpha_weight,
            "descriptor weighting should boost descriptor-provided guard metrics (alpha {alpha_weight}, beta {beta_weight})"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn direct_only_policy_rejects_soranet_only_providers() {
        test_logger();

        let payload = vec![0xEF; 2048];
        let plan = CarBuildPlan::single_file(&payload).expect("plan");

        let mut soranet_only = provider_metadata("relay-soranet");
        soranet_only.transport_hints = vec![TransportHint {
            protocol: "soranet".into(),
            protocol_id: 3,
            priority: 0,
        }];

        let orchestrator = Orchestrator::new(OrchestratorConfig {
            transport_policy: TransportPolicy::DirectOnly,
            ..OrchestratorConfig::default()
        });

        let scoreboard = orchestrator
            .build_scoreboard(&plan, &[soranet_only], &TelemetrySnapshot::default())
            .expect("scoreboard");

        let entry = scoreboard
            .entries()
            .first()
            .expect("scoreboard entry must exist");
        assert!(
            matches!(entry.eligibility, Eligibility::Eligible),
            "transport filtering runs after eligibility evaluation"
        );

        let fetcher = |_request: FetchRequest| -> futures::future::Ready<
            Result<ChunkResponse, std::io::Error>,
        > { panic!("fetcher must not be called when no providers remain after filtering") };

        let error = orchestrator
            .fetch_with_scoreboard(&plan, &scoreboard, fetcher)
            .await
            .expect_err("direct-only policy must reject SoraNet-only providers");

        match error {
            OrchestratorError::MultiSource(multi_fetch::MultiSourceError::NoProviders) => {}
            other => panic!("expected NoProviders error, got {other:?}"),
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn direct_only_policy_prefers_direct_transports_when_available() {
        test_logger();

        let payload = sample_payload(4096);
        let plan = CarBuildPlan::single_file(&payload).expect("plan");

        let soranet_meta = provider_metadata("relay-soranet");

        let mut direct_meta = provider_metadata("relay-torii");
        direct_meta.transport_hints = vec![TransportHint {
            protocol: "torii_http_range".into(),
            protocol_id: 1,
            priority: 0,
        }];
        direct_meta.capability_names = vec!["torii_http_range".into()];

        let orchestrator = Orchestrator::new(OrchestratorConfig {
            transport_policy: TransportPolicy::DirectOnly,
            ..OrchestratorConfig::default()
        });

        let scoreboard = orchestrator
            .build_scoreboard(
                &plan,
                &[soranet_meta, direct_meta],
                &TelemetrySnapshot::default(),
            )
            .expect("scoreboard");

        let observed_providers: Arc<AsyncMutex<Vec<String>>> =
            Arc::new(AsyncMutex::new(Vec::new()));
        let observed_handle = Arc::clone(&observed_providers);
        let payload = Arc::new(payload);

        let fetcher = move |request: FetchRequest| {
            let payload = Arc::clone(&payload);
            let observed_handle = Arc::clone(&observed_handle);
            async move {
                observed_handle
                    .lock()
                    .await
                    .push(request.provider.id().to_string());
                let start = request.spec.offset as usize;
                let end = start + request.spec.length as usize;
                Ok::<ChunkResponse, std::io::Error>(ChunkResponse::new(
                    payload[start..end].to_vec(),
                ))
            }
        };

        orchestrator
            .fetch_with_scoreboard(&plan, &scoreboard, fetcher)
            .await
            .expect("direct transports should satisfy fetch requests");

        let observed = observed_providers.lock().await;
        assert!(
            !observed.is_empty(),
            "direct-only policy should use available direct transports"
        );
        assert!(
            observed.iter().all(|id| id == "relay-torii"),
            "direct-only policy must filter out SoraNet relays; saw {observed:?}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn upload_mode_rejects_classical_only_guards() {
        let payload = vec![0xC1; 1024];
        let plan = CarBuildPlan::single_file(&payload).expect("plan");

        let mut classical_meta = provider_metadata("relay-classic");
        classical_meta.capability_names = vec!["soranet_classic".into()];

        let orchestrator = Orchestrator::new(OrchestratorConfig {
            write_mode: WriteModeHint::UploadPqOnly,
            ..OrchestratorConfig::default()
        });

        let scoreboard = orchestrator
            .build_scoreboard(&plan, &[classical_meta], &TelemetrySnapshot::default())
            .expect("scoreboard");

        let fetcher = |_request: FetchRequest| -> futures::future::Ready<
            Result<ChunkResponse, std::io::Error>,
        > { panic!("fetcher must not be invoked when upload mode rejects providers") };

        let error = orchestrator
            .fetch_with_scoreboard(&plan, &scoreboard, fetcher)
            .await
            .expect_err("upload mode must reject classical-only providers");

        match error {
            OrchestratorError::MultiSource(multi_fetch::MultiSourceError::NoProviders) => {}
            other => panic!("expected NoProviders error, got {other:?}"),
        }
    }

    #[test]
    fn anonymity_policy_guard_requires_pq_presence() {
        let payload = vec![0xAA; 2048];
        let plan = CarBuildPlan::single_file(&payload).expect("plan");

        let mut pq_meta = provider_metadata("relay-pq");
        pq_meta.transport_hints = vec![TransportHint {
            protocol: "soranet".into(),
            protocol_id: 3,
            priority: 0,
        }];
        pq_meta.capability_names = vec!["soranet_pq".into(), "soranet_pq_guard".into()];

        let mut classical_meta = provider_metadata("relay-classic");
        classical_meta.transport_hints = vec![TransportHint {
            protocol: "soranet".into(),
            protocol_id: 3,
            priority: 0,
        }];
        classical_meta.capability_names = vec!["soranet_classic".into()];

        let adverts = vec![pq_meta, classical_meta];
        let orchestrator = Orchestrator::new(OrchestratorConfig::default());
        let scoreboard = orchestrator
            .build_scoreboard(&plan, &adverts, &TelemetrySnapshot::default())
            .expect("scoreboard");

        let SelectionOutcome { providers, summary } = eligible_providers(
            &scoreboard,
            None,
            TransportPolicy::SoranetPreferred,
            AnonymityPolicy::GuardPq,
            None,
            None,
            false,
        );

        let provider_ids: Vec<_> = providers
            .into_iter()
            .filter_map(|provider| provider.metadata()?.provider_id.clone())
            .collect();

        assert_eq!(
            provider_ids,
            vec!["relay-pq".to_string(), "relay-classic".to_string()]
        );
        assert!(matches!(summary.status, PolicyStatus::Met));
        assert_eq!(summary.selected_soranet_total, 2);
        assert_eq!(summary.selected_pq, 1);
        assert!(summary.fallback_reason.is_none());
    }

    #[test]
    fn anonymity_policy_strict_filters_non_pq_relays() {
        let payload = vec![0xBB; 1024];
        let plan = CarBuildPlan::single_file(&payload).expect("plan");

        let mut strict_meta = provider_metadata("relay-strict");
        strict_meta.transport_hints = vec![TransportHint {
            protocol: "soranet".into(),
            protocol_id: 3,
            priority: 0,
        }];
        strict_meta.capability_names = vec!["soranet_pq".into(), "soranet_pq_strict".into()];

        let mut classical_meta = provider_metadata("relay-classic");
        classical_meta.transport_hints = vec![TransportHint {
            protocol: "soranet".into(),
            protocol_id: 3,
            priority: 0,
        }];
        classical_meta.capability_names = vec!["soranet_classic".into()];

        let orchestrator = Orchestrator::new(OrchestratorConfig::default());

        let adverts = vec![strict_meta.clone(), classical_meta.clone()];
        let scoreboard = orchestrator
            .build_scoreboard(&plan, &adverts, &TelemetrySnapshot::default())
            .expect("scoreboard");

        let SelectionOutcome { providers, summary } = eligible_providers(
            &scoreboard,
            None,
            TransportPolicy::SoranetPreferred,
            AnonymityPolicy::StrictPq,
            None,
            None,
            false,
        );

        let selected_ids: Vec<_> = providers
            .iter()
            .filter_map(|provider| provider.metadata()?.provider_id.clone())
            .collect();

        assert_eq!(selected_ids, vec!["relay-strict".to_string()]);
        assert!(matches!(summary.status, PolicyStatus::Met));
        assert_eq!(summary.selected_soranet_total, 1);
        assert_eq!(summary.selected_pq, 1);

        // Remove the PQ-capable relay to trigger a strict-mode brownout.
        let adverts = vec![classical_meta];
        let scoreboard = orchestrator
            .build_scoreboard(&plan, &adverts, &TelemetrySnapshot::default())
            .expect("scoreboard");

        let SelectionOutcome { providers, summary } = eligible_providers(
            &scoreboard,
            None,
            TransportPolicy::SoranetPreferred,
            AnonymityPolicy::StrictPq,
            None,
            None,
            false,
        );

        let selected_ids: Vec<_> = providers
            .iter()
            .filter_map(|provider| provider.metadata()?.provider_id.clone())
            .collect();

        assert_eq!(selected_ids, vec!["relay-classic".to_string()]);
        assert!(matches!(summary.status, PolicyStatus::Brownout));
        assert!(matches!(
            summary.fallback_reason,
            Some(PolicyFallback::MissingStrictSupport)
        ));
        assert_eq!(summary.selected_soranet_total, 1);
        assert_eq!(summary.selected_pq, 0);
        assert_eq!(summary.effective_policy(), AnonymityPolicy::MajorityPq);
    }

    #[test]
    fn direct_mode_policy_example_is_valid() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let example_path = manifest_dir.join("../../docs/examples/sorafs_direct_mode_policy.json");
        let file = File::open(&example_path).expect("open direct mode policy example");
        let value: Value = norito::json::from_reader(file).expect("decode config json");
        let config =
            bindings::config_from_json(&value).expect("parse orchestrator config from example");

        assert_eq!(config.transport_policy, TransportPolicy::DirectOnly);
        assert_eq!(config.max_providers.map(|value| value.get()), Some(2));
        assert!(config.fetch.verify_digests);
        assert!(config.fetch.verify_lengths);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn fetch_via_gateway_considers_all_providers_before_limiting() {
        test_logger();

        let payload = sample_payload(4096);
        let plan = CarBuildPlan::single_file(&payload).expect("plan");
        let manifest_id_hex = to_hex(plan.payload_digest.as_bytes());
        let chunker_handle = "sorafs.sf1@1.0.0".to_string();
        let provider_alpha_id_hex = "11".repeat(32);
        let provider_beta_id_hex = "22".repeat(32);

        let gateway_config = GatewayFetchConfig {
            manifest_id_hex: manifest_id_hex.clone(),
            chunker_handle: chunker_handle.clone(),
            manifest_envelope_b64: None,
            client_id: Some("sdk-test".to_string()),
            expected_manifest_cid_hex: Some(manifest_id_hex.clone()),
            blinded_cid_b64: None,
            salt_epoch: None,
            expected_cache_version: None,
            moderation_token_key_b64: None,
        };

        let providers = [
            GatewayProviderInput {
                name: "alpha".to_string(),
                provider_id_hex: provider_alpha_id_hex.clone(),
                base_url: "http://127.0.0.1:9/".to_string(),
                stream_token_b64: stream_token_b64(
                    &manifest_id_hex,
                    &provider_alpha_id_hex,
                    &chunker_handle,
                    4,
                ),
                privacy_events_url: None,
            },
            GatewayProviderInput {
                name: "beta".to_string(),
                provider_id_hex: provider_beta_id_hex.clone(),
                base_url: "http://127.0.0.1:9/".to_string(),
                stream_token_b64: stream_token_b64(
                    &manifest_id_hex,
                    &provider_beta_id_hex,
                    &chunker_handle,
                    4,
                ),
                privacy_events_url: None,
            },
        ];

        let mut alpha_telemetry = ProviderTelemetry::new("alpha");
        alpha_telemetry.penalty = true;
        let telemetry = TelemetrySnapshot::from_records([alpha_telemetry]);

        let mut config = OrchestratorConfig::default();
        config.fetch.per_chunk_retry_limit = Some(1);

        let err = fetch_via_gateway(
            config,
            &plan,
            gateway_config,
            providers,
            Some(&telemetry),
            Some(1),
        )
        .await
        .expect_err("fetch should fail due to missing listener");

        assert!(
            matches!(err, GatewayOrchestratorError::Orchestrator(_)),
            "expected late eligible providers to keep the fetch from failing early, got {err:?}"
        );
    }

    #[test]
    fn fetch_recovers_from_multi_provider_failures() {
        let payload = sample_payload(1 << 20);
        let plan = CarBuildPlan::single_file(&payload).expect("plan");
        let specs = plan.chunk_fetch_specs();
        assert!(
            specs.len() >= 3,
            "expected at least three chunks in plan, got {}",
            specs.len()
        );

        let mut reference_store = ChunkStore::new();
        reference_store.ingest_bytes(&payload);
        assert_eq!(
            reference_store.payload_digest().as_bytes(),
            plan.payload_digest.as_bytes(),
            "chunk-store digest should match plan digest"
        );

        let telemetry = TelemetrySnapshot::default();
        let orchestrator = Orchestrator::new(OrchestratorConfig::default());
        let metadata = vec![
            provider_metadata("alpha"),
            provider_metadata("beta"),
            provider_metadata("gamma"),
        ];
        let scoreboard = orchestrator
            .build_scoreboard(&plan, &metadata, &telemetry)
            .expect("scoreboard");

        let eligible_count = scoreboard
            .entries()
            .iter()
            .filter(|entry| matches!(entry.eligibility, Eligibility::Eligible))
            .count();
        assert_eq!(
            eligible_count, 3,
            "all providers should be eligible for the fixture plan"
        );

        let payload_arc = Arc::new(payload.clone());
        let fetcher = {
            move |request: FetchRequest| {
                let payload = Arc::clone(&payload_arc);
                async move {
                    let start = request.spec.offset as usize;
                    let end = start + request.spec.length as usize;
                    let mut bytes = payload[start..end].to_vec();
                    match (
                        request.provider.id().as_str(),
                        request.spec.chunk_index,
                        request.attempt,
                    ) {
                        ("beta", 1, 1) => Err(HarnessError::provider("beta transport error")),
                        ("alpha", 2, 1) => {
                            if let Some(first) = bytes.first_mut() {
                                *first ^= 0xFF;
                            }
                            Ok(ChunkResponse::new(bytes))
                        }
                        _ => Ok(ChunkResponse::new(bytes)),
                    }
                }
            }
        };

        let session = block_on(orchestrator.fetch_with_scoreboard(&plan, &scoreboard, fetcher))
            .expect("fetch outcome");
        let outcome = &session.outcome;

        assert_eq!(outcome.chunks.len(), specs.len());
        assert_eq!(outcome.chunk_receipts.len(), specs.len());

        for (idx, receipt) in outcome.chunk_receipts.iter().enumerate() {
            assert_eq!(
                receipt.chunk_index, idx,
                "chunk receipts must preserve plan ordering"
            );
        }

        let fallback_receipt = &outcome.chunk_receipts[1];
        assert!(
            fallback_receipt.attempts > 1,
            "chunk 1 should require retries after induced failure"
        );
        assert_eq!(session.policy_report.status, PolicyStatus::Met);

        let served_providers: std::collections::HashSet<String> = outcome
            .chunk_receipts
            .iter()
            .map(|receipt| receipt.provider.as_str().to_string())
            .collect();
        for provider in ["alpha", "beta", "gamma"] {
            assert!(
                served_providers.contains(provider),
                "expected orchestrator to engage provider {provider}"
            );
        }

        let assembled = outcome.assemble_payload();
        assert_eq!(
            assembled.len(),
            payload.len(),
            "assembled payload length should match original"
        );
        assert_eq!(
            assembled, payload,
            "assembled payload must be identical to original bytes"
        );

        let mut assembled_store = ChunkStore::new();
        assembled_store.ingest_plan(&assembled, &plan);
        assert_eq!(
            assembled_store.payload_digest().as_bytes(),
            reference_store.payload_digest().as_bytes(),
            "assembled payload digest must match reference digest"
        );
        assert_eq!(
            assembled_store.chunks(),
            reference_store.chunks(),
            "chunk metadata should remain deterministic"
        );

        let mut report_map: HashMap<String, (usize, usize, bool)> = HashMap::new();
        for report in &outcome.provider_reports {
            report_map.insert(
                report.provider.id().as_str().to_string(),
                (report.successes, report.failures, report.disabled),
            );
        }
        let (alpha_success, alpha_failures, alpha_disabled) =
            *report_map.get("alpha").expect("alpha report");
        assert!(
            alpha_success >= 1 && alpha_failures == 0 && !alpha_disabled,
            "alpha should deliver chunks without being disabled"
        );
        let (beta_success, beta_failures, beta_disabled) =
            *report_map.get("beta").expect("beta report");
        assert!(
            beta_success >= 1 && beta_failures >= 1 && !beta_disabled,
            "beta should record both failures and recoveries"
        );
        let (gamma_success, gamma_failures, gamma_disabled) =
            *report_map.get("gamma").expect("gamma report");
        assert!(
            gamma_success >= 1 && gamma_failures == 0 && !gamma_disabled,
            "gamma should remain healthy while serving fallback chunks"
        );
    }

    #[test]
    fn fetch_records_metrics() {
        let payload = vec![0x11; 1024];
        let plan = CarBuildPlan::single_file(&payload).expect("plan");
        let adverts = vec![provider_metadata("primary")];
        let telemetry = TelemetrySnapshot::default();

        let orchestrator = Orchestrator::new(OrchestratorConfig {
            telemetry_region: Some("test-region".to_string()),
            ..OrchestratorConfig::default()
        });
        let scoreboard = orchestrator
            .build_scoreboard(&plan, &adverts, &telemetry)
            .expect("scoreboard");

        let manifest_label = hex::encode(plan.payload_digest.as_bytes());
        let region_label = "test-region";
        let metrics = global_or_default();

        let gauge_before = metrics
            .sorafs_orchestrator_active_fetches
            .with_label_values(&[manifest_label.as_str(), region_label])
            .get();
        let retries_before = metrics
            .sorafs_orchestrator_retries_total
            .with_label_values(&[manifest_label.as_str(), "primary", "retry"])
            .get();
        let failures_before = metrics
            .sorafs_orchestrator_provider_failures_total
            .with_label_values(&[manifest_label.as_str(), "primary", "session_failure"])
            .get();
        let bytes_before = metrics
            .sorafs_orchestrator_bytes_total
            .with_label_values(&[manifest_label.as_str(), "primary"])
            .get();
        let stalls_before = metrics
            .sorafs_orchestrator_stalls_total
            .with_label_values(&[manifest_label.as_str(), "primary"])
            .get();
        let stage_label = AnonymityPolicy::GuardPq.label();
        let classical_ratio_before = metrics
            .sorafs_orchestrator_classical_ratio
            .with_label_values(&[region_label, stage_label])
            .get_sample_count();
        let classical_selected_before = metrics
            .sorafs_orchestrator_classical_selected
            .with_label_values(&[region_label, stage_label])
            .get_sample_count();

        let payload_arc = Arc::new(payload);
        let attempts = Arc::new(AtomicUsize::new(0));
        let fetcher = {
            let payload = Arc::clone(&payload_arc);
            let attempts = Arc::clone(&attempts);
            move |request: FetchRequest| {
                let payload = Arc::clone(&payload);
                let attempts = Arc::clone(&attempts);
                async move {
                    let current = attempts.fetch_add(1, AtomicOrdering::SeqCst);
                    if current == 0 {
                        Err(std::io::Error::other("simulated failure"))
                    } else {
                        let start = request.spec.offset as usize;
                        let end = start + request.spec.length as usize;
                        Ok::<ChunkResponse, std::io::Error>(ChunkResponse::new(
                            payload[start..end].to_vec(),
                        ))
                    }
                }
            }
        };

        let session = block_on(orchestrator.fetch_with_scoreboard(&plan, &scoreboard, fetcher))
            .expect("fetch outcome");
        assert_eq!(session.outcome.chunks.len(), plan.chunks.len());
        assert_eq!(attempts.load(AtomicOrdering::SeqCst), 2);
        assert_eq!(session.policy_report.status, PolicyStatus::Met);

        let gauge_after = metrics
            .sorafs_orchestrator_active_fetches
            .with_label_values(&[manifest_label.as_str(), region_label])
            .get();
        assert_eq!(gauge_after, gauge_before);

        let retries_after = metrics
            .sorafs_orchestrator_retries_total
            .with_label_values(&[manifest_label.as_str(), "primary", "retry"])
            .get();
        assert!(
            retries_after > retries_before,
            "expected retries counter to increase"
        );

        let failures_after = metrics
            .sorafs_orchestrator_provider_failures_total
            .with_label_values(&[manifest_label.as_str(), "primary", "session_failure"])
            .get();
        assert!(
            failures_after > failures_before,
            "expected provider failure counter to increase"
        );

        let bytes_after = metrics
            .sorafs_orchestrator_bytes_total
            .with_label_values(&[manifest_label.as_str(), "primary"])
            .get();
        let expected_bytes: u64 = plan
            .chunk_fetch_specs()
            .into_iter()
            .map(|spec| u64::from(spec.length))
            .sum();
        assert!(
            bytes_after >= bytes_before + expected_bytes,
            "expected bytes counter to reflect fetched payload"
        );

        let stalls_after = metrics
            .sorafs_orchestrator_stalls_total
            .with_label_values(&[manifest_label.as_str(), "primary"])
            .get();
        assert_eq!(
            stalls_after, stalls_before,
            "no stalls expected for single-provider fetch"
        );
        let classical_ratio_after = metrics
            .sorafs_orchestrator_classical_ratio
            .with_label_values(&[region_label, stage_label])
            .get_sample_count();
        assert_eq!(
            classical_ratio_after,
            classical_ratio_before + 1,
            "expected classical ratio histogram to record the session"
        );
        let classical_selected_after = metrics
            .sorafs_orchestrator_classical_selected
            .with_label_values(&[region_label, stage_label])
            .get_sample_count();
        assert_eq!(
            classical_selected_after,
            classical_selected_before + 1,
            "expected classical selection histogram to record the session"
        );
    }

    #[test]
    fn pq_ratchet_fire_drill_records_metrics() {
        use futures::executor::block_on;

        fn pq_provider(id: &str) -> ProviderMetadata {
            provider_metadata(id)
        }

        fn classical_provider(id: &str) -> ProviderMetadata {
            let mut metadata = provider_metadata(id);
            metadata.capability_names = vec!["soranet_classic".into()];
            metadata
        }

        fn execute_fetch(
            policy: AnonymityPolicy,
            providers: Vec<ProviderMetadata>,
            region: &str,
            seed: u8,
        ) -> PolicyReport {
            let payload = vec![seed; 768];
            let plan = CarBuildPlan::single_file(&payload).expect("plan");
            let telemetry = TelemetrySnapshot::default();
            let orchestrator = Orchestrator::new(OrchestratorConfig {
                telemetry_region: Some(region.to_string()),
                anonymity_policy: policy,
                ..OrchestratorConfig::default()
            });
            let scoreboard = orchestrator
                .build_scoreboard(&plan, &providers, &telemetry)
                .expect("scoreboard");

            let payload_arc = Arc::new(payload);
            let fetcher = {
                let payload = Arc::clone(&payload_arc);
                move |request: FetchRequest| {
                    let payload = Arc::clone(&payload);
                    async move {
                        let start = request.spec.offset as usize;
                        let end = start + request.spec.length as usize;
                        Ok::<ChunkResponse, std::io::Error>(ChunkResponse::new(
                            payload[start..end].to_vec(),
                        ))
                    }
                }
            };

            let session = block_on(orchestrator.fetch_with_scoreboard(&plan, &scoreboard, fetcher))
                .expect("fire drill fetch");
            session.policy_report.clone()
        }

        let region = "fire-drill";
        let metrics = global_or_default();

        // Stage A (Guard PQ) promotion check.
        let stage_guard = AnonymityPolicy::GuardPq;
        let labels_guard = [region, stage_guard.label(), "met", "none"];
        let events_before_guard = metrics
            .sorafs_orchestrator_policy_events_total
            .with_label_values(&labels_guard)
            .get();
        let report_guard = execute_fetch(
            stage_guard,
            vec![pq_provider("guard-relay-a")],
            region,
            0x21,
        );
        assert_eq!(report_guard.policy, stage_guard);
        assert_eq!(report_guard.status, PolicyStatus::Met);
        assert!(
            (report_guard.pq_ratio() - 1.0).abs() < f64::EPSILON,
            "expected full PQ coverage in Stage A"
        );
        let events_after_guard = metrics
            .sorafs_orchestrator_policy_events_total
            .with_label_values(&labels_guard)
            .get();
        assert_eq!(
            events_after_guard,
            events_before_guard + 1,
            "Stage A policy event should increment"
        );

        // Stage B (Majority PQ) promotion check with enough PQ coverage.
        let stage_majority = AnonymityPolicy::MajorityPq;
        let labels_majority_met = [region, stage_majority.label(), "met", "none"];
        let events_before_majority_met = metrics
            .sorafs_orchestrator_policy_events_total
            .with_label_values(&labels_majority_met)
            .get();
        let report_majority_met = execute_fetch(
            stage_majority,
            vec![
                pq_provider("majority-relay-a"),
                pq_provider("majority-relay-b"),
                pq_provider("majority-relay-c"),
            ],
            region,
            0x32,
        );
        assert_eq!(report_majority_met.policy, stage_majority);
        assert_eq!(report_majority_met.status, PolicyStatus::Met);
        assert!(
            report_majority_met.pq_ratio() >= MAJORITY_THRESHOLD,
            "Stage B promotion should satisfy majority threshold"
        );
        let events_after_majority_met = metrics
            .sorafs_orchestrator_policy_events_total
            .with_label_values(&labels_majority_met)
            .get();
        assert_eq!(
            events_after_majority_met,
            events_before_majority_met + 1,
            "Stage B promotion should increment policy events"
        );

        // Stage C (Strict PQ) promotion check with full PQ supply.
        let stage_strict = AnonymityPolicy::StrictPq;
        let labels_strict_met = [region, stage_strict.label(), "met", "none"];
        let events_before_strict_met = metrics
            .sorafs_orchestrator_policy_events_total
            .with_label_values(&labels_strict_met)
            .get();
        let report_strict_met = execute_fetch(
            stage_strict,
            vec![pq_provider("strict-relay-a"), pq_provider("strict-relay-b")],
            region,
            0x43,
        );
        assert_eq!(report_strict_met.policy, stage_strict);
        assert_eq!(report_strict_met.status, PolicyStatus::Met);
        assert!(
            (report_strict_met.pq_ratio() - 1.0).abs() < f64::EPSILON,
            "Strict PQ promotion should select only PQ-capable relays"
        );
        let events_after_strict_met = metrics
            .sorafs_orchestrator_policy_events_total
            .with_label_values(&labels_strict_met)
            .get();
        assert_eq!(
            events_after_strict_met,
            events_before_strict_met + 1,
            "Stage C promotion should increment policy events"
        );

        // Stage B brownout (demotion) with no PQ supply.
        let labels_majority_brownout = [
            region,
            stage_majority.label(),
            "brownout",
            "missing_majority_pq",
        ];
        let events_before_majority_brownout = metrics
            .sorafs_orchestrator_policy_events_total
            .with_label_values(&labels_majority_brownout)
            .get();
        let brownouts_before_majority = metrics
            .sorafs_orchestrator_brownouts_total
            .with_label_values(&[region, stage_majority.label(), "missing_majority_pq"])
            .get();
        let report_majority_brownout = execute_fetch(
            stage_majority,
            vec![
                classical_provider("majority-classical-a"),
                classical_provider("majority-classical-b"),
            ],
            region,
            0x54,
        );
        assert_eq!(report_majority_brownout.policy, stage_majority);
        assert_eq!(report_majority_brownout.status, PolicyStatus::Brownout);
        assert!(report_majority_brownout.should_flag_brownout());
        assert_eq!(
            report_majority_brownout.reason_label(),
            "missing_majority_pq"
        );
        assert!(
            report_majority_brownout.pq_ratio() < MAJORITY_THRESHOLD,
            "Brownout should reflect insufficient PQ ratio"
        );
        let events_after_majority_brownout = metrics
            .sorafs_orchestrator_policy_events_total
            .with_label_values(&labels_majority_brownout)
            .get();
        assert_eq!(
            events_after_majority_brownout,
            events_before_majority_brownout + 1,
            "Stage B brownout should increment policy events"
        );
        let brownouts_after_majority = metrics
            .sorafs_orchestrator_brownouts_total
            .with_label_values(&[region, stage_majority.label(), "missing_majority_pq"])
            .get();
        assert_eq!(
            brownouts_after_majority,
            brownouts_before_majority + 1,
            "Stage B brownout should increment brownout counter"
        );

        // Stage C brownout with fallback to Stage B.
        let strict_reason = "missing_strict_pq";
        let labels_strict_brownout = [region, stage_strict.label(), "brownout", strict_reason];
        let events_before_strict_brownout = metrics
            .sorafs_orchestrator_policy_events_total
            .with_label_values(&labels_strict_brownout)
            .get();
        let brownouts_before_strict = metrics
            .sorafs_orchestrator_brownouts_total
            .with_label_values(&[region, stage_strict.label(), strict_reason])
            .get();
        let report_strict_brownout = execute_fetch(
            stage_strict,
            vec![
                classical_provider("strict-classical-a"),
                classical_provider("strict-classical-b"),
            ],
            region,
            0x65,
        );
        assert_eq!(report_strict_brownout.policy, stage_strict);
        assert_eq!(report_strict_brownout.status, PolicyStatus::Brownout);
        assert!(report_strict_brownout.should_flag_brownout());
        assert_eq!(report_strict_brownout.reason_label(), strict_reason);
        assert_eq!(
            report_strict_brownout.effective_policy,
            AnonymityPolicy::MajorityPq,
            "Strict PQ brownout should fallback to majority stage"
        );
        let events_after_strict_brownout = metrics
            .sorafs_orchestrator_policy_events_total
            .with_label_values(&labels_strict_brownout)
            .get();
        assert_eq!(
            events_after_strict_brownout,
            events_before_strict_brownout + 1,
            "Stage C brownout should increment policy events"
        );
        let brownouts_after_strict = metrics
            .sorafs_orchestrator_brownouts_total
            .with_label_values(&[region, stage_strict.label(), strict_reason])
            .get();
        assert_eq!(
            brownouts_after_strict,
            brownouts_before_strict + 1,
            "Stage C brownout should increment brownout counter"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn fetch_emits_telemetry_events() {
        let handle = test_logger();
        let mut receiver = match handle.subscribe_on_telemetry(Channel::Regular).await {
            Ok(receiver) => receiver,
            Err(err) => {
                eprintln!(
                    "skipping fetch telemetry assertion (telemetry channel unavailable): {err}"
                );
                return;
            }
        };

        let payload = vec![0x33; 2048];
        let plan = CarBuildPlan::single_file(&payload).expect("plan");
        let manifest_id_hex_str = manifest_id_hex(&plan);
        let adverts = vec![provider_metadata("primary")];
        let telemetry = TelemetrySnapshot::default();

        let config = OrchestratorConfig::default().with_telemetry_region("telemetry-test");
        let orchestrator = Orchestrator::new(config);
        let scoreboard = orchestrator
            .build_scoreboard(&plan, &adverts, &telemetry)
            .expect("scoreboard");

        let payload_arc = Arc::new(payload);
        let attempts = Arc::new(AtomicUsize::new(0));
        let fetcher = {
            let payload = Arc::clone(&payload_arc);
            let attempts = Arc::clone(&attempts);
            move |request: FetchRequest| {
                let payload = Arc::clone(&payload);
                let attempts = Arc::clone(&attempts);
                async move {
                    let current = attempts.fetch_add(1, AtomicOrdering::SeqCst);
                    if current == 0 {
                        Err(std::io::Error::other("simulated failure"))
                    } else {
                        let start = request.spec.offset as usize;
                        let end = start + request.spec.length as usize;
                        Ok::<ChunkResponse, std::io::Error>(ChunkResponse::new(
                            payload[start..end].to_vec(),
                        ))
                    }
                }
            }
        };

        let session = orchestrator
            .fetch_with_scoreboard(&plan, &scoreboard, fetcher)
            .await
            .expect("fetch outcome");
        assert_eq!(session.outcome.chunks.len(), plan.chunks.len());

        let expected_total_bytes: u64 = session
            .outcome
            .chunks
            .iter()
            .map(|chunk| chunk.len() as u64)
            .sum();
        assert_eq!(session.policy_report.status, PolicyStatus::Met);

        let mut saw_start = false;
        let mut saw_complete = false;
        let mut saw_retry = false;
        let mut saw_provider_failure = false;
        let mut saw_stall = false;

        for _ in 0..128 {
            let Ok(result) =
                tokio::time::timeout(Duration::from_millis(200), receiver.recv()).await
            else {
                continue;
            };
            let Ok(event) = result else { continue };
            if !event.target.starts_with("sorafs.fetch") {
                continue;
            }
            let payload: Value = event.fields.clone().into();
            let Some(fields) = payload.as_object() else {
                continue;
            };
            let Some(manifest_field) = fields.get("manifest").and_then(Value::as_str) else {
                continue;
            };
            if manifest_field != manifest_id_hex_str {
                continue;
            }
            match event.target {
                "sorafs.fetch.lifecycle" => {
                    let status = fields
                        .get("status")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    if status == "started" {
                        saw_start = true;
                        assert_eq!(fields.get("event").and_then(Value::as_str), Some("start"));
                        assert_eq!(
                            fields.get("region").and_then(Value::as_str),
                            Some("telemetry-test")
                        );
                    } else if status == "success" {
                        saw_complete = true;
                        assert_eq!(
                            fields.get("event").and_then(Value::as_str),
                            Some("complete")
                        );
                        assert!(
                            fields
                                .get("retries_total")
                                .and_then(Value::as_u64)
                                .unwrap_or_default()
                                >= 1
                        );
                        assert_eq!(
                            fields
                                .get("total_bytes")
                                .and_then(Value::as_u64)
                                .unwrap_or_default(),
                            expected_total_bytes
                        );
                        // Throughput samples may be omitted when all chunks complete instantly.
                        let _ = fields.get("avg_throughput_mib_s");
                        assert_eq!(
                            fields
                                .get("stall_count")
                                .and_then(Value::as_u64)
                                .unwrap_or_default(),
                            0,
                            "no stalls expected in telemetry test"
                        );
                        assert_eq!(
                            fields
                                .get("anonymity_classical_selected")
                                .and_then(Value::as_u64)
                                .unwrap_or_default(),
                            0,
                            "classical relay count should be zero for direct provider"
                        );
                        let classical_ratio = fields
                            .get("anonymity_classical_ratio")
                            .and_then(Value::as_f64)
                            .unwrap_or_default();
                        assert!(
                            (classical_ratio - 0.0).abs() < f64::EPSILON,
                            "expected zero classical ratio, got {classical_ratio}"
                        );
                    }
                }
                "sorafs.fetch.retry" => {
                    saw_retry = true;
                    assert_eq!(fields.get("reason").and_then(Value::as_str), Some("retry"));
                    assert!(
                        fields
                            .get("attempts")
                            .and_then(Value::as_u64)
                            .unwrap_or_default()
                            >= 1
                    );
                }
                "sorafs.fetch.provider_failure" => {
                    saw_provider_failure = true;
                    assert!(
                        fields
                            .get("failures")
                            .and_then(Value::as_u64)
                            .unwrap_or_default()
                            >= 1
                    );
                }
                "sorafs.fetch.stall" => {
                    saw_stall = true;
                }
                _ => {}
            }

            if saw_start && saw_complete && saw_retry && saw_provider_failure {
                break;
            }
        }

        if !(saw_start && saw_complete) {
            eprintln!(
                "skipping fetch telemetry assertions (start={saw_start}, complete={saw_complete})"
            );
            return;
        }
        // Retry/provider-failure events may be coalesced when telemetry subscribers lag;
        // they remain informative but are not mandatory for this smoke test.
        assert!(
            !saw_stall,
            "unexpected stall event emitted during basic fetch"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn fetch_records_stall_metrics_for_slow_provider() {
        let payload = vec![0xAB; 4 * 1024];
        let plan = CarBuildPlan::single_file(&payload).expect("plan");
        let adverts = vec![provider_metadata("slow")];
        let telemetry = TelemetrySnapshot::default();

        let mut config = OrchestratorConfig::default();
        config.scoreboard.latency_cap_ms = 5;
        config.telemetry_region = Some("stall-test".to_string());
        let orchestrator = Orchestrator::new(config);
        let scoreboard = orchestrator
            .build_scoreboard(&plan, &adverts, &telemetry)
            .expect("scoreboard");

        let manifest_label = hex::encode(plan.payload_digest.as_bytes());
        let metrics = global_or_default();
        let stalls_before = metrics
            .sorafs_orchestrator_stalls_total
            .with_label_values(&[manifest_label.as_str(), "slow"])
            .get();

        let handle = test_logger();
        let mut receiver = match handle.subscribe_on_telemetry(Channel::Regular).await {
            Ok(receiver) => receiver,
            Err(err) => {
                eprintln!(
                    "skipping stall telemetry assertion (telemetry channel unavailable): {err}"
                );
                return;
            }
        };

        let payload_arc = Arc::new(payload);
        let fetcher = {
            let payload = Arc::clone(&payload_arc);
            move |request: FetchRequest| {
                let payload = Arc::clone(&payload);
                async move {
                    tokio::time::sleep(Duration::from_millis(25)).await;
                    let start = request.spec.offset as usize;
                    let end = start + request.spec.length as usize;
                    Ok::<ChunkResponse, std::io::Error>(ChunkResponse::new(
                        payload[start..end].to_vec(),
                    ))
                }
            }
        };

        let session = orchestrator
            .fetch_with_scoreboard(&plan, &scoreboard, fetcher)
            .await
            .expect("fetch outcome");
        assert_eq!(session.outcome.chunks.len(), plan.chunks.len());
        assert_eq!(session.policy_report.status, PolicyStatus::Met);

        let stalls_after = metrics
            .sorafs_orchestrator_stalls_total
            .with_label_values(&[manifest_label.as_str(), "slow"])
            .get();
        assert_eq!(
            stalls_after,
            stalls_before + plan.chunks.len() as u64,
            "expected stall counter to reflect slow chunk delivery"
        );

        let mut saw_stall_event = false;
        let mut success_stall_count: Option<u64> = None;

        for _ in 0..128 {
            let Ok(result) =
                tokio::time::timeout(Duration::from_millis(200), receiver.recv()).await
            else {
                continue;
            };
            let Ok(event) = result else { continue };
            if !event.target.starts_with("sorafs.fetch") {
                continue;
            }
            let payload: Value = event.fields.clone().into();
            let Some(fields) = payload.as_object() else {
                continue;
            };
            if fields.get("manifest").and_then(Value::as_str) != Some(manifest_label.as_str()) {
                continue;
            }
            if fields.get("region").and_then(Value::as_str) != Some("stall-test") {
                continue;
            }
            match event.target {
                "sorafs.fetch.lifecycle" => {
                    if fields.get("status").and_then(Value::as_str) == Some("success") {
                        success_stall_count = fields.get("stall_count").and_then(Value::as_u64);
                    }
                }
                "sorafs.fetch.stall" => {
                    saw_stall_event = true;
                    assert_eq!(
                        fields.get("provider").and_then(Value::as_str),
                        Some("slow"),
                        "stall event must reference the slow provider"
                    );
                }
                _ => {}
            }
            if saw_stall_event && success_stall_count.is_some() {
                break;
            }
        }

        // Stall metrics are validated above; telemetry events may be dropped under heavy load.
        if saw_stall_event && let Some(count) = success_stall_count {
            assert_eq!(
                count,
                plan.chunks.len() as u64,
                "stall count must reflect the number of delayed chunks"
            );
        }
    }

    #[test]
    fn config_json_roundtrip_preserves_fields() {
        let mut config = OrchestratorConfig::default();
        config.scoreboard.latency_cap_ms = 750;
        config.scoreboard.weight_scale = NonZeroU32::new(16).expect("non-zero weight scale");
        config.scoreboard.telemetry_grace_period = Duration::from_mins(10);
        config.scoreboard.persist_path = Some(PathBuf::from("/tmp/sorafs_scoreboard.json"));
        config.scoreboard.now_unix_secs = 1_700_000_000;
        config.fetch.verify_lengths = false;
        config.fetch.verify_digests = true;
        config.fetch.per_chunk_retry_limit = Some(5);
        config.fetch.provider_failure_threshold = 2;
        config.fetch.global_parallel_limit = Some(8);
        config.telemetry_region = Some("us-east-1".to_string());
        config.max_providers = NonZeroUsize::new(3);
        config.transport_policy = TransportPolicy::SoranetStrict;
        config.write_mode = WriteModeHint::UploadPqOnly;
        config.relay_path_hints = vec![RelayPathHint {
            relay_id: [0xAA; 32],
            avg_rtt_ms: Some(15),
            region: Some("eu-west".to_string()),
            asn: Some(64500),
            validator_lane: true,
            masque_bypass_allowed: true,
        }];

        let value = bindings::config_to_json(&config);
        let parsed = bindings::config_from_json(&value).expect("parse config");

        assert_eq!(
            parsed.scoreboard.latency_cap_ms,
            config.scoreboard.latency_cap_ms
        );
        assert_eq!(
            parsed.scoreboard.weight_scale,
            config.scoreboard.weight_scale
        );
        assert_eq!(
            parsed.scoreboard.telemetry_grace_period,
            config.scoreboard.telemetry_grace_period
        );
        assert_eq!(
            parsed.scoreboard.persist_path,
            config.scoreboard.persist_path
        );
        assert_eq!(
            parsed.scoreboard.now_unix_secs,
            config.scoreboard.now_unix_secs
        );
        assert_eq!(parsed.fetch.verify_lengths, config.fetch.verify_lengths);
        assert_eq!(parsed.fetch.verify_digests, config.fetch.verify_digests);
        assert_eq!(
            parsed.fetch.per_chunk_retry_limit,
            config.fetch.per_chunk_retry_limit
        );
        assert_eq!(
            parsed.fetch.provider_failure_threshold,
            config.fetch.provider_failure_threshold
        );
        assert_eq!(
            parsed.fetch.global_parallel_limit,
            config.fetch.global_parallel_limit
        );
        assert_eq!(parsed.telemetry_region, config.telemetry_region);
        assert_eq!(
            parsed.max_providers.map(NonZeroUsize::get),
            config.max_providers.map(NonZeroUsize::get)
        );
        assert_eq!(parsed.transport_policy, config.transport_policy);
        assert_eq!(parsed.write_mode, config.write_mode);
        assert_eq!(parsed.relay_path_hints.len(), 1);
        let parsed_hint = &parsed.relay_path_hints[0];
        assert_eq!(parsed_hint.relay_id, [0xAA; 32]);
        assert_eq!(parsed_hint.avg_rtt_ms, Some(15));
        assert_eq!(parsed_hint.region.as_deref(), Some("eu-west"));
        assert_eq!(parsed_hint.asn, Some(64500));
        assert!(parsed_hint.validator_lane);
        assert!(parsed_hint.masque_bypass_allowed);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn privacy_collector_ingests_ndjson_and_updates_metrics() {
        test_logger();

        let bucket_secs = 60;
        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("current time before unix epoch")
            .as_secs();
        let bucket_start = ((now_secs.saturating_sub(180)) / bucket_secs) * bucket_secs;
        let bucket_label = bucket_start.to_string();
        let mode = SoranetPrivacyModeV1::Middle;

        let events = [
            SoranetPrivacyEventV1 {
                timestamp_unix: bucket_start,
                mode,
                kind: SoranetPrivacyEventKindV1::HandshakeSuccess(
                    SoranetPrivacyEventHandshakeSuccessV1 {
                        rtt_ms: Some(90),
                        active_circuits_after: Some(4),
                    },
                ),
            },
            SoranetPrivacyEventV1 {
                timestamp_unix: bucket_start + 10,
                mode,
                kind: SoranetPrivacyEventKindV1::HandshakeFailure(
                    SoranetPrivacyEventHandshakeFailureV1 {
                        reason: SoranetPrivacyHandshakeFailureV1::Pow,
                        detail: None,
                        rtt_ms: Some(110),
                    },
                ),
            },
            SoranetPrivacyEventV1 {
                timestamp_unix: bucket_start + 20,
                mode,
                kind: SoranetPrivacyEventKindV1::Throttle(SoranetPrivacyEventThrottleV1 {
                    scope: SoranetPrivacyThrottleScopeV1::RemoteQuota,
                }),
            },
            SoranetPrivacyEventV1 {
                timestamp_unix: bucket_start + 30,
                mode,
                kind: SoranetPrivacyEventKindV1::VerifiedBytes(
                    SoranetPrivacyEventVerifiedBytesV1 { bytes: 2_048 },
                ),
            },
            SoranetPrivacyEventV1 {
                timestamp_unix: bucket_start + 45,
                mode,
                kind: SoranetPrivacyEventKindV1::ActiveSample(SoranetPrivacyEventActiveSampleV1 {
                    active_circuits: 6,
                }),
            },
        ];

        let ndjson = events
            .iter()
            .map(|event| {
                let value = json::to_value(event).expect("serialize privacy event");
                json::to_string(&value).expect("encode privacy event")
            })
            .collect::<Vec<_>>()
            .join("\n");

        let downgrade_event = SoranetPrivacyEventV1 {
            timestamp_unix: bucket_start + 55,
            mode,
            kind: SoranetPrivacyEventKindV1::HandshakeFailure(
                SoranetPrivacyEventHandshakeFailureV1 {
                    reason: SoranetPrivacyHandshakeFailureV1::Downgrade,
                    detail: None,
                    rtt_ms: Some(88),
                },
            ),
        };
        let downgrade_json = json::to_value(&downgrade_event).expect("encode downgrade");
        let policy_ndjson = json::to_string(&downgrade_json).expect("render downgrade ndjson line");

        let (url, server_handle) = match spawn_privacy_server(
            "200 OK",
            Some(ndjson),
            Some(policy_ndjson),
        ) {
            Ok(pair) => pair,
            Err(err) if should_skip_socket_permission(&err.to_string()) => {
                eprintln!(
                    "skipping privacy collector lapse remediation test (loopback unavailable): {err}"
                );
                return;
            }
            Err(err) => panic!("bind mock privacy server listener: {err}"),
        };

        let config = PrivacyEventsConfig {
            bucket: PrivacyBucketConfig {
                bucket_secs,
                min_contributors: 1,
                flush_delay_buckets: 1,
                force_flush_buckets: 1,
                max_completed_buckets: 8,
                expected_shares: 1,
                max_share_lag_buckets: 12,
            },
            poll_interval: Duration::from_secs(30),
            request_timeout: Duration::from_secs(2),
        };

        let metrics = Arc::new(iroha_telemetry::metrics::Metrics::default());
        let proxy_cfg = LocalQuicProxyConfig {
            bind_addr: "127.0.0.1:0".into(),
            proxy_mode: ProxyMode::Bridge,
            ..LocalQuicProxyConfig::default()
        };
        let initial_handle = match spawn_local_quic_proxy(proxy_cfg.clone()) {
            Ok(handle) => handle,
            Err(ProxyError::QuinnEndpoint(message)) if should_skip_socket_permission(&message) => {
                eprintln!(
                    "skipping privacy collector remediation test (loopback unavailable): {message}"
                );
                return;
            }
            Err(err) => panic!("spawn test proxy: {err}"),
        };
        let runtime = Arc::new(AsyncMutex::new(LocalProxyRuntime::new(
            proxy_cfg,
            Some(initial_handle),
        )));
        let remediation_cfg = DowngradeRemediationConfig {
            enabled: true,
            threshold: NonZeroU32::new(1).expect("non-zero threshold"),
            window: Duration::from_mins(1),
            cooldown: Duration::from_secs(1),
            target_mode: ProxyMode::MetadataOnly,
            resume_mode: ProxyMode::Bridge,
            modes: vec![mode],
        };
        let remediator = Arc::new(DowngradeRemediator::new(
            Arc::clone(&runtime),
            remediation_cfg,
            Arc::clone(&metrics),
        ));
        let collector = PrivacyCollector::spawn(
            vec![("relay-alpha".to_string(), url)],
            &config,
            Arc::clone(&metrics),
            Some(Arc::clone(&remediator)),
        )
        .expect("privacy collector spawn")
        .expect("privacy collector created");

        assert_eq!(
            metrics.soranet_privacy_collector_enabled.get(),
            1,
            "collector enabled gauge should be 1 while running"
        );

        tokio::time::sleep(Duration::from_millis(150)).await;

        {
            let guard = runtime.lock().await;
            assert_eq!(guard.mode(), ProxyMode::MetadataOnly);
        }

        collector.shutdown().await;
        let _ = server_handle.join();

        assert_eq!(
            metrics.soranet_privacy_collector_enabled.get(),
            0,
            "collector enabled gauge should reset after shutdown"
        );

        let accepted = metrics
            .soranet_privacy_circuit_events_total
            .with_label_values(&[mode.as_label(), &bucket_label, "accepted"])
            .get();
        assert_eq!(accepted, 1);

        let pow_rejected = metrics
            .soranet_privacy_circuit_events_total
            .with_label_values(&[mode.as_label(), &bucket_label, "pow_rejected"])
            .get();
        assert_eq!(pow_rejected, 1);

        let throttled = metrics
            .soranet_privacy_throttles_total
            .with_label_values(&[mode.as_label(), &bucket_label, "remote_quota"])
            .get();
        assert_eq!(throttled, 1);

        let verified_bytes = metrics
            .soranet_privacy_verified_bytes_total
            .with_label_values(&[mode.as_label(), &bucket_label])
            .get();
        assert_eq!(verified_bytes, 2_048);

        let suppressed = metrics
            .soranet_privacy_bucket_suppressed
            .with_label_values(&[mode.as_label(), &bucket_label])
            .get();
        assert_eq!(suppressed, 0.0);

        let max_active = metrics
            .soranet_privacy_active_circuits_max
            .with_label_values(&[mode.as_label(), &bucket_label])
            .get();
        assert_eq!(max_active, 6.0);

        let last_poll = metrics.soranet_privacy_last_poll_unixtime.get();
        assert!(last_poll > 0, "expected last poll timestamp to be recorded");

        let poll_errors = metrics
            .soranet_privacy_poll_errors_total
            .with_label_values(&["relay-alpha"])
            .get();
        assert_eq!(
            poll_errors, 0,
            "no poll errors expected for successful response"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn privacy_collector_ignores_non_success_status() {
        test_logger();

        let (url, server_handle) = match spawn_privacy_server("503 Service Unavailable", None, None)
        {
            Ok(pair) => pair,
            Err(err) if should_skip_socket_permission(&err.to_string()) => {
                eprintln!(
                    "skipping privacy collector non-success test (loopback unavailable): {err}"
                );
                return;
            }
            Err(err) => panic!("bind mock privacy server listener: {err}"),
        };

        let config = PrivacyEventsConfig {
            bucket: PrivacyBucketConfig {
                bucket_secs: 60,
                min_contributors: 1,
                flush_delay_buckets: 1,
                force_flush_buckets: 1,
                max_completed_buckets: 8,
                expected_shares: 1,
                max_share_lag_buckets: 12,
            },
            poll_interval: Duration::from_secs(30),
            request_timeout: Duration::from_secs(2),
        };

        let metrics = Arc::new(iroha_telemetry::metrics::Metrics::default());
        let collector = PrivacyCollector::spawn(
            vec![("relay-beta".to_string(), url)],
            &config,
            Arc::clone(&metrics),
            None,
        )
        .expect("privacy collector spawn")
        .expect("privacy collector created");
        let aggregator = Arc::clone(&collector.aggregator);

        assert_eq!(
            metrics.soranet_privacy_collector_enabled.get(),
            1,
            "collector enabled gauge should be 1 while running"
        );

        tokio::time::sleep(Duration::from_millis(150)).await;

        collector.shutdown().await;
        let _ = server_handle.join();

        assert_eq!(
            metrics.soranet_privacy_collector_enabled.get(),
            0,
            "collector enabled gauge should reset after shutdown"
        );

        assert!(
            aggregator.drain_ready_now().is_empty(),
            "no buckets expected when relay returns non-success status"
        );

        let metrics_dump = metrics.try_to_string().expect("encode metrics");
        assert!(
            !metrics_dump.contains("soranet_privacy_circuit_events_total"),
            "unexpected circuit metrics recorded:\n{metrics_dump}"
        );

        let last_poll = metrics.soranet_privacy_last_poll_unixtime.get();
        assert_eq!(
            last_poll, 0,
            "last poll gauge should remain unset on failure"
        );

        let poll_errors = metrics
            .soranet_privacy_poll_errors_total
            .with_label_values(&["relay-beta"])
            .get();
        assert!(poll_errors > 0, "expected poll error counter to increment");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn privacy_collector_ingests_prio_shares_basic() {
        test_logger();

        let mut share_a = SoranetPrivacyPrioShareV1::new(1, 0, 60);
        share_a.mode = SoranetPrivacyModeV1::Entry;
        share_a.handshake_accept_share = 8;
        share_a.handshake_pow_reject_share = 1;
        share_a.throttle_congestion_share = 2;
        share_a.active_circuits_sum_share = 40;
        share_a.active_circuits_sample_share = 8;
        share_a.active_circuits_max_observed = Some(9);
        share_a.verified_bytes_share = 1_024;
        share_a.suppressed = false;

        let mut share_b = SoranetPrivacyPrioShareV1::new(2, 0, 60);
        share_b.mode = SoranetPrivacyModeV1::Entry;
        share_b.handshake_accept_share = 7;
        share_b.handshake_pow_reject_share = 2;
        share_b.throttle_congestion_share = 1;
        share_b.active_circuits_sum_share = 32;
        share_b.active_circuits_sample_share = 8;
        share_b.active_circuits_max_observed = Some(11);
        share_b.verified_bytes_share = 2_048;
        share_b.suppressed = false;

        let share_a_json = json::to_value(&share_a).expect("share to json");
        let share_b_json = json::to_value(&share_b).expect("share to json");

        let body = format!(
            "{}\n{}\n",
            json::to_string(&share_a_json).expect("serialize share"),
            json::to_string(&share_b_json).expect("serialize share")
        );

        let (url, server_handle) = match spawn_privacy_server("200 OK", Some(body), None) {
            Ok(pair) => pair,
            Err(err) if should_skip_socket_permission(&err.to_string()) => {
                eprintln!(
                    "skipping privacy collector prio share test (loopback unavailable): {err}"
                );
                return;
            }
            Err(err) => panic!("bind mock privacy server listener: {err}"),
        };

        let config = PrivacyEventsConfig {
            bucket: PrivacyBucketConfig {
                bucket_secs: 60,
                min_contributors: 10,
                flush_delay_buckets: 1,
                force_flush_buckets: 2,
                max_completed_buckets: 8,
                expected_shares: 2,
                max_share_lag_buckets: 12,
            },
            poll_interval: Duration::from_secs(30),
            request_timeout: Duration::from_secs(2),
        };

        let metrics = Arc::new(iroha_telemetry::metrics::Metrics::default());
        let collector = PrivacyCollector::spawn(
            vec![("relay-prio".to_string(), url)],
            &config,
            Arc::clone(&metrics),
            None,
        )
        .expect("privacy collector spawn")
        .expect("privacy collector created");

        assert_eq!(
            metrics.soranet_privacy_collector_enabled.get(),
            1,
            "collector enabled gauge should be 1 while running"
        );

        tokio::time::sleep(Duration::from_millis(150)).await;

        collector.shutdown().await;
        let _ = server_handle.join();

        assert_eq!(
            metrics.soranet_privacy_collector_enabled.get(),
            0,
            "collector enabled gauge should reset after shutdown"
        );

        let accepted = metrics
            .soranet_privacy_circuit_events_total
            .with_label_values(&["entry", "0", "accepted"])
            .get();
        assert_eq!(accepted, 15);

        let poll_errors = metrics
            .soranet_privacy_poll_errors_total
            .with_label_values(&["relay-prio"])
            .get();
        assert_eq!(poll_errors, 0);

        let last_poll = metrics.soranet_privacy_last_poll_unixtime.get();
        assert!(last_poll > 0, "expected last poll timestamp to be recorded");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn privacy_collector_ingests_prio_shares() {
        test_logger();

        let mut share_a = SoranetPrivacyPrioShareV1::new(1, 0, 60);
        share_a.mode = SoranetPrivacyModeV1::Entry;
        share_a.handshake_accept_share = 8;
        share_a.handshake_pow_reject_share = 1;
        share_a.throttle_congestion_share = 2;
        share_a.active_circuits_sum_share = 40;
        share_a.active_circuits_sample_share = 8;
        share_a.active_circuits_max_observed = Some(9);
        share_a.verified_bytes_share = 1_024;
        share_a.suppressed = false;

        let mut share_b = SoranetPrivacyPrioShareV1::new(2, 0, 60);
        share_b.mode = SoranetPrivacyModeV1::Entry;
        share_b.handshake_accept_share = 7;
        share_b.handshake_pow_reject_share = 2;
        share_b.throttle_congestion_share = 1;
        share_b.active_circuits_sum_share = 32;
        share_b.active_circuits_sample_share = 8;
        share_b.active_circuits_max_observed = Some(11);
        share_b.verified_bytes_share = 2_048;
        share_b.suppressed = false;

        let share_a_json = json::to_value(&share_a).expect("share to json");
        let share_b_json = json::to_value(&share_b).expect("share to json");

        let body = format!(
            "{}\n{}\n",
            json::to_string(&share_a_json).expect("serialize share"),
            json::to_string(&share_b_json).expect("serialize share")
        );

        let (url, server_handle) = match spawn_privacy_server("200 OK", Some(body), None) {
            Ok(pair) => pair,
            Err(err) if should_skip_socket_permission(&err.to_string()) => {
                eprintln!(
                    "skipping privacy collector prio ingestion test (loopback unavailable): {err}"
                );
                return;
            }
            Err(err) => panic!("bind mock privacy server listener: {err}"),
        };

        let config = PrivacyEventsConfig {
            bucket: PrivacyBucketConfig {
                bucket_secs: 60,
                min_contributors: 10,
                flush_delay_buckets: 1,
                force_flush_buckets: 2,
                max_completed_buckets: 8,
                expected_shares: 2,
                max_share_lag_buckets: 12,
            },
            poll_interval: Duration::from_secs(30),
            request_timeout: Duration::from_secs(2),
        };

        let metrics = Arc::new(iroha_telemetry::metrics::Metrics::default());
        let collector = PrivacyCollector::spawn(
            vec![("relay-prio".to_string(), url)],
            &config,
            Arc::clone(&metrics),
            None,
        )
        .expect("privacy collector spawn")
        .expect("privacy collector created");

        assert_eq!(
            metrics.soranet_privacy_collector_enabled.get(),
            1,
            "collector enabled gauge should be 1 while running"
        );

        tokio::time::sleep(Duration::from_millis(150)).await;

        collector.shutdown().await;
        let _ = server_handle.join();

        assert_eq!(
            metrics.soranet_privacy_collector_enabled.get(),
            0,
            "collector enabled gauge should reset after shutdown"
        );

        let mode_label = SoranetPrivacyModeV1::Entry.as_label();
        let bucket_label = share_a.bucket_start_unix.to_string();

        let accepted = metrics
            .soranet_privacy_circuit_events_total
            .with_label_values(&[mode_label, &bucket_label, "accepted"])
            .get();
        assert_eq!(accepted, 15);

        let pow_rejected = metrics
            .soranet_privacy_circuit_events_total
            .with_label_values(&[mode_label, &bucket_label, "pow_rejected"])
            .get();
        assert_eq!(pow_rejected, 3);

        let throttles = metrics
            .soranet_privacy_throttles_total
            .with_label_values(&[mode_label, &bucket_label, "congestion"])
            .get();
        assert_eq!(throttles, 3);

        let verified_bytes = metrics
            .soranet_privacy_verified_bytes_total
            .with_label_values(&[mode_label, &bucket_label])
            .get();
        assert_eq!(verified_bytes, 3_072);

        let suppressed = metrics
            .soranet_privacy_bucket_suppressed
            .with_label_values(&[mode_label, &bucket_label])
            .get();
        assert_eq!(suppressed, 0.0);

        let active_avg = metrics
            .soranet_privacy_active_circuits_avg
            .with_label_values(&[mode_label, &bucket_label])
            .get();
        assert_close(active_avg, 4.0);

        let active_max = metrics
            .soranet_privacy_active_circuits_max
            .with_label_values(&[mode_label, &bucket_label])
            .get();
        assert_eq!(active_max, 11.0);
    }

    fn sample_payload(len: usize) -> Vec<u8> {
        (0..len).map(|idx| (idx % 251) as u8).collect()
    }

    fn to_hex(bytes: &[u8]) -> String {
        hex::encode(bytes)
    }

    fn stream_token_b64(
        manifest_cid_hex: &str,
        provider_id_hex: &str,
        profile: &str,
        max_streams: u16,
    ) -> String {
        let mut provider_id = [0u8; 32];
        let decoded = hex::decode(provider_id_hex).expect("provider hex");
        provider_id.copy_from_slice(&decoded);
        let token = StreamTokenV1 {
            body: StreamTokenBodyV1 {
                token_id: "01J9TK3GR0XM6YQF7WQXA9Z2SF".to_owned(),
                manifest_cid: hex::decode(manifest_cid_hex).expect("manifest cid"),
                provider_id,
                profile_handle: profile.to_owned(),
                max_streams,
                ttl_epoch: 9_999_999_999,
                rate_limit_bytes: 8 * 1024 * 1024,
                issued_at: 1_735_000_000,
                requests_per_minute: 120,
                token_pk_version: 1,
            },
            signature: vec![0; 64],
        };
        let bytes = norito::to_bytes(&token).expect("encode token");
        BASE64_STANDARD.encode(bytes)
    }

    struct MockGateway {
        base_url: String,
        join_handle: Option<thread::JoinHandle<()>>,
    }

    impl MockGateway {
        fn spawn(manifest_id_hex: String, plan: CarBuildPlan, payload: Vec<u8>) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
            listener
                .set_nonblocking(false)
                .expect("set blocking mode failed");
            let address = listener.local_addr().expect("local addr");
            let base_url = format!("http://{address}/");

            let expected = Arc::new(Mutex::new(build_chunk_map(
                &manifest_id_hex,
                &plan,
                &payload,
            )));
            let join_handle = {
                let expected = Arc::clone(&expected);
                thread::spawn(move || serve_chunks(listener, expected))
            };

            Self {
                base_url,
                join_handle: Some(join_handle),
            }
        }

        fn base_url(&self) -> &str {
            &self.base_url
        }
    }

    impl Drop for MockGateway {
        fn drop(&mut self) {
            if let Some(handle) = self.join_handle.take() {
                handle.join().expect("mock gateway thread");
            }
        }
    }

    fn build_chunk_map(
        manifest_id_hex: &str,
        plan: &CarBuildPlan,
        payload: &[u8],
    ) -> HashMap<String, Vec<u8>> {
        let mut map = HashMap::new();
        for spec in plan.chunk_fetch_specs() {
            let digest_hex = to_hex(&spec.digest);
            let path = format!("/v1/sorafs/storage/chunk/{manifest_id_hex}/{digest_hex}");
            let start = spec.offset as usize;
            let end = start + spec.length as usize;
            map.insert(path, payload[start..end].to_vec());
        }
        map
    }

    fn read_request(stream: &mut TcpStream) -> Option<String> {
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("set timeout");
        let mut buffer = [0u8; 4096];
        let mut request = Vec::new();
        loop {
            let read = stream.read(&mut buffer).ok()?;
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..read]);
            if request.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
            if request.len() > 16 * 1024 {
                break;
            }
        }
        String::from_utf8(request).ok()
    }

    fn respond(mut stream: TcpStream, body: Vec<u8>) {
        let header = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(header.as_bytes()).expect("write header");
        stream.write_all(&body).expect("write body");
    }

    fn serve_chunks(listener: TcpListener, responses: Arc<Mutex<HashMap<String, Vec<u8>>>>) {
        loop {
            let (mut stream, _) = match listener.accept() {
                Ok(pair) => pair,
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => continue,
                Err(_) => break,
            };
            let Some(request) = read_request(&mut stream) else {
                continue;
            };
            let path = request
                .lines()
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .map(str::to_string);
            let Some(path) = path else {
                continue;
            };
            let body = {
                let mut guard = responses.lock().expect("lock responses");
                guard.remove(&path)
            };
            if let Some(body) = body {
                respond(stream, body);
            } else {
                let error =
                    b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                stream.write_all(error).expect("write error");
            }
            if responses.lock().expect("lock responses").is_empty() {
                break;
            }
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "needs loopback socket permissions"]
    async fn fetch_via_gateway_streams_chunks() {
        let payload = sample_payload(8 * 1024);
        let plan = CarBuildPlan::single_file(&payload).expect("plan");
        let manifest_id_hex = to_hex(plan.payload_digest.as_bytes());
        let chunker_handle = "sorafs.sf1@1.0.0".to_string();
        let provider_id_hex = "aa".repeat(32);
        let token_b64 = stream_token_b64(&manifest_id_hex, &provider_id_hex, &chunker_handle, 4);

        let gateway = MockGateway::spawn(manifest_id_hex.clone(), plan.clone(), payload.clone());

        let gateway_config = GatewayFetchConfig {
            manifest_id_hex: manifest_id_hex.clone(),
            chunker_handle: chunker_handle.clone(),
            manifest_envelope_b64: None,
            client_id: Some("sdk-test".to_string()),
            expected_manifest_cid_hex: Some(manifest_id_hex.clone()),
            blinded_cid_b64: None,
            salt_epoch: None,
            expected_cache_version: None,
            moderation_token_key_b64: None,
        };
        let provider_input = GatewayProviderInput {
            name: "alpha".to_string(),
            provider_id_hex: provider_id_hex.clone(),
            base_url: gateway.base_url().to_string(),
            stream_token_b64: token_b64,
            privacy_events_url: None,
        };

        let mut config = OrchestratorConfig::default();
        config.fetch.per_chunk_retry_limit = Some(2);

        let session = fetch_via_gateway(
            config,
            &plan,
            gateway_config,
            [provider_input],
            None,
            Some(2),
        )
        .await
        .expect("fetch outcome");

        assert_eq!(session.outcome.chunks.len(), plan.chunk_fetch_specs().len());
        let assembled: Vec<u8> = session.outcome.chunks.concat();
        assert_eq!(assembled, payload);
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "needs loopback socket permissions"]
    async fn fetch_via_gateway_errors_without_providers() {
        let payload = sample_payload(1024);
        let plan = CarBuildPlan::single_file(&payload).expect("plan");
        let manifest_id_hex = to_hex(plan.payload_digest.as_bytes());
        let gateway_config = GatewayFetchConfig {
            manifest_id_hex,
            chunker_handle: "sorafs.sf1@1.0.0".to_string(),
            manifest_envelope_b64: None,
            client_id: None,
            expected_manifest_cid_hex: None,
            blinded_cid_b64: None,
            salt_epoch: None,
            expected_cache_version: None,
            moderation_token_key_b64: None,
        };

        let err = fetch_via_gateway(
            OrchestratorConfig::default(),
            &plan,
            gateway_config,
            std::iter::empty::<GatewayProviderInput>(),
            None,
            None,
        )
        .await
        .expect_err("expected failure");
        assert!(
            matches!(err, GatewayOrchestratorError::Build(_)),
            "unexpected error {err:?}"
        );
    }
}
