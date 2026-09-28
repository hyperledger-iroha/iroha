//! Process-local, non-consensus operator diagnostics.
//!
//! These snapshots are not consensus state: Nexus fee and public-lane staking
//! economics, DvP/PvP settlement events, lane relay envelopes, lane and
//! dataspace commitment snapshots, lane governance readiness, block-pipeline
//! execution diagnostics, the gossip duplicate counter, transaction-queue
//! pressure, peer key policy rejects, and the local-peer-removed flag.
//! Consensus status is published separately by the consensus driver.
use crate::{
    governance::manifest::{GovernanceRules, LaneManifestStatus, RuntimeUpgradeHook},
    queue::QueuePressureSnapshot,
};
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
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
            COMMITTED_LANE_STATUS_STATE_APPLIED_BY_CANONICAL_BLOCK, LaneBlockCommitment,
            LaneBlockProposalV1, LaneBlockQcV1, SumeragiLaneBlockSessionStatus,
            SumeragiLanePayloadOwnership,
        },
    },
    isi::settlement::{SettlementAtomicity, SettlementExecutionOrder},
    nexus::{LaneRelayEnvelope, LaneRelayError},
};
use iroha_model_base::{topology::DataSpaceId, topology::LaneId};
use iroha_primitives::numeric::Quantity;
use iroha_telemetry::metrics;
#[cfg(test)]
use std::sync::Condvar;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Mutex, MutexGuard, OnceLock},
};
pub(crate) fn lock_operator_status_slot<T>(
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
static DATASPACE_ACTIVITY: OnceLock<Mutex<Vec<DataspaceActivitySnapshot>>> = OnceLock::new();
static LANE_COMMITMENTS: OnceLock<Mutex<Vec<LaneCommitmentSnapshot>>> = OnceLock::new();
static DATASPACE_COMMITMENTS: OnceLock<Mutex<Vec<DataspaceCommitmentSnapshot>>> = OnceLock::new();
static LANE_SETTLEMENT_COMMITMENTS: OnceLock<Mutex<Vec<LaneBlockCommitment>>> = OnceLock::new();
static LANE_RELAY_ENVELOPES: OnceLock<Mutex<Vec<LaneRelayEnvelope>>> = OnceLock::new();
static LANE_GOVERNANCE: OnceLock<Mutex<Vec<LaneGovernanceSnapshot>>> = OnceLock::new();
static NEXUS_FEE_STATUS: OnceLock<Mutex<NexusFeeSnapshot>> = OnceLock::new();
#[derive(Debug, Default)]
struct NexusStakingStatusState {
    lanes: BTreeMap<LaneId, NexusStakingLaneSnapshot>,
    reset_epoch: u64,
}
static NEXUS_STAKING_STATUS: OnceLock<Mutex<NexusStakingStatusState>> = OnceLock::new();
enum PublicLaneStakingStatusUpdate {
    Bonded {
        lane_id: LaneId,
        amount: Quantity,
        increase: bool,
    },
    PendingUnbond {
        lane_id: LaneId,
        amount: Quantity,
        increase: bool,
    },
    Slash {
        lane_id: LaneId,
    },
}
#[derive(Default)]
struct PublicLaneStakingStatusOverlayFrame {
    reset_epoch: u64,
    updates: Vec<PublicLaneStakingStatusUpdate>,
}
std::thread_local! {
    static PUBLIC_LANE_STAKING_STATUS_OVERLAYS:
        std::cell::RefCell<Vec<PublicLaneStakingStatusOverlayFrame>> =
        const { std::cell::RefCell::new(Vec::new()) };
}
/// Transaction-local overlay for process-local public-lane staking diagnostics.
///
/// Updates recorded on the creating thread remain private until [`Self::commit`]
/// is called. Dropping the guard discards them. Overlays are nestable and must
/// be completed in last-in, first-out order.
#[must_use = "dropping a public-lane staking status overlay rolls back its updates"]
pub(crate) struct PublicLaneStakingStatusOverlay {
    depth: usize,
    finished: bool,
    _not_send_or_sync: core::marker::PhantomData<std::rc::Rc<()>>,
}
impl PublicLaneStakingStatusOverlay {
    /// Merge staged updates into the parent overlay or publish them globally.
    pub(crate) fn commit(mut self) {
        self.finish(true);
    }
    fn finish(&mut self, commit: bool) {
        if self.finished {
            return;
        }
        self.finished = true;
        let updates = PUBLIC_LANE_STAKING_STATUS_OVERLAYS.with(|overlays| {
            let mut overlays = overlays.borrow_mut();
            assert_eq!(
                overlays.len(),
                self.depth,
                "public-lane staking status overlays must be completed in last-in, first-out order"
            );
            let mut frame = overlays
                .pop()
                .expect("public-lane staking status overlay stack must contain the active guard");
            if !commit {
                return None;
            }
            if let Some(parent) = overlays.last_mut() {
                parent.updates.append(&mut frame.updates);
                None
            } else {
                Some((frame.reset_epoch, frame.updates))
            }
        });
        if let Some((reset_epoch, updates)) = updates {
            apply_public_lane_staking_status_updates(Some(reset_epoch), updates);
        }
    }
}
impl Drop for PublicLaneStakingStatusOverlay {
    fn drop(&mut self) {
        self.finish(false);
    }
}
/// Begin a transaction-local public-lane staking diagnostics overlay.
pub(crate) fn begin_public_lane_staking_status_overlay() -> PublicLaneStakingStatusOverlay {
    let reset_epoch =
        lock_operator_status_slot(nexus_staking_slot(), "nexus staking status").reset_epoch;
    let depth = PUBLIC_LANE_STAKING_STATUS_OVERLAYS.with(|overlays| {
        let mut overlays = overlays.borrow_mut();
        overlays.push(PublicLaneStakingStatusOverlayFrame {
            reset_epoch,
            ..PublicLaneStakingStatusOverlayFrame::default()
        });
        overlays.len()
    });
    PublicLaneStakingStatusOverlay {
        depth,
        finished: false,
        _not_send_or_sync: core::marker::PhantomData,
    }
}
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
pub(crate) const LANE_PAYLOAD_OWNERSHIPS_CAP: usize = 128;
pub(crate) const COMMITTED_LANE_BLOCKS_CAP: usize = 128;
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
    /// Failures due to config/asset parsing errors.
    pub config_errors_total: u64,
    /// Failures while executing the fee debit.
    pub transfer_failures_total: u64,
    /// Last attempted fee amount if available.
    pub last_amount: Option<Quantity>,
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
        amount: Quantity,
        /// Asset definition id string.
        asset_id: String,
    },
    /// Fee debit failed to apply.
    TransferFailed {
        /// Payer classification.
        payer_kind: NexusFeePayer,
        /// Account that attempted to pay.
        payer_id: String,
        /// Amount attempted.
        amount: Quantity,
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
    pub bonded: Quantity,
    /// Total pending-unbond stake recorded.
    pub pending_unbond: Quantity,
    /// Total slashes applied.
    pub slash_total: u64,
}
impl Default for NexusStakingLaneSnapshot {
    fn default() -> Self {
        Self {
            lane_id: LaneId::new(0),
            bonded: Quantity::zero(),
            pending_unbond: Quantity::zero(),
            slash_total: 0,
        }
    }
}
#[cfg(test)]
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
#[cfg(test)]
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
/// Execution readiness for a certified lane-local block awaiting canonical merge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommittedLaneBlockExecutionStatus {
    /// The block has proposal/prepare/commit certificates, but no executable lane payload yet.
    AwaitingExecutablePayload,
    /// Accepted entrypoints are locally recoverable, but merge execution is not prepared yet.
    PayloadAvailableAwaitingExecutor,
    /// Accepted entrypoints have been durably recovered for canonical merge application.
    PayloadRecoveredAwaitingStateApplication,
    /// Recovered entrypoints passed execution preflight at the current canonical WSV base.
    PayloadPreflightedAwaitingStateApplication,
    /// Recovered entrypoints produced at least one rejection during execution preflight.
    PayloadPreflightRejectedAwaitingStateApplication,
    /// Canonical application receipt disagrees with durable execution preflight results.
    ApplicationReceiptConflictsWithPreflight,
    /// This lane block cannot execute until its certified predecessor is applied.
    AwaitingPredecessorApplication,
    /// Accepted entrypoints already have canonical committed results recorded locally.
    StateAppliedByCanonicalBlock,
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
            | Self::StateAppliedByCanonicalBlock => true,
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
    /// Build an operator snapshot from one fully validated committed lane session.
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
/// Bounded lane diagnostics reconstructed from current State and durable Kura evidence.
///
/// This snapshot intentionally excludes adapter/session caches so a restarted peer reports
/// the same durable lane identities as an uninterrupted peer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DurableLaneDiagnosticsSnapshot {
    /// Latest canonical payload ownership for each active lane route.
    pub lane_payload_ownerships: Vec<SumeragiLanePayloadOwnership>,
    /// Certified lane blocks and their durable execution readiness.
    pub committed_lane_blocks: Vec<CommittedLaneBlockSnapshot>,
    /// Durable certified-session summaries.
    pub lane_block_sessions: Vec<SumeragiLaneBlockSessionStatus>,
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
}
impl From<&LanePrivacyCommitment> for LanePrivacyCommitmentSnapshot {
    fn from(commitment: &LanePrivacyCommitment) -> Self {
        let scheme = match commitment.scheme() {
            CommitmentScheme::Merkle(merkle) => LanePrivacyCommitmentSchemeSnapshot::Merkle {
                root: hash_of_bytes(*merkle.root()),
                max_depth: merkle.max_depth(),
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
fn nexus_staking_slot() -> &'static Mutex<NexusStakingStatusState> {
    NEXUS_STAKING_STATUS.get_or_init(|| Mutex::new(NexusStakingStatusState::default()))
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
fn staking_lane_entry(
    status: &mut BTreeMap<LaneId, NexusStakingLaneSnapshot>,
    lane_id: LaneId,
) -> &mut NexusStakingLaneSnapshot {
    status
        .entry(lane_id)
        .or_insert_with(|| NexusStakingLaneSnapshot {
            lane_id,
            ..NexusStakingLaneSnapshot::default()
        })
}
fn adjust_quantity_value(current: Quantity, delta: &Quantity, increase: bool) -> Quantity {
    if delta.is_zero() {
        return current;
    }
    if increase {
        let base = current.clone();
        current.checked_add(delta).unwrap_or_else(|_| {
            iroha_logger::warn!(
                %base,
                %delta,
                "nexus staking accumulator overflowed; clamping to Quantity::zero()"
            );
            Quantity::zero()
        })
    } else {
        let base = current.clone();
        current.checked_sub(delta).unwrap_or_else(|_| {
            iroha_logger::warn!(
                %base,
                %delta,
                "nexus staking accumulator underflowed; clamping to Quantity::zero()"
            );
            Quantity::zero()
        })
    }
}
fn apply_public_lane_staking_status_update(
    status: &mut NexusStakingStatusState,
    update: PublicLaneStakingStatusUpdate,
) {
    match update {
        PublicLaneStakingStatusUpdate::Bonded {
            lane_id,
            amount,
            increase,
        } => {
            let snapshot = staking_lane_entry(&mut status.lanes, lane_id);
            snapshot.bonded = adjust_quantity_value(snapshot.bonded.clone(), &amount, increase);
        }
        PublicLaneStakingStatusUpdate::PendingUnbond {
            lane_id,
            amount,
            increase,
        } => {
            let snapshot = staking_lane_entry(&mut status.lanes, lane_id);
            snapshot.pending_unbond =
                adjust_quantity_value(snapshot.pending_unbond.clone(), &amount, increase);
        }
        PublicLaneStakingStatusUpdate::Slash { lane_id } => {
            let snapshot = staking_lane_entry(&mut status.lanes, lane_id);
            snapshot.slash_total = snapshot.slash_total.saturating_add(1);
        }
    }
}
fn apply_public_lane_staking_status_updates(
    expected_reset_epoch: Option<u64>,
    updates: impl IntoIterator<Item = PublicLaneStakingStatusUpdate>,
) {
    #[cfg(test)]
    let Some(_guard) = try_reentrant_test_guard(&RBC_STATUS_TEST_LOCK) else {
        return;
    };
    let mut status = lock_operator_status_slot(nexus_staking_slot(), "nexus staking status");
    if expected_reset_epoch.is_some_and(|epoch| epoch != status.reset_epoch) {
        return;
    }
    for update in updates {
        apply_public_lane_staking_status_update(&mut status, update);
    }
}
fn record_public_lane_staking_status_update(update: PublicLaneStakingStatusUpdate) {
    let unstaged = PUBLIC_LANE_STAKING_STATUS_OVERLAYS.with(|overlays| {
        let mut overlays = overlays.borrow_mut();
        if let Some(frame) = overlays.last_mut() {
            frame.updates.push(update);
            None
        } else {
            Some(update)
        }
    });
    if let Some(update) = unstaged {
        apply_public_lane_staking_status_updates(None, core::iter::once(update));
    }
}
/// Record a bonded stake delta for a Nexus lane.
pub fn record_public_lane_bonded_delta(lane_id: LaneId, amount: &Quantity, increase: bool) {
    record_public_lane_staking_status_update(PublicLaneStakingStatusUpdate::Bonded {
        lane_id,
        amount: amount.clone(),
        increase,
    });
}
/// Record a pending-unbond delta for a Nexus lane.
pub fn record_public_lane_pending_unbond_delta(lane_id: LaneId, amount: &Quantity, increase: bool) {
    record_public_lane_staking_status_update(PublicLaneStakingStatusUpdate::PendingUnbond {
        lane_id,
        amount: amount.clone(),
        increase,
    });
}
/// Record a slash event for a Nexus lane.
pub fn record_public_lane_slash(lane_id: LaneId) {
    record_public_lane_staking_status_update(PublicLaneStakingStatusUpdate::Slash { lane_id });
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
        guard.lanes.remove(lane_id);
    }
    guard.reset_epoch = guard
        .reset_epoch
        .checked_add(1)
        .expect("nexus staking reset epoch must not overflow");
}
#[cfg(test)]
/// Latest aggregated Nexus fee snapshot.
pub fn nexus_fee_snapshot() -> NexusFeeSnapshot {
    lock_operator_status_slot(nexus_fee_slot(), "nexus fee status").clone()
}
#[cfg(test)]
/// Latest aggregated Nexus staking snapshot.
pub fn nexus_staking_snapshot() -> NexusStakingSnapshot {
    let guard = lock_operator_status_slot(nexus_staking_slot(), "nexus staking status");
    let mut lanes: Vec<_> = guard.lanes.values().cloned().collect();
    lanes.sort_by_key(|lane| lane.lane_id.as_u32());
    NexusStakingSnapshot { lanes }
}
/// Shared lock for tests that mutate global Nexus fee state.
#[cfg(test)]
pub(crate) fn nexus_fee_test_lock() -> &'static NexusFeeTestLock {
    static LOCK: NexusFeeTestLock = NexusFeeTestLock;
    &LOCK
}
#[cfg(test)]
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
        guard.lanes.clear();
        guard.reset_epoch = guard
            .reset_epoch
            .checked_add(1)
            .expect("nexus staking reset epoch must not overflow");
    }
}
#[cfg(test)]
mod public_lane_staking_status_overlay_tests {
    use super::*;

