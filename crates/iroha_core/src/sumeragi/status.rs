//! Process-local operator diagnostics for Sumeragi v2 and Nexus lanes.
//!
//! Consensus state itself is published exclusively as the exact reducer-owned
//! [`SumeragiV2Status`]. The remaining snapshots in this module are
//! non-consensus Nexus economics, settlement, lane, and adapter diagnostics.

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
#[cfg(test)]
use std::sync::Condvar;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::{Mutex, MutexGuard, OnceLock},
};

use iroha_crypto::{
    Hash, Hash as UntypedHash, HashOf,
    privacy::{CommitmentScheme, LanePrivacyCommitment},
};
use iroha_data_model::{
    block::{
        BlockHeader,
        consensus::{
            COMMITTED_LANE_STATUS_APPLICATION_RECEIPT_CONFLICTS_WITH_PREFLIGHT,
            COMMITTED_LANE_STATUS_AWAITING_EXECUTABLE_PAYLOAD,
            COMMITTED_LANE_STATUS_AWAITING_PREDECESSOR_APPLICATION,
            COMMITTED_LANE_STATUS_PAYLOAD_AVAILABLE_AWAITING_EXECUTOR,
            COMMITTED_LANE_STATUS_PAYLOAD_PREFLIGHT_REJECTED_AWAITING_STATE_APPLICATION,
            COMMITTED_LANE_STATUS_PAYLOAD_PREFLIGHTED_AWAITING_STATE_APPLICATION,
            COMMITTED_LANE_STATUS_PAYLOAD_RECOVERED_AWAITING_STATE_APPLICATION,
            COMMITTED_LANE_STATUS_STATE_APPLIED_BY_CANONICAL_BLOCK,
            COMMITTED_LANE_STATUS_STATE_APPLIED_BY_DIRECT_EXECUTION, LaneBlockCommitment,
            LaneBlockProposalV1, LaneBlockQcV1, SumeragiLaneBlockSessionStatus,
            SumeragiLanePayloadOwnership,
        },
        consensus_v2::SumeragiV2Status,
    },
    consensus::ConsensusKeyRecord,
    da::commitment::DaCommitmentBundle,
    isi::settlement::{SettlementAtomicity, SettlementExecutionOrder},
    nexus::{DataSpaceId, LaneId, LaneRelayEnvelope, LaneRelayError},
};
use iroha_primitives::numeric::Numeric;
use iroha_telemetry::metrics;
use norito::codec::{Decode, Encode};

use crate::{
    governance::manifest::{GovernanceRules, LaneManifestStatus, RuntimeUpgradeHook},
    queue::{BackpressureState, QueuePressureSnapshot},
};

static SUMERAGI_V2_STATUS: OnceLock<Mutex<Option<SumeragiV2Status>>> = OnceLock::new();

/// Publish the exact protocol-v2 reducer snapshot served by Torii.
pub fn set_v2_status(status: SumeragiV2Status) {
    *SUMERAGI_V2_STATUS
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(status);
}

/// Return the latest protocol-v2 reducer snapshot, if v2 has started.
#[must_use]
pub fn v2_status() -> Option<SumeragiV2Status> {
    SUMERAGI_V2_STATUS.get().and_then(|slot| {
        slot.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    })
}

/// Clear protocol-v2 status during shutdown and isolated tests.
pub fn clear_v2_status() {
    if let Some(slot) = SUMERAGI_V2_STATUS.get() {
        *slot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    }
}

/// Legacy lane-RBC mismatch labels retained only by lane-local telemetry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RbcMismatchKind {
    /// Chunk digest does not match the declared digest list.
    ChunkDigest,
    /// Payload hash does not match the expected value.
    PayloadHash,
    /// Merkle root for chunk digests does not match the expected root.
    ChunkRoot,
}

impl RbcMismatchKind {
    /// Stable telemetry label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ChunkDigest => "chunk_digest",
            Self::PayloadHash => "payload_hash",
            Self::ChunkRoot => "chunk_root",
        }
    }
}

fn lock_operator_status_slot<T>(
    slot: &'static Mutex<T>,
    label: &'static str,
) -> MutexGuard<'static, T> {
    match slot.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            iroha_logger::warn!(
                "Sumeragi {label} mutex was poisoned; recovering operator status snapshot"
            );
            poisoned.into_inner()
        }
    }
}

static SETTLEMENT_STATUS: OnceLock<Mutex<SettlementStatusState>> = OnceLock::new();
static LANE_ACTIVITY: OnceLock<Mutex<Vec<LaneActivitySnapshot>>> = OnceLock::new();
static PIPELINE_EXECUTION: OnceLock<Mutex<PipelineExecutionSnapshot>> = OnceLock::new();
static ACCESS_SET_SOURCES: OnceLock<Mutex<AccessSetSourceSummary>> = OnceLock::new();
static DATASPACE_ACTIVITY: OnceLock<Mutex<Vec<DataspaceActivitySnapshot>>> = OnceLock::new();
static LANE_COMMITMENTS: OnceLock<Mutex<Vec<LaneCommitmentSnapshot>>> = OnceLock::new();
static DATASPACE_COMMITMENTS: OnceLock<Mutex<Vec<DataspaceCommitmentSnapshot>>> = OnceLock::new();
static LANE_SETTLEMENT_COMMITMENTS: OnceLock<Mutex<Vec<LaneBlockCommitment>>> = OnceLock::new();
static LANE_RELAY_ENVELOPES: OnceLock<Mutex<Vec<LaneRelayEnvelope>>> = OnceLock::new();
static LANE_PAYLOAD_OWNERSHIPS: OnceLock<Mutex<Vec<SumeragiLanePayloadOwnership>>> =
    OnceLock::new();
static COMMITTED_LANE_BLOCKS: OnceLock<Mutex<Vec<CommittedLaneBlockSnapshot>>> = OnceLock::new();
static LANE_BLOCK_SESSIONS: OnceLock<Mutex<Vec<SumeragiLaneBlockSessionStatus>>> = OnceLock::new();
static LANE_GOVERNANCE: OnceLock<Mutex<Vec<LaneGovernanceSnapshot>>> = OnceLock::new();
static NEXUS_FEE_STATUS: OnceLock<Mutex<NexusFeeSnapshot>> = OnceLock::new();
static NEXUS_STAKING_STATUS: OnceLock<Mutex<BTreeMap<LaneId, NexusStakingLaneSnapshot>>> =
    OnceLock::new();
static PIPELINE_CONFLICT_RATE_BPS: AtomicU64 = AtomicU64::new(0);
static TX_QUEUE_DEPTH: AtomicU64 = AtomicU64::new(0);
static TX_QUEUE_CAPACITY: AtomicU64 = AtomicU64::new(0);
static TX_QUEUE_RETAINED_BYTES: AtomicU64 = AtomicU64::new(0);
static TX_QUEUE_MAX_RETAINED_BYTES: AtomicU64 = AtomicU64::new(0);
static TX_QUEUE_SATURATED: AtomicBool = AtomicBool::new(false);
static TX_QUEUE_SATURATED_BY_COUNT: AtomicBool = AtomicBool::new(false);
static TX_QUEUE_SATURATED_BY_BYTES: AtomicBool = AtomicBool::new(false);
static TX_QUEUE_SATURATED_BY_AGE: AtomicBool = AtomicBool::new(false);
static TX_QUEUE_OLDEST_QUEUED_AGE_MS: AtomicU64 = AtomicU64::new(0);

const LANE_RELAY_ENVELOPES_CAP: usize = 64;
const LANE_PAYLOAD_OWNERSHIPS_CAP: usize = 128;
const COMMITTED_LANE_BLOCKS_CAP: usize = 128;

/// Actor responsible for paying a Nexus fee.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NexusFeePayer {
    /// Transaction authority paid the fee.
    Payer,
    /// A sponsor covered the fee.
    Sponsor,
}

/// Aggregated Nexus fee debit outcomes for status/telemetry surfacing.
#[derive(Clone, Debug, Default)]
pub struct NexusFeeSnapshot {
    /// Total fee debits applied successfully.
    pub charged_total: u64,
    /// Successful debits that used the payer account.
    pub charged_via_payer_total: u64,
    /// Successful debits that used a sponsor account.
    pub charged_via_sponsor_total: u64,
    /// Rejections because sponsorship was disabled.
    pub sponsor_disabled_total: u64,
    /// Rejections because the sponsor did not authorize the payer.
    pub sponsor_unauthorized_total: u64,
    /// Rejections because the fee exceeded `sponsor_max_fee`.
    pub sponsor_cap_exceeded_total: u64,
    /// Failures due to config/asset parsing errors.
    pub config_errors_total: u64,
    /// Failures while executing the fee debit.
    pub transfer_failures_total: u64,
    /// Last attempted fee amount if available.
    pub last_amount: Option<Numeric>,
    /// Asset definition id used for the last attempt.
    pub last_asset_id: Option<String>,
    /// Payer classification for the last attempt.
    pub last_payer: Option<NexusFeePayer>,
    /// Account id string for the last attempt.
    pub last_payer_id: Option<String>,
    /// Most recent error message (if any).
    pub last_error: Option<String>,
}

/// Outcome emitted when attempting to debit Nexus fees.
#[derive(Clone, Debug)]
pub enum NexusFeeEvent {
    /// Fee charged successfully.
    Charged {
        /// Whether payer or sponsor covered the fee.
        payer_kind: NexusFeePayer,
        /// Account id that paid.
        payer_id: String,
        /// Amount charged.
        amount: Numeric,
        /// Asset definition id string.
        asset_id: String,
    },
    /// Sponsorship was disabled.
    SponsorDisabled {
        /// Account attempting to sponsor the fee.
        payer_id: String,
    },
    /// Sponsor did not authorize the payer.
    SponsorUnauthorized {
        /// Sponsor account that was requested.
        sponsor_id: String,
        /// Transaction authority that attempted to use the sponsor.
        authority_id: String,
    },
    /// Sponsorship exceeded configured cap.
    SponsorCapExceeded {
        /// Account that attempted to sponsor.
        payer_id: String,
        /// Maximum allowed fee.
        max_fee: Numeric,
        /// Attempted fee.
        attempted_fee: Numeric,
    },
    /// Fee debit failed to apply.
    TransferFailed {
        /// Payer classification.
        payer_kind: NexusFeePayer,
        /// Account that attempted to pay.
        payer_id: String,
        /// Amount attempted.
        amount: Numeric,
        /// Asset definition id string.
        asset_id: String,
        /// Human-readable reason.
        reason: String,
    },
    /// Fee failed due to invalid configuration.
    ConfigInvalid {
        /// Human-readable error cause.
        reason: String,
    },
}

/// Per-lane staking summary for Nexus public lanes.
#[derive(Clone, Debug)]
pub struct NexusStakingLaneSnapshot {
    /// Lane identifier.
    pub lane_id: LaneId,
    /// Total bonded stake recorded.
    pub bonded: Numeric,
    /// Total pending-unbond stake recorded.
    pub pending_unbond: Numeric,
    /// Total slashes applied.
    pub slash_total: u64,
}

