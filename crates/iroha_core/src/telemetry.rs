//! Metrics and status reporting.
//!
//! This module still exposes the single-lane TEU gauges used before the
//! Nexus scheduler becomes active. The wiring mirrors the future Nexus layout
//! (lanes and data-spaces) so callers can adopt the eventual multi-lane feeds
//! by swapping in the real scheduler hooks once they land; until then we keep
//! the fallback values in this module to avoid breaking operator dashboards.

pub mod capability;

#[cfg(feature = "telemetry")]
use std::collections::btree_map::Entry as BTreeEntry;
#[cfg(feature = "telemetry")]
use std::sync::Mutex;
#[cfg_attr(not(feature = "telemetry"), allow(unused_imports))]
use std::time::{SystemTime, UNIX_EPOCH};
use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroUsize,
    sync::{
        Arc, RwLock as StdRwLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

use http::StatusCode;
use iroha_config::parameters::actual::{DataspaceGossipFallback, RestrictedPublicPayload};
use iroha_crypto::Hash;
#[cfg(debug_assertions)]
use iroha_crypto::HashOf;
#[cfg_attr(not(feature = "telemetry"), allow(unused_imports))]
use iroha_data_model::da::types::DaRentQuote;
#[cfg(feature = "telemetry")]
use iroha_data_model::events::data::{
    DataEvent, governance::GovernanceEvent, social::SocialEvent,
    space_directory::SpaceDirectoryEvent,
};
#[cfg_attr(not(feature = "telemetry"), allow(unused_imports))]
use iroha_data_model::soranet::privacy_metrics::{
    SoranetPrivacyEventV1, SoranetPrivacyPrioShareV1,
};
#[cfg_attr(not(feature = "telemetry"), allow(unused_imports))]
use iroha_data_model::{
    Identifiable,
    asset::AssetDefinitionId,
    block::BlockHeader,
    consensus::CommitCertificate,
    nexus::{
        AxtPolicySnapshot, AxtRejectReason, DataSpaceCatalog, DataSpaceId, LaneCatalog, LaneId,
        LaneStorageProfile, LaneVisibility, PublicLaneValidatorStatus, UniversalAccountId,
    },
    peer::PeerId,
    proof::ProofStatus,
    query::QueryOutput,
};
#[cfg_attr(not(feature = "telemetry"), allow(unused_imports))]
use iroha_data_model::{
    account::AccountId,
    domain::DomainId,
    isi::settlement::{
        SettlementAtomicity, SettlementExecutionOrder, SettlementId, SettlementPlan,
    },
    kaigi::KaigiRelayHealthStatus,
    name::Name,
};
use iroha_data_model::{events::data::sorafs::SorafsProofHealthAlert, oracle::OraclePenaltyKind};
use iroha_futures::supervisor::{Child, OnShutdown};
use iroha_p2p::OnlinePeers;
use iroha_primitives::{json::Json, numeric::Numeric, time::TimeSource};
#[cfg(feature = "telemetry")]
use iroha_telemetry::metrics::GaugeVec;
#[cfg(feature = "telemetry")]
use iroha_telemetry::metrics::global_sorafs_node_otel;
pub use iroha_telemetry::metrics::{
    GOVERNANCE_MANIFEST_RECENT_CAP, GovernanceManifestActivation, Halo2Status,
    LaneSettlementBuffer, LaneSettlementSnapshot, LaneSwaplineSnapshot, Metrics,
    MicropaymentCreditSnapshot, MicropaymentSampleStatus, MicropaymentTicketCounters,
    NexusDataspaceTeuStatus, NexusLaneRuntimeUpgradeHookStatus, NexusLaneTeuBuckets,
    NexusLaneTeuStatus, SchedulerLayerWidthBuckets, TxGossipCaps, TxGossipSnapshot, TxGossipStatus,
};
use iroha_telemetry::privacy::{PrivacyBucketConfig, PrivacyShareError, SoranetSecureAggregator};
use ivm::host::{ZkCurve, ZkHalo2Backend, ZkHalo2Config};
use mv::storage::StorageReadOnly;
#[cfg(feature = "telemetry")]
use norito::streaming::{
    ContentKeyUpdate, EncryptionSuite, SyncDiagnostics, TelemetryAuditOutcome,
    TelemetryDecodeStats, TelemetryEncodeStats, TelemetryEnergyStats, TelemetryEvent,
    TelemetryNetworkStats, TelemetrySecurityStats,
};
use rust_decimal::{Decimal, prelude::ToPrimitive};
use settlement_router::policy::BufferStatus;
use tokio::sync::{RwLock, mpsc, oneshot, watch};

#[cfg(feature = "telemetry")]
use crate::pipeline::access::AccessSetSource;
#[cfg_attr(not(feature = "telemetry"), allow(unused_imports))]
use crate::smartcontracts::isi::settlement::{SETTLEMENT_KIND_DVP, SETTLEMENT_KIND_PVP};
use crate::{
    da::{DaPinIntentValidationError, DaShardCursorError},
    gossiper::{GossipPlane, gossip_plane_label},
    governance::manifest::{LaneManifestRegistryHandle, LaneManifestStatus},
    json_macros::{JsonDeserialize, JsonSerialize},
    kura::Kura,
    nexus::space_directory::SpaceDirectoryManifestSet,
    queue::{Queue, QueueLimits},
    state::{State, StateReadOnly, WorldReadOnly},
    sumeragi::{
        da::{GateReason, GateSatisfaction},
        message::BlockMessage,
        rbc_store::StorePressure,
        status::{
            self, DataspaceCommitmentSnapshot, LaneCommitmentSnapshot, SettlementOutcomeKind,
        },
    },
};

const PHASE_PREVOTE: &str = "prevote";
const PHASE_PRECOMMIT: &str = "precommit";
const PHASE_AVAILABLE: &str = "available";
const PIPELINE_BUCKET_LABELS: [&str; 8] = ["1", "2", "4", "8", "16", "32", "64", "128"];

#[cfg(feature = "telemetry")]
fn json_value<T: norito::json::JsonSerialize + ?Sized>(value: &T) -> norito::json::Value {
    norito::json::to_value(value).expect("serialize json value")
}

#[cfg(feature = "telemetry")]
fn telemetry_event_to_json(event: &TelemetryEvent) -> norito::json::Value {
    match event {
        TelemetryEvent::Encode(stats) => encode_event_to_json(stats),
        TelemetryEvent::Decode(stats) => decode_event_to_json(stats),
        TelemetryEvent::Network(stats) => network_event_to_json(stats),
        TelemetryEvent::Security(stats) => security_event_to_json(stats),
        TelemetryEvent::Energy(stats) => energy_event_to_json(stats),
        TelemetryEvent::AuditOutcome(outcome) => audit_outcome_to_json(outcome),
    }
}

#[cfg(feature = "telemetry")]
fn encode_event_to_json(stats: &TelemetryEncodeStats) -> norito::json::Value {
    let mut map = norito::json::Map::new();
    map.insert("kind".to_string(), norito::json!("encode"));
    map.insert("segment".to_string(), norito::json!(stats.segment));
    map.insert(
        "avg_latency_ms".to_string(),
        norito::json!(stats.avg_latency_ms),
    );
    map.insert(
        "dropped_layers".to_string(),
        norito::json!(stats.dropped_layers),
    );
    norito::json::Value::Object(map)
}

#[cfg(feature = "telemetry")]
fn decode_event_to_json(stats: &TelemetryDecodeStats) -> norito::json::Value {
    let mut map = norito::json::Map::new();
    map.insert("kind".to_string(), norito::json!("decode"));
    map.insert("segment".to_string(), norito::json!(stats.segment));
    map.insert("buffer_ms".to_string(), norito::json!(stats.buffer_ms));
    map.insert(
        "dropped_frames".to_string(),
        norito::json!(stats.dropped_frames),
    );
    map.insert(
        "max_decode_queue_ms".to_string(),
        norito::json!(stats.max_decode_queue_ms),
    );
    norito::json::Value::Object(map)
}

#[cfg(feature = "telemetry")]
fn network_event_to_json(stats: &TelemetryNetworkStats) -> norito::json::Value {
    let mut map = norito::json::Map::new();
    map.insert("kind".to_string(), norito::json!("network"));
    map.insert("rtt_ms".to_string(), norito::json!(stats.rtt_ms));
    map.insert(
        "loss_percent_x100".to_string(),
        norito::json!(stats.loss_percent_x100),
    );
    map.insert("fec_repairs".to_string(), norito::json!(stats.fec_repairs));
    map.insert(
        "fec_failures".to_string(),
        norito::json!(stats.fec_failures),
    );
    map.insert(
        "datagram_reinjects".to_string(),
        norito::json!(stats.datagram_reinjects),
    );
    norito::json::Value::Object(map)
}

#[cfg(feature = "telemetry")]
fn security_event_to_json(stats: &TelemetrySecurityStats) -> norito::json::Value {
    let mut map = norito::json::Map::new();
    map.insert("kind".to_string(), norito::json!("security"));
    map.insert(
        "suite".to_string(),
        StreamingTelemetry::suite_to_json(&stats.suite),
    );
    map.insert("rekeys".to_string(), norito::json!(stats.rekeys));
    map.insert(
        "gck_rotations".to_string(),
        norito::json!(stats.gck_rotations),
    );
    if let Some(key_id) = &stats.last_content_key_id {
        map.insert("last_content_key_id".to_string(), norito::json!(key_id));
    }
    if let Some(valid_from) = stats.last_content_key_valid_from {
        map.insert(
            "last_content_key_valid_from".to_string(),
            norito::json!(valid_from),
        );
    }
    norito::json::Value::Object(map)
}

#[cfg(feature = "telemetry")]
fn energy_event_to_json(stats: &TelemetryEnergyStats) -> norito::json::Value {
    let mut map = norito::json::Map::new();
    map.insert("kind".to_string(), norito::json!("energy"));
    map.insert("segment".to_string(), norito::json!(stats.segment));
    map.insert(
        "encoder_milliwatts".to_string(),
        norito::json!(stats.encoder_milliwatts),
    );
    map.insert(
        "decoder_milliwatts".to_string(),
        norito::json!(stats.decoder_milliwatts),
    );
    norito::json::Value::Object(map)
}

#[cfg(feature = "telemetry")]
fn audit_outcome_to_json(outcome: &TelemetryAuditOutcome) -> norito::json::Value {
    let mut map = norito::json::Map::new();
    map.insert("kind".to_string(), norito::json!("audit_outcome"));
    map.insert("trace_id".to_string(), norito::json!(outcome.trace_id));
    map.insert(
        "slot_height".to_string(),
        norito::json!(outcome.slot_height),
    );
    map.insert("reviewer".to_string(), norito::json!(outcome.reviewer));
    map.insert("status".to_string(), norito::json!(outcome.status));
    if let Some(url) = &outcome.mitigation_url {
        map.insert("mitigation_url".to_string(), norito::json!(url));
    }
    norito::json::Value::Object(map)
}

#[cfg_attr(not(feature = "telemetry"), allow(dead_code))]
#[inline]
fn u64_to_f64(value: u64) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    {
        value as f64
    }
}

#[cfg_attr(not(feature = "telemetry"), allow(dead_code))]
fn decimal_to_f64(value: &Decimal) -> f64 {
    value.to_f64().unwrap_or_default()
}

#[inline]
#[allow(dead_code)]
fn usize_to_f64(value: usize) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    {
        value as f64
    }
}

#[derive(Clone, Debug)]
struct LaneMetadataSnapshot {
    alias: String,
    dataspace_id: DataSpaceId,
    dataspace_alias: Option<String>,
    visibility: LaneVisibility,
    storage_profile: LaneStorageProfile,
    lane_type: Option<String>,
    governance: Option<String>,
    settlement: Option<String>,
    scheduler_teu_capacity: Option<u64>,
    scheduler_starvation_bound_slots: Option<u64>,
}

impl Default for LaneMetadataSnapshot {
    fn default() -> Self {
        Self {
            alias: String::new(),
            dataspace_id: DataSpaceId::GLOBAL,
            dataspace_alias: None,
            visibility: LaneVisibility::Public,
            storage_profile: LaneStorageProfile::FullReplica,
            lane_type: None,
            governance: None,
            settlement: None,
            scheduler_teu_capacity: None,
            scheduler_starvation_bound_slots: None,
        }
    }
}

#[derive(Clone, Debug, Default)]
struct DataspaceMetadataSnapshot {
    alias: String,
    description: Option<String>,
    fault_tolerance: u32,
}

#[derive(Clone, Debug, Default)]
struct DataspaceLabelSet {
    alias: String,
    profile: String,
    dataspace_id: String,
}

#[derive(Clone, Debug, Default)]
struct SpaceDirectoryActiveIndex {
    entries: BTreeSet<(UniversalAccountId, u64)>,
    counts: BTreeMap<u64, u64>,
}

#[allow(dead_code)]
impl SpaceDirectoryActiveIndex {
    fn clear(&mut self) {
        self.entries.clear();
        self.counts.clear();
    }

    fn insert(&mut self, uaid: UniversalAccountId, dataspace: u64) {
        if self.entries.insert((uaid, dataspace)) {
            *self.counts.entry(dataspace).or_default() += 1;
        }
    }

    fn remove(&mut self, uaid: UniversalAccountId, dataspace: u64) {
        if self.entries.remove(&(uaid, dataspace)) {
            if let Some(entry) = self.counts.get_mut(&dataspace) {
                *entry = entry.saturating_sub(1);
                if *entry == 0 {
                    self.counts.remove(&dataspace);
                }
            }
        }
    }

    fn count(&self, dataspace: u64) -> u64 {
        self.counts.get(&dataspace).copied().unwrap_or(0)
    }

    fn counts_snapshot(&self) -> BTreeMap<u64, u64> {
        self.counts
            .iter()
            .map(|(id, count)| (*id, *count))
            .collect()
    }
}

/// Gauge update for per-lane TEU metrics.
#[derive(Clone, Copy, Debug, Default)]
pub struct LaneTeuGaugeUpdate {
    /// Configured TEU capacity for the current slot.
    pub capacity: u64,
    /// TEU committed in the latest envelope.
    pub committed: u64,
    /// Bucket breakdown for committed TEU.
    pub buckets: NexusLaneTeuBuckets,
    /// Active circuit-breaker trigger level (0 = normal).
    pub trigger_level: u64,
    /// Starvation bound applied to this lane (in slots).
    pub starvation_bound_slots: u64,
}

const NEXUS_HEADROOM_WARN_PCT: u64 = 15;

/// Gauge update for per-dataspace TEU metrics.
#[derive(Clone, Copy, Debug, Default)]
pub struct DataspaceTeuGaugeUpdate {
    /// Pending TEU backlog remaining after scheduling.
    pub backlog: u64,
    /// Slots since the dataspace was last served.
    pub age_slots: u64,
    /// Latest SFQ virtual-finish tag.
    pub virtual_finish: u64,
}

/// Summary of per-lane pipeline activity for the latest block.
#[derive(Clone, Copy, Debug, Default)]
pub struct LanePipelineSummary {
    /// Block height for which this summary was recorded.
    pub block_height: u64,
    /// Transactions scheduled/executed for this lane.
    pub tx_vertices: u64,
    /// Conflict edges among the transactions for this lane.
    pub tx_edges: u64,
    /// Overlay fragments executed for this lane.
    pub overlay_count: u64,
    /// Total overlay instructions executed for this lane.
    pub overlay_instr_total: u64,
    /// Total overlay byte size executed for this lane.
    pub overlay_bytes_total: u64,
    /// RBC chunks contributed by this lane in the latest block (approximate).
    pub rbc_chunks: u64,
    /// Total payload bytes attributed to this lane for RBC chunking.
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
}

/// Summary of per-dataspace activity for the latest block.
#[derive(Clone, Copy, Debug, Default)]
pub struct DataspacePipelineSummary {
    /// Transactions scheduled for this dataspace.
    pub tx_served: u64,
}

#[cfg(feature = "telemetry")]
#[derive(Clone)]
struct NexusConfigDiff {
    knob: &'static str,
    baseline: norito::json::Value,
    current: norito::json::Value,
}

#[cfg(feature = "telemetry")]
fn push_nexus_diff(
    diffs: &mut Vec<NexusConfigDiff>,
    knob: &'static str,
    baseline: norito::json::Value,
    current: norito::json::Value,
) {
    if baseline != current {
        diffs.push(NexusConfigDiff {
            knob,
            baseline,
            current,
        });
    }
}

#[cfg(feature = "telemetry")]
#[derive(Clone, Copy, Debug)]
/// Aggregated counters for confidential Merkle tree maintenance.
pub struct ConfidentialTreeStats {
    /// Total number of commitments stored in the tree.
    pub commitments: u64,
    /// Current depth of the tree.
    pub tree_depth: u64,
    /// Count of root history records retained.
    pub root_history: u64,
    /// Number of checkpoints currently tracked for the frontier.
    pub frontier_checkpoints: u64,
    /// Height of the latest recorded frontier checkpoint (0 when none exists).
    pub last_checkpoint_height: u64,
    /// Commitment count captured alongside the latest checkpoint.
    pub last_checkpoint_commitments: u64,
    /// How many times the root history evicted entries.
    pub root_evictions: u64,
    /// How many times the frontier evicted entries.
    pub frontier_evictions: u64,
}

fn proof_status_label(status: ProofStatus) -> &'static str {
    use ProofStatus as PS;
    match status {
        PS::Submitted => "submitted",
        PS::Verified => "verified",
        PS::Rejected => "rejected",
    }
}

fn proposal_status_label(status: crate::state::GovernanceProposalStatus) -> &'static str {
    use crate::state::GovernanceProposalStatus as GPS;
    match status {
        GPS::Proposed => "proposed",
        GPS::Approved => "approved",
        GPS::Rejected => "rejected",
        GPS::Enacted => "enacted",
    }
}

#[cfg(feature = "telemetry")]
#[allow(dead_code)]
fn access_set_source_label(source: AccessSetSource) -> &'static str {
    match source {
        AccessSetSource::ManifestHints => "manifest_hints",
        AccessSetSource::EntrypointHints => "entrypoint_hints",
        AccessSetSource::PrepassMerge => "prepass_merge",
        AccessSetSource::ConservativeFallback => "conservative_fallback",
    }
}

/// Snapshot of the last observed AXT proof cache state for a dataspace.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct AxtProofCacheStatus {
    /// Dataspace identifier.
    pub dataspace: DataSpaceId,
    /// Cache status label (hit/miss/reject/expired/cleared).
    pub status: String,
    /// Manifest root the proof was validated against (if any).
    pub manifest_root: Option<[u8; 32]>,
    /// Slot at which the proof was verified.
    pub verified_slot: u64,
    /// Expiry slot (with skew applied) recorded for the proof.
    pub expiry_slot: Option<u64>,
}

/// Snapshot of the latest AXT policy rejection for debugging/alerting.
#[derive(Copy, Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct AxtPolicyRejectSnapshot {
    /// Lane associated with the rejected usage.
    pub lane: LaneId,
    /// Reason label describing the reject category.
    pub reason: AxtRejectReason,
    /// AXT policy snapshot version active when the rejection occurred.
    pub snapshot_version: u64,
}

/// Aggregate AXT debug status returned over Torii debug endpoints.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct AxtDebugStatus {
    /// Current AXT policy snapshot version observed by the host.
    pub policy_snapshot_version: u64,
    /// Most recent rejection (if any).
    pub last_reject: Option<AxtPolicyRejectSnapshot>,
    /// Cached proof state by dataspace.
    pub cache: Vec<AxtProofCacheStatus>,
    /// Optional hints for refreshing handles after a rejection.
    pub hints: Vec<AxtRejectHint>,
}

/// Hint to help callers refresh handles after a rejected AXT envelope.
#[derive(Copy, Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct AxtRejectHint {
    /// Dataspace associated with the rejected handle.
    pub dataspace: DataSpaceId,
    /// Target lane for the handle.
    pub target_lane: LaneId,
    /// Next minimum handle era required by the policy.
    pub next_min_handle_era: u64,
    /// Next minimum sub-nonce required by the policy.
    pub next_min_sub_nonce: u64,
    /// Reason label for the rejection (e.g., `era`, `sub_nonce`, `expiry`).
    pub reason: AxtRejectReason,
}

/// Slice of metrics used to be used from within [`State`].
///
/// Needed to brake the circular dependency from [`Telemetry`] to [`State`].
#[derive(Clone)]
pub struct StateTelemetry {
    metrics: Arc<Metrics>,
    enabled: Arc<AtomicBool>,
    nexus_enabled: Arc<AtomicBool>,
    time_source: TimeSource,
    lane_metadata: Arc<StdRwLock<BTreeMap<u32, LaneMetadataSnapshot>>>,
    dataspace_metadata: Arc<StdRwLock<BTreeMap<u64, DataspaceMetadataSnapshot>>>,
    lane_manifest_registry: Arc<StdRwLock<Option<LaneManifestRegistryHandle>>>,
    space_directory_active_index: Arc<StdRwLock<SpaceDirectoryActiveIndex>>,
    soranet_privacy: Arc<SoranetSecureAggregator>,
    axt_proof_cache: Arc<StdRwLock<BTreeMap<DataSpaceId, AxtProofCacheStatus>>>,
    axt_last_policy_reject: Arc<StdRwLock<Option<AxtPolicyRejectSnapshot>>>,
    axt_policy_snapshot_version: Arc<AtomicU64>,
    axt_reject_hints: Arc<StdRwLock<BTreeMap<DataSpaceId, AxtRejectHint>>>,
    #[cfg(feature = "telemetry")]
    governance_status_cache: Arc<Mutex<BTreeMap<[u8; 32], crate::state::GovernanceProposalStatus>>>,
}

fn reset_nexus_metrics(metrics: &Metrics) {
    metrics.nexus_lane_configured_total.set(0);
    metrics.nexus_lane_id_placeholder.set(0);
    metrics.nexus_dataspace_id_placeholder.set(0);
    metrics.nexus_lane_governance_sealed.reset();
    metrics.nexus_lane_governance_sealed_total.set(0);
    metrics
        .nexus_lane_governance_sealed_aliases
        .write()
        .expect("lane governance aliases lock poisoned")
        .clear();
    metrics.nexus_lane_block_height.reset();
    metrics.nexus_lane_finality_lag_slots.reset();
    metrics.nexus_lane_settlement_backlog_xor.reset();
    metrics.nexus_public_lane_validator_total.reset();
    metrics.nexus_public_lane_validator_activation_total.reset();
    metrics.nexus_public_lane_validator_reject_total.reset();
    metrics.nexus_public_lane_stake_bonded.reset();
    metrics.nexus_public_lane_unbond_pending.reset();
    metrics.nexus_public_lane_reward_total.reset();
    metrics.nexus_public_lane_slash_total.reset();
    metrics.nexus_scheduler_lane_teu_capacity.reset();
    metrics.nexus_scheduler_lane_teu_slot_committed.reset();
    metrics.nexus_scheduler_lane_trigger_level.reset();
    metrics.nexus_scheduler_starvation_bound_slots.reset();
    metrics.nexus_scheduler_lane_teu_slot_breakdown.reset();
    metrics.nexus_scheduler_lane_teu_deferral_total.reset();
    metrics.nexus_scheduler_lane_headroom_events_total.reset();
    metrics.nexus_scheduler_must_serve_truncations_total.reset();
    metrics.nexus_scheduler_dataspace_teu_backlog.reset();
    metrics.nexus_scheduler_dataspace_age_slots.reset();
    metrics.nexus_scheduler_dataspace_virtual_finish.reset();
    metrics
        .nexus_scheduler_lane_teu_status
        .write()
        .expect("lane TEU status cache lock poisoned")
        .clear();
}

impl StateTelemetry {
    /// Create from [`Metrics`]
    pub fn new(metrics: Arc<Metrics>, enabled: bool) -> Self {
        Self::with_privacy_config(metrics, enabled, PrivacyBucketConfig::default())
    }

    /// Access the underlying metrics registry.
    #[cfg(all(test, feature = "telemetry"))]
    pub(crate) fn metrics_ref(&self) -> &Metrics {
        &self.metrics
    }

    /// Create a [`StateTelemetry`] instance using an explicit `SoraNet` privacy bucket configuration.
    pub fn with_privacy_config(
        metrics: Arc<Metrics>,
        enabled: bool,
        privacy_config: PrivacyBucketConfig,
    ) -> Self {
        let soranet_privacy = Arc::new(
            SoranetSecureAggregator::new(privacy_config).expect("valid SoraNet privacy config"),
        );
        let telemetry = Self {
            metrics,
            enabled: Arc::new(AtomicBool::new(enabled)),
            nexus_enabled: Arc::new(AtomicBool::new(true)),
            time_source: TimeSource::new_system(),
            lane_metadata: Arc::new(StdRwLock::new(BTreeMap::new())),
            dataspace_metadata: Arc::new(StdRwLock::new(BTreeMap::new())),
            lane_manifest_registry: Arc::new(StdRwLock::new(None)),
            space_directory_active_index: Arc::new(StdRwLock::new(
                SpaceDirectoryActiveIndex::default(),
            )),
            soranet_privacy,
            axt_proof_cache: Arc::new(StdRwLock::new(BTreeMap::new())),
            axt_last_policy_reject: Arc::new(StdRwLock::new(None)),
            axt_policy_snapshot_version: Arc::new(AtomicU64::new(0)),
            axt_reject_hints: Arc::new(StdRwLock::new(BTreeMap::new())),
            #[cfg(feature = "telemetry")]
            governance_status_cache: Arc::new(Mutex::new(BTreeMap::new())),
        };
        telemetry.set_nexus_catalogs(&LaneCatalog::default(), &DataSpaceCatalog::default());
        telemetry
    }

    /// Create [`StateTelemetry`] using parameters supplied from configuration.
    pub fn from_privacy_parameters(
        metrics: Arc<Metrics>,
        enabled: bool,
        privacy: &iroha_config::parameters::actual::SoranetPrivacy,
    ) -> Self {
        let privacy_config = PrivacyBucketConfig {
            bucket_secs: privacy.bucket_secs,
            min_contributors: privacy.min_handshakes,
            flush_delay_buckets: privacy.flush_delay_buckets,
            force_flush_buckets: privacy.force_flush_buckets,
            max_completed_buckets: privacy.max_completed_buckets,
            expected_shares: privacy.expected_shares,
            max_share_lag_buckets: privacy.max_share_lag_buckets,
        };
        Self::with_privacy_config(metrics, enabled, privacy_config)
    }

    /// Access the `SoraNet` privacy aggregator.
    pub fn soranet_privacy(&self) -> Arc<SoranetSecureAggregator> {
        Arc::clone(&self.soranet_privacy)
    }

    /// Replace the cached Nexus lane/dataspace metadata used for telemetry labels.
    pub fn set_nexus_catalogs(
        &self,
        lane_catalog: &LaneCatalog,
        dataspace_catalog: &DataSpaceCatalog,
    ) {
        if !self.nexus_enabled() {
            self.reset_nexus_lane_metrics();
            self.clear_nexus_cache_state();
            return;
        }
        self.metrics
            .nexus_lane_configured_total
            .set(u64::from(lane_catalog.lane_count().get()));
        let lane_ids: Vec<u32> = lane_catalog
            .lanes()
            .iter()
            .map(|lane| lane.id.as_u32())
            .collect();
        let dataspace_lookup: BTreeMap<u64, DataspaceMetadataSnapshot> = dataspace_catalog
            .entries()
            .iter()
            .map(|entry| {
                (
                    entry.id.as_u64(),
                    DataspaceMetadataSnapshot {
                        alias: entry.alias.clone(),
                        description: entry.description.clone(),
                        fault_tolerance: entry.fault_tolerance,
                    },
                )
            })
            .collect();
        {
            let mut guard = self
                .lane_metadata
                .write()
                .expect("lane metadata lock poisoned");
            guard.clear();
            for lane in lane_catalog.lanes() {
                let dataspace_alias = dataspace_lookup
                    .get(&lane.dataspace_id.as_u64())
                    .map(|entry| entry.alias.clone());
                let scheduler_teu_capacity = lane
                    .metadata
                    .get("scheduler.teu_capacity")
                    .and_then(|raw| raw.trim().parse::<u64>().ok());
                let scheduler_starvation_bound_slots = lane
                    .metadata
                    .get("scheduler.starvation_bound_slots")
                    .and_then(|raw| raw.trim().parse::<u64>().ok());
                guard.insert(
                    lane.id.as_u32(),
                    LaneMetadataSnapshot {
                        alias: lane.alias.clone(),
                        dataspace_id: lane.dataspace_id,
                        dataspace_alias,
                        visibility: lane.visibility,
                        storage_profile: lane.storage,
                        lane_type: lane.lane_type.clone(),
                        governance: lane.governance.clone(),
                        settlement: lane.settlement.clone(),
                        scheduler_teu_capacity,
                        scheduler_starvation_bound_slots,
                    },
                );
            }
        }
        self.refresh_lane_metadata_cache();

        {
            let mut guard = self
                .dataspace_metadata
                .write()
                .expect("dataspace metadata lock poisoned");
            guard.clear();
            for (dataspace_id, snapshot) in &dataspace_lookup {
                guard.insert(*dataspace_id, snapshot.clone());
            }
        }
        self.refresh_dataspace_metadata_cache(&lane_ids);
        self.refresh_space_directory_active_gauges();
    }

    fn refresh_lane_metadata_cache(&self) {
        let snapshots: Vec<(u32, LaneMetadataSnapshot)> = {
            let guard = self
                .lane_metadata
                .read()
                .expect("lane metadata lock poisoned");
            guard.iter().map(|(k, v)| (*k, v.clone())).collect()
        };
        let valid_lane_ids: BTreeSet<u32> = snapshots.iter().map(|(id, _)| *id).collect();
        let mut guard = self
            .metrics
            .nexus_scheduler_lane_teu_status
            .write()
            .expect("lane TEU cache poisoned");
        for (lane_id, info) in snapshots {
            let entry = guard.entry(lane_id).or_insert_with(|| NexusLaneTeuStatus {
                lane_id,
                ..NexusLaneTeuStatus::default()
            });
            entry.lane_id = lane_id;
            if entry.block_height == 0 {
                entry.block_height = self.metrics.block_height.get();
            }
            entry.alias.clone_from(&info.alias);
            entry.dataspace_id = info.dataspace_id.as_u64();
            entry.dataspace_alias.clone_from(&info.dataspace_alias);
            entry.visibility = Some(info.visibility.as_str().to_string());
            entry.storage_profile = info.storage_profile.as_str().to_string();
            entry.lane_type.clone_from(&info.lane_type);
            entry.governance.clone_from(&info.governance);
            entry.settlement.clone_from(&info.settlement);
            entry.scheduler_teu_capacity_override = info.scheduler_teu_capacity;
            entry.scheduler_starvation_bound_override = info.scheduler_starvation_bound_slots;
            self.apply_manifest_status(LaneId::new(lane_id), entry);
        }
        guard.retain(|lane_id, _| valid_lane_ids.contains(lane_id));
    }

    fn refresh_dataspace_metadata_cache(&self, lane_ids: &[u32]) {
        let snapshot_map: BTreeMap<u64, DataspaceMetadataSnapshot> = {
            let guard = self
                .dataspace_metadata
                .read()
                .expect("dataspace metadata lock poisoned");
            guard.iter().map(|(k, v)| (*k, v.clone())).collect()
        };
        let lane_id_set: BTreeSet<u32> = lane_ids.iter().copied().collect();
        let mut guard = self
            .metrics
            .nexus_scheduler_dataspace_teu_status
            .write()
            .expect("dataspace TEU cache poisoned");
        guard.retain(|(lane_id, dataspace_id), _| {
            lane_id_set.contains(lane_id) && snapshot_map.contains_key(dataspace_id)
        });
        for lane_id in lane_ids {
            for (dataspace_id, info) in &snapshot_map {
                let entry = guard.entry((*lane_id, *dataspace_id)).or_insert_with(|| {
                    NexusDataspaceTeuStatus {
                        lane_id: *lane_id,
                        dataspace_id: *dataspace_id,
                        ..NexusDataspaceTeuStatus::default()
                    }
                });
                entry.lane_id = *lane_id;
                entry.dataspace_id = *dataspace_id;
                entry.alias.clone_from(&info.alias);
                entry.description.clone_from(&info.description);
                entry.fault_tolerance = info.fault_tolerance;
            }
        }
    }

    fn dataspace_labels(&self, dataspace: DataSpaceId) -> DataspaceLabelSet {
        let id = dataspace.as_u64();
        if let Ok(guard) = self.dataspace_metadata.read() {
            if let Some(entry) = guard.get(&id) {
                let profile = entry
                    .description
                    .clone()
                    .unwrap_or_else(|| entry.alias.clone());
                return DataspaceLabelSet {
                    alias: entry.alias.clone(),
                    profile,
                    dataspace_id: id.to_string(),
                };
            }
        }
        let fallback = format!("dataspace-{id}");
        DataspaceLabelSet {
            alias: fallback.clone(),
            profile: fallback.clone(),
            dataspace_id: id.to_string(),
        }
    }

    /// Record the configured caps and frame limits for transaction gossip.
    #[allow(clippy::too_many_arguments)]
    pub fn record_tx_gossip_caps(
        &self,
        frame_cap_bytes: usize,
        public_cap: Option<usize>,
        restricted_cap: Option<usize>,
        drop_unknown: bool,
        fallback: DataspaceGossipFallback,
        policy: RestrictedPublicPayload,
        public_target_reshuffle: std::time::Duration,
        restricted_target_reshuffle: std::time::Duration,
    ) {
        if !self.is_enabled() {
            return;
        }
        let public_target_reshuffle_ms =
            u64::try_from(public_target_reshuffle.as_millis()).unwrap_or(u64::MAX);
        let restricted_target_reshuffle_ms =
            u64::try_from(restricted_target_reshuffle.as_millis()).unwrap_or(u64::MAX);
        self.metrics
            .tx_gossip_frame_cap_bytes
            .set(frame_cap_bytes as u64);
        self.metrics
            .tx_gossip_public_target_cap
            .set(public_cap.map_or(0, |cap| cap as u64));
        self.metrics
            .tx_gossip_restricted_target_cap
            .set(restricted_cap.map_or(0, |cap| cap as u64));
        self.metrics
            .tx_gossip_public_target_reshuffle_ms
            .set(public_target_reshuffle_ms);
        self.metrics
            .tx_gossip_restricted_target_reshuffle_ms
            .set(restricted_target_reshuffle_ms);
        self.metrics
            .tx_gossip_drop_unknown_dataspace
            .set(u64::from(drop_unknown));
        self.metrics
            .tx_gossip_restricted_fallback
            .set(match fallback {
                DataspaceGossipFallback::Drop => 0,
                DataspaceGossipFallback::UsePublicOverlay => 1,
            });
        self.metrics
            .tx_gossip_restricted_public_policy
            .set(match policy {
                RestrictedPublicPayload::Forward => 1,
                RestrictedPublicPayload::Refuse => 0,
            });
        let mut caps = self
            .metrics
            .tx_gossip_caps
            .write()
            .expect("tx gossip caps cache poisoned");
        *caps = TxGossipCaps {
            frame_cap_bytes: frame_cap_bytes as u64,
            public_target_cap: public_cap.map(|cap| cap as u64),
            restricted_target_cap: restricted_cap.map(|cap| cap as u64),
            public_target_reshuffle_ms: Some(public_target_reshuffle_ms),
            restricted_target_reshuffle_ms: Some(restricted_target_reshuffle_ms),
            drop_unknown_dataspace: drop_unknown,
            restricted_fallback: match fallback {
                DataspaceGossipFallback::Drop => "drop".to_string(),
                DataspaceGossipFallback::UsePublicOverlay => "public_overlay".to_string(),
            },
            restricted_public_policy: match policy {
                RestrictedPublicPayload::Forward => "forward".to_string(),
                RestrictedPublicPayload::Refuse => "refuse".to_string(),
            },
        };
    }

    /// Record a transaction gossip attempt (sent or dropped) with labels for status/metrics.
    #[allow(clippy::too_many_arguments)]
    pub fn record_tx_gossip_attempt(
        &self,
        plane: GossipPlane,
        dataspace: DataSpaceId,
        lane_ids: &[LaneId],
        targets: &[PeerId],
        target_cap: Option<NonZeroUsize>,
        sent: bool,
        reason: Option<&str>,
        fallback_used: bool,
        fallback_surface: Option<&str>,
        batch_txs: usize,
        frame_bytes: usize,
    ) {
        if !self.is_enabled() {
            return;
        }
        let mut target_peers: Vec<String> = targets.iter().map(ToString::to_string).collect();
        target_peers.sort();
        target_peers.dedup();
        let target_count = targets.len();
        let target_cap_value = target_cap.map_or(0, NonZeroUsize::get) as u64;
        let plane_label = gossip_plane_label(plane);
        let ds_label = dataspace.as_u64().to_string();
        if sent {
            self.metrics
                .tx_gossip_sent_total
                .with_label_values(&[plane_label, ds_label.as_str()])
                .inc();
            self.metrics
                .tx_gossip_targets
                .with_label_values(&[plane_label, ds_label.as_str()])
                .set(target_count as u64);
        } else {
            let reason = reason.unwrap_or("unknown");
            self.metrics
                .tx_gossip_dropped_total
                .with_label_values(&[plane_label, ds_label.as_str(), reason])
                .inc();
            self.metrics
                .tx_gossip_targets
                .with_label_values(&[plane_label, ds_label.as_str()])
                .set(0);
        }
        if fallback_used {
            let surface = fallback_surface.unwrap_or(if sent { "forward" } else { "drop" });
            self.metrics
                .tx_gossip_fallback_total
                .with_label_values(&[plane_label, ds_label.as_str(), surface])
                .inc();
        }
        let dataspace_alias = self
            .dataspace_metadata
            .read()
            .expect("dataspace metadata lock poisoned")
            .get(&dataspace.as_u64())
            .map(|meta| meta.alias.clone())
            .filter(|alias| !alias.is_empty());
        let mut lanes: Vec<u32> = lane_ids.iter().map(|lane| lane.as_u32()).collect();
        lanes.sort_unstable();
        lanes.dedup();
        let snapshot = TxGossipStatus {
            plane: plane_label.to_string(),
            dataspace_id: dataspace.as_u64(),
            dataspace_alias,
            lane_ids: lanes,
            targets: target_count as u64,
            target_peers,
            fallback_used,
            fallback_surface: fallback_surface.map(ToOwned::to_owned),
            outcome: if sent {
                "sent".to_string()
            } else {
                "dropped".to_string()
            },
            reason: reason.map(ToOwned::to_owned),
            target_cap: target_cap_value,
            batch_txs: batch_txs as u64,
            frame_bytes: frame_bytes as u64,
        };
        let mut cache = self
            .metrics
            .tx_gossip_status
            .write()
            .expect("tx gossip status cache poisoned");
        cache.retain(|entry| {
            !(entry.plane == snapshot.plane && entry.dataspace_id == snapshot.dataspace_id)
        });
        cache.push(snapshot);
        cache.sort_by(|a, b| {
            a.plane
                .cmp(&b.plane)
                .then_with(|| a.dataspace_id.cmp(&b.dataspace_id))
        });
    }

    fn publish_space_directory_active(&self, dataspace: DataSpaceId, count: u64) {
        let labels = self.dataspace_labels(dataspace);
        self.metrics.set_space_directory_active_manifests(
            labels.alias.as_str(),
            labels.dataspace_id.as_str(),
            labels.profile.as_str(),
            count,
        );
    }

    fn refresh_space_directory_active_gauges(&self) {
        let mut dataspace_ids: BTreeSet<u64> = {
            let guard = self
                .dataspace_metadata
                .read()
                .expect("dataspace metadata lock poisoned");
            guard.keys().copied().collect()
        };
        let counts_snapshot = self
            .space_directory_active_index
            .read()
            .map(|index| {
                let snapshot = index.counts_snapshot();
                dataspace_ids.extend(snapshot.keys().copied());
                snapshot
            })
            .unwrap_or_default();
        for dataspace_id in dataspace_ids {
            let count = counts_snapshot
                .get(&dataspace_id)
                .copied()
                .unwrap_or_default();
            self.publish_space_directory_active(DataSpaceId::new(dataspace_id), count);
        }
    }

    /// Seed the cached Space Directory bindings from the on-ledger manifest registry.
    pub fn seed_space_directory_manifests(
        &self,
        manifests: &impl StorageReadOnly<UniversalAccountId, SpaceDirectoryManifestSet>,
    ) {
        let mut guard = self
            .space_directory_active_index
            .write()
            .expect("space directory cache poisoned");
        guard.clear();
        for (uaid, set) in manifests.iter() {
            for (dataspace, record) in set.iter() {
                if record.is_active() {
                    guard.insert(*uaid, dataspace.as_u64());
                }
            }
        }
        drop(guard);
        self.refresh_space_directory_active_gauges();
    }

    #[allow(dead_code)]
    fn record_space_directory_activation(&self, uaid: UniversalAccountId, dataspace: DataSpaceId) {
        let count = {
            let mut guard = self
                .space_directory_active_index
                .write()
                .expect("space directory cache poisoned");
            guard.insert(uaid, dataspace.as_u64());
            guard.count(dataspace.as_u64())
        };
        self.publish_space_directory_active(dataspace, count);
    }

    #[allow(dead_code)]
    fn record_space_directory_deactivation(
        &self,
        uaid: UniversalAccountId,
        dataspace: DataSpaceId,
    ) {
        let count = {
            let mut guard = self
                .space_directory_active_index
                .write()
                .expect("space directory cache poisoned");
            guard.remove(uaid, dataspace.as_u64());
            guard.count(dataspace.as_u64())
        };
        self.publish_space_directory_active(dataspace, count);
    }

    #[allow(dead_code)]
    fn record_space_directory_revocation(&self, dataspace: DataSpaceId, reason: Option<&str>) {
        let labels = self.dataspace_labels(dataspace);
        let reason_label = reason
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("unspecified");
        self.metrics.inc_space_directory_revocations(
            labels.alias.as_str(),
            labels.dataspace_id.as_str(),
            reason_label,
        );
    }

    /// Record per-lane settlement telemetry snapshot (State view).
    #[cfg_attr(not(feature = "telemetry"), allow(dead_code))]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn record_lane_settlement_snapshot_metrics(
        &self,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        xor_due_micro: u128,
        variance_micro: u128,
        haircut_bps: u16,
        swapline: Option<(&str, u128)>,
        buffer: Option<&crate::block::SettlementBufferSnapshot>,
    ) {
        if !self.nexus_lane_metrics_enabled() {
            return;
        }
        let lane_label = lane_id.as_u32().to_string();
        let dataspace_label = dataspace_id.as_u64().to_string();
        let buffer_metrics = buffer.map(|snapshot| LaneSettlementBuffer {
            remaining: decimal_to_f64(snapshot.remaining()),
            capacity: decimal_to_f64(snapshot.capacity()),
            status: match snapshot.status() {
                BufferStatus::Normal => 0.0,
                BufferStatus::Alert => 1.0,
                BufferStatus::Throttle => 2.0,
                BufferStatus::XorOnly => 3.0,
                BufferStatus::Halt => 4.0,
            },
        });
        let swapline_metrics = swapline.map(|(profile, utilisation_micro)| LaneSwaplineSnapshot {
            profile,
            utilisation_micro,
        });
        self.metrics
            .record_lane_settlement_snapshot(LaneSettlementSnapshot {
                lane_id: lane_label.as_str(),
                dataspace_id: dataspace_label.as_str(),
                xor_due_micro,
                variance_micro,
                haircut_bps,
                swapline: swapline_metrics,
                buffer: buffer_metrics,
            });
        self.with_lane_snapshot(lane_id, |entry| {
            entry.settlement_backlog_xor_micro = xor_due_micro;
        });
    }

    /// Increment settlement conversion counters for the provided lane/dataspace.
    #[cfg(feature = "telemetry")]
    pub(crate) fn inc_settlement_conversion_total(
        &self,
        lane_label: &str,
        dataspace_label: &str,
        source_token: &str,
        count: u64,
    ) {
        if !self.nexus_lane_metrics_enabled() || count == 0 {
            return;
        }
        self.metrics.inc_settlement_conversion_total(
            lane_label,
            dataspace_label,
            source_token,
            count,
        );
    }

    /// Increment the cumulative haircut total for the provided lane/dataspace.
    #[cfg(feature = "telemetry")]
    pub(crate) fn inc_settlement_haircut_total(
        &self,
        lane_label: &str,
        dataspace_label: &str,
        haircut_micro: u128,
    ) {
        if !self.nexus_lane_metrics_enabled() || haircut_micro == 0 {
            return;
        }
        self.metrics
            .inc_settlement_haircut_total(lane_label, dataspace_label, haircut_micro);
    }

    /// Update validator lifecycle counters for a public lane.
    #[cfg(feature = "telemetry")]
    pub fn record_public_lane_validator_status(
        &self,
        lane_id: LaneId,
        previous: Option<&PublicLaneValidatorStatus>,
        current: &PublicLaneValidatorStatus,
    ) {
        if !self.nexus_lane_metrics_enabled() {
            return;
        }
        if let Some(prev) = previous {
            if Self::validator_status_label(prev) == Self::validator_status_label(current) {
                return;
            }
            self.adjust_public_lane_validator_count(lane_id, prev, -1);
        }
        self.adjust_public_lane_validator_count(lane_id, current, 1);
    }

    /// Record a rejected public-lane validator operation grouped by reason.
    #[cfg(feature = "telemetry")]
    pub fn record_public_lane_validator_reject(&self, reason: &str) {
        if !self.nexus_lane_metrics_enabled() {
            return;
        }
        self.metrics
            .nexus_public_lane_validator_reject_total
            .with_label_values(&[reason])
            .inc();
    }

    /// Increase the bonded stake gauge for the provided lane.
    #[cfg(feature = "telemetry")]
    pub fn increase_public_lane_bonded(&self, lane_id: LaneId, amount: &Numeric) {
        self.adjust_public_lane_amount(
            &self.metrics.nexus_public_lane_stake_bonded,
            lane_id,
            amount,
            true,
        );
    }

    /// Decrease the bonded stake gauge for the provided lane.
    #[cfg(feature = "telemetry")]
    pub fn decrease_public_lane_bonded(&self, lane_id: LaneId, amount: &Numeric) {
        self.adjust_public_lane_amount(
            &self.metrics.nexus_public_lane_stake_bonded,
            lane_id,
            amount,
            false,
        );
    }

    /// Increase the pending-unbond gauge for the provided lane.
    #[cfg(feature = "telemetry")]
    pub fn increase_public_lane_pending_unbond(&self, lane_id: LaneId, amount: &Numeric) {
        self.adjust_public_lane_amount(
            &self.metrics.nexus_public_lane_unbond_pending,
            lane_id,
            amount,
            true,
        );
    }

    /// Decrease the pending-unbond gauge for the provided lane.
    #[cfg(feature = "telemetry")]
    pub fn decrease_public_lane_pending_unbond(&self, lane_id: LaneId, amount: &Numeric) {
        self.adjust_public_lane_amount(
            &self.metrics.nexus_public_lane_unbond_pending,
            lane_id,
            amount,
            false,
        );
    }

    /// Record a reward distribution for a public lane.
    #[cfg(feature = "telemetry")]
    pub fn record_public_lane_reward(&self, lane_id: LaneId, amount: &Numeric) {
        self.adjust_public_lane_amount(
            &self.metrics.nexus_public_lane_reward_total,
            lane_id,
            amount,
            true,
        );
    }

    /// Increment the slash counter for a public lane.
    #[cfg(feature = "telemetry")]
    pub fn record_public_lane_slash(&self, lane_id: LaneId) {
        if !self.nexus_lane_metrics_enabled() {
            return;
        }
        let lane_label = Self::lane_label(lane_id);
        self.metrics
            .nexus_public_lane_slash_total
            .with_label_values(&[lane_label.as_str()])
            .inc();
    }

    #[cfg(feature = "telemetry")]
    fn adjust_public_lane_validator_count(
        &self,
        lane_id: LaneId,
        status: &PublicLaneValidatorStatus,
        delta: i64,
    ) {
        if !self.nexus_lane_metrics_enabled() {
            return;
        }
        let lane_label = Self::lane_label(lane_id);
        let counter = self
            .metrics
            .nexus_public_lane_validator_total
            .with_label_values(&[lane_label.as_str(), Self::validator_status_label(status)]);
        let current = counter.get();
        let next = (current + delta).max(0);
        counter.set(next);
    }

    #[cfg(feature = "telemetry")]
    fn adjust_public_lane_amount(
        &self,
        gauge: &GaugeVec,
        lane_id: LaneId,
        amount: &Numeric,
        increase: bool,
    ) {
        if !self.nexus_lane_metrics_enabled() {
            return;
        }
        let lane_label = Self::lane_label(lane_id);
        let metric = gauge.with_label_values(&[lane_label.as_str()]);
        let delta = amount.clone().to_f64();
        let base = metric.get();
        let updated = if increase {
            base + delta
        } else {
            (base - delta).max(0.0)
        };
        metric.set(updated);
    }

    #[cfg(feature = "telemetry")]
    fn lane_label(lane_id: LaneId) -> String {
        lane_id.as_u32().to_string()
    }

    #[cfg(feature = "telemetry")]
    /// Record a public-lane validator activation event.
    pub fn record_public_lane_validator_activation(&self, lane_id: LaneId, epoch: u64) {
        if !self.nexus_lane_metrics_enabled() {
            return;
        }
        let lane_label = Self::lane_label(lane_id);
        self.metrics
            .nexus_public_lane_validator_activation_total
            .with_label_values(&[lane_label.as_str()])
            .inc();
        iroha_logger::debug!(
            lane = lane_label.as_str(),
            epoch,
            "public lane validator activated"
        );
    }

    #[cfg(feature = "telemetry")]
    fn validator_status_label(status: &PublicLaneValidatorStatus) -> &'static str {
        match status {
            PublicLaneValidatorStatus::PendingActivation(_) => "pending",
            PublicLaneValidatorStatus::Active => "active",
            PublicLaneValidatorStatus::Jailed(_) => "jailed",
            PublicLaneValidatorStatus::Exiting(_) => "exiting",
            PublicLaneValidatorStatus::Exited => "exited",
            PublicLaneValidatorStatus::Slashed(_) => "slashed",
        }
    }

    #[cfg(feature = "telemetry")]
    /// Compare the active Nexus configuration against the single-lane baseline, emitting a diff event.
    ///
    /// Returns a Norito JSON payload summarizing the changed knobs when differences are detected.
    pub fn record_nexus_config_diff(
        &self,
        nexus: &iroha_config::parameters::actual::Nexus,
    ) -> Option<norito::json::Value> {
        if !self.is_enabled() {
            return None;
        }

        let diffs = Self::collect_nexus_config_diffs(nexus);
        if diffs.is_empty() {
            return None;
        }

        for diff in &diffs {
            self.metrics
                .nexus_config_diff_total
                .with_label_values(&[diff.knob, "active"])
                .inc();
        }

        let payload: Vec<norito::json::Value> = diffs
            .iter()
            .map(|diff| {
                let mut map = norito::json::Map::new();
                map.insert("knob".to_string(), json_value(diff.knob));
                map.insert("baseline".to_string(), diff.baseline.clone());
                map.insert("current".to_string(), diff.current.clone());
                norito::json::Value::Object(map)
            })
            .collect();

        let mut root = norito::json::Map::new();
        root.insert("profile".to_string(), json_value("active"));
        root.insert("diffs".to_string(), norito::json::Value::Array(payload));
        Some(norito::json::Value::Object(root))
    }

    #[cfg(feature = "telemetry")]
    #[allow(clippy::too_many_lines)]
    fn collect_nexus_config_diffs(
        nexus: &iroha_config::parameters::actual::Nexus,
    ) -> Vec<NexusConfigDiff> {
        let mut diffs = Vec::new();

        let baseline_lane_catalog = LaneCatalog::default();
        let baseline_dataspace_catalog = DataSpaceCatalog::default();
        let baseline_registry = iroha_config::parameters::actual::LaneRegistry::default();
        let baseline_fusion = iroha_config::parameters::actual::Fusion::default();
        let baseline_commit = iroha_config::parameters::actual::Commit::default();
        let baseline_da = iroha_config::parameters::actual::Da::default();
        let baseline_governance = iroha_config::parameters::actual::GovernanceCatalog::default();

        push_nexus_diff(
            &mut diffs,
            "nexus.lane_catalog.count",
            json_value(&baseline_lane_catalog.lane_count().get()),
            json_value(&nexus.lane_catalog.lane_count().get()),
        );

        let baseline_lane_aliases: Vec<String> = baseline_lane_catalog
            .lanes()
            .iter()
            .map(|lane| lane.alias.clone())
            .collect();
        let current_lane_aliases: Vec<String> = nexus
            .lane_catalog
            .lanes()
            .iter()
            .map(|lane| lane.alias.clone())
            .collect();
        push_nexus_diff(
            &mut diffs,
            "nexus.lane_catalog.aliases",
            json_value(&baseline_lane_aliases),
            json_value(&current_lane_aliases),
        );

        let baseline_lane_details: Vec<norito::json::Value> = baseline_lane_catalog
            .lanes()
            .iter()
            .map(|lane| {
                let mut map = norito::json::Map::new();
                map.insert("id".to_string(), json_value(&lane.id.as_u32()));
                map.insert("alias".to_string(), json_value(&lane.alias));
                map.insert(
                    "dataspace_id".to_string(),
                    json_value(&lane.dataspace_id.as_u64()),
                );
                map.insert(
                    "visibility".to_string(),
                    json_value(&lane.visibility.as_str()),
                );
                map.insert("lane_type".to_string(), json_value(&lane.lane_type));
                map.insert("governance".to_string(), json_value(&lane.governance));
                map.insert("settlement".to_string(), json_value(&lane.settlement));
                map.insert("storage".to_string(), json_value(&lane.storage.as_str()));
                map.insert("metadata".to_string(), json_value(&lane.metadata));
                norito::json::Value::Object(map)
            })
            .collect();
        let current_lane_details: Vec<norito::json::Value> = nexus
            .lane_catalog
            .lanes()
            .iter()
            .map(|lane| {
                let mut map = norito::json::Map::new();
                map.insert("id".to_string(), json_value(&lane.id.as_u32()));
                map.insert("alias".to_string(), json_value(&lane.alias));
                map.insert(
                    "dataspace_id".to_string(),
                    json_value(&lane.dataspace_id.as_u64()),
                );
                map.insert(
                    "visibility".to_string(),
                    json_value(&lane.visibility.as_str()),
                );
                map.insert("lane_type".to_string(), json_value(&lane.lane_type));
                map.insert("governance".to_string(), json_value(&lane.governance));
                map.insert("settlement".to_string(), json_value(&lane.settlement));
                map.insert("storage".to_string(), json_value(&lane.storage.as_str()));
                map.insert("metadata".to_string(), json_value(&lane.metadata));
                norito::json::Value::Object(map)
            })
            .collect();
        push_nexus_diff(
            &mut diffs,
            "nexus.lane_catalog.details",
            json_value(&baseline_lane_details),
            json_value(&current_lane_details),
        );

        push_nexus_diff(
            &mut diffs,
            "nexus.dataspace_catalog.count",
            json_value(&baseline_dataspace_catalog.entries().len()),
            json_value(&nexus.dataspace_catalog.entries().len()),
        );
        let baseline_dataspace_details: Vec<norito::json::Value> = baseline_dataspace_catalog
            .entries()
            .iter()
            .map(|entry| {
                let mut map = norito::json::Map::new();
                map.insert("id".to_string(), json_value(&entry.id.as_u64()));
                map.insert("alias".to_string(), json_value(&entry.alias));
                map.insert("description".to_string(), json_value(&entry.description));
                map.insert(
                    "fault_tolerance".to_string(),
                    json_value(&entry.fault_tolerance),
                );
                norito::json::Value::Object(map)
            })
            .collect();
        let current_dataspace_details: Vec<norito::json::Value> = nexus
            .dataspace_catalog
            .entries()
            .iter()
            .map(|entry| {
                let mut map = norito::json::Map::new();
                map.insert("id".to_string(), json_value(&entry.id.as_u64()));
                map.insert("alias".to_string(), json_value(&entry.alias));
                map.insert("description".to_string(), json_value(&entry.description));
                map.insert(
                    "fault_tolerance".to_string(),
                    json_value(&entry.fault_tolerance),
                );
                norito::json::Value::Object(map)
            })
            .collect();
        push_nexus_diff(
            &mut diffs,
            "nexus.dataspace_catalog.details",
            json_value(&baseline_dataspace_details),
            json_value(&current_dataspace_details),
        );

        push_nexus_diff(
            &mut diffs,
            "nexus.routing.default_lane",
            json_value(&LaneId::SINGLE.as_u32()),
            json_value(&nexus.routing_policy.default_lane.as_u32()),
        );
        push_nexus_diff(
            &mut diffs,
            "nexus.routing.default_dataspace",
            json_value(&DataSpaceId::GLOBAL.as_u64()),
            json_value(&nexus.routing_policy.default_dataspace.as_u64()),
        );
        let routing_rules: Vec<norito::json::Value> = nexus
            .routing_policy
            .rules
            .iter()
            .map(|rule| {
                let mut matcher = norito::json::Map::new();
                matcher.insert("account".to_string(), json_value(&rule.matcher.account));
                matcher.insert(
                    "instruction".to_string(),
                    json_value(&rule.matcher.instruction),
                );
                matcher.insert(
                    "description".to_string(),
                    json_value(&rule.matcher.description),
                );

                let mut map = norito::json::Map::new();
                map.insert("lane".to_string(), json_value(&rule.lane.as_u32()));
                map.insert(
                    "dataspace".to_string(),
                    json_value(&rule.dataspace.map(DataSpaceId::as_u64)),
                );
                map.insert("matcher".to_string(), norito::json::Value::Object(matcher));
                norito::json::Value::Object(map)
            })
            .collect();
        let empty_rules: Vec<norito::json::Value> = Vec::new();
        push_nexus_diff(
            &mut diffs,
            "nexus.routing.rules",
            json_value(&empty_rules),
            json_value(&routing_rules),
        );

        let registry_manifest_baseline = baseline_registry
            .manifest_directory
            .as_ref()
            .map(|path| path.display().to_string());
        let registry_manifest_current = nexus
            .registry
            .manifest_directory
            .as_ref()
            .map(|path| path.display().to_string());
        push_nexus_diff(
            &mut diffs,
            "nexus.registry.manifest_directory",
            json_value(&registry_manifest_baseline),
            json_value(&registry_manifest_current),
        );
        let registry_cache_baseline = baseline_registry
            .cache_directory
            .as_ref()
            .map(|path| path.display().to_string());
        let registry_cache_current = nexus
            .registry
            .cache_directory
            .as_ref()
            .map(|path| path.display().to_string());
        push_nexus_diff(
            &mut diffs,
            "nexus.registry.cache_directory",
            json_value(&registry_cache_baseline),
            json_value(&registry_cache_current),
        );
        push_nexus_diff(
            &mut diffs,
            "nexus.registry.poll_interval_secs",
            json_value(&baseline_registry.poll_interval.as_secs()),
            json_value(&nexus.registry.poll_interval.as_secs()),
        );

        let fusion_to_json = |fusion: &iroha_config::parameters::actual::Fusion| {
            let mut map = norito::json::Map::new();
            map.insert("floor_teu".to_string(), json_value(&fusion.floor_teu));
            map.insert("exit_teu".to_string(), json_value(&fusion.exit_teu));
            map.insert(
                "observation_slots".to_string(),
                json_value(&fusion.observation_slots.get()),
            );
            map.insert(
                "max_window_slots".to_string(),
                json_value(&fusion.max_window_slots.get()),
            );
            norito::json::Value::Object(map)
        };
        push_nexus_diff(
            &mut diffs,
            "nexus.fusion",
            fusion_to_json(&baseline_fusion),
            fusion_to_json(&nexus.fusion),
        );

        let commit_to_json = |commit: &iroha_config::parameters::actual::Commit| {
            let mut map = norito::json::Map::new();
            map.insert(
                "window_slots".to_string(),
                json_value(&commit.window_slots.get()),
            );
            norito::json::Value::Object(map)
        };
        push_nexus_diff(
            &mut diffs,
            "nexus.commit",
            commit_to_json(&baseline_commit),
            commit_to_json(&nexus.commit),
        );

        let da_to_json = |da: &iroha_config::parameters::actual::Da| {
            let mut map = norito::json::Map::new();
            map.insert(
                "q_in_slot_total".to_string(),
                json_value(&da.q_in_slot_total.get()),
            );
            map.insert(
                "q_in_slot_per_ds_min".to_string(),
                json_value(&da.q_in_slot_per_ds_min.get()),
            );
            map.insert(
                "sample_size_base".to_string(),
                json_value(&da.sample_size_base.get()),
            );
            map.insert(
                "sample_size_max".to_string(),
                json_value(&da.sample_size_max.get()),
            );
            map.insert(
                "threshold_base".to_string(),
                json_value(&da.threshold_base.get()),
            );
            map.insert(
                "per_attester_shards".to_string(),
                json_value(&da.per_attester_shards.get()),
            );

            let mut audit = norito::json::Map::new();
            audit.insert(
                "sample_size".to_string(),
                json_value(&da.audit.sample_size.get()),
            );
            audit.insert(
                "window_count".to_string(),
                json_value(&da.audit.window_count.get()),
            );
            audit.insert(
                "interval_secs".to_string(),
                json_value(&da.audit.interval.as_secs()),
            );
            map.insert("audit".to_string(), norito::json::Value::Object(audit));

            let mut recovery = norito::json::Map::new();
            recovery.insert(
                "request_timeout_secs".to_string(),
                json_value(&da.recovery.request_timeout.as_secs()),
            );
            map.insert(
                "recovery".to_string(),
                norito::json::Value::Object(recovery),
            );

            let mut rotation = norito::json::Map::new();
            rotation.insert(
                "max_hits_per_window".to_string(),
                json_value(&da.rotation.max_hits_per_window.get()),
            );
            rotation.insert(
                "window_slots".to_string(),
                json_value(&da.rotation.window_slots.get()),
            );
            rotation.insert("seed_tag".to_string(), json_value(&da.rotation.seed_tag));
            rotation.insert(
                "latency_decay".to_string(),
                json_value(&da.rotation.latency_decay),
            );
            map.insert(
                "rotation".to_string(),
                norito::json::Value::Object(rotation),
            );

            norito::json::Value::Object(map)
        };
        push_nexus_diff(
            &mut diffs,
            "nexus.da",
            da_to_json(&baseline_da),
            da_to_json(&nexus.da),
        );

        push_nexus_diff(
            &mut diffs,
            "nexus.governance.default_module",
            json_value(&baseline_governance.default_module),
            json_value(&nexus.governance.default_module),
        );
        let governance_modules: Vec<norito::json::Value> = nexus
            .governance
            .modules
            .iter()
            .map(|(name, module)| {
                let mut map = norito::json::Map::new();
                map.insert("name".to_string(), json_value(name));
                map.insert("type".to_string(), json_value(&module.module_type));
                map.insert("params".to_string(), json_value(&module.params));
                norito::json::Value::Object(map)
            })
            .collect();
        push_nexus_diff(
            &mut diffs,
            "nexus.governance.modules",
            json_value(&Vec::<norito::json::Value>::new()),
            json_value(&governance_modules),
        );

        diffs
    }

    fn lane_metadata_snapshot(&self, lane_id: LaneId) -> LaneMetadataSnapshot {
        self.lane_metadata
            .read()
            .expect("lane metadata lock poisoned")
            .get(&lane_id.as_u32())
            .cloned()
            .unwrap_or_else(|| LaneMetadataSnapshot {
                alias: format!("lane-{}", lane_id.as_u32()),
                ..LaneMetadataSnapshot::default()
            })
    }

    fn dataspace_metadata_snapshot(&self, dataspace_id: DataSpaceId) -> DataspaceMetadataSnapshot {
        self.dataspace_metadata
            .read()
            .expect("dataspace metadata lock poisoned")
            .get(&dataspace_id.as_u64())
            .cloned()
            .unwrap_or_else(|| DataspaceMetadataSnapshot {
                alias: format!("dataspace-{}", dataspace_id.as_u64()),
                ..DataspaceMetadataSnapshot::default()
            })
    }

    fn apply_lane_metadata(&self, lane_id: LaneId, entry: &mut NexusLaneTeuStatus) {
        let info = self.lane_metadata_snapshot(lane_id);
        entry.alias.clone_from(&info.alias);
        entry.dataspace_id = info.dataspace_id.as_u64();
        entry.dataspace_alias.clone_from(&info.dataspace_alias);
        entry.visibility = Some(info.visibility.as_str().to_string());
        entry.storage_profile = info.storage_profile.as_str().to_string();
        entry.lane_type = info.lane_type;
        entry.governance = info.governance;
        entry.settlement = info.settlement;
    }

    fn apply_dataspace_metadata(
        &self,
        dataspace_id: DataSpaceId,
        entry: &mut NexusDataspaceTeuStatus,
    ) {
        let info = self.dataspace_metadata_snapshot(dataspace_id);
        entry.alias = info.alias;
        entry.description = info.description;
        entry.fault_tolerance = info.fault_tolerance;
    }

    fn apply_manifest_status(&self, lane_id: LaneId, entry: &mut NexusLaneTeuStatus) {
        let registry = self
            .lane_manifest_registry
            .read()
            .expect("manifest registry lock poisoned")
            .clone();
        if let Some(registry) = registry
            && let Some(status) = registry.status(lane_id)
        {
            entry.manifest_required = status.governance.is_some();
            entry.manifest_ready = status.manifest_path.is_some();
            entry.manifest_path = status
                .manifest_path
                .as_ref()
                .map(|path| path.display().to_string());
            entry.manifest_validators = Vec::new();
            entry.manifest_quorum = None;
            entry.manifest_protected_namespaces = Vec::new();
            entry.manifest_runtime_upgrade = None;
            if let Some(rules) = status.rules() {
                entry.manifest_validators =
                    rules.validators.iter().map(ToString::to_string).collect();
                entry.manifest_quorum = rules.quorum;
                entry.manifest_protected_namespaces = rules
                    .protected_namespaces
                    .iter()
                    .map(ToString::to_string)
                    .collect();
                entry.manifest_runtime_upgrade = rules.hooks.runtime_upgrade.as_ref().map(|hook| {
                    let allowed_ids = hook
                        .allowed_ids
                        .as_ref()
                        .map(|ids| ids.iter().cloned().collect())
                        .unwrap_or_default();
                    NexusLaneRuntimeUpgradeHookStatus {
                        allow: hook.allow,
                        require_metadata: hook.require_metadata,
                        metadata_key: hook.metadata_key.as_ref().map(ToString::to_string),
                        allowed_ids,
                    }
                });
            }
            return;
        }
        entry.manifest_required = false;
        entry.manifest_ready = true;
        entry.manifest_path = None;
        entry.manifest_validators = Vec::new();
        entry.manifest_quorum = None;
        entry.manifest_protected_namespaces = Vec::new();
        entry.manifest_runtime_upgrade = None;
    }

    /// Update manifest registry used by telemetry snapshots.
    pub fn set_lane_manifest_registry(&self, registry: LaneManifestRegistryHandle) {
        let statuses = registry.statuses();
        *self
            .lane_manifest_registry
            .write()
            .expect("manifest registry lock poisoned") = Some(registry);
        self.refresh_lane_metadata_cache();
        self.record_lane_governance_statuses(&statuses);
    }

    /// Record the outcome of applying a Nexus lane lifecycle plan.
    pub fn record_lane_lifecycle_outcome(&self, result: &str) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        self.metrics
            .nexus_lane_lifecycle_applied_total
            .with_label_values(&[result])
            .inc();
    }

    /// Update telemetry gauges reflecting governance seal status per lane.
    pub fn record_lane_governance_statuses(&self, statuses: &[LaneManifestStatus]) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        let previous_sealed: BTreeSet<String> = self
            .metrics
            .lane_governance_sealed_aliases()
            .into_iter()
            .collect();
        let mut current_sealed = BTreeSet::new();
        self.metrics.nexus_lane_governance_sealed.reset();
        let mut sealed_aliases = Vec::new();
        for status in statuses {
            let sealed = status.governance.is_some() && status.manifest_path.is_none();
            self.metrics
                .nexus_lane_governance_sealed
                .with_label_values(&[status.alias.as_str()])
                .set(u64::from(sealed));
            if sealed {
                sealed_aliases.push(status.alias.clone());
                current_sealed.insert(status.alias.clone());
            }
        }
        for alias in previous_sealed.difference(&current_sealed) {
            self.metrics
                .nexus_lane_governance_sealed
                .with_label_values(&[alias.as_str()])
                .set(0);
        }
        self.metrics
            .nexus_lane_governance_sealed_total
            .set(sealed_aliases.len() as u64);
        self.metrics
            .set_lane_governance_sealed_aliases(sealed_aliases);
    }

    /// Record the latest `SoraFS` fee projection for `provider`.
    pub fn record_sorafs_fee_projection(&self, provider: &str, fee_nanos: u128) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .record_sorafs_fee_projection(provider, fee_nanos);
        }
    }

    /// Increment the dispute submission counter for `SoraFS` capacity flows.
    pub fn inc_sorafs_disputes(&self, result: &'static str) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.inc_sorafs_disputes(result);
        }
    }

    #[cfg(feature = "telemetry")]
    /// Mirror a data-event into telemetry gauges.
    pub fn ingest_data_event(&self, event: &DataEvent) {
        match event {
            DataEvent::Governance(ev) => self.on_governance_event(ev),
            DataEvent::Social(ev) => self.on_social_event(ev),
            DataEvent::SpaceDirectory(ev) => self.on_space_directory_event(ev),
            _ => {}
        }
    }

    /// Record the OpenSSL SM preview toggle state (0/1 gauge).
    pub fn set_sm_openssl_preview(&self, enabled: bool) {
        self.metrics.sm_openssl_preview.set(u64::from(enabled));
    }

    /// Publish the active Halo2 verifier configuration (gauges + status snapshot).
    pub fn set_halo2_runtime_config(&self, cfg: ZkHalo2Config) {
        let curve_id = match cfg.curve {
            ZkCurve::Pallas => 0,
            ZkCurve::Pasta => 1,
            ZkCurve::Goldilocks => 2,
            ZkCurve::Bn254 => 3,
        };
        let backend_id = match cfg.backend {
            ZkHalo2Backend::Ipa => 0,
            ZkHalo2Backend::Unsupported => 1,
        };
        self.metrics.zk_halo2_enabled.set(u64::from(cfg.enabled));
        self.metrics.zk_halo2_curve_id.set(curve_id);
        self.metrics.zk_halo2_backend_id.set(backend_id);
        self.metrics.zk_halo2_max_k.set(u64::from(cfg.max_k));
        self.metrics
            .zk_halo2_verifier_budget_ms
            .set(cfg.verifier_budget_ms);
        self.metrics
            .zk_halo2_verifier_max_batch
            .set(u64::from(cfg.verifier_max_batch));
        let curve_label = match cfg.curve {
            ZkCurve::Pallas => "pallas",
            ZkCurve::Pasta => "pasta",
            ZkCurve::Goldilocks => "goldilocks",
            ZkCurve::Bn254 => "bn254",
        };
        let backend_label = match cfg.backend {
            ZkHalo2Backend::Ipa => "ipa",
            ZkHalo2Backend::Unsupported => "unsupported",
        };
        let mut guard = self
            .metrics
            .halo2_status
            .write()
            .expect("halo2 status lock poisoned");
        *guard = Halo2Status {
            enabled: cfg.enabled,
            curve: curve_label.to_string(),
            backend: backend_label.to_string(),
            max_k: cfg.max_k,
            verifier_budget_ms: cfg.verifier_budget_ms,
            verifier_max_batch: cfg.verifier_max_batch,
        };
    }

    #[cfg(feature = "telemetry")]
    fn on_governance_event(&self, event: &GovernanceEvent) {
        use crate::state::GovernanceProposalStatus as GPS;
        match event {
            GovernanceEvent::ProposalSubmitted(payload) => {
                self.update_governance_status(payload.id, GPS::Proposed);
            }
            GovernanceEvent::ProposalApproved(payload) => {
                self.update_governance_status(payload.id, GPS::Approved);
            }
            GovernanceEvent::ProposalRejected(payload) => {
                self.update_governance_status(payload.id, GPS::Rejected);
            }
            GovernanceEvent::ProposalEnacted(payload) => {
                self.update_governance_status(payload.id, GPS::Enacted);
            }
            GovernanceEvent::LockCreated(_) => {
                self.record_governance_bond_event("lock_created");
            }
            GovernanceEvent::LockExtended(_) => {
                self.record_governance_bond_event("lock_extended");
            }
            GovernanceEvent::LockUnlocked(_) => {
                self.record_governance_bond_event("lock_unlocked");
            }
            GovernanceEvent::LockSlashed(_) => {
                self.record_governance_bond_event("lock_slashed");
            }
            GovernanceEvent::LockRestituted(_) => {
                self.record_governance_bond_event("lock_restituted");
            }
            GovernanceEvent::CouncilPersisted(payload) => {
                self.record_council_draw(payload);
            }
            GovernanceEvent::CitizenServiceRecorded(payload) => {
                self.record_citizen_service_event(payload.event, payload.slashed);
            }
            _ => {}
        }
    }

    #[cfg(feature = "telemetry")]
    fn on_social_event(&self, event: &SocialEvent) {
        let label = match event {
            SocialEvent::RewardPaid(_) => "reward_paid",
            SocialEvent::EscrowCreated(_) => "escrow_created",
            SocialEvent::EscrowReleased(_) => "escrow_released",
            SocialEvent::EscrowCancelled(_) => "escrow_cancelled",
        };
        self.metrics
            .social_events_total
            .with_label_values(&[label])
            .inc();
        match event {
            SocialEvent::RewardPaid(payload) => {
                self.metrics
                    .social_budget_spent
                    .set(payload.budget.spent.clone().to_f64());
                self.metrics
                    .social_campaign_active
                    .set(if payload.promo_active { 1.0 } else { 0.0 });
                self.metrics
                    .social_halted
                    .set(if payload.halted { 1.0 } else { 0.0 });
                if let Some(campaign) = payload.campaign.as_ref() {
                    let spent = campaign.spent.clone();
                    let cap = payload.campaign_cap.clone();
                    self.metrics
                        .social_campaign_spent
                        .set(spent.clone().to_f64());
                    self.metrics.social_campaign_cap.set(cap.clone().to_f64());
                    let remaining = cap.checked_sub(spent).unwrap_or_else(Numeric::zero);
                    self.metrics
                        .social_campaign_remaining
                        .set(remaining.to_f64());
                } else {
                    self.metrics
                        .social_campaign_spent
                        .set(Numeric::zero().to_f64());
                    self.metrics
                        .social_campaign_cap
                        .set(payload.campaign_cap.clone().to_f64());
                    self.metrics
                        .social_campaign_remaining
                        .set(payload.campaign_cap.clone().to_f64());
                }
            }
            SocialEvent::EscrowCreated(_) => {
                self.metrics.social_open_escrows.inc();
            }
            SocialEvent::EscrowReleased(_) | SocialEvent::EscrowCancelled(_) => {
                let current = self.metrics.social_open_escrows.get();
                if current > 0 {
                    self.metrics.social_open_escrows.dec();
                } else {
                    self.metrics.social_open_escrows.set(0);
                }
            }
        }
    }

    #[cfg(feature = "telemetry")]
    fn update_governance_status(
        &self,
        proposal_id: [u8; 32],
        new_status: crate::state::GovernanceProposalStatus,
    ) {
        let prev = {
            let mut cache = self
                .governance_status_cache
                .lock()
                .expect("governance proposal cache poisoned");
            cache.insert(proposal_id, new_status)
        };
        self.record_governance_proposal_transition(prev, new_status);
    }

    #[cfg(feature = "telemetry")]
    pub(crate) fn on_space_directory_event(&self, event: &SpaceDirectoryEvent) {
        match event {
            SpaceDirectoryEvent::ManifestActivated(payload) => {
                self.record_space_directory_activation(payload.uaid, payload.dataspace);
            }
            SpaceDirectoryEvent::ManifestExpired(payload) => {
                self.record_space_directory_deactivation(payload.uaid, payload.dataspace);
            }
            SpaceDirectoryEvent::ManifestRevoked(payload) => {
                self.record_space_directory_deactivation(payload.uaid, payload.dataspace);
                self.record_space_directory_revocation(
                    payload.dataspace,
                    payload.reason.as_deref(),
                );
            }
        }
    }

    /// Record a manifest revision for the provided dataspace.
    pub fn record_space_directory_revision(&self, dataspace: DataSpaceId) {
        let labels = self.dataspace_labels(dataspace);
        self.metrics
            .inc_space_directory_revision(labels.alias.as_str(), labels.dataspace_id.as_str());
    }

    #[cfg(feature = "telemetry")]
    /// Seed governance proposal gauges and cache with explicit id/status pairs.
    pub fn seed_governance_proposals(
        &self,
        proposals: impl IntoIterator<Item = ([u8; 32], crate::state::GovernanceProposalStatus)>,
    ) {
        use crate::state::GovernanceProposalStatus as GPS;

        let mut counts = [0u64; 4];
        let mut cache = self
            .governance_status_cache
            .lock()
            .expect("governance proposal cache poisoned");
        cache.clear();
        for (id, status) in proposals {
            cache.insert(id, status);
            match status {
                GPS::Proposed => counts[0] = counts[0].saturating_add(1),
                GPS::Approved => counts[1] = counts[1].saturating_add(1),
                GPS::Rejected => counts[2] = counts[2].saturating_add(1),
                GPS::Enacted => counts[3] = counts[3].saturating_add(1),
            }
        }
        drop(cache);

        if !self.is_enabled() {
            return;
        }

        for (status, count) in [
            (GPS::Proposed, counts[0]),
            (GPS::Approved, counts[1]),
            (GPS::Rejected, counts[2]),
            (GPS::Enacted, counts[3]),
        ] {
            self.metrics
                .governance_proposals_status
                .with_label_values(&[proposal_status_label(status)])
                .set(count);
        }
    }

    /// Whether telemetry observations are enabled.
    #[inline]
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }
    /// Whether Nexus lane/dataspace telemetry is allowed.
    #[inline]
    pub fn nexus_enabled(&self) -> bool {
        self.nexus_enabled.load(Ordering::Relaxed)
    }
    /// Enable or disable Nexus lane/dataspace telemetry.
    #[inline]
    pub fn set_nexus_enabled(&self, enabled: bool) {
        self.nexus_enabled.store(enabled, Ordering::Relaxed);
        if !enabled {
            self.reset_nexus_lane_metrics();
            self.clear_nexus_cache_state();
        }
    }
    /// Whether Nexus lane/dataspace metrics should be emitted.
    #[inline]
    fn nexus_lane_metrics_enabled(&self) -> bool {
        self.is_enabled() && self.nexus_enabled()
    }

    fn reset_nexus_lane_metrics(&self) {
        reset_nexus_metrics(&self.metrics);
    }

    fn clear_nexus_cache_state(&self) {
        self.space_directory_active_index
            .write()
            .expect("space directory active index lock poisoned")
            .clear();
        self.lane_metadata
            .write()
            .expect("lane metadata lock poisoned")
            .clear();
        self.dataspace_metadata
            .write()
            .expect("dataspace metadata lock poisoned")
            .clear();
        self.lane_manifest_registry
            .write()
            .expect("lane manifest registry lock poisoned")
            .take();
        self.axt_policy_snapshot_version.store(0, Ordering::Relaxed);
        self.axt_proof_cache
            .write()
            .expect("AXT proof cache lock poisoned")
            .clear();
        self.axt_last_policy_reject
            .write()
            .expect("AXT last reject cache lock poisoned")
            .take();
        self.axt_reject_hints
            .write()
            .expect("AXT reject hints cache lock poisoned")
            .clear();
        #[cfg(feature = "telemetry")]
        {
            self.governance_status_cache
                .lock()
                .expect("governance status cache lock poisoned")
                .clear();
        }
    }
    /// Enable or disable telemetry observations at runtime.
    #[inline]
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }
    /// Enable telemetry observations at runtime.
    #[inline]
    pub fn enable(&self) {
        self.set_enabled(true);
    }
    /// Disable telemetry observations at runtime.
    #[inline]
    pub fn disable(&self) {
        self.set_enabled(false);
    }

    /// Record an ISI execution attempt.
    pub fn record_isi_total(&self, name: &str) {
        if !self.is_enabled() {
            return;
        }
        self.metrics.isi.with_label_values(&[name, "total"]).inc();
    }

    /// Record a successful ISI execution.
    pub fn record_isi_success(&self, name: &str) {
        if !self.is_enabled() {
            return;
        }
        self.metrics.isi.with_label_values(&[name, "success"]).inc();
    }

    /// Record ISI execution latency.
    pub fn record_isi_time(&self, name: &str, elapsed: Duration) {
        if !self.is_enabled() {
            return;
        }
        let elapsed_ms = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX);
        self.metrics
            .isi_times
            .with_label_values(&[name])
            .observe(u64_to_f64(elapsed_ms));
    }

    fn record_lane_placeholders(&self, lane_id: LaneId) {
        self.record_lane_with_dataspace(lane_id, DataSpaceId::GLOBAL);
    }

    fn record_lane_with_dataspace(&self, lane_id: LaneId, dataspace_id: DataSpaceId) {
        self.metrics
            .nexus_lane_id_placeholder
            .set(u64::from(lane_id));
        self.metrics
            .nexus_dataspace_id_placeholder
            .set(dataspace_id.as_u64());
    }

    fn lane_label_values(&self, lane_id: LaneId) -> (String, String) {
        let lane_label = lane_id.as_u32().to_string();
        let dataspace_label = self
            .lane_metadata
            .read()
            .ok()
            .and_then(|guard| guard.get(&lane_id.as_u32()).map(|entry| entry.dataspace_id))
            .unwrap_or(DataSpaceId::GLOBAL)
            .as_u64()
            .to_string();
        (lane_label, dataspace_label)
    }

    fn with_lane_snapshot<F>(&self, lane_id: LaneId, update: F)
    where
        F: FnOnce(&mut NexusLaneTeuStatus),
    {
        let mut guard = self
            .metrics
            .nexus_scheduler_lane_teu_status
            .write()
            .expect("lane TEU cache poisoned");
        let entry = guard
            .entry(lane_id.as_u32())
            .or_insert_with(|| NexusLaneTeuStatus {
                lane_id: lane_id.as_u32(),
                ..NexusLaneTeuStatus::default()
            });
        entry.lane_id = lane_id.as_u32();
        self.apply_lane_metadata(lane_id, entry);
        self.apply_manifest_status(lane_id, entry);
        update(entry);
    }

    fn lane_headroom_pct(update: &LaneTeuGaugeUpdate) -> u64 {
        if update.capacity == 0 {
            return 100;
        }
        update.buckets.headroom.saturating_mul(100) / update.capacity.max(1)
    }

    fn should_emit_lane_headroom_event(headroom_pct: u64, update: &LaneTeuGaugeUpdate) -> bool {
        update.capacity > 0
            && (update.buckets.headroom == 0
                || headroom_pct <= NEXUS_HEADROOM_WARN_PCT
                || update.trigger_level > 0)
    }

    fn emit_lane_headroom_event(
        &self,
        lane_id: LaneId,
        snapshot: &NexusLaneTeuStatus,
        update: &LaneTeuGaugeUpdate,
        headroom_pct: u64,
    ) -> Option<norito::json::Value> {
        if !self.is_enabled() {
            return None;
        }
        let payload = Self::build_lane_headroom_payload(lane_id, snapshot, update, headroom_pct);
        match Json::try_new(payload.clone()) {
            Ok(json) => {
                iroha_logger::telemetry!(msg = "nexus.scheduler.headroom", event = %json)
            }
            Err(err) => iroha_logger::warn!(
                ?err,
                lane = lane_id.as_u32(),
                "failed to encode lane headroom telemetry"
            ),
        }
        Some(payload)
    }

    fn build_lane_headroom_payload(
        lane_id: LaneId,
        snapshot: &NexusLaneTeuStatus,
        update: &LaneTeuGaugeUpdate,
        headroom_pct: u64,
    ) -> norito::json::Value {
        use norito::json::{Map, Value};

        let mut buckets = Map::new();
        buckets.insert("floor".into(), Value::from(update.buckets.floor));
        buckets.insert("headroom".into(), Value::from(update.buckets.headroom));
        buckets.insert("must_serve".into(), Value::from(update.buckets.must_serve));
        buckets.insert(
            "circuit_breaker".into(),
            Value::from(update.buckets.circuit_breaker),
        );

        let mut deferrals = Map::new();
        deferrals.insert(
            "cap_exceeded".into(),
            Value::from(snapshot.deferrals.cap_exceeded),
        );
        deferrals.insert(
            "envelope_limit".into(),
            Value::from(snapshot.deferrals.envelope_limit),
        );
        deferrals.insert("quota".into(), Value::from(snapshot.deferrals.quota));
        deferrals.insert(
            "circuit_breaker".into(),
            Value::from(snapshot.deferrals.circuit_breaker),
        );

        let mut payload = Map::new();
        payload.insert("lane_id".into(), Value::from(lane_id.as_u32()));
        payload.insert("capacity".into(), Value::from(update.capacity));
        payload.insert("committed".into(), Value::from(update.committed));
        payload.insert("headroom_teu".into(), Value::from(update.buckets.headroom));
        payload.insert("headroom_pct".into(), Value::from(headroom_pct));
        payload.insert("trigger_level".into(), Value::from(update.trigger_level));
        payload.insert(
            "starvation_bound_slots".into(),
            Value::from(update.starvation_bound_slots),
        );
        payload.insert("buckets".into(), Value::Object(buckets));
        payload.insert("deferrals".into(), Value::Object(deferrals));
        payload.insert(
            "must_serve_truncations".into(),
            Value::from(snapshot.must_serve_truncations),
        );
        Value::Object(payload)
    }

    #[cfg(feature = "telemetry")]
    /// Record per-lane pipeline summary data for the latest block.
    pub fn record_lane_pipeline_summary(&self, lane_id: LaneId, summary: LanePipelineSummary) {
        if !self.nexus_lane_metrics_enabled() {
            return;
        }
        let (lane_label, dataspace_label) = self.lane_label_values(lane_id);
        self.metrics.set_lane_block_height(
            lane_label.as_str(),
            dataspace_label.as_str(),
            summary.block_height,
        );
        self.with_lane_snapshot(lane_id, |entry| {
            entry.block_height = summary.block_height;
            entry.finality_lag_slots = 0;
            entry.tx_vertices = summary.tx_vertices;
            entry.tx_edges = summary.tx_edges;
            entry.overlay_count = summary.overlay_count;
            entry.overlay_instr_total = summary.overlay_instr_total;
            entry.overlay_bytes_total = summary.overlay_bytes_total;
            entry.rbc_chunks = summary.rbc_chunks;
            entry.rbc_bytes_total = summary.rbc_bytes_total;
            entry.peak_layer_width = summary.peak_layer_width;
            entry.layer_count = summary.layer_count;
            entry.avg_layer_width = summary.avg_layer_width;
            entry.median_layer_width = summary.median_layer_width;
            entry.scheduler_utilization_pct = summary.scheduler_utilization_pct;
            entry.layer_width_buckets = summary.layer_width_buckets;
            entry.detached_prepared = summary.detached_prepared;
            entry.detached_merged = summary.detached_merged;
            entry.detached_fallback = summary.detached_fallback;
            entry.quarantine_executed = summary.quarantine_executed;
        });
    }

    /// Refresh per-lane finality lag gauges relative to the provided head height.
    pub fn update_lane_finality_lag(&self, head_height: u64) {
        if !self.is_enabled() {
            return;
        }
        let mut updates = Vec::new();
        {
            let mut guard = self
                .metrics
                .nexus_scheduler_lane_teu_status
                .write()
                .expect("lane TEU cache poisoned");
            for entry in guard.values_mut() {
                let lag = head_height.saturating_sub(entry.block_height);
                entry.finality_lag_slots = lag;
                updates.push((entry.lane_id, entry.dataspace_id, entry.block_height, lag));
            }
        }
        for (lane_id, dataspace_id, block_height, lag) in updates {
            let lane_label = lane_id.to_string();
            let dataspace_label = dataspace_id.to_string();
            self.metrics
                .set_lane_block_height(&lane_label, &dataspace_label, block_height);
            self.metrics
                .set_lane_finality_lag(&lane_label, &dataspace_label, lag);
        }
    }

    /// Record finality information derived from a lane relay envelope.
    pub fn record_lane_relay_finality(
        &self,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        block_height: u64,
        head_height: u64,
        rbc_bytes_total: u64,
    ) {
        if !self.nexus_lane_metrics_enabled() {
            return;
        }
        self.record_lane_with_dataspace(lane_id, dataspace_id);
        let lag = head_height.saturating_sub(block_height);
        let (lane_label, dataspace_label) = self.lane_label_values(lane_id);
        self.metrics.set_lane_block_height(
            lane_label.as_str(),
            dataspace_label.as_str(),
            block_height,
        );
        self.metrics
            .set_lane_finality_lag(lane_label.as_str(), dataspace_label.as_str(), lag);
        self.with_lane_snapshot(lane_id, |entry| {
            entry.block_height = block_height;
            entry.finality_lag_slots = lag;
            entry.rbc_bytes_total = rbc_bytes_total;
        });
    }

    /// Record use of emergency validator overrides during lane relay validation.
    pub fn record_lane_relay_emergency_override(
        &self,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        outcome: &str,
    ) {
        if !self.nexus_lane_metrics_enabled() {
            return;
        }
        self.record_lane_with_dataspace(lane_id, dataspace_id);
        let (lane_label, dataspace_label) = self.lane_label_values(lane_id);
        self.metrics
            .lane_relay_emergency_override_total
            .with_label_values(&[lane_label.as_str(), dataspace_label.as_str(), outcome])
            .inc();
    }

    fn with_dataspace_snapshot<F>(&self, lane_id: LaneId, dataspace_id: DataSpaceId, update: F)
    where
        F: FnOnce(&mut NexusDataspaceTeuStatus),
    {
        let mut guard = self
            .metrics
            .nexus_scheduler_dataspace_teu_status
            .write()
            .expect("dataspace TEU cache poisoned");
        let key = (lane_id.as_u32(), dataspace_id.as_u64());
        let entry = guard.entry(key).or_insert_with(|| NexusDataspaceTeuStatus {
            lane_id: lane_id.as_u32(),
            dataspace_id: dataspace_id.as_u64(),
            ..NexusDataspaceTeuStatus::default()
        });
        entry.lane_id = lane_id.as_u32();
        entry.dataspace_id = dataspace_id.as_u64();
        self.apply_dataspace_metadata(dataspace_id, entry);
        update(entry);
    }

    #[cfg(feature = "telemetry")]
    /// Record per-dataspace pipeline summary data for the latest block.
    pub fn record_dataspace_pipeline_summary(
        &self,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        summary: DataspacePipelineSummary,
    ) {
        if !self.nexus_lane_metrics_enabled() {
            return;
        }
        self.with_dataspace_snapshot(lane_id, dataspace_id, |entry| {
            entry.tx_served = summary.tx_served;
        });
    }

    /// Current unix timestamp in milliseconds (used for settlement event snapshots).
    #[inline]
    fn now_millis(&self) -> u64 {
        let now = self.time_source.now().as_millis();
        u64::try_from(now).unwrap_or(u64::MAX)
    }

    /// Record a successful SM helper syscall invocation.
    pub fn note_sm_syscall_success(&self, kind: &'static str, mode: &'static str) {
        if self.is_enabled() {
            self.metrics
                .sm_syscall_total
                .with_label_values(&[kind, mode])
                .inc();
        }
    }

    /// Record a failed SM helper syscall invocation with a reason label.
    pub fn note_sm_syscall_failure(
        &self,
        kind: &'static str,
        mode: &'static str,
        reason: &'static str,
    ) {
        if self.is_enabled() {
            self.metrics
                .sm_syscall_failures_total
                .with_label_values(&[kind, mode, reason])
                .inc();
        }
    }

    /// Record an AXT policy rejection grouped by lane id and reason.
    pub fn note_axt_policy_reject(
        &self,
        lane: LaneId,
        reason: AxtRejectReason,
        snapshot_version: u64,
    ) {
        let resolved_version = if snapshot_version == 0 {
            self.axt_policy_snapshot_version.load(Ordering::SeqCst)
        } else {
            snapshot_version
        };
        {
            let mut guard = self
                .axt_last_policy_reject
                .write()
                .expect("axt reject cache poisoned");
            *guard = Some(AxtPolicyRejectSnapshot {
                lane,
                reason,
                snapshot_version: resolved_version,
            });
        }
        if self.nexus_lane_metrics_enabled() {
            let lane_label = lane.as_u32().to_string();
            let reason_label = reason.label();
            self.metrics
                .axt_policy_reject_total
                .with_label_values(&[lane_label.as_str(), reason_label])
                .inc();
        }
    }

    /// Record which source supplied the latest AXT policy snapshot (cache hit/miss).
    /// Record whether AXT policy hydration pulled from cache or required a rebuild.
    ///
    /// Canonical `event` labels: `cache_hit` and `cache_miss`.
    pub fn note_axt_policy_snapshot_cache_event(&self, event: &'static str) {
        if self.nexus_lane_metrics_enabled() {
            self.metrics
                .axt_policy_snapshot_cache_events_total
                .with_label_values(&[event])
                .inc();
        }
    }

    /// Record an AXT proof cache event grouped by label.
    ///
    /// Canonical `event` labels: `hit`, `miss`, `expired`, `cleared`, `pruned`.
    pub fn note_axt_proof_cache_event(&self, event: &'static str) {
        if self.nexus_lane_metrics_enabled() {
            self.metrics
                .axt_proof_cache_events_total
                .with_label_values(&[event])
                .inc();
        }
    }

    fn manifest_root_or_zero(manifest_root: Option<[u8; 32]>) -> [u8; 32] {
        manifest_root.unwrap_or([0; 32])
    }

    fn manifest_root_label(manifest_root: Option<[u8; 32]>) -> String {
        hex::encode(Self::manifest_root_or_zero(manifest_root))
    }

    fn remove_axt_proof_cache_metric(&self, entry: &AxtProofCacheStatus) {
        let ds_label = entry.dataspace.as_u64().to_string();
        let slot_label = entry.verified_slot.to_string();
        let root_hex = Self::manifest_root_label(entry.manifest_root);
        let _ = self.metrics.axt_proof_cache_state.remove_label_values(&[
            ds_label.as_str(),
            entry.status.as_str(),
            root_hex.as_str(),
            slot_label.as_str(),
        ]);
    }

    /// Publish the current AXT proof cache state for a dataspace.
    ///
    /// Labels: `dsid`, `status`, `manifest_root_hex`, `verified_slot`; value: `expiry_slot` (with skew).
    pub fn set_axt_proof_cache_state(
        &self,
        dsid: DataSpaceId,
        status: &'static str,
        manifest_root: [u8; 32],
        verified_slot: u64,
        expiry_slot: Option<u64>,
    ) {
        if !self.is_enabled() {
            return;
        }
        let expiry_slot_raw = expiry_slot;
        let ds_label = dsid.as_u64().to_string();
        let slot_label = verified_slot.to_string();
        let expiry_slot = expiry_slot_raw.map_or(0, |slot| i64::try_from(slot).unwrap_or(i64::MAX));
        let manifest_root_opt = manifest_root
            .iter()
            .any(|byte| *byte != 0)
            .then_some(manifest_root);
        if let Ok(mut guard) = self.axt_proof_cache.write() {
            if let Some(prev) = guard.get(&dsid) {
                self.remove_axt_proof_cache_metric(prev);
            }
            guard.insert(
                dsid,
                AxtProofCacheStatus {
                    dataspace: dsid,
                    status: status.to_owned(),
                    manifest_root: manifest_root_opt,
                    verified_slot,
                    expiry_slot: expiry_slot_raw,
                },
            );
        }
        let root_hex = hex::encode(manifest_root);
        self.metrics
            .axt_proof_cache_state
            .with_label_values(&[
                ds_label.as_str(),
                status,
                root_hex.as_str(),
                slot_label.as_str(),
            ])
            .set(expiry_slot);
    }

    /// Remove cached AXT proof state for a dataspace.
    pub fn clear_axt_proof_cache_state(
        &self,
        dsid: DataSpaceId,
        status: &'static str,
        manifest_root: [u8; 32],
        verified_slot: u64,
    ) {
        if !self.is_enabled() {
            return;
        }
        let ds_label = dsid.as_u64().to_string();
        let slot_label = verified_slot.to_string();
        let manifest_root_opt = manifest_root
            .iter()
            .any(|byte| *byte != 0)
            .then_some(manifest_root);
        if let Ok(mut guard) = self.axt_proof_cache.write() {
            if let Some(prev) = guard.get(&dsid) {
                self.remove_axt_proof_cache_metric(prev);
            }
            guard.insert(
                dsid,
                AxtProofCacheStatus {
                    dataspace: dsid,
                    status: String::from("cleared"),
                    manifest_root: manifest_root_opt,
                    verified_slot,
                    expiry_slot: None,
                },
            );
        }
        let root_hex = hex::encode(manifest_root);
        let _ = self.metrics.axt_proof_cache_state.remove_label_values(&[
            ds_label.as_str(),
            status,
            root_hex.as_str(),
            slot_label.as_str(),
        ]);
        self.metrics
            .axt_proof_cache_state
            .with_label_values(&[
                ds_label.as_str(),
                "cleared",
                root_hex.as_str(),
                slot_label.as_str(),
            ])
            .set(0);
    }

    /// Snapshot the current AXT proof cache state for diagnostics and debug endpoints.
    #[must_use]
    pub fn axt_proof_cache_status_snapshot(&self) -> Vec<AxtProofCacheStatus> {
        if !self.is_enabled() {
            return Vec::new();
        }
        self.axt_proof_cache
            .read()
            .map(|map| map.values().cloned().collect())
            .unwrap_or_default()
    }

    /// Record a hint describing the next required handle era/sub-nonce after a rejection.
    pub fn set_axt_reject_hint(
        &self,
        dsid: DataSpaceId,
        target_lane: LaneId,
        next_min_handle_era: u64,
        next_min_sub_nonce: u64,
        reason: AxtRejectReason,
    ) {
        if !self.is_enabled() {
            return;
        }
        if let Ok(mut guard) = self.axt_reject_hints.write() {
            guard.insert(
                dsid,
                AxtRejectHint {
                    dataspace: dsid,
                    target_lane,
                    next_min_handle_era,
                    next_min_sub_nonce,
                    reason,
                },
            );
        }
    }

    /// Snapshot the current set of AXT reject hints.
    #[must_use]
    pub fn axt_reject_hints_snapshot(&self) -> Vec<AxtRejectHint> {
        if !self.is_enabled() {
            return Vec::new();
        }
        self.axt_reject_hints
            .read()
            .map(|map| map.values().copied().collect())
            .unwrap_or_default()
    }

    /// Set the version hash for the AXT policy snapshot used when hydrating the host.
    pub fn set_axt_policy_snapshot_version(&self, snapshot: &AxtPolicySnapshot) {
        let version = if snapshot.entries.is_empty() {
            0
        } else if snapshot.version != 0 {
            snapshot.version
        } else {
            AxtPolicySnapshot::compute_version(&snapshot.entries)
        };
        self.axt_policy_snapshot_version
            .store(version, Ordering::SeqCst);
        if !self.is_enabled() {
            return;
        }
        self.metrics.axt_policy_snapshot_version.set(version);
    }

    /// Snapshot the most recent AXT policy rejection (if any).
    #[must_use]
    pub fn axt_policy_reject_snapshot(&self) -> Option<AxtPolicyRejectSnapshot> {
        self.axt_last_policy_reject
            .read()
            .ok()
            .and_then(|guard| *guard)
    }

    /// Latest recorded AXT policy snapshot version.
    #[must_use]
    pub fn axt_policy_snapshot_version_value(&self) -> u64 {
        self.axt_policy_snapshot_version.load(Ordering::SeqCst)
    }

    /// Composite snapshot containing the policy version, last rejection, cache state, and hints.
    #[must_use]
    pub fn axt_debug_status(&self) -> AxtDebugStatus {
        AxtDebugStatus {
            policy_snapshot_version: self.axt_policy_snapshot_version_value(),
            last_reject: self.axt_policy_reject_snapshot(),
            cache: self.axt_proof_cache_status_snapshot(),
            hints: self.axt_reject_hints_snapshot(),
        }
    }

    /// Record a settlement completion event.
    pub fn note_settlement_success(&self, kind: &'static str) {
        if self.is_enabled() {
            self.metrics
                .settlement_events_total
                .with_label_values(&[kind, "success", "-"])
                .inc();
        }
    }

    /// Record a settlement failure with a categorized reason.
    pub fn note_settlement_failure(&self, kind: &'static str, reason: &'static str) {
        if self.is_enabled() {
            self.metrics
                .settlement_events_total
                .with_label_values(&[kind, "failure", reason])
                .inc();
        }
    }

    fn settlement_order_label(order: SettlementExecutionOrder) -> &'static str {
        match order {
            SettlementExecutionOrder::DeliveryThenPayment => "delivery_then_payment",
            SettlementExecutionOrder::PaymentThenDelivery => "payment_then_delivery",
        }
    }

    fn settlement_atomicity_label(atomicity: SettlementAtomicity) -> &'static str {
        match atomicity {
            SettlementAtomicity::AllOrNothing => "all_or_nothing",
            SettlementAtomicity::CommitFirstLeg => "commit_first_leg",
            SettlementAtomicity::CommitSecondLeg => "commit_second_leg",
        }
    }

    fn dvp_final_state_label(delivery_committed: bool, payment_committed: bool) -> &'static str {
        match (delivery_committed, payment_committed) {
            (false, false) => "none",
            (true, false) => "delivery_only",
            (false, true) => "payment_only",
            (true, true) => "both",
        }
    }

    fn pvp_final_state_label(primary_committed: bool, counter_committed: bool) -> &'static str {
        match (primary_committed, counter_committed) {
            (false, false) => "none",
            (true, false) => "primary_only",
            (false, true) => "counter_only",
            (true, true) => "both",
        }
    }

    /// Record `DvP` settlement finality state for status/metrics surfaces.
    pub fn record_dvp_finality(
        &self,
        settlement_id: &SettlementId,
        plan: SettlementPlan,
        outcome: SettlementOutcomeKind,
        failure_reason: Option<&'static str>,
        delivery_committed: bool,
        payment_committed: bool,
    ) {
        if !self.is_enabled() {
            return;
        }
        let final_state_label = Self::dvp_final_state_label(delivery_committed, payment_committed);
        self.metrics
            .settlement_finality_events_total
            .with_label_values(&[SETTLEMENT_KIND_DVP, outcome.as_str(), final_state_label])
            .inc();
        let observed_at_ms = self.now_millis();
        status::record_dvp_settlement_event(status::DvpSettlementEventUpdate {
            observed_at_ms,
            settlement_id: Some(settlement_id.to_string()),
            plan_order: plan.order(),
            plan_atomicity: plan.atomicity(),
            outcome,
            failure_reason: failure_reason.map(str::to_owned),
            final_state_label: final_state_label.to_owned(),
            delivery_committed,
            payment_committed,
        });
    }

    /// Record `PvP` settlement finality state and FX window telemetry.
    #[allow(clippy::too_many_arguments)]
    pub fn record_pvp_finality(
        &self,
        settlement_id: &SettlementId,
        plan: SettlementPlan,
        outcome: SettlementOutcomeKind,
        failure_reason: Option<&'static str>,
        primary_committed: bool,
        counter_committed: bool,
        fx_window_ms: Option<u64>,
    ) {
        if !self.is_enabled() {
            return;
        }
        let final_state_label = Self::pvp_final_state_label(primary_committed, counter_committed);
        self.metrics
            .settlement_finality_events_total
            .with_label_values(&[SETTLEMENT_KIND_PVP, outcome.as_str(), final_state_label])
            .inc();
        if let Some(window_ms) = fx_window_ms {
            self.metrics
                .settlement_fx_window_ms
                .with_label_values(&[
                    SETTLEMENT_KIND_PVP,
                    Self::settlement_order_label(plan.order()),
                    Self::settlement_atomicity_label(plan.atomicity()),
                ])
                .observe(u64_to_f64(window_ms));
        }
        let observed_at_ms = self.now_millis();
        status::record_pvp_settlement_event(status::PvpSettlementEventUpdate {
            observed_at_ms,
            settlement_id: Some(settlement_id.to_string()),
            plan_order: plan.order(),
            plan_atomicity: plan.atomicity(),
            outcome,
            failure_reason: failure_reason.map(str::to_owned),
            final_state_label: final_state_label.to_owned(),
            primary_committed,
            counter_committed,
            fx_window_ms,
        });
    }

    /// Record a PSP fraud assessment missing event (e.g., missing metadata or grace).
    pub fn record_fraud_missing_assessment(
        &self,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        dataspace_label: &str,
        tenant: &str,
        cause: &'static str,
    ) {
        if self.nexus_lane_metrics_enabled() {
            self.record_lane_with_dataspace(lane_id, dataspace_id);
            let tenant_trimmed = tenant.trim();
            let tenant_label = if tenant_trimmed.is_empty() {
                "unknown"
            } else {
                tenant_trimmed
            };
            let lane_label = lane_id.as_u32().to_string();
            let labels = [tenant_label, lane_label.as_str(), dataspace_label, cause];
            self.metrics
                .fraud_psp_missing_assessment_total
                .with_label_values(&labels)
                .inc();
        }
    }

    /// Record that a merge-ledger entry was committed.
    pub fn record_merge_ledger_entry(&self, epoch_id: u64, global_state_root: &Hash) {
        if self.is_enabled() {
            self.metrics.merge_ledger_entries_total.inc();
            self.metrics.merge_ledger_latest_epoch.set(epoch_id);
            if let Ok(mut guard) = self.metrics.merge_ledger_latest_root_hex.write() {
                *guard = Some(global_state_root.to_string());
            }
        }
    }

    /// Record a Kaigi relay registration event.
    pub fn record_kaigi_relay_registration(
        &self,
        domain: &iroha_data_model::domain::DomainId,
        bandwidth_class: u8,
    ) {
        if self.is_enabled() {
            let domain_label = domain.to_string();
            self.metrics
                .kaigi_relay_registered_total
                .with_label_values(&[domain_label.as_str()])
                .inc();
            self.metrics
                .kaigi_relay_registration_bandwidth
                .with_label_values(&[domain_label.as_str()])
                .observe(u64_to_f64(u64::from(bandwidth_class)));
        }
    }

    /// Record a Kaigi relay manifest update.
    pub fn record_kaigi_manifest_update(
        &self,
        domain: &iroha_data_model::domain::DomainId,
        action: &'static str,
        hop_count: u32,
    ) {
        if self.is_enabled() {
            let domain_label = domain.to_string();
            self.metrics
                .kaigi_relay_manifest_updates_total
                .with_label_values(&[domain_label.as_str(), action])
                .inc();
            self.metrics
                .kaigi_relay_manifest_hop_count
                .with_label_values(&[domain_label.as_str()])
                .observe(u64_to_f64(u64::from(hop_count)));
        }
    }

    /// Record a Kaigi relay failover event.
    pub fn record_kaigi_failover(&self, domain: &DomainId, call: &Name, hop_count: u32) {
        if self.is_enabled() {
            let domain_label = domain.to_string();
            let call_label = call.to_string();
            self.metrics
                .kaigi_relay_failover_total
                .with_label_values(&[domain_label.as_str(), call_label.as_str()])
                .inc();
            self.metrics
                .kaigi_relay_failover_hop_count
                .with_label_values(&[domain_label.as_str()])
                .observe(u64_to_f64(u64::from(hop_count)));
        }
    }

    /// Record a Kaigi relay health update.
    pub fn record_kaigi_relay_health(
        &self,
        domain: &DomainId,
        relay: &AccountId,
        status: KaigiRelayHealthStatus,
    ) {
        if self.is_enabled() {
            let domain_label = domain.to_string();
            let relay_label = relay.to_string();
            let status_label = status.label();
            self.metrics
                .kaigi_relay_health_reports_total
                .with_label_values(&[domain_label.as_str(), status_label])
                .inc();
            self.metrics
                .kaigi_relay_health_state
                .with_label_values(&[domain_label.as_str(), relay_label.as_str()])
                .set(status.metric_value());
        }
    }

    /// Record per-lane TEU metrics for the latest scheduler envelope.
    pub fn record_nexus_scheduler_lane_teu(&self, lane_id: LaneId, update: LaneTeuGaugeUpdate) {
        if self.nexus_lane_metrics_enabled() {
            self.record_lane_placeholders(lane_id);
            let lane_label = lane_id.as_u32().to_string();
            self.metrics
                .nexus_scheduler_lane_teu_capacity
                .with_label_values(&[lane_label.as_str()])
                .set(update.capacity);
            self.metrics
                .nexus_scheduler_lane_teu_slot_committed
                .with_label_values(&[lane_label.as_str()])
                .set(update.committed);
            self.metrics
                .nexus_scheduler_lane_trigger_level
                .with_label_values(&[lane_label.as_str()])
                .set(update.trigger_level);
            self.metrics
                .nexus_scheduler_starvation_bound_slots
                .with_label_values(&[lane_label.as_str()])
                .set(update.starvation_bound_slots);
            for (bucket, value) in update.buckets.iter() {
                self.metrics
                    .nexus_scheduler_lane_teu_slot_breakdown
                    .with_label_values(&[lane_label.as_str(), bucket])
                    .set(value);
            }
            let headroom_pct = Self::lane_headroom_pct(&update);
            let emit_headroom_event = Self::should_emit_lane_headroom_event(headroom_pct, &update);
            if emit_headroom_event {
                self.metrics
                    .nexus_scheduler_lane_headroom_events_total
                    .with_label_values(&[lane_label.as_str()])
                    .inc();
            }
            self.with_lane_snapshot(lane_id, |snapshot| {
                snapshot.capacity = update.capacity;
                snapshot.committed = update.committed;
                snapshot.buckets = update.buckets;
                snapshot.trigger_level = update.trigger_level;
                snapshot.starvation_bound_slots = update.starvation_bound_slots;
                if emit_headroom_event {
                    let _ = self.emit_lane_headroom_event(lane_id, snapshot, &update, headroom_pct);
                }
            });
        }
    }

    /// Increment TEU deferral counters for the provided lane and reason.
    pub fn inc_nexus_scheduler_lane_teu_deferral(
        &self,
        lane_id: LaneId,
        reason: &'static str,
        amount: u64,
    ) {
        if self.nexus_lane_metrics_enabled() {
            self.record_lane_placeholders(lane_id);
            let lane_label = lane_id.as_u32().to_string();
            self.metrics
                .nexus_scheduler_lane_teu_deferral_total
                .with_label_values(&[lane_label.as_str(), reason])
                .inc_by(amount);
            self.with_lane_snapshot(lane_id, |snapshot| {
                snapshot.deferrals.increment(reason, amount);
            });
        }
    }

    /// Increment the must-serve truncation counter for the provided lane.
    pub fn inc_nexus_scheduler_must_serve_truncations(&self, lane_id: LaneId, amount: u64) {
        if self.nexus_lane_metrics_enabled() {
            self.record_lane_placeholders(lane_id);
            let lane_label = lane_id.as_u32().to_string();
            self.metrics
                .nexus_scheduler_must_serve_truncations_total
                .with_label_values(&[lane_label.as_str()])
                .inc_by(amount);
            self.with_lane_snapshot(lane_id, |snapshot| {
                snapshot.must_serve_truncations =
                    snapshot.must_serve_truncations.saturating_add(amount);
            });
        }
    }

    /// Record per-dataspace TEU backlog metrics for the latest scheduler envelope.
    pub fn record_nexus_scheduler_dataspace_teu(
        &self,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        update: DataspaceTeuGaugeUpdate,
    ) {
        if self.nexus_lane_metrics_enabled() {
            self.record_lane_placeholders(lane_id);
            let lane_label = lane_id.as_u32().to_string();
            let ds_label = dataspace_id.as_u64().to_string();
            self.metrics
                .nexus_scheduler_dataspace_teu_backlog
                .with_label_values(&[lane_label.as_str(), ds_label.as_str()])
                .set(update.backlog);
            self.metrics
                .nexus_scheduler_dataspace_age_slots
                .with_label_values(&[lane_label.as_str(), ds_label.as_str()])
                .set(update.age_slots);
            self.metrics
                .nexus_scheduler_dataspace_virtual_finish
                .with_label_values(&[lane_label.as_str(), ds_label.as_str()])
                .set(update.virtual_finish);
            self.with_dataspace_snapshot(lane_id, dataspace_id, |snapshot| {
                snapshot.backlog = update.backlog;
                snapshot.age_slots = update.age_slots;
                snapshot.virtual_finish = update.virtual_finish;
            });
        }
    }

    /// Record invalid PSP fraud metadata fields encountered during admission.
    pub fn record_fraud_invalid_metadata(
        &self,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        dataspace_label: &str,
        tenant: &str,
        field: &'static str,
    ) {
        if self.nexus_lane_metrics_enabled() {
            self.record_lane_with_dataspace(lane_id, dataspace_id);
            let tenant_trimmed = tenant.trim();
            let tenant_label = if tenant_trimmed.is_empty() {
                "unknown"
            } else {
                tenant_trimmed
            };
            let lane_label = lane_id.as_u32().to_string();
            let labels = [tenant_label, field, lane_label.as_str(), dataspace_label];
            self.metrics
                .fraud_psp_invalid_metadata_total
                .with_label_values(&labels)
                .inc();
        }
    }

    /// Record PSP fraud assessment metrics (counts, score/latency histograms).
    #[allow(clippy::too_many_arguments)]
    pub fn record_fraud_assessment(
        &self,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        dataspace_label: &str,
        tenant: &str,
        band: &str,
        score_bps: Option<u16>,
        latency_ms: Option<u64>,
    ) {
        if self.nexus_lane_metrics_enabled() {
            self.record_lane_with_dataspace(lane_id, dataspace_id);
            let tenant_trimmed = tenant.trim();
            let tenant_label = if tenant_trimmed.is_empty() {
                "unknown"
            } else {
                tenant_trimmed
            };
            let lane_label = lane_id.as_u32().to_string();
            let assessment_labels = [tenant_label, band, lane_label.as_str(), dataspace_label];
            self.metrics
                .fraud_psp_assessments_total
                .with_label_values(&assessment_labels)
                .inc();

            if let Some(score) = score_bps {
                self.metrics
                    .fraud_psp_score_bps
                    .with_label_values(&assessment_labels)
                    .observe(u64_to_f64(u64::from(score)));
            }
            if let Some(latency) = latency_ms {
                let latency_labels = [tenant_label, lane_label.as_str(), dataspace_label];
                self.metrics
                    .fraud_psp_latency_ms
                    .with_label_values(&latency_labels)
                    .observe(u64_to_f64(latency));
            }
        }
    }

    /// Record proof verification metrics (latency/size) for the given backend and outcome.
    pub fn record_zk_verify(
        &self,
        backend: &str,
        status: ProofStatus,
        proof_bytes: usize,
        latency_ms: u64,
    ) {
        if self.is_enabled() {
            let status_label = proof_status_label(status);
            let proof_bytes_value =
                u64::try_from(proof_bytes).map_or_else(|_| u64_to_f64(u64::MAX), u64_to_f64);
            self.metrics
                .zk_verify_latency_ms
                .with_label_values(&[backend, status_label])
                .observe(u64_to_f64(latency_ms));
            self.metrics
                .zk_verify_proof_bytes
                .with_label_values(&[backend, status_label])
                .observe(proof_bytes_value);
        }
    }

    #[cfg(feature = "telemetry")]
    /// Record snapshot + eviction telemetry for confidential commitment trees.
    pub fn record_confidential_tree_stats(
        &self,
        asset_id: &AssetDefinitionId,
        stats: ConfidentialTreeStats,
    ) {
        if !self.is_enabled() {
            return;
        }
        let asset_label = asset_id.to_string();
        let labels = [&asset_label[..]];
        self.metrics
            .confidential_tree_commitments
            .with_label_values(&labels)
            .set(stats.commitments);
        self.metrics
            .confidential_tree_depth
            .with_label_values(&labels)
            .set(stats.tree_depth);
        self.metrics
            .confidential_root_history_entries
            .with_label_values(&labels)
            .set(stats.root_history);
        self.metrics
            .confidential_frontier_checkpoints
            .with_label_values(&labels)
            .set(stats.frontier_checkpoints);
        self.metrics
            .confidential_frontier_last_height
            .with_label_values(&labels)
            .set(stats.last_checkpoint_height);
        self.metrics
            .confidential_frontier_last_commitments
            .with_label_values(&labels)
            .set(stats.last_checkpoint_commitments);
        if stats.root_evictions > 0 {
            self.metrics
                .confidential_root_evictions_total
                .with_label_values(&labels)
                .inc_by(stats.root_evictions);
        }
        if stats.frontier_evictions > 0 {
            self.metrics
                .confidential_frontier_evictions_total
                .with_label_values(&labels)
                .inc_by(stats.frontier_evictions);
        }
    }

    /// Record a mismatch between PSP disposition and the fraud engine decision.
    pub fn record_fraud_outcome_mismatch(
        &self,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        dataspace_label: &str,
        tenant: &str,
        direction: &'static str,
    ) {
        if self.nexus_lane_metrics_enabled() {
            self.record_lane_with_dataspace(lane_id, dataspace_id);
            let tenant_trimmed = tenant.trim();
            let tenant_label = if tenant_trimmed.is_empty() {
                "unknown"
            } else {
                tenant_trimmed
            };
            let lane_label = lane_id.as_u32().to_string();
            let labels = [
                tenant_label,
                direction,
                lane_label.as_str(),
                dataspace_label,
            ];
            self.metrics
                .fraud_psp_outcome_mismatch_total
                .with_label_values(&labels)
                .inc();
        }
    }

    /// Record attestation verification outcomes for fraud assessments.
    pub fn record_fraud_attestation(
        &self,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        dataspace_label: &str,
        tenant: &str,
        engine_id: &str,
        status: &'static str,
    ) {
        if !self.nexus_lane_metrics_enabled() {
            return;
        }
        self.record_lane_with_dataspace(lane_id, dataspace_id);
        let tenant_trimmed = tenant.trim();
        let tenant_label = if tenant_trimmed.is_empty() {
            "unknown"
        } else {
            tenant_trimmed
        };
        let engine_trimmed = engine_id.trim();
        let engine_label = if engine_trimmed.is_empty() {
            "unknown"
        } else {
            engine_trimmed
        };
        let lane_label = lane_id.as_u32().to_string();
        let labels = [
            tenant_label,
            engine_label,
            lane_label.as_str(),
            dataspace_label,
            status,
        ];
        self.metrics
            .fraud_psp_attestation_total
            .with_label_values(&labels)
            .inc();
    }

    /// Commit an observation of amounts used in transactions
    pub fn observe_tx_amount(&self, value: f64) {
        if self.is_enabled() {
            self.metrics.tx_amounts.observe(value);
        }
    }

    /// Set DAG vertices/edges for the latest validated block.
    pub fn set_pipeline_dag(&self, lane_id: LaneId, vertices: u64, edges: u64) {
        if self.nexus_lane_metrics_enabled() {
            self.record_lane_placeholders(lane_id);
            self.metrics.pipeline_dag_vertices.set(vertices);
            self.metrics.pipeline_dag_edges.set(edges);
        }
    }

    /// Set DAG conflict rate in basis points for the latest validated block.
    pub fn set_pipeline_conflict_rate_bps(&self, lane_id: LaneId, bps: u64) {
        if self.nexus_lane_metrics_enabled() {
            self.record_lane_placeholders(lane_id);
            self.metrics.pipeline_conflict_rate_bps.set(bps);
        }
    }

    /// Set overlay counters for the latest validated block.
    pub fn set_pipeline_overlays(&self, lane_id: LaneId, overlays: u64, total_instructions: u64) {
        if self.nexus_lane_metrics_enabled() {
            self.record_lane_placeholders(lane_id);
            self.metrics.pipeline_overlay_count.set(overlays);
            self.metrics
                .pipeline_overlay_instructions
                .set(total_instructions);
        }
    }

    /// Increment the access-set source counter by `count`.
    #[cfg(feature = "telemetry")]
    #[allow(dead_code)]
    pub(crate) fn inc_pipeline_access_set_source(&self, source: AccessSetSource, count: u64) {
        if self.nexus_lane_metrics_enabled() && count > 0 {
            let label = access_set_source_label(source);
            self.metrics
                .pipeline_access_set_source_total
                .with_label_values(&[label])
                .inc_by(count);
        }
    }

    /// Increment runtime upgrade event counter labeled by kind
    pub fn inc_runtime_upgrade_event(&self, kind: &'static str) {
        if self.is_enabled() {
            self.metrics
                .runtime_upgrade_events_total
                .with_label_values(&[kind])
                .inc();
        }
    }

    /// Increment runtime upgrade provenance rejection counter labeled by reason.
    pub fn record_runtime_upgrade_provenance_rejection(
        &self,
        reason: iroha_data_model::runtime::RuntimeUpgradeProvenanceError,
    ) {
        if self.is_enabled() {
            let label = reason.as_label();
            self.metrics
                .runtime_upgrade_provenance_rejections_total
                .with_label_values(&[label])
                .inc();
        }
    }

    /// Update governance proposal counters on status transition.
    pub fn record_governance_proposal_transition(
        &self,
        from: Option<crate::state::GovernanceProposalStatus>,
        to: crate::state::GovernanceProposalStatus,
    ) {
        if !self.is_enabled() {
            return;
        }
        if let Some(prev) = from {
            if prev == to {
                return;
            }
            let prev_label = proposal_status_label(prev);
            let gauge = self
                .metrics
                .governance_proposals_status
                .with_label_values(&[prev_label]);
            let current = gauge.get();
            gauge.set(current.saturating_sub(1));
        }
        let to_label = proposal_status_label(to);
        let gauge = self
            .metrics
            .governance_proposals_status
            .with_label_values(&[to_label]);
        let current = gauge.get();
        gauge.set(current.saturating_add(1));
    }

    /// Increment a governance bond lifecycle counter.
    pub fn record_governance_bond_event(&self, event: &'static str) {
        if !self.is_enabled() {
            return;
        }
        self.metrics
            .governance_bond_events_total
            .with_label_values(&[event])
            .inc();
    }

    /// Set the total citizen count gauge.
    pub fn record_citizens_total(&self, total: u64) {
        if !self.is_enabled() {
            return;
        }
        self.metrics.governance_citizens_total.set(total);
    }

    /// Increment citizen service discipline counters.
    pub fn record_citizen_service_event(
        &self,
        event: iroha_data_model::isi::governance::CitizenServiceEvent,
        _slashed: u128,
    ) {
        if !self.is_enabled() {
            return;
        }
        let label = match event {
            iroha_data_model::isi::governance::CitizenServiceEvent::Decline => "decline",
            iroha_data_model::isi::governance::CitizenServiceEvent::NoShow => "no_show",
            iroha_data_model::isi::governance::CitizenServiceEvent::Misconduct => "misconduct",
        };
        self.metrics
            .governance_citizen_service_events_total
            .with_label_values(&[label])
            .inc();
    }

    /// Record council/parliament draw metadata for observability.
    pub fn record_council_draw(
        &self,
        payload: &iroha_data_model::events::data::governance::GovernanceCouncilPersisted,
    ) {
        if !self.is_enabled() {
            return;
        }
        self.metrics
            .governance_council_members
            .set(u64::from(payload.members_count));
        self.metrics
            .governance_council_alternates
            .set(u64::from(payload.alternates_count));
        self.metrics
            .governance_council_candidates
            .set(u64::from(payload.candidates_count));
        self.metrics
            .governance_council_verified
            .set(u64::from(payload.verified));
        self.metrics.governance_council_epoch.set(payload.epoch);
    }

    /// Seed governance proposal gauges with the provided statuses.
    pub fn seed_governance_proposal_statuses(
        &self,
        statuses: impl IntoIterator<Item = crate::state::GovernanceProposalStatus>,
    ) {
        use crate::state::GovernanceProposalStatus as GPS;

        let collected: Vec<GPS> = statuses.into_iter().collect();

        #[cfg(feature = "telemetry")]
        {
            let mut cache = self
                .governance_status_cache
                .lock()
                .expect("governance proposal cache poisoned");
            cache.clear();
            for (idx, status) in collected.iter().copied().enumerate() {
                let mut id = [0u8; 32];
                id[..8].copy_from_slice(&(idx as u64).to_le_bytes());
                cache.insert(id, status);
            }
        }

        if !self.is_enabled() {
            return;
        }

        let mut counts = [0u64; 4];
        for status in collected {
            match status {
                GPS::Proposed => counts[0] = counts[0].saturating_add(1),
                GPS::Approved => counts[1] = counts[1].saturating_add(1),
                GPS::Rejected => counts[2] = counts[2].saturating_add(1),
                GPS::Enacted => counts[3] = counts[3].saturating_add(1),
            }
        }

        for (status, count) in [
            (GPS::Proposed, counts[0]),
            (GPS::Approved, counts[1]),
            (GPS::Rejected, counts[2]),
            (GPS::Enacted, counts[3]),
        ] {
            self.metrics
                .governance_proposals_status
                .with_label_values(&[proposal_status_label(status)])
                .set(count);
        }
    }

    /// Record protected-namespace enforcement outcome (allowed or rejected).
    pub fn record_protected_namespace_enforcement(&self, outcome: &'static str) {
        if self.is_enabled() {
            self.metrics
                .governance_protected_namespace_total
                .with_label_values(&[outcome])
                .inc();
        }
    }

    /// Record manifest quorum enforcement outcome (satisfied or rejected).
    pub fn record_manifest_quorum_enforcement(&self, outcome: &'static str) {
        if self.is_enabled() {
            self.metrics
                .governance_manifest_quorum_total
                .with_label_values(&[outcome])
                .inc();
        }
    }

    /// Record overall manifest admission outcome (allowed or rejected by reason).
    pub fn record_manifest_admission(&self, result: &'static str) {
        if self.is_enabled() {
            self.metrics
                .governance_manifest_admission_total
                .with_label_values(&[result])
                .inc();
        }
    }

    /// Record manifest hook enforcement outcome for a specific hook.
    pub fn record_manifest_hook_enforcement(&self, hook: &'static str, outcome: &'static str) {
        if self.is_enabled() {
            self.metrics
                .governance_manifest_hook_total
                .with_label_values(&[hook, outcome])
                .inc();
        }
    }

    /// Record a manifest activation event emitted by governance enactment.
    pub fn record_manifest_activation(
        &self,
        activation: Option<GovernanceManifestActivation>,
        event: &'static str,
    ) {
        if self.is_enabled() {
            self.metrics
                .governance_manifest_activations_total
                .with_label_values(&[event])
                .inc();
            if let Some(activation) = activation {
                let mut recent = self
                    .metrics
                    .governance_manifest_recent
                    .write()
                    .expect("governance manifest cache lock poisoned");
                recent.push_front(activation);
                if recent.len() > GOVERNANCE_MANIFEST_RECENT_CAP {
                    recent.pop_back();
                }
            }
        }
    }

    /// Set component partitioning stats (component count, max size, histogram buckets by `le`).
    /// Buckets correspond to `le` in [1,2,4,8,16,32,64,128]. Values are component counts per bucket.
    pub fn set_pipeline_components(
        &self,
        lane_id: LaneId,
        count: u64,
        max_size: u64,
        buckets: [u64; 8],
    ) {
        if self.nexus_lane_metrics_enabled() {
            self.record_lane_placeholders(lane_id);
            self.metrics.pipeline_comp_count.set(count);
            self.metrics.pipeline_comp_max.set(max_size);
            // Emit histogram-style buckets with `le` labels
            for (le, val) in PIPELINE_BUCKET_LABELS.iter().zip(buckets.iter()) {
                self.metrics
                    .pipeline_comp_hist_bucket
                    .with_label_values(&[le])
                    .set(*val);
            }
        }
    }

    /// Set peak layer width (max txs in any layer) for the latest validated block.
    pub fn set_pipeline_peak_layer_width(&self, lane_id: LaneId, width: u64) {
        if self.nexus_lane_metrics_enabled() {
            self.record_lane_placeholders(lane_id);
            self.metrics.pipeline_peak_layer_width.set(width);
        }
    }

    /// Set average and median layer widths (rounded to integers).
    pub fn set_pipeline_layer_avg_median(&self, lane_id: LaneId, avg: u64, median: u64) {
        if self.nexus_lane_metrics_enabled() {
            self.record_lane_placeholders(lane_id);
            self.metrics.pipeline_layer_avg_width.set(avg);
            self.metrics.pipeline_layer_median_width.set(median);
        }
    }

    /// Set layer-width histogram buckets. Buckets `le` = [1,2,4,8,16,32,64,128]. Values are layer counts per bucket.
    pub fn set_pipeline_layer_width_hist(&self, lane_id: LaneId, buckets: [u64; 8]) {
        if self.nexus_lane_metrics_enabled() {
            self.record_lane_placeholders(lane_id);
            for (le, val) in PIPELINE_BUCKET_LABELS.iter().zip(buckets.iter()) {
                self.metrics
                    .pipeline_layer_width_hist_bucket
                    .with_label_values(&[le])
                    .set(*val);
            }
        }
    }

    /// Set scheduler layer count for the latest validated block.
    pub fn set_pipeline_layer_count(&self, lane_id: LaneId, count: u64) {
        if self.nexus_lane_metrics_enabled() {
            self.record_lane_placeholders(lane_id);
            self.metrics.pipeline_layer_count.set(count);
        }
    }

    /// Set scheduler utilization (percent 0..100) for the latest validated block.
    pub fn set_pipeline_scheduler_utilization_pct(&self, lane_id: LaneId, pct: u64) {
        if self.nexus_lane_metrics_enabled() {
            self.record_lane_placeholders(lane_id);
            self.metrics
                .pipeline_scheduler_utilization_pct
                .set(pct.min(100));
        }
    }

    /// Set total Norito-encoded overlay bytes for the latest validated block.
    pub fn set_pipeline_overlay_bytes(&self, lane_id: LaneId, total_bytes: u64) {
        if self.nexus_lane_metrics_enabled() {
            self.record_lane_placeholders(lane_id);
            self.metrics.pipeline_overlay_bytes.set(total_bytes);
        }
    }

    /// Set detached pipeline counters for the latest validated block.
    pub fn set_pipeline_detached_prepared(&self, lane_id: LaneId, count: u64) {
        if self.nexus_lane_metrics_enabled() {
            self.record_lane_placeholders(lane_id);
            self.metrics.pipeline_detached_prepared.set(count);
        }
    }

    /// Set detached-merged counter for the latest validated block.
    pub fn set_pipeline_detached_merged(&self, lane_id: LaneId, count: u64) {
        if self.nexus_lane_metrics_enabled() {
            self.record_lane_placeholders(lane_id);
            self.metrics.pipeline_detached_merged.set(count);
        }
    }

    /// Set detached-fallback counter for the latest validated block.
    pub fn set_pipeline_detached_fallback(&self, lane_id: LaneId, count: u64) {
        if self.nexus_lane_metrics_enabled() {
            self.record_lane_placeholders(lane_id);
            self.metrics.pipeline_detached_fallback.set(count);
        }
    }

    /// Observe snapshot query lane iterable metrics for a completed first batch.
    pub fn observe_snapshot_iterable(
        &self,
        mode_label: &'static str,
        elapsed_ms: f64,
        output: &QueryOutput,
    ) {
        if self.is_enabled() {
            self.metrics
                .query_snapshot_lane_first_batch_ms
                .with_label_values(&[mode_label])
                .observe(elapsed_ms);
            let first_batch_items: u64 = output
                .batch
                .tuple
                .iter()
                .map(|entry| entry.len() as u64)
                .sum();
            self.metrics
                .query_snapshot_lane_first_batch_items
                .with_label_values(&[mode_label])
                .observe(u64_to_f64(first_batch_items));
            self.metrics
                .query_snapshot_lane_remaining_items
                .with_label_values(&[mode_label])
                .set(output.remaining_items);
            if output.continue_cursor.is_some() {
                self.metrics
                    .query_snapshot_lane_cursors_total
                    .with_label_values(&[mode_label])
                    .inc();
            }
        }
    }

    /// Observe a pipeline stage timing (milliseconds) labeled by `stage`.
    pub fn observe_pipeline_stage_ms(&self, lane_id: LaneId, stage: &'static str, ms: f64) {
        if self.nexus_lane_metrics_enabled() {
            self.record_lane_placeholders(lane_id);
            let lane_label = lane_id.as_u32().to_string();
            self.metrics
                .pipeline_stage_ms
                .with_label_values(&[lane_label.as_str(), stage])
                .observe(ms);
        }
    }

    /// Observe AMX prepare latency (milliseconds) for the provided lane.
    pub fn observe_amx_prepare_ms(&self, lane_id: LaneId, ms: f64) {
        if self.nexus_lane_metrics_enabled() {
            self.record_lane_placeholders(lane_id);
            let lane_label = lane_id.as_u32().to_string();
            self.metrics
                .amx_prepare_ms
                .with_label_values(&[lane_label.as_str()])
                .observe(ms);
        }
    }

    /// Observe AMX commit latency (milliseconds) for the provided lane.
    pub fn observe_amx_commit_ms(&self, lane_id: LaneId, ms: f64) {
        if self.nexus_lane_metrics_enabled() {
            self.record_lane_placeholders(lane_id);
            let lane_label = lane_id.as_u32().to_string();
            self.metrics
                .amx_commit_ms
                .with_label_values(&[lane_label.as_str()])
                .observe(ms);
        }
    }

    /// Observe IVM execution latency (milliseconds) for the provided lane.
    pub fn observe_ivm_exec_ms(&self, lane_id: LaneId, ms: f64) {
        if self.nexus_lane_metrics_enabled() {
            self.record_lane_placeholders(lane_id);
            let lane_label = lane_id.as_u32().to_string();
            self.metrics
                .ivm_exec_ms
                .with_label_values(&[lane_label.as_str()])
                .observe(ms);
        }
    }

    /// Increment the AMX abort counter for the provided lane/stage.
    pub fn inc_amx_abort(&self, lane_id: LaneId, stage: &'static str) {
        if self.nexus_lane_metrics_enabled() {
            self.record_lane_placeholders(lane_id);
            let lane_label = lane_id.as_u32().to_string();
            self.metrics
                .amx_abort_total
                .with_label_values(&[lane_label.as_str(), stage])
                .inc();
        }
    }

    /// Set BLS signature verification counters for the latest validated block.
    pub fn set_pipeline_sig_bls_counts(
        &self,
        lane_id: LaneId,
        same_msg_agg: u64,
        multi_msg_agg: u64,
        deterministic: u64,
    ) {
        if self.nexus_lane_metrics_enabled() {
            self.record_lane_placeholders(lane_id);
            self.metrics.pipeline_sig_bls_agg_same.set(same_msg_agg);
            self.metrics.pipeline_sig_bls_agg_multi.set(multi_msg_agg);
            self.metrics
                .pipeline_sig_bls_deterministic
                .set(deterministic);
        }
    }

    /// Increment cumulative BLS aggregate verification counters for the provided lane/result.
    pub fn inc_pipeline_sig_bls_result(&self, lane_id: LaneId, same_message: bool, success: bool) {
        if self.nexus_lane_metrics_enabled() {
            self.record_lane_placeholders(lane_id);
            let lane_label = lane_id.as_u32().to_string();
            let result_label = if success { "success" } else { "failure" };
            let counter = if same_message {
                &self.metrics.pipeline_sig_bls_agg_same_total
            } else {
                &self.metrics.pipeline_sig_bls_agg_multi_total
            };
            counter
                .with_label_values(&[lane_label.as_str(), result_label])
                .inc();
        }
    }

    /// Get BLS signature verification counters for the latest validated block.
    /// Returns (`same_message_aggregate`, `multi_message_aggregate`, deterministic).
    pub fn pipeline_sig_bls_counts(&self) -> (u64, u64, u64) {
        (
            self.metrics.pipeline_sig_bls_agg_same.get(),
            self.metrics.pipeline_sig_bls_agg_multi.get(),
            self.metrics.pipeline_sig_bls_deterministic.get(),
        )
    }

    /// Get cumulative BLS aggregate counters broken down by result (success, failure).
    /// Returns ((`same_success`, `same_failure`), (`multi_success`, `multi_failure`)).
    pub fn pipeline_sig_bls_result_totals(&self) -> ((u64, u64), (u64, u64)) {
        if !self.nexus_lane_metrics_enabled() {
            return ((0, 0), (0, 0));
        }
        let lane_label = self.metrics.nexus_lane_id_placeholder.get().to_string();
        let same_success = self
            .metrics
            .pipeline_sig_bls_agg_same_total
            .with_label_values(&[lane_label.as_str(), "success"])
            .get();
        let same_failure = self
            .metrics
            .pipeline_sig_bls_agg_same_total
            .with_label_values(&[lane_label.as_str(), "failure"])
            .get();
        let multi_success = self
            .metrics
            .pipeline_sig_bls_agg_multi_total
            .with_label_values(&[lane_label.as_str(), "success"])
            .get();
        let multi_failure = self
            .metrics
            .pipeline_sig_bls_agg_multi_total
            .with_label_values(&[lane_label.as_str(), "failure"])
            .get();
        ((same_success, same_failure), (multi_success, multi_failure))
    }

    /// Get detached-pipeline counters for the latest validated block.
    /// Returns (prepared, merged, fallback).
    pub fn pipeline_detached_counts(&self) -> (u64, u64, u64) {
        (
            self.metrics.pipeline_detached_prepared.get(),
            self.metrics.pipeline_detached_merged.get(),
            self.metrics.pipeline_detached_fallback.get(),
        )
    }

    /// Set total gas used for the current (latest) block.
    pub fn set_block_gas_used(&self, gas: u64) {
        if self.is_enabled() {
            self.metrics.block_gas_used.set(gas);
        }
    }

    /// Set confidential gas usage for the current transaction/block pair.
    pub fn set_confidential_gas_usage(&self, tx_gas: u64, block_gas: u64) {
        if self.is_enabled() {
            self.metrics.confidential_gas_tx_used.set(tx_gas);
            self.metrics.confidential_gas_block_used.set(block_gas);
            self.metrics.confidential_gas_total.inc_by(tx_gas);
        }
    }

    /// Record oracle settlement context (TWAP price/window, haircut, staleness).
    #[cfg(feature = "telemetry")]
    pub fn observe_oracle_settlement_context(
        &self,
        twap_local_per_xor: &Decimal,
        twap_window_seconds: u32,
        epsilon_bps: u16,
        staleness_ms: u64,
    ) {
        if self.is_enabled() {
            if let Some(price) = twap_local_per_xor.to_f64() {
                self.metrics.oracle_price_local_per_xor.set(price);
            }
            self.metrics
                .oracle_twap_window_seconds
                .set(u64::from(twap_window_seconds));
            self.metrics
                .oracle_haircut_basis_points
                .set(u64::from(epsilon_bps));
            let staleness_seconds = Duration::from_millis(staleness_ms).as_secs_f64();
            self.metrics
                .oracle_staleness_seconds
                .set(staleness_seconds.max(0.0));
        }
    }

    /// Record oracle aggregation metrics (observation count and latency).
    #[cfg(feature = "telemetry")]
    pub fn observe_oracle_aggregation(
        &self,
        feed_id: &iroha_data_model::oracle::FeedId,
        observation_count: u64,
        evidence_hashes: &[Hash],
        duration: Duration,
    ) {
        if self.is_enabled() {
            let label = feed_id.as_str();
            self.metrics
                .oracle_feed_events_total
                .with_label_values(&[label])
                .inc();
            self.metrics
                .oracle_observations_total
                .with_label_values(&[label])
                .inc_by(observation_count);
            self.metrics
                .oracle_aggregation_duration_ms
                .with_label_values(&[label])
                .observe(duration.as_secs_f64() * 1_000.0);
            if !evidence_hashes.is_empty() {
                self.metrics
                    .oracle_feed_events_with_evidence_total
                    .with_label_values(&[label])
                    .inc();
                self.metrics
                    .oracle_evidence_hashes_total
                    .with_label_values(&[label])
                    .inc_by(evidence_hashes.len() as u64);
            }
        }
    }

    /// Record a twitter binding attestation event (piggybacks on oracle feed counters).
    #[cfg(feature = "telemetry")]
    pub fn observe_twitter_binding(
        &self,
        feed_id: &str,
        status: iroha_data_model::oracle::TwitterBindingStatus,
        expired: bool,
    ) {
        if self.is_enabled() {
            let _ = status;
            let _expired_label = if expired { "true" } else { "false" };
            self.metrics
                .oracle_feed_events_total
                .with_label_values(&[feed_id])
                .inc();
        }
    }

    /// Record oracle reward distribution.
    #[cfg(feature = "telemetry")]
    pub fn observe_oracle_reward(
        &self,
        feed_id: &iroha_data_model::oracle::FeedId,
        amount: &Numeric,
    ) {
        if self.is_enabled() {
            let label = feed_id.as_str();
            let amt = amount.try_mantissa_u128().unwrap_or(0);
            self.metrics
                .oracle_rewards_total
                .with_label_values(&[label])
                .inc_by(u64::try_from(amt).unwrap_or(u64::MAX));
        }
    }

    /// Record oracle penalty distribution.
    #[cfg(feature = "telemetry")]
    pub fn observe_oracle_penalty(
        &self,
        feed_id: &iroha_data_model::oracle::FeedId,
        amount: &Numeric,
        _kind: OraclePenaltyKind,
    ) {
        if self.is_enabled() {
            let label = feed_id.as_str();
            let amt = amount.try_mantissa_u128().unwrap_or(0);
            self.metrics
                .oracle_penalties_total
                .with_label_values(&[label])
                .inc_by(u64::try_from(amt).unwrap_or(u64::MAX));
        }
    }

    /// Record oracle reward distribution (no-op when telemetry is disabled).
    #[cfg(not(feature = "telemetry"))]
    #[allow(unused_variables)]
    pub fn observe_oracle_reward(
        &self,
        feed_id: &iroha_data_model::oracle::FeedId,
        amount: &Numeric,
    ) {
    }

    /// Record oracle penalty distribution (no-op when telemetry is disabled).
    #[cfg(not(feature = "telemetry"))]
    #[allow(unused_variables)]
    pub fn observe_oracle_penalty(
        &self,
        feed_id: &iroha_data_model::oracle::FeedId,
        amount: &Numeric,
        _kind: OraclePenaltyKind,
    ) {
    }

    /// Record oracle settlement context (TWAP price/window, haircut, staleness).
    #[cfg(not(feature = "telemetry"))]
    #[allow(unused_variables)]
    pub fn observe_oracle_settlement_context(
        &self,
        twap_local_per_xor: &Decimal,
        twap_window_seconds: u32,
        epsilon_bps: u16,
        staleness_ms: u64,
    ) {
    }

    /// Set number of transactions classified into the quarantine lane for the latest block.
    pub fn set_pipeline_quarantine_classified(&self, lane_id: LaneId, count: u64) {
        if self.is_enabled() {
            self.record_lane_placeholders(lane_id);
            self.metrics.pipeline_quarantine_classified.set(count);
        }
    }

    /// Set number of transactions rejected due to quarantine overflow for the latest block.
    pub fn set_pipeline_quarantine_overflow(&self, lane_id: LaneId, count: u64) {
        if self.is_enabled() {
            self.record_lane_placeholders(lane_id);
            self.metrics.pipeline_quarantine_overflow.set(count);
        }
    }

    /// Set number of transactions executed in quarantine lane for the latest block.
    pub fn set_pipeline_quarantine_executed(&self, lane_id: LaneId, count: u64) {
        if self.is_enabled() {
            self.record_lane_placeholders(lane_id);
            self.metrics.pipeline_quarantine_executed.set(count);
        }
    }

    /// Add to the total fee units for the current (latest) block.
    pub fn add_block_fee_units(&self, delta_units: u64) {
        if self.is_enabled() {
            let cur = self.metrics.block_fee_total_units.get();
            self.metrics
                .block_fee_total_units
                .set(cur.saturating_add(delta_units));
        }
    }

    /// Reset total fee units for the current (latest) block.
    pub fn reset_block_fee_units(&self) {
        if self.is_enabled() {
            self.metrics.block_fee_total_units.set(0);
        }
    }

    /// Record proof-health alert telemetry for a provider.
    #[cfg(feature = "telemetry")]
    /// Record a `SoraFS` proof-health alert snapshot in the cached status state.
    /// Push a `SoraFS` proof-health alert into the `/status` overlay cache.
    pub fn record_sorafs_proof_health_alert(&self, alert: &SorafsProofHealthAlert) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        emit_sorafs_proof_health_alert(&self.metrics, alert);
    }

    #[cfg(not(feature = "telemetry"))]
    #[allow(unused_variables)]
    /// Push a `SoraFS` proof-health alert into the `/status` overlay cache.
    pub fn record_sorafs_proof_health_alert(&self, alert: &SorafsProofHealthAlert) {
        let _ = alert;
    }
}

#[cfg(feature = "telemetry")]
/// Increment the social rejection counter for the provided reason label.
pub fn record_social_rejection(telemetry: &StateTelemetry, reason: &'static str) {
    telemetry
        .metrics
        .social_rejections_total
        .with_label_values(&[reason])
        .inc();
    if reason == "multisig_direct_sign" {
        telemetry.metrics.multisig_direct_sign_reject_total.inc();
    }
}

#[cfg(not(feature = "telemetry"))]
/// No-op social rejection recorder when telemetry is disabled.
pub fn record_social_rejection(_: &StateTelemetry, _: &'static str) {}

impl Default for StateTelemetry {
    fn default() -> Self {
        Self::new(Arc::new(Metrics::default()), true)
    }
}

impl core::ops::Deref for StateTelemetry {
    type Target = Metrics;

    fn deref(&self) -> &Self::Target {
        self.metrics.as_ref()
    }
}

/// Streaming telemetry helper capturing Norito streaming metrics.
#[cfg(feature = "telemetry")]
#[derive(Clone)]
pub struct StreamingTelemetry {
    metrics: Arc<Metrics>,
    enabled: Arc<AtomicBool>,
    parity_buckets: Arc<Mutex<BTreeMap<PeerId, &'static str>>>,
    rekeys_total: Arc<AtomicU64>,
    gck_rotations_total: Arc<AtomicU64>,
}

#[cfg(feature = "telemetry")]
impl StreamingTelemetry {
    /// Construct a streaming telemetry handle backed by the shared metrics registry.
    pub fn new(metrics: Arc<Metrics>, enabled: bool) -> Self {
        Self {
            metrics,
            enabled: Arc::new(AtomicBool::new(enabled)),
            parity_buckets: Arc::new(Mutex::new(BTreeMap::new())),
            rekeys_total: Arc::new(AtomicU64::new(0)),
            gck_rotations_total: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Return whether telemetry collection is enabled.
    #[inline]
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// Enable or disable telemetry collection.
    #[inline]
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    /// Enable telemetry collection.
    #[inline]
    pub fn enable(&self) {
        self.set_enabled(true);
    }

    /// Disable telemetry collection.
    #[inline]
    pub fn disable(&self) {
        self.set_enabled(false);
    }

    /// Record an accepted HPKE rekey for the provided suite.
    pub fn record_hpke_rekey(&self, suite: &EncryptionSuite) {
        if !self.is_enabled() {
            return;
        }
        let label = match suite {
            EncryptionSuite::X25519ChaCha20Poly1305(_) => "x25519",
            EncryptionSuite::Kyber768XChaCha20Poly1305(_) => "kyber768",
        };
        self.metrics
            .streaming_hpke_rekeys_total
            .with_label_values(&[label])
            .inc();
        let total = self.rekeys_total.fetch_add(1, Ordering::Relaxed) + 1;
        let gck_total = self.gck_rotations_total.load(Ordering::Relaxed);
        let event = TelemetryEvent::Security(TelemetrySecurityStats {
            suite: *suite,
            rekeys: u32::try_from(total).unwrap_or(u32::MAX),
            gck_rotations: u32::try_from(gck_total).unwrap_or(u32::MAX),
            last_content_key_id: None,
            last_content_key_valid_from: None,
        });
        self.emit_event("streaming_security", &event);
    }

    /// Record a content key rotation event.
    pub fn record_content_key_update(
        &self,
        suite: Option<&EncryptionSuite>,
        update: &ContentKeyUpdate,
    ) {
        if self.is_enabled() {
            self.metrics.streaming_gck_rotations_total.inc();
            let total = self.gck_rotations_total.fetch_add(1, Ordering::Relaxed) + 1;
            let rekeys = self.rekeys_total.load(Ordering::Relaxed);
            let suite = suite
                .copied()
                .unwrap_or(EncryptionSuite::X25519ChaCha20Poly1305([0; 32]));
            let event = TelemetryEvent::Security(TelemetrySecurityStats {
                suite,
                rekeys: u32::try_from(rekeys).unwrap_or(u32::MAX),
                gck_rotations: u32::try_from(total).unwrap_or(u32::MAX),
                last_content_key_id: Some(update.content_key_id),
                last_content_key_valid_from: Some(update.valid_from_segment),
            });
            self.emit_event("streaming_security", &event);
        }
    }

    /// Record encode telemetry exported by the publisher.
    pub fn record_encode_stats(&self, stats: &TelemetryEncodeStats) {
        if !self.is_enabled() {
            return;
        }
        self.metrics
            .streaming_encode_latency_ms
            .observe(u64_to_f64(u64::from(stats.avg_latency_ms)));
        self.metrics
            .streaming_encode_dropped_layers_total
            .inc_by(u64::from(stats.dropped_layers));
        self.metrics
            .streaming_encode_audio_jitter_ms
            .observe(u64_to_f64(u64::from(stats.avg_audio_jitter_ms)));
        self.metrics
            .streaming_encode_audio_max_jitter_ms
            .set(u64::from(stats.max_audio_jitter_ms));
        self.emit_event("streaming_encode", &TelemetryEvent::Encode(*stats));
    }

    /// Record decoder telemetry exported by viewers.
    pub fn record_decode_stats(&self, stats: &TelemetryDecodeStats) {
        if !self.is_enabled() {
            return;
        }
        self.metrics
            .streaming_decode_buffer_ms
            .observe(u64_to_f64(u64::from(stats.buffer_ms)));
        self.metrics
            .streaming_decode_dropped_frames_total
            .inc_by(u64::from(stats.dropped_frames));
        self.metrics
            .streaming_decode_max_queue_ms
            .observe(u64_to_f64(u64::from(stats.max_decode_queue_ms)));
        self.metrics
            .streaming_decode_av_drift_ms
            .observe(u64_to_f64(u64::from(stats.avg_av_drift_ms.unsigned_abs())));
        self.metrics
            .streaming_decode_max_drift_ms
            .set(u64::from(stats.max_av_drift_ms));
        self.emit_event("streaming_decode", &TelemetryEvent::Decode(*stats));
    }

    /// Record network telemetry emitted by viewers or relays.
    pub fn record_network_stats(&self, stats: &TelemetryNetworkStats) {
        if !self.is_enabled() {
            return;
        }
        self.metrics
            .streaming_network_rtt_ms
            .observe(u64_to_f64(u64::from(stats.rtt_ms)));
        self.metrics
            .streaming_network_loss_percent_x100
            .observe(u64_to_f64(u64::from(stats.loss_percent_x100)));
        self.metrics
            .streaming_network_fec_repairs_total
            .inc_by(u64::from(stats.fec_repairs));
        self.metrics
            .streaming_network_fec_failures_total
            .inc_by(u64::from(stats.fec_failures));
        self.metrics
            .streaming_network_datagram_reinjects_total
            .inc_by(u64::from(stats.datagram_reinjects));
        self.emit_event("streaming_network", &TelemetryEvent::Network(*stats));
    }

    /// Record viewer sync diagnostics emitted in receiver reports.
    pub fn record_sync_diagnostics(&self, diagnostics: &SyncDiagnostics) {
        if !self.is_enabled() {
            return;
        }
        self.metrics
            .streaming_audio_jitter_ms
            .observe(u64_to_f64(u64::from(diagnostics.avg_audio_jitter_ms)));
        self.metrics
            .streaming_audio_max_jitter_ms
            .set(u64::from(diagnostics.max_audio_jitter_ms));
        self.metrics
            .streaming_av_drift_ms
            .observe(u64_to_f64(u64::from(
                diagnostics.avg_av_drift_ms.unsigned_abs(),
            )));
        self.metrics
            .streaming_av_max_drift_ms
            .set(u64::from(diagnostics.max_av_drift_ms));
        self.metrics
            .streaming_av_drift_ewma_ms
            .set(i64::from(diagnostics.ewma_av_drift_ms));
        self.metrics
            .streaming_av_sync_window_ms
            .set(u64::from(diagnostics.window_ms));
        if diagnostics.violation_count > 0 {
            self.metrics
                .streaming_av_sync_violation_total
                .inc_by(u64::from(diagnostics.violation_count));
        }
    }

    /// Record energy telemetry exported by publishers/viewers.
    pub fn record_energy_stats(&self, stats: &TelemetryEnergyStats) {
        if !self.is_enabled() {
            return;
        }
        self.metrics
            .streaming_energy_encoder_mw
            .observe(u64_to_f64(u64::from(stats.encoder_milliwatts)));
        self.metrics
            .streaming_energy_decoder_mw
            .observe(u64_to_f64(u64::from(stats.decoder_milliwatts)));
        self.emit_event("streaming_energy", &TelemetryEvent::Energy(*stats));
    }

    /// Update the active parity bucket for the given peer.
    pub fn record_fec_parity(&self, peer: &PeerId, parity: u8) {
        if !self.is_enabled() {
            return;
        }
        let bucket = match parity {
            0 => "0",
            1 => "1",
            2 => "2",
            3 => "3",
            4 => "4",
            _ => "ge5",
        };
        let mut guard = self
            .parity_buckets
            .lock()
            .expect("streaming parity bucket map poisoned");
        match guard.entry(peer.clone()) {
            BTreeEntry::Occupied(mut entry) => {
                let previous = *entry.get();
                if previous != bucket {
                    self.metrics
                        .streaming_fec_parity_current
                        .with_label_values(&[previous])
                        .dec();
                    self.metrics
                        .streaming_fec_parity_current
                        .with_label_values(&[bucket])
                        .inc();
                    entry.insert(bucket);
                }
            }
            BTreeEntry::Vacant(entry) => {
                self.metrics
                    .streaming_fec_parity_current
                    .with_label_values(&[bucket])
                    .inc();
                entry.insert(bucket);
            }
        }
    }

    /// Increment the sent datagram counter by the provided delta.
    pub fn inc_quic_datagrams_sent(&self, delta: u64) {
        if self.is_enabled() {
            self.metrics
                .streaming_quic_datagrams_sent_total
                .inc_by(delta);
        }
    }

    /// Increment the dropped datagram counter by the provided delta.
    pub fn inc_quic_datagrams_dropped(&self, delta: u64) {
        if self.is_enabled() {
            self.metrics
                .streaming_quic_datagrams_dropped_total
                .inc_by(delta);
        }
    }

    /// Record a feedback timeout event.
    pub fn inc_feedback_timeout(&self) {
        if self.is_enabled() {
            self.metrics.streaming_feedback_timeout_total.inc();
        }
    }

    /// Record a privacy redaction failure event.
    pub fn inc_privacy_redaction_failure(&self) {
        if self.is_enabled() {
            self.metrics.streaming_privacy_redaction_fail_total.inc();
        }
    }

    fn emit_event(&self, msg: &str, event: &TelemetryEvent) {
        if !self.is_enabled() {
            return;
        }
        let payload = telemetry_event_to_json(event);
        match norito::json::to_json(&payload) {
            Ok(json) => iroha_logger::telemetry!(msg = msg, event = json),
            Err(err) => iroha_logger::warn!(
                ?err,
                "failed to encode streaming telemetry event for {}",
                msg
            ),
        }
    }

    fn suite_to_json(suite: &EncryptionSuite) -> norito::json::Value {
        match suite {
            EncryptionSuite::X25519ChaCha20Poly1305(fingerprint) => {
                let mut map = norito::json::Map::new();
                map.insert(
                    "suite".to_string(),
                    norito::json!("x25519_chacha20poly1305"),
                );
                map.insert(
                    "fingerprint".to_string(),
                    norito::json!(hex::encode(fingerprint)),
                );
                norito::json::Value::Object(map)
            }
            EncryptionSuite::Kyber768XChaCha20Poly1305(fingerprint) => {
                let mut map = norito::json::Map::new();
                map.insert(
                    "suite".to_string(),
                    norito::json!("kyber768_xchacha20poly1305"),
                );
                map.insert(
                    "fingerprint".to_string(),
                    norito::json!(hex::encode(fingerprint)),
                );
                norito::json::Value::Object(map)
            }
        }
    }
}

#[cfg(feature = "telemetry")]
fn emit_sorafs_proof_health_alert(metrics: &Metrics, alert: &SorafsProofHealthAlert) {
    let provider_hex = hex::encode(alert.provider_id.as_bytes());
    let trigger = match (alert.triggered_by_pdp, alert.triggered_by_potr) {
        (true, true) => "both",
        (true, false) => "pdp",
        (false, true) => "potr",
        (false, false) => "unknown",
    };
    metrics.record_sorafs_proof_health_alert(
        provider_hex.as_str(),
        trigger,
        alert.penalty_applied_nano > 0 && !alert.cooldown_active,
        alert.pdp_failures,
        alert.potr_breaches,
        alert.penalty_applied_nano,
        alert.cooldown_active,
        alert.window_end_epoch,
    );
}

/// Update gauges that track transaction queue load and saturation as observed by consensus.
pub fn record_state_tx_queue_backpressure(
    telemetry: &StateTelemetry,
    depth: u64,
    capacity: u64,
    saturated: bool,
) {
    if telemetry.is_enabled() {
        telemetry.metrics.sumeragi_tx_queue_depth.set(depth);
        telemetry.metrics.sumeragi_tx_queue_capacity.set(capacity);
        telemetry
            .metrics
            .sumeragi_tx_queue_saturated
            .set(u64::from(saturated));
    }
}

/// Update gauges that reflect the most recent tiered-state snapshot.
pub fn record_state_tiered_snapshot(
    telemetry: &StateTelemetry,
    snapshot_index: u64,
    hot_entries: usize,
    cold_entries: usize,
    cold_bytes: u64,
    hot_promotions: usize,
    hot_demotions: usize,
    hot_grace_overflow_keys: usize,
    hot_grace_overflow_bytes: u64,
    cold_reused_entries: usize,
    cold_reused_bytes: u64,
) {
    if telemetry.is_enabled() {
        telemetry
            .metrics
            .state_tiered_last_snapshot_index
            .set(snapshot_index);
        telemetry
            .metrics
            .state_tiered_hot_entries
            .set(hot_entries as u64);
        telemetry
            .metrics
            .state_tiered_cold_entries
            .set(cold_entries as u64);
        telemetry.metrics.state_tiered_cold_bytes.set(cold_bytes);
        telemetry
            .metrics
            .state_tiered_hot_promotions
            .set(hot_promotions as u64);
        telemetry
            .metrics
            .state_tiered_hot_demotions
            .set(hot_demotions as u64);
        telemetry
            .metrics
            .state_tiered_hot_grace_overflow_keys
            .set(hot_grace_overflow_keys as u64);
        telemetry
            .metrics
            .state_tiered_hot_grace_overflow_bytes
            .set(hot_grace_overflow_bytes);
        telemetry
            .metrics
            .state_tiered_cold_reused_entries
            .set(cold_reused_entries as u64);
        telemetry
            .metrics
            .state_tiered_cold_reused_bytes
            .set(cold_reused_bytes);
    }
}

const CHANNEL_CAPACITY: usize = 1024;
#[cfg(feature = "telemetry")]
/// Upper bound for waiting on the telemetry actor to refresh metrics.
///
/// Keep this short so HTTP status/metrics endpoints cannot stall the runtime
/// when the telemetry task is unavailable or backlogged.
const METRICS_SYNC_TIMEOUT: Duration = Duration::from_millis(500);

#[cfg_attr(not(feature = "telemetry"), allow(dead_code))]
enum Message {
    Sync { reply: oneshot::Sender<()> },
}

#[cfg(feature = "telemetry")]
type MicropaymentSampleStore = Arc<StdRwLock<BTreeMap<String, MicropaymentSampleRecord>>>;
#[cfg(not(feature = "telemetry"))]
type MicropaymentSampleStore = ();

/// Handle to the telemetry state
pub struct Telemetry {
    actor: mpsc::Sender<Message>,
    last_reported_block: Arc<RwLock<Option<BlockCommitReport>>>,
    metrics: Arc<Metrics>,
    enabled: Arc<AtomicBool>,
    nexus_enabled: Arc<AtomicBool>,
    time_source: TimeSource,
    soranet_privacy: Arc<SoranetSecureAggregator>,
    #[cfg_attr(not(feature = "telemetry"), allow(dead_code))]
    micropayment_samples: MicropaymentSampleStore,
    da_slot_rescheduled: Arc<AtomicBool>,
    da_slots_total: Arc<AtomicU64>,
    da_slots_quorum_met: Arc<AtomicU64>,
}

impl Clone for Telemetry {
    fn clone(&self) -> Self {
        #[cfg(feature = "telemetry")]
        let micropayment_samples = Arc::clone(&self.micropayment_samples);
        #[cfg(not(feature = "telemetry"))]
        let micropayment_samples = ();

        Self {
            actor: self.actor.clone(),
            last_reported_block: Arc::clone(&self.last_reported_block),
            metrics: Arc::clone(&self.metrics),
            enabled: Arc::clone(&self.enabled),
            nexus_enabled: Arc::clone(&self.nexus_enabled),
            time_source: self.time_source.clone(),
            soranet_privacy: Arc::clone(&self.soranet_privacy),
            micropayment_samples,
            da_slot_rescheduled: Arc::clone(&self.da_slot_rescheduled),
            da_slots_total: Arc::clone(&self.da_slots_total),
            da_slots_quorum_met: Arc::clone(&self.da_slots_quorum_met),
        }
    }
}

/// Snapshot of the most recent micropayment telemetry observation per provider.
#[cfg_attr(not(feature = "telemetry"), allow(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MicropaymentSampleRecord {
    /// Aggregated credit statistics for the sampling window.
    pub credits: MicropaymentCreditSnapshot,
    /// Lottery ticket counters captured for the sampling window.
    pub tickets: MicropaymentTicketCounters,
}

/// Outcome emitted when planning a missing-block fetch after QC-first arrival.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissingBlockFetchOutcome {
    /// A fetch request was issued.
    Requested,
    /// No peers were available to target for a fetch.
    NoTargets,
    /// A retry backoff window suppressed a fetch attempt.
    Backoff,
}

impl MissingBlockFetchOutcome {
    fn label(self) -> &'static str {
        match self {
            MissingBlockFetchOutcome::Requested => "requested",
            MissingBlockFetchOutcome::NoTargets => "no_targets",
            MissingBlockFetchOutcome::Backoff => "backoff",
        }
    }
}

/// Target set used when requesting a missing block payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissingBlockFetchTargetKind {
    /// Request targets derived from the QC signer set.
    Signers,
    /// Request targets derived from the full commit topology.
    Topology,
}

impl MissingBlockFetchTargetKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            MissingBlockFetchTargetKind::Signers => "signers",
            MissingBlockFetchTargetKind::Topology => "topology",
        }
    }
}

/// Outcome classification for the DA manifest guard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManifestGuardResult {
    /// Guard accepted the block's DA manifests.
    Allowed,
    /// Guard rejected the block due to missing or invalid manifests.
    Rejected,
}

impl ManifestGuardResult {
    #[must_use]
    fn label(self) -> &'static str {
        match self {
            ManifestGuardResult::Allowed => "allowed",
            ManifestGuardResult::Rejected => "rejected",
        }
    }
}

/// Reason why the DA manifest guard produced its outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManifestGuardReason {
    /// Manifests were present and matched the commitment hash.
    Ok,
    /// Manifest could not be found in the spool directory.
    Missing,
    /// Manifest hash diverged from the commitment.
    HashMismatch,
    /// Manifest could not be read from disk.
    ReadError,
    /// Spool directory scan failed.
    SpoolScan,
}

impl ManifestGuardReason {
    #[must_use]
    fn label(self) -> &'static str {
        match self {
            ManifestGuardReason::Ok => "ok",
            ManifestGuardReason::Missing => "missing",
            ManifestGuardReason::HashMismatch => "hash_mismatch",
            ManifestGuardReason::ReadError => "read_error",
            ManifestGuardReason::SpoolScan => "spool_scan",
        }
    }
}

/// Cache outcome classification used for DA spool/manifest caching telemetry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CacheResult {
    /// Cache entry served without disk re-read.
    Hit,
    /// Cache entry was refreshed from disk.
    Miss,
}

impl CacheResult {
    #[must_use]
    fn label(self) -> &'static str {
        match self {
            CacheResult::Hit => "hit",
            CacheResult::Miss => "miss",
        }
    }
}

/// DA spool cache classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DaSpoolCacheKind {
    /// Commitment bundle spool.
    Commitments,
    /// Pin intent spool.
    PinIntents,
    /// Receipt spool.
    Receipts,
}

impl DaSpoolCacheKind {
    #[must_use]
    fn label(self) -> &'static str {
        match self {
            DaSpoolCacheKind::Commitments => "commitments",
            DaSpoolCacheKind::PinIntents => "pin_intents",
            DaSpoolCacheKind::Receipts => "receipts",
        }
    }
}

/// Outcome classification for DA pin intent spool handling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PinIntentSpoolResult {
    /// Pin intent was retained for sealing.
    Kept,
    /// Pin intent was dropped or skipped.
    Dropped,
}

impl PinIntentSpoolResult {
    #[must_use]
    fn label(self) -> &'static str {
        match self {
            PinIntentSpoolResult::Kept => "kept",
            PinIntentSpoolResult::Dropped => "dropped",
        }
    }
}

/// Reason why a pin intent was kept or dropped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PinIntentSpoolReason {
    /// Pin intent survived validation and dedupe.
    Kept,
    /// Manifest hash was zero or missing.
    ZeroManifest,
    /// Intent duplicated lane/epoch/sequence/ticket.
    DuplicateIntent,
    /// Alias collision superseded this intent.
    AliasSuperseded,
    /// Lane was not present in the lane catalog.
    UnknownLane,
    /// Owner account missing from WSV.
    UnknownOwner,
    /// Storage ticket was zeroed.
    ZeroStorageTicket,
    /// Intent was already sealed into a previous block.
    SealedDuplicate,
}

impl PinIntentSpoolReason {
    #[must_use]
    fn label(self) -> &'static str {
        match self {
            PinIntentSpoolReason::Kept => "kept",
            PinIntentSpoolReason::ZeroManifest => "zero_manifest",
            PinIntentSpoolReason::DuplicateIntent => "duplicate",
            PinIntentSpoolReason::AliasSuperseded => "alias_superseded",
            PinIntentSpoolReason::UnknownLane => "unknown_lane",
            PinIntentSpoolReason::UnknownOwner => "unknown_owner",
            PinIntentSpoolReason::ZeroStorageTicket => "zero_storage_ticket",
            PinIntentSpoolReason::SealedDuplicate => "sealed_duplicate",
        }
    }
}

impl From<&DaPinIntentValidationError> for PinIntentSpoolReason {
    fn from(error: &DaPinIntentValidationError) -> Self {
        match error {
            DaPinIntentValidationError::UnknownLane { .. } => PinIntentSpoolReason::UnknownLane,
            DaPinIntentValidationError::UnknownOwner { .. } => PinIntentSpoolReason::UnknownOwner,
            DaPinIntentValidationError::DuplicateIntent { .. } => {
                PinIntentSpoolReason::DuplicateIntent
            }
            DaPinIntentValidationError::ZeroManifestHash { .. } => {
                PinIntentSpoolReason::ZeroManifest
            }
            DaPinIntentValidationError::ZeroStorageTicket { .. } => {
                PinIntentSpoolReason::ZeroStorageTicket
            }
            DaPinIntentValidationError::AliasSuperseded { .. } => {
                PinIntentSpoolReason::AliasSuperseded
            }
        }
    }
}

impl Telemetry {
    #[inline]
    fn default_micropayment_samples() -> MicropaymentSampleStore {
        #[cfg(feature = "telemetry")]
        {
            Arc::new(StdRwLock::new(BTreeMap::new()))
        }
        #[cfg(not(feature = "telemetry"))]
        {}
    }

    /// Lightweight constructor for tests: does not spawn the background actor.
    pub fn new(metrics: Arc<Metrics>, enabled: bool) -> Self {
        let (actor, _handle) = mpsc::channel(CHANNEL_CAPACITY);
        let soranet_privacy = Arc::new(
            SoranetSecureAggregator::new(PrivacyBucketConfig::default())
                .expect("valid default SoraNet privacy config"),
        );
        let telemetry = Telemetry {
            actor,
            last_reported_block: Arc::new(RwLock::new(None)),
            metrics,
            enabled: Arc::new(AtomicBool::new(enabled)),
            nexus_enabled: Arc::new(AtomicBool::new(true)),
            time_source: TimeSource::new_system(),
            soranet_privacy,
            micropayment_samples: Self::default_micropayment_samples(),
            da_slot_rescheduled: Arc::new(AtomicBool::new(false)),
            da_slots_total: Arc::new(AtomicU64::new(0)),
            da_slots_quorum_met: Arc::new(AtomicU64::new(0)),
        };
        #[cfg(feature = "telemetry")]
        {
            if telemetry.is_enabled() {
                let telemetry_clone = telemetry.clone();
                global_sorafs_node_otel().set_micropayment_sink(Some(Arc::new(
                    move |provider, credits, tickets| {
                        telemetry_clone
                            .record_sorafs_micropayment_sample(provider, credits, tickets);
                    },
                )));
            } else {
                global_sorafs_node_otel().set_micropayment_sink(None);
            }
        }
        telemetry
    }

    /// Whether telemetry observations are enabled.
    #[inline]
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }
    /// Whether Nexus lane/dataspace telemetry is allowed.
    #[inline]
    pub fn nexus_enabled(&self) -> bool {
        self.nexus_enabled.load(Ordering::Relaxed)
    }
    /// Enable or disable Nexus lane/dataspace telemetry.
    #[inline]
    pub fn set_nexus_enabled(&self, enabled: bool) {
        self.nexus_enabled.store(enabled, Ordering::Relaxed);
        if !enabled {
            reset_nexus_metrics(&self.metrics);
        }
    }
    /// Whether Nexus lane/dataspace metrics should be emitted.
    #[inline]
    fn nexus_lane_metrics_enabled(&self) -> bool {
        self.is_enabled() && self.nexus_enabled()
    }

    /// Enable or disable telemetry observations at runtime.
    #[inline]
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    /// Enable telemetry observations at runtime.
    #[inline]
    pub fn enable(&self) {
        self.set_enabled(true);
    }

    /// Disable telemetry observations at runtime.
    #[inline]
    pub fn disable(&self) {
        self.set_enabled(false);
    }
    /// Record `NEW_VIEW` publish (counter placeholder; wired later).
    pub fn inc_new_view_publish(&self) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.sumeragi_new_view_publish_total.inc();
        }
    }

    /// Record `NEW_VIEW` received (counter placeholder; wired later).
    pub fn inc_new_view_recv(&self) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.sumeragi_new_view_recv_total.inc();
        }
    }

    /// Record `NEW_VIEW` dropped because the peer holds a conflicting lock.
    pub fn inc_new_view_dropped_by_lock(&self) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.sumeragi_new_view_dropped_by_lock_total.inc();
        }
    }

    /// Record the outcome of planning a missing-block fetch on QC-first arrival.
    #[allow(clippy::cast_precision_loss)]
    pub fn note_missing_block_fetch(
        &self,
        outcome: MissingBlockFetchOutcome,
        targets: usize,
        dwell: Duration,
        target_kind: Option<MissingBlockFetchTargetKind>,
    ) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        self.metrics
            .sumeragi_missing_block_fetch_total
            .with_label_values(&[outcome.label()])
            .inc();
        if let Some(kind) = target_kind {
            self.metrics
                .sumeragi_missing_block_fetch_target_total
                .with_label_values(&[kind.label()])
                .inc();
        }
        self.metrics
            .sumeragi_missing_block_fetch_targets
            .observe(targets as f64);
        self.metrics
            .sumeragi_missing_block_fetch_dwell_ms
            .observe(dwell.as_millis() as f64);
    }

    #[inline]
    fn da_gate_reason_label(reason: GateReason) -> (&'static str, u64) {
        match reason {
            GateReason::MissingAvailabilityQc => ("missing_availability_qc", 1),
            GateReason::ManifestGuard { kind, .. } => {
                let code = match kind {
                    crate::sumeragi::da::ManifestGateKind::Missing => 3,
                    crate::sumeragi::da::ManifestGateKind::HashMismatch => 4,
                    crate::sumeragi::da::ManifestGateKind::ReadFailed => 5,
                    crate::sumeragi::da::ManifestGateKind::SpoolScan => 6,
                };
                (kind.as_str(), code)
            }
        }
    }

    #[inline]
    fn da_gate_satisfaction_label(satisfaction: GateSatisfaction) -> (&'static str, u64) {
        match satisfaction {
            GateSatisfaction::AvailabilityQc => ("availability_qc", 1),
        }
    }

    /// Update the last-seen DA availability reason gauge. Passing `None` clears the gauge to zero.
    pub fn set_da_gate_last_reason(&self, reason: Option<GateReason>) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        let code = reason.map_or(0, |reason| Self::da_gate_reason_label(reason).1);
        self.metrics.sumeragi_da_gate_last_reason.set(code);
    }

    /// Record that DA availability tracking observed a missing-evidence reason.
    pub fn note_da_gate_block(&self, reason: GateReason) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        let (label, code) = Self::da_gate_reason_label(reason);
        self.metrics
            .sumeragi_da_gate_block_total
            .with_label_values(&[label])
            .inc();
        self.metrics.sumeragi_da_gate_last_reason.set(code);
    }

    /// Record a transition that satisfied a DA availability condition.
    pub fn note_da_gate_satisfaction(&self, satisfaction: GateSatisfaction) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        let (label, code) = Self::da_gate_satisfaction_label(satisfaction);
        self.metrics
            .sumeragi_da_gate_satisfied_total
            .with_label_values(&[label])
            .inc();
        self.metrics.sumeragi_da_gate_last_satisfied.set(code);
    }

    /// Record the outcome of the DA manifest guard for a block payload.
    pub fn note_da_manifest_guard(&self, result: ManifestGuardResult, reason: ManifestGuardReason) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        self.metrics
            .sumeragi_da_manifest_guard_total
            .with_label_values(&[result.label(), reason.label()])
            .inc();
    }

    /// Record the outcome of the DA manifest cache lookup.
    pub fn note_da_manifest_cache(&self, result: CacheResult) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        self.metrics
            .sumeragi_da_manifest_cache_total
            .with_label_values(&[result.label()])
            .inc();
    }

    /// Record the outcome of the DA spool cache lookup.
    pub fn note_da_spool_cache(&self, kind: DaSpoolCacheKind, result: CacheResult) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        self.metrics
            .sumeragi_da_spool_cache_total
            .with_label_values(&[kind.label(), result.label()])
            .inc();
    }

    /// Record how a DA pin intent from the spool was handled.
    pub fn note_da_pin_intent_spool(
        &self,
        result: PinIntentSpoolResult,
        reason: PinIntentSpoolReason,
    ) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        self.metrics
            .sumeragi_da_pin_intent_spool_total
            .with_label_values(&[result.label(), reason.label()])
            .inc();
    }

    /// Record a QC validation error grouped by reason.
    pub fn note_qc_validation_error(&self, reason: &'static str) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        self.metrics
            .sumeragi_qc_validation_errors_total
            .with_label_values(&[reason])
            .inc();
    }

    /// Record a validation-gate reject grouped by reason before voting.
    pub fn note_validation_reject(&self, reason: &'static str, height: u64, view: u64) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        self.metrics
            .sumeragi_validation_reject_total
            .with_label_values(&[reason])
            .inc();
        let reason_code = Self::validation_reject_reason_code(reason);
        self.metrics
            .sumeragi_validation_reject_last_reason
            .set(reason_code);
        self.metrics
            .sumeragi_validation_reject_last_height
            .set(height);
        self.metrics.sumeragi_validation_reject_last_view.set(view);
        let timestamp_ms = u64::try_from(self.time_source.now().as_millis()).unwrap_or(u64::MAX);
        self.metrics
            .sumeragi_validation_reject_last_timestamp_ms
            .set(timestamp_ms);
    }

    #[inline]
    fn validation_reject_reason_code(reason: &str) -> u64 {
        match reason {
            "stateless" => 1,
            "execution" => 2,
            "prev_hash" => 3,
            "prev_height" => 4,
            "topology" => 5,
            _ => 0,
        }
    }

    /// Record which roster source was used for a block-sync update.
    pub fn note_block_sync_roster_source(&self, source: &str) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        self.metrics
            .sumeragi_block_sync_roster_source_total
            .with_label_values(&[source])
            .inc();
    }

    /// Record why a block-sync update was dropped due to roster validation.
    pub fn note_block_sync_roster_drop(&self, reason: &str) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        self.metrics
            .sumeragi_block_sync_roster_drop_total
            .with_label_values(&[reason])
            .inc();
    }

    /// Record a view-change trigger grouped by cause.
    pub fn note_view_change_cause(&self, cause: &'static str) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        self.metrics
            .sumeragi_view_change_cause_total
            .with_label_values(&[cause])
            .inc();
        let now_ms = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
            .unwrap_or(0);
        self.metrics
            .sumeragi_view_change_cause_last_timestamp_ms
            .with_label_values(&[cause])
            .set(now_ms);
    }

    /// Record the number of QC signers present in the bitmap versus counted for a phase.
    #[allow(clippy::cast_precision_loss)]
    pub fn note_qc_signer_counts(&self, phase: &'static str, present: usize, counted: usize) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        self.metrics
            .sumeragi_qc_signer_counts
            .with_label_values(&[phase, "present"])
            .observe(present as f64);
        self.metrics
            .sumeragi_qc_signer_counts
            .with_label_values(&[phase, "counted"])
            .observe(counted as f64);
    }

    /// Record an invalid-signature drop grouped by message kind and whether it was logged or throttled.
    pub fn inc_invalid_signature(&self, kind: &'static str, outcome: &'static str) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        self.metrics
            .sumeragi_invalid_signature_total
            .with_label_values(&[kind, outcome])
            .inc();
    }

    /// Record an AXT policy rejection grouped by lane id and reason.
    pub fn note_axt_policy_reject(
        &self,
        lane: LaneId,
        reason: AxtRejectReason,
        snapshot_version: u64,
    ) {
        let _ = snapshot_version;
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        let lane_label = lane.as_u32().to_string();
        let reason = reason.label();
        self.metrics
            .axt_policy_reject_total
            .with_label_values(&[lane_label.as_str(), reason])
            .inc();
    }

    /// Record whether AXT policy hydration pulled from cache or required a rebuild.
    ///
    /// Canonical `event` labels: `cache_hit` and `cache_miss`.
    pub fn note_axt_policy_snapshot_cache_event(&self, event: &'static str) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .axt_policy_snapshot_cache_events_total
                .with_label_values(&[event])
                .inc();
        }
    }

    /// Record an AXT proof cache event grouped by label.
    ///
    /// Canonical `event` labels: `hit`, `miss`, `expired`, `cleared`, `pruned`.
    pub fn note_axt_proof_cache_event(&self, event: &'static str) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .axt_proof_cache_events_total
                .with_label_values(&[event])
                .inc();
        }
    }

    /// Publish the current AXT proof cache state for a dataspace.
    ///
    /// Labels: `dsid`, `status`, `manifest_root_hex`, `verified_slot`; value: `expiry_slot` (with skew).
    pub fn set_axt_proof_cache_state(
        &self,
        dsid: DataSpaceId,
        status: &'static str,
        manifest_root: [u8; 32],
        verified_slot: u64,
        expiry_slot: Option<u64>,
    ) {
        if self.enabled.load(Ordering::Relaxed) {
            let ds_label = dsid.as_u64().to_string();
            let slot_label = verified_slot.to_string();
            let root_hex = hex::encode(manifest_root);
            let expiry_slot = expiry_slot.map_or(0, |slot| i64::try_from(slot).unwrap_or(i64::MAX));
            self.metrics
                .axt_proof_cache_state
                .with_label_values(&[
                    ds_label.as_str(),
                    status,
                    root_hex.as_str(),
                    slot_label.as_str(),
                ])
                .set(expiry_slot);
        }
    }

    /// Remove cached AXT proof state for a dataspace.
    pub fn clear_axt_proof_cache_state(
        &self,
        dsid: DataSpaceId,
        status: &'static str,
        manifest_root: [u8; 32],
        verified_slot: u64,
    ) {
        if self.enabled.load(Ordering::Relaxed) {
            let ds_label = dsid.as_u64().to_string();
            let slot_label = verified_slot.to_string();
            let root_hex = hex::encode(manifest_root);
            let _ = self.metrics.axt_proof_cache_state.remove_label_values(&[
                ds_label.as_str(),
                status,
                root_hex.as_str(),
                slot_label.as_str(),
            ]);
        }
    }

    /// Set the version hash for the AXT policy snapshot used when hydrating the host.
    pub fn set_axt_policy_snapshot_version(&self, snapshot: &AxtPolicySnapshot) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        let version = if snapshot.entries.is_empty() {
            0
        } else if snapshot.version != 0 {
            snapshot.version
        } else {
            AxtPolicySnapshot::compute_version(&snapshot.entries)
        };
        self.metrics.axt_policy_snapshot_version.set(version);
    }

    /// Record a consensus message sent over the network (votes and QCs).
    pub fn note_consensus_message_sent(&self, msg: &BlockMessage) {
        if self.enabled.load(Ordering::Relaxed) {
            self.record_consensus_message(msg, true);
        }
    }

    /// Record a consensus message received from the network (votes and QCs).
    pub fn note_consensus_message_received(&self, msg: &BlockMessage) {
        if self.enabled.load(Ordering::Relaxed) {
            self.record_consensus_message(msg, false);
        }
    }

    fn record_consensus_message(&self, msg: &BlockMessage, sent: bool) {
        match msg {
            BlockMessage::PrevoteVote(_) => {
                if sent {
                    self.metrics
                        .sumeragi_votes_sent_total
                        .with_label_values(&[PHASE_PREVOTE])
                        .inc();
                } else {
                    self.metrics
                        .sumeragi_votes_received_total
                        .with_label_values(&[PHASE_PREVOTE])
                        .inc();
                }
            }
            BlockMessage::PrecommitVote(_) => {
                if sent {
                    self.metrics
                        .sumeragi_votes_sent_total
                        .with_label_values(&[PHASE_PRECOMMIT])
                        .inc();
                } else {
                    self.metrics
                        .sumeragi_votes_received_total
                        .with_label_values(&[PHASE_PRECOMMIT])
                        .inc();
                }
            }
            BlockMessage::AvailabilityVote(_) => {
                if sent {
                    self.metrics
                        .sumeragi_votes_sent_total
                        .with_label_values(&[PHASE_AVAILABLE])
                        .inc();
                } else {
                    self.metrics
                        .sumeragi_votes_received_total
                        .with_label_values(&[PHASE_AVAILABLE])
                        .inc();
                }
            }
            BlockMessage::PrevoteQC(_) => {
                if sent {
                    self.metrics
                        .sumeragi_qc_sent_total
                        .with_label_values(&[PHASE_PREVOTE])
                        .inc();
                } else {
                    self.metrics
                        .sumeragi_qc_received_total
                        .with_label_values(&[PHASE_PREVOTE])
                        .inc();
                }
            }
            BlockMessage::PrecommitQC(_) => {
                if sent {
                    self.metrics
                        .sumeragi_qc_sent_total
                        .with_label_values(&[PHASE_PRECOMMIT])
                        .inc();
                } else {
                    self.metrics
                        .sumeragi_qc_received_total
                        .with_label_values(&[PHASE_PRECOMMIT])
                        .inc();
                }
            }
            BlockMessage::AvailabilityQC(_) => {
                if sent {
                    self.metrics
                        .sumeragi_qc_sent_total
                        .with_label_values(&[PHASE_AVAILABLE])
                        .inc();
                } else {
                    self.metrics
                        .sumeragi_qc_received_total
                        .with_label_values(&[PHASE_AVAILABLE])
                        .inc();
                }
            }
            _ => {}
        }
    }

    /// Update gauges tracking missing-block retry posture.
    pub fn set_missing_block_retry_window_ms(&self, retry_window_ms: u64) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .sumeragi_missing_block_retry_window_ms
                .set(retry_window_ms);
        }
    }

    /// Update gauges tracking inflight missing-block requests.
    pub fn set_missing_block_inflight(&self, active: usize, oldest_ms: u64) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .sumeragi_missing_block_requests
                .set(u64::try_from(active).unwrap_or(u64::MAX));
            self.metrics.sumeragi_missing_block_oldest_ms.set(oldest_ms);
        }
    }

    /// Record dwell time from first QC arrival until payload observation.
    pub fn observe_missing_block_dwell(&self, dwell: Duration) {
        if self.enabled.load(Ordering::Relaxed) {
            let ms = dwell.as_secs_f64() * 1_000.0;
            self.metrics.sumeragi_missing_block_dwell_ms.observe(ms);
        }
    }

    /// Increment when an `ExecutionQC` is assembled (placeholder counter).
    pub fn inc_exec_qc_assembled(&self) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.sumeragi_exec_qc_assembled_total.inc();
        }
    }

    /// Increment when a Witness-availability QC is assembled (placeholder counter).
    pub fn inc_wa_qc_assembled(&self) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.sumeragi_wa_qc_assembled_total.inc();
        }
    }

    /// Increment VRF commit/reveal reject counter labeled by reason.
    pub fn inc_vrf_reject_by_reason(&self, reason: &'static str) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .sumeragi_vrf_rejects_total_by_reason
                .with_label_values(&[reason])
                .inc();
        }
    }

    /// Record a consensus membership mismatch against a peer for the given height/view.
    pub fn note_membership_mismatch(
        &self,
        peer: &iroha_data_model::peer::PeerId,
        height: u64,
        view: u64,
    ) {
        if self.enabled.load(Ordering::Relaxed) {
            let peer_label = peer.to_string();
            let height_label = height.to_string();
            let view_label = view.to_string();
            self.metrics
                .sumeragi_membership_mismatch_total
                .with_label_values(&[
                    peer_label.as_str(),
                    height_label.as_str(),
                    view_label.as_str(),
                ])
                .inc();
            self.metrics
                .sumeragi_membership_mismatch_active
                .with_label_values(&[peer_label.as_str()])
                .set(1);
        }
    }

    /// Clear the active membership mismatch gauge for a peer when alignment is confirmed.
    pub fn clear_membership_mismatch(&self, peer: &iroha_data_model::peer::PeerId) {
        if self.enabled.load(Ordering::Relaxed) {
            let peer_label = peer.to_string();
            self.metrics
                .sumeragi_membership_mismatch_active
                .with_label_values(&[peer_label.as_str()])
                .set(0);
        }
    }

    /// Update membership view-hash gauges (height/view/epoch context + truncated hash).
    pub fn set_membership_view_hash(&self, height: u64, view: u64, epoch: u64, hash: [u8; 32]) {
        if self.enabled.load(Ordering::Relaxed) {
            let mut truncated = [0u8; 8];
            truncated.copy_from_slice(&hash[..8]);
            let value = u64::from_be_bytes(truncated);
            self.metrics.sumeragi_membership_view_hash.set(value);
            self.metrics.sumeragi_membership_height.set(height);
            self.metrics.sumeragi_membership_view.set(view);
            self.metrics.sumeragi_membership_epoch.set(epoch);
        }
    }

    /// Set highest QC height. Placeholder uses existing gauge for visibility.
    pub fn set_highest_qc_height(&self, h: u64) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.sumeragi_highest_qc_height.set(h);
        }
    }

    /// Set current leader index. Placeholder; currently unused.
    pub fn set_leader_index(&self, idx: u64) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.sumeragi_leader_index.set(idx);
        }
    }

    /// Set locked QC height.
    pub fn set_locked_qc_height(&self, h: u64) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.sumeragi_locked_qc_height.set(h);
        }
    }

    /// Set locked QC view.
    pub fn set_locked_qc_view(&self, v: u64) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.sumeragi_locked_qc_view.set(v);
        }
    }

    /// Update gauges that track transaction queue load and saturation as observed by consensus.
    pub fn record_tx_queue_backpressure(&self, depth: u64, capacity: u64, saturated: bool) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.sumeragi_tx_queue_depth.set(depth);
            self.metrics.sumeragi_tx_queue_capacity.set(capacity);
            self.metrics
                .sumeragi_tx_queue_saturated
                .set(u64::from(saturated));
        }
    }

    /// Increment post-to-peer counter labeled by peer id (collector routing/backpressure insight)
    pub fn inc_post_to_peer(&self, peer: &iroha_data_model::peer::PeerId) {
        if self.enabled.load(Ordering::Relaxed) {
            let label = peer.to_string();
            self.metrics
                .sumeragi_post_to_peer_total
                .with_label_values(&[label.as_str()])
                .inc();
        }
    }

    /// Increment Torii pre-auth rejection counter for the provided reason label.
    pub fn inc_torii_pre_auth_reject(&self, reason: &'static str) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .torii_pre_auth_reject_total
                .with_label_values(&[reason])
                .inc();
        }
    }

    /// Increment Torii operator auth event counters.
    pub fn inc_torii_operator_auth(
        &self,
        action: &'static str,
        result: &'static str,
        reason: &'static str,
    ) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.inc_torii_operator_auth(action, result, reason);
        }
    }

    /// Increment Torii operator auth lockout counters.
    pub fn inc_torii_operator_auth_lockout(&self, action: &'static str, reason: &'static str) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.inc_torii_operator_auth_lockout(action, reason);
        }
    }

    /// Record a signature-limit rejection with the observed count and configured cap.
    pub fn inc_torii_signature_limit_reject(
        &self,
        count: u64,
        limit: u64,
        authority: &'static str,
    ) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.torii_signature_limit_total.inc();
            self.metrics
                .torii_signature_limit_by_authority_total
                .with_label_values(&[authority])
                .inc();
            self.metrics.torii_signature_limit_last_count.set(count);
            self.metrics.torii_signature_limit_max.set(limit);
        }
    }

    /// Record a rejection when NTS is unhealthy for time-sensitive admission.
    pub fn inc_torii_nts_unhealthy_reject(&self) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.torii_nts_unhealthy_reject_total.inc();
        }
    }

    /// Record a rejection of a transaction directly signed by a multisig account.
    pub fn inc_torii_multisig_direct_sign_reject(&self) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.torii_multisig_direct_sign_reject_total.inc();
        }
    }

    /// Increment Torii `SoraFS` admission counter for the given result/reason pair.
    pub fn inc_torii_sorafs_admission(&self, result: &'static str, reason: &'static str) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .torii_sorafs_admission_total
                .with_label_values(&[result, reason])
                .inc();
        }
    }

    /// Record an invalid Torii address observation labeled by endpoint + reason.
    pub fn inc_torii_address_invalid(&self, endpoint: &'static str, reason: &'static str) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.inc_torii_address_invalid(endpoint, reason);
        }
    }

    /// Record the domain kind observed for a successfully parsed address.
    pub fn inc_torii_address_domain(&self, endpoint: &'static str, domain_kind: &str) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.inc_torii_address_domain(endpoint, domain_kind);
        }
    }

    /// Record the `address_format` selection emitted by a Torii endpoint.
    pub fn inc_torii_address_format(&self, endpoint: &'static str, format: &'static str) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.inc_torii_address_format(endpoint, format);
        }
    }

    /// Record a Norito-RPC gate observation grouped by rollout stage and outcome.
    pub fn inc_torii_norito_rpc_gate(&self, stage: &'static str, outcome: &'static str) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.inc_torii_norito_rpc_gate(stage, outcome);
        }
    }

    /// Record a Torii Local-12 selector collision categorized by endpoint + kind.
    pub fn inc_torii_address_collision(&self, endpoint: &'static str, kind: &'static str) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.inc_torii_address_collision(endpoint, kind);
        }
    }

    /// Record a Torii Local-12 selector collision grouped by endpoint + domain.
    pub fn inc_torii_address_collision_domain(&self, endpoint: &'static str, domain: &str) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .inc_torii_address_collision_domain(endpoint, domain);
        }
    }

    /// Record whether Torii endpoints enforce strict address literal parsing.
    pub fn set_torii_address_strict_mode(&self, strict: bool) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.set_torii_address_strict_mode(strict);
        }
    }

    /// Record a DA rent quote projection for observability.
    pub fn record_da_rent_quote(
        &self,
        cluster_label: &str,
        storage_label: &str,
        gib_months: u64,
        rent_quote: &DaRentQuote,
    ) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .record_da_rent_quote(cluster_label, storage_label, gib_months, rent_quote);
        }
    }

    /// Record the result of a DA receipt ingestion attempt.
    pub fn record_da_receipt_outcome(
        &self,
        lane_id: u32,
        epoch: u64,
        sequence: u64,
        outcome: &str,
        cursor_advanced: bool,
    ) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.record_da_receipt_outcome(
                lane_id,
                epoch,
                sequence,
                outcome,
                cursor_advanced,
            );
        }
    }

    /// Update the highest-seen DA receipt cursor for a lane/epoch.
    pub fn set_da_receipt_cursor(&self, lane_id: u32, epoch: u64, sequence: u64) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.set_da_receipt_cursor(lane_id, epoch, sequence);
        }
    }

    /// Record a DA shard cursor event with lane/shard labels.
    pub fn record_da_shard_cursor_event(
        &self,
        event: &str,
        lane_id: u32,
        shard_id: u32,
        block_height: u64,
    ) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .record_da_shard_cursor_event(event, lane_id, shard_id, block_height);
        }
    }

    /// Record a DA shard cursor violation derived from the supplied error.
    pub fn record_da_shard_cursor_violation(
        &self,
        err: &DaShardCursorError,
        lane_id: u32,
        shard_id: u32,
        block_height: u64,
    ) {
        let reason = match *err {
            DaShardCursorError::Regression { .. } => "regression",
            DaShardCursorError::MissingCursor { .. } => "missing_cursor",
            DaShardCursorError::StaleCursor { .. } => "stale_cursor",
            DaShardCursorError::UnknownLane { .. } => "unknown_lane",
        };
        self.record_da_shard_cursor_event(reason, lane_id, shard_id, block_height);
    }

    /// Record the shard cursor lag measured in blocks.
    pub fn record_da_shard_cursor_lag(&self, lane_id: u32, shard_id: u32, lag_blocks: i64) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .set_da_shard_cursor_lag(lane_id, shard_id, lag_blocks);
        }
    }

    /// Record the projected `SoraFS` fee for `provider` in nano units.
    pub fn record_sorafs_fee_projection(&self, provider: &str, fee_nanos: u128) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .record_sorafs_fee_projection(provider, fee_nanos);
        }
    }

    /// Increment the `SoraFS` dispute counter for the provided result label.
    pub fn inc_sorafs_disputes(&self, result: &'static str) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.inc_sorafs_disputes(result);
        }
    }

    /// Record a `SoraFS` micropayment usage sample for the given provider.
    #[cfg(feature = "telemetry")]
    pub fn record_sorafs_micropayment_sample(
        &self,
        provider: &str,
        credits: MicropaymentCreditSnapshot,
        tickets: MicropaymentTicketCounters,
    ) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        if let Ok(mut guard) = self.micropayment_samples.write() {
            guard.insert(
                provider.to_string(),
                MicropaymentSampleRecord { credits, tickets },
            );
        }
    }

    /// Retrieve the most recent micropayment sample recorded for the provider, if any.
    #[cfg(feature = "telemetry")]
    #[must_use]
    pub fn micropayment_sample(&self, provider: &str) -> Option<MicropaymentSampleRecord> {
        self.micropayment_samples
            .read()
            .ok()
            .and_then(|map| map.get(provider).copied())
    }

    /// Surface all cached `SoraFS` micropayment samples.
    #[cfg(feature = "telemetry")]
    #[must_use]
    pub fn sorafs_micropayment_samples(&self) -> Vec<MicropaymentSampleStatus> {
        self.micropayment_samples
            .read()
            .map(|map| {
                map.iter()
                    .map(|(provider_id_hex, record)| MicropaymentSampleStatus {
                        provider_id_hex: provider_id_hex.clone(),
                        credits: record.credits,
                        tickets: record.tickets,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Emit an audit outcome telemetry event for routed-trace checkpoints.
    #[cfg(feature = "telemetry")]
    pub fn record_audit_outcome(
        &self,
        outcome: &TelemetryAuditOutcome,
    ) -> Option<norito::json::Value> {
        if !self.is_enabled() {
            return None;
        }

        self.metrics
            .nexus_audit_outcome_total
            .with_label_values(&[outcome.trace_id.as_str(), outcome.status.as_str()])
            .inc();
        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0))
            .as_secs();
        self.metrics
            .nexus_audit_outcome_last_timestamp
            .with_label_values(&[outcome.trace_id.as_str()])
            .set(now_secs);

        let payload = telemetry_event_to_json(&TelemetryEvent::AuditOutcome(outcome.clone()));
        match norito::json::to_json(&payload) {
            Ok(json) => iroha_logger::telemetry!(msg = "nexus.audit.outcome", event = json),
            Err(err) => {
                iroha_logger::warn!(?err, "failed to encode audit outcome telemetry event");
            }
        }
        Some(payload)
    }

    #[cfg(not(feature = "telemetry"))]
    /// No-op when telemetry is disabled.
    pub fn record_sorafs_micropayment_sample(
        &self,
        provider: &str,
        credits: MicropaymentCreditSnapshot,
        tickets: MicropaymentTicketCounters,
    ) {
        let _ = (self, provider, credits, tickets);
    }

    #[cfg(not(feature = "telemetry"))]
    #[must_use]
    /// Micropayment samples are unavailable when telemetry is disabled.
    pub fn micropayment_sample(&self, _provider: &str) -> Option<MicropaymentSampleRecord> {
        None
    }

    #[cfg(not(feature = "telemetry"))]
    #[must_use]
    /// Micropayment samples are unavailable when telemetry is disabled.
    pub fn sorafs_micropayment_samples(&self) -> Vec<MicropaymentSampleStatus> {
        Vec::new()
    }

    /// Record a proof stream outcome and optional latency.
    #[cfg(feature = "telemetry")]
    pub fn record_sorafs_proof_stream_event(
        &self,
        kind: &str,
        result: &str,
        reason: Option<&str>,
        provider_id: Option<&str>,
        tier: Option<&str>,
        latency_ms: Option<f64>,
    ) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.record_sorafs_proof_stream_event(
                kind,
                result,
                reason,
                provider_id,
                tier,
                latency_ms,
            );
        }
    }

    /// Record proof-health alert telemetry for a provider.
    #[cfg(feature = "telemetry")]
    pub fn record_sorafs_proof_health_alert(&self, alert: &SorafsProofHealthAlert) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        emit_sorafs_proof_health_alert(&self.metrics, alert);
    }

    #[cfg(not(feature = "telemetry"))]
    #[allow(unused_variables)]
    /// No-op proof-health alert recorder when the `telemetry` feature is disabled.
    pub fn record_sorafs_proof_health_alert(&self, alert: &SorafsProofHealthAlert) {
        let _ = alert;
    }

    /// Record `SoraFS` chunk-range fetch telemetry exposed by Torii.
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
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.record_sorafs_chunk_range(
                endpoint,
                status,
                bytes,
                chunker,
                profile,
                provider_id,
                tier,
                latency_ms,
            );
        }
    }

    /// Update the provider range capability gauge.
    pub fn set_sorafs_provider_range_capability(&self, feature: &str, count: i64) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .set_sorafs_provider_range_capability(feature, count);
        }
    }

    /// Increment the range fetch throttle counter for `reason`.
    pub fn inc_sorafs_range_fetch_throttle(&self, reason: &str) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.inc_sorafs_range_fetch_throttle(reason);
        }
    }

    /// Increment the active range fetch concurrency gauge.
    pub fn inc_sorafs_range_fetch_concurrency(&self) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.inc_sorafs_range_fetch_concurrency();
        }
    }

    /// Decrement the active range fetch concurrency gauge.
    pub fn dec_sorafs_range_fetch_concurrency(&self) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.dec_sorafs_range_fetch_concurrency();
        }
    }

    /// Record a GAR policy violation observed by the gateway.
    pub fn record_sorafs_gar_violation(&self, reason: &str, detail: &str) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.record_sorafs_gar_violation(reason, detail);
        }
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
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .record_sorafs_gateway_refusal(status, reason, profile, provider_id, scope);
        }
    }

    /// Publish metadata about the canonical `SoraFS` gateway fixture bundle.
    pub fn set_sorafs_gateway_fixture_metadata(
        &self,
        version: &str,
        profile: &str,
        digest_hex: &str,
        released_at_unix: u64,
    ) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.set_sorafs_gateway_fixture_metadata(
                version,
                profile,
                digest_hex,
                released_at_unix,
            );
        }
    }

    /// Observe encoder-to-ingest latency for an incoming Taikai segment.
    pub fn observe_taikai_ingest_latency(&self, cluster: &str, stream: &str, latency_ms: u32) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .observe_taikai_ingest_latency(cluster, stream, latency_ms);
        }
    }

    /// Observe live-edge drift for an incoming Taikai segment (signed gauge + absolute histogram).
    pub fn observe_taikai_live_edge_drift(&self, cluster: &str, stream: &str, drift_ms: i32) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .observe_taikai_live_edge_drift(cluster, stream, drift_ms);
        }
    }

    /// Increment Taikai ingest error counters grouped by cluster/stream/reason.
    pub fn inc_taikai_ingest_error(&self, cluster: &str, stream: &str, reason: &str) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .inc_taikai_ingest_error(cluster, stream, reason);
        }
    }

    /// Record a Taikai alias rotation accepted via `/v1/da/ingest`.
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
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.record_taikai_alias_rotation(
                cluster,
                event,
                stream,
                alias_namespace,
                alias_name,
                window_start_sequence,
                window_end_sequence,
                manifest_digest_hex,
            );
        }
    }

    /// Observe proof verification metrics (latency/size) for the given backend/status.
    pub fn observe_zk_verify(
        &self,
        backend: &str,
        status: ProofStatus,
        proof_bytes: usize,
        latency_ms: u64,
    ) {
        if self.enabled.load(Ordering::Relaxed) {
            let status_label = proof_status_label(status);
            let proof_bytes_value =
                u64::try_from(proof_bytes).map_or_else(|_| u64_to_f64(u64::MAX), u64_to_f64);
            self.metrics
                .zk_verify_latency_ms
                .with_label_values(&[backend, status_label])
                .observe(u64_to_f64(latency_ms));
            self.metrics
                .zk_verify_proof_bytes
                .with_label_values(&[backend, status_label])
                .observe(proof_bytes_value);
        }
    }

    /// Increment Torii active connection gauge for the provided scheme label.
    pub fn inc_torii_active_conn(&self, scheme: &'static str) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .torii_active_connections_total
                .with_label_values(&[scheme])
                .inc();
        }
    }

    /// Decrement Torii active connection gauge for the provided scheme label.
    pub fn dec_torii_active_conn(&self, scheme: &'static str) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .torii_active_connections_total
                .with_label_values(&[scheme])
                .dec();
        }
    }

    /// Increment background-post enqueued counter labeled by kind {Post,Broadcast}
    pub fn inc_bg_post_enqueued(&self, kind: &'static str) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .sumeragi_bg_post_enqueued_total
                .with_label_values(&[kind])
                .inc();
            // Update queue depth (global)
            let cur = self.metrics.sumeragi_bg_post_queue_depth.get();
            self.metrics
                .sumeragi_bg_post_queue_depth
                .set(cur.saturating_add(1));
        }
    }

    /// Increment background-post overflow counter labeled by kind {Post,Broadcast}
    pub fn inc_bg_post_overflow(&self, kind: &'static str) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .sumeragi_bg_post_overflow_total
                .with_label_values(&[kind])
                .inc();
        }
    }

    /// Increment background-post drop counter labeled by kind {Post,Broadcast}
    pub fn inc_bg_post_drop(&self, kind: &'static str) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .sumeragi_bg_post_drop_total
                .with_label_values(&[kind])
                .inc();
        }
    }

    /// Increment per-peer background-post queue depth for Post tasks.
    pub fn inc_bg_post_queue_depth_for_peer(&self, peer: &iroha_data_model::peer::PeerId) {
        if self.enabled.load(Ordering::Relaxed) {
            let label = peer.to_string();
            let g = self
                .metrics
                .sumeragi_bg_post_queue_depth_by_peer
                .with_label_values(&[label.as_str()]);
            let cur = g.get();
            g.set(cur.saturating_add(1));
        }
    }

    /// Decrement background-post queue depth (global) and per-peer when applicable.
    pub fn dec_bg_post_queue_depth(&self) {
        if self.enabled.load(Ordering::Relaxed) {
            let cur = self.metrics.sumeragi_bg_post_queue_depth.get();
            self.metrics
                .sumeragi_bg_post_queue_depth
                .set(cur.saturating_sub(1));
        }
    }

    /// Decrement per-peer background-post queue depth for Post tasks.
    pub fn dec_bg_post_queue_depth_for_peer(&self, peer: &iroha_data_model::peer::PeerId) {
        if self.enabled.load(Ordering::Relaxed) {
            let label = peer.to_string();
            let g = self
                .metrics
                .sumeragi_bg_post_queue_depth_by_peer
                .with_label_values(&[label.as_str()]);
            let cur = g.get();
            g.set(cur.saturating_sub(1));
        }
    }

    /// Observe background-post age in milliseconds for a given kind {Post,Broadcast}.
    pub fn observe_bg_post_age_ms(&self, kind: &'static str, ms: f64) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .sumeragi_bg_post_age_ms
                .with_label_values(&[kind])
                .observe(ms.max(0.0));
        }
    }

    /// Set `NEW_VIEW` receipts count for a specific (height, view)
    pub fn set_new_view_receipts(&self, height: u64, view: u64, count: u64) {
        if self.enabled.load(Ordering::Relaxed) {
            let h = height.to_string();
            let v = view.to_string();
            self.metrics
                .sumeragi_new_view_receipts_by_hv
                .with_label_values(&[h.as_str(), v.as_str()])
                .set(count);
        }
    }

    /// Set current RBC sessions active gauge.
    pub fn set_rbc_sessions_active(&self, active: u64) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.sumeragi_rbc_sessions_active.set(active);
        }
    }

    /// Increment RBC sessions pruned counter by `delta`.
    pub fn inc_rbc_sessions_pruned(&self, delta: u64) {
        if self.enabled.load(Ordering::Relaxed) {
            for _ in 0..delta {
                self.metrics.sumeragi_rbc_sessions_pruned_total.inc();
            }
        }
    }

    /// Increment RBC READY broadcasts counter.
    pub fn inc_rbc_ready_broadcasts(&self) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.sumeragi_rbc_ready_broadcasts_total.inc();
        }
    }

    /// Increment RBC DELIVER broadcasts counter.
    pub fn inc_rbc_deliver_broadcasts(&self) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.sumeragi_rbc_deliver_broadcasts_total.inc();
        }
    }

    /// Add to RBC payload bytes delivered (cumulative gauge).
    pub fn add_rbc_payload_bytes_delivered(&self, bytes: u64) {
        if self.enabled.load(Ordering::Relaxed) {
            let cur = self
                .metrics
                .sumeragi_rbc_payload_bytes_delivered_total
                .get();
            self.metrics
                .sumeragi_rbc_payload_bytes_delivered_total
                .set(cur.saturating_add(bytes));
        }
    }

    /// Increment counter for RBC DELIVER deferrals waiting on READY quorum.
    pub fn inc_rbc_deliver_defer_ready(&self) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.sumeragi_rbc_deliver_defer_ready_total.inc();
        }
    }

    /// Increment counter for RBC DELIVER deferrals waiting on missing chunks.
    pub fn inc_rbc_deliver_defer_chunks(&self) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.sumeragi_rbc_deliver_defer_chunks_total.inc();
        }
    }

    /// Increment DA deadline reschedule counter when transactions are re-queued.
    pub fn inc_da_reschedule(&self, mode_tag: &str) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.set_sumeragi_mode_tag(mode_tag);
            self.metrics.sumeragi_rbc_da_reschedule_total.inc();
            self.metrics
                .sumeragi_rbc_da_reschedule_by_mode_total
                .with_label_values(&[mode_tag])
                .inc();
            self.da_slot_rescheduled.store(true, Ordering::Relaxed);
        }
    }

    /// Record an RBC abort (e.g., DA deadline or payload eviction) labeled by consensus mode.
    pub fn inc_rbc_abort(&self, mode_tag: &str) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.set_sumeragi_mode_tag(mode_tag);
            self.metrics
                .sumeragi_rbc_abort_total
                .with_label_values(&[mode_tag])
                .inc();
        }
    }

    /// Record when the commit pipeline runs from the pacemaker tick loop.
    pub fn note_commit_pipeline_tick(&self, mode_tag: &str, has_pending: bool) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        self.metrics.set_sumeragi_mode_tag(mode_tag);
        let outcome = if has_pending { "active" } else { "idle" };
        self.metrics
            .sumeragi_commit_pipeline_tick_total
            .with_label_values(&[mode_tag, outcome])
            .inc();
    }

    /// Record a prevote-quorum timeout that triggered a view change.
    pub fn inc_prevote_timeout(&self, mode_tag: &str) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.set_sumeragi_mode_tag(mode_tag);
            self.metrics
                .sumeragi_prevote_timeout_total
                .with_label_values(&[mode_tag])
                .inc();
        }
    }

    /// Increment availability vote ingestion counter.
    pub fn inc_da_vote_ingested(&self) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.sumeragi_da_votes_ingested_total.inc();
        }
    }

    /// Increment availability vote ingestion counter labeled by collector index.
    pub fn inc_da_vote_ingested_for_collector(&self, idx: usize) {
        if self.enabled.load(Ordering::Relaxed) {
            let label = idx.to_string();
            self.metrics
                .sumeragi_da_votes_ingested_by_collector
                .with_label_values(&[label.as_str()])
                .inc();
        }
    }

    /// Increment availability vote ingestion counter labeled by collector peer id.
    pub fn inc_da_vote_ingested_for_peer(&self, peer: &iroha_data_model::peer::PeerId) {
        if self.enabled.load(Ordering::Relaxed) {
            let label = peer.to_string();
            self.metrics
                .sumeragi_da_votes_ingested_by_peer
                .with_label_values(&[label.as_str()])
                .inc();
        }
    }

    /// Observe QC assembly latency in milliseconds for the provided kind (e.g., `availability`).
    pub fn observe_qc_latency_ms(&self, kind: &'static str, ms: u64) {
        if self.enabled.load(Ordering::Relaxed) {
            let value = u64_to_f64(ms);
            self.metrics
                .sumeragi_qc_assembly_latency_ms
                .with_label_values(&[kind])
                .observe(value.max(0.0));
            self.metrics
                .sumeragi_qc_last_latency_ms
                .with_label_values(&[kind])
                .set(ms);
        }
    }

    /// Update persisted RBC store usage and pressure gauges.
    pub fn set_rbc_store_pressure(&self, pressure: StorePressure) {
        if self.enabled.load(Ordering::Relaxed) {
            let sessions = pressure.sessions() as u64;
            let bytes = pressure.bytes() as u64;
            let level = match pressure {
                StorePressure::Normal { .. } => 0,
                StorePressure::SoftLimit { .. } => 1,
                StorePressure::HardLimit { .. } => 2,
            };
            self.metrics.sumeragi_rbc_store_sessions.set(sessions);
            self.metrics.sumeragi_rbc_store_bytes.set(bytes);
            self.metrics.sumeragi_rbc_store_pressure.set(level);
        }
    }

    /// Increment proposal deferral counter when RBC store back-pressure is active.
    pub fn inc_rbc_store_backpressure_deferrals(&self) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.sumeragi_rbc_backpressure_deferrals_total.inc();
        }
    }

    /// Increment RBC store eviction counter when persistence prunes sessions.
    pub fn inc_rbc_store_evictions(&self, count: u64) {
        if count == 0 || !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        self.metrics
            .sumeragi_rbc_store_evictions_total
            .inc_by(count);
    }

    /// Increment kura persistence failure counter labeled by outcome.
    pub fn inc_kura_store_failure(&self, outcome: &'static str) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .sumeragi_kura_store_failures_total
                .with_label_values(&[outcome])
                .inc();
        }
    }

    /// Record the most recent kura persistence retry attempt/backoff.
    pub fn set_kura_store_retry(&self, attempt: u64, backoff_ms: u64) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .sumeragi_kura_store_last_retry_attempt
                .set(attempt);
            self.metrics
                .sumeragi_kura_store_last_retry_backoff_ms
                .set(backoff_ms);
        }
    }

    /// Increment proposal deferral counter when the pacemaker stops due to queue backpressure.
    pub fn inc_pacemaker_backpressure_deferrals(&self) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .sumeragi_pacemaker_backpressure_deferrals_total
                .inc();
        }
    }

    /// Update RBC backlog gauges derived from active sessions.
    pub fn set_rbc_backlog(&self, total_missing: u64, max_missing: u64, pending: u64) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .sumeragi_rbc_backlog_chunks_total
                .set(total_missing);
            self.metrics
                .sumeragi_rbc_backlog_chunks_max
                .set(max_missing);
            self.metrics
                .sumeragi_rbc_backlog_sessions_pending
                .set(pending);
        }
    }

    /// Update gauges for pending RBC stashes awaiting INIT.
    pub fn set_rbc_pending(&self, sessions: u64, chunks: u64, bytes: u64) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.sumeragi_rbc_pending_sessions.set(sessions);
            self.metrics.sumeragi_rbc_pending_chunks.set(chunks);
            self.metrics.sumeragi_rbc_pending_bytes.set(bytes);
        }
    }

    /// Record dropped pending RBC frames by reason (`cap/session_cap/ttl`).
    pub fn inc_rbc_pending_drop(&self, reason: &str, frames: u64, bytes: u64) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        let frames = frames.max(1);
        self.metrics
            .sumeragi_rbc_pending_drops_total
            .with_label_values(&[reason])
            .inc_by(frames);
        self.metrics
            .sumeragi_rbc_pending_dropped_bytes_total
            .with_label_values(&[reason])
            .inc_by(bytes);
    }

    /// Record pending RBC stash evictions (sessions).
    pub fn inc_rbc_pending_evicted(&self, sessions: u64) {
        if sessions == 0 || !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        self.metrics
            .sumeragi_rbc_pending_evicted_total
            .inc_by(sessions);
    }

    /// Update per-lane RBC backlog gauges.
    pub fn set_rbc_lane_backlog(&self, entries: &[crate::sumeragi::status::LaneRbcSnapshot]) {
        if !self.nexus_lane_metrics_enabled() {
            return;
        }

        let metrics = &self.metrics;
        metrics.sumeragi_rbc_lane_tx_count.reset();
        metrics.sumeragi_rbc_lane_total_chunks.reset();
        metrics.sumeragi_rbc_lane_pending_chunks.reset();
        metrics.sumeragi_rbc_lane_bytes_total.reset();

        for entry in entries {
            let lane = entry.lane_id.to_string();
            metrics
                .sumeragi_rbc_lane_tx_count
                .with_label_values(&[lane.as_str()])
                .set(entry.tx_count);
            metrics
                .sumeragi_rbc_lane_total_chunks
                .with_label_values(&[lane.as_str()])
                .set(entry.total_chunks);
            metrics
                .sumeragi_rbc_lane_pending_chunks
                .with_label_values(&[lane.as_str()])
                .set(entry.pending_chunks);
            metrics
                .sumeragi_rbc_lane_bytes_total
                .with_label_values(&[lane.as_str()])
                .set(entry.rbc_bytes_total);
        }
    }

    /// Update per-dataspace RBC backlog gauges.
    pub fn set_rbc_dataspace_backlog(
        &self,
        entries: &[crate::sumeragi::status::DataspaceRbcSnapshot],
    ) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }

        let metrics = &self.metrics;
        metrics.sumeragi_rbc_dataspace_tx_count.reset();
        metrics.sumeragi_rbc_dataspace_total_chunks.reset();
        metrics.sumeragi_rbc_dataspace_pending_chunks.reset();
        metrics.sumeragi_rbc_dataspace_bytes_total.reset();

        for entry in entries {
            let lane = entry.lane_id.to_string();
            let dataspace = entry.dataspace_id.to_string();
            let labels = [lane.as_str(), dataspace.as_str()];
            metrics
                .sumeragi_rbc_dataspace_tx_count
                .with_label_values(&labels)
                .set(entry.tx_count);
            metrics
                .sumeragi_rbc_dataspace_total_chunks
                .with_label_values(&labels)
                .set(entry.total_chunks);
            metrics
                .sumeragi_rbc_dataspace_pending_chunks
                .with_label_values(&labels)
                .set(entry.pending_chunks);
            metrics
                .sumeragi_rbc_dataspace_bytes_total
                .with_label_values(&labels)
                .set(entry.rbc_bytes_total);
        }
    }

    /// Record the latest `SoraFS` metering snapshot for `provider`.
    #[cfg(feature = "telemetry")]
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
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.record_sorafs_metering(
                provider,
                declared_gib,
                effective_gib,
                utilised_gib,
                outstanding_gib,
                outstanding_orders,
                gib_hours,
                orders_issued,
                orders_completed,
                orders_failed,
                uptime_bps,
                por_bps,
            );
        }
    }

    /// Record the latest `SoraFS` storage scheduler snapshot for `provider`.
    #[cfg(feature = "telemetry")]
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
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.record_sorafs_storage(
                provider,
                bytes_used,
                bytes_capacity,
                pin_queue_depth,
                fetch_inflight,
                fetch_bytes_per_sec,
                por_inflight,
                por_samples_success,
                por_samples_failed,
            );
        }
    }

    /// Record the `PoR` ingestion backlog for a manifest/provider pair.
    #[cfg(feature = "telemetry")]
    pub fn record_sorafs_por_ingestion_backlog(
        &self,
        provider: &str,
        manifest: &str,
        pending: u64,
    ) {
        self.metrics
            .record_sorafs_por_ingestion_backlog(provider, manifest, pending);
    }

    /// Record the cumulative `PoR` ingestion failure count for a manifest/provider pair.
    #[cfg(feature = "telemetry")]
    pub fn record_sorafs_por_ingestion_failures(
        &self,
        provider: &str,
        manifest: &str,
        failures_total: u64,
    ) {
        self.metrics
            .record_sorafs_por_ingestion_failures(provider, manifest, failures_total);
    }

    /// Record aggregate `SoraFS` registry statistics exposed by Torii.
    #[cfg(feature = "telemetry")]
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
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.record_sorafs_registry(
                manifests_pending,
                manifests_approved,
                manifests_retired,
                alias_total,
                orders_pending,
                orders_completed,
                orders_expired,
                sla_met,
                sla_missed,
                completion_latencies,
                deadline_slack_epochs,
            );
        }
    }

    /// Record `SoraFS` alias cache observations exposed by Torii.
    #[cfg(feature = "telemetry")]
    pub fn record_sorafs_alias_cache(&self, result: &str, reason: &str, age_secs: f64) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .record_sorafs_alias_cache(result, reason, age_secs);
        }
    }

    /// Record HTTP request metrics for Torii (content type, method, status, latency, bytes).
    pub fn observe_torii_http_request(
        &self,
        method: &str,
        status: StatusCode,
        content_type: &str,
        duration: Duration,
        response_bytes: Option<u64>,
    ) {
        if self.enabled.load(Ordering::Relaxed) {
            let status_label = status.as_u16().to_string();
            let content_label = if content_type.is_empty() {
                String::from("unknown")
            } else {
                content_type.to_ascii_lowercase()
            };
            self.metrics
                .torii_http_requests_total
                .with_label_values(&[content_label.as_str(), method, status_label.as_str()])
                .inc();
            self.metrics
                .torii_http_request_duration_seconds
                .with_label_values(&[content_label.as_str(), method])
                .observe(duration.as_secs_f64());
            if let Some(bytes) = response_bytes {
                self.metrics
                    .torii_http_response_bytes_total
                    .with_label_values(&[content_label.as_str(), method, status_label.as_str()])
                    .inc_by(bytes);
            }
        }
    }

    /// Record Torii API version negotiation outcomes.
    pub fn observe_torii_api_version(&self, result: &str, version: &str) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .torii_api_version_negotiated_total
                .with_label_values(&[result, version])
                .inc();
        }
    }

    /// Record metrics for the content gateway path.
    pub fn observe_torii_content_request(&self, outcome: &str, bytes: u64, duration: Duration) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .torii_content_requests_total
                .with_label_values(&[outcome])
                .inc();
            self.metrics
                .torii_content_request_duration_seconds
                .with_label_values(&[outcome])
                .observe(duration.as_secs_f64());
            self.metrics
                .torii_content_response_bytes_total
                .with_label_values(&[outcome])
                .inc_by(bytes);
        }
    }

    /// Record proof endpoint request metrics.
    pub fn observe_torii_proof_request(
        &self,
        endpoint: &str,
        outcome: &str,
        bytes: u64,
        duration: Duration,
    ) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .record_torii_proof_request(endpoint, outcome, bytes, duration);
        }
    }

    /// Increment proof cache hit counter for the provided endpoint.
    pub fn inc_torii_proof_cache_hit(&self, endpoint: &str) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.inc_torii_proof_cache_hit(endpoint);
        }
    }

    /// Record scheme-level Torii request latency and failure counters.
    pub fn observe_torii_request_by_scheme(
        &self,
        scheme: &str,
        status: StatusCode,
        duration: Duration,
    ) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .torii_request_duration_seconds
                .with_label_values(&[scheme])
                .observe(duration.as_secs_f64());
            if status.is_client_error() || status.is_server_error() {
                let status_label = status.as_u16().to_string();
                self.metrics
                    .torii_request_failures_total
                    .with_label_values(&[scheme, status_label.as_str()])
                    .inc();
            }
        }
    }

    /// Record a rejected attachment during Torii sanitization.
    pub fn inc_torii_attachment_reject(&self, reason: &str) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.inc_torii_attachment_reject(reason);
        }
    }

    /// Record attachment sanitization latency in milliseconds.
    pub fn observe_torii_attachment_sanitize_ms(&self, millis: u64) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.observe_torii_attachment_sanitize_ms(millis);
        }
    }

    /// Observe attachment size/latency for the background prover.
    pub fn observe_torii_zk_prover(
        &self,
        status: &'static str,
        content_type: &str,
        size_bytes: u64,
        latency_ms: u64,
    ) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .torii_zk_prover_attachment_bytes
                .with_label_values(&[status, content_type])
                .observe(u64_to_f64(size_bytes));
            self.metrics
                .torii_zk_prover_latency_ms
                .with_label_values(&[status])
                .observe(u64_to_f64(latency_ms));
        }
    }

    /// Increment the TTL GC counter for the background prover by `deleted` entries.
    pub fn inc_torii_zk_prover_gc(&self, deleted: u64) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.torii_zk_prover_gc_total.inc_by(deleted);
        }
    }

    /// Set the number of background prover attachments currently in flight.
    pub fn set_torii_zk_prover_inflight(&self, inflight: u64) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.torii_zk_prover_inflight.set(inflight);
        }
    }

    /// Set the number of background prover attachments pending processing.
    pub fn set_torii_zk_prover_pending(&self, pending: u64) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.torii_zk_prover_pending.set(pending);
        }
    }

    /// Record bytes processed and duration for the last background prover scan.
    pub fn record_torii_zk_prover_scan(&self, bytes: u64, millis: u64) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.torii_zk_prover_last_scan_bytes.set(bytes);
            self.metrics.torii_zk_prover_last_scan_ms.set(millis);
        }
    }

    /// Increment the background prover budget exhaustion counter.
    pub fn inc_torii_zk_prover_budget_exhausted(&self, reason: &'static str) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .torii_zk_prover_budget_exhausted_total
                .with_label_values(&[reason])
                .inc();
        }
    }

    /// Increment the in-flight proof stream gauge for `kind`.
    #[cfg(feature = "telemetry")]
    pub fn inc_sorafs_proof_stream_inflight(&self, kind: &str) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.inc_sorafs_proof_stream_inflight(kind);
        }
    }

    /// Decrement the in-flight proof stream gauge for `kind`.
    #[cfg(feature = "telemetry")]
    pub fn dec_sorafs_proof_stream_inflight(&self, kind: &str) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.dec_sorafs_proof_stream_inflight(kind);
        }
    }

    /// Update TLS state gauges exposed by Torii.
    pub fn set_sorafs_tls_state(&self, ech_enabled: bool, expiry: Option<Duration>) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.set_sorafs_tls_state(ech_enabled, expiry);
        }
    }

    /// Record the outcome of a TLS renewal attempt.
    pub fn record_sorafs_tls_renewal(&self, result: &str) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.record_sorafs_tls_renewal(result);
        }
    }

    /// Update the gauge tracking the active `SoraFS` gateway fixture version.
    pub fn set_sorafs_gateway_fixture_version(&self, version: &str) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.set_sorafs_gateway_fixture_version(version);
        }
    }

    /// Increment Torii contract error counter for the provided endpoint label.
    pub fn inc_torii_contract_error(&self, endpoint: &'static str) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .torii_contract_errors_total
                .with_label_values(&[endpoint])
                .inc();
        }
    }

    /// Increment Torii contract throttle counter for the provided endpoint label.
    pub fn inc_torii_contract_throttle(&self, endpoint: &'static str) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .torii_contract_throttled_total
                .with_label_values(&[endpoint])
                .inc();
        }
    }

    /// Increment Torii proof throttle counter for the provided endpoint label.
    pub fn inc_torii_proof_throttle(&self, endpoint: &'static str) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .torii_proof_throttled_total
                .with_label_values(&[endpoint])
                .inc();
        }
    }

    #[cfg(not(feature = "telemetry"))]
    /// No-op when telemetry is disabled.
    #[allow(clippy::too_many_arguments)]
    pub fn record_sorafs_metering(
        &self,
        _provider: &str,
        _declared_gib: u64,
        _effective_gib: u64,
        _utilised_gib: u64,
        _outstanding_gib: u64,
        _outstanding_orders: u64,
        _gib_hours: f64,
        _orders_issued: u64,
        _orders_completed: u64,
        _orders_failed: u64,
        _uptime_bps: u32,
        _por_bps: u32,
    ) {
    }

    #[cfg(not(feature = "telemetry"))]
    /// No-op when telemetry is disabled.
    pub fn record_sorafs_por_ingestion_backlog(
        &self,
        _provider: &str,
        _manifest: &str,
        _pending: u64,
    ) {
    }

    #[cfg(not(feature = "telemetry"))]
    /// No-op when telemetry is disabled.
    pub fn record_sorafs_por_ingestion_failures(
        &self,
        _provider: &str,
        _manifest: &str,
        _failures_total: u64,
    ) {
    }

    #[cfg(not(feature = "telemetry"))]
    /// No-op when telemetry is disabled.
    #[allow(clippy::too_many_arguments)]
    pub fn record_sorafs_storage(
        &self,
        _provider: &str,
        _bytes_used: u64,
        _bytes_capacity: u64,
        _pin_queue_depth: u64,
        _fetch_inflight: u64,
        _fetch_bytes_per_sec: u64,
        _por_inflight: u64,
        _por_samples_success: u64,
        _por_samples_failed: u64,
    ) {
    }

    #[cfg(not(feature = "telemetry"))]
    /// No-op when telemetry is disabled.
    #[allow(clippy::too_many_arguments)]
    pub fn record_sorafs_registry(
        &self,
        _manifests_pending: u64,
        _manifests_approved: u64,
        _manifests_retired: u64,
        _alias_total: u64,
        _orders_pending: u64,
        _orders_completed: u64,
        _orders_expired: u64,
        _sla_met: u64,
        _sla_missed: u64,
        _completion_latencies: &[f64],
        _deadline_slack_epochs: &[f64],
    ) {
    }

    #[cfg(not(feature = "telemetry"))]
    /// No-op when telemetry is disabled.
    pub fn record_sorafs_alias_cache(&self, _result: &str, _reason: &str, _age_secs: f64) {}

    #[cfg(not(feature = "telemetry"))]
    /// No-op when telemetry is disabled.
    pub fn inc_sorafs_proof_stream_inflight(&self, _kind: &str) {}

    #[cfg(not(feature = "telemetry"))]
    /// No-op when telemetry is disabled.
    pub fn dec_sorafs_proof_stream_inflight(&self, _kind: &str) {}

    #[cfg(not(feature = "telemetry"))]
    /// No-op when telemetry is disabled.
    pub fn record_sorafs_proof_stream_event(
        &self,
        _kind: &str,
        _result: &str,
        _reason: Option<&str>,
        _provider_id: Option<&str>,
        _tier: Option<&str>,
        _latency_ms: Option<f64>,
    ) {
    }

    /// Update per-lane and per-dataspace commitment metrics using the latest queue snapshot.
    #[cfg(not(feature = "telemetry"))]
    #[allow(unused_variables)]
    pub fn record_lane_commitments(
        &self,
        lane_entries: &[LaneCommitmentSnapshot],
        dataspace_entries: &[DataspaceCommitmentSnapshot],
        limits: &QueueLimits,
    ) {
        #[allow(unused_variables)]
        let _ = (lane_entries, dataspace_entries, limits);
    }

    /// Update per-lane and per-dataspace commitment metrics using the latest queue snapshot.
    #[cfg(feature = "telemetry")]
    pub fn record_lane_commitments(
        &self,
        lane_entries: &[LaneCommitmentSnapshot],
        dataspace_entries: &[DataspaceCommitmentSnapshot],
        limits: &QueueLimits,
    ) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }

        self.metrics.nexus_scheduler_lane_teu_capacity.reset();
        self.metrics.nexus_scheduler_lane_teu_slot_committed.reset();
        self.metrics.nexus_scheduler_lane_trigger_level.reset();
        self.metrics.nexus_scheduler_starvation_bound_slots.reset();
        self.metrics.nexus_scheduler_lane_teu_slot_breakdown.reset();
        self.metrics.nexus_scheduler_dataspace_teu_backlog.reset();
        self.metrics.nexus_scheduler_dataspace_age_slots.reset();
        self.metrics
            .nexus_scheduler_dataspace_virtual_finish
            .reset();

        for entry in lane_entries {
            let lane_id = LaneId::new(entry.lane_id);
            let lane_label = entry.lane_id.to_string();
            let limit = limits.for_lane(lane_id);
            let buckets = NexusLaneTeuBuckets {
                floor: entry.teu_total,
                headroom: limit.teu_capacity.saturating_sub(entry.teu_total),
                must_serve: 0,
                circuit_breaker: 0,
            };

            self.metrics
                .nexus_scheduler_lane_teu_capacity
                .with_label_values(&[lane_label.as_str()])
                .set(limit.teu_capacity);
            self.metrics
                .nexus_scheduler_lane_teu_slot_committed
                .with_label_values(&[lane_label.as_str()])
                .set(entry.teu_total);
            self.metrics
                .nexus_scheduler_lane_trigger_level
                .with_label_values(&[lane_label.as_str()])
                .set(0);
            self.metrics
                .nexus_scheduler_starvation_bound_slots
                .with_label_values(&[lane_label.as_str()])
                .set(limit.starvation_bound_slots);
            for (bucket, value) in buckets.iter() {
                self.metrics
                    .nexus_scheduler_lane_teu_slot_breakdown
                    .with_label_values(&[lane_label.as_str(), bucket])
                    .set(value);
            }
        }

        for entry in dataspace_entries {
            let lane_label = entry.lane_id.to_string();
            let dataspace_label = entry.dataspace_id.to_string();
            self.metrics
                .nexus_scheduler_dataspace_teu_backlog
                .with_label_values(&[lane_label.as_str(), dataspace_label.as_str()])
                .set(0);
            self.metrics
                .nexus_scheduler_dataspace_age_slots
                .with_label_values(&[lane_label.as_str(), dataspace_label.as_str()])
                .set(0);
            self.metrics
                .nexus_scheduler_dataspace_virtual_finish
                .with_label_values(&[lane_label.as_str(), dataspace_label.as_str()])
                .set(0);
        }
    }

    /// Set pacemaker current backoff window (ms)
    pub fn set_pacemaker_backoff_ms(&self, ms: u64) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.sumeragi_pacemaker_backoff_ms.set(ms);
        }
    }

    /// Set pacemaker RTT floor (ms)
    pub fn set_pacemaker_rtt_floor_ms(&self, ms: u64) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.sumeragi_pacemaker_rtt_floor_ms.set(ms);
        }
    }

    /// Set static pacemaker config gauges: multipliers and max backoff
    pub fn set_pacemaker_config(&self, backoff_mul: u64, rtt_floor_mul: u64, max_backoff_ms: u64) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .sumeragi_pacemaker_backoff_multiplier
                .set(backoff_mul);
            self.metrics
                .sumeragi_pacemaker_rtt_floor_multiplier
                .set(rtt_floor_mul);
            self.metrics
                .sumeragi_pacemaker_max_backoff_ms
                .set(max_backoff_ms);
        }
    }

    /// Set pacemaker jitter band config (permille)
    pub fn set_pacemaker_jitter_permille(&self, permille: u64) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .sumeragi_pacemaker_jitter_frac_permille
                .set(permille);
        }
    }

    /// Set pacemaker jitter magnitude (ms)
    pub fn set_pacemaker_jitter_ms(&self, ms: u64) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.sumeragi_pacemaker_jitter_ms.set(ms);
        }
    }

    /// Observe per-phase latency in milliseconds (labeled by `phase`).
    pub fn observe_phase_latency_ms(&self, phase: &'static str, ms: f64) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .sumeragi_phase_latency_ms
                .with_label_values(&[phase])
                .observe(ms.max(0.0));
        }
    }

    /// Record the per-phase EMA latency (milliseconds) for the given `phase`.
    pub fn set_phase_latency_ema_ms(&self, phase: &'static str, ms: f64) {
        if self.enabled.load(Ordering::Relaxed) {
            let clamped = ms.clamp(0.0, u64_to_f64(u64::MAX));
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let quantized = clamped.round() as u64;
            self.metrics
                .sumeragi_phase_latency_ema_ms
                .with_label_values(&[phase])
                .set(quantized);
        }
    }

    /// Record the aggregated pipeline EMA latency (ms) across pacemaker-controlled phases.
    pub fn set_phase_latency_total_ema_ms(&self, ms: f64) {
        if self.enabled.load(Ordering::Relaxed) {
            let clamped = ms.clamp(0.0, u64_to_f64(u64::MAX));
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let quantized = clamped.round() as u64;
            self.metrics.sumeragi_phase_total_ema_ms.set(quantized);
        }
    }

    /// Set elapsed time in current consensus round (ms)
    pub fn set_pacemaker_round_elapsed_ms(&self, ms: u64) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.sumeragi_pacemaker_round_elapsed_ms.set(ms);
        }
    }

    /// Set current view-timeout target window (ms)
    pub fn set_pacemaker_view_timeout_target_ms(&self, ms: u64) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .sumeragi_pacemaker_view_timeout_target_ms
                .set(ms);
        }
    }

    /// Set remaining time until current view-timeout elapses (ms)
    pub fn set_pacemaker_view_timeout_remaining_ms(&self, ms: u64) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .sumeragi_pacemaker_view_timeout_remaining_ms
                .set(ms);
        }
    }
    /// Increase dropped messages metric
    pub fn inc_dropped_messages(&self) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.dropped_messages.inc();
        }
    }

    /// Increase dropped block messages metric (consensus path)
    pub fn inc_dropped_block_message(&self) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.sumeragi_dropped_block_messages_total.inc();
            // Keep aggregate counter in sync
            self.metrics.dropped_messages.inc();
        }
    }

    /// Increase dropped control messages metric (control path)
    pub fn inc_dropped_control_message(&self) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.sumeragi_dropped_control_messages_total.inc();
            // Keep aggregate counter in sync
            self.metrics.dropped_messages.inc();
        }
    }

    /// Set view changes metrics
    pub fn set_view_changes(&self, value: u64) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.view_changes.set(value);
        }
    }

    /// Increment counter: votes accepted at proxy tail
    pub fn inc_tail_vote(&self) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.sumeragi_tail_votes_total.inc();
        }
    }

    /// Increment counter: widen-before-rotate events
    pub fn inc_widen_before_rotate(&self) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.sumeragi_widen_before_rotate_total.inc();
        }
    }

    /// Increment counter: view-change suggestions
    pub fn inc_view_change_suggest(&self) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.sumeragi_view_change_suggest_total.inc();
        }
    }

    /// Increment counter: view-change installs
    pub fn inc_view_change_install(&self) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.sumeragi_view_change_install_total.inc();
        }
    }

    fn inc_view_change_proof_gauge(&self, outcome: &'static str) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .sumeragi_view_change_proof_total
                .with_label_values(&[outcome])
                .inc();
        }
    }

    /// Increment counter: view-change proofs accepted (advanced the chain)
    pub fn inc_view_change_proof_accepted(&self) {
        self.inc_view_change_proof_gauge("accepted");
    }

    /// Increment counter: view-change proofs ignored as stale/outdated
    pub fn inc_view_change_proof_stale(&self) {
        self.inc_view_change_proof_gauge("stale");
    }

    /// Increment counter: view-change proofs rejected due to validation errors
    pub fn inc_view_change_proof_rejected(&self) {
        self.inc_view_change_proof_gauge("rejected");
    }

    /// Increment counter: redundant vote sends to collectors
    pub fn inc_redundant_send(&self) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.sumeragi_redundant_sends_total.inc();
        }
    }

    /// Increment counter: redundant vote send labeled by collector index
    pub fn inc_redundant_send_for_collector(&self, idx: usize) {
        if self.enabled.load(Ordering::Relaxed) {
            let label = idx.to_string();
            self.metrics
                .sumeragi_redundant_sends_by_collector
                .with_label_values(&[label.as_str()])
                .inc();
        }
    }

    /// Increment `NPoS` PRF: this node selected as collector in current (h,v).
    pub fn inc_npos_collector_selected(&self) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.sumeragi_npos_collector_selected_total.inc();
        }
    }

    /// Increment `NPoS` PRF collector assignment counter labeled by collector index.
    pub fn inc_npos_collector_assignment_for_idx(&self, idx: usize) {
        if self.enabled.load(Ordering::Relaxed) {
            let label = idx.to_string();
            self.metrics
                .sumeragi_npos_collector_assignments_by_idx
                .with_label_values(&[label.as_str()])
                .inc();
        }
    }

    /// Increment VRF non-reveal penalty counters by signer index
    pub fn inc_vrf_non_reveal_for_signer(&self, idx: usize) {
        if self.enabled.load(Ordering::Relaxed) {
            let label = idx.to_string();
            self.metrics
                .sumeragi_vrf_non_reveal_by_signer
                .with_label_values(&[label.as_str()])
                .inc();
        }
    }

    /// Increment total non-reveal penalties applied for an epoch
    pub fn inc_vrf_non_reveal_total(&self, count: u64, _epoch: u64) {
        if self.enabled.load(Ordering::Relaxed) {
            for _ in 0..count {
                self.metrics.sumeragi_vrf_non_reveal_penalties_total.inc();
            }
        }
    }

    /// Increment no-participation penalty counters by signer index
    pub fn inc_vrf_no_participation_for_signer(&self, idx: usize) {
        if self.enabled.load(Ordering::Relaxed) {
            let label = idx.to_string();
            self.metrics
                .sumeragi_vrf_no_participation_by_signer
                .with_label_values(&[label.as_str()])
                .inc();
        }
    }

    /// Increment total no-participation penalties applied for an epoch
    pub fn inc_vrf_no_participation_total(&self, count: u64, _epoch: u64) {
        if self.enabled.load(Ordering::Relaxed) {
            for _ in 0..count {
                self.metrics.sumeragi_vrf_no_participation_total.inc();
            }
        }
    }

    /// Increment counter when this node emits a VRF commit.
    pub fn inc_vrf_commit_emitted(&self) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.sumeragi_vrf_commits_emitted_total.inc();
        }
    }

    /// Increment counter when this node emits a VRF reveal.
    pub fn inc_vrf_reveal_emitted(&self) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.sumeragi_vrf_reveals_emitted_total.inc();
        }
    }

    /// Increment counter when this node accepts a late VRF reveal.
    pub fn inc_vrf_reveal_late(&self) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.sumeragi_vrf_reveals_late_total.inc();
        }
    }

    /// Increment counter: redundant vote send labeled by collector peer id
    pub fn inc_redundant_send_for_peer(&self, peer: &iroha_data_model::peer::PeerId) {
        if self.enabled.load(Ordering::Relaxed) {
            let label = peer.to_string();
            self.metrics
                .sumeragi_redundant_sends_by_peer
                .with_label_values(&[label.as_str()])
                .inc();
        }
    }

    /// Increment redundant send counter labeled by local validator index (role index)
    pub fn inc_redundant_send_for_role(&self, role_idx: usize) {
        if self.enabled.load(Ordering::Relaxed) {
            let label = role_idx.to_string();
            self.metrics
                .sumeragi_redundant_sends_by_role_idx
                .with_label_values(&[label.as_str()])
                .inc();
        }
    }

    /// Increment gossip fallback counter when redundant plan exhausts collectors.
    pub fn inc_gossip_fallback(&self) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.sumeragi_gossip_fallback_total.inc();
        }
    }

    /// Set gossip fallback counter (best-effort, used for status snapshot alignment).
    pub fn set_gossip_fallback_total(&self, total: u64) {
        if self.enabled.load(Ordering::Relaxed) {
            let current = self.metrics.sumeragi_gossip_fallback_total.get();
            if total > current {
                self.metrics
                    .sumeragi_gossip_fallback_total
                    .inc_by(total - current);
            }
        }
    }

    /// Increment counter when `BlockCreated` violates the locked QC gate.
    pub fn inc_block_created_dropped_by_lock(&self) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .sumeragi_block_created_dropped_by_lock_total
                .inc();
        }
    }

    /// Increment counter when `BlockCreated` fails hint validation.
    pub fn inc_block_created_hint_mismatch(&self) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .sumeragi_block_created_hint_mismatch_total
                .inc();
        }
    }

    /// Increment counter when `BlockCreated` fails proposal validation.
    pub fn inc_block_created_proposal_mismatch(&self) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .sumeragi_block_created_proposal_mismatch_total
                .inc();
        }
    }

    /// Set current collector parameters (`collectors_k`, `redundant_send_r`) as gauges
    pub fn set_collectors_params(&self, collectors_k: u64, redundant_send_r: u64) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.sumeragi_collectors_k.set(collectors_k);
            self.metrics.sumeragi_redundant_send_r.set(redundant_send_r);
        }
    }

    /// Record the active epoch scheduling parameters (length and commit/reveal offsets).
    pub fn set_epoch_parameters(&self, length_blocks: u64, commit_offset: u64, reveal_offset: u64) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.sumeragi_epoch_length_blocks.set(length_blocks);
            self.metrics
                .sumeragi_epoch_commit_deadline_offset
                .set(commit_offset);
            self.metrics
                .sumeragi_epoch_reveal_deadline_offset
                .set(reveal_offset);
        }
    }

    /// Record the current PRF context (epoch seed, height, view) if telemetry is enabled.
    pub fn set_prf_context(&self, seed: Option<[u8; 32]>, height: u64, view: u64) {
        if self.enabled.load(Ordering::Relaxed) {
            {
                let mut guard = self
                    .metrics
                    .sumeragi_prf_epoch_seed_hex
                    .write()
                    .expect("sumeragi PRF seed lock poisoned");
                *guard = seed.map(hex::encode);
            }
            self.metrics.sumeragi_prf_height.set(height);
            self.metrics.sumeragi_prf_view.set(view);
        }
    }

    /// Record the current/staged consensus mode tags for status/telemetry snapshots.
    pub fn set_mode_tags(
        &self,
        mode_tag: &str,
        staged_mode_tag: Option<&str>,
        staged_activation_height: Option<u64>,
    ) {
        if self.enabled.load(Ordering::Relaxed) {
            if let Ok(mut guard) = self.metrics.sumeragi_mode_tag.write() {
                *guard = mode_tag.to_string();
            }
            if let Ok(mut guard) = self.metrics.sumeragi_staged_mode_tag.write() {
                *guard = staged_mode_tag.map(ToOwned::to_owned);
            }
            if let Ok(mut guard) = self.metrics.sumeragi_staged_mode_activation_height.write() {
                *guard = staged_activation_height;
            }
        }
    }

    /// Record the observed lag (in blocks) since a staged mode activation height elapsed.
    pub fn set_mode_activation_lag(&self, lag_blocks: Option<u64>) {
        if self.enabled.load(Ordering::Relaxed) {
            if let Ok(mut guard) = self.metrics.sumeragi_mode_activation_lag_blocks_opt.write() {
                *guard = lag_blocks;
            }
            let lag_blocks_i64 = i64::try_from(lag_blocks.unwrap_or(0)).unwrap_or(i64::MAX);
            self.metrics
                .sumeragi_mode_activation_lag_blocks
                .set(lag_blocks_i64);
        }
    }

    /// Record whether runtime consensus mode flips are allowed by configuration.
    pub fn set_mode_flip_kill_switch(&self, enabled: bool) {
        if self.enabled.load(Ordering::Relaxed) {
            let value = i64::from(enabled);
            self.metrics.sumeragi_mode_flip_kill_switch.set(value);
        }
    }

    /// Increment the counter for blocked mode flip attempts (e.g., kill switch).
    pub fn inc_mode_flip_blocked(&self, mode_tag: &str, timestamp_ms: u64) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .sumeragi_mode_flip_blocked_total
                .with_label_values(&[mode_tag])
                .inc();
            let ts_i64 = i64::try_from(timestamp_ms).unwrap_or(i64::MAX);
            self.metrics
                .sumeragi_last_mode_flip_timestamp_ms
                .set(ts_i64);
        }
    }

    /// Increment the counter for failed mode flip attempts.
    pub fn inc_mode_flip_failure(&self, mode_tag: &str, timestamp_ms: u64) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .sumeragi_mode_flip_failure_total
                .with_label_values(&[mode_tag])
                .inc();
            let ts_i64 = i64::try_from(timestamp_ms).unwrap_or(i64::MAX);
            self.metrics
                .sumeragi_last_mode_flip_timestamp_ms
                .set(ts_i64);
        }
    }

    /// Increment the counter for successful mode flips.
    pub fn inc_mode_flip_success(&self, mode_tag: &str, timestamp_ms: u64) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .sumeragi_mode_flip_success_total
                .with_label_values(&[mode_tag])
                .inc();
            let ts_i64 = i64::try_from(timestamp_ms).unwrap_or(i64::MAX);
            self.metrics
                .sumeragi_last_mode_flip_timestamp_ms
                .set(ts_i64);
        }
    }

    /// Set number of collectors targeted for the current voting block.
    pub fn set_collectors_targeted_current(&self, targeted: u64) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .sumeragi_collectors_targeted_current
                .set(targeted);
        }
    }

    /// Observe histogram: collectors targeted per block
    pub fn observe_collectors_targeted_per_block(&self, targeted: u64) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .sumeragi_collectors_targeted_per_block
                .observe(u64_to_f64(targeted));
        }
    }

    /// Observe certificate size distribution (number of signatures)
    pub fn observe_cert_size(&self, size: u64) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.sumeragi_cert_size.observe(u64_to_f64(size));
        }
    }

    /// Record the latest commit-signature counts (present vs counted vs set-B vs required).
    pub fn set_commit_signature_totals(
        &self,
        present: u64,
        counted: u64,
        set_b_signatures: u64,
        required: u64,
    ) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics.sumeragi_commit_signatures_present.set(present);
            self.metrics.sumeragi_commit_signatures_counted.set(counted);
            self.metrics
                .sumeragi_commit_signatures_set_b
                .set(set_b_signatures);
            self.metrics
                .sumeragi_commit_signatures_required
                .set(required);
        }
    }

    /// Record the latest commit certificate summary (best-effort).
    pub fn set_commit_certificate_summary(&self, cert: &CommitCertificate) {
        if self.enabled.load(Ordering::Relaxed) {
            self.metrics
                .sumeragi_commit_certificate_height
                .set(cert.height);
            self.metrics.sumeragi_commit_certificate_view.set(cert.view);
            self.metrics
                .sumeragi_commit_certificate_epoch
                .set(cert.epoch);
            self.metrics
                .sumeragi_commit_certificate_signatures_total
                .set(u64::try_from(cert.signatures.len()).unwrap_or(u64::MAX));
            self.metrics
                .sumeragi_commit_certificate_validator_set_len
                .set(u64::try_from(cert.validator_set.len()).unwrap_or(u64::MAX));
        }
    }

    /// Report the event of block commit, measuring the block time.
    pub fn report_block_commit_blocking(&self, block_header: &BlockHeader) {
        let report = BlockCommitReport::new(block_header, &self.time_source);
        if self.enabled.load(Ordering::Relaxed) {
            let commit_time_u64 = u64::try_from(report.commit_time.as_millis()).unwrap_or(u64::MAX);
            let commit_time = u64_to_f64(commit_time_u64);
            self.metrics.commit_time_ms.observe(commit_time);
            self.metrics.slot_duration_ms.observe(commit_time);
            self.metrics.slot_duration_ms_latest.set(commit_time_u64);
            let slot_had_reschedule = self.da_slot_rescheduled.swap(false, Ordering::Relaxed);
            let total = self.da_slots_total.fetch_add(1, Ordering::Relaxed) + 1;
            if !slot_had_reschedule {
                self.da_slots_quorum_met.fetch_add(1, Ordering::Relaxed);
            }
            let successes = self.da_slots_quorum_met.load(Ordering::Relaxed);
            let ratio = if total == 0 {
                1.0
            } else {
                (u64_to_f64(successes) / u64_to_f64(total)).clamp(0.0, 1.0)
            };
            self.metrics.da_quorum_ratio.set(ratio);
        }

        // This function is called from within the main loop.
        // We absolutely don't want to block it.
        // However, there is only one reader (the actor loop),
        // and it acquires the lock for a very brief period of time;
        // thus it shouldn't be a problem.
        let mut lock = self.last_reported_block.blocking_write();
        *lock = Some(report);
    }

    /// Some metrics are updated lazily, on demand.
    /// This async function completes once data is up to date.
    #[cfg(feature = "telemetry")]
    pub async fn metrics(&self) -> &Metrics {
        let sync_result = async {
            let (tx, rx) = oneshot::channel();
            self.actor
                .try_send(Message::Sync { reply: tx })
                .map_err(|err| format!("schedule telemetry sync: {err}"))?;
            tokio::time::timeout(METRICS_SYNC_TIMEOUT, rx)
                .await
                .map_err(|_| "telemetry sync timed out")?
                .map_err(|_| "telemetry actor closed")?;
            Ok::<(), String>(())
        }
        .await;

        if let Err(err) = sync_result {
            iroha_logger::warn!(
                ?err,
                timeout_ms = METRICS_SYNC_TIMEOUT.as_millis(),
                "telemetry sync failed; returning last metrics snapshot"
            );
        }
        &self.metrics
    }

    /// Access the `SoraNet` privacy aggregator.
    pub fn soranet_privacy(&self) -> Arc<SoranetSecureAggregator> {
        Arc::clone(&self.soranet_privacy)
    }

    /// Record a `SoraNet` privacy telemetry event and publish ready buckets.
    pub fn record_soranet_privacy_event(&self, event: &SoranetPrivacyEventV1) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        self.soranet_privacy.record_event(event);
        self.publish_ready_soranet_privacy_buckets();
    }

    /// Ingest a `SoraNet` privacy Prio share emitted by a collector.
    ///
    /// # Errors
    /// Returns [`PrivacyShareError`] when the share fails validation or aggregation.
    pub fn ingest_soranet_privacy_share(
        &self,
        share: SoranetPrivacyPrioShareV1,
    ) -> Result<(), PrivacyShareError> {
        if !self.enabled.load(Ordering::Relaxed) {
            return Ok(());
        }
        self.soranet_privacy.ingest_prio_share(share)?;
        self.publish_ready_soranet_privacy_buckets();
        Ok(())
    }

    fn publish_ready_soranet_privacy_buckets(&self) {
        let (buckets, snapshot) = self.soranet_privacy.drain_ready_now_with_snapshot();
        self.metrics
            .record_soranet_privacy_queue_snapshot(&snapshot);
        for bucket in buckets {
            self.metrics.record_soranet_privacy_bucket(&bucket);
        }
    }
}

/// Record the shard cursor lag measured in blocks for a given telemetry handle.
pub fn record_da_shard_cursor_lag(
    telemetry: &StateTelemetry,
    lane_id: u32,
    shard_id: u32,
    lag_blocks: i64,
) {
    if telemetry.enabled.load(Ordering::Relaxed) {
        telemetry
            .metrics
            .set_da_shard_cursor_lag(lane_id, shard_id, lag_blocks);
    }
}

impl From<StateTelemetry> for Telemetry {
    fn from(st: StateTelemetry) -> Self {
        let (actor, _handle) = mpsc::channel(CHANNEL_CAPACITY);
        Telemetry {
            actor,
            last_reported_block: Arc::new(RwLock::new(None)),
            metrics: st.metrics.clone(),
            enabled: st.enabled.clone(),
            nexus_enabled: st.nexus_enabled.clone(),
            time_source: TimeSource::new_system(),
            soranet_privacy: st.soranet_privacy(),
            micropayment_samples: Self::default_micropayment_samples(),
            da_slot_rescheduled: Arc::new(AtomicBool::new(false)),
            da_slots_total: Arc::new(AtomicU64::new(0)),
            da_slots_quorum_met: Arc::new(AtomicU64::new(0)),
        }
    }
}

struct Actor {
    handle: mpsc::Receiver<Message>,
    last_reported_block: Arc<RwLock<Option<BlockCommitReport>>>,
    last_sync_block: usize,
    last_online_peers: BTreeSet<PeerId>,
    online_peers: watch::Receiver<OnlinePeers>,
    metrics: Arc<Metrics>,
    state: Arc<State>,
    kura: Arc<Kura>,
    queue: Arc<Queue>,
    enabled: Arc<AtomicBool>,
    time_source: TimeSource,
}

impl Actor {
    async fn run(mut self) {
        #[cfg(feature = "zk-preverify")]
        crate::zk::start_lane();
        while let Some(message) = self.handle.recv().await {
            match message {
                Message::Sync { reply } => {
                    self.sync().await;
                    let _ = reply.send(());
                }
            }
        }
    }

    fn seed_last_reported_block(&self) -> Option<BlockCommitReport> {
        let next_height = self.last_sync_block.checked_add(1)?;
        let index = NonZeroUsize::new(next_height)?;
        let block = self.kura.get_block(index)?;
        let header = block.header();
        Some(BlockCommitReport::new(&header, &self.time_source))
    }

    #[allow(clippy::too_many_lines)]
    async fn sync(&mut self) {
        // Master switch: when telemetry is disabled, skip recording/updating all metrics.
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        let mut peer_count;
        let mut current_online;
        {
            let peer_snapshot = self.online_peers.borrow();
            peer_count = peer_snapshot.len() as u64;
            current_online = peer_snapshot
                .iter()
                .map(|peer| peer.id().clone())
                .collect::<BTreeSet<_>>();
        }
        if crate::sumeragi::status::local_peer_removed() {
            peer_count = 0;
            current_online.clear();
        }

        let mut connected_delta = 0u64;
        for peer_id in &current_online {
            if !self.last_online_peers.contains(peer_id) {
                connected_delta += 1;
            }
        }
        let mut disconnected_delta = 0u64;
        for peer_id in &self.last_online_peers {
            if !current_online.contains(peer_id) {
                disconnected_delta += 1;
            }
        }
        if connected_delta > 0 {
            self.metrics
                .p2p_peer_churn_total
                .with_label_values(&["connected"])
                .inc_by(connected_delta);
        }
        if disconnected_delta > 0 {
            self.metrics
                .p2p_peer_churn_total
                .with_label_values(&["disconnected"])
                .inc_by(disconnected_delta);
        }
        self.last_online_peers = current_online;
        self.metrics.connected_peers.set(peer_count);
        self.metrics.queue_size.set(self.queue.tx_len() as u64);
        // P2P counters (gauges): sample from p2p module
        self.metrics
            .p2p_dropped_posts
            .set(iroha_p2p::network::dropped_post_count());
        self.metrics
            .p2p_dropped_broadcasts
            .set(iroha_p2p::network::dropped_broadcast_count());
        self.metrics
            .p2p_handshake_failures
            .set(iroha_p2p::peer::handshake_failure_count());
        self.metrics
            .p2p_low_post_throttled_total
            .set(iroha_p2p::network::low_post_throttled_count());
        self.metrics
            .p2p_low_broadcast_throttled_total
            .set(iroha_p2p::network::low_broadcast_throttled_count());
        self.metrics
            .p2p_post_overflow_total
            .set(iroha_p2p::network::post_overflow_count());
        // Per-topic breakdown
        // High/Low by topic
        let by = &self.metrics.p2p_post_overflow_by_topic;
        by.with_label_values(&["High", "Consensus"])
            .set(iroha_p2p::network::post_overflow_consensus_high_count());
        by.with_label_values(&["High", "Control"])
            .set(iroha_p2p::network::post_overflow_control_high_count());
        by.with_label_values(&["High", "BlockSync"])
            .set(iroha_p2p::network::post_overflow_block_sync_high_count());
        by.with_label_values(&["High", "TxGossip"])
            .set(iroha_p2p::network::post_overflow_tx_gossip_high_count());
        by.with_label_values(&["High", "PeerGossip"])
            .set(iroha_p2p::network::post_overflow_peer_gossip_high_count());
        by.with_label_values(&["High", "Health"])
            .set(iroha_p2p::network::post_overflow_health_high_count());
        by.with_label_values(&["High", "Other"])
            .set(iroha_p2p::network::post_overflow_other_high_count());
        by.with_label_values(&["Low", "Consensus"])
            .set(iroha_p2p::network::post_overflow_consensus_low_count());
        by.with_label_values(&["Low", "Control"])
            .set(iroha_p2p::network::post_overflow_control_low_count());
        by.with_label_values(&["Low", "BlockSync"])
            .set(iroha_p2p::network::post_overflow_block_sync_low_count());
        by.with_label_values(&["Low", "TxGossip"])
            .set(iroha_p2p::network::post_overflow_tx_gossip_low_count());
        by.with_label_values(&["Low", "PeerGossip"])
            .set(iroha_p2p::network::post_overflow_peer_gossip_low_count());
        by.with_label_values(&["Low", "Health"])
            .set(iroha_p2p::network::post_overflow_health_low_count());
        by.with_label_values(&["Low", "Other"])
            .set(iroha_p2p::network::post_overflow_other_low_count());
        // Network Time Service (basic gauges)
        let nts = crate::time::now();
        self.metrics.nts_offset_ms.set(nts.offset_ms);
        self.metrics.nts_confidence_ms.set(nts.confidence_ms);
        self.metrics.nts_peers_sampled.set(nts.peer_count as u64);
        self.metrics.nts_samples_used.set(nts.sample_count as u64);
        self.metrics.nts_healthy.set(i64::from(nts.health.healthy));
        self.metrics.nts_fallback.set(i64::from(nts.fallback));
        self.metrics
            .nts_min_samples_ok
            .set(i64::from(nts.health.min_samples_ok));
        self.metrics
            .nts_offset_ok
            .set(i64::from(nts.health.offset_ok));
        self.metrics
            .nts_confidence_ok
            .set(i64::from(nts.health.confidence_ok));
        // NTS RTT histogram (export counters)
        let bounds = crate::time::rtt_bucket_bounds_ms();
        let counts = crate::time::rtt_bucket_counts();
        for (le, cnt) in bounds.iter().zip(counts.iter()) {
            self.metrics
                .nts_rtt_ms_bucket
                .with_label_values(&[&le.to_string()])
                .set(*cnt);
        }
        self.metrics.nts_rtt_ms_sum.set(crate::time::rtt_ms_sum());
        self.metrics
            .nts_rtt_ms_count
            .set(crate::time::rtt_ms_count());
        self.metrics
            .p2p_dns_refresh_total
            .set(iroha_p2p::network::dns_refresh_count());
        self.metrics
            .p2p_dns_ttl_refresh_total
            .set(iroha_p2p::network::dns_ttl_refresh_count());
        self.metrics
            .p2p_dns_resolution_fail_total
            .set(iroha_p2p::network::dns_resolution_fail_count());
        self.metrics
            .p2p_dns_reconnect_success_total
            .set(iroha_p2p::network::dns_reconnect_success_count());
        self.metrics
            .p2p_backoff_scheduled_total
            .set(iroha_p2p::network::backoff_scheduled_count());
        self.metrics
            .p2p_accept_throttled_total
            .set(iroha_p2p::network::accept_throttled_count());
        self.metrics
            .p2p_accept_bucket_evictions_total
            .set(iroha_p2p::network::accept_bucket_evictions_count());
        self.metrics
            .p2p_accept_buckets_current
            .set(iroha_p2p::network::accept_bucket_count());
        let prefix_cache = &self.metrics.p2p_accept_prefix_cache_total;
        prefix_cache
            .with_label_values(&["hit"])
            .set(iroha_p2p::network::accept_prefix_hits_count());
        prefix_cache
            .with_label_values(&["miss"])
            .set(iroha_p2p::network::accept_prefix_misses_count());
        let throttle = &self.metrics.p2p_accept_throttle_decisions_total;
        throttle
            .with_label_values(&["prefix", "allowed"])
            .set(iroha_p2p::network::accept_prefix_allowed_count());
        throttle
            .with_label_values(&["prefix", "throttled"])
            .set(iroha_p2p::network::accept_prefix_throttled_count());
        throttle
            .with_label_values(&["ip", "allowed"])
            .set(iroha_p2p::network::accept_ip_allowed_count());
        throttle
            .with_label_values(&["ip", "throttled"])
            .set(iroha_p2p::network::accept_ip_throttled_count());
        self.metrics
            .p2p_incoming_cap_reject_total
            .set(iroha_p2p::network::incoming_cap_reject_count());
        self.metrics
            .p2p_total_cap_reject_total
            .set(iroha_p2p::network::total_cap_reject_count());
        self.metrics
            .p2p_ws_inbound_total
            .set(iroha_p2p::network::ws_inbound_total());
        self.metrics
            .p2p_ws_outbound_total
            .set(iroha_p2p::network::ws_outbound_total());
        // High/Low bounded-queue drops (network actor queues)
        let drops = &self.metrics.p2p_queue_dropped_total;
        drops
            .with_label_values(&["High", "Post"])
            .set(iroha_p2p::network::dropped_post_high_count());
        drops
            .with_label_values(&["Low", "Post"])
            .set(iroha_p2p::network::dropped_post_low_count());
        drops
            .with_label_values(&["High", "Broadcast"])
            .set(iroha_p2p::network::dropped_broadcast_high_count());
        drops
            .with_label_values(&["Low", "Broadcast"])
            .set(iroha_p2p::network::dropped_broadcast_low_count());
        // Handshake latency histogram (buckets)
        let bounds = iroha_p2p::peer::handshake_bucket_bounds_ms();
        let counts = iroha_p2p::peer::handshake_bucket_counts();
        for (le, cnt) in bounds.iter().zip(counts.iter()) {
            self.metrics
                .p2p_handshake_ms_bucket
                .with_label_values(&[&le.to_string()])
                .set(*cnt);
        }
        self.metrics
            .p2p_handshake_ms_sum
            .set(iroha_p2p::peer::handshake_ms_sum());
        self.metrics
            .p2p_handshake_ms_count
            .set(iroha_p2p::peer::handshake_ms_count());
        // Handshake error taxonomy
        let herr = &self.metrics.p2p_handshake_error_total;
        herr.with_label_values(&["timeout"])
            .set(iroha_p2p::peer::handshake_error_timeout());
        herr.with_label_values(&["preface"])
            .set(iroha_p2p::peer::handshake_error_preface());
        herr.with_label_values(&["verify"])
            .set(iroha_p2p::peer::handshake_error_verify());
        herr.with_label_values(&["decrypt"])
            .set(iroha_p2p::peer::handshake_error_decrypt());
        herr.with_label_values(&["codec"])
            .set(iroha_p2p::peer::handshake_error_codec());
        herr.with_label_values(&["io"])
            .set(iroha_p2p::peer::handshake_error_io());
        herr.with_label_values(&["other"])
            .set(iroha_p2p::peer::handshake_error_other());
        // Topic frame cap violations
        let caps = &self.metrics.p2p_frame_cap_violations_total;
        caps.with_label_values(&["Consensus"])
            .set(iroha_p2p::network::cap_violations_consensus());
        caps.with_label_values(&["Control"])
            .set(iroha_p2p::network::cap_violations_control());
        caps.with_label_values(&["BlockSync"])
            .set(iroha_p2p::network::cap_violations_block_sync());
        caps.with_label_values(&["TxGossip"])
            .set(iroha_p2p::network::cap_violations_tx_gossip());
        caps.with_label_values(&["PeerGossip"])
            .set(iroha_p2p::network::cap_violations_peer_gossip());
        caps.with_label_values(&["Health"])
            .set(iroha_p2p::network::cap_violations_health());
        caps.with_label_values(&["Other"])
            .set(iroha_p2p::network::cap_violations_other());
        // Optional: reconnect successes (uncomment if exposed in metrics)
        // self.metrics
        //     .p2p_dns_reconnect_success_total
        //     .set(iroha_p2p::network::dns_reconnect_success_count());

        let last_reported_block = {
            let mut lock = self.last_reported_block.write().await;
            if lock.is_none() {
                *lock = self.seed_last_reported_block();
            }
            if let Some(mut latest) = lock.take() {
                // Catch up if commit notifications were missed: walk forward until the
                // latest persisted block and refresh the cursor.
                loop {
                    let Some(next_index) = latest.height.checked_add(1).and_then(NonZeroUsize::new)
                    else {
                        break;
                    };
                    let Some(next_block) = self.kura.get_block(next_index) else {
                        break;
                    };
                    let header = next_block.header();
                    latest = BlockCommitReport::new(&header, &self.time_source);
                }
                *lock = Some(latest);
            }
            let Some(value) = *lock else {
                // wait until genesis
                return;
            };
            value
        };

        let state_view = self.state.view();

        let start_index = self.last_sync_block;
        {
            let mut inc_txs_accepted = 0;
            let mut inc_txs_rejected = 0;
            let mut inc_blocks = 0;
            let mut inc_blocks_non_empty = 0;

            let mut block_index = start_index;
            while block_index < last_reported_block.height {
                let Some(block) = NonZeroUsize::new(
                    block_index
                        .checked_add(1)
                        .expect("INTERNAL BUG: Blockchain height exceeds usize::MAX"),
                )
                .and_then(|index| self.kura.get_block(index)) else {
                    break;
                };
                block_index += 1;

                let block_txs_rejected = block.errors().count() as u64;
                let block_txs_all = block.external_transactions().count() as u64;
                let block_txs_approved = block_txs_all.saturating_sub(block_txs_rejected);

                inc_blocks += 1;
                inc_txs_accepted += block_txs_approved;
                inc_txs_rejected += block_txs_rejected;
                if block_counts_as_non_empty(block.as_ref()) {
                    inc_blocks_non_empty += 1;
                }

                if block_index == last_reported_block.height {
                    // for some reason, using `debug_assert!(..)` doesn't work here
                    // in release build Rust complains about the absent `.hash` field (feature gated via
                    // `debug_assertions`), which doesn't make sense - the whole statement should be feature-gated too
                    #[cfg(debug_assertions)]
                    assert_eq!(
                        block.hash(),
                        last_reported_block.hash,
                        "BUG: Reported block hash is different (reported {}, actual {})",
                        last_reported_block.hash,
                        block.hash()
                    );
                    #[allow(clippy::cast_precision_loss)]
                    self.metrics.last_commit_time_ms.set(
                        u64::try_from(last_reported_block.commit_time.as_millis())
                            .expect("time should fit into u64"),
                    );
                    #[cfg(feature = "zk-preverify")]
                    {
                        // Enqueue the latest block for background proving
                        crate::zk::enqueue_block_for_proving(&block.header());
                    }
                }
            }
            self.last_sync_block = block_index;

            self.metrics
                .txs
                .with_label_values(&["accepted"])
                .inc_by(inc_txs_accepted);
            self.metrics
                .txs
                .with_label_values(&["rejected"])
                .inc_by(inc_txs_rejected);
            self.metrics
                .txs
                .with_label_values(&["total"])
                .inc_by(inc_txs_accepted + inc_txs_rejected);
            self.metrics.block_height.inc_by(inc_blocks);
            self.metrics
                .block_height_non_empty
                .inc_by(inc_blocks_non_empty);
        }

        #[allow(clippy::cast_possible_truncation)]
        if let Some(timestamp) = state_view.genesis_timestamp() {
            let curr_time = self.time_source.get_unix_time();

            // this will overflow in 584,942,417 years
            self.metrics.uptime_since_genesis_ms.set(
                (curr_time - timestamp)
                    .as_millis()
                    .try_into()
                    .expect("Timestamp should fit into u64"),
            )
        }

        // Below metrics could be out of sync with the "latest block" metric,
        // since state_view might be potentially ahead of the last reported block.
        // This is fine because this time window _should_ be very narrow.

        self.metrics
            .domains
            .set(state_view.world().domains().len() as u64);
        for domain in state_view.world().domains_iter() {
            match self
                .metrics
                .accounts
                .get_metric_with_label_values(&[domain.id().name().as_ref()])
            {
                Err(err) => {
                    #[cfg(debug_assertions)]
                    panic!("BUG: Failed to compose domains: {err}");
                    #[cfg(not(debug_assertions))]
                    iroha_logger::error!(?err, "Failed to compose domains")
                }
                Ok(metrics) => {
                    metrics.set(
                        state_view
                            .world()
                            .accounts_in_domain_iter(domain.id())
                            .count() as u64,
                    );
                }
            }
        }

        // Runtime: update active ABI versions count from current state view
        let active_abi_versions_count = state_view.world().active_abi_versions().len() as u64;
        self.metrics
            .runtime_active_abi_versions_count
            .set(active_abi_versions_count);

        // Update IVM pre-decode cache counters from global ivm cache
        let stats = ivm::ivm_cache::global_stats();
        self.metrics.ivm_cache_hits.set(stats.hits);
        self.metrics.ivm_cache_misses.set(stats.misses);
        self.metrics.ivm_cache_evictions.set(stats.evictions);
        self.metrics
            .ivm_cache_decoded_streams
            .set(stats.decoded_streams);
        self.metrics
            .ivm_cache_decoded_ops_total
            .set(stats.decoded_ops_total);
        self.metrics
            .ivm_cache_decode_failures
            .set(stats.decode_failures);
        self.metrics
            .ivm_cache_decode_time_ns_total
            .set(stats.decode_time_ns_total);
    }
}

fn block_counts_as_non_empty(block: &iroha_data_model::block::SignedBlock) -> bool {
    if block.entrypoint_hashes().count() != 0 {
        return true;
    }
    block.header().is_genesis()
}

#[derive(Copy, Clone, Debug)]
struct BlockCommitReport {
    /// Only in debug, to ensure consistency
    #[cfg(debug_assertions)]
    hash: HashOf<BlockHeader>,
    height: usize,
    commit_time: Duration,
}

impl BlockCommitReport {
    fn new(block_header: &BlockHeader, time_source: &TimeSource) -> Self {
        let commit_time = if block_header.is_genesis() {
            Duration::ZERO
        } else {
            let now = time_source.get_unix_time();
            let created_at = block_header.creation_time();
            debug_assert!(now >= created_at);
            now - created_at
        };
        Self {
            #[cfg(debug_assertions)]
            hash: block_header.hash(),
            height: usize::try_from(block_header.height().get())
                .expect("block height should fit into usize"),
            commit_time,
        }
    }
}

/// Start the telemetry service
pub fn start(
    metrics: Arc<Metrics>,
    state: Arc<State>,
    kura: Arc<Kura>,
    queue: Arc<Queue>,
    online_peers: watch::Receiver<OnlinePeers>,
    time_source: TimeSource,
    enabled: bool,
) -> (Telemetry, Child) {
    let (actor, handle) = mpsc::channel(CHANNEL_CAPACITY);
    let last_reported_block = Arc::new(RwLock::new(None));
    let enabled_arc = Arc::new(AtomicBool::new(enabled));
    let soranet_privacy = Arc::new(
        SoranetSecureAggregator::new(PrivacyBucketConfig::default())
            .expect("valid default SoraNet privacy config"),
    );
    (
        Telemetry {
            actor,
            last_reported_block: last_reported_block.clone(),
            metrics: metrics.clone(),
            enabled: enabled_arc.clone(),
            nexus_enabled: Arc::new(AtomicBool::new(true)),
            time_source: time_source.clone(),
            soranet_privacy: Arc::clone(&soranet_privacy),
            micropayment_samples: Telemetry::default_micropayment_samples(),
            da_slot_rescheduled: Arc::new(AtomicBool::new(false)),
            da_slots_total: Arc::new(AtomicU64::new(0)),
            da_slots_quorum_met: Arc::new(AtomicU64::new(0)),
        },
        Child::new(
            tokio::spawn(
                Actor {
                    handle,
                    metrics,
                    state,
                    kura,
                    queue,
                    last_sync_block: 0,
                    last_online_peers: BTreeSet::new(),
                    last_reported_block,
                    online_peers,
                    enabled: enabled_arc,
                    time_source,
                }
                .run(),
            ),
            OnShutdown::Abort,
        ),
    )
}

#[cfg(all(feature = "telemetry", test))]
#[allow(clippy::disallowed_types, clippy::float_cmp)]
mod tests {
    use std::{
        collections::{BTreeMap, HashSet},
        path::PathBuf,
        sync::Arc,
        time::Duration,
    };

    use iroha_config::parameters::actual::{
        ConfidentialGas as ActualConfidentialGas, ConsensusMode,
    };
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, PrivateKey};
    #[cfg(feature = "telemetry")]
    use iroha_data_model::events::data::social::{
        SocialEvent, ViralEscrowCancelled, ViralEscrowCreated,
    };
    #[cfg(feature = "telemetry")]
    use iroha_data_model::social::ViralEscrowRecord;
    use iroha_data_model::{
        ChainId, Level,
        account::AccountId,
        asset::AssetDefinitionId,
        events::data::space_directory::{SpaceDirectoryEvent, SpaceDirectoryManifestRevoked},
        isi::Log,
        nexus::{
            AssetPermissionManifest, AxtPolicyBinding, AxtPolicyEntry, AxtPolicySnapshot,
            DataSpaceId, DataSpaceMetadata, LaneConfig, LaneId, ManifestVersion,
            UniversalAccountId,
        },
        oracle::KeyedHash,
        peer::{Peer, PeerId},
        prelude::TransactionBuilder,
    };
    #[cfg(feature = "telemetry")]
    use iroha_data_model::{
        events::data::sorafs::SorafsProofHealthAlert, sorafs::capacity::ProviderId,
    };
    use iroha_primitives::{
        addr::{SocketAddr, socket_addr},
        time::{MockTimeHandle, TimeSource},
    };
    use iroha_telemetry::metrics::Collector;
    use iroha_test_samples::gen_account_in;
    use nonzero_ext::nonzero;
    #[cfg(feature = "telemetry")]
    use norito::streaming::{
        SyncDiagnostics, TelemetryAuditOutcome, TelemetryDecodeStats, TelemetryEncodeStats,
        TelemetryEnergyStats, TelemetryNetworkStats,
    };
    use tokio::task::spawn_blocking;

    #[cfg(feature = "telemetry")]
    use super::StreamingTelemetry;
    use super::*;
    use crate::{
        block::{BlockBuilder, CommittedBlock, NewBlock},
        governance::manifest::{GovernanceRules, LaneManifestRegistry, LaneManifestStatus},
        nexus::space_directory::{SpaceDirectoryManifestRecord, SpaceDirectoryManifestSet},
        pipeline::access::AccessSetSource,
        prelude::World,
        query::store::LiveQueryStore,
        sumeragi::{
            consensus,
            message::{BlockMessage, PrecommitQCMsg, PrevoteVoteMsg},
            network_topology::Topology,
            status,
        },
        tx::AcceptedTransaction,
    };

    #[tokio::test]
    async fn metrics_returns_when_actor_not_running() {
        let telemetry = Telemetry::new(Arc::new(Metrics::default()), true);
        tokio::time::timeout(Duration::from_millis(100), telemetry.metrics())
            .await
            .expect("metrics() should not hang when the actor channel is closed");
    }

    #[test]
    fn commit_signature_totals_metrics_updated() {
        let metrics = Arc::new(iroha_telemetry::metrics::Metrics::default());
        let telemetry = Telemetry::new(metrics.clone(), true);

        telemetry.set_commit_signature_totals(5, 3, 2, 4);

        assert_eq!(metrics.sumeragi_commit_signatures_present.get(), 5);
        assert_eq!(metrics.sumeragi_commit_signatures_counted.get(), 3);
        assert_eq!(metrics.sumeragi_commit_signatures_set_b.get(), 2);
        assert_eq!(metrics.sumeragi_commit_signatures_required.get(), 4);
    }

    #[test]
    fn commit_certificate_summary_metrics_updated() {
        let metrics = Arc::new(iroha_telemetry::metrics::Metrics::default());
        let telemetry = Telemetry::new(metrics.clone(), true);

        let peer_a = PeerId::new(KeyPair::random().public_key().clone());
        let peer_b = PeerId::new(KeyPair::random().public_key().clone());
        let validator_set = vec![peer_a, peer_b];
        let validator_set_hash = HashOf::new(&validator_set);
        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xAB; 32]));
        let cert = CommitCertificate {
            height: 42,
            block_hash,
            view: 7,
            epoch: 1,
            validator_set_hash,
            validator_set_hash_version: iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            validator_set: validator_set.clone(),
            signatures: Vec::new(),
        };

        telemetry.set_commit_certificate_summary(&cert);

        assert_eq!(metrics.sumeragi_commit_certificate_height.get(), 42);
        assert_eq!(metrics.sumeragi_commit_certificate_view.get(), 7);
        assert_eq!(metrics.sumeragi_commit_certificate_epoch.get(), 1);
        assert_eq!(
            metrics.sumeragi_commit_certificate_signatures_total.get(),
            0
        );
        assert_eq!(
            metrics.sumeragi_commit_certificate_validator_set_len.get(),
            2
        );
    }

    #[test]
    fn isi_metrics_record_when_enabled() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(metrics.clone(), true);

        telemetry.record_isi_total("register_domain");
        telemetry.record_isi_success("register_domain");
        telemetry.record_isi_time("register_domain", Duration::from_millis(42));

        assert_eq!(
            metrics
                .isi
                .with_label_values(&["register_domain", "total"])
                .get(),
            1
        );
        assert_eq!(
            metrics
                .isi
                .with_label_values(&["register_domain", "success"])
                .get(),
            1
        );
        let histogram = metrics.isi_times.with_label_values(&["register_domain"]);
        assert_eq!(histogram.get_sample_count(), 1);
        assert_eq!(histogram.get_sample_sum(), 42.0);
    }

    #[test]
    fn isi_metrics_skip_when_disabled() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(metrics.clone(), false);

        telemetry.record_isi_total("register_domain");
        telemetry.record_isi_success("register_domain");
        telemetry.record_isi_time("register_domain", Duration::from_millis(42));

        assert_eq!(
            metrics
                .isi
                .with_label_values(&["register_domain", "total"])
                .get(),
            0
        );
        assert_eq!(
            metrics
                .isi
                .with_label_values(&["register_domain", "success"])
                .get(),
            0
        );
        let histogram = metrics.isi_times.with_label_values(&["register_domain"]);
        assert_eq!(histogram.get_sample_count(), 0);
    }

    #[test]
    fn block_sync_roster_source_metric_increments() {
        let metrics = Arc::new(iroha_telemetry::metrics::Metrics::default());
        let telemetry = Telemetry::new(metrics.clone(), true);

        telemetry.note_block_sync_roster_source("commit_roster_journal");
        telemetry.note_block_sync_roster_source("commit_roster_journal");

        assert_eq!(
            metrics
                .sumeragi_block_sync_roster_source_total
                .with_label_values(&["commit_roster_journal"])
                .get(),
            2
        );
    }

    #[test]
    fn view_change_cause_metric_increments_for_validation_reject() {
        let metrics = Arc::new(iroha_telemetry::metrics::Metrics::default());
        let telemetry = Telemetry::new(metrics.clone(), true);

        telemetry.note_view_change_cause("validation_reject");
        telemetry.note_view_change_cause("validation_reject");

        assert_eq!(
            metrics
                .sumeragi_view_change_cause_total
                .with_label_values(&["validation_reject"])
                .get(),
            2
        );
        let ts = metrics
            .sumeragi_view_change_cause_last_timestamp_ms
            .with_label_values(&["validation_reject"])
            .get();
        assert!(
            ts > 0,
            "view-change cause gauge should record a timestamp for validation_reject"
        );
    }

    #[test]
    fn block_sync_roster_drop_metric_increments() {
        let metrics = Arc::new(iroha_telemetry::metrics::Metrics::default());
        let telemetry = Telemetry::new(metrics.clone(), true);

        telemetry.note_block_sync_roster_drop("missing");

        assert_eq!(
            metrics
                .sumeragi_block_sync_roster_drop_total
                .with_label_values(&["missing"])
                .get(),
            1
        );
    }

    #[test]
    fn settlement_conversion_metrics_update_when_enabled() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(metrics.clone(), true);
        telemetry.inc_settlement_conversion_total("lane-1", "ds-9", "xor#sora", 3);
        telemetry.inc_settlement_haircut_total("lane-1", "ds-9", 2_000_000);

        let conversions = metrics
            .settlement_conversion_total
            .with_label_values(&["lane-1", "ds-9", "xor#sora"])
            .get();
        assert_eq!(conversions, 3);

        let haircut = metrics
            .settlement_haircut_total
            .with_label_values(&["lane-1", "ds-9"])
            .get();
        assert!(
            (haircut - (2_000_000_f64 / 1_000_000.0)).abs() < f64::EPSILON,
            "haircut counter records XOR units"
        );
    }

    #[cfg(feature = "telemetry")]
    #[allow(clippy::too_many_lines)]
    #[test]
    fn tx_gossip_caps_and_status_cached() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(metrics.clone(), true);

        telemetry.record_tx_gossip_caps(
            2_048,
            Some(3),
            None,
            true,
            DataspaceGossipFallback::UsePublicOverlay,
            RestrictedPublicPayload::Refuse,
            std::time::Duration::from_secs(2),
            std::time::Duration::from_secs(5),
        );

        assert_eq!(metrics.tx_gossip_frame_cap_bytes.get(), 2_048);
        assert_eq!(metrics.tx_gossip_public_target_cap.get(), 3);
        assert_eq!(metrics.tx_gossip_restricted_target_cap.get(), 0);
        assert_eq!(metrics.tx_gossip_public_target_reshuffle_ms.get(), 2_000);
        assert_eq!(
            metrics.tx_gossip_restricted_target_reshuffle_ms.get(),
            5_000
        );
        assert_eq!(metrics.tx_gossip_restricted_public_policy.get(), 0);
        assert_eq!(metrics.tx_gossip_drop_unknown_dataspace.get(), 1);
        assert_eq!(metrics.tx_gossip_restricted_fallback.get(), 1);
        let caps = metrics
            .tx_gossip_caps
            .read()
            .expect("tx gossip caps cache poisoned")
            .clone();
        assert_eq!(caps.public_target_reshuffle_ms, Some(2_000));
        assert_eq!(caps.restricted_target_reshuffle_ms, Some(5_000));
        assert_eq!(caps.restricted_public_policy, "refuse");
        assert_eq!(caps.restricted_fallback, "public_overlay");
        assert!(caps.drop_unknown_dataspace);

        let dataspace = DataSpaceId::new(7);
        let lane = LaneId::new(2);
        let peer_id = PeerId::new(KeyPair::random().public_key().clone());
        let targets = vec![peer_id];
        telemetry.record_tx_gossip_attempt(
            GossipPlane::Restricted,
            dataspace,
            &[lane],
            &targets,
            Some(nonzero!(3usize)),
            true,
            Some("restricted_public_overlay_forward"),
            true,
            Some("public_overlay"),
            2,
            128,
        );

        let sent = metrics
            .tx_gossip_sent_total
            .with_label_values(&["restricted", "7"])
            .get();
        assert_eq!(sent, 1);
        let targets = metrics
            .tx_gossip_targets
            .with_label_values(&["restricted", "7"])
            .get();
        assert_eq!(targets, 1);
        let fallback_forward = metrics
            .tx_gossip_fallback_total
            .with_label_values(&["restricted", "7", "public_overlay"])
            .get();
        assert_eq!(fallback_forward, 1);

        let status = metrics
            .tx_gossip_status
            .read()
            .expect("tx gossip status cache poisoned")
            .clone();
        assert_eq!(status.len(), 1);
        let entry = &status[0];
        assert_eq!(entry.dataspace_id, 7);
        assert_eq!(entry.lane_ids, vec![lane.as_u32()]);
        assert_eq!(entry.targets, 1);
        assert!(entry.fallback_used);
        assert_eq!(entry.target_cap, 3);
        assert_eq!(entry.batch_txs, 2);
        assert_eq!(entry.frame_bytes, 128);
        assert_eq!(entry.outcome, "sent");
        assert_eq!(
            entry.reason.as_deref(),
            Some("restricted_public_overlay_forward")
        );
        assert_eq!(entry.fallback_surface.as_deref(), Some("public_overlay"));
        assert_eq!(entry.target_peers.len(), 1);

        telemetry.record_tx_gossip_attempt(
            GossipPlane::Restricted,
            dataspace,
            &[lane],
            &[],
            Some(nonzero!(3usize)),
            false,
            Some("no_restricted_targets"),
            true,
            Some("public_overlay"),
            0,
            0,
        );
        let dropped = metrics
            .tx_gossip_dropped_total
            .with_label_values(&["restricted", "7", "no_restricted_targets"])
            .get();
        assert_eq!(dropped, 1);
        let targets_after = metrics
            .tx_gossip_targets
            .with_label_values(&["restricted", "7"])
            .get();
        assert_eq!(targets_after, 0);
        let fallback_drop = metrics
            .tx_gossip_fallback_total
            .with_label_values(&["restricted", "7", "public_overlay"])
            .get();
        assert_eq!(fallback_drop, 2);
        let status = metrics
            .tx_gossip_status
            .read()
            .expect("tx gossip status cache poisoned")
            .clone();
        assert_eq!(status.len(), 1);
        assert_eq!(status[0].reason.as_deref(), Some("no_restricted_targets"));
        assert!(status[0].fallback_used);
        assert_eq!(status[0].outcome, "dropped");
    }

    #[cfg(feature = "telemetry")]
    #[allow(clippy::too_many_lines)]
    #[tokio::test]
    async fn status_exposes_tx_gossip_targets_with_aliases() {
        use iroha_data_model::{
            nexus::{
                DataSpaceCatalog, DataSpaceId, DataSpaceMetadata, LaneCatalog, LaneConfig, LaneId,
                LaneVisibility,
            },
            peer::PeerId,
        };
        use iroha_telemetry::metrics::Status;
        use iroha_test_samples::PEER_KEYPAIR;
        use nonzero_ext::nonzero;

        use super::StateTelemetry;

        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(metrics.clone(), true);

        let lane_catalog = LaneCatalog::new(
            nonzero!(2_u32),
            vec![
                LaneConfig {
                    id: LaneId::new(1),
                    dataspace_id: DataSpaceId::new(7),
                    alias: "restricted-alpha".to_owned(),
                    visibility: LaneVisibility::Restricted,
                    ..LaneConfig::default()
                },
                LaneConfig {
                    id: LaneId::new(2),
                    dataspace_id: DataSpaceId::new(9),
                    alias: "restricted-beta".to_owned(),
                    visibility: LaneVisibility::Restricted,
                    ..LaneConfig::default()
                },
            ],
        )
        .expect("lane catalog");
        let dataspace_catalog = DataSpaceCatalog::new(vec![
            DataSpaceMetadata {
                id: DataSpaceId::new(7),
                alias: "alpha".to_owned(),
                description: Some("restricted alpha".to_owned()),
                fault_tolerance: 1,
            },
            DataSpaceMetadata {
                id: DataSpaceId::new(9),
                alias: "beta".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("dataspace catalog");
        telemetry.set_nexus_catalogs(&lane_catalog, &dataspace_catalog);

        let peer = PeerId::new(PEER_KEYPAIR.public_key().clone());
        telemetry.record_tx_gossip_attempt(
            GossipPlane::Restricted,
            DataSpaceId::new(7),
            &[LaneId::new(1)],
            &[peer],
            Some(nonzero!(3usize)),
            true,
            None,
            false,
            None,
            2,
            256,
        );
        telemetry.record_tx_gossip_attempt(
            GossipPlane::Restricted,
            DataSpaceId::new(9),
            &[LaneId::new(2)],
            &[],
            Some(nonzero!(2usize)),
            false,
            Some("no_restricted_targets"),
            true,
            Some("public_overlay"),
            0,
            0,
        );

        let status = Status::from(&*telemetry);

        let alpha = status
            .tx_gossip
            .targets
            .iter()
            .find(|entry| entry.dataspace_id == 7)
            .expect("alpha dataspace entry");
        assert_eq!(alpha.dataspace_alias.as_deref(), Some("alpha"));
        assert_eq!(alpha.lane_ids, vec![1]);
        assert_eq!(alpha.targets, 1);
        assert_eq!(alpha.target_peers.len(), 1);
        assert_eq!(alpha.outcome, "sent");
        assert!(!alpha.fallback_used);
        assert!(alpha.reason.is_none());

        let beta = status
            .tx_gossip
            .targets
            .iter()
            .find(|entry| entry.dataspace_id == 9)
            .expect("beta dataspace entry");
        assert_eq!(beta.dataspace_alias.as_deref(), Some("beta"));
        assert_eq!(beta.lane_ids, vec![2]);
        assert_eq!(beta.targets, 0);
        assert_eq!(beta.outcome, "dropped");
        assert_eq!(beta.reason.as_deref(), Some("no_restricted_targets"));
        assert!(beta.fallback_used);
        assert_eq!(beta.fallback_surface.as_deref(), Some("public_overlay"));
        assert_eq!(beta.target_cap, 2);
        assert_eq!(beta.batch_txs, 0);
        assert_eq!(beta.frame_bytes, 0);
    }

    #[test]
    fn settlement_conversion_metrics_skip_when_disabled() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(metrics.clone(), false);
        telemetry.inc_settlement_conversion_total("lane-2", "ds-4", "xor#sora", 5);
        telemetry.inc_settlement_haircut_total("lane-2", "ds-4", 5_000_000);

        let conversions = metrics
            .settlement_conversion_total
            .with_label_values(&["lane-2", "ds-4", "xor#sora"])
            .get();
        assert_eq!(
            conversions, 0,
            "disabled telemetry does not increment conversion counters"
        );
        let haircut = metrics
            .settlement_haircut_total
            .with_label_values(&["lane-2", "ds-4"])
            .get();
        assert!(
            (haircut - 0.0).abs() < f64::EPSILON,
            "disabled telemetry does not increment haircut counters"
        );
    }

    #[test]
    fn missing_block_fetch_telemetry_records_metrics() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = Telemetry::new(metrics.clone(), true);
        telemetry.note_missing_block_fetch(
            MissingBlockFetchOutcome::Requested,
            3,
            Duration::from_millis(25),
            Some(MissingBlockFetchTargetKind::Signers),
        );

        let requested = metrics
            .sumeragi_missing_block_fetch_total
            .with_label_values(&["requested"])
            .get();
        assert_eq!(requested, 1);
        assert_eq!(
            metrics
                .sumeragi_missing_block_fetch_targets
                .get_sample_sum(),
            3.0
        );
        let targets = metrics
            .sumeragi_missing_block_fetch_target_total
            .with_label_values(&["signers"])
            .get();
        assert_eq!(targets, 1);
        assert_eq!(
            metrics
                .sumeragi_missing_block_fetch_dwell_ms
                .get_sample_sum(),
            25.0
        );
    }

    #[test]
    fn validation_reject_telemetry_tracks_last_details() {
        let metrics = Arc::new(Metrics::default());
        let mut telemetry = Telemetry::new(metrics.clone(), true);
        let (_handle, time_source) = TimeSource::new_mock(Duration::from_millis(1500));
        telemetry.time_source = time_source;

        telemetry.note_validation_reject("prev_height", 7, 3);

        assert_eq!(
            metrics
                .sumeragi_validation_reject_total
                .with_label_values(&["prev_height"])
                .get(),
            1
        );
        assert_eq!(
            metrics.sumeragi_validation_reject_last_reason.get(),
            4,
            "prev_height should map to code 4"
        );
        assert_eq!(metrics.sumeragi_validation_reject_last_height.get(), 7);
        assert_eq!(metrics.sumeragi_validation_reject_last_view.get(), 3);
        assert_eq!(
            metrics.sumeragi_validation_reject_last_timestamp_ms.get(),
            1500
        );
    }

    #[test]
    fn missing_block_retry_window_gauge_updates() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = Telemetry::new(metrics.clone(), true);
        telemetry.set_missing_block_retry_window_ms(250);

        assert_eq!(metrics.sumeragi_missing_block_retry_window_ms.get(), 250);
    }

    #[test]
    fn missing_block_inflight_gauges_update_counts_and_oldest() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = Telemetry::new(metrics.clone(), true);

        telemetry.set_missing_block_inflight(2, 75);

        assert_eq!(metrics.sumeragi_missing_block_requests.get(), 2);
        assert_eq!(metrics.sumeragi_missing_block_oldest_ms.get(), 75);
    }

    #[test]
    fn missing_block_dwell_histogram_records_observation() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = Telemetry::new(metrics.clone(), true);

        telemetry.observe_missing_block_dwell(Duration::from_millis(12));

        assert_eq!(
            metrics.sumeragi_missing_block_dwell_ms.get_sample_count(),
            1
        );
        assert_eq!(
            metrics.sumeragi_missing_block_dwell_ms.get_sample_sum(),
            12.0
        );
    }

    #[test]
    fn kura_store_failure_metrics_increment_by_outcome() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = Telemetry::new(metrics.clone(), true);

        telemetry.inc_kura_store_failure("retry");
        telemetry.inc_kura_store_failure("abort");

        assert_eq!(
            metrics
                .sumeragi_kura_store_failures_total
                .with_label_values(&["retry"])
                .get(),
            1
        );
        assert_eq!(
            metrics
                .sumeragi_kura_store_failures_total
                .with_label_values(&["abort"])
                .get(),
            1
        );
    }

    #[test]
    fn kura_store_retry_gauges_record_attempt_and_backoff() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = Telemetry::new(metrics.clone(), true);

        telemetry.set_kura_store_retry(2, 40);

        assert_eq!(metrics.sumeragi_kura_store_last_retry_attempt.get(), 2);
        assert_eq!(metrics.sumeragi_kura_store_last_retry_backoff_ms.get(), 40);

        let disabled = Telemetry::new(metrics.clone(), false);
        disabled.set_kura_store_retry(5, 99);

        assert_eq!(metrics.sumeragi_kura_store_last_retry_attempt.get(), 2);
        assert_eq!(metrics.sumeragi_kura_store_last_retry_backoff_ms.get(), 40);
    }

    #[test]
    fn da_gate_reason_metrics_update_counters_and_gauges() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = Telemetry::new(metrics.clone(), true);

        telemetry.note_da_gate_block(GateReason::MissingAvailabilityQc);
        telemetry.set_da_gate_last_reason(Some(GateReason::ManifestGuard {
            lane: LaneId::new(1),
            epoch: 7,
            sequence: 3,
            kind: crate::sumeragi::da::ManifestGateKind::HashMismatch,
        }));
        assert_eq!(metrics.sumeragi_da_gate_last_reason.get(), 4);
        telemetry.set_da_gate_last_reason(None);

        assert_eq!(
            metrics
                .sumeragi_da_gate_block_total
                .with_label_values(&["missing_availability_qc"])
                .get(),
            1
        );
        assert_eq!(
            metrics.sumeragi_da_gate_last_reason.get(),
            0,
            "clearing the gate reason should reset the gauge to zero"
        );
    }

    #[test]
    fn da_gate_satisfaction_metrics_record_transitions() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = Telemetry::new(metrics.clone(), true);

        telemetry.note_da_gate_satisfaction(GateSatisfaction::AvailabilityQc);

        assert_eq!(
            metrics
                .sumeragi_da_gate_satisfied_total
                .with_label_values(&["availability_qc"])
                .get(),
            1
        );
        assert_eq!(metrics.sumeragi_da_gate_last_satisfied.get(), 1);
    }

    #[test]
    fn manifest_guard_metrics_record_outcomes() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = Telemetry::new(metrics.clone(), true);

        telemetry.note_da_manifest_guard(ManifestGuardResult::Allowed, ManifestGuardReason::Ok);
        telemetry
            .note_da_manifest_guard(ManifestGuardResult::Rejected, ManifestGuardReason::Missing);

        assert_eq!(
            metrics
                .sumeragi_da_manifest_guard_total
                .with_label_values(&["allowed", "ok"])
                .get(),
            1
        );
        assert_eq!(
            metrics
                .sumeragi_da_manifest_guard_total
                .with_label_values(&["rejected", "missing"])
                .get(),
            1
        );
    }

    #[test]
    fn da_spool_cache_metrics_record_hits_and_misses() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = Telemetry::new(metrics.clone(), true);

        telemetry.note_da_spool_cache(DaSpoolCacheKind::Commitments, CacheResult::Miss);
        telemetry.note_da_spool_cache(DaSpoolCacheKind::Commitments, CacheResult::Hit);
        telemetry.note_da_spool_cache(DaSpoolCacheKind::Receipts, CacheResult::Hit);

        assert_eq!(
            metrics
                .sumeragi_da_spool_cache_total
                .with_label_values(&["commitments", "miss"])
                .get(),
            1
        );
        assert_eq!(
            metrics
                .sumeragi_da_spool_cache_total
                .with_label_values(&["commitments", "hit"])
                .get(),
            1
        );
        assert_eq!(
            metrics
                .sumeragi_da_spool_cache_total
                .with_label_values(&["receipts", "hit"])
                .get(),
            1
        );
    }

    #[test]
    fn da_manifest_cache_metrics_record_hits_and_misses() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = Telemetry::new(metrics.clone(), true);

        telemetry.note_da_manifest_cache(CacheResult::Miss);
        telemetry.note_da_manifest_cache(CacheResult::Hit);

        assert_eq!(
            metrics
                .sumeragi_da_manifest_cache_total
                .with_label_values(&["miss"])
                .get(),
            1
        );
        assert_eq!(
            metrics
                .sumeragi_da_manifest_cache_total
                .with_label_values(&["hit"])
                .get(),
            1
        );
    }

    #[test]
    fn pin_intent_spool_metrics_record_outcomes() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = Telemetry::new(metrics.clone(), true);

        telemetry.note_da_pin_intent_spool(
            PinIntentSpoolResult::Dropped,
            PinIntentSpoolReason::ZeroManifest,
        );
        telemetry.note_da_pin_intent_spool(
            PinIntentSpoolResult::Dropped,
            PinIntentSpoolReason::SealedDuplicate,
        );
        telemetry.note_da_pin_intent_spool(PinIntentSpoolResult::Kept, PinIntentSpoolReason::Kept);

        assert_eq!(
            metrics
                .sumeragi_da_pin_intent_spool_total
                .with_label_values(&["dropped", "zero_manifest"])
                .get(),
            1
        );
        assert_eq!(
            metrics
                .sumeragi_da_pin_intent_spool_total
                .with_label_values(&["dropped", "sealed_duplicate"])
                .get(),
            1
        );
        assert_eq!(
            metrics
                .sumeragi_da_pin_intent_spool_total
                .with_label_values(&["kept", "kept"])
                .get(),
            1
        );
    }

    #[test]
    fn qc_validation_error_counter_increments_by_reason() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = Telemetry::new(metrics.clone(), true);

        telemetry.note_qc_validation_error("invalid_signature");

        assert_eq!(
            metrics
                .sumeragi_qc_validation_errors_total
                .with_label_values(&["invalid_signature"])
                .get(),
            1
        );
        assert_eq!(
            metrics
                .sumeragi_qc_validation_errors_total
                .with_label_values(&["missing_votes"])
                .get(),
            0
        );
    }

    #[test]
    fn qc_signer_count_histogram_records_counts() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = Telemetry::new(metrics.clone(), true);

        telemetry.note_qc_signer_counts("commit", 5, 3);

        let present = metrics
            .sumeragi_qc_signer_counts
            .with_label_values(&["commit", "present"]);
        let counted = metrics
            .sumeragi_qc_signer_counts
            .with_label_values(&["commit", "counted"]);
        assert_eq!(present.get_sample_count(), 1);
        assert_eq!(present.get_sample_sum(), 5.0);
        assert_eq!(counted.get_sample_count(), 1);
        assert_eq!(counted.get_sample_sum(), 3.0);
    }

    #[test]
    fn invalid_signature_counter_tracks_outcomes() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = Telemetry::new(metrics.clone(), true);

        telemetry.inc_invalid_signature("vote", "logged");
        telemetry.inc_invalid_signature("vote", "throttled");

        assert_eq!(
            metrics
                .sumeragi_invalid_signature_total
                .with_label_values(&["vote", "logged"])
                .get(),
            1
        );
        assert_eq!(
            metrics
                .sumeragi_invalid_signature_total
                .with_label_values(&["vote", "throttled"])
                .get(),
            1
        );
    }

    #[test]
    fn axt_policy_reject_counter_increments_by_reason() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = Telemetry::new(metrics.clone(), true);
        let lane = LaneId::new(3);

        telemetry.note_axt_policy_reject(lane, AxtRejectReason::Lane, 7);
        assert_eq!(
            metrics
                .axt_policy_reject_total
                .with_label_values(&["3", "lane"])
                .get(),
            1
        );

        telemetry.disable();
        telemetry.note_axt_policy_reject(lane, AxtRejectReason::Manifest, 7);
        assert_eq!(
            metrics
                .axt_policy_reject_total
                .with_label_values(&["3", "manifest"])
                .get(),
            1,
            "disabled telemetry must not record additional rejects"
        );
    }

    #[test]
    fn axt_policy_reject_snapshot_tracks_reason_and_version() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(metrics, true);
        let lane = LaneId::new(5);

        telemetry.note_axt_policy_reject(lane, AxtRejectReason::Expiry, 42);
        let snapshot = telemetry
            .axt_policy_reject_snapshot()
            .expect("snapshot recorded");
        assert_eq!(snapshot.lane, lane);
        assert_eq!(snapshot.reason, AxtRejectReason::Expiry);
        assert_eq!(snapshot.snapshot_version, 42);

        let dsid = DataSpaceId::new(99);
        telemetry.set_axt_reject_hint(dsid, lane, 5, 7, AxtRejectReason::HandleEra);
        let entry = AxtPolicyBinding {
            dsid,
            policy: AxtPolicyEntry {
                manifest_root: [1; 32],
                target_lane: lane,
                min_handle_era: 5,
                min_sub_nonce: 7,
                current_slot: 0,
            },
        };
        telemetry.set_axt_policy_snapshot_version(&AxtPolicySnapshot {
            version: 99,
            entries: vec![entry],
        });
        telemetry.set_axt_proof_cache_state(dsid, "hit", [1; 32], 3, Some(4));
        let debug_status = telemetry.axt_debug_status();
        assert_eq!(debug_status.policy_snapshot_version, 99);
        assert_eq!(
            debug_status.last_reject.as_ref().map(|r| r.reason),
            Some(AxtRejectReason::Expiry)
        );
        assert_eq!(debug_status.hints.len(), 1);
        assert_eq!(debug_status.cache.len(), 1);
    }

    #[test]
    fn axt_policy_snapshot_cache_events_respect_enabled_flag() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = Telemetry::new(metrics.clone(), true);

        telemetry.note_axt_policy_snapshot_cache_event("cache_hit");
        assert_eq!(
            metrics
                .axt_policy_snapshot_cache_events_total
                .with_label_values(&["cache_hit"])
                .get(),
            1
        );

        telemetry.disable();
        telemetry.note_axt_policy_snapshot_cache_event("cache_miss");
        assert_eq!(
            metrics
                .axt_policy_snapshot_cache_events_total
                .with_label_values(&["cache_miss"])
                .get(),
            0,
            "disabled telemetry must not record cache misses"
        );
    }

    #[test]
    fn axt_proof_cache_events_respect_enabled_flag() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = Telemetry::new(metrics.clone(), true);

        telemetry.note_axt_proof_cache_event("hit");
        assert_eq!(
            metrics
                .axt_proof_cache_events_total
                .with_label_values(&["hit"])
                .get(),
            1
        );

        telemetry.disable();
        telemetry.note_axt_proof_cache_event("miss");
        assert_eq!(
            metrics
                .axt_proof_cache_events_total
                .with_label_values(&["miss"])
                .get(),
            0,
            "disabled telemetry must not record cache misses"
        );
    }

    #[test]
    fn axt_proof_cache_state_tracks_entries() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = Telemetry::new(metrics.clone(), true);
        let dsid = DataSpaceId::new(5);
        let manifest_root = [0xAA; 32];
        let labels = ["5", "miss", &hex::encode(manifest_root), "7"];

        telemetry.set_axt_proof_cache_state(dsid, "miss", manifest_root, 7, Some(30));
        assert_eq!(
            metrics
                .axt_proof_cache_state
                .with_label_values(&labels)
                .get(),
            30
        );

        telemetry.clear_axt_proof_cache_state(dsid, "miss", manifest_root, 7);
        assert_eq!(
            metrics
                .axt_proof_cache_state
                .with_label_values(&labels)
                .get(),
            0
        );
    }

    #[test]
    fn state_telemetry_tracks_axt_proof_cache_snapshot() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(metrics.clone(), true);
        let dsid = DataSpaceId::new(6);
        let manifest_root = [0xBC; 32];
        let labels = ["6", "cleared", &hex::encode(manifest_root), "4"];

        telemetry.set_axt_proof_cache_state(dsid, "miss", manifest_root, 4, Some(12));
        let snapshot = telemetry.axt_proof_cache_status_snapshot();
        assert_eq!(snapshot.len(), 1);
        let entry = &snapshot[0];
        assert_eq!(entry.dataspace, dsid);
        assert_eq!(entry.status, "miss");
        assert_eq!(entry.manifest_root, Some(manifest_root));
        assert_eq!(entry.verified_slot, 4);
        assert_eq!(entry.expiry_slot, Some(12));

        telemetry.clear_axt_proof_cache_state(dsid, "miss", manifest_root, 4);
        let snapshot = telemetry.axt_proof_cache_status_snapshot();
        assert_eq!(snapshot.len(), 1);
        let entry = &snapshot[0];
        assert_eq!(entry.dataspace, dsid);
        assert_eq!(entry.status, "cleared");
        assert_eq!(entry.manifest_root, Some(manifest_root));
        assert_eq!(entry.verified_slot, 4);
        assert_eq!(entry.expiry_slot, None);

        assert_eq!(
            metrics
                .axt_proof_cache_state
                .with_label_values(&labels)
                .get(),
            0
        );
    }

    #[test]
    fn axt_policy_snapshot_version_tracks_hash() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = Telemetry::new(metrics.clone(), true);
        let entries = vec![
            AxtPolicyBinding {
                dsid: DataSpaceId::new(7),
                policy: AxtPolicyEntry {
                    manifest_root: [0x11; 32],
                    target_lane: LaneId::new(3),
                    min_handle_era: 4,
                    min_sub_nonce: 2,
                    current_slot: 9,
                },
            },
            AxtPolicyBinding {
                dsid: DataSpaceId::new(8),
                policy: AxtPolicyEntry {
                    manifest_root: [0x22; 32],
                    target_lane: LaneId::new(4),
                    min_handle_era: 5,
                    min_sub_nonce: 3,
                    current_slot: 10,
                },
            },
        ];
        let snapshot = AxtPolicySnapshot {
            version: AxtPolicySnapshot::compute_version(&entries),
            entries,
        };

        telemetry.set_axt_policy_snapshot_version(&snapshot);

        let expected = snapshot.version;

        assert_eq!(metrics.axt_policy_snapshot_version.get(), expected);

        telemetry.disable();
        telemetry.set_axt_policy_snapshot_version(&AxtPolicySnapshot::default());
        assert_eq!(
            metrics.axt_policy_snapshot_version.get(),
            expected,
            "disabled telemetry must not change the recorded version"
        );
    }

    struct SystemUnderTest {
        telemetry: Telemetry,
        _child: Child,
        online_peers_tx: watch::Sender<OnlinePeers>,
        mock_time_handle: MockTimeHandle,
        time_source: TimeSource,
        kura: Arc<Kura>,
        state: Arc<State>,
        chain_id: ChainId,
        account_id: AccountId,
        account_keypair: KeyPair,
        leader_private_key: PrivateKey,
        topology: Topology,
    }

    #[test]
    fn lane_manifest_readiness_exposed() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(metrics.clone(), true);
        let lane_catalog = LaneCatalog::new(
            nonzero!(1_u32),
            vec![LaneConfig {
                id: LaneId::new(0),
                alias: "gov".to_string(),
                governance: Some("parliament".to_string()),
                ..LaneConfig::default()
            }],
        )
        .expect("lane catalog");
        telemetry.set_nexus_catalogs(&lane_catalog, &DataSpaceCatalog::default());

        let mut statuses = BTreeMap::new();
        statuses.insert(
            LaneId::new(0),
            LaneManifestStatus {
                lane: LaneId::new(0),
                alias: "gov".to_string(),
                dataspace: DataSpaceId::GLOBAL,
                visibility: LaneVisibility::Public,
                storage: LaneStorageProfile::FullReplica,
                governance: Some("parliament".to_string()),
                manifest_path: None,
                governance_rules: None,
                privacy_commitments: Vec::new(),
            },
        );
        telemetry
            .set_lane_manifest_registry(Arc::new(LaneManifestRegistry::from_statuses(statuses)));
        telemetry.record_lane_pipeline_summary(LaneId::new(0), LanePipelineSummary::default());
        let snapshot = metrics
            .nexus_scheduler_lane_teu_status
            .read()
            .expect("lane TEU cache")
            .get(&0)
            .cloned()
            .expect("lane snapshot");
        assert!(snapshot.manifest_required);
        assert!(!snapshot.manifest_ready);

        let mut ready_statuses = BTreeMap::new();
        ready_statuses.insert(
            LaneId::new(0),
            LaneManifestStatus {
                lane: LaneId::new(0),
                alias: "gov".to_string(),
                dataspace: DataSpaceId::GLOBAL,
                visibility: LaneVisibility::Public,
                storage: LaneStorageProfile::FullReplica,
                governance: Some("parliament".to_string()),
                manifest_path: Some(PathBuf::from("/tmp/manifest.json")),
                governance_rules: Some(GovernanceRules::default()),
                privacy_commitments: Vec::new(),
            },
        );
        telemetry.set_lane_manifest_registry(Arc::new(LaneManifestRegistry::from_statuses(
            ready_statuses,
        )));
        telemetry.record_lane_pipeline_summary(LaneId::new(0), LanePipelineSummary::default());
        let updated = metrics
            .nexus_scheduler_lane_teu_status
            .read()
            .expect("lane TEU cache")
            .get(&0)
            .cloned()
            .expect("lane snapshot");
        assert!(updated.manifest_required);
        assert!(updated.manifest_ready);
    }

    #[test]
    fn lane_relay_emergency_override_metric_increments() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(metrics.clone(), true);
        let lane_id = LaneId::new(0);
        let dataspace_id = DataSpaceId::new(7);
        let lane_catalog = LaneCatalog::new(
            nonzero!(1_u32),
            vec![LaneConfig {
                id: lane_id,
                dataspace_id,
                alias: "alpha".to_string(),
                ..LaneConfig::default()
            }],
        )
        .expect("lane catalog");
        telemetry.set_nexus_catalogs(&lane_catalog, &DataSpaceCatalog::default());

        telemetry.record_lane_relay_emergency_override(lane_id, dataspace_id, "applied");

        let lane_label = lane_id.as_u32().to_string();
        let dataspace_label = dataspace_id.as_u64().to_string();
        assert_eq!(
            metrics
                .lane_relay_emergency_override_total
                .with_label_values(&[lane_label.as_str(), dataspace_label.as_str(), "applied",])
                .get(),
            1
        );
    }

    #[test]
    fn lane_relay_emergency_override_metric_skips_when_disabled() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(metrics.clone(), false);
        let lane_id = LaneId::SINGLE;
        let dataspace_id = DataSpaceId::GLOBAL;

        telemetry.record_lane_relay_emergency_override(lane_id, dataspace_id, "missing");

        let lane_label = lane_id.as_u32().to_string();
        let dataspace_label = dataspace_id.as_u64().to_string();
        assert_eq!(
            metrics
                .lane_relay_emergency_override_total
                .with_label_values(&[lane_label.as_str(), dataspace_label.as_str(), "missing",])
                .get(),
            0
        );
    }

    #[test]
    fn lane_relay_emergency_override_metric_skips_when_nexus_disabled() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(metrics.clone(), true);
        let lane_id = LaneId::SINGLE;
        let dataspace_id = DataSpaceId::GLOBAL;
        telemetry.set_nexus_enabled(false);

        telemetry.record_lane_relay_emergency_override(lane_id, dataspace_id, "missing");

        let lane_label = lane_id.as_u32().to_string();
        let dataspace_label = dataspace_id.as_u64().to_string();
        assert_eq!(
            metrics
                .lane_relay_emergency_override_total
                .with_label_values(&[lane_label.as_str(), dataspace_label.as_str(), "missing",])
                .get(),
            0
        );
    }

    #[test]
    fn amx_metrics_recorded() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(metrics.clone(), true);
        let lane_catalog = LaneCatalog::new(
            nonzero!(1_u32),
            vec![LaneConfig {
                id: LaneId::new(7),
                alias: "exec".to_string(),
                ..LaneConfig::default()
            }],
        )
        .expect("lane catalog");
        telemetry.set_nexus_catalogs(&lane_catalog, &DataSpaceCatalog::default());

        let lane = LaneId::new(7);
        telemetry.observe_amx_prepare_ms(lane, 12.0);
        telemetry.observe_amx_commit_ms(lane, 4.0);
        telemetry.observe_ivm_exec_ms(lane, 9.0);
        telemetry.inc_amx_abort(lane, "prepare");

        assert_eq!(
            metrics
                .amx_prepare_ms
                .with_label_values(&["7"])
                .get_sample_count(),
            1
        );
        assert_eq!(
            metrics
                .amx_commit_ms
                .with_label_values(&["7"])
                .get_sample_count(),
            1
        );
        assert_eq!(
            metrics
                .ivm_exec_ms
                .with_label_values(&["7"])
                .get_sample_count(),
            1
        );
        assert_eq!(
            metrics
                .amx_abort_total
                .with_label_values(&["7", "prepare"])
                .get(),
            1
        );
    }

    #[cfg(feature = "telemetry")]
    #[test]
    fn sorafs_proof_health_metrics_recorded() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(metrics.clone(), true);
        let provider_id = ProviderId::new([0x11; 32]);
        let alert = SorafsProofHealthAlert {
            provider_id,
            window_start_epoch: 1,
            window_end_epoch: 2,
            prior_strikes: 0,
            strike_threshold: 1,
            pdp_challenges: 4,
            pdp_failures: 3,
            potr_windows: 2,
            potr_breaches: 1,
            triggered_by_pdp: true,
            triggered_by_potr: false,
            max_pdp_failures: 0,
            max_potr_breaches: 0,
            penalty_bond_bps: 100,
            penalty_applied_nano: 42,
            cooldown_active: false,
        };
        telemetry.record_sorafs_proof_health_alert(&alert);
        let provider_hex = hex::encode(provider_id.as_bytes());
        assert_eq!(
            metrics
                .torii_sorafs_proof_health_alerts_total
                .with_label_values(&[provider_hex.as_str(), "pdp", "penalty_applied"])
                .get(),
            1
        );
        assert_eq!(
            metrics
                .torii_sorafs_proof_health_pdp_failures
                .with_label_values(&[provider_hex.as_str()])
                .get(),
            i64::from(alert.pdp_failures)
        );
        assert_eq!(
            metrics
                .torii_sorafs_proof_health_potr_breaches
                .with_label_values(&[provider_hex.as_str()])
                .get(),
            i64::from(alert.potr_breaches)
        );
        let penalty_nano =
            u64::try_from(alert.penalty_applied_nano.min(u128::from(u64::MAX))).expect("clamped");
        assert_eq!(
            metrics
                .torii_sorafs_proof_health_penalty_nano
                .with_label_values(&[provider_hex.as_str()])
                .get(),
            penalty_nano
        );
        assert_eq!(
            metrics
                .torii_sorafs_proof_health_window_end_epoch
                .with_label_values(&[provider_hex.as_str()])
                .get(),
            alert.window_end_epoch
        );
        assert_eq!(
            metrics
                .torii_sorafs_proof_health_cooldown
                .with_label_values(&[provider_hex.as_str()])
                .get(),
            0
        );
    }

    #[cfg(feature = "telemetry")]
    #[test]
    #[allow(clippy::too_many_lines)]
    fn confidential_tree_metrics_recorded() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(metrics.clone(), true);
        let asset_id: AssetDefinitionId = "rose#sora".parse().expect("asset id");
        let label = asset_id.to_string();

        let initial = ConfidentialTreeStats {
            commitments: 42,
            tree_depth: 17,
            root_history: 8,
            frontier_checkpoints: 3,
            last_checkpoint_height: 24,
            last_checkpoint_commitments: 21,
            root_evictions: 0,
            frontier_evictions: 0,
        };

        telemetry.record_confidential_tree_stats(&asset_id, initial);

        assert_eq!(
            metrics
                .confidential_tree_commitments
                .with_label_values(&[label.as_str()])
                .get(),
            initial.commitments
        );
        assert_eq!(
            metrics
                .confidential_tree_depth
                .with_label_values(&[label.as_str()])
                .get(),
            initial.tree_depth
        );
        assert_eq!(
            metrics
                .confidential_root_history_entries
                .with_label_values(&[label.as_str()])
                .get(),
            initial.root_history
        );
        assert_eq!(
            metrics
                .confidential_frontier_checkpoints
                .with_label_values(&[label.as_str()])
                .get(),
            initial.frontier_checkpoints
        );
        assert_eq!(
            metrics
                .confidential_frontier_last_height
                .with_label_values(&[label.as_str()])
                .get(),
            initial.last_checkpoint_height
        );
        assert_eq!(
            metrics
                .confidential_frontier_last_commitments
                .with_label_values(&[label.as_str()])
                .get(),
            initial.last_checkpoint_commitments
        );
        assert_eq!(
            metrics
                .confidential_root_evictions_total
                .with_label_values(&[label.as_str()])
                .get(),
            0
        );
        assert_eq!(
            metrics
                .confidential_frontier_evictions_total
                .with_label_values(&[label.as_str()])
                .get(),
            0
        );

        let updated = ConfidentialTreeStats {
            commitments: 64,
            tree_depth: 19,
            root_history: 10,
            frontier_checkpoints: 4,
            last_checkpoint_height: 48,
            last_checkpoint_commitments: 34,
            root_evictions: 2,
            frontier_evictions: 1,
        };

        telemetry.record_confidential_tree_stats(&asset_id, updated);

        assert_eq!(
            metrics
                .confidential_tree_commitments
                .with_label_values(&[label.as_str()])
                .get(),
            updated.commitments
        );
        assert_eq!(
            metrics
                .confidential_tree_depth
                .with_label_values(&[label.as_str()])
                .get(),
            updated.tree_depth
        );
        assert_eq!(
            metrics
                .confidential_root_history_entries
                .with_label_values(&[label.as_str()])
                .get(),
            updated.root_history
        );
        assert_eq!(
            metrics
                .confidential_frontier_checkpoints
                .with_label_values(&[label.as_str()])
                .get(),
            updated.frontier_checkpoints
        );
        assert_eq!(
            metrics
                .confidential_frontier_last_height
                .with_label_values(&[label.as_str()])
                .get(),
            updated.last_checkpoint_height
        );
        assert_eq!(
            metrics
                .confidential_frontier_last_commitments
                .with_label_values(&[label.as_str()])
                .get(),
            updated.last_checkpoint_commitments
        );
        assert_eq!(
            metrics
                .confidential_root_evictions_total
                .with_label_values(&[label.as_str()])
                .get(),
            updated.root_evictions
        );
        assert_eq!(
            metrics
                .confidential_frontier_evictions_total
                .with_label_values(&[label.as_str()])
                .get(),
            updated.frontier_evictions
        );
    }

    #[cfg(feature = "telemetry")]
    #[test]
    fn confidential_tree_metrics_skip_when_disabled() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(metrics.clone(), false);
        let asset_id: AssetDefinitionId = "rose#sora".parse().expect("asset id");
        let label = asset_id.to_string();
        let stats = ConfidentialTreeStats {
            commitments: 5,
            tree_depth: 6,
            root_history: 2,
            frontier_checkpoints: 1,
            last_checkpoint_height: 11,
            last_checkpoint_commitments: 7,
            root_evictions: 3,
            frontier_evictions: 4,
        };

        telemetry.record_confidential_tree_stats(&asset_id, stats);

        assert_eq!(
            metrics
                .confidential_tree_commitments
                .with_label_values(&[label.as_str()])
                .get(),
            0
        );
        assert_eq!(
            metrics
                .confidential_root_evictions_total
                .with_label_values(&[label.as_str()])
                .get(),
            0
        );
        assert_eq!(
            metrics
                .confidential_frontier_evictions_total
                .with_label_values(&[label.as_str()])
                .get(),
            0
        );
        assert_eq!(
            metrics
                .confidential_frontier_last_height
                .with_label_values(&[label.as_str()])
                .get(),
            0
        );
        assert_eq!(
            metrics
                .confidential_frontier_last_commitments
                .with_label_values(&[label.as_str()])
                .get(),
            0
        );
    }

    #[cfg(feature = "telemetry")]
    #[test]
    fn confidential_gas_schedule_metrics_update() {
        let metrics = Metrics::default();
        let gas = ActualConfidentialGas {
            proof_base: 7,
            per_public_input: 11,
            per_proof_byte: 13,
            per_nullifier: 17,
            per_commitment: 19,
        };

        metrics.set_confidential_gas_schedule(&gas);

        assert_eq!(metrics.confidential_gas_base_verify.get(), gas.proof_base);
        assert_eq!(
            metrics.confidential_gas_per_public_input.get(),
            gas.per_public_input
        );
        assert_eq!(
            metrics.confidential_gas_per_proof_byte.get(),
            gas.per_proof_byte
        );
        assert_eq!(
            metrics.confidential_gas_per_nullifier.get(),
            gas.per_nullifier
        );
        assert_eq!(
            metrics.confidential_gas_per_commitment.get(),
            gas.per_commitment
        );
    }

    #[cfg(feature = "telemetry")]
    #[test]
    fn confidential_gas_usage_metrics_update() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(metrics.clone(), true);
        telemetry.set_confidential_gas_usage(42, 84);
        assert_eq!(metrics.confidential_gas_tx_used.get(), 42);
        assert_eq!(metrics.confidential_gas_block_used.get(), 84);
        assert_eq!(metrics.confidential_gas_total.get(), 42);
    }

    #[cfg(feature = "telemetry")]
    #[test]
    fn block_fee_units_reset_clears_gauge() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(metrics.clone(), true);
        telemetry.add_block_fee_units(42);
        assert_eq!(metrics.block_fee_total_units.get(), 42);
        telemetry.reset_block_fee_units();
        assert_eq!(metrics.block_fee_total_units.get(), 0);
    }

    #[test]
    fn space_directory_metrics_cover_lifecycle() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(metrics.clone(), true);
        let dataspace = DataSpaceId::new(11);
        let dataspace_catalog = DataSpaceCatalog::new(vec![DataSpaceMetadata {
            id: dataspace,
            alias: "cbdc".to_string(),
            description: Some("CBDC lane profile".to_string()),
            fault_tolerance: 1,
        }])
        .expect("dataspace catalog");
        telemetry.set_nexus_catalogs(&LaneCatalog::default(), &dataspace_catalog);

        let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::telemetry"));
        let manifest = AssetPermissionManifest {
            version: ManifestVersion::V1,
            uaid,
            dataspace,
            issued_ms: 1,
            activation_epoch: 5,
            expiry_epoch: None,
            entries: Vec::new(),
        };
        let mut record = SpaceDirectoryManifestRecord::new(manifest);
        record.lifecycle.mark_activated(5);
        let mut manifest_set = SpaceDirectoryManifestSet::default();
        manifest_set.upsert(record);

        let mut world = World::default();
        {
            let storage = world.space_directory_manifests_mut_for_testing();
            storage.insert(uaid, manifest_set);
        }
        let manifest_view = world.space_directory_manifests.view();
        telemetry.seed_space_directory_manifests(&manifest_view);

        assert_eq!(
            metrics
                .nexus_space_directory_active_manifests
                .with_label_values(&["cbdc", "11", "CBDC lane profile"])
                .get(),
            1
        );

        let revoked = SpaceDirectoryEvent::ManifestRevoked(SpaceDirectoryManifestRevoked {
            dataspace,
            uaid,
            manifest_hash: Hash::new(b"manifest-hash"),
            revoked_epoch: 9,
            reason: Some("fraud".to_string()),
        });
        telemetry.on_space_directory_event(&revoked);

        assert_eq!(
            metrics
                .nexus_space_directory_active_manifests
                .with_label_values(&["cbdc", "11", "CBDC lane profile"])
                .get(),
            0
        );
        assert_eq!(
            metrics
                .nexus_space_directory_revocations_total
                .with_label_values(&["cbdc", "11", "fraud"])
                .get(),
            1
        );

        telemetry.record_space_directory_revision(dataspace);
        assert_eq!(
            metrics
                .nexus_space_directory_revision_total
                .with_label_values(&["cbdc", "11"])
                .get(),
            1
        );
    }

    #[test]
    fn lane_headroom_pct_computes_expected_ratio() {
        let low = LaneTeuGaugeUpdate {
            capacity: 1_000,
            committed: 950,
            buckets: NexusLaneTeuBuckets {
                floor: 600,
                headroom: 50,
                must_serve: 300,
                circuit_breaker: 0,
            },
            trigger_level: 0,
            starvation_bound_slots: 16,
        };
        assert_eq!(StateTelemetry::lane_headroom_pct(&low), 5);

        let zero_capacity = LaneTeuGaugeUpdate {
            capacity: 0,
            committed: 0,
            buckets: NexusLaneTeuBuckets::default(),
            trigger_level: 0,
            starvation_bound_slots: 0,
        };
        assert_eq!(StateTelemetry::lane_headroom_pct(&zero_capacity), 100);
    }

    #[test]
    fn lane_headroom_event_emits_expected_fields() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(metrics.clone(), true);
        let mut snapshot = NexusLaneTeuStatus::default();
        snapshot.deferrals.cap_exceeded = 3;
        snapshot.must_serve_truncations = 1;
        let update = LaneTeuGaugeUpdate {
            capacity: 1_000,
            committed: 950,
            buckets: NexusLaneTeuBuckets {
                floor: 600,
                headroom: 50,
                must_serve: 300,
                circuit_breaker: 50,
            },
            trigger_level: 0,
            starvation_bound_slots: 24,
        };

        let payload = telemetry
            .emit_lane_headroom_event(LaneId::new(2), &snapshot, &update, 5)
            .expect("headroom payload");
        let json = norito::json::to_string(&payload).expect("serialize headroom payload");
        assert!(json.contains("\"lane_id\":2"));
        assert!(json.contains("\"headroom_pct\":5"));
        assert!(json.contains("\"cap_exceeded\":3"));
    }

    #[test]
    fn lane_headroom_event_triggers_below_threshold() {
        let low = LaneTeuGaugeUpdate {
            capacity: 1_000,
            committed: 950,
            buckets: NexusLaneTeuBuckets {
                floor: 600,
                headroom: 50,
                must_serve: 300,
                circuit_breaker: 50,
            },
            trigger_level: 0,
            starvation_bound_slots: 24,
        };
        let high = LaneTeuGaugeUpdate {
            capacity: 1_000,
            committed: 800,
            buckets: NexusLaneTeuBuckets {
                floor: 600,
                headroom: 200,
                must_serve: 200,
                circuit_breaker: 0,
            },
            trigger_level: 0,
            starvation_bound_slots: 24,
        };
        let low_pct = StateTelemetry::lane_headroom_pct(&low);
        let high_pct = StateTelemetry::lane_headroom_pct(&high);
        assert!(StateTelemetry::should_emit_lane_headroom_event(
            low_pct, &low
        ));
        assert!(!StateTelemetry::should_emit_lane_headroom_event(
            high_pct, &high
        ));
    }

    #[cfg(feature = "telemetry")]
    #[test]
    fn social_escrow_gauge_and_rejections_recorded() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(metrics.clone(), true);
        let binding = KeyedHash::new(
            "pepper-social-test",
            b"pepper-social-test",
            b"handle-social",
        );
        let sender: AccountId = "alice@wonderland".parse().expect("account id");
        let escrow = ViralEscrowRecord {
            binding_hash: binding.clone(),
            sender,
            amount: Numeric::new(5, 0),
            created_at_ms: 10,
        };

        telemetry.on_social_event(&SocialEvent::EscrowCreated(ViralEscrowCreated {
            escrow: escrow.clone(),
        }));
        assert_eq!(metrics.social_open_escrows.get(), 1);

        telemetry.on_social_event(&SocialEvent::EscrowCancelled(ViralEscrowCancelled {
            escrow,
            cancelled_at_ms: 20,
        }));
        assert_eq!(metrics.social_open_escrows.get(), 0);

        record_social_rejection(&telemetry, "duplicate_escrow");
        assert_eq!(
            metrics
                .social_rejections_total
                .with_label_values(&["duplicate_escrow"])
                .get(),
            1
        );
    }

    #[test]
    fn oracle_metrics_recorded() {
        use iroha_crypto::Hash;
        use rust_decimal::Decimal;

        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(metrics.clone(), true);
        telemetry.observe_oracle_settlement_context(
            &Decimal::new(1250, 2), // 12.50
            75,
            150,
            4_500,
        );

        assert_eq!(metrics.oracle_twap_window_seconds.get(), 75);
        assert_eq!(metrics.oracle_haircut_basis_points.get(), 150);
        assert!((metrics.oracle_price_local_per_xor.get() - 12.5).abs() < f64::EPSILON);
        assert!(
            (metrics.oracle_staleness_seconds.get() - 4.5).abs() < f64::EPSILON,
            "expected 4.5s staleness"
        );

        let feed_id: iroha_data_model::oracle::FeedId =
            "price_xor_usd".parse().expect("feed id parses");
        let evidence_hashes = vec![Hash::new(b"evidence-1"), Hash::new(b"evidence-2")];
        telemetry.observe_oracle_aggregation(
            &feed_id,
            3,
            &evidence_hashes,
            Duration::from_millis(12),
        );
        let observations = metrics
            .oracle_observations_total
            .get_metric_with_label_values(&[feed_id.as_str()])
            .expect("counter present")
            .get();
        assert_eq!(observations, 3);
        let feed_events = metrics
            .oracle_feed_events_total
            .get_metric_with_label_values(&[feed_id.as_str()])
            .expect("counter present")
            .get();
        assert_eq!(feed_events, 1);
        let with_evidence = metrics
            .oracle_feed_events_with_evidence_total
            .get_metric_with_label_values(&[feed_id.as_str()])
            .expect("counter present")
            .get();
        assert_eq!(with_evidence, 1);
        let evidence_hashes_total = metrics
            .oracle_evidence_hashes_total
            .get_metric_with_label_values(&[feed_id.as_str()])
            .expect("counter present")
            .get();
        assert_eq!(evidence_hashes_total, 2);
        let histogram = metrics
            .oracle_aggregation_duration_ms
            .get_metric_with_label_values(&[feed_id.as_str()])
            .expect("histogram present");
        assert_eq!(histogram.get_sample_count(), 1);
        assert!(
            histogram.get_sample_sum() >= 12.0,
            "histogram sum should record latency"
        );
    }

    #[test]
    #[cfg(feature = "telemetry")]
    fn sorafs_node_micropayment_sink_updates_telemetry_store() {
        let metrics = Arc::new(Metrics::default());
        super::global_sorafs_node_otel().set_micropayment_sink(None);
        let telemetry = Telemetry::new(metrics, true);

        let provider = "feedcafe";
        let credits = MicropaymentCreditSnapshot {
            deterministic_charge: 42,
            credit_generated: 21,
            credit_applied: 11,
            credit_carry: 10,
            outstanding: 1,
        };
        let tickets = MicropaymentTicketCounters {
            processed: 5,
            won: 2,
            duplicate: 1,
        };

        super::global_sorafs_node_otel().record_micropayment_sample(provider, credits, tickets);

        let stored = telemetry
            .micropayment_sample(provider)
            .expect("micropayment sample recorded");
        assert_eq!(stored.credits, credits);
        assert_eq!(stored.tickets, tickets);

        super::global_sorafs_node_otel().set_micropayment_sink(None);
    }

    #[test]
    #[cfg(feature = "telemetry")]
    fn audit_outcome_event_includes_expected_fields() {
        use norito::json::to_string as to_json_string;

        let metrics = Arc::new(Metrics::default());
        let telemetry = Telemetry::new(metrics, true);
        let outcome = TelemetryAuditOutcome {
            trace_id: "TRACE-TEST".to_string(),
            slot_height: 42,
            reviewer: "qa-veracity".to_string(),
            status: "fail".to_string(),
            mitigation_url: Some("https://example.net/mitigation".to_string()),
        };

        let payload = telemetry
            .record_audit_outcome(&outcome)
            .expect("telemetry enabled");
        let serialized = to_json_string(&payload).expect("serialize audit event");
        assert!(
            serialized.contains("\"trace_id\":\"TRACE-TEST\""),
            "audit event should include trace identifier"
        );
        assert!(
            serialized.contains("\"status\":\"fail\""),
            "audit event should include status label"
        );
        assert!(
            serialized.contains("mitigation"),
            "audit event should propagate mitigation link"
        );
        assert_eq!(
            telemetry
                .metrics
                .nexus_audit_outcome_total
                .with_label_values(&["TRACE-TEST", "fail"])
                .get(),
            1
        );
        assert!(
            telemetry
                .metrics
                .nexus_audit_outcome_last_timestamp
                .with_label_values(&["TRACE-TEST"])
                .get()
                > 0
        );
    }

    #[cfg(feature = "telemetry")]
    fn record_encode_metrics(telemetry: &StreamingTelemetry, metrics: &Metrics) {
        let encode_stats = TelemetryEncodeStats {
            segment: 7,
            avg_latency_ms: 18,
            dropped_layers: 3,
            avg_audio_jitter_ms: 5,
            max_audio_jitter_ms: 9,
        };
        telemetry.record_encode_stats(&encode_stats);
        assert_eq!(
            metrics.streaming_encode_dropped_layers_total.get(),
            u64::from(encode_stats.dropped_layers)
        );
        assert_eq!(metrics.streaming_encode_latency_ms.get_sample_count(), 1);
        assert_eq!(
            metrics.streaming_encode_audio_jitter_ms.get_sample_count(),
            1
        );
        assert_eq!(
            metrics.streaming_encode_audio_max_jitter_ms.get(),
            u64::from(encode_stats.max_audio_jitter_ms)
        );
    }

    #[cfg(feature = "telemetry")]
    fn record_decode_metrics(telemetry: &StreamingTelemetry, metrics: &Metrics) {
        let decode_stats = TelemetryDecodeStats {
            segment: 7,
            buffer_ms: 120,
            dropped_frames: 2,
            max_decode_queue_ms: 240,
            avg_av_drift_ms: -3,
            max_av_drift_ms: 11,
        };
        telemetry.record_decode_stats(&decode_stats);
        assert_eq!(
            metrics.streaming_decode_dropped_frames_total.get(),
            u64::from(decode_stats.dropped_frames)
        );
        assert_eq!(metrics.streaming_decode_buffer_ms.get_sample_count(), 1);
        assert_eq!(metrics.streaming_decode_max_queue_ms.get_sample_count(), 1);
        assert_eq!(metrics.streaming_decode_av_drift_ms.get_sample_count(), 1);
        assert_eq!(
            metrics.streaming_decode_max_drift_ms.get(),
            u64::from(decode_stats.max_av_drift_ms)
        );
    }

    #[cfg(feature = "telemetry")]
    fn record_network_metrics(telemetry: &StreamingTelemetry, metrics: &Metrics) {
        let network_stats = TelemetryNetworkStats {
            rtt_ms: 33,
            loss_percent_x100: 125,
            fec_repairs: 4,
            fec_failures: 1,
            datagram_reinjects: 6,
        };
        telemetry.record_network_stats(&network_stats);
        assert_eq!(
            metrics.streaming_network_fec_repairs_total.get(),
            u64::from(network_stats.fec_repairs)
        );
        assert_eq!(
            metrics.streaming_network_fec_failures_total.get(),
            u64::from(network_stats.fec_failures)
        );
        assert_eq!(
            metrics.streaming_network_datagram_reinjects_total.get(),
            u64::from(network_stats.datagram_reinjects)
        );
    }

    #[cfg(feature = "telemetry")]
    fn record_sync_metrics(telemetry: &StreamingTelemetry, metrics: &Metrics) {
        let sync_diagnostics = SyncDiagnostics {
            window_ms: 500,
            samples: 96,
            avg_audio_jitter_ms: 6,
            max_audio_jitter_ms: 12,
            avg_av_drift_ms: -4,
            max_av_drift_ms: 15,
            ewma_av_drift_ms: -2,
            violation_count: 3,
        };
        telemetry.record_sync_diagnostics(&sync_diagnostics);
        assert_eq!(metrics.streaming_audio_jitter_ms.get_sample_count(), 1);
        assert_eq!(
            metrics.streaming_audio_max_jitter_ms.get(),
            u64::from(sync_diagnostics.max_audio_jitter_ms)
        );
        assert_eq!(metrics.streaming_av_drift_ms.get_sample_count(), 1);
        assert_eq!(
            metrics.streaming_av_max_drift_ms.get(),
            u64::from(sync_diagnostics.max_av_drift_ms)
        );
        assert_eq!(
            metrics.streaming_av_drift_ewma_ms.get(),
            i64::from(sync_diagnostics.ewma_av_drift_ms)
        );
        assert_eq!(
            metrics.streaming_av_sync_window_ms.get(),
            u64::from(sync_diagnostics.window_ms)
        );
        assert_eq!(
            metrics.streaming_av_sync_violation_total.get(),
            u64::from(sync_diagnostics.violation_count)
        );
        assert_eq!(metrics.streaming_network_rtt_ms.get_sample_count(), 1);
    }

    #[cfg(feature = "telemetry")]
    fn record_energy_metrics(telemetry: &StreamingTelemetry, metrics: &Metrics) {
        let energy_stats = TelemetryEnergyStats {
            segment: 7,
            encoder_milliwatts: 950,
            decoder_milliwatts: 870,
        };
        telemetry.record_energy_stats(&energy_stats);
        assert_eq!(metrics.streaming_energy_encoder_mw.get_sample_count(), 1);
        assert_eq!(metrics.streaming_energy_decoder_mw.get_sample_count(), 1);
    }

    #[test]
    #[cfg(feature = "telemetry")]
    fn streaming_telemetry_records_metrics() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = StreamingTelemetry::new(metrics.clone(), true);
        let metrics_ref: &Metrics = &metrics;

        record_encode_metrics(&telemetry, metrics_ref);
        record_decode_metrics(&telemetry, metrics_ref);
        record_network_metrics(&telemetry, metrics_ref);
        record_sync_metrics(&telemetry, metrics_ref);
        record_energy_metrics(&telemetry, metrics_ref);
    }

    #[test]
    fn state_telemetry_detached_counters_set() {
        let metrics = Arc::new(iroha_telemetry::metrics::Metrics::default());
        let st = StateTelemetry::new(metrics.clone(), true);
        st.set_pipeline_detached_prepared(LaneId::SINGLE, 3);
        st.set_pipeline_detached_merged(LaneId::SINGLE, 2);
        st.set_pipeline_detached_fallback(LaneId::SINGLE, 1);
        assert_eq!(metrics.pipeline_detached_prepared.get(), 3);
        assert_eq!(metrics.pipeline_detached_merged.get(), 2);
        assert_eq!(metrics.pipeline_detached_fallback.get(), 1);
    }

    #[test]
    fn state_telemetry_pipeline_access_sources_and_conflict_rate_set() {
        let metrics = Arc::new(Metrics::default());
        let st = StateTelemetry::new(metrics.clone(), true);

        st.inc_pipeline_access_set_source(AccessSetSource::ManifestHints, 2);
        st.inc_pipeline_access_set_source(AccessSetSource::PrepassMerge, 1);
        st.set_pipeline_conflict_rate_bps(LaneId::SINGLE, 250);

        assert_eq!(
            metrics
                .pipeline_access_set_source_total
                .with_label_values(&["manifest_hints"])
                .get(),
            2
        );
        assert_eq!(
            metrics
                .pipeline_access_set_source_total
                .with_label_values(&["prepass_merge"])
                .get(),
            1
        );
        assert_eq!(metrics.pipeline_conflict_rate_bps.get(), 250);
    }

    #[test]
    fn consensus_message_counters_update() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = Telemetry::new(metrics.clone(), true);

        let vote_hash = HashOf::<iroha_data_model::block::Header>::from_untyped_unchecked(
            Hash::prehashed([0x11; Hash::LENGTH]),
        );
        let vote = consensus::Vote {
            phase: consensus::Phase::Prevote,
            block_hash: vote_hash,
            height: 1,
            view: 1,
            epoch: 0,
            signer: 0,
            bls_sig: Vec::new(),
            signature: Vec::new(),
        };
        let vote_msg = BlockMessage::PrevoteVote(PrevoteVoteMsg(vote.clone()));
        telemetry.note_consensus_message_sent(&vote_msg);
        telemetry.note_consensus_message_received(&vote_msg);

        assert_eq!(
            metrics
                .sumeragi_votes_sent_total
                .with_label_values(&[super::PHASE_PREVOTE])
                .get(),
            1
        );
        assert_eq!(
            metrics
                .sumeragi_votes_received_total
                .with_label_values(&[super::PHASE_PREVOTE])
                .get(),
            1
        );

        let qc_hash = HashOf::<iroha_data_model::block::Header>::from_untyped_unchecked(
            Hash::prehashed([0x22; Hash::LENGTH]),
        );
        let qc = consensus::Qc {
            phase: consensus::Phase::Precommit,
            subject_block_hash: qc_hash,
            height: 2,
            view: 3,
            epoch: 0,
            aggregate: consensus::QcAggregate {
                signers_bitmap: vec![0x01],
                bls_aggregate_signature: Vec::new(),
            },
        };
        let qc_msg = BlockMessage::PrecommitQC(PrecommitQCMsg(qc.clone()));
        telemetry.note_consensus_message_sent(&qc_msg);
        telemetry.note_consensus_message_received(&qc_msg);

        assert_eq!(
            metrics
                .sumeragi_qc_sent_total
                .with_label_values(&[super::PHASE_PRECOMMIT])
                .get(),
            1
        );
        assert_eq!(
            metrics
                .sumeragi_qc_received_total
                .with_label_values(&[super::PHASE_PRECOMMIT])
                .get(),
            1
        );
    }

    #[test]
    fn nexus_teu_metrics_recorded() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(metrics.clone(), true);

        let lane_update = LaneTeuGaugeUpdate {
            capacity: 128,
            committed: 64,
            buckets: NexusLaneTeuBuckets {
                floor: 32,
                headroom: 32,
                must_serve: 0,
                circuit_breaker: 0,
            },
            trigger_level: 1,
            starvation_bound_slots: 42,
        };
        telemetry.record_nexus_scheduler_lane_teu(LaneId::SINGLE, lane_update);
        telemetry.inc_nexus_scheduler_lane_teu_deferral(LaneId::SINGLE, "cap_exceeded", 3);
        telemetry.inc_nexus_scheduler_must_serve_truncations(LaneId::SINGLE, 2);
        telemetry.record_nexus_scheduler_dataspace_teu(
            LaneId::SINGLE,
            DataSpaceId::GLOBAL,
            DataspaceTeuGaugeUpdate {
                backlog: 5,
                age_slots: 3,
                virtual_finish: 7,
            },
        );

        let lane_label = LaneId::SINGLE.as_u32().to_string();
        assert_eq!(
            metrics
                .nexus_scheduler_lane_teu_capacity
                .with_label_values(&[lane_label.as_str()])
                .get(),
            128
        );
        assert_eq!(
            metrics
                .nexus_scheduler_lane_trigger_level
                .with_label_values(&[lane_label.as_str()])
                .get(),
            1
        );
        for (bucket, value) in lane_update.buckets.iter() {
            assert_eq!(
                metrics
                    .nexus_scheduler_lane_teu_slot_breakdown
                    .with_label_values(&[lane_label.as_str(), bucket])
                    .get(),
                value
            );
        }
        assert_eq!(
            metrics
                .nexus_scheduler_lane_teu_deferral_total
                .with_label_values(&[lane_label.as_str(), "cap_exceeded"])
                .get(),
            3
        );
        assert_eq!(
            metrics
                .nexus_scheduler_must_serve_truncations_total
                .with_label_values(&[lane_label.as_str()])
                .get(),
            2
        );

        let lane_snapshots = metrics
            .nexus_scheduler_lane_teu_status
            .read()
            .expect("lane TEU cache poisoned");
        let snapshot = lane_snapshots
            .get(&LaneId::SINGLE.as_u32())
            .expect("lane snapshot missing");
        assert_eq!(snapshot.capacity, lane_update.capacity);
        assert_eq!(snapshot.committed, lane_update.committed);
        assert_eq!(snapshot.deferrals.cap_exceeded, 3);
        assert_eq!(snapshot.must_serve_truncations, 2);

        let ds_label = DataSpaceId::GLOBAL.as_u64().to_string();
        assert_eq!(
            metrics
                .nexus_scheduler_dataspace_teu_backlog
                .with_label_values(&[lane_label.as_str(), ds_label.as_str()])
                .get(),
            5
        );
        let ds_snapshots = metrics
            .nexus_scheduler_dataspace_teu_status
            .read()
            .expect("dataspace TEU cache poisoned");
        let ds_snapshot = ds_snapshots
            .get(&(LaneId::SINGLE.as_u32(), DataSpaceId::GLOBAL.as_u64()))
            .expect("dataspace snapshot missing");
        assert_eq!(ds_snapshot.backlog, 5);
        assert_eq!(ds_snapshot.age_slots, 3);
        assert_eq!(ds_snapshot.virtual_finish, 7);
    }

    #[test]
    fn nexus_lane_metrics_skipped_when_disabled() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(metrics.clone(), true);
        telemetry.set_nexus_enabled(false);

        telemetry.record_nexus_scheduler_lane_teu(
            LaneId::SINGLE,
            LaneTeuGaugeUpdate {
                capacity: 50,
                committed: 25,
                buckets: NexusLaneTeuBuckets {
                    floor: 10,
                    headroom: 40,
                    must_serve: 0,
                    circuit_breaker: 0,
                },
                trigger_level: 1,
                starvation_bound_slots: 4,
            },
        );
        telemetry.inc_nexus_scheduler_lane_teu_deferral(LaneId::SINGLE, "cap_exceeded", 2);
        telemetry.record_nexus_scheduler_dataspace_teu(
            LaneId::SINGLE,
            DataSpaceId::GLOBAL,
            DataspaceTeuGaugeUpdate {
                backlog: 9,
                age_slots: 3,
                virtual_finish: 11,
            },
        );
        telemetry.set_pipeline_layer_count(LaneId::SINGLE, 7);

        let lane_label = LaneId::SINGLE.as_u32().to_string();
        let ds_label = DataSpaceId::GLOBAL.as_u64().to_string();
        assert_eq!(
            metrics
                .nexus_scheduler_lane_teu_capacity
                .with_label_values(&[lane_label.as_str()])
                .get(),
            0
        );
        assert_eq!(
            metrics
                .nexus_scheduler_lane_teu_deferral_total
                .with_label_values(&[lane_label.as_str(), "cap_exceeded"])
                .get(),
            0
        );
        assert_eq!(
            metrics
                .nexus_scheduler_dataspace_teu_backlog
                .with_label_values(&[lane_label.as_str(), ds_label.as_str()])
                .get(),
            0
        );
        assert_eq!(metrics.pipeline_layer_count.get(), 0);
    }

    #[test]
    fn sumeragi_new_view_counters_and_highest_qc_gauge() {
        use std::sync::Arc;

        let metrics = Arc::new(Metrics::default());
        let tel = Telemetry::new(metrics.clone(), true);

        // Ensure initial values are zero
        assert_eq!(metrics.sumeragi_new_view_publish_total.get(), 0);
        assert_eq!(metrics.sumeragi_new_view_recv_total.get(), 0);
        assert_eq!(metrics.sumeragi_highest_qc_height.get(), 0);

        // Increment publish twice and receive three times
        tel.inc_new_view_publish();
        tel.inc_new_view_publish();
        tel.inc_new_view_recv();
        tel.inc_new_view_recv();
        tel.inc_new_view_recv();

        assert_eq!(metrics.sumeragi_new_view_publish_total.get(), 2);
        assert_eq!(metrics.sumeragi_new_view_recv_total.get(), 3);

        // Record per-(height,view) receipt count
        tel.set_new_view_receipts(7, 3, 5);
        assert_eq!(
            metrics
                .sumeragi_new_view_receipts_by_hv
                .with_label_values(&["7", "3"])
                .get(),
            5
        );

        // Set highest QC height
        tel.set_highest_qc_height(64);
        assert_eq!(metrics.sumeragi_highest_qc_height.get(), 64);
    }

    #[cfg(feature = "telemetry")]
    #[test]
    fn queue_backpressure_metrics_updated() {
        use std::sync::Arc;

        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(metrics.clone(), true);

        record_state_tx_queue_backpressure(&telemetry, 12, 64, false);
        assert_eq!(metrics.sumeragi_tx_queue_depth.get(), 12);
        assert_eq!(metrics.sumeragi_tx_queue_capacity.get(), 64);
        assert_eq!(metrics.sumeragi_tx_queue_saturated.get(), 0);

        record_state_tx_queue_backpressure(&telemetry, 63, 64, true);
        assert_eq!(metrics.sumeragi_tx_queue_depth.get(), 63);
        assert_eq!(metrics.sumeragi_tx_queue_capacity.get(), 64);
        assert_eq!(metrics.sumeragi_tx_queue_saturated.get(), 1);
    }

    #[cfg(feature = "telemetry")]
    #[test]
    fn prf_context_updates_metrics() {
        use std::sync::Arc;

        let metrics = Arc::new(Metrics::default());
        let telemetry = Telemetry::new(metrics.clone(), true);
        let seed = [0xAB; 32];

        telemetry.set_prf_context(Some(seed), 42, 3);
        assert_eq!(metrics.sumeragi_prf_height.get(), 42);
        assert_eq!(metrics.sumeragi_prf_view.get(), 3);
        let stored = metrics
            .sumeragi_prf_epoch_seed_hex
            .read()
            .expect("PRF seed lock poisoned")
            .clone();
        let expected = hex::encode(seed);
        assert_eq!(stored.as_deref(), Some(expected.as_str()));

        telemetry.set_prf_context(None, 0, 0);
        assert_eq!(metrics.sumeragi_prf_height.get(), 0);
        assert_eq!(metrics.sumeragi_prf_view.get(), 0);
        assert!(
            metrics
                .sumeragi_prf_epoch_seed_hex
                .read()
                .expect("PRF seed lock poisoned")
                .is_none()
        );
    }

    #[cfg(feature = "telemetry")]
    #[test]
    fn tiered_snapshot_metrics_updated() {
        use std::sync::Arc;

        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(metrics.clone(), true);

        record_state_tiered_snapshot(&telemetry, 7, 3, 2, 1024, 1, 2, 3, 2048, 4, 512);
        assert_eq!(metrics.state_tiered_last_snapshot_index.get(), 7);
        assert_eq!(metrics.state_tiered_hot_entries.get(), 3);
        assert_eq!(metrics.state_tiered_cold_entries.get(), 2);
        assert_eq!(metrics.state_tiered_cold_bytes.get(), 1024);
        assert_eq!(metrics.state_tiered_hot_promotions.get(), 1);
        assert_eq!(metrics.state_tiered_hot_demotions.get(), 2);
        assert_eq!(metrics.state_tiered_hot_grace_overflow_keys.get(), 3);
        assert_eq!(metrics.state_tiered_hot_grace_overflow_bytes.get(), 2048);
        assert_eq!(metrics.state_tiered_cold_reused_entries.get(), 4);
        assert_eq!(metrics.state_tiered_cold_reused_bytes.get(), 512);
    }

    #[cfg(feature = "telemetry")]
    #[test]
    fn governance_proposal_transition_updates_gauges() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(metrics.clone(), true);

        telemetry.record_governance_proposal_transition(
            None,
            crate::state::GovernanceProposalStatus::Proposed,
        );
        assert_eq!(
            metrics
                .governance_proposals_status
                .with_label_values(&["proposed"])
                .get(),
            1
        );

        telemetry.record_governance_proposal_transition(
            Some(crate::state::GovernanceProposalStatus::Proposed),
            crate::state::GovernanceProposalStatus::Approved,
        );
        assert_eq!(
            metrics
                .governance_proposals_status
                .with_label_values(&["proposed"])
                .get(),
            0
        );
        assert_eq!(
            metrics
                .governance_proposals_status
                .with_label_values(&["approved"])
                .get(),
            1
        );
    }

    #[cfg(feature = "telemetry")]
    #[test]
    fn governance_proposal_gauge_seeding_resets_counts() {
        use crate::state::GovernanceProposalStatus as GPS;

        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(metrics.clone(), true);

        telemetry.record_governance_proposal_transition(None, GPS::Proposed);
        telemetry.seed_governance_proposal_statuses([GPS::Approved, GPS::Approved, GPS::Rejected]);

        assert_eq!(
            metrics
                .governance_proposals_status
                .with_label_values(&["proposed"])
                .get(),
            0
        );
        assert_eq!(
            metrics
                .governance_proposals_status
                .with_label_values(&["approved"])
                .get(),
            2
        );
        assert_eq!(
            metrics
                .governance_proposals_status
                .with_label_values(&["rejected"])
                .get(),
            1
        );
        assert_eq!(
            metrics
                .governance_proposals_status
                .with_label_values(&["enacted"])
                .get(),
            0
        );
    }

    #[cfg(feature = "telemetry")]
    #[test]
    fn governance_events_drive_metrics_via_ingest() {
        use std::sync::Arc;

        use iroha_data_model::events::data::governance::{
            GovernanceEvent, GovernanceProposalApproved, GovernanceProposalEnacted,
        };

        use crate::state::GovernanceProposalStatus as GPS;

        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(metrics.clone(), true);
        let proposal_id = [0xAB; 32];

        telemetry.seed_governance_proposals([(proposal_id, GPS::Proposed)]);

        telemetry.ingest_data_event(&DataEvent::Governance(GovernanceEvent::ProposalApproved(
            GovernanceProposalApproved { id: proposal_id },
        )));
        assert_eq!(
            metrics
                .governance_proposals_status
                .with_label_values(&["proposed"])
                .get(),
            0
        );
        assert_eq!(
            metrics
                .governance_proposals_status
                .with_label_values(&["approved"])
                .get(),
            1
        );

        telemetry.ingest_data_event(&DataEvent::Governance(GovernanceEvent::ProposalEnacted(
            GovernanceProposalEnacted { id: proposal_id },
        )));
        assert_eq!(
            metrics
                .governance_proposals_status
                .with_label_values(&["approved"])
                .get(),
            0
        );
        assert_eq!(
            metrics
                .governance_proposals_status
                .with_label_values(&["enacted"])
                .get(),
            1
        );
    }

    #[cfg(feature = "telemetry")]
    #[test]
    fn council_persist_event_updates_gauges() {
        use std::sync::Arc;

        use iroha_data_model::{
            events::data::governance::{GovernanceCouncilPersisted, GovernanceEvent},
            isi::governance::CouncilDerivationKind,
        };

        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(metrics.clone(), true);

        telemetry.on_governance_event(&GovernanceEvent::CouncilPersisted(
            GovernanceCouncilPersisted {
                epoch: 4,
                members_count: 3,
                alternates_count: 1,
                verified: 2,
                candidates_count: 5,
                derived_by: CouncilDerivationKind::Vrf,
            },
        ));

        assert_eq!(metrics.governance_council_members.get(), 3);
        assert_eq!(metrics.governance_council_alternates.get(), 1);
        assert_eq!(metrics.governance_council_candidates.get(), 5);
        assert_eq!(metrics.governance_council_verified.get(), 2);
        assert_eq!(metrics.governance_council_epoch.get(), 4);
    }

    #[cfg(feature = "telemetry")]
    #[test]
    fn governance_bond_events_increment() {
        use std::sync::Arc;

        use iroha_data_model::events::data::governance::{
            GovernanceEvent, GovernanceLockCreated, GovernanceLockExtended, GovernanceLockUnlocked,
        };

        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(metrics.clone(), true);

        telemetry.ingest_data_event(&DataEvent::Governance(GovernanceEvent::LockCreated(
            GovernanceLockCreated {
                referendum_id: "ref".to_string(),
                owner: iroha_test_samples::ALICE_ID.clone(),
                amount: 10,
                expiry_height: 5,
            },
        )));
        telemetry.ingest_data_event(&DataEvent::Governance(GovernanceEvent::LockExtended(
            GovernanceLockExtended {
                referendum_id: "ref".to_string(),
                owner: iroha_test_samples::ALICE_ID.clone(),
                amount: 12,
                expiry_height: 6,
            },
        )));
        telemetry.ingest_data_event(&DataEvent::Governance(GovernanceEvent::LockUnlocked(
            GovernanceLockUnlocked {
                referendum_id: "ref".to_string(),
                owner: iroha_test_samples::ALICE_ID.clone(),
                amount: 12,
            },
        )));

        assert_eq!(
            metrics
                .governance_bond_events_total
                .with_label_values(&["lock_created"])
                .get(),
            1
        );
        assert_eq!(
            metrics
                .governance_bond_events_total
                .with_label_values(&["lock_extended"])
                .get(),
            1
        );
        assert_eq!(
            metrics
                .governance_bond_events_total
                .with_label_values(&["lock_unlocked"])
                .get(),
            1
        );
    }

    #[cfg(feature = "telemetry")]
    #[test]
    fn citizen_service_events_increment() {
        use std::sync::Arc;

        use iroha_data_model::isi::governance::CitizenServiceEvent;

        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(metrics.clone(), true);

        telemetry.record_citizen_service_event(CitizenServiceEvent::Decline, 0);
        telemetry.record_citizen_service_event(CitizenServiceEvent::Misconduct, 10);

        assert_eq!(
            metrics
                .governance_citizen_service_events_total
                .with_label_values(&["decline"])
                .get(),
            1
        );
        assert_eq!(
            metrics
                .governance_citizen_service_events_total
                .with_label_values(&["misconduct"])
                .get(),
            1
        );
    }

    #[test]
    fn protected_namespace_enforcement_and_manifest_activation_metrics() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(metrics.clone(), true);

        telemetry.record_protected_namespace_enforcement("allowed");
        telemetry.record_protected_namespace_enforcement("rejected");
        telemetry.record_manifest_quorum_enforcement("satisfied");
        telemetry.record_manifest_quorum_enforcement("rejected");
        assert_eq!(
            metrics
                .governance_protected_namespace_total
                .with_label_values(&["allowed"])
                .get(),
            1
        );
        assert_eq!(
            metrics
                .governance_protected_namespace_total
                .with_label_values(&["rejected"])
                .get(),
            1
        );
        assert_eq!(
            metrics
                .governance_manifest_quorum_total
                .with_label_values(&["satisfied"])
                .get(),
            1
        );
        assert_eq!(
            metrics
                .governance_manifest_quorum_total
                .with_label_values(&["rejected"])
                .get(),
            1
        );

        telemetry.record_manifest_activation(None, "manifest_inserted");
        let activation = GovernanceManifestActivation {
            namespace: "apps".to_string(),
            contract_id: "demo.contract".to_string(),
            code_hash_hex: "deadbeef".to_string(),
            abi_hash_hex: Some("cafebabe".to_string()),
            height: 9,
            activated_at_ms: 42,
        };
        telemetry.record_manifest_activation(Some(activation.clone()), "instance_bound");
        assert_eq!(
            metrics
                .governance_manifest_activations_total
                .with_label_values(&["manifest_inserted"])
                .get(),
            1
        );
        assert_eq!(
            metrics
                .governance_manifest_activations_total
                .with_label_values(&["instance_bound"])
                .get(),
            1
        );
        let recent = metrics
            .governance_manifest_recent
            .read()
            .expect("governance manifest cache lock poisoned");
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].namespace, activation.namespace);
    }

    impl SystemUnderTest {
        fn new() -> Self {
            let metrics = Arc::new(Metrics::default());

            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = Arc::new(State::with_telemetry(
                World::default(),
                kura.clone(),
                query_handle,
                StateTelemetry::new(metrics.clone(), true),
            ));
            let (peers_tx, peers_rx) = watch::channel(<_>::default());
            let (mock_time_handle, time_source) = TimeSource::new_mock(Duration::default());
            let queue = Arc::new(Queue::test(
                iroha_config::parameters::actual::Queue {
                    capacity: nonzero!(10usize),
                    capacity_per_user: nonzero!(10usize),
                    transaction_time_to_live: Duration::from_secs(100),
                },
                &time_source,
            ));

            let (telemetry, child) = start(
                metrics,
                state.clone(),
                kura.clone(),
                queue,
                peers_rx,
                time_source.clone(),
                true,
            );

            let chain_id = state.chain_id.clone();
            let (leader_public_key, leader_private_key) =
                KeyPair::random_with_algorithm(Algorithm::BlsNormal).into_parts();
            let peer_id = PeerId::new(leader_public_key);
            let topology = Topology::new(vec![peer_id]);
            let (account_id, account_keypair) = gen_account_in("wonderland");

            Self {
                telemetry,
                _child: child,
                mock_time_handle,
                time_source,
                kura,
                state,
                chain_id,
                online_peers_tx: peers_tx,
                account_id,
                account_keypair,
                topology,
                leader_private_key,
            }
        }

        fn create_block(&self) -> NewBlock {
            let (max_clock_drift, tx_limits) = {
                let params = &self.state.view().world.parameters;
                (params.sumeragi().max_clock_drift(), params.transaction())
            };

            let tx = TransactionBuilder::new_with_time_source(
                self.chain_id.clone(),
                self.account_id.clone(),
                &self.time_source,
            )
            .with_instructions([Log::new(Level::DEBUG, "meow".to_string())])
            .sign(self.account_keypair.private_key());
            let crypto_cfg = self.state.crypto();
            let tx = AcceptedTransaction::accept(
                tx,
                &self.chain_id,
                max_clock_drift,
                tx_limits,
                crypto_cfg.as_ref(),
            )
            .unwrap();
            BlockBuilder::new_with_time_source(vec![tx], self.time_source.clone())
                .chain(0, self.state.view().latest_block().as_deref())
                .sign(&self.leader_private_key)
                .unpack(|_| {})
        }

        fn commit_block(&self, block: NewBlock) -> CommittedBlock {
            let mut state_block = self.state.block(block.header());
            let block = block
                .validate_and_record_transactions(&mut state_block)
                .unpack(|_| {})
                .commit(&self.topology)
                .unpack(|_| {})
                .unwrap();
            let _events =
                state_block.apply_without_execution(&block, self.topology.as_ref().to_owned());
            state_block.commit().unwrap();

            self.kura
                .store_block(block.clone())
                .expect("store block for telemetry fixture");

            block
        }

        async fn report_commit_block(&self, block_header: &BlockHeader) {
            let handle = self.telemetry.clone();
            let header = *block_header;
            spawn_blocking(move || handle.report_block_commit_blocking(&header))
                .await
                .unwrap();
        }

        async fn force_sync(&self) {
            let _ = self.telemetry.metrics().await;
        }
    }

    fn random_peer(addr: SocketAddr) -> Peer {
        Peer::new(addr, KeyPair::random().public_key().clone())
    }

    #[tokio::test]
    async fn initial_metrics() {
        let sut = SystemUnderTest::new();

        let metrics = sut.telemetry.metrics().await;

        assert_eq!(metrics.block_height.get(), 0);
        assert_eq!(metrics.last_commit_time_ms.get(), 0);
        assert_eq!(metrics.domains.get(), 0);
        assert_eq!(metrics.connected_peers.get(), 0);
    }

    #[tokio::test]
    async fn telemetry_disabled_skips_updates() {
        use iroha_primitives::time::TimeSource;
        use tokio::sync::watch;
        // Build minimal components
        let metrics = Arc::new(Metrics::default());
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = Arc::new(State::with_telemetry(
            World::default(),
            kura.clone(),
            query,
            StateTelemetry::new(metrics.clone(), false),
        ));
        let (peers_tx, peers_rx) = watch::channel(<_>::default());
        let (_mh, time_source) = TimeSource::new_mock(Duration::default());
        let queue = Arc::new(Queue::test(
            iroha_config::parameters::actual::Queue {
                capacity: nonzero_ext::nonzero!(10usize),
                capacity_per_user: nonzero_ext::nonzero!(10usize),
                transaction_time_to_live: Duration::from_secs(100),
            },
            &time_source,
        ));

        let (tel, _child) = start(
            metrics.clone(),
            state,
            kura,
            queue,
            peers_rx,
            time_source,
            false, // disabled
        );
        let _ = peers_tx; // keep sender alive

        // Attempt to change metrics via Telemetry API
        tel.inc_dropped_messages();
        tel.set_view_changes(42);
        // Force a sync; actor should no-op when disabled
        let m = tel.metrics().await;
        assert_eq!(m.dropped_messages.get(), 0);
        assert_eq!(m.view_changes.get(), 0);
    }

    #[test]
    fn pacemaker_gauges_setters_update_metrics() {
        let metrics = Arc::new(Metrics::default());
        let tel = Telemetry::new(Arc::clone(&metrics), true);
        tel.set_pacemaker_round_elapsed_ms(111);
        tel.set_pacemaker_view_timeout_target_ms(222);
        tel.set_pacemaker_view_timeout_remaining_ms(33);
        assert_eq!(metrics.sumeragi_pacemaker_round_elapsed_ms.get(), 111);
        assert_eq!(metrics.sumeragi_pacemaker_view_timeout_target_ms.get(), 222);
        assert_eq!(
            metrics.sumeragi_pacemaker_view_timeout_remaining_ms.get(),
            33
        );
    }

    #[test]
    fn view_change_proof_metrics_increment() {
        let metrics = Arc::new(Metrics::default());
        let tel = Telemetry::new(Arc::clone(&metrics), true);
        tel.inc_view_change_proof_accepted();
        tel.inc_view_change_proof_stale();
        tel.inc_view_change_proof_rejected();
        assert_eq!(
            metrics
                .sumeragi_view_change_proof_total
                .with_label_values(&["accepted"])
                .get(),
            1
        );
        assert_eq!(
            metrics
                .sumeragi_view_change_proof_total
                .with_label_values(&["stale"])
                .get(),
            1
        );
        assert_eq!(
            metrics
                .sumeragi_view_change_proof_total
                .with_label_values(&["rejected"])
                .get(),
            1
        );
    }

    #[tokio::test]
    async fn ivm_cache_counters_exposed() {
        use ivm::ivm_cache::global_get_with_meta;
        // Arrange system and baseline
        let sut = SystemUnderTest::new();
        let stats0 = ivm::ivm_cache::global_stats();
        // Build tiny code (HALT) and matching metadata
        let mut code = Vec::new();
        code.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        let meta = ivm::ProgramMetadata::default();
        // Miss on first get, hit on second
        let _ = global_get_with_meta(&code, &meta).expect("predecode ok");
        let _ = global_get_with_meta(&code, &meta).expect("predecode hit");

        // Force telemetry sync
        sut.force_sync().await;
        let metrics = sut.telemetry.metrics().await;
        // Verify the counters are >= baseline + 1
        assert!(metrics.ivm_cache_misses.get() > stats0.misses);
        assert!(metrics.ivm_cache_hits.get() > stats0.hits);
        // Evictions may or may not have changed, but the gauge should be >= baseline
        assert!(metrics.ivm_cache_evictions.get() >= stats0.evictions);
        assert_eq!(
            metrics.ivm_cache_decoded_streams.get(),
            stats0.decoded_streams + 1,
            "expected exactly one new decoded stream"
        );
        assert_eq!(
            metrics.ivm_cache_decoded_ops_total.get(),
            stats0.decoded_ops_total + 1,
            "expected HALT decode to add one op"
        );
        assert!(
            metrics.ivm_cache_decode_time_ns_total.get() >= stats0.decode_time_ns_total,
            "decode time counter must not regress"
        );
        assert_eq!(
            metrics.ivm_cache_decode_failures.get(),
            stats0.decode_failures,
            "decode failures should not increase on successful decode"
        );
    }

    #[tokio::test]
    async fn sumeragi_backpressure_counters_increment() {
        use iroha_data_model::peer::PeerId;
        // Build telemetry with metrics enabled
        let metrics = std::sync::Arc::new(Metrics::default());
        let tel = Telemetry::new(metrics.clone(), true);
        // Fake peer id via random key
        let peer = PeerId::new(iroha_crypto::KeyPair::random().public_key().clone());
        tel.inc_post_to_peer(&peer);
        tel.inc_bg_post_enqueued("Post");
        tel.inc_bg_post_overflow("Post");
        tel.inc_bg_post_drop("Post");
        assert_eq!(
            metrics
                .sumeragi_post_to_peer_total
                .with_label_values(&[peer.to_string().as_str()])
                .get(),
            1
        );
        assert_eq!(
            metrics
                .sumeragi_bg_post_enqueued_total
                .with_label_values(&["Post"])
                .get(),
            1
        );
        assert_eq!(
            metrics
                .sumeragi_bg_post_overflow_total
                .with_label_values(&["Post"])
                .get(),
            1
        );
        assert_eq!(
            metrics
                .sumeragi_bg_post_drop_total
                .with_label_values(&["Post"])
                .get(),
            1
        );
    }

    #[tokio::test]
    async fn set_online_peers() {
        let sut = SystemUnderTest::new();
        let peers: HashSet<_> = vec![
            random_peer(socket_addr!(100.100.100.100:80)),
            random_peer(socket_addr!(200.100.30.1:9001)),
        ]
        .into_iter()
        .collect();

        sut.online_peers_tx.send(peers.clone()).unwrap();
        let metrics = sut.telemetry.metrics().await;

        assert_eq!(metrics.connected_peers.get(), 2);
        assert_eq!(
            metrics
                .p2p_peer_churn_total
                .with_label_values(&["connected"])
                .get(),
            2
        );
        assert_eq!(
            metrics
                .p2p_peer_churn_total
                .with_label_values(&["disconnected"])
                .get(),
            0
        );

        // Remove one peer and ensure churn metrics capture the disconnect event.
        let mut remaining = peers.clone();
        if let Some(to_remove) = remaining.iter().next().cloned() {
            remaining.remove(&to_remove);
        }
        sut.online_peers_tx.send(remaining.clone()).unwrap();
        let metrics = sut.telemetry.metrics().await;
        assert_eq!(metrics.connected_peers.get(), remaining.len() as u64);
        assert_eq!(
            metrics
                .p2p_peer_churn_total
                .with_label_values(&["connected"])
                .get(),
            2
        );
        assert_eq!(
            metrics
                .p2p_peer_churn_total
                .with_label_values(&["disconnected"])
                .get(),
            1
        );

        // Finally, drop all peers to observe another disconnect increment.
        sut.online_peers_tx.send(HashSet::new()).unwrap();
        let metrics = sut.telemetry.metrics().await;
        assert_eq!(metrics.connected_peers.get(), 0);
        assert_eq!(
            metrics
                .p2p_peer_churn_total
                .with_label_values(&["disconnected"])
                .get(),
            2
        );
    }

    #[tokio::test]
    async fn commit_blocks() {
        // this indicates time padding applied in the block builder
        const CORRECTION: u64 = 1;

        let sut = SystemUnderTest::new();

        // commit first (genesis) block
        let block = sut.create_block();
        sut.mock_time_handle.advance(Duration::from_millis(100));
        let block = sut.commit_block(block);
        sut.report_commit_block(&block.as_ref().header()).await;

        let metrics = sut.telemetry.metrics().await;
        assert_eq!(metrics.block_height.get(), 1);
        assert_eq!(metrics.block_height_non_empty.get(), 1);
        assert_eq!(metrics.last_commit_time_ms.get(), 0); // zero for genesis

        let first_accepted = metrics.txs.with_label_values(&["accepted"]).get();
        let first_rejected = metrics.txs.with_label_values(&["rejected"]).get();
        let total_first = metrics.txs.with_label_values(&["total"]).get();
        assert_eq!(first_accepted + first_rejected, total_first);
        assert!(total_first >= 1);

        // second block
        let block = sut.create_block();
        sut.mock_time_handle.advance(Duration::from_millis(150));
        let block = sut.commit_block(block);
        sut.report_commit_block(&block.as_ref().header()).await;

        let metrics = sut.telemetry.metrics().await;
        assert_eq!(metrics.block_height.get(), 2);
        assert_eq!(metrics.block_height_non_empty.get(), 2);
        assert_eq!(metrics.last_commit_time_ms.get(), 150 - CORRECTION);
        assert_eq!(
            metrics.slot_duration_ms_latest.get(),
            150 - CORRECTION,
            "slot-duration gauge should mirror last commit latency"
        );
        assert_eq!(
            metrics.slot_duration_ms.get_sample_count(),
            2,
            "slot-duration histogram should record each reported block"
        );
        let accepted = metrics.txs.with_label_values(&["accepted"]).get();
        let rejected = metrics.txs.with_label_values(&["rejected"]).get();
        let total = metrics.txs.with_label_values(&["total"]).get();
        assert_eq!(accepted + rejected, total);
        assert!(accepted >= 1, "at least one tx should be accepted");
        assert!(
            total > total_first,
            "tx counters should increase after second block"
        );

        // third block - committed, but not reported yet
        let block = sut.create_block();
        sut.mock_time_handle.advance(Duration::from_millis(170));
        let block = sut.commit_block(block);

        sut.report_commit_block(&block.as_ref().header()).await;

        let metrics = sut.telemetry.metrics().await;
        assert_eq!(metrics.block_height.get(), 3);
        assert_eq!(metrics.block_height_non_empty.get(), 3);
        assert_eq!(metrics.last_commit_time_ms.get(), 170 - CORRECTION);
        assert_eq!(metrics.slot_duration_ms_latest.get(), 170 - CORRECTION);
        assert_eq!(metrics.slot_duration_ms.get_sample_count(), 3);
    }

    #[tokio::test]
    async fn da_quorum_ratio_tracks_reschedules() {
        const CORRECTION: u64 = 1;
        let sut = SystemUnderTest::new();

        // First block: no reschedules, ratio should be 1.0.
        let block = sut.create_block();
        sut.mock_time_handle.advance(Duration::from_millis(100));
        let block = sut.commit_block(block);
        sut.report_commit_block(&block.as_ref().header()).await;
        let metrics = sut.telemetry.metrics().await;
        assert_eq!(metrics.last_commit_time_ms.get(), 0);
        assert!(
            (metrics.da_quorum_ratio.get() - 1.0).abs() < f64::EPSILON,
            "expected quorum ratio to start at 1.0"
        );

        // Second block: mark a DA reschedule before commit, ratio drops to 0.5.
        let block = sut.create_block();
        sut.mock_time_handle.advance(Duration::from_millis(150));
        sut.telemetry.inc_da_reschedule("npos");
        let block = sut.commit_block(block);
        sut.report_commit_block(&block.as_ref().header()).await;
        let metrics = sut.telemetry.metrics().await;
        assert_eq!(metrics.last_commit_time_ms.get(), 150 - CORRECTION);
        assert!(
            (metrics.da_quorum_ratio.get() - 0.5).abs() < 1e-9,
            "expected quorum ratio of 0.5 after one reschedule"
        );
        assert_eq!(
            metrics
                .sumeragi_rbc_da_reschedule_by_mode_total
                .with_label_values(&["npos"])
                .get(),
            1,
            "mode-tagged DA reschedule counter should increment"
        );
    }

    #[tokio::test]
    async fn rbc_abort_counter_tracks_mode_tag() {
        let sut = SystemUnderTest::new();

        sut.telemetry.inc_rbc_abort("npos");

        let metrics = sut.telemetry.metrics().await;
        assert_eq!(
            metrics
                .sumeragi_rbc_abort_total
                .with_label_values(&["npos"])
                .get(),
            1,
            "mode-tagged RBC abort counter should increment"
        );
    }

    #[tokio::test]
    async fn commit_pipeline_tick_metrics_track_outcome() {
        let sut = SystemUnderTest::new();
        let metrics = sut.telemetry.metrics().await;
        let baseline_idle = metrics
            .sumeragi_commit_pipeline_tick_total
            .with_label_values(&["i2", "idle"])
            .get();
        let baseline_active = metrics
            .sumeragi_commit_pipeline_tick_total
            .with_label_values(&["i2", "active"])
            .get();
        let baseline_status = status::commit_pipeline_tick_total();

        sut.telemetry.note_commit_pipeline_tick("i2", false);
        sut.telemetry.note_commit_pipeline_tick("i2", true);
        status::note_commit_pipeline_tick(ConsensusMode::Permissioned, true);

        let metrics = sut.telemetry.metrics().await;
        assert_eq!(
            metrics
                .sumeragi_commit_pipeline_tick_total
                .with_label_values(&["i2", "idle"])
                .get(),
            baseline_idle + 1,
            "idle outcome counter must advance"
        );
        assert_eq!(
            metrics
                .sumeragi_commit_pipeline_tick_total
                .with_label_values(&["i2", "active"])
                .get(),
            baseline_active + 1,
            "active outcome counter must advance"
        );
        assert_eq!(
            status::commit_pipeline_tick_total(),
            baseline_status + 1,
            "status counter should record only active tick runs"
        );
    }

    #[tokio::test]
    async fn prevote_timeout_metrics_track_mode() {
        let sut = SystemUnderTest::new();
        let guard = super::status::prevote_timeout_test_guard();
        super::status::reset_prevote_timeout_for_tests();
        drop(guard);
        let metrics = sut.telemetry.metrics().await;
        let baseline = metrics
            .sumeragi_prevote_timeout_total
            .with_label_values(&["i2"])
            .get();
        let status_baseline = super::status::prevote_timeout_total();

        sut.telemetry.inc_prevote_timeout("i2");
        super::status::inc_prevote_timeout();

        let metrics = sut.telemetry.metrics().await;
        assert_eq!(
            metrics
                .sumeragi_prevote_timeout_total
                .with_label_values(&["i2"])
                .get(),
            baseline + 1,
            "prevote timeout metric should increment for the mode tag"
        );
        assert_eq!(
            super::status::prevote_timeout_total(),
            status_baseline + 1,
            "status counter should track prevote timeouts"
        );
    }

    #[tokio::test]
    async fn prevote_timeout_metrics_track_npos_mode() {
        let sut = SystemUnderTest::new();
        let guard = super::status::prevote_timeout_test_guard();
        super::status::reset_prevote_timeout_for_tests();
        drop(guard);
        let metrics = sut.telemetry.metrics().await;
        let baseline = metrics
            .sumeragi_prevote_timeout_total
            .with_label_values(&["npos"])
            .get();
        let status_baseline = super::status::prevote_timeout_total();

        sut.telemetry.inc_prevote_timeout("npos");
        super::status::inc_prevote_timeout();

        let metrics = sut.telemetry.metrics().await;
        assert_eq!(
            metrics
                .sumeragi_prevote_timeout_total
                .with_label_values(&["npos"])
                .get(),
            baseline + 1,
            "prevote timeout metric should increment for NPoS mode"
        );
        assert_eq!(
            super::status::prevote_timeout_total(),
            status_baseline + 1,
            "status counter should track prevote timeouts in NPoS mode"
        );
    }

    #[tokio::test]
    async fn p2p_queue_drop_labels_reflect_counters() {
        let sut = SystemUnderTest::new();
        // Baseline
        let metrics = sut.telemetry.metrics().await;
        let drops = &metrics.p2p_queue_dropped_total;
        let baseline_high_post = drops.with_label_values(&["High", "Post"]).get();
        let baseline_low_post = drops.with_label_values(&["Low", "Post"]).get();
        let baseline_high_broadcast = drops.with_label_values(&["High", "Broadcast"]).get();
        let baseline_low_broadcast = drops.with_label_values(&["Low", "Broadcast"]).get();

        // Bump P2P counters directly
        iroha_p2p::network::inc_queue_drop_for_test(true, false, 3); // High/Post +3
        iroha_p2p::network::inc_queue_drop_for_test(false, false, 2); // Low/Post +2
        iroha_p2p::network::inc_queue_drop_for_test(true, true, 5); // High/Broadcast +5
        iroha_p2p::network::inc_queue_drop_for_test(false, true, 7); // Low/Broadcast +7

        // Force telemetry to sync metrics from p2p counters
        sut.force_sync().await;
        let metrics = sut.telemetry.metrics().await;
        let drops = &metrics.p2p_queue_dropped_total;
        assert!(drops.with_label_values(&["High", "Post"]).get() >= baseline_high_post + 3);
        assert!(drops.with_label_values(&["Low", "Post"]).get() >= baseline_low_post + 2);
        assert!(
            drops.with_label_values(&["High", "Broadcast"]).get() >= baseline_high_broadcast + 5
        );
        assert!(drops.with_label_values(&["Low", "Broadcast"]).get() >= baseline_low_broadcast + 7);
    }

    #[tokio::test]
    async fn p2p_overflow_label_matrix_reflects_counters() {
        use iroha_p2p::network::{inc_post_overflow_for_test as bump, message::Topic};
        let sut = SystemUnderTest::new();
        // Baseline read
        let metrics = sut.telemetry.metrics().await;
        let by = &metrics.p2p_post_overflow_by_topic;
        let mut baseline = BTreeMap::new();
        for prio in ["High", "Low"] {
            for topic in [
                "Consensus",
                "Control",
                "BlockSync",
                "TxGossip",
                "PeerGossip",
                "Health",
                "Other",
            ] {
                let v = by.with_label_values(&[prio, topic]).get();
                baseline.insert((prio.to_string(), topic.to_string()), v);
            }
        }

        // Bump P2P network counters for each pair by +1
        let topics = [
            Topic::Consensus,
            Topic::Control,
            Topic::BlockSync,
            Topic::TxGossip,
            Topic::PeerGossip,
            Topic::Health,
            Topic::Other,
        ];
        for &t in &topics {
            bump(true, t, 1);
            bump(false, t, 1);
        }

        // Force sync and assert increments >= 1 for each label pair
        sut.force_sync().await;
        let metrics = sut.telemetry.metrics().await;
        let by = &metrics.p2p_post_overflow_by_topic;
        for prio in ["High", "Low"] {
            for topic in [
                "Consensus",
                "Control",
                "BlockSync",
                "TxGossip",
                "PeerGossip",
                "Health",
                "Other",
            ] {
                let after = by.with_label_values(&[prio, topic]).get();
                let before = baseline
                    .get(&(prio.to_string(), topic.to_string()))
                    .copied()
                    .unwrap_or(0);
                assert!(
                    after > before,
                    "expected {prio}:{topic} to increase by at least 1"
                );
            }
        }
    }

    #[test]
    fn membership_mismatch_metrics_toggle() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = Telemetry::new(Arc::clone(&metrics), true);
        let peer_id = PeerId::new(KeyPair::random().public_key().clone());
        let peer_label = peer_id.to_string();

        let counter = metrics
            .sumeragi_membership_mismatch_total
            .with_label_values(&[peer_label.as_str(), "0", "0"]);
        assert_eq!(counter.get(), 0);

        telemetry.note_membership_mismatch(&peer_id, 0, 0);
        assert_eq!(counter.get(), 1);

        let gauge = metrics
            .sumeragi_membership_mismatch_active
            .with_label_values(&[peer_label.as_str()]);
        assert_eq!(gauge.get(), 1);

        telemetry.clear_membership_mismatch(&peer_id);
        assert_eq!(gauge.get(), 0);
    }

    #[test]
    fn membership_view_hash_metrics_update() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = Telemetry::new(Arc::clone(&metrics), true);
        let hash = [0xABu8; 32];
        telemetry.set_membership_view_hash(12, 3, 5, hash);
        assert_eq!(metrics.sumeragi_membership_height.get(), 12);
        assert_eq!(metrics.sumeragi_membership_view.get(), 3);
        assert_eq!(metrics.sumeragi_membership_epoch.get(), 5);
        let mut truncated = [0u8; 8];
        truncated.copy_from_slice(&hash[..8]);
        let expected = u64::from_be_bytes(truncated);
        assert_eq!(metrics.sumeragi_membership_view_hash.get(), expected);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn torii_pre_auth_metrics_track_usage() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = Telemetry::new(Arc::clone(&metrics), true);
        telemetry.inc_torii_pre_auth_reject("rate");
        assert_eq!(
            metrics
                .torii_pre_auth_reject_total
                .with_label_values(&["rate"])
                .get(),
            1
        );
        telemetry.inc_torii_operator_auth("gate", "allowed", "session");
        assert_eq!(
            metrics
                .torii_operator_auth_total
                .with_label_values(&["gate", "allowed", "session"])
                .get(),
            1
        );
        telemetry.inc_torii_operator_auth_lockout("gate", "invalid_session");
        assert_eq!(
            metrics
                .torii_operator_auth_lockout_total
                .with_label_values(&["gate", "invalid_session"])
                .get(),
            1
        );
        telemetry.inc_torii_active_conn("http");
        assert_eq!(
            metrics
                .torii_active_connections_total
                .with_label_values(&["http"])
                .get(),
            1
        );
        telemetry.dec_torii_active_conn("http");
        assert_eq!(
            metrics
                .torii_active_connections_total
                .with_label_values(&["http"])
                .get(),
            0
        );

        telemetry.inc_torii_signature_limit_reject(5, 3, "single");
        assert_eq!(metrics.torii_signature_limit_total.get(), 1);
        assert_eq!(
            metrics
                .torii_signature_limit_by_authority_total
                .with_label_values(&["single"])
                .get(),
            1
        );
        assert_eq!(metrics.torii_signature_limit_last_count.get(), 5);
        assert_eq!(metrics.torii_signature_limit_max.get(), 3);
        telemetry.inc_torii_nts_unhealthy_reject();
        assert_eq!(metrics.torii_nts_unhealthy_reject_total.get(), 1);
        telemetry.inc_torii_multisig_direct_sign_reject();
        assert_eq!(metrics.torii_multisig_direct_sign_reject_total.get(), 1);
        telemetry.inc_torii_attachment_reject("type");
        assert_eq!(
            metrics
                .torii_attachment_reject_total
                .with_label_values(&["type"])
                .get(),
            1
        );
        telemetry.observe_torii_attachment_sanitize_ms(12);
        assert_eq!(
            metrics
                .torii_attachment_sanitize_ms
                .with_label_values::<&str>(&[])
                .get_sample_count(),
            1
        );
        telemetry.disable();
        telemetry.inc_torii_signature_limit_reject(10, 7, "single");
        assert_eq!(
            metrics.torii_signature_limit_total.get(),
            1,
            "disabled telemetry must not record additional signature-limit increments"
        );
        assert_eq!(
            metrics
                .torii_signature_limit_by_authority_total
                .with_label_values(&["single"])
                .get(),
            1,
            "disabled telemetry must not record labeled signature-limit increments"
        );
        assert_eq!(
            metrics.torii_signature_limit_last_count.get(),
            5,
            "disabled telemetry must not update the last-count gauge"
        );
        telemetry.inc_torii_nts_unhealthy_reject();
        assert_eq!(
            metrics.torii_nts_unhealthy_reject_total.get(),
            1,
            "disabled telemetry must not record additional NTS rejects"
        );
        telemetry.inc_torii_multisig_direct_sign_reject();
        assert_eq!(
            metrics.torii_multisig_direct_sign_reject_total.get(),
            1,
            "disabled telemetry must not record additional direct-sign rejects"
        );
        telemetry.inc_torii_operator_auth("gate", "denied", "missing_session");
        assert_eq!(
            metrics
                .torii_operator_auth_total
                .with_label_values(&["gate", "allowed", "session"])
                .get(),
            1,
            "disabled telemetry must not record operator auth events"
        );
        telemetry.inc_torii_operator_auth_lockout("gate", "missing_session");
        assert_eq!(
            metrics
                .torii_operator_auth_lockout_total
                .with_label_values(&["gate", "invalid_session"])
                .get(),
            1,
            "disabled telemetry must not record operator auth lockouts"
        );
        telemetry.inc_torii_attachment_reject("type");
        assert_eq!(
            metrics
                .torii_attachment_reject_total
                .with_label_values(&["type"])
                .get(),
            1,
            "disabled telemetry must not record attachment rejects"
        );
        telemetry.observe_torii_attachment_sanitize_ms(7);
        assert_eq!(
            metrics
                .torii_attachment_sanitize_ms
                .with_label_values::<&str>(&[])
                .get_sample_count(),
            1,
            "disabled telemetry must not record attachment sanitize latency"
        );
    }

    #[test]
    fn multisig_direct_sign_rejection_metrics_recorded() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(Arc::clone(&metrics), true);

        record_social_rejection(&telemetry, "multisig_direct_sign");
        assert_eq!(
            metrics
                .social_rejections_total
                .with_label_values(&["multisig_direct_sign"])
                .get(),
            1
        );
        assert_eq!(metrics.multisig_direct_sign_reject_total.get(), 1);

        record_social_rejection(&telemetry, "other_reason");
        assert_eq!(
            metrics
                .social_rejections_total
                .with_label_values(&["other_reason"])
                .get(),
            1
        );
        assert_eq!(
            metrics.multisig_direct_sign_reject_total.get(),
            1,
            "counter should not increment for unrelated social rejections"
        );
    }

    #[test]
    fn sm_syscall_counters_respect_enablement() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(Arc::clone(&metrics), true);

        telemetry.note_sm_syscall_success("hash", "-");
        assert_eq!(
            metrics
                .sm_syscall_total
                .with_label_values(&["hash", "-"])
                .get(),
            1
        );

        telemetry.note_sm_syscall_failure("hash", "-", "disabled");
        assert_eq!(
            metrics
                .sm_syscall_failures_total
                .with_label_values(&["hash", "-", "disabled"])
                .get(),
            1
        );

        telemetry.disable();
        telemetry.note_sm_syscall_success("hash", "-");
        assert_eq!(
            metrics
                .sm_syscall_total
                .with_label_values(&["hash", "-"])
                .get(),
            1,
            "disabled telemetry must not record additional successes"
        );
    }

    #[test]
    fn nexus_lane_metrics_reset_when_disabled() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(Arc::clone(&metrics), true);

        metrics
            .nexus_lane_block_height
            .with_label_values(&["0", "global"])
            .set(5);
        metrics
            .nexus_scheduler_lane_teu_capacity
            .with_label_values(&["0"])
            .set(10);
        metrics
            .nexus_public_lane_validator_total
            .with_label_values(&["0", "active"])
            .set(2);

        telemetry.set_nexus_enabled(false);

        assert_eq!(metrics.nexus_lane_configured_total.get(), 0);
        assert!(
            metrics.nexus_lane_block_height.collect().is_empty(),
            "lane block height metrics should reset when Nexus is disabled"
        );
        assert!(
            metrics
                .nexus_scheduler_lane_teu_capacity
                .collect()
                .is_empty(),
            "scheduler lane metrics should reset when Nexus is disabled"
        );
        assert!(
            metrics
                .nexus_public_lane_validator_total
                .collect()
                .is_empty(),
            "public-lane validator metrics should reset when Nexus is disabled"
        );
        assert!(
            telemetry
                .lane_metadata
                .read()
                .expect("lane metadata lock")
                .is_empty()
        );
        assert!(
            telemetry
                .dataspace_metadata
                .read()
                .expect("dataspace metadata lock")
                .is_empty()
        );
        assert!(
            telemetry
                .metrics
                .nexus_lane_governance_sealed_aliases
                .read()
                .expect("lane alias cache lock")
                .is_empty()
        );
        assert!(
            telemetry
                .metrics
                .nexus_scheduler_lane_teu_status
                .read()
                .expect("lane TEU status cache lock")
                .is_empty()
        );
    }

    #[test]
    fn settlement_event_counters_track_outcomes() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(Arc::clone(&metrics), true);
        telemetry.note_settlement_success("dvp");
        assert_eq!(
            metrics
                .settlement_events_total
                .with_label_values(&["dvp", "success", "-"])
                .get(),
            1
        );
        telemetry.note_settlement_failure("pvp", "insufficient_funds");
        assert_eq!(
            metrics
                .settlement_events_total
                .with_label_values(&["pvp", "failure", "insufficient_funds"])
                .get(),
            1
        );
        telemetry.disable();
        telemetry.note_settlement_success("dvp");
        telemetry.note_settlement_failure("pvp", "insufficient_funds");
        assert_eq!(
            metrics
                .settlement_events_total
                .with_label_values(&["dvp", "success", "-"])
                .get(),
            1,
            "disabled telemetry must not record additional successes"
        );
        assert_eq!(
            metrics
                .settlement_events_total
                .with_label_values(&["pvp", "failure", "insufficient_funds"])
                .get(),
            1,
            "disabled telemetry must not record additional failures"
        );
    }

    #[test]
    fn settlement_finality_updates_metrics_and_status() {
        crate::sumeragi::status::settlement_status_reset_for_tests();
        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(Arc::clone(&metrics), true);
        let dvp_id: SettlementId = "trade-1".parse().expect("settlement id");
        let dvp_plan = SettlementPlan::new(
            SettlementExecutionOrder::DeliveryThenPayment,
            SettlementAtomicity::CommitFirstLeg,
        );
        telemetry.record_dvp_finality(
            &dvp_id,
            dvp_plan,
            SettlementOutcomeKind::Success,
            None,
            true,
            false,
        );
        assert_eq!(
            metrics
                .settlement_finality_events_total
                .with_label_values(&["dvp", "success", "delivery_only"])
                .get(),
            1
        );

        let pvp_id: SettlementId = "fx-1".parse().expect("settlement id");
        let pvp_plan = SettlementPlan::new(
            SettlementExecutionOrder::DeliveryThenPayment,
            SettlementAtomicity::CommitFirstLeg,
        );
        telemetry.record_pvp_finality(
            &pvp_id,
            pvp_plan,
            SettlementOutcomeKind::Failure,
            Some("insufficient_funds"),
            true,
            false,
            Some(42),
        );
        assert_eq!(
            metrics
                .settlement_finality_events_total
                .with_label_values(&["pvp", "failure", "primary_only"])
                .get(),
            1
        );
        assert_eq!(
            metrics
                .settlement_fx_window_ms
                .with_label_values(&["pvp", "delivery_then_payment", "commit_first_leg"])
                .get_sample_count(),
            1
        );

        let snapshot = crate::sumeragi::status::settlement_snapshot();
        assert_eq!(snapshot.dvp.success_total, 1);
        let dvp_event = snapshot.dvp.last_event.expect("dvp last event");
        assert_eq!(dvp_event.final_state_label, "delivery_only");
        assert_eq!(dvp_event.outcome, SettlementOutcomeKind::Success);
        assert!(dvp_event.delivery_committed);
        assert!(!dvp_event.payment_committed);

        assert_eq!(snapshot.pvp.failure_total, 1);
        let pvp_event = snapshot.pvp.last_event.expect("pvp last event");
        assert_eq!(pvp_event.final_state_label, "primary_only");
        assert_eq!(pvp_event.outcome, SettlementOutcomeKind::Failure);
        assert!(pvp_event.primary_committed);
        assert!(!pvp_event.counter_committed);
        assert_eq!(pvp_event.fx_window_ms, Some(42));
        assert_eq!(
            pvp_event.failure_reason.as_deref(),
            Some("insufficient_funds")
        );
    }

    #[test]
    #[cfg(feature = "telemetry")]
    fn lane_settlement_backlog_updates_metrics() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(metrics.clone(), true);
        let lane_catalog = LaneCatalog::new(
            nonzero!(1_u32),
            vec![LaneConfig {
                id: LaneId::new(0),
                dataspace_id: DataSpaceId::new(5),
                alias: "exec".to_string(),
                visibility: LaneVisibility::Public,
                storage: LaneStorageProfile::FullReplica,
                ..LaneConfig::default()
            }],
        )
        .expect("lane catalog");
        telemetry.set_nexus_catalogs(&lane_catalog, &DataSpaceCatalog::default());
        telemetry.record_lane_settlement_snapshot_metrics(
            LaneId::new(0),
            DataSpaceId::new(5),
            1_500_000,
            0,
            0,
            None,
            None,
        );
        let lane_label = LaneId::new(0).as_u32().to_string();
        let dataspace_label = DataSpaceId::new(5).as_u64().to_string();
        let backlog = metrics
            .nexus_lane_settlement_backlog_xor
            .with_label_values(&[lane_label.as_str(), dataspace_label.as_str()])
            .get();
        assert!(
            (backlog - 1_500_000.0).abs() < f64::EPSILON,
            "expected backlog gauge to track XOR micro value"
        );
        let guard = metrics
            .nexus_scheduler_lane_teu_status
            .read()
            .expect("lane TEU cache");
        let snapshot = guard
            .get(&LaneId::new(0).as_u32())
            .expect("lane snapshot missing");
        assert_eq!(snapshot.settlement_backlog_xor_micro, 1_500_000);
    }

    #[test]
    #[cfg(feature = "telemetry")]
    fn public_lane_metrics_track_validator_and_amounts() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(metrics.clone(), true);
        let lane = LaneId::new(5);

        telemetry.record_public_lane_validator_status(
            lane,
            None,
            &PublicLaneValidatorStatus::PendingActivation(0),
        );
        assert_eq!(
            metrics
                .nexus_public_lane_validator_total
                .with_label_values(&["5", "pending"])
                .get(),
            1
        );
        telemetry.record_public_lane_validator_status(
            lane,
            Some(&PublicLaneValidatorStatus::PendingActivation(0)),
            &PublicLaneValidatorStatus::Active,
        );
        assert_eq!(
            metrics
                .nexus_public_lane_validator_total
                .with_label_values(&["5", "pending"])
                .get(),
            0
        );
        assert_eq!(
            metrics
                .nexus_public_lane_validator_total
                .with_label_values(&["5", "active"])
                .get(),
            1
        );

        telemetry.increase_public_lane_bonded(lane, &Numeric::new(1_000, 0));
        telemetry.decrease_public_lane_bonded(lane, &Numeric::new(250, 0));
        let bonded = metrics
            .nexus_public_lane_stake_bonded
            .with_label_values(&["5"])
            .get();
        assert!(
            (bonded - 750.0).abs() < f64::EPSILON,
            "bonded stake gauge should track deltas"
        );

        telemetry.increase_public_lane_pending_unbond(lane, &Numeric::new(400, 0));
        telemetry.decrease_public_lane_pending_unbond(lane, &Numeric::new(150, 0));
        let pending = metrics
            .nexus_public_lane_unbond_pending
            .with_label_values(&["5"])
            .get();
        assert!(
            (pending - 250.0).abs() < f64::EPSILON,
            "pending unbond gauge should track deltas"
        );

        telemetry.record_public_lane_reward(lane, &Numeric::new(100, 0));
        telemetry.record_public_lane_reward(lane, &Numeric::new(50, 0));
        let rewards = metrics
            .nexus_public_lane_reward_total
            .with_label_values(&["5"])
            .get();
        assert!(
            (rewards - 150.0).abs() < f64::EPSILON,
            "reward gauge should accumulate"
        );

        telemetry.record_public_lane_slash(lane);
        assert_eq!(
            metrics
                .nexus_public_lane_slash_total
                .with_label_values(&["5"])
                .get(),
            1
        );
    }

    #[test]
    fn torii_request_scheme_metrics_capture_latency_and_failures() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = Telemetry::new(metrics.clone(), true);

        telemetry.observe_torii_request_by_scheme(
            "norito_rpc",
            StatusCode::OK,
            Duration::from_millis(25),
        );
        assert_eq!(
            metrics
                .torii_request_duration_seconds
                .with_label_values(&["norito_rpc"])
                .get_sample_count(),
            1,
            "latency histogram should capture successful request"
        );
        assert_eq!(
            metrics
                .torii_request_failures_total
                .with_label_values(&["norito_rpc", "500"])
                .get(),
            0,
            "successes should not increment failure counters"
        );

        telemetry.observe_torii_request_by_scheme(
            "norito_rpc",
            StatusCode::INTERNAL_SERVER_ERROR,
            Duration::from_millis(10),
        );
        assert_eq!(
            metrics
                .torii_request_duration_seconds
                .with_label_values(&["norito_rpc"])
                .get_sample_count(),
            2,
            "latency histogram should record every observation"
        );
        assert_eq!(
            metrics
                .torii_request_failures_total
                .with_label_values(&["norito_rpc", "500"])
                .get(),
            1,
            "5xx responses must increment failure counters"
        );
    }

    #[test]
    fn genesis_commit_time_is_zero() {
        let (time_handle, time_source) = TimeSource::new_mock(Duration::from_millis(1500));
        let header = BlockBuilder::new_with_time_source(vec![], time_source.clone())
            .chain(1, None)
            .sign(KeyPair::random().private_key())
            .unpack(|_| {})
            .header();

        time_handle.advance(Duration::from_secs(12));
        let report = BlockCommitReport::new(&header, &time_source);

        assert_eq!(report.commit_time, Duration::ZERO)
    }

    #[test]
    fn block_payload_detects_transaction_blocks() {
        let block = block_with_transactions(2);
        assert!(block_counts_as_non_empty(&block));
    }

    #[test]
    fn block_payload_flags_genesis_without_transactions() {
        let block = empty_block(1);
        assert!(block_counts_as_non_empty(&block));
    }

    #[test]
    fn block_payload_rejects_non_genesis_empty_block() {
        let block = empty_block(2);
        assert!(!block_counts_as_non_empty(&block));
    }

    fn empty_block(height: u64) -> iroha_data_model::block::SignedBlock {
        use std::num::NonZeroU64;

        use iroha_crypto::SignatureOf;
        use iroha_data_model::block::{BlockHeader, BlockSignature};

        let header = BlockHeader::new(
            NonZeroU64::new(height).expect("height must be > 0"),
            None,
            None,
            None,
            0,
            0,
        );
        let signer = KeyPair::random();
        let signature = BlockSignature::new(0, SignatureOf::new(signer.private_key(), &header));

        iroha_data_model::block::SignedBlock::presigned(signature, header, Vec::new())
    }

    fn block_with_transactions(height: u64) -> iroha_data_model::block::SignedBlock {
        use std::num::NonZeroU64;

        use iroha_crypto::SignatureOf;
        use iroha_data_model::{
            ChainId, DomainId,
            block::{BlockHeader, BlockSignature},
            transaction::signed::SignedTransaction,
        };

        fn dummy_transaction() -> SignedTransaction {
            let chain_id: ChainId = "test-chain".parse().expect("chain id");
            let domain_id: DomainId = "wonderland".parse().expect("domain id");
            let key_pair = KeyPair::random();
            let authority = AccountId::new(domain_id, key_pair.public_key().clone());
            TransactionBuilder::new(chain_id, authority).sign(key_pair.private_key())
        }

        let header = BlockHeader::new(
            NonZeroU64::new(height).expect("height must be > 0"),
            None,
            None,
            None,
            0,
            0,
        );
        let signer = KeyPair::random();
        let signature = BlockSignature::new(0, SignatureOf::new(signer.private_key(), &header));
        let tx = dummy_transaction();

        iroha_data_model::block::SignedBlock::presigned(signature, header, vec![tx])
    }
}