    fn lane_snapshot(lane_id: LaneId) -> Option<NexusStakingLaneSnapshot> {
        nexus_staking_snapshot()
            .lanes
            .into_iter()
            .find(|lane| lane.lane_id == lane_id)
    }

    #[test]
    fn updates_remain_immediate_without_an_overlay() {
        let _guard = rbc_status_test_guard();
        reset_nexus_economics_for_tests();
        let lane_id = LaneId::new(41);

        record_public_lane_bonded_delta(lane_id, &Quantity::from(7_u32), true);
        record_public_lane_pending_unbond_delta(lane_id, &Quantity::from(2_u32), true);
        record_public_lane_slash(lane_id);

        let snapshot = lane_snapshot(lane_id).expect("unscoped updates must publish immediately");
        assert_eq!(snapshot.bonded, Quantity::from(7_u32));
        assert_eq!(snapshot.pending_unbond, Quantity::from(2_u32));
        assert_eq!(snapshot.slash_total, 1);
        reset_nexus_economics_for_tests();
    }

    #[test]
    fn dropping_an_overlay_discards_every_staking_update() {
        let _guard = rbc_status_test_guard();
        reset_nexus_economics_for_tests();
        let lane_id = LaneId::new(42);

        {
            let _overlay = begin_public_lane_staking_status_overlay();
            record_public_lane_bonded_delta(lane_id, &Quantity::from(7_u32), true);
            record_public_lane_pending_unbond_delta(lane_id, &Quantity::from(2_u32), true);
            record_public_lane_slash(lane_id);
            assert!(lane_snapshot(lane_id).is_none());
        }

        assert!(lane_snapshot(lane_id).is_none());
        reset_nexus_economics_for_tests();
    }