impl Default for NexusStakingLaneSnapshot {
    fn default() -> Self {
        Self {
            lane_id: LaneId::new(0),
            bonded: Numeric::zero(),
            pending_unbond: Numeric::zero(),
            slash_total: 0,
        }
    }
}

/// Aggregated Nexus staking snapshot (all lanes).
#[derive(Clone, Debug, Default)]
pub struct NexusStakingSnapshot {
    /// Per-lane staking summaries.
    pub lanes: Vec<NexusStakingLaneSnapshot>,
}

// Whether this node has been removed from the world state (peer unregistered).
static LOCAL_REMOVED_FROM_WORLD: AtomicBool = AtomicBool::new(false);

/// Record whether the local peer is present in the world state.
pub fn set_local_removed_from_world(removed: bool) {
    #[cfg(test)]
    let _guard = local_removed_test_guard();
    LOCAL_REMOVED_FROM_WORLD.store(removed, Ordering::Relaxed);
}

/// Check if the local peer has been removed from the world state.
pub fn local_peer_removed() -> bool {
    #[cfg(test)]
    let _guard = local_removed_test_guard();
    LOCAL_REMOVED_FROM_WORLD.load(Ordering::Relaxed)
}

/// Outcome classification for settlement telemetry snapshots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettlementOutcomeKind {
    /// Settlement executed successfully.
    Success,
    /// Settlement execution failed (preconditions or execution error).
    Failure,
}

impl SettlementOutcomeKind {
    /// String label used for metrics and status JSON.
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            SettlementOutcomeKind::Success => "success",
            SettlementOutcomeKind::Failure => "failure",
        }
    }
}

/// Aggregated settlement telemetry counters captured by the local peer.
#[derive(Clone, Debug, Default)]
pub struct SettlementStatusSnapshot {
    /// Delivery-versus-payment telemetry snapshot.
    pub dvp: DvpSettlementSnapshot,
    /// Payment-versus-payment telemetry snapshot.
    pub pvp: PvpSettlementSnapshot,
}

/// Derived counters and the last event snapshot for `DvP` settlements.
#[derive(Clone, Debug, Default)]
pub struct DvpSettlementSnapshot {
    /// Successful `DvP` executions observed locally.
    pub success_total: u64,
    /// Failed `DvP` executions observed locally.
    pub failure_total: u64,
    /// Final-state counter map keyed by `none|delivery_only|payment_only|both`.
    pub final_state_totals: BTreeMap<String, u64>,
    /// Failure reason counters keyed by telemetry label.
    pub failure_reasons: BTreeMap<String, u64>,
    /// Last observed `DvP` settlement event.
    pub last_event: Option<DvpSettlementEventSnapshot>,
}

/// Telemetry snapshot describing a single `DvP` settlement event.
#[derive(Clone, Debug)]
pub struct DvpSettlementEventSnapshot {
    /// Milliseconds since Unix epoch when the event was recorded.
    pub observed_at_ms: u64,
    /// Settlement identifier provided by the instruction.
    pub settlement_id: Option<String>,
    /// Execution order recorded for the settlement plan.
    pub plan_order: SettlementExecutionOrder,
    /// Atomicity policy applied to the settlement plan.
    pub plan_atomicity: SettlementAtomicity,
    /// Outcome classification (success/failure).
    pub outcome: SettlementOutcomeKind,
    /// Failure reason label when outcome is failure.
    pub failure_reason: Option<String>,
    /// Final state label (`none`, `delivery_only`, `payment_only`, `both`).
    pub final_state_label: String,
    /// Whether the delivery leg remained committed after execution.
    pub delivery_committed: bool,
    /// Whether the payment leg remained committed after execution.
    pub payment_committed: bool,
}

impl Default for DvpSettlementEventSnapshot {
    fn default() -> Self {
        Self {
            observed_at_ms: 0,
            settlement_id: None,
            plan_order: SettlementExecutionOrder::DeliveryThenPayment,
            plan_atomicity: SettlementAtomicity::AllOrNothing,
            outcome: SettlementOutcomeKind::Success,
            failure_reason: None,
            final_state_label: "none".to_string(),
            delivery_committed: false,
            payment_committed: false,
        }
    }
}

/// Derived counters and the last event snapshot for `PvP` settlements.
#[derive(Clone, Debug, Default)]
pub struct PvpSettlementSnapshot {
    /// Successful `PvP` executions observed locally.
    pub success_total: u64,
    /// Failed `PvP` executions observed locally.
    pub failure_total: u64,
    /// Final-state counter map keyed by `none|primary_only|counter_only|both`.
    pub final_state_totals: BTreeMap<String, u64>,
    /// Failure reason counters keyed by telemetry label.
    pub failure_reasons: BTreeMap<String, u64>,
    /// Last observed `PvP` settlement event.
    pub last_event: Option<PvpSettlementEventSnapshot>,
}

/// Telemetry snapshot describing a single `PvP` settlement event.
#[derive(Clone, Debug)]
pub struct PvpSettlementEventSnapshot {
    /// Milliseconds since Unix epoch when the event was recorded.
    pub observed_at_ms: u64,
    /// Settlement identifier provided by the instruction.
    pub settlement_id: Option<String>,
    /// Execution order recorded for the settlement plan.
    pub plan_order: SettlementExecutionOrder,
    /// Atomicity policy applied to the settlement plan.
    pub plan_atomicity: SettlementAtomicity,
    /// Outcome classification (success/failure).
    pub outcome: SettlementOutcomeKind,
    /// Failure reason label when outcome is failure.
    pub failure_reason: Option<String>,
    /// Final state label (`none`, `primary_only`, `counter_only`, `both`).
    pub final_state_label: String,
    /// Whether the primary leg remained committed after execution.
    pub primary_committed: bool,
    /// Whether the counter leg remained committed after execution.
    pub counter_committed: bool,
    /// Observed FX window in milliseconds (time between committed legs).
    pub fx_window_ms: Option<u64>,
}

impl Default for PvpSettlementEventSnapshot {
    fn default() -> Self {
        Self {
            observed_at_ms: 0,
            settlement_id: None,
            plan_order: SettlementExecutionOrder::DeliveryThenPayment,
            plan_atomicity: SettlementAtomicity::AllOrNothing,
            outcome: SettlementOutcomeKind::Success,
            failure_reason: None,
            final_state_label: "none".to_string(),
            primary_committed: false,
            counter_committed: false,
            fx_window_ms: None,
        }
    }
}

#[derive(Clone, Debug, Default)]
struct SettlementStatusState {
    dvp: DvpSettlementSnapshot,
    pvp: PvpSettlementSnapshot,
}

fn settlement_status_slot() -> &'static Mutex<SettlementStatusState> {
    SETTLEMENT_STATUS.get_or_init(|| Mutex::new(SettlementStatusState::default()))
}

/// Update payload produced when a `DvP` settlement completes.
#[derive(Clone, Debug)]
pub struct DvpSettlementEventUpdate {
    /// Milliseconds since Unix epoch when the event was recorded.
    pub observed_at_ms: u64,
    /// Settlement identifier provided by the instruction (if any).
    pub settlement_id: Option<String>,
    /// Execution order recorded for the settlement plan.
    pub plan_order: SettlementExecutionOrder,
    /// Atomicity policy applied to the settlement plan.
    pub plan_atomicity: SettlementAtomicity,
    /// Outcome classification (success or failure).
    pub outcome: SettlementOutcomeKind,
    /// Failure reason label when outcome is failure.
    pub failure_reason: Option<String>,
    /// Final state label (`none`, `delivery_only`, `payment_only`, or `both`).
    pub final_state_label: String,
    /// Whether the delivery leg remained committed after execution.
    pub delivery_committed: bool,
    /// Whether the payment leg remained committed after execution.
    pub payment_committed: bool,
}

/// Update payload produced when a `PvP` settlement completes.
#[derive(Clone, Debug)]
pub struct PvpSettlementEventUpdate {
    /// Milliseconds since Unix epoch when the event was recorded.
    pub observed_at_ms: u64,
    /// Settlement identifier provided by the instruction (if any).
    pub settlement_id: Option<String>,
    /// Execution order recorded for the settlement plan.
    pub plan_order: SettlementExecutionOrder,
    /// Atomicity policy applied to the settlement plan.
    pub plan_atomicity: SettlementAtomicity,
    /// Outcome classification (success or failure).
    pub outcome: SettlementOutcomeKind,
    /// Failure reason label when outcome is failure.
    pub failure_reason: Option<String>,
    /// Final state label (`none`, `primary_only`, `counter_only`, or `both`).
    pub final_state_label: String,
    /// Whether the primary leg remained committed after execution.
    pub primary_committed: bool,
    /// Whether the counter leg remained committed after execution.
    pub counter_committed: bool,
    /// Observed FX window in milliseconds (time between committed legs).
    pub fx_window_ms: Option<u64>,
}

/// Record a `DvP` settlement telemetry update.
pub fn record_dvp_settlement_event(update: DvpSettlementEventUpdate) {
    let mut guard = lock_operator_status_slot(settlement_status_slot(), "settlement status");
    let entry = &mut guard.dvp;
    match update.outcome {
        SettlementOutcomeKind::Success => {
            entry.success_total = entry.success_total.saturating_add(1)
        }
        SettlementOutcomeKind::Failure => {
            entry.failure_total = entry.failure_total.saturating_add(1)
        }
    }
    *entry
        .final_state_totals
        .entry(update.final_state_label.clone())
        .or_default() += 1;
    if let Some(reason) = update.failure_reason.clone() {
        *entry.failure_reasons.entry(reason).or_default() += 1;
    }
    entry.last_event = Some(DvpSettlementEventSnapshot {
        observed_at_ms: update.observed_at_ms,
        settlement_id: update.settlement_id,
        plan_order: update.plan_order,
        plan_atomicity: update.plan_atomicity,
        outcome: update.outcome,
        failure_reason: update.failure_reason,
        final_state_label: update.final_state_label,
        delivery_committed: update.delivery_committed,
        payment_committed: update.payment_committed,
    });
}

/// Record a `PvP` settlement telemetry update.
pub fn record_pvp_settlement_event(update: PvpSettlementEventUpdate) {
    let mut guard = lock_operator_status_slot(settlement_status_slot(), "settlement status");
    let entry = &mut guard.pvp;
    match update.outcome {
        SettlementOutcomeKind::Success => {
            entry.success_total = entry.success_total.saturating_add(1)
        }
        SettlementOutcomeKind::Failure => {
            entry.failure_total = entry.failure_total.saturating_add(1)
        }
    }
    *entry
        .final_state_totals
        .entry(update.final_state_label.clone())
        .or_default() += 1;
    if let Some(reason) = update.failure_reason.clone() {
        *entry.failure_reasons.entry(reason).or_default() += 1;
    }
    entry.last_event = Some(PvpSettlementEventSnapshot {
        observed_at_ms: update.observed_at_ms,
        settlement_id: update.settlement_id,
        plan_order: update.plan_order,
        plan_atomicity: update.plan_atomicity,
        outcome: update.outcome,
        failure_reason: update.failure_reason,
        final_state_label: update.final_state_label,
        primary_committed: update.primary_committed,
        counter_committed: update.counter_committed,
        fx_window_ms: update.fx_window_ms,
    });
}

/// Read-only snapshot of settlement telemetry state.
pub fn settlement_snapshot() -> SettlementStatusSnapshot {
    let guard = lock_operator_status_slot(settlement_status_slot(), "settlement status");
    SettlementStatusSnapshot {
        dvp: guard.dvp.clone(),
        pvp: guard.pvp.clone(),
    }
}

/// Per-lane execution summary for operator dashboards.
#[derive(Clone, Copy, Debug, Default)]
pub struct LaneActivitySnapshot {
    /// Lane identifier (numeric).
    pub lane_id: u32,
    /// Transactions executed for this lane.
    pub tx_vertices: u64,
    /// Conflict edges among those transactions.
    pub tx_edges: u64,
    /// Overlay fragments executed for this lane.
    pub overlay_count: u64,
    /// Total overlay instructions executed for this lane.
    pub overlay_instr_total: u64,
    /// Total overlay bytes executed for this lane.
    pub overlay_bytes_total: u64,
    /// Approximate number of RBC chunks attributed to this lane.
    pub rbc_chunks: u64,
    /// Approximate total RBC payload bytes attributed to this lane.
    pub rbc_bytes_total: u64,
    /// Transactions prepared for detached overlay execution.
    pub detached_prepared: u64,
    /// Detached transaction deltas merged without sequential fallback.
    pub detached_merged: u64,
    /// Detached transaction deltas that fell back to sequential execution.
    pub detached_fallback: u64,
    /// Sequential fallbacks caused by fee postprocessing requirements.
    pub detached_fallback_fee_postprocessing: u64,
    /// Sequential fallbacks caused by a user-provided executor.
    pub detached_fallback_user_executor: u64,
    /// Sequential fallbacks caused by durable smart-contract state changes.
    pub detached_fallback_durable_state: u64,
    /// Sequential fallbacks caused by unsupported detached instructions.
    pub detached_fallback_unsupported_instruction: u64,
    /// Sequential fallbacks caused by rejected detached evaluation.
    pub detached_fallback_rejected_eval: u64,
    /// Sequential fallbacks caused by overlay build errors.
    pub detached_fallback_overlay_error: u64,
    /// Quarantine transactions executed in the sequential quarantine lane.
    pub quarantine_executed: u64,
}

/// Aggregate execution summary for the latest block pipeline run.
#[derive(Clone, Copy, Debug, Default)]
pub struct PipelineExecutionSnapshot {
    /// Total transaction vertices across all lanes.
    pub tx_vertices_total: u64,
    /// Total conflict edges across all lanes.
    pub tx_edges_total: u64,
    /// Total overlay fragments executed across all lanes.
    pub overlay_count_total: u64,
    /// Total overlay instructions executed across all lanes.
    pub overlay_instr_total: u64,
    /// Total overlay bytes executed across all lanes.
    pub overlay_bytes_total: u64,
    /// Total RBC chunks attributed across all lanes.
    pub rbc_chunks_total: u64,
    /// Total RBC payload bytes attributed across all lanes.
    pub rbc_bytes_total: u64,
    /// Transactions prepared for detached overlay execution.
    pub detached_prepared_total: u64,
    /// Detached transaction deltas merged without sequential fallback.
    pub detached_merged_total: u64,
    /// Detached transaction deltas that fell back to sequential execution.
    pub detached_fallback_total: u64,
    /// Sequential fallbacks caused by fee postprocessing requirements.
    pub detached_fallback_fee_postprocessing_total: u64,
    /// Sequential fallbacks caused by a user-provided executor.
    pub detached_fallback_user_executor_total: u64,
    /// Sequential fallbacks caused by durable smart-contract state changes.
    pub detached_fallback_durable_state_total: u64,
    /// Sequential fallbacks caused by unsupported detached instructions.
    pub detached_fallback_unsupported_instruction_total: u64,
    /// Sequential fallbacks caused by rejected detached evaluation.
    pub detached_fallback_rejected_eval_total: u64,
    /// Sequential fallbacks caused by overlay build errors.
    pub detached_fallback_overlay_error_total: u64,
    /// Quarantine transactions executed in the sequential quarantine lane.
    pub quarantine_executed_total: u64,
}

/// Summary of access-set sources used for IVM transactions in the latest block.
#[derive(Clone, Copy, Debug, Default)]
pub struct AccessSetSourceSummary {
    /// Transactions using manifest-level access-set hints.
    pub manifest_hints: u64,
    /// Transactions using entrypoint-level access-set hints.
    pub entrypoint_hints: u64,
    /// Transactions derived from the dynamic prepass (merged sources).
    pub prepass_merge: u64,
    /// Transactions that fell back to the conservative global set.
    pub conservative_fallback: u64,
}

/// Per-dataspace execution summary for operator dashboards.
#[derive(Clone, Copy, Debug, Default)]
pub struct DataspaceActivitySnapshot {
    /// Owning lane identifier (numeric).
    pub lane_id: u32,
    /// Dataspace identifier.
    pub dataspace_id: u64,
    /// Transactions executed for this dataspace.
    pub tx_served: u64,
}

/// Aggregated per-lane RBC backlog snapshot for operator dashboards.
#[derive(Clone, Copy, Debug, Default, Encode, Decode, PartialEq, Eq)]
pub struct LaneRbcSnapshot {
    /// Lane identifier (numeric).
    pub lane_id: u32,
    /// Transactions contributing payload bytes in this lane across active sessions.
    pub tx_count: u64,
    /// Total RBC chunks attributed to this lane across active sessions.
    pub total_chunks: u64,
    /// RBC chunks still pending delivery for this lane across active sessions.
    pub pending_chunks: u64,
    /// Total RBC payload bytes attributed to this lane across active sessions.
    pub rbc_bytes_total: u64,
}

/// Aggregated per-dataspace RBC backlog snapshot for operator dashboards.
#[derive(Clone, Copy, Debug, Default, Encode, Decode, PartialEq, Eq)]
pub struct DataspaceRbcSnapshot {
    /// Owning lane identifier (numeric).
    pub lane_id: u32,
    /// Dataspace identifier (numeric).
    pub dataspace_id: u64,
    /// Transactions contributing payload bytes for this dataspace across active sessions.
    pub tx_count: u64,
    /// Total RBC chunks attributed to this dataspace across active sessions.
    pub total_chunks: u64,
    /// RBC chunks still pending delivery for this dataspace across active sessions.
    pub pending_chunks: u64,
    /// Total RBC payload bytes attributed to this dataspace across active sessions.
    pub rbc_bytes_total: u64,
}

/// Aggregated per-lane commitment summary for recently committed blocks.
#[derive(Clone, Copy, Debug)]
pub struct LaneCommitmentSnapshot {
    /// Block height associated with the commitment.
    pub block_height: u64,
    /// Lane identifier (numeric).
    pub lane_id: u32,
    /// Number of transactions routed to this lane in the block.
    pub tx_count: u64,
    /// Total RBC chunks attributed to this lane.
    pub total_chunks: u64,
    /// Total RBC payload bytes attributed to this lane.
    pub rbc_bytes_total: u64,
    /// Total TEU attributed to this lane.
    pub teu_total: u64,
    /// Block hash identifying the commitment.
    pub block_hash: HashOf<BlockHeader>,
}

/// Aggregated per-dataspace commitment summary for recently committed blocks.
#[derive(Clone, Copy, Debug)]
pub struct DataspaceCommitmentSnapshot {
    /// Block height associated with the commitment.
    pub block_height: u64,
    /// Lane identifier (numeric).
    pub lane_id: u32,
    /// Dataspace identifier (numeric).
    pub dataspace_id: u64,
    /// Number of transactions routed to this dataspace.
    pub tx_count: u64,
    /// Total RBC chunks attributed to this dataspace.
    pub total_chunks: u64,
    /// Total RBC payload bytes attributed to this dataspace.
    pub rbc_bytes_total: u64,
    /// Total TEU attributed to this dataspace.
    pub teu_total: u64,
    /// Block hash identifying the commitment.
    pub block_hash: HashOf<BlockHeader>,
}

/// Execution readiness for a certified standalone lane-local block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommittedLaneBlockExecutionStatus {
    /// The block has proposal/prepare/commit certificates, but no executable lane payload yet.
    AwaitingExecutablePayload,
    /// Accepted entrypoints are locally recoverable, but standalone execution is not wired yet.
    PayloadAvailableAwaitingExecutor,
    /// Accepted entrypoints have been durably recovered for standalone state application.
    PayloadRecoveredAwaitingStateApplication,
    /// Recovered entrypoints passed direct-execution preflight at the current local state tip.
    PayloadPreflightedAwaitingStateApplication,
    /// Recovered entrypoints produced at least one rejection during direct-execution preflight.
    PayloadPreflightRejectedAwaitingStateApplication,
    /// Canonical application receipt disagrees with durable direct-execution preflight results.
    ApplicationReceiptConflictsWithPreflight,
    /// This lane block cannot execute until its certified predecessor is applied.
    AwaitingPredecessorApplication,
    /// Accepted entrypoints already have canonical committed results recorded locally.
    StateAppliedByCanonicalBlock,
    /// Accepted entrypoints were directly applied to local WSV without a canonical block append.
    StateAppliedByDirectExecution,
}

impl CommittedLaneBlockExecutionStatus {
    /// Stable operator-facing label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AwaitingExecutablePayload => COMMITTED_LANE_STATUS_AWAITING_EXECUTABLE_PAYLOAD,
            Self::PayloadAvailableAwaitingExecutor => {
                COMMITTED_LANE_STATUS_PAYLOAD_AVAILABLE_AWAITING_EXECUTOR
            }
            Self::PayloadRecoveredAwaitingStateApplication => {
                COMMITTED_LANE_STATUS_PAYLOAD_RECOVERED_AWAITING_STATE_APPLICATION
            }
            Self::PayloadPreflightedAwaitingStateApplication => {
                COMMITTED_LANE_STATUS_PAYLOAD_PREFLIGHTED_AWAITING_STATE_APPLICATION
            }
            Self::PayloadPreflightRejectedAwaitingStateApplication => {
                COMMITTED_LANE_STATUS_PAYLOAD_PREFLIGHT_REJECTED_AWAITING_STATE_APPLICATION
            }
            Self::ApplicationReceiptConflictsWithPreflight => {
                COMMITTED_LANE_STATUS_APPLICATION_RECEIPT_CONFLICTS_WITH_PREFLIGHT
            }
            Self::AwaitingPredecessorApplication => {
                COMMITTED_LANE_STATUS_AWAITING_PREDECESSOR_APPLICATION
            }
            Self::StateAppliedByCanonicalBlock => {
                COMMITTED_LANE_STATUS_STATE_APPLIED_BY_CANONICAL_BLOCK
            }
            Self::StateAppliedByDirectExecution => {
                COMMITTED_LANE_STATUS_STATE_APPLIED_BY_DIRECT_EXECUTION
            }
        }
    }

    /// Whether the committed lane block can be handed to a standalone executor.
    #[must_use]
    pub const fn executable_payload_available(self) -> bool {
        match self {
            Self::AwaitingExecutablePayload => false,
            Self::PayloadAvailableAwaitingExecutor
            | Self::PayloadRecoveredAwaitingStateApplication
            | Self::PayloadPreflightedAwaitingStateApplication
            | Self::StateAppliedByCanonicalBlock
            | Self::StateAppliedByDirectExecution => true,
            Self::ApplicationReceiptConflictsWithPreflight
            | Self::PayloadPreflightRejectedAwaitingStateApplication
            | Self::AwaitingPredecessorApplication => false,
        }
    }
}