    #[test]
    fn committing_an_overlay_publishes_ordered_updates() {
        let _guard = rbc_status_test_guard();
        reset_nexus_economics_for_tests();
        let lane_id = LaneId::new(43);
        let overlay = begin_public_lane_staking_status_overlay();
        record_public_lane_bonded_delta(lane_id, &Quantity::from(5_u32), true);
        record_public_lane_bonded_delta(lane_id, &Quantity::from(10_u32), false);
        record_public_lane_bonded_delta(lane_id, &Quantity::from(3_u32), true);
        record_public_lane_pending_unbond_delta(lane_id, &Quantity::from(2_u32), true);
        record_public_lane_slash(lane_id);
        assert!(lane_snapshot(lane_id).is_none());

        overlay.commit();

        let snapshot = lane_snapshot(lane_id).expect("committed overlay must publish updates");
        assert_eq!(snapshot.bonded, Quantity::from(3_u32));
        assert_eq!(snapshot.pending_unbond, Quantity::from(2_u32));
        assert_eq!(snapshot.slash_total, 1);
        reset_nexus_economics_for_tests();
    }

    #[test]
    fn lifecycle_reset_prevents_a_stale_overlay_from_resurrecting_a_lane() {
        let _guard = rbc_status_test_guard();
        reset_nexus_economics_for_tests();
        let lane_id = LaneId::new(44);
        record_public_lane_bonded_delta(lane_id, &Quantity::from(1_u32), true);

        let overlay = begin_public_lane_staking_status_overlay();
        record_public_lane_bonded_delta(lane_id, &Quantity::from(5_u32), true);
        record_public_lane_slash(lane_id);
        let lanes_to_reset = BTreeSet::from([lane_id]);
        reset_public_lane_staking_lanes(&lanes_to_reset);
        assert!(lane_snapshot(lane_id).is_none());

        overlay.commit();

        assert!(lane_snapshot(lane_id).is_none());
        reset_nexus_economics_for_tests();
    }