/// Standalone lane-local block that has proposal, prepare QC, and commit QC.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommittedLaneBlockSnapshot {
    /// Lane whose local block is committed.
    pub lane_id: LaneId,
    /// Dataspace bound to the committed lane-local block.
    pub dataspace_id: DataSpaceId,
    /// Lane-local block height.
    pub lane_block_height: u64,
    /// Lane-local consensus view.
    pub lane_block_view: u64,
    /// Stable hash of the standalone lane block descriptor.
    pub descriptor_hash: Hash,
    /// Stable hash of the standalone lane block proposal.
    pub proposal_hash: Hash,
    /// Execution readiness of the certified standalone lane-local block.
    pub execution_status: CommittedLaneBlockExecutionStatus,
    /// Proposal artifact committed by the QCs.
    pub proposal: LaneBlockProposalV1,
    /// Prepare QC for the proposal.
    pub prepare_qc: LaneBlockQcV1,
    /// Commit QC for the proposal.
    pub commit_qc: LaneBlockQcV1,
}

impl CommittedLaneBlockSnapshot {
    pub(crate) fn from_committed_session_with_execution_status(
        session: &crate::lane_consensus::CommittedLaneBlockSession,
        execution_status: CommittedLaneBlockExecutionStatus,
    ) -> Self {
        let descriptor = &session.proposal.descriptor;
        Self {
            lane_id: descriptor.lane_id,
            dataspace_id: descriptor.dataspace_id,
            lane_block_height: descriptor.lane_block_height,
            lane_block_view: descriptor.lane_block_view,
            descriptor_hash: descriptor.descriptor_hash,
            proposal_hash: session.proposal.proposal_hash,
            execution_status,
            proposal: session.proposal.clone(),
            prepare_qc: session.prepare_qc.clone(),
            commit_qc: session.commit_qc.clone(),
        }
    }

    /// Whether the committed lane block has enough payload material for execution.
    #[must_use]
    pub const fn executable_payload_available(&self) -> bool {
        self.execution_status.executable_payload_available()
    }
}

/// Governance manifest snapshot for a lane.
#[derive(Clone, Debug, Default)]
pub struct LaneGovernanceSnapshot {
    /// Numeric lane identifier.
    pub lane_id: u32,
    /// Human-readable lane alias.
    pub alias: String,
    /// Dataspace identifier bound to the lane.
    pub dataspace_id: u64,
    /// Declarative visibility profile (`public` / `restricted`).
    pub visibility: String,
    /// Storage profile advertised for the lane.
    pub storage_profile: String,
    /// Governance module configured for the lane, if any.
    pub governance: Option<String>,
    /// Whether the lane requires a governance manifest.
    pub manifest_required: bool,
    /// Whether a manifest has been loaded and validated.
    pub manifest_ready: bool,
    /// Source path for the manifest (best-effort; operator visibility).
    pub manifest_path: Option<String>,
    /// Validator identifiers derived from the manifest.
    pub validator_ids: Vec<String>,
    /// Quorum threshold applied to the lane (if provided).
    pub quorum: Option<u32>,
    /// Protected namespaces enforced by the manifest.
    pub protected_namespaces: Vec<String>,
    /// Runtime-upgrade governance hook snapshot when configured.
    pub runtime_upgrade: Option<LaneRuntimeUpgradeHookSnapshot>,
    /// Privacy commitments advertised by the lane manifest.
    pub privacy_commitments: Vec<LanePrivacyCommitmentSnapshot>,
}

/// Snapshot of a privacy commitment registered for a lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LanePrivacyCommitmentSnapshot {
    /// Stable identifier assigned to the commitment.
    pub id: u16,
    /// Scheme-specific metadata captured at registry time.
    pub scheme: LanePrivacyCommitmentSchemeSnapshot,
}

/// Scheme metadata surfaced for observability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LanePrivacyCommitmentSchemeSnapshot {
    /// Merkle-root commitment and audit-path depth budget.
    Merkle {
        /// Root hash that commits to the private dataset.
        root: [u8; 32],
        /// Maximum Merkle proof depth the lane operator promises to serve.
        max_depth: u8,
    },
    /// zk-SNARK circuit commitment exposing the hash bindings.
    Snark {
        /// Circuit identifier within the manifest's SNARK registry.
        circuit_id: u16,
        /// BLAKE3 digest of the verifying key used for audits.
        verifying_key_digest: [u8; 32],
        /// Hash of the public statement constrained by the circuit.
        statement_hash: [u8; 32],
        /// Hash of the proof artifact stored alongside the commitment.
        proof_hash: [u8; 32],
    },
}

impl From<&LanePrivacyCommitment> for LanePrivacyCommitmentSnapshot {
    fn from(commitment: &LanePrivacyCommitment) -> Self {
        let scheme = match commitment.scheme() {
            CommitmentScheme::Merkle(merkle) => LanePrivacyCommitmentSchemeSnapshot::Merkle {
                root: hash_of_bytes(*merkle.root()),
                max_depth: merkle.max_depth(),
            },
            CommitmentScheme::Snark(snark) => LanePrivacyCommitmentSchemeSnapshot::Snark {
                circuit_id: snark.circuit_id().get(),
                verifying_key_digest: *snark.verifying_key_digest(),
                statement_hash: *snark.statement_hash(),
                proof_hash: *snark.proof_hash(),
            },
        };
        Self {
            id: commitment.id().get(),
            scheme,
        }
    }
}

fn hash_of_bytes<T>(hash: HashOf<T>) -> [u8; 32] {
    let untyped: UntypedHash = hash.into();
    untyped.into()
}

/// Runtime-upgrade governance hook snapshot.
#[derive(Clone, Debug, Default)]
pub struct LaneRuntimeUpgradeHookSnapshot {
    /// Whether runtime-upgrade instructions are allowed.
    pub allow: bool,
    /// Whether runtime-upgrade instructions must include metadata.
    pub require_metadata: bool,
    /// Metadata key enforced by the manifest, if specified.
    pub metadata_key: Option<String>,
    /// Allowed metadata identifiers when an allowlist is configured.
    pub allowed_ids: Vec<String>,
}

fn nexus_fee_slot() -> &'static Mutex<NexusFeeSnapshot> {
    NEXUS_FEE_STATUS.get_or_init(|| Mutex::new(NexusFeeSnapshot::default()))
}

fn nexus_staking_slot() -> &'static Mutex<BTreeMap<LaneId, NexusStakingLaneSnapshot>> {
    NEXUS_STAKING_STATUS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

/// Record a Nexus fee debit outcome for later status/telemetry surfacing.
pub fn record_nexus_fee_event(event: NexusFeeEvent) {
    #[cfg(test)]
    let Some(_guard) = try_reentrant_test_guard(&RBC_STATUS_TEST_LOCK) else {
        return;
    };
    let mut guard = lock_operator_status_slot(nexus_fee_slot(), "nexus fee status");
    match event {
        NexusFeeEvent::Charged {
            payer_kind,
            payer_id,
            amount,
            asset_id,
        } => {
            guard.charged_total = guard.charged_total.saturating_add(1);
            match payer_kind {
                NexusFeePayer::Payer => {
                    guard.charged_via_payer_total = guard.charged_via_payer_total.saturating_add(1);
                }
                NexusFeePayer::Sponsor => {
                    guard.charged_via_sponsor_total =
                        guard.charged_via_sponsor_total.saturating_add(1);
                }
            }
            guard.last_amount = Some(amount);
            guard.last_asset_id = Some(asset_id);
            guard.last_payer = Some(payer_kind);
            guard.last_payer_id = Some(payer_id);
            guard.last_error = None;
        }
        NexusFeeEvent::SponsorDisabled { payer_id } => {
            guard.sponsor_disabled_total = guard.sponsor_disabled_total.saturating_add(1);
            guard.last_payer = Some(NexusFeePayer::Sponsor);
            guard.last_payer_id = Some(payer_id);
            guard.last_error = Some("sponsorship disabled".to_string());
        }
        NexusFeeEvent::SponsorUnauthorized {
            sponsor_id,
            authority_id,
        } => {
            guard.sponsor_unauthorized_total = guard.sponsor_unauthorized_total.saturating_add(1);
            guard.last_payer = Some(NexusFeePayer::Sponsor);
            guard.last_payer_id = Some(sponsor_id);
            guard.last_error = Some(format!(
                "sponsor not authorized for authority {authority_id}"
            ));
        }
        NexusFeeEvent::SponsorCapExceeded {
            payer_id,
            max_fee,
            attempted_fee,
        } => {
            guard.sponsor_cap_exceeded_total = guard.sponsor_cap_exceeded_total.saturating_add(1);
            guard.last_payer = Some(NexusFeePayer::Sponsor);
            guard.last_payer_id = Some(payer_id);
            guard.last_amount = Some(attempted_fee);
            guard.last_error = Some(format!("sponsor_max_fee exceeded (max={max_fee})"));
        }
        NexusFeeEvent::TransferFailed {
            payer_kind,
            payer_id,
            amount,
            asset_id,
            reason,
        } => {
            guard.transfer_failures_total = guard.transfer_failures_total.saturating_add(1);
            guard.last_payer = Some(payer_kind);
            guard.last_payer_id = Some(payer_id);
            guard.last_amount = Some(amount);
            guard.last_asset_id = Some(asset_id);
            guard.last_error = Some(reason);
        }
        NexusFeeEvent::ConfigInvalid { reason } => {
            guard.config_errors_total = guard.config_errors_total.saturating_add(1);
            guard.last_error = Some(reason);
        }
    }
}

fn update_staking_lane<F>(lane_id: LaneId, mut update: F)
where
    F: FnMut(&mut NexusStakingLaneSnapshot),
{
    #[cfg(test)]
    let Some(_guard) = try_reentrant_test_guard(&RBC_STATUS_TEST_LOCK) else {
        return;
    };
    let mut guard = lock_operator_status_slot(nexus_staking_slot(), "nexus staking status");
    let entry = guard
        .entry(lane_id)
        .or_insert_with(|| NexusStakingLaneSnapshot {
            lane_id,
            ..NexusStakingLaneSnapshot::default()
        });
    update(entry);
}

fn adjust_numeric_value(current: Numeric, delta: &Numeric, increase: bool) -> Numeric {
    if delta.is_zero() {
        return current;
    }
    if increase {
        let base = current.clone();
        current.checked_add(delta.clone()).unwrap_or_else(|| {
            iroha_logger::warn!(
                %base,
                %delta,
                "nexus staking accumulator overflowed; clamping to Numeric::zero()"
            );
            Numeric::zero()
        })
    } else {
        let base = current.clone();
        current.checked_sub(delta.clone()).unwrap_or_else(|| {
            iroha_logger::warn!(
                %base,
                %delta,
                "nexus staking accumulator underflowed; clamping to Numeric::zero()"
            );
            Numeric::zero()
        })
    }
}

/// Record a bonded stake delta for a Nexus lane.
pub fn record_public_lane_bonded_delta(lane_id: LaneId, amount: &Numeric, increase: bool) {
    update_staking_lane(lane_id, |snapshot| {
        snapshot.bonded = adjust_numeric_value(snapshot.bonded.clone(), amount, increase);
    });
}

/// Record a pending-unbond delta for a Nexus lane.
pub fn record_public_lane_pending_unbond_delta(lane_id: LaneId, amount: &Numeric, increase: bool) {
    update_staking_lane(lane_id, |snapshot| {
        snapshot.pending_unbond =
            adjust_numeric_value(snapshot.pending_unbond.clone(), amount, increase);
    });
}

/// Record a slash event for a Nexus lane.
pub fn record_public_lane_slash(lane_id: LaneId) {
    update_staking_lane(lane_id, |snapshot| {
        snapshot.slash_total = snapshot.slash_total.saturating_add(1);
    });
}

/// Remove accumulated Nexus public-lane staking status for reset lanes.
pub fn reset_public_lane_staking_lanes(lanes_to_reset: &BTreeSet<LaneId>) {
    if lanes_to_reset.is_empty() {
        return;
    }
    #[cfg(test)]
    let Some(_guard) = try_reentrant_test_guard(&RBC_STATUS_TEST_LOCK) else {
        return;
    };

    let mut guard = lock_operator_status_slot(nexus_staking_slot(), "nexus staking status");
    for lane_id in lanes_to_reset {
        guard.remove(lane_id);
    }
}

/// Latest aggregated Nexus fee snapshot.
pub fn nexus_fee_snapshot() -> NexusFeeSnapshot {
    lock_operator_status_slot(nexus_fee_slot(), "nexus fee status").clone()
}

/// Latest aggregated Nexus staking snapshot.
pub fn nexus_staking_snapshot() -> NexusStakingSnapshot {
    let guard = lock_operator_status_slot(nexus_staking_slot(), "nexus staking status");
    let mut lanes: Vec<_> = guard.values().cloned().collect();
    lanes.sort_by_key(|lane| lane.lane_id.as_u32());
    NexusStakingSnapshot { lanes }
}

/// Shared lock for tests that mutate global Nexus fee state.
#[cfg(not(test))]
pub fn nexus_fee_test_lock() -> &'static std::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
}

/// Shared lock for tests that mutate global Nexus fee state.
#[cfg(test)]
pub(crate) fn nexus_fee_test_lock() -> &'static NexusFeeTestLock {
    static LOCK: NexusFeeTestLock = NexusFeeTestLock;
    &LOCK
}

/// Clear Nexus economics snapshots (test-only helper).
pub fn reset_nexus_economics_for_tests() {
    #[cfg(test)]
    let _guard = rbc_status_test_guard();
    {
        let mut guard = lock_operator_status_slot(nexus_fee_slot(), "nexus fee status");
        *guard = NexusFeeSnapshot::default();
    }
    {
        let mut guard = lock_operator_status_slot(nexus_staking_slot(), "nexus staking status");
        guard.clear();
    }
}

/// Reasons a peer-consensus-key admission can be rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeerKeyPolicyRejectReason {
    /// Required HSM binding missing.
    MissingHsm,
    /// Public-key algorithm not allowed by policy.
    DisallowedAlgorithm,
    /// HSM provider not allowed by policy.
    DisallowedProvider,
    /// Activation height violates lead-time policy.
    LeadTimeViolation,
    /// Activation height is in the past.
    ActivationInPast,
    /// Expiry occurs before activation.
    ExpiryBeforeActivation,
    /// Consensus-key identifier collides with an existing id for the same public key.
    IdentifierCollision,
}

impl PeerKeyPolicyRejectReason {
    /// Return a stable label for telemetry.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MissingHsm => "missing_hsm",
            Self::DisallowedAlgorithm => "disallowed_algorithm",
            Self::DisallowedProvider => "disallowed_provider",
            Self::LeadTimeViolation => "lead_time_violation",
            Self::ActivationInPast => "activation_in_past",
            Self::ExpiryBeforeActivation => "expiry_before_activation",
            Self::IdentifierCollision => "identifier_collision",
        }
    }
}

static PEER_KEY_POLICY_REJECT_TOTAL: AtomicU64 = AtomicU64::new(0);
static PEER_KEY_POLICY_LAST_REASON: OnceLock<Mutex<Option<&'static str>>> = OnceLock::new();

/// Record a peer consensus-key policy rejection.
pub fn record_peer_key_policy_reject(reason: PeerKeyPolicyRejectReason) {
    #[cfg(test)]
    let Some(_guard) = try_reentrant_test_guard(&PEER_KEY_POLICY_TEST_LOCK) else {
        return;
    };
    PEER_KEY_POLICY_REJECT_TOTAL.fetch_add(1, Ordering::Relaxed);
    *lock_operator_status_slot(
        PEER_KEY_POLICY_LAST_REASON.get_or_init(|| Mutex::new(None)),
        "peer key policy reason",
    ) = Some(reason.as_str());
}

/// Reset peer-key policy diagnostics in isolated tests.
#[cfg(test)]
pub(crate) fn reset_peer_key_policy_counters_for_tests() {
    let _guard = peer_key_policy_test_guard();
    PEER_KEY_POLICY_REJECT_TOTAL.store(0, Ordering::Relaxed);
    *lock_operator_status_slot(
        PEER_KEY_POLICY_LAST_REASON.get_or_init(|| Mutex::new(None)),
        "peer key policy reason",
    ) = None;
}

/// Read the compact peer-key rejection diagnostic in isolated unit tests.
#[cfg(test)]
pub(crate) fn peer_key_policy_reject_snapshot_for_tests() -> (u64, Option<&'static str>) {
    let total = PEER_KEY_POLICY_REJECT_TOTAL.load(Ordering::Relaxed);
    let last_reason = *lock_operator_status_slot(
        PEER_KEY_POLICY_LAST_REASON.get_or_init(|| Mutex::new(None)),
        "peer key policy reason",
    );
    (total, last_reason)
}

const KEY_LIFECYCLE_HISTORY_CAP: usize = 128;
static KEY_LIFECYCLE_HISTORY: OnceLock<Mutex<VecDeque<ConsensusKeyRecord>>> = OnceLock::new();

fn key_history_slot() -> &'static Mutex<VecDeque<ConsensusKeyRecord>> {
    KEY_LIFECYCLE_HISTORY.get_or_init(|| Mutex::new(VecDeque::new()))
}

/// Record a consensus-key lifecycle entry for the remaining legacy Torii endpoint.
pub fn record_consensus_key(record: ConsensusKeyRecord) {
    let mut history = lock_operator_status_slot(key_history_slot(), "key lifecycle history");
    history.retain(|existing| existing.id != record.id);
    history.push_back(record);
    while history.len() > KEY_LIFECYCLE_HISTORY_CAP {
        history.pop_front();
    }
}

/// Return consensus-key lifecycle entries newest first.
#[must_use]
pub fn consensus_key_history() -> Vec<ConsensusKeyRecord> {
    lock_operator_status_slot(key_history_slot(), "key lifecycle history")
        .iter()
        .rev()
        .cloned()
        .collect()
}

/// Clear consensus-key lifecycle history in tests.
#[cfg(test)]
pub fn reset_consensus_keys_for_tests() {
    lock_operator_status_slot(key_history_slot(), "key lifecycle history").clear();
}

static VRF_PENALTY_EPOCH: AtomicU64 = AtomicU64::new(0);
static VRF_NON_REVEAL_TOTAL: AtomicU64 = AtomicU64::new(0);
static VRF_NO_PARTICIPATION_TOTAL: AtomicU64 = AtomicU64::new(0);
static VRF_LATE_REVEALS_TOTAL: AtomicU64 = AtomicU64::new(0);

/// Return the legacy VRF penalty counters still consumed by one Torii route.
#[must_use]
pub fn vrf_penalty_snapshot() -> (u64, u64, u64, u64) {
    (
        VRF_PENALTY_EPOCH.load(Ordering::Relaxed),
        VRF_NON_REVEAL_TOTAL.load(Ordering::Relaxed),
        VRF_NO_PARTICIPATION_TOTAL.load(Ordering::Relaxed),
        VRF_LATE_REVEALS_TOTAL.load(Ordering::Relaxed),
    )
}

/// Worker-loop queue identifiers used by the remaining async adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkerQueueKind {
    /// Vote-related messages.
    Votes,
    /// Block payload messages.
    BlockPayload,
    /// Legacy lane-RBC chunk transport.
    RbcChunks,
    /// Fallback block/control messages.
    Blocks,
    /// Consensus control-flow messages.
    Consensus,
    /// Lane relay envelopes.
    LaneRelay,
    /// Background post requests.
    Background,
}

static WORKER_QUEUE_DEPTHS: [AtomicU64; 7] = [const { AtomicU64::new(0) }; 7];
static WORKER_QUEUE_DROPS: [AtomicU64; 7] = [const { AtomicU64::new(0) }; 7];

const fn worker_queue_index(kind: WorkerQueueKind) -> usize {
    match kind {
        WorkerQueueKind::Votes => 0,
        WorkerQueueKind::BlockPayload => 1,
        WorkerQueueKind::RbcChunks => 2,
        WorkerQueueKind::Blocks => 3,
        WorkerQueueKind::Consensus => 4,
        WorkerQueueKind::LaneRelay => 5,
        WorkerQueueKind::Background => 6,
    }
}

/// Record an enqueue for the given adapter queue.
pub fn record_worker_queue_enqueue(kind: WorkerQueueKind) {
    WORKER_QUEUE_DEPTHS[worker_queue_index(kind)].fetch_add(1, Ordering::Relaxed);
}

/// Record a dropped enqueue for the given adapter queue.
pub fn record_worker_queue_drop(kind: WorkerQueueKind) {
    WORKER_QUEUE_DROPS[worker_queue_index(kind)].fetch_add(1, Ordering::Relaxed);
}

static GOSSIP_DUPLICATE_KNOWN_SKIPPED_TOTAL: AtomicU64 = AtomicU64::new(0);

/// Count a duplicate transaction skipped by gossip.
pub fn inc_gossip_duplicate_known_skipped() {
    GOSSIP_DUPLICATE_KNOWN_SKIPPED_TOTAL.fetch_add(1, Ordering::Relaxed);
}

fn lane_activity_slot() -> &'static Mutex<Vec<LaneActivitySnapshot>> {
    LANE_ACTIVITY.get_or_init(|| Mutex::new(Vec::new()))
}

fn access_set_source_slot() -> &'static Mutex<AccessSetSourceSummary> {
    ACCESS_SET_SOURCES.get_or_init(|| Mutex::new(AccessSetSourceSummary::default()))
}

fn dataspace_activity_slot() -> &'static Mutex<Vec<DataspaceActivitySnapshot>> {
    DATASPACE_ACTIVITY.get_or_init(|| Mutex::new(Vec::new()))
}

fn pipeline_execution_slot() -> &'static Mutex<PipelineExecutionSnapshot> {
    PIPELINE_EXECUTION.get_or_init(|| Mutex::new(PipelineExecutionSnapshot::default()))
}