    #[test]
    fn nested_commit_remains_private_and_follows_the_outer_outcome() {
        let _guard = rbc_status_test_guard();
        reset_nexus_economics_for_tests();
        let discarded_lane = LaneId::new(45);
        {
            let _outer = begin_public_lane_staking_status_overlay();
            record_public_lane_bonded_delta(discarded_lane, &Quantity::from(5_u32), true);
            let inner = begin_public_lane_staking_status_overlay();
            record_public_lane_slash(discarded_lane);
            inner.commit();
            assert!(lane_snapshot(discarded_lane).is_none());
        }
        assert!(lane_snapshot(discarded_lane).is_none());

        let committed_lane = LaneId::new(46);
        let outer = begin_public_lane_staking_status_overlay();
        record_public_lane_bonded_delta(committed_lane, &Quantity::from(5_u32), true);
        {
            let _inner = begin_public_lane_staking_status_overlay();
            record_public_lane_bonded_delta(committed_lane, &Quantity::from(9_u32), true);
            record_public_lane_slash(committed_lane);
        }
        let inner = begin_public_lane_staking_status_overlay();
        record_public_lane_pending_unbond_delta(committed_lane, &Quantity::from(2_u32), true);
        inner.commit();
        assert!(lane_snapshot(committed_lane).is_none());
        outer.commit();

        let snapshot =
            lane_snapshot(committed_lane).expect("outer commit must publish its updates");
        assert_eq!(snapshot.bonded, Quantity::from(5_u32));
        assert_eq!(snapshot.pending_unbond, Quantity::from(2_u32));
        assert_eq!(snapshot.slash_total, 0);
        reset_nexus_economics_for_tests();
    }
}
/// Reasons a peer-consensus-key admission can be rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeerKeyPolicyRejectReason {
    /// Public-key algorithm not allowed by policy.
    DisallowedAlgorithm,
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
            Self::DisallowedAlgorithm => "disallowed_algorithm",
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
/// Worker-loop queue identifiers used by the remaining async adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkerQueueKind {
    /// Vote-related messages.
    Votes,
    /// Block payload messages.
    BlockPayload,
    /// Fallback block/control messages.
    Blocks,
    /// Consensus control-flow messages.
    Consensus,
    /// Lane relay envelopes.
    LaneRelay,
    /// Background post requests.
    Background,
}
static WORKER_QUEUE_DEPTHS: [AtomicU64; 6] = [const { AtomicU64::new(0) }; 6];
static WORKER_QUEUE_DROPS: [AtomicU64; 6] = [const { AtomicU64::new(0) }; 6];
const fn worker_queue_index(kind: WorkerQueueKind) -> usize {
    match kind {
        WorkerQueueKind::Votes => 0,
        WorkerQueueKind::BlockPayload => 1,
        WorkerQueueKind::Blocks => 2,
        WorkerQueueKind::Consensus => 3,
        WorkerQueueKind::LaneRelay => 4,
        WorkerQueueKind::Background => 5,
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
fn dataspace_activity_slot() -> &'static Mutex<Vec<DataspaceActivitySnapshot>> {
    DATASPACE_ACTIVITY.get_or_init(|| Mutex::new(Vec::new()))
}
fn pipeline_execution_slot() -> &'static Mutex<PipelineExecutionSnapshot> {
    PIPELINE_EXECUTION.get_or_init(|| Mutex::new(PipelineExecutionSnapshot::default()))
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
type LaneRelayKey = (
    iroha_model_base::topology::LaneId,
    iroha_model_base::topology::DataSpaceId,
    Hash,
    u64,
);
fn lane_relay_key(envelope: &LaneRelayEnvelope) -> LaneRelayKey {
    (
        envelope.lane_id,
        envelope.dataspace_id,
        envelope.lane_incarnation,
        envelope.block_height,
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
            envelope.validate_fastpq_proof_metadata()
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
        if !storage[existing].same_finality_effect(&envelope) {
            let err = LaneRelayError::ConflictingRelay {
                lane: envelope.lane_id,
                height: envelope.block_height,
            };
            record_relay_error(&err);
            iroha_logger::warn!(
                lane_id = %envelope.lane_id,
                dataspace_id = %envelope.dataspace_id,
                block_height = envelope.block_height,
                error_kind = err.as_label(),
                "dropping conflicting lane relay envelope for finalized coordinates"
            );
            return;
        }
        if storage[existing].has_merge_admission_material()
            && !envelope.has_merge_admission_material()
        {
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
#[cfg(any(test, feature = "iroha-core-tests"))]
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
#[cfg(test)]
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
    lock_operator_status_slot(lane_governance_slot(), "lane governance snapshot")
        .retain(|entry| !lane_matches(entry.lane_id));
}
#[cfg(test)]
pub(crate) fn lane_scoped_status_fingerprint_for_tests() -> String {
    format!(
        "{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}",
        lock_operator_status_slot(lane_activity_slot(), "lane activity snapshot"),
        lock_operator_status_slot(dataspace_activity_slot(), "dataspace activity snapshot"),
        lock_operator_status_slot(lane_commitments_slot(), "lane commitments snapshot"),
        lock_operator_status_slot(
            dataspace_commitments_slot(),
            "dataspace commitments snapshot"
        ),
        lock_operator_status_slot(
            lane_settlement_commitments_slot(),
            "lane settlement commitments snapshot"
        ),
        lock_operator_status_slot(lane_relay_envelopes_slot(), "lane relay envelopes snapshot"),
        lock_operator_status_slot(lane_governance_slot(), "lane governance snapshot"),
        lock_operator_status_slot(nexus_staking_slot(), "nexus staking status")
            .lanes
            .clone(),
        lock_operator_status_slot(nexus_fee_slot(), "nexus fee status"),
    )
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
/// Return the cached lane relay envelopes used by Nexus diagnostics.
pub fn lane_relay_envelopes_snapshot() -> Vec<LaneRelayEnvelope> {
    lock_operator_status_slot(lane_relay_envelopes_slot(), "lane relay envelopes snapshot").clone()
}
fn lane_governance_slot() -> &'static Mutex<Vec<LaneGovernanceSnapshot>> {
    LANE_GOVERNANCE.get_or_init(|| Mutex::new(Vec::new()))
}
/// Replace the governance manifest snapshot used by Nexus diagnostics.
pub fn set_lane_governance_snapshot(entries: Vec<LaneGovernanceSnapshot>) {
    *lock_operator_status_slot(lane_governance_slot(), "lane governance snapshot") = entries;
}
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
            let manifest_ready = manifest_required && status.governance_rules.is_some();
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
mod tests {
    #[test]
    fn lane_rbc_reset_clears_surviving_adapter_diagnostics() {
        let _guard = super::rbc_status_test_guard();
        super::lock_operator_status_slot(super::lane_activity_slot(), "lane activity test").push(
            super::LaneActivitySnapshot {
                lane_id: 7,
                ..super::LaneActivitySnapshot::default()
            },
        );
        super::lock_operator_status_slot(
            super::dataspace_activity_slot(),
            "dataspace activity test",
        )
        .push(super::DataspaceActivitySnapshot {
            lane_id: 7,
            dataspace_id: 9,
            tx_served: 1,
        });
        super::lock_operator_status_slot(
            super::pipeline_execution_slot(),
            "pipeline execution test",
        )
        .rbc_chunks_total = 3;
        super::reset_rbc_backlog_stats_for_tests();
        assert!(
            super::lock_operator_status_slot(super::lane_activity_slot(), "lane activity test")
                .is_empty()
        );
        assert!(
            super::lock_operator_status_slot(
                super::dataspace_activity_slot(),
                "dataspace activity test",
            )
            .is_empty()
        );
        assert_eq!(
            super::lock_operator_status_slot(
                super::pipeline_execution_slot(),
                "pipeline execution test",
            )
            .rbc_chunks_total,
            0
        );
    }
}
include!("status/test_guards.rs");