/// Replace the lane-activity adapter diagnostic.
pub fn set_lane_activity_snapshot(entries: Vec<LaneActivitySnapshot>) {
    *lock_operator_status_slot(lane_activity_slot(), "lane activity snapshot") = entries;
}

/// Replace the aggregate pipeline-execution adapter diagnostic.
pub fn set_pipeline_execution_snapshot(snapshot: PipelineExecutionSnapshot) {
    #[cfg(test)]
    let Some(_guard) = try_reentrant_test_guard(&RBC_STATUS_TEST_LOCK) else {
        return;
    };
    *lock_operator_status_slot(pipeline_execution_slot(), "pipeline execution snapshot") = snapshot;
}

/// Test-only wrapper that reads only the pipeline adapter diagnostic without
/// cloning the rest of the public non-consensus status snapshot.
#[cfg(test)]
pub(crate) struct PipelineExecutionTestSnapshot {
    /// Aggregate adapter counters asserted by block-pipeline tests.
    pub(crate) pipeline_execution: PipelineExecutionSnapshot,
}

/// Read the aggregate pipeline-execution diagnostic in isolated unit tests.
#[cfg(test)]
pub(crate) fn pipeline_execution_snapshot_for_tests() -> PipelineExecutionTestSnapshot {
    PipelineExecutionTestSnapshot {
        pipeline_execution: lock_operator_status_slot(
            pipeline_execution_slot(),
            "pipeline execution snapshot",
        )
        .clone(),
    }
}

/// Replace the access-set source adapter diagnostic.
pub fn set_access_set_source_summary(summary: AccessSetSourceSummary) {
    *lock_operator_status_slot(access_set_source_slot(), "access-set source snapshot") = summary;
}

/// Record the latest conflict rate (basis points) for the pipeline DAG.
pub fn set_pipeline_conflict_rate_bps(bps: u64) {
    PIPELINE_CONFLICT_RATE_BPS.store(bps, Ordering::Relaxed);
}

/// Replace the dataspace-activity adapter diagnostic.
pub fn set_dataspace_activity_snapshot(entries: Vec<DataspaceActivitySnapshot>) {
    *lock_operator_status_slot(dataspace_activity_slot(), "dataspace activity snapshot") = entries;
}

fn lane_commitments_slot() -> &'static Mutex<Vec<LaneCommitmentSnapshot>> {
    LANE_COMMITMENTS.get_or_init(|| Mutex::new(Vec::new()))
}

fn dataspace_commitments_slot() -> &'static Mutex<Vec<DataspaceCommitmentSnapshot>> {
    DATASPACE_COMMITMENTS.get_or_init(|| Mutex::new(Vec::new()))
}

fn lane_settlement_commitments_slot() -> &'static Mutex<Vec<LaneBlockCommitment>> {
    LANE_SETTLEMENT_COMMITMENTS.get_or_init(|| Mutex::new(Vec::new()))
}

fn lane_relay_envelopes_slot() -> &'static Mutex<Vec<LaneRelayEnvelope>> {
    LANE_RELAY_ENVELOPES.get_or_init(|| Mutex::new(Vec::new()))
}

fn lane_payload_ownerships_slot() -> &'static Mutex<Vec<SumeragiLanePayloadOwnership>> {
    LANE_PAYLOAD_OWNERSHIPS.get_or_init(|| Mutex::new(Vec::new()))
}

fn committed_lane_blocks_slot() -> &'static Mutex<Vec<CommittedLaneBlockSnapshot>> {
    COMMITTED_LANE_BLOCKS.get_or_init(|| Mutex::new(Vec::new()))
}

fn lane_block_sessions_slot() -> &'static Mutex<Vec<SumeragiLaneBlockSessionStatus>> {
    LANE_BLOCK_SESSIONS.get_or_init(|| Mutex::new(Vec::new()))
}

type LaneRelayKey = (
    iroha_data_model::nexus::LaneId,
    iroha_data_model::nexus::DataSpaceId,
    u64,
    HashOf<BlockHeader>,
    Option<HashOf<DaCommitmentBundle>>,
    Option<Hash>,
    HashOf<LaneBlockCommitment>,
    u64,
    Option<[u8; 32]>,
);

fn lane_relay_key(envelope: &LaneRelayEnvelope) -> LaneRelayKey {
    (
        envelope.lane_id,
        envelope.dataspace_id,
        envelope.block_height,
        envelope.block_header.hash(),
        envelope.da_commitment_hash,
        envelope.lane_block_descriptor_hash,
        envelope.settlement_hash,
        envelope.rbc_bytes_total,
        envelope.manifest_root,
    )
}

fn record_relay_error(err: &LaneRelayError) {
    if let Some(metrics) = metrics::global() {
        metrics
            .lane_relay_invalid_total
            .with_label_values(&[err.as_label()])
            .inc();
    }
}

fn upsert_lane_relay_envelope(storage: &mut Vec<LaneRelayEnvelope>, envelope: LaneRelayEnvelope) {
    match envelope.verify().and_then(|()| {
        if envelope.fastpq_proof.is_some() {
            envelope.verify_fastpq_proof_material()
        } else {
            Ok(())
        }
    }) {
        Ok(()) => {}
        Err(err) => {
            record_relay_error(&err);
            iroha_logger::warn!(
                lane_id = %envelope.lane_id,
                dataspace_id = %envelope.dataspace_id,
                block_height = envelope.block_height,
                error_kind = err.as_label(),
                error = %err,
                "dropping lane relay envelope with failed structural verification"
            );
            return;
        }
    }

    let key = lane_relay_key(&envelope);
    if let Some(existing) = storage
        .iter()
        .position(|candidate| lane_relay_key(candidate) == key)
    {
        if storage[existing].is_merge_admissible() && !envelope.is_merge_admissible() {
            return;
        }
        storage[existing] = envelope;
    } else {
        storage.push(envelope);
        if storage.len() > LANE_RELAY_ENVELOPES_CAP {
            let drain = storage.len() - LANE_RELAY_ENVELOPES_CAP;
            storage.drain(0..drain);
        }
    }
}

/// Replace the aggregated lane/dataspace commitment snapshots used by Nexus diagnostics.
pub fn set_lane_commitments(
    lane_entries: Vec<LaneCommitmentSnapshot>,
    dataspace_entries: Vec<DataspaceCommitmentSnapshot>,
) {
    {
        let mut guard =
            lock_operator_status_slot(lane_commitments_slot(), "lane commitments snapshot");
        *guard = lane_entries;
    }
    {
        let mut guard = lock_operator_status_slot(
            dataspace_commitments_slot(),
            "dataspace commitments snapshot",
        );
        *guard = dataspace_entries;
    }
}

/// Replace the aggregated lane settlement commitments used by Nexus diagnostics.
pub fn set_lane_settlement_commitments(entries: Vec<LaneBlockCommitment>) {
    let mut guard = lock_operator_status_slot(
        lane_settlement_commitments_slot(),
        "lane settlement commitments snapshot",
    );
    *guard = entries;
}

/// Replace the stored lane relay envelopes captured during block sealing.
pub fn set_lane_relay_envelopes(entries: Vec<LaneRelayEnvelope>) {
    let mut guard =
        lock_operator_status_slot(lane_relay_envelopes_slot(), "lane relay envelopes snapshot");
    guard.clear();
    for envelope in entries {
        upsert_lane_relay_envelope(&mut guard, envelope);
    }
}

/// Append a single validated lane relay envelope to the cached snapshot.
pub fn push_lane_relay_envelope(envelope: LaneRelayEnvelope) {
    let mut guard =
        lock_operator_status_slot(lane_relay_envelopes_slot(), "lane relay envelopes snapshot");
    upsert_lane_relay_envelope(&mut guard, envelope);
}

/// Update the planned lane-local DA ownership identities used by Nexus diagnostics.
///
/// Updates are merged by `(lane_id, dataspace_id)` so a proposal for one lane
/// does not erase the latest ownership evidence for another active lane. Empty
/// updates are no-ops; use [`clear_lane_payload_ownerships`] for deliberate
/// test/shutdown cleanup.
pub fn set_lane_payload_ownerships(mut entries: Vec<SumeragiLanePayloadOwnership>) {
    entries.retain(|entry| match entry.validate_replay_material() {
        Ok(()) => true,
        Err(err) => {
            iroha_logger::warn!(
                lane_id = %entry.lane_id,
                dataspace_id = %entry.dataspace_id,
                lane_block_height = entry.lane_block_height,
                lane_block_view = entry.lane_block_view,
                error = %err,
                "dropping lane payload ownership status with invalid replay material"
            );
            false
        }
    });
    let mut guard = lock_operator_status_slot(
        lane_payload_ownerships_slot(),
        "lane payload ownership snapshot",
    );
    if entries.is_empty() {
        return;
    }
    for entry in entries {
        upsert_lane_payload_ownership(&mut guard, entry);
    }
    if guard.len() > LANE_PAYLOAD_OWNERSHIPS_CAP {
        guard.sort_by_key(lane_payload_ownership_retention_key);
        let drain = guard.len() - LANE_PAYLOAD_OWNERSHIPS_CAP;
        guard.drain(0..drain);
    }
}

/// Clear all cached lane-local DA/RBC ownership identities.
pub fn clear_lane_payload_ownerships() {
    let mut guard = lock_operator_status_slot(
        lane_payload_ownerships_slot(),
        "lane payload ownership snapshot",
    );
    guard.clear();
}

fn upsert_lane_payload_ownership(
    entries: &mut Vec<SumeragiLanePayloadOwnership>,
    entry: SumeragiLanePayloadOwnership,
) {
    if let Some(existing) = entries.iter_mut().find(|existing| {
        existing.lane_id == entry.lane_id && existing.dataspace_id == entry.dataspace_id
    }) {
        if lane_payload_ownership_retention_key(&entry)
            >= lane_payload_ownership_retention_key(existing)
        {
            *existing = entry;
        }
        return;
    }
    entries.push(entry);
}

fn lane_payload_ownership_retention_key(
    entry: &SumeragiLanePayloadOwnership,
) -> (u64, u64, u64, u64, u32, u64) {
    (
        entry.lane_block_height,
        entry.lane_block_view,
        entry.proposal_height,
        entry.proposal_view,
        entry.lane_id.as_u32(),
        entry.dataspace_id.as_u64(),
    )
}

fn validate_committed_lane_block_snapshot(
    entry: &CommittedLaneBlockSnapshot,
) -> Result<(), String> {
    let descriptor = &entry.proposal.descriptor;
    if entry.lane_id != descriptor.lane_id
        || entry.dataspace_id != descriptor.dataspace_id
        || entry.lane_block_height != descriptor.lane_block_height
        || entry.lane_block_view != descriptor.lane_block_view
        || entry.descriptor_hash != descriptor.descriptor_hash
        || entry.proposal_hash != entry.proposal.proposal_hash
    {
        return Err("summary fields do not match embedded lane-block proposal".to_owned());
    }

    let session = crate::lane_consensus::CommittedLaneBlockSession {
        proposal: entry.proposal.clone(),
        prepare_qc: entry.prepare_qc.clone(),
        commit_qc: entry.commit_qc.clone(),
    };
    crate::lane_consensus::validate_committed_lane_block_session(&session)
        .map_err(|err| err.to_string())
}

/// Replace the committed standalone lane-block snapshot used by Nexus diagnostics.
pub fn set_committed_lane_blocks(mut entries: Vec<CommittedLaneBlockSnapshot>) {
    entries.retain(
        |entry| match validate_committed_lane_block_snapshot(entry) {
            Ok(()) => true,
            Err(err) => {
                iroha_logger::warn!(
                    lane_id = %entry.lane_id,
                    dataspace_id = %entry.dataspace_id,
                    lane_block_height = entry.lane_block_height,
                    lane_block_view = entry.lane_block_view,
                    error = %err,
                    "dropping committed lane block status with invalid certified identity"
                );
                false
            }
        },
    );
    if entries.len() > COMMITTED_LANE_BLOCKS_CAP {
        let drain = entries.len() - COMMITTED_LANE_BLOCKS_CAP;
        entries.drain(0..drain);
    }
    let mut guard = lock_operator_status_slot(
        committed_lane_blocks_slot(),
        "committed lane block snapshot",
    );
    *guard = entries;
}

/// Remove lane-scoped operator status snapshots for lanes whose runtime state was reset.
pub fn prune_lane_scoped_snapshots(lanes_to_reset: &BTreeSet<LaneId>) {
    if lanes_to_reset.is_empty() {
        return;
    }
    let lane_matches = |lane_id: u32| lanes_to_reset.contains(&LaneId::new(lane_id));

    lock_operator_status_slot(lane_activity_slot(), "lane activity snapshot")
        .retain(|entry| !lane_matches(entry.lane_id));
    lock_operator_status_slot(dataspace_activity_slot(), "dataspace activity snapshot")
        .retain(|entry| !lane_matches(entry.lane_id));
    lock_operator_status_slot(lane_commitments_slot(), "lane commitments snapshot")
        .retain(|entry| !lane_matches(entry.lane_id));
    lock_operator_status_slot(
        dataspace_commitments_slot(),
        "dataspace commitments snapshot",
    )
    .retain(|entry| !lane_matches(entry.lane_id));
    lock_operator_status_slot(
        lane_settlement_commitments_slot(),
        "lane settlement commitments snapshot",
    )
    .retain(|entry| !lanes_to_reset.contains(&entry.lane_id));
    lock_operator_status_slot(lane_relay_envelopes_slot(), "lane relay envelopes snapshot")
        .retain(|entry| !lanes_to_reset.contains(&entry.lane_id));
    lock_operator_status_slot(
        lane_payload_ownerships_slot(),
        "lane payload ownership snapshot",
    )
    .retain(|entry| !lanes_to_reset.contains(&entry.lane_id));
    lock_operator_status_slot(
        committed_lane_blocks_slot(),
        "committed lane block snapshot",
    )
    .retain(|entry| !lanes_to_reset.contains(&entry.lane_id));
    lock_operator_status_slot(lane_block_sessions_slot(), "lane block sessions snapshot")
        .retain(|entry| !lanes_to_reset.contains(&entry.lane_id));
    lock_operator_status_slot(lane_governance_slot(), "lane governance snapshot")
        .retain(|entry| !lane_matches(entry.lane_id));
}

fn lane_commitments_snapshot() -> Vec<LaneCommitmentSnapshot> {
    lock_operator_status_slot(lane_commitments_slot(), "lane commitments snapshot").clone()
}

fn dataspace_commitments_snapshot() -> Vec<DataspaceCommitmentSnapshot> {
    lock_operator_status_slot(
        dataspace_commitments_slot(),
        "dataspace commitments snapshot",
    )
    .clone()
}

fn lane_settlement_commitments_snapshot() -> Vec<LaneBlockCommitment> {
    lock_operator_status_slot(
        lane_settlement_commitments_slot(),
        "lane settlement commitments snapshot",
    )
    .clone()
}

#[allow(dead_code)]
/// Return the cached lane relay envelopes used by Nexus diagnostics.
pub fn lane_relay_envelopes_snapshot() -> Vec<LaneRelayEnvelope> {
    lock_operator_status_slot(lane_relay_envelopes_slot(), "lane relay envelopes snapshot").clone()
}

/// Return the cached lane-local DA ownership snapshot used by Nexus diagnostics.
pub fn lane_payload_ownerships_snapshot() -> Vec<SumeragiLanePayloadOwnership> {
    lock_operator_status_slot(
        lane_payload_ownerships_slot(),
        "lane payload ownership snapshot",
    )
    .clone()
}

/// Return the cached standalone committed lane-block snapshot used by Nexus diagnostics.
pub fn committed_lane_blocks_snapshot() -> Vec<CommittedLaneBlockSnapshot> {
    lock_operator_status_slot(
        committed_lane_blocks_slot(),
        "committed lane block snapshot",
    )
    .clone()
}

/// Replace the cached standalone lane-block session snapshot used by Nexus diagnostics.
pub fn set_lane_block_sessions(entries: Vec<SumeragiLaneBlockSessionStatus>) {
    *lock_operator_status_slot(lane_block_sessions_slot(), "lane block sessions snapshot") =
        entries;
}

/// Return the cached standalone lane-block session snapshot used by Nexus diagnostics.
pub fn lane_block_sessions_snapshot() -> Vec<SumeragiLaneBlockSessionStatus> {
    lock_operator_status_slot(lane_block_sessions_slot(), "lane block sessions snapshot").clone()
}

fn lane_governance_slot() -> &'static Mutex<Vec<LaneGovernanceSnapshot>> {
    LANE_GOVERNANCE.get_or_init(|| Mutex::new(Vec::new()))
}

/// Replace the governance manifest snapshot used by Nexus diagnostics.
pub fn set_lane_governance_snapshot(entries: Vec<LaneGovernanceSnapshot>) {
    *lock_operator_status_slot(lane_governance_slot(), "lane governance snapshot") = entries;
}

#[cfg_attr(not(any(test, feature = "telemetry")), allow(dead_code))]
/// Return the cached governance manifest snapshot used by Nexus diagnostics.
pub fn lane_governance_snapshot() -> Vec<LaneGovernanceSnapshot> {
    lock_operator_status_slot(lane_governance_slot(), "lane governance snapshot").clone()
}

fn runtime_upgrade_hook_snapshot(hook: &RuntimeUpgradeHook) -> LaneRuntimeUpgradeHookSnapshot {
    LaneRuntimeUpgradeHookSnapshot {
        allow: hook.allow,
        require_metadata: hook.require_metadata,
        metadata_key: hook
            .metadata_key
            .as_ref()
            .map(std::string::ToString::to_string),
        allowed_ids: hook
            .allowed_ids
            .as_ref()
            .map(|ids| ids.iter().cloned().collect())
            .unwrap_or_default(),
    }
}

fn governance_rules_snapshot(
    rules: &GovernanceRules,
) -> (
    Vec<String>,
    Option<u32>,
    Vec<String>,
    Option<LaneRuntimeUpgradeHookSnapshot>,
) {
    let validators = rules
        .validators
        .iter()
        .map(std::string::ToString::to_string)
        .collect();
    let quorum = rules.quorum;
    let protected_namespaces = rules
        .protected_namespaces
        .iter()
        .map(std::string::ToString::to_string)
        .collect();
    let runtime_upgrade = rules
        .hooks
        .runtime_upgrade
        .as_ref()
        .map(runtime_upgrade_hook_snapshot);
    (validators, quorum, protected_namespaces, runtime_upgrade)
}

/// Update governance manifest snapshots from the provided registry statuses.
pub fn update_lane_governance_from_statuses(statuses: &[LaneManifestStatus]) {
    let snapshots = statuses
        .iter()
        .map(|status| {
            let manifest_required = status.governance.is_some();
            let manifest_ready = manifest_required && status.manifest_path.is_some();
            let manifest_path = status
                .manifest_path
                .as_ref()
                .map(|path| path.display().to_string());
            let mut snapshot = LaneGovernanceSnapshot {
                lane_id: status.lane.as_u32(),
                alias: status.alias.clone(),
                dataspace_id: status.dataspace.as_u64(),
                visibility: status.visibility.as_str().to_string(),
                storage_profile: status.storage.as_str().to_string(),
                governance: status.governance.clone(),
                manifest_required,
                manifest_ready,
                manifest_path,
                ..LaneGovernanceSnapshot::default()
            };
            if let Some(rules) = status.governance_rules.as_ref() {
                let (validators, quorum, namespaces, runtime_upgrade) =
                    governance_rules_snapshot(rules);
                snapshot.validator_ids = validators;
                snapshot.quorum = quorum;
                snapshot.protected_namespaces = namespaces;
                snapshot.runtime_upgrade = runtime_upgrade;
            }
            snapshot.privacy_commitments = status
                .privacy_commitments
                .iter()
                .map(LanePrivacyCommitmentSnapshot::from)
                .collect();
            snapshot
        })
        .collect();
    set_lane_governance_snapshot(snapshots);
}

/// Lane-local Nexus diagnostics kept separate from global v2 consensus status.
#[derive(Clone, Debug, Default)]
pub struct StatusSnapshot {
    /// Aggregate block-pipeline execution diagnostics; this is adapter state,
    /// not a global consensus phase or recovery signal.
    pub pipeline_execution: PipelineExecutionSnapshot,
    /// Lane-local block commitments retained for Nexus diagnostics.
    pub lane_commitments: Vec<LaneCommitmentSnapshot>,
    /// Dataspace-local commitments retained for Nexus diagnostics.
    pub dataspace_commitments: Vec<DataspaceCommitmentSnapshot>,
    /// Lane-local settlement commitments.
    pub lane_settlement_commitments: Vec<LaneBlockCommitment>,
    /// Certified lane relay envelopes.
    pub lane_relay_envelopes: Vec<LaneRelayEnvelope>,
    /// Lane-local payload ownership commitments.
    pub lane_payload_ownerships: Vec<SumeragiLanePayloadOwnership>,
    /// Standalone committed lane-block state.
    pub committed_lane_blocks: Vec<CommittedLaneBlockSnapshot>,
    /// Lane-local consensus sessions.
    pub lane_block_sessions: Vec<SumeragiLaneBlockSessionStatus>,
    /// Count of governance-sealed lanes.
    pub lane_governance_sealed_total: u32,
    /// Aliases of governance-sealed lanes.
    pub lane_governance_sealed_aliases: Vec<String>,
    /// Lane governance readiness.
    pub lane_governance: Vec<LaneGovernanceSnapshot>,
}

fn lane_governance_sealed_summary() -> (u32, Vec<String>, Vec<LaneGovernanceSnapshot>) {
    let lane_governance = lane_governance_snapshot();
    let aliases: Vec<_> = lane_governance
        .iter()
        .filter(|entry| entry.manifest_required && !entry.manifest_ready)
        .map(|entry| entry.alias.clone())
        .collect();
    let total = u32::try_from(aliases.len()).unwrap_or(u32::MAX);
    (total, aliases, lane_governance)
}

/// Snapshot non-consensus Nexus lane diagnostics.
#[must_use]
pub fn snapshot() -> StatusSnapshot {
    let (lane_governance_sealed_total, lane_governance_sealed_aliases, lane_governance) =
        lane_governance_sealed_summary();
    StatusSnapshot {
        pipeline_execution: lock_operator_status_slot(
            pipeline_execution_slot(),
            "pipeline execution snapshot",
        )
        .clone(),
        lane_commitments: lane_commitments_snapshot(),
        dataspace_commitments: dataspace_commitments_snapshot(),
        lane_settlement_commitments: lane_settlement_commitments_snapshot(),
        lane_relay_envelopes: lane_relay_envelopes_snapshot(),
        lane_payload_ownerships: lane_payload_ownerships_snapshot(),
        committed_lane_blocks: committed_lane_blocks_snapshot(),
        lane_block_sessions: lane_block_sessions_snapshot(),
        lane_governance_sealed_total,
        lane_governance_sealed_aliases,
        lane_governance,
    }
}

/// Latest transaction-queue pressure published for operator queries.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TxQueueBackpressureSnapshot {
    /// Number of transactions waiting in the local queue.
    pub depth: u64,
    /// Configured transaction queue capacity.
    pub capacity: u64,
    /// Estimated retained transaction queue bytes.
    pub retained_bytes: u64,
    /// Configured retained transaction queue byte budget.
    pub max_retained_bytes: u64,
    /// Whether the queue reached capacity. This mirrors the public `saturated` field.
    pub saturated: bool,
    /// Whether the queue reached capacity.
    pub saturated_by_count: bool,
    /// Whether the queue exhausted its retained-byte budget.
    pub saturated_by_bytes: bool,
    /// Whether the oldest queued transaction exceeded the latency budget.
    pub saturated_by_age: bool,
    /// Age in milliseconds of the oldest queued transaction.
    pub oldest_queued_age_ms: u64,
}

/// Record the latest transaction-queue pressure snapshot for operator queries.
pub fn set_tx_queue_pressure(snapshot: QueuePressureSnapshot) {
    let saturated_by_count = snapshot.saturated_by_count;
    let saturated_by_bytes = snapshot.saturated_by_bytes;
    let saturated = saturated_by_count || saturated_by_bytes;
    TX_QUEUE_DEPTH.store(snapshot.queued_tx_count as u64, Ordering::Relaxed);
    TX_QUEUE_CAPACITY.store(snapshot.capacity.get() as u64, Ordering::Relaxed);
    TX_QUEUE_RETAINED_BYTES.store(snapshot.retained_bytes, Ordering::Relaxed);
    TX_QUEUE_MAX_RETAINED_BYTES.store(snapshot.max_retained_bytes.get(), Ordering::Relaxed);
    TX_QUEUE_SATURATED.store(saturated, Ordering::Relaxed);
    TX_QUEUE_SATURATED_BY_COUNT.store(saturated_by_count, Ordering::Relaxed);
    TX_QUEUE_SATURATED_BY_BYTES.store(saturated_by_bytes, Ordering::Relaxed);
    TX_QUEUE_SATURATED_BY_AGE.store(snapshot.saturated_by_age, Ordering::Relaxed);
    TX_QUEUE_OLDEST_QUEUED_AGE_MS.store(snapshot.oldest_queued_tx_age_ms, Ordering::Relaxed);
}

/// Record the latest transaction-queue backpressure snapshot for operator queries.
pub fn set_tx_queue_backpressure(state: BackpressureState) {
    match state {
        BackpressureState::Healthy { queued, capacity } => {
            TX_QUEUE_DEPTH.store(queued as u64, Ordering::Relaxed);
            TX_QUEUE_CAPACITY.store(capacity.get() as u64, Ordering::Relaxed);
            TX_QUEUE_RETAINED_BYTES.store(0, Ordering::Relaxed);
            TX_QUEUE_MAX_RETAINED_BYTES.store(0, Ordering::Relaxed);
            TX_QUEUE_SATURATED.store(false, Ordering::Relaxed);
            TX_QUEUE_SATURATED_BY_COUNT.store(false, Ordering::Relaxed);
            TX_QUEUE_SATURATED_BY_BYTES.store(false, Ordering::Relaxed);
            TX_QUEUE_SATURATED_BY_AGE.store(false, Ordering::Relaxed);
            TX_QUEUE_OLDEST_QUEUED_AGE_MS.store(0, Ordering::Relaxed);
        }
        BackpressureState::Saturated { queued, capacity } => {
            TX_QUEUE_DEPTH.store(queued as u64, Ordering::Relaxed);
            TX_QUEUE_CAPACITY.store(capacity.get() as u64, Ordering::Relaxed);
            TX_QUEUE_RETAINED_BYTES.store(0, Ordering::Relaxed);
            TX_QUEUE_MAX_RETAINED_BYTES.store(0, Ordering::Relaxed);
            TX_QUEUE_SATURATED.store(true, Ordering::Relaxed);
            TX_QUEUE_SATURATED_BY_COUNT.store(true, Ordering::Relaxed);
            TX_QUEUE_SATURATED_BY_BYTES.store(false, Ordering::Relaxed);
            TX_QUEUE_SATURATED_BY_AGE.store(false, Ordering::Relaxed);
            TX_QUEUE_OLDEST_QUEUED_AGE_MS.store(0, Ordering::Relaxed);
        }
    }
}

/// Snapshot the recorded transaction-queue backpressure state.
pub fn tx_queue_backpressure() -> TxQueueBackpressureSnapshot {
    TxQueueBackpressureSnapshot {
        depth: TX_QUEUE_DEPTH.load(Ordering::Relaxed),
        capacity: TX_QUEUE_CAPACITY.load(Ordering::Relaxed),
        retained_bytes: TX_QUEUE_RETAINED_BYTES.load(Ordering::Relaxed),
        max_retained_bytes: TX_QUEUE_MAX_RETAINED_BYTES.load(Ordering::Relaxed),
        saturated: TX_QUEUE_SATURATED.load(Ordering::Relaxed),
        saturated_by_count: TX_QUEUE_SATURATED_BY_COUNT.load(Ordering::Relaxed),
        saturated_by_bytes: TX_QUEUE_SATURATED_BY_BYTES.load(Ordering::Relaxed),
        saturated_by_age: TX_QUEUE_SATURATED_BY_AGE.load(Ordering::Relaxed),
        oldest_queued_age_ms: TX_QUEUE_OLDEST_QUEUED_AGE_MS.load(Ordering::Relaxed),
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TestLockOwner {
    Task(tokio::task::Id),
    Thread(std::thread::ThreadId),
}

#[cfg(test)]
thread_local! {
    static TEST_LOCK_OWNER_OVERRIDE: std::cell::Cell<Option<TestLockOwner>> =
        const { std::cell::Cell::new(None) };
}

#[cfg(test)]
impl TestLockOwner {
    fn current() -> Self {
        if let Some(owner) = TEST_LOCK_OWNER_OVERRIDE.with(std::cell::Cell::get) {
            return owner;
        }
        tokio::task::try_id().map_or_else(|| Self::Thread(std::thread::current().id()), Self::Task)
    }
}

#[cfg(test)]
#[derive(Default)]
struct TestLockState {
    owner: Option<TestLockOwner>,
    depth: usize,
}

#[cfg(test)]
#[derive(Default)]
struct TestLock {
    state: Mutex<TestLockState>,
    cvar: Condvar,
}

#[cfg(test)]
pub(crate) struct TestLockGuard {
    lock: &'static TestLock,
    owner: TestLockOwner,
}

#[cfg(test)]
impl Drop for TestLockGuard {
    fn drop(&mut self) {
        let mut state = self
            .lock
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.owner == Some(self.owner) {
            state.depth = state.depth.saturating_sub(1);
            if state.depth == 0 {
                state.owner = None;
                self.lock.cvar.notify_one();
            }
        }
    }
}

#[cfg(test)]
static STATUS_TEST_GLOBAL_LOCK: OnceLock<TestLock> = OnceLock::new();
#[cfg(test)]
static RBC_STATUS_TEST_LOCK: OnceLock<TestLock> = OnceLock::new();
#[cfg(test)]
static PEER_KEY_POLICY_TEST_LOCK: OnceLock<TestLock> = OnceLock::new();
#[cfg(test)]
static LOCAL_REMOVED_TEST_LOCK: OnceLock<TestLock> = OnceLock::new();
#[cfg(test)]
static LANE_RELAY_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[cfg(test)]
fn canonical_test_lock(_: &'static OnceLock<TestLock>) -> &'static TestLock {
    STATUS_TEST_GLOBAL_LOCK.get_or_init(TestLock::default)
}

#[cfg(test)]
fn reentrant_test_guard(lock: &'static OnceLock<TestLock>) -> TestLockGuard {
    let owner = TestLockOwner::current();
    let lock = canonical_test_lock(lock);
    let mut state = lock
        .state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    loop {
        match state.owner {
            None => {
                state.owner = Some(owner);
                state.depth = 1;
                break;
            }
            Some(current) if current == owner => {
                state.depth = state.depth.saturating_add(1);
                break;
            }
            Some(_) => {
                state = lock
                    .cvar
                    .wait(state)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
            }
        }
    }
    TestLockGuard { lock, owner }
}

#[cfg(test)]
fn try_reentrant_test_guard(lock: &'static OnceLock<TestLock>) -> Option<TestLockGuard> {
    let owner = TestLockOwner::current();
    let lock = canonical_test_lock(lock);
    let mut state = lock
        .state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    match state.owner {
        None => {
            state.owner = Some(owner);
            state.depth = 1;
            Some(TestLockGuard { lock, owner })
        }
        Some(current) if current == owner => {
            state.depth = state.depth.saturating_add(1);
            Some(TestLockGuard { lock, owner })
        }
        Some(_) => None,
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct NexusFeeTestLock;

#[cfg(test)]
pub(crate) struct NexusFeeTestGuard(TestLockGuard);

#[cfg(test)]
impl NexusFeeTestLock {
    pub(crate) fn lock(&'static self) -> Result<NexusFeeTestGuard, std::convert::Infallible> {
        Ok(NexusFeeTestGuard(reentrant_test_guard(
            &RBC_STATUS_TEST_LOCK,
        )))
    }
}

#[cfg(test)]
pub(crate) fn rbc_status_test_guard() -> TestLockGuard {
    reentrant_test_guard(&RBC_STATUS_TEST_LOCK)
}

#[cfg(test)]
pub(crate) fn peer_key_policy_test_guard() -> TestLockGuard {
    reentrant_test_guard(&PEER_KEY_POLICY_TEST_LOCK)
}

#[cfg(test)]
pub(crate) fn local_removed_test_guard() -> TestLockGuard {
    reentrant_test_guard(&LOCAL_REMOVED_TEST_LOCK)
}

#[cfg(test)]
pub(crate) fn lane_relay_test_guard() -> std::sync::MutexGuard<'static, ()> {
    LANE_RELAY_TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("lane relay test lock poisoned")
}

#[cfg(test)]
/// Reset settlement telemetry counters for isolated tests.
pub fn settlement_status_reset_for_tests() {
    *lock_operator_status_slot(settlement_status_slot(), "settlement status") =
        SettlementStatusState::default();
}
