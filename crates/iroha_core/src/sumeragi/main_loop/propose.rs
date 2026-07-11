//! Proposal assembly and pacemaker-driven propose path.

use super::proposals::block_payload_bytes;
use super::*;
use crate::smartcontracts::isi::triggers::set::SetReadOnly;
use crate::smartcontracts::isi::triggers::specialized::LoadedActionTrait;
use crate::state::WorldReadOnly;
#[cfg(test)]
use crate::state::{StateBlock, StateReadOnly};
use core::num::{NonZeroU64, NonZeroUsize};
use iroha_data_model::block::{
    BlockExecutionContextBundle, CertifiedMergeLedgerReference,
    consensus::{LaneBlockProposalPayloadHintV1, SumeragiLanePayloadOwnership},
};
use iroha_data_model::consensus::{
    CommitStakeSnapshot as ModelCommitStakeSnapshot,
    CommitStakeSnapshotEntry as ModelCommitStakeSnapshotEntry, PreviousRosterEvidence,
    VALIDATOR_SET_HASH_VERSION_V1, ValidatorSetCheckpoint, default_chain_order_hash,
};
use iroha_data_model::events::EventFilter;
use iroha_data_model::prelude::Repeats;
use mv::storage::StorageReadOnly;
use std::collections::BTreeSet;

const PROPOSAL_STALE_WINDOW_TX_QUANTUM: usize = 128;
const PROPOSAL_STALE_WINDOW_PREP_TX_QUANTUM: usize = 32;
const PROPOSAL_STALE_WINDOW_FULL_BATCH_PREP_GRACE: usize = 2;
const PROPOSAL_STALE_WINDOW_MAX_MULTIPLIER: u32 = 8;

const fn should_defer_ordinary_proposal_for_merge(
    has_queue_work: bool,
    certified_merge_ready: bool,
    preparation_grace_active: bool,
) -> bool {
    has_queue_work && !certified_merge_ready && preparation_grace_active
}

#[derive(Debug, Default)]
struct FinalLanePayloadPlan {
    ownerships: Vec<SumeragiLanePayloadOwnership>,
    lane_block_proposal_artifacts: Vec<crate::sumeragi::consensus::LaneBlockProposalV1>,
    lane_block_prepare_vote_plans: Vec<super::lane_scheduler::LaneBlockVotePlan>,
}

#[derive(Default)]
struct ProposalDaStage {
    commitments: Option<DaCommitmentBundle>,
    pins: Option<DaPinIntentBundle>,
}

/// Panic-safe ownership for transaction guards selected during proposal assembly.
///
/// A guard's ordinary `Drop` removes its accepted transaction. Proposal assembly has many
/// consensus and sidecar invariants that may panic in debug/test builds, so every local batch
/// carries enough recovery context to return itself atomically while unwinding. Normal paths
/// explicitly empty the wrapper through atomic return or actor quarantine.
pub(super) struct ProposalTransactionGuards {
    guards: Vec<crate::queue::TransactionGuard>,
    queue: Arc<Queue>,
    state: Arc<State>,
}

impl ProposalTransactionGuards {
    pub(super) fn new(queue: Arc<Queue>, state: Arc<State>) -> Self {
        Self {
            guards: Vec::new(),
            queue,
            state,
        }
    }

    fn from_vec(
        guards: Vec<crate::queue::TransactionGuard>,
        queue: Arc<Queue>,
        state: Arc<State>,
    ) -> Self {
        Self {
            guards,
            queue,
            state,
        }
    }

    fn take_all(&mut self) -> Vec<crate::queue::TransactionGuard> {
        std::mem::take(&mut self.guards)
    }
}

impl std::ops::Deref for ProposalTransactionGuards {
    type Target = Vec<crate::queue::TransactionGuard>;

    fn deref(&self) -> &Self::Target {
        &self.guards
    }
}

impl std::ops::DerefMut for ProposalTransactionGuards {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.guards
    }
}

impl Drop for ProposalTransactionGuards {
    fn drop(&mut self) {
        if self.guards.is_empty() {
            return;
        }
        let guard_count = self.guards.len();
        match self
            .queue
            .return_transaction_guards(&mut self.guards, self.state.as_ref())
        {
            Ok(report) => {
                warn!(
                    guard_count,
                    ?report,
                    "returned proposal transaction guards after unexpected scope exit"
                );
            }
            Err(err) => {
                error!(
                    ?err,
                    guard_count = self.guards.len(),
                    "leaking proposal transaction guards after unwind recovery invariant failure"
                );
                let guards = std::mem::take(&mut self.guards);
                std::mem::forget(guards);
            }
        }
    }
}

#[cfg(test)]
std::thread_local! {
    static FAIL_PROPOSAL_PUBLICATION_TAIL_ONCE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static PROPOSAL_INNER_REBUILD_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PROPOSAL_GUARD_RETURN_ADMISSION_FLOOD: std::cell::RefCell<Option<Vec<AcceptedTransaction<'static>>>> = const { std::cell::RefCell::new(None) };
    static PROPOSAL_GUARD_RETURN_ADMISSION_FLOOD_REPORT: std::cell::Cell<ProposalGuardReturnAdmissionFloodReport> = const { std::cell::Cell::new(ProposalGuardReturnAdmissionFloodReport::EMPTY) };
}

#[cfg(test)]
pub(super) struct ProposalPublicationTailFailpointGuard;

#[cfg(test)]
impl Drop for ProposalPublicationTailFailpointGuard {
    fn drop(&mut self) {
        FAIL_PROPOSAL_PUBLICATION_TAIL_ONCE.set(false);
    }
}

#[cfg(test)]
pub(super) fn fail_proposal_publication_tail_once() -> ProposalPublicationTailFailpointGuard {
    FAIL_PROPOSAL_PUBLICATION_TAIL_ONCE.with(|failpoint| {
        assert!(
            !failpoint.replace(true),
            "publication failpoint already armed"
        );
    });
    ProposalPublicationTailFailpointGuard
}

#[cfg(test)]
fn take_proposal_publication_tail_failpoint() -> bool {
    FAIL_PROPOSAL_PUBLICATION_TAIL_ONCE.with(|failpoint| failpoint.replace(false))
}

#[cfg(test)]
pub(super) fn reset_proposal_inner_rebuild_count() {
    PROPOSAL_INNER_REBUILD_COUNT.set(0);
}

#[cfg(test)]
pub(super) fn proposal_inner_rebuild_count() -> usize {
    PROPOSAL_INNER_REBUILD_COUNT.get()
}

#[cfg(test)]
fn record_proposal_inner_rebuild() {
    PROPOSAL_INNER_REBUILD_COUNT.with(|count| count.set(count.get().saturating_add(1)));
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ProposalGuardReturnAdmissionFloodReport {
    pub(super) attempted: usize,
    pub(super) admitted: usize,
    pub(super) full: usize,
}

#[cfg(test)]
impl ProposalGuardReturnAdmissionFloodReport {
    const EMPTY: Self = Self {
        attempted: 0,
        admitted: 0,
        full: 0,
    };
}

#[cfg(test)]
pub(super) struct ProposalGuardReturnAdmissionFloodGuard;

#[cfg(test)]
impl Drop for ProposalGuardReturnAdmissionFloodGuard {
    fn drop(&mut self) {
        PROPOSAL_GUARD_RETURN_ADMISSION_FLOOD.with(|flood| {
            flood.borrow_mut().take();
        });
        PROPOSAL_GUARD_RETURN_ADMISSION_FLOOD_REPORT
            .with(|report| report.set(ProposalGuardReturnAdmissionFloodReport::EMPTY));
    }
}

#[cfg(test)]
pub(super) fn flood_proposal_guard_return_admission_once(
    transactions: Vec<AcceptedTransaction<'static>>,
) -> ProposalGuardReturnAdmissionFloodGuard {
    PROPOSAL_GUARD_RETURN_ADMISSION_FLOOD.with(|flood| {
        assert!(
            flood.borrow_mut().replace(transactions).is_none(),
            "proposal guard-return admission flood already armed"
        );
    });
    PROPOSAL_GUARD_RETURN_ADMISSION_FLOOD_REPORT
        .with(|report| report.set(ProposalGuardReturnAdmissionFloodReport::EMPTY));
    ProposalGuardReturnAdmissionFloodGuard
}

#[cfg(test)]
pub(super) fn proposal_guard_return_admission_flood_report()
-> ProposalGuardReturnAdmissionFloodReport {
    PROPOSAL_GUARD_RETURN_ADMISSION_FLOOD_REPORT.get()
}

pub(super) const fn should_seed_frontier_backup_transport(
    da_enabled: bool,
    inline_frontier_block_created_transport: bool,
    inline_block_created_backup: bool,
) -> bool {
    da_enabled && inline_frontier_block_created_transport && inline_block_created_backup
}

pub(super) fn resolve_prev_block_for_proposal(
    proposal_height: u64,
    highest_qc: &crate::sumeragi::consensus::QcHeaderRef,
    kura: &Kura,
    pending_blocks: &BTreeMap<HashOf<BlockHeader>, PendingBlock>,
) -> Option<Arc<SignedBlock>> {
    let mut prev_block = proposal_height.checked_sub(1).and_then(|prev| {
        let prev_height_usize = if let Ok(value) = usize::try_from(prev) {
            value
        } else {
            warn!(
                height = proposal_height,
                "previous height exceeds usize; skipping cached block lookup"
            );
            return None;
        };
        NonZeroUsize::new(prev_height_usize).and_then(|nz| kura.get_block(nz))
    });
    if prev_block.is_none() && proposal_height > 1 {
        if let Some(pending_parent) = pending_blocks
            .get(&highest_qc.subject_block_hash)
            .filter(|pending| pending.height + 1 == proposal_height)
        {
            prev_block = Some(pending_parent.block.clone().into());
        }
    }
    prev_block
}

fn precommit_qc_for_view_change(
    highest_qc: Option<crate::sumeragi::consensus::QcHeaderRef>,
    committed_qc: Option<crate::sumeragi::consensus::QcHeaderRef>,
) -> Option<crate::sumeragi::consensus::QcHeaderRef> {
    let highest_precommit =
        highest_qc.filter(|qc| qc.phase == crate::sumeragi::consensus::Phase::Commit);
    match (highest_precommit, committed_qc) {
        (Some(highest), Some(committed)) => {
            if (highest.height, highest.view) >= (committed.height, committed.view) {
                Some(highest)
            } else {
                Some(committed)
            }
        }
        (Some(highest), None) => Some(highest),
        (None, Some(committed)) => Some(committed),
        (None, None) => None,
    }
}

fn queued_committed_frontier_fallback_allowed(
    resilience_enabled: bool,
    tracked_view: u64,
    pending_queue_len: usize,
    active_pending: usize,
    tracked_height: u64,
    committed_height: u64,
    precommit_qc_matches_committed_frontier: bool,
    partial_new_view_blocks_fallback: bool,
    blocked_by_ingress: bool,
    frontier_dependency_clear: bool,
) -> bool {
    resilience_enabled
        && tracked_view > 0
        && pending_queue_len > 0
        && active_pending == 0
        && tracked_height == committed_height.saturating_add(1)
        && precommit_qc_matches_committed_frontier
        && !partial_new_view_blocks_fallback
        && !blocked_by_ingress
        && frontier_dependency_clear
}

#[cfg(test)]
mod queued_committed_frontier_fallback_allowed_tests {
    use super::queued_committed_frontier_fallback_allowed;

    #[derive(Clone, Copy)]
    struct Case {
        resilience_enabled: bool,
        tracked_view: u64,
        pending_queue_len: usize,
        active_pending: usize,
        tracked_height: u64,
        committed_height: u64,
        precommit_qc_matches_committed_frontier: bool,
        partial_new_view_blocks_fallback: bool,
        blocked_by_ingress: bool,
        frontier_dependency_clear: bool,
    }

    impl Case {
        fn pk2_stuck_frontier() -> Self {
            Self {
                resilience_enabled: true,
                tracked_view: 648,
                pending_queue_len: 1,
                active_pending: 0,
                tracked_height: 2667,
                committed_height: 2666,
                precommit_qc_matches_committed_frontier: true,
                partial_new_view_blocks_fallback: false,
                blocked_by_ingress: false,
                frontier_dependency_clear: true,
            }
        }

        fn allowed(self) -> bool {
            queued_committed_frontier_fallback_allowed(
                self.resilience_enabled,
                self.tracked_view,
                self.pending_queue_len,
                self.active_pending,
                self.tracked_height,
                self.committed_height,
                self.precommit_qc_matches_committed_frontier,
                self.partial_new_view_blocks_fallback,
                self.blocked_by_ingress,
                self.frontier_dependency_clear,
            )
        }
    }

    #[test]
    fn allows_queued_high_view_frontier_with_committed_qc() {
        assert!(Case::pk2_stuck_frontier().allowed());
    }

    #[test]
    fn rejects_unsafe_or_non_frontier_states() {
        let reject_cases = [
            Case {
                resilience_enabled: false,
                ..Case::pk2_stuck_frontier()
            },
            Case {
                tracked_view: 0,
                ..Case::pk2_stuck_frontier()
            },
            Case {
                pending_queue_len: 0,
                ..Case::pk2_stuck_frontier()
            },
            Case {
                active_pending: 1,
                ..Case::pk2_stuck_frontier()
            },
            Case {
                tracked_height: 2666,
                ..Case::pk2_stuck_frontier()
            },
            Case {
                tracked_height: 2668,
                ..Case::pk2_stuck_frontier()
            },
            Case {
                precommit_qc_matches_committed_frontier: false,
                ..Case::pk2_stuck_frontier()
            },
            Case {
                partial_new_view_blocks_fallback: true,
                ..Case::pk2_stuck_frontier()
            },
            Case {
                blocked_by_ingress: true,
                ..Case::pk2_stuck_frontier()
            },
            Case {
                frontier_dependency_clear: false,
                ..Case::pk2_stuck_frontier()
            },
        ];

        for case in reject_cases {
            assert!(!case.allowed());
        }
    }
}

fn model_stake_snapshot(
    snapshot: crate::sumeragi::stake_snapshot::CommitStakeSnapshot,
) -> ModelCommitStakeSnapshot {
    ModelCommitStakeSnapshot {
        validator_set_hash: snapshot.validator_set_hash,
        entries: snapshot
            .entries
            .into_iter()
            .map(|entry| ModelCommitStakeSnapshotEntry {
                peer_id: entry.peer_id,
                stake: entry.stake,
            })
            .collect(),
    }
}

fn previous_roster_evidence_for_parent(
    state: &State,
    kura: &Kura,
    fallback_consensus_mode: ConsensusMode,
    parent_block: &SignedBlock,
) -> Option<PreviousRosterEvidence> {
    let parent_height = parent_block.header().height().get();
    let parent_hash = parent_block.hash();
    let metadata = crate::block_sync::message::roster_metadata_from_state(
        state,
        kura,
        parent_height,
        parent_hash,
        fallback_consensus_mode,
    )?;
    let checkpoint = metadata.validator_checkpoint.or_else(|| {
        metadata.commit_qc.as_ref().map(|qc| {
            ValidatorSetCheckpoint::new_with_chain_order(
                qc.height,
                qc.view,
                qc.subject_block_hash,
                qc.chain_order_hash,
                qc.rechain_seq,
                qc.parent_state_root,
                qc.post_state_root,
                qc.validator_set.clone(),
                qc.aggregate.signers_bitmap.clone(),
                qc.aggregate.bls_aggregate_signature.clone(),
                qc.validator_set_hash_version,
                None,
            )
        })
    })?;
    Some(PreviousRosterEvidence {
        height: parent_height,
        block_hash: parent_hash,
        validator_checkpoint: checkpoint,
        stake_snapshot: metadata.stake_snapshot.map(model_stake_snapshot),
    })
}

fn previous_roster_evidence_for_hash_only_parent(
    state: &State,
    consensus_mode: ConsensusMode,
    parent_height: u64,
    parent_hash: HashOf<BlockHeader>,
    roster: &[PeerId],
) -> Option<PreviousRosterEvidence> {
    if roster.is_empty() {
        return None;
    }
    let checkpoint = ValidatorSetCheckpoint::new_with_chain_order(
        parent_height,
        0,
        parent_hash,
        default_chain_order_hash(),
        0,
        Hash::prehashed([0; Hash::LENGTH]),
        Hash::prehashed([0; Hash::LENGTH]),
        roster.to_vec(),
        Vec::new(),
        Vec::new(),
        VALIDATOR_SET_HASH_VERSION_V1,
        None,
    );
    let stake_snapshot = match consensus_mode {
        ConsensusMode::Permissioned => None,
        ConsensusMode::Npos => {
            let world = state.world_view();
            let nexus = state.nexus_snapshot();
            let active_lane_ids = nexus
                .enabled
                .then(|| crate::state::nexus_active_lane_ids(&nexus));
            CommitStakeSnapshot::from_roster_with_active_lanes(
                &world,
                roster,
                active_lane_ids.as_ref(),
            )
            .map(model_stake_snapshot)
        }
    };
    Some(PreviousRosterEvidence {
        height: parent_height,
        block_hash: parent_hash,
        validator_checkpoint: checkpoint,
        stake_snapshot,
    })
}

fn known_lane_block_tips_for_proposal(
    state: &State,
    proposal_height: u64,
) -> Vec<super::lane_scheduler::LaneBlockTip> {
    let nexus = state.nexus_snapshot();
    let mut tips = state
        .lane_block_artifact_tips_snapshot_cached()
        .into_iter()
        .map(
            |(
                lane_id,
                dataspace_id,
                lane_incarnation,
                latest_lane_block_height,
                latest_lane_block_descriptor_hash,
            )| {
                super::lane_scheduler::LaneBlockTip {
                    lane_id,
                    dataspace_id,
                    lane_incarnation,
                    latest_lane_block_height,
                    latest_lane_block_descriptor_hash,
                }
            },
        )
        .collect::<Vec<_>>();
    tips.extend(
        state
            .lane_relay_snapshot()
            .into_iter()
            .filter(|relay| {
                let relay_proposal_height = relay.block_header.height().get();
                relay.is_merge_admissible()
                    && relay.lane_block_descriptor_hash.is_some()
                    && state.da_lane_visible_after_reset(relay_proposal_height, relay.lane_id)
                    && crate::state::consensus_lane_dataspace_at_height(
                        relay.lane_id,
                        &nexus,
                        proposal_height,
                    ) == Some(relay.dataspace_id)
                    && state.lane_incarnation_at_height(relay.lane_id, relay_proposal_height)
                        == Some(relay.lane_incarnation)
                    && state.lane_incarnation_at_height(relay.lane_id, proposal_height)
                        == Some(relay.lane_incarnation)
            })
            .map(|relay| super::lane_scheduler::LaneBlockTip {
                lane_id: relay.lane_id,
                dataspace_id: relay.dataspace_id,
                lane_incarnation: relay.lane_incarnation,
                latest_lane_block_height: relay.block_height,
                latest_lane_block_descriptor_hash: relay_tip_descriptor_hash_for_proposal(&relay),
            }),
    );
    tips.extend(
        state
            .certified_lane_block_tips_snapshot_cached()
            .into_iter()
            .map(
                |(
                    lane_id,
                    dataspace_id,
                    lane_incarnation,
                    latest_lane_block_height,
                    descriptor_hash,
                )| {
                    super::lane_scheduler::LaneBlockTip {
                        lane_id,
                        dataspace_id,
                        lane_incarnation,
                        latest_lane_block_height,
                        latest_lane_block_descriptor_hash: descriptor_hash,
                    }
                },
            ),
    );
    tips
}

fn relay_tip_descriptor_hash_for_proposal(
    relay: &iroha_data_model::nexus::LaneRelayEnvelope,
) -> Option<Hash> {
    if relay.is_merge_admissible() {
        relay.lane_block_descriptor_hash
    } else {
        None
    }
}

fn next_cached_slot_timeout_streak(
    previous: Option<CachedSlotTimeoutTrigger>,
    height: u64,
    view: u64,
) -> u8 {
    previous
        .filter(|last| last.height == height && view > last.view)
        .map_or(0, |last| last.streak.saturating_add(1))
}

fn cached_slot_timeout_hysteresis_remaining(
    mode: ConsensusMode,
    quorum_timeout: Duration,
    previous: Option<CachedSlotTimeoutTrigger>,
    height: u64,
    view: u64,
    now: Instant,
) -> Option<Duration> {
    if !matches!(mode, ConsensusMode::Npos) || quorum_timeout == Duration::ZERO {
        return None;
    }
    let last = previous?;
    if last.height != height || view <= last.view {
        return None;
    }
    let streak = next_cached_slot_timeout_streak(previous, height, view)
        .max(1)
        .min(3);
    let hysteresis = super::saturating_mul_duration(quorum_timeout, u32::from(streak) + 1);
    let elapsed = now.saturating_duration_since(last.at);
    (elapsed < hysteresis).then(|| hysteresis.saturating_sub(elapsed))
}

pub(super) fn cached_slot_effective_quorum_timeout(
    quorum_timeout: Duration,
    rebroadcast_cooldown: Duration,
    precommit_votes_at_view: usize,
    quorum: usize,
    missing_local_data: bool,
    consensus_queue_backlog: bool,
    rbc_session_incomplete: bool,
) -> Duration {
    let near_commit_quorum = precommit_votes_at_view > 0
        && precommit_votes_at_view < quorum
        && precommit_votes_at_view.saturating_add(1) >= quorum;
    let near_quorum_fast_timeout_allowed = near_commit_quorum
        && missing_local_data
        && !consensus_queue_backlog
        && !rbc_session_incomplete;
    if near_quorum_fast_timeout_allowed {
        super::reschedule::near_quorum_payload_timeout(rebroadcast_cooldown).min(quorum_timeout)
    } else {
        quorum_timeout
    }
}

#[cfg(test)]
fn trim_batch_for_size_cap<T, U>(
    tx_batch: &mut Vec<T>,
    routing_batch: &mut Vec<U>,
    sizes: &mut Vec<usize>,
    removed: &mut Vec<(T, U)>,
    mut excess_bytes: usize,
) -> usize {
    debug_assert_eq!(tx_batch.len(), routing_batch.len());
    debug_assert_eq!(tx_batch.len(), sizes.len());
    let mut removed_count = 0usize;
    while excess_bytes > 0 && tx_batch.len() > 1 {
        let tx = match tx_batch.pop() {
            Some(tx) => tx,
            None => break,
        };
        let routing = match routing_batch.pop() {
            Some(routing) => routing,
            None => break,
        };
        let size = sizes.pop().unwrap_or(1).max(1);
        excess_bytes = excess_bytes.saturating_sub(size);
        removed.push((tx, routing));
        removed_count = removed_count.saturating_add(1);
    }
    removed_count
}

fn trim_batch_for_size_cap_with_plans<T, U, V>(
    tx_batch: &mut Vec<T>,
    routing_batch: &mut Vec<U>,
    routing_plan_batch: &mut Vec<V>,
    sizes: &mut Vec<usize>,
    removed: &mut Vec<(T, V)>,
    mut excess_bytes: usize,
) -> usize {
    debug_assert_eq!(tx_batch.len(), routing_batch.len());
    debug_assert_eq!(tx_batch.len(), routing_plan_batch.len());
    debug_assert_eq!(tx_batch.len(), sizes.len());
    let mut removed_count = 0usize;
    while excess_bytes > 0 && tx_batch.len() > 1 {
        let tx = match tx_batch.pop() {
            Some(tx) => tx,
            None => break,
        };
        let _routing = match routing_batch.pop() {
            Some(routing) => routing,
            None => break,
        };
        let routing_plan = match routing_plan_batch.pop() {
            Some(routing_plan) => routing_plan,
            None => break,
        };
        let size = sizes.pop().unwrap_or(1).max(1);
        excess_bytes = excess_bytes.saturating_sub(size);
        removed.push((tx, routing_plan));
        removed_count = removed_count.saturating_add(1);
    }
    removed_count
}

fn defer_batch_lanes_with_plans<T, V>(
    tx_batch: &mut Vec<T>,
    routing_batch: &mut Vec<RoutingDecision>,
    routing_plan_batch: &mut Vec<V>,
    sizes: &mut Vec<usize>,
    deferred_lanes: &BTreeSet<LaneId>,
    removed: &mut Vec<(T, V)>,
) -> usize {
    debug_assert_eq!(tx_batch.len(), routing_batch.len());
    debug_assert_eq!(tx_batch.len(), routing_plan_batch.len());
    debug_assert_eq!(tx_batch.len(), sizes.len());
    if deferred_lanes.is_empty() || tx_batch.is_empty() {
        return 0;
    }

    let txs = std::mem::take(tx_batch);
    let routes = std::mem::take(routing_batch);
    let plans = std::mem::take(routing_plan_batch);
    let encoded_sizes = std::mem::take(sizes);
    let mut removed_count = 0usize;

    tx_batch.reserve(txs.len());
    routing_batch.reserve(routes.len());
    routing_plan_batch.reserve(plans.len());
    sizes.reserve(encoded_sizes.len());
    for (((tx, route), plan), size) in txs
        .into_iter()
        .zip(routes.into_iter())
        .zip(plans.into_iter())
        .zip(encoded_sizes.into_iter())
    {
        if deferred_lanes.contains(&route.lane_id) {
            removed.push((tx, plan));
            removed_count = removed_count.saturating_add(1);
        } else {
            tx_batch.push(tx);
            routing_batch.push(route);
            routing_plan_batch.push(plan);
            sizes.push(size);
        }
    }

    removed_count
}

fn reorder_vec_by_indices<T>(vec: &mut Vec<T>, order: &[usize]) {
    assert_eq!(
        vec.len(),
        order.len(),
        "reorder index set must match vector length"
    );
    if vec.len() <= 1 {
        return;
    }

    let mut slots = std::mem::take(vec)
        .into_iter()
        .map(Some)
        .collect::<Vec<_>>();
    vec.reserve(slots.len());
    for &idx in order {
        let value = slots
            .get_mut(idx)
            .and_then(Option::take)
            .expect("reorder index set must be a valid permutation");
        vec.push(value);
    }
}

#[cfg(test)]
fn canonicalize_parallel_batch_by_key<T, U, K, F>(
    tx_batch: &mut Vec<T>,
    routing_batch: &mut Vec<U>,
    sizes: &mut Vec<usize>,
    key: F,
) where
    K: Ord,
    F: Fn(&T) -> K,
{
    assert_eq!(
        tx_batch.len(),
        routing_batch.len(),
        "routing decisions must align with transactions"
    );
    assert_eq!(
        tx_batch.len(),
        sizes.len(),
        "transaction sizes must align with transactions"
    );
    if tx_batch.len() <= 1 {
        return;
    }

    let mut entries: Vec<_> = std::mem::take(tx_batch)
        .into_iter()
        .zip(std::mem::take(routing_batch))
        .zip(std::mem::take(sizes))
        .enumerate()
        .map(|(idx, ((tx, routing), size))| (key(&tx), idx, tx, routing, size))
        .collect();

    entries.sort_unstable_by(|lhs, rhs| lhs.0.cmp(&rhs.0).then_with(|| lhs.1.cmp(&rhs.1)));

    tx_batch.reserve(entries.len());
    routing_batch.reserve(entries.len());
    sizes.reserve(entries.len());
    for (_, _, tx, routing, size) in entries {
        tx_batch.push(tx);
        routing_batch.push(routing);
        sizes.push(size);
    }
}

#[cfg(test)]
fn canonicalize_proposal_batch(
    tx_batch: &mut Vec<AcceptedTransaction<'static>>,
    routing_batch: &mut Vec<RoutingDecision>,
    sizes: &mut Vec<usize>,
) {
    canonicalize_parallel_batch_by_key(
        tx_batch,
        routing_batch,
        sizes,
        crate::tx::AcceptedTransaction::hash_as_entrypoint,
    );
}

fn canonicalize_proposal_batch_with_plans(
    tx_batch: &mut Vec<AcceptedTransaction<'static>>,
    routing_batch: &mut Vec<RoutingDecision>,
    routing_plan_batch: &mut Vec<crate::queue::RoutingPlan>,
    sizes: &mut Vec<usize>,
) {
    assert_eq!(
        tx_batch.len(),
        routing_batch.len(),
        "routing decisions must align with transactions"
    );
    assert_eq!(
        tx_batch.len(),
        routing_plan_batch.len(),
        "routing plans must align with transactions"
    );
    assert_eq!(
        tx_batch.len(),
        sizes.len(),
        "transaction sizes must align with transactions"
    );
    if tx_batch.len() <= 1 {
        return;
    }

    let mut entries: Vec<_> = std::mem::take(tx_batch)
        .into_iter()
        .zip(std::mem::take(routing_batch))
        .zip(std::mem::take(routing_plan_batch))
        .zip(std::mem::take(sizes))
        .enumerate()
        .map(|(idx, (((tx, routing), routing_plan), size))| {
            (
                tx.hash_as_entrypoint(),
                idx,
                tx,
                routing,
                routing_plan,
                size,
            )
        })
        .collect();

    entries.sort_unstable_by(|lhs, rhs| lhs.0.cmp(&rhs.0).then_with(|| lhs.1.cmp(&rhs.1)));

    tx_batch.reserve(entries.len());
    routing_batch.reserve(entries.len());
    routing_plan_batch.reserve(entries.len());
    sizes.reserve(entries.len());
    for (_, _, tx, routing, routing_plan, size) in entries {
        tx_batch.push(tx);
        routing_batch.push(routing);
        routing_plan_batch.push(routing_plan);
        sizes.push(size);
    }
}

fn refresh_proposal_routing_from_state(
    tx_batch: &[AcceptedTransaction<'static>],
    routing_batch: &mut Vec<RoutingDecision>,
    routing_plan_batch: &mut Vec<crate::queue::RoutingPlan>,
    state_view: &crate::state::StateView<'_>,
    ledger_time_ms: u64,
    proposal_height: u64,
) -> Result<bool> {
    if tx_batch.len() != routing_batch.len() || tx_batch.len() != routing_plan_batch.len() {
        return Err(eyre!(
            "proposal routing vector length mismatch: txs={} routes={} plans={}",
            tx_batch.len(),
            routing_batch.len(),
            routing_plan_batch.len()
        ));
    }
    if tx_batch.is_empty() {
        return Ok(false);
    }

    let nexus = &state_view.nexus;
    let mut refreshed_routing = Vec::with_capacity(tx_batch.len());
    let mut refreshed_plans = Vec::with_capacity(tx_batch.len());
    for (idx, tx) in tx_batch.iter().enumerate() {
        let refreshed_plan =
            crate::queue::evaluate_policy_plan_with_nexus_and_world_at_block_height(
                nexus,
                tx,
                state_view.world(),
                ledger_time_ms,
                proposal_height,
            )
            .map_err(|err| {
                eyre!(
                    "proposal routing cannot be resolved from committed state at index {idx}: {err}"
                )
            })?;
        refreshed_routing.push(refreshed_plan.coordinator_route());
        refreshed_plans.push(refreshed_plan);
    }

    let changed = routing_batch.as_slice() != refreshed_routing.as_slice()
        || routing_plan_batch.as_slice() != refreshed_plans.as_slice();
    if changed {
        *routing_batch = refreshed_routing;
        *routing_plan_batch = refreshed_plans;
    }
    Ok(changed)
}

fn collect_sccp_messages_for_active_proposal_routes<F>(
    tx_batch: &[AcceptedTransaction<'static>],
    routing_batch: &[RoutingDecision],
    nexus: &iroha_config::parameters::actual::Nexus,
    proposal_height: u64,
    is_already_recorded: F,
) -> Result<Vec<crate::bridge::RecordedSccpMessage>>
where
    F: Fn(&iroha_data_model::bridge::SccpOutboundMessageKey) -> bool,
{
    if tx_batch.len() != routing_batch.len() {
        return Err(eyre!(
            "proposal SCCP routing vector length mismatch: txs={} routes={}",
            tx_batch.len(),
            routing_batch.len()
        ));
    }
    if !nexus.enabled {
        return Ok(Vec::new());
    }
    Ok(
        crate::bridge::collect_new_sccp_messages_from_accepted_transactions_where(
            tx_batch,
            |tx_index| proposal_route_is_active(routing_batch[tx_index], nexus, proposal_height),
            is_already_recorded,
        ),
    )
}

fn proposal_route_is_active(
    route: RoutingDecision,
    nexus: &iroha_config::parameters::actual::Nexus,
    proposal_height: u64,
) -> bool {
    crate::state::nexus_active_lane_dataspace_at_height(route.lane_id, nexus, proposal_height)
        .is_some_and(|dataspace_id| dataspace_id == route.dataspace_id)
}

#[cfg(test)]
fn collect_sccp_messages_for_committable_proposal_routes(
    tx_batch: &[AcceptedTransaction<'static>],
    routing_batch: &[RoutingDecision],
    nexus: &iroha_config::parameters::actual::Nexus,
    proposal_height: u64,
    state: &State,
    header: BlockHeader,
) -> Result<Vec<crate::bridge::RecordedSccpMessage>> {
    if tx_batch.len() != routing_batch.len() {
        return Err(eyre!(
            "proposal SCCP routing vector length mismatch: txs={} routes={}",
            tx_batch.len(),
            routing_batch.len()
        ));
    }
    if !nexus.enabled {
        return Ok(Vec::new());
    }

    let mut candidate_messages = Vec::with_capacity(tx_batch.len());
    let mut has_candidate = false;
    for (tx_index, tx) in tx_batch.iter().enumerate() {
        if !proposal_route_is_active(routing_batch[tx_index], nexus, proposal_height) {
            candidate_messages.push(None);
            continue;
        }
        let messages = crate::bridge::collect_sccp_messages_from_accepted_transaction(tx_index, tx);
        if messages.is_empty() {
            candidate_messages.push(None);
        } else {
            has_candidate = true;
            candidate_messages.push(Some(messages));
        }
    }
    if !has_candidate {
        return Ok(Vec::new());
    }

    let mut preflight_block = state.block(header);
    let accounts = StateReadOnly::accounts_snapshot(&preflight_block);
    let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::with_capacity(
        preflight_block.pipeline.cache_size,
    );
    Ok(collect_sccp_messages_after_ordered_preflight(
        tx_batch,
        routing_batch,
        candidate_messages,
        |_, transaction, route| {
            let Some(tx) = signed_transaction_for_proposal_preflight(transaction) else {
                return Ok(false);
            };
            preflight_proposal_transaction(
                &mut preflight_block,
                tx,
                transaction.hash_as_entrypoint(),
                transaction.encoded_len(),
                route,
                &accounts,
                &mut ivm_cache,
            )?;
            Ok(true)
        },
    ))
}

#[cfg(test)]
fn collect_sccp_messages_after_ordered_preflight<F>(
    tx_batch: &[AcceptedTransaction<'static>],
    routing_batch: &[RoutingDecision],
    candidate_messages: Vec<Option<Vec<crate::bridge::RecordedSccpMessage>>>,
    mut preflight_transaction: F,
) -> Vec<crate::bridge::RecordedSccpMessage>
where
    F: FnMut(
        usize,
        &AcceptedTransaction<'static>,
        RoutingDecision,
    ) -> std::result::Result<bool, String>,
{
    let mut committable_messages = Vec::new();
    for (tx_index, ((transaction, route), maybe_messages)) in tx_batch
        .iter()
        .zip(routing_batch.iter().copied())
        .zip(candidate_messages.into_iter())
        .enumerate()
    {
        let candidate_count = maybe_messages.as_ref().map_or(0, Vec::len);
        match preflight_transaction(tx_index, transaction, route) {
            Ok(true) => {
                if let Some(messages) = maybe_messages {
                    committable_messages.extend(messages);
                }
            }
            Ok(false) => {}
            Err(reason) => {
                if candidate_count > 0 {
                    let entrypoint_hash = transaction.hash_as_entrypoint();
                    debug!(
                        tx_index,
                        tx = %entrypoint_hash,
                        lane = route.lane_id.as_u32(),
                        dataspace = route.dataspace_id.as_u64(),
                        reason,
                        "excluding SCCP records from proposal commitment root after preflight rejection"
                    );
                }
            }
        }
    }
    committable_messages
}

#[cfg(test)]
fn signed_transaction_for_proposal_preflight<'a>(
    transaction: &'a AcceptedTransaction<'_>,
) -> Option<&'a SignedTransaction> {
    match transaction.entrypoint() {
        iroha_data_model::transaction::TransactionEntrypoint::External(tx) => Some(tx),
        iroha_data_model::transaction::TransactionEntrypoint::SealedReveal(reveal) => {
            Some(reveal.signed_transaction())
        }
        iroha_data_model::transaction::TransactionEntrypoint::SealedCommitment(_)
        | iroha_data_model::transaction::TransactionEntrypoint::PrivateKaigi(_)
        | iroha_data_model::transaction::TransactionEntrypoint::Time(_) => None,
    }
}

#[cfg(test)]
#[cfg(test)]
fn preflight_proposal_transaction(
    state_block: &mut StateBlock<'_>,
    tx: &SignedTransaction,
    entrypoint_hash: HashOf<iroha_data_model::transaction::TransactionEntrypoint>,
    encoded_len: usize,
    routing: RoutingDecision,
    accounts: &Arc<Vec<AccountId>>,
    ivm_cache: &mut crate::smartcontracts::ivm::cache::IvmCache,
) -> std::result::Result<(), String> {
    let streaming_meta =
        crate::pipeline::overlay::resolve_streaming_metadata(&*state_block, tx.authority());
    let prepared =
        crate::pipeline::overlay::build_prepared_overlay_for_transaction_with_accounts_zk(
            tx,
            Arc::clone(accounts),
            &*state_block,
            state_block.zk.halo2.enabled || state_block.zk.stark.enabled,
            &state_block._curr_block,
            streaming_meta,
            ivm_cache,
            state_block.pipeline.dynamic_prepass,
        )
        .map_err(|err| err.to_string())?;
    let overlay = prepared.overlay;

    let max_instrs = state_block.pipeline.overlay_max_instructions;
    if max_instrs > 0 && overlay.instruction_count() > max_instrs {
        return Err(format!(
            "overlay exceeds max instructions: {} > {max_instrs}",
            overlay.instruction_count()
        ));
    }
    let max_bytes = state_block.pipeline.overlay_max_bytes;
    let byte_size = overlay.byte_size() as u64;
    if max_bytes > 0 && byte_size > max_bytes {
        return Err(format!(
            "overlay exceeds max bytes: {byte_size} > {max_bytes}"
        ));
    }

    let authority = tx.authority().clone();
    let chunk_size = state_block.pipeline.overlay_chunk_instructions.max(1);
    let mut state_tx = state_block.transaction();
    state_tx.current_lane_id = Some(routing.lane_id);
    state_tx.current_dataspace_id = Some(routing.dataspace_id);
    state_tx.world.current_dataspace_id = Some(routing.dataspace_id);
    state_tx.tx_call_hash = Some(iroha_crypto::Hash::from(entrypoint_hash));
    state_tx.current_tx_hash = Some(tx.hash());
    let admission = StateBlock::validate_stateful_admission(tx, &mut state_tx, Some(routing))
        .map_err(|reason| format!("{reason:?}"))?;
    let executor = state_tx.world.executor.clone();
    crate::executor::configure_executor_fuel_budget(&executor, &mut state_tx, tx.metadata())
        .map_err(|err| format!("{err:?}"))?;
    overlay
        .apply_with_chunk(&mut state_tx, &authority, chunk_size)
        .map_err(|err| format!("{err:?}"))?;
    crate::executor::charge_fees_for_applied_overlay_with_encoded_len(
        &mut state_tx,
        &authority,
        tx,
        &overlay,
        encoded_len,
    )
    .map_err(|err| format!("{err:?}"))?;
    state_tx
        .execute_data_triggers_dfs(&authority)
        .map_err(|err| format!("{err:?}"))?;
    if let Some(seq) = admission.sequence_to_commit {
        state_tx.world.tx_sequences.insert(admission.authority, seq);
    }
    state_tx.apply();
    Ok(())
}

fn proposal_sccp_commitment_root_after_execution(
    state: &crate::state::State,
    builder: &crate::block::BlockBuilder<crate::block::Chained>,
    sccp_root: Option<[u8; 32]>,
    private_key: &iroha_crypto::PrivateKey,
    local_validator_index: u32,
) -> Result<Option<[u8; 32]>> {
    let probe_block: SignedBlock = builder
        .clone()
        .with_sccp_commitment_root(sccp_root)
        .try_sign_with_index(private_key, u64::from(local_validator_index))
        .map_err(|err| eyre!("failed to sign SCCP root probe block: {err}"))?
        .unpack(|_| {})
        .into();
    let mut state_block = if let Some(reference) = probe_block
        .execution_context()
        .and_then(|bundle| bundle.merge_entry.as_ref())
    {
        state
            .block_with_certified_merge_reference(probe_block.header(), reference)
            .map_err(|err| eyre!("failed to stage certified merge entry for SCCP probe: {err}"))?
    } else {
        state.block(probe_block.header())
    };
    crate::block::ValidBlock::sccp_commitment_root_after_execution(probe_block, &mut state_block)
        .map_err(|err| eyre!("failed to derive SCCP commitment root after execution: {err}"))
}

const PROPOSAL_TIME_PADDING: std::time::Duration = std::time::Duration::from_millis(1);

#[derive(Debug, Clone, Copy)]
pub(super) struct InternalProposalWork {
    pub(super) time_triggers: bool,
    pub(super) da_commitments: bool,
    pub(super) da_receipts: bool,
    pub(super) da_pin_intents: bool,
    pub(super) certified_merge: bool,
    pub(super) autoscale_maintenance: bool,
}

impl InternalProposalWork {
    pub(super) const fn has_work(self) -> bool {
        self.time_triggers
            || self.da_commitments
            || self.da_receipts
            || self.da_pin_intents
            || self.certified_merge
            || self.autoscale_maintenance
    }

    const fn has_non_autoscale_work(self) -> bool {
        self.time_triggers
            || self.da_commitments
            || self.da_receipts
            || self.da_pin_intents
            || self.certified_merge
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ProposalBackpressure {
    pub(super) queue_state: BackpressureState,
    pub(super) active_pending: bool,
    pub(super) rbc_backlog: bool,
    pub(super) relay_backpressure: bool,
    pub(super) consensus_queue_backpressure: bool,
}

impl ProposalBackpressure {
    pub(super) fn should_defer(self) -> bool {
        self.queue_state.is_saturated()
            || self.active_pending
            || self.rbc_backlog
            || self.relay_backpressure
            || self.consensus_queue_backpressure
    }

    pub(super) fn only_pacing_backpressure(self) -> bool {
        (self.queue_state.is_saturated() || self.consensus_queue_backpressure)
            && !(self.active_pending || self.rbc_backlog || self.relay_backpressure)
    }
}

fn consensus_queue_backpressure(
    depths: status::WorkerQueueDepthSnapshot,
    block_payload_cap: usize,
    rbc_chunk_cap: usize,
) -> bool {
    let block_payload_cap = u64::try_from(block_payload_cap.max(1)).unwrap_or(u64::MAX);
    let rbc_chunk_cap = u64::try_from(rbc_chunk_cap.max(1)).unwrap_or(u64::MAX);
    depths.block_payload_rx >= block_payload_cap || depths.rbc_chunk_rx >= rbc_chunk_cap
}

fn age_starved_queue_allows_stale_pending_override(
    saturated_by_age: bool,
    saturated_by_count: bool,
    ingress_starvation_override: bool,
    recent_pending_consensus_progress: bool,
) -> bool {
    saturated_by_age
        && !saturated_by_count
        && ingress_starvation_override
        && !recent_pending_consensus_progress
}

fn da_payload_budget(
    rbc_chunk_max_bytes: usize,
    rbc_pending_max_bytes: usize,
    rbc_pending_max_chunks: usize,
    block_max_payload_bytes: Option<NonZeroUsize>,
) -> usize {
    let rbc_budget = rbc_chunk_max_bytes
        .max(1)
        .saturating_mul(usize::try_from(RBC_MAX_TOTAL_CHUNKS).expect("fits in usize"));
    let pending_budget = rbc_pending_max_bytes.min(
        rbc_chunk_max_bytes
            .max(1)
            .saturating_mul(rbc_pending_max_chunks.max(1)),
    );
    let payload_budget = block_max_payload_bytes.map_or(usize::MAX, NonZeroUsize::get);
    payload_budget.min(rbc_budget).min(pending_budget)
}

impl Actor {
    fn native_amx_receipt_for_plan(
        &mut self,
        _tx: &AcceptedTransaction<'_>,
        plan: &crate::queue::RoutingPlan,
        _block_height: u64,
    ) -> Result<Option<NativeAmxReceipt>, &'static str> {
        let crate::queue::RoutingPlan::NativeAmx(_) = plan else {
            return Ok(None);
        };
        Err("native AMX receipt generation is owned exclusively by Sumeragi v2")
    }

    pub(super) fn native_amx_receipts_for_batch(
        &mut self,
        tx_batch: &[AcceptedTransaction<'static>],
        routing_plan_batch: &[crate::queue::RoutingPlan],
        block_height: u64,
    ) -> Result<Vec<Option<NativeAmxReceipt>>, &'static str> {
        let mut receipts = Vec::with_capacity(tx_batch.len());
        for (tx, plan) in tx_batch.iter().zip(routing_plan_batch) {
            receipts.push(self.native_amx_receipt_for_plan(tx, plan, block_height)?);
        }
        Ok(receipts)
    }

    pub(super) fn frontier_missing_qc_liveness_active(&self, height: u64, view: u64) -> bool {
        self.subsystems
            .propose
            .proposal_liveness
            .is_some_and(|slot| {
                slot.height == height
                    && slot.view == view
                    && matches!(
                        slot.state,
                        ProposalLivenessState::AwaitingProposalAfterMissingQc
                            | ProposalLivenessState::RecoveryAcquireDependencies
                    )
            })
    }

    fn missing_qc_liveness_allows_frontier_self_proposal(
        &self,
        height: u64,
        view: u64,
        committed_height: u64,
        _pending_queue_len: usize,
        precommit_qc: Option<crate::sumeragi::consensus::QcHeaderRef>,
    ) -> Option<crate::sumeragi::consensus::QcHeaderRef> {
        if height != committed_height.saturating_add(1) || view == 0 {
            return None;
        }
        if !self.frontier_missing_qc_liveness_active(height, view) {
            return None;
        }
        let qc = precommit_qc?;
        if height == qc.height.saturating_add(1) {
            Some(qc)
        } else {
            None
        }
    }

    pub(super) fn vote_locked_frontier_recovery_ready(
        &self,
        height: u64,
        view: u64,
        now: Instant,
    ) -> bool {
        let Some(lock) = self.same_height_vote_lock_blocking_candidate(height, view, None) else {
            return false;
        };
        if self.same_height_vote_lock_superseded_by_committed_frontier_new_view(height, view, &lock)
        {
            return false;
        }
        if self.same_height_block_has_observed_qc(lock.block_hash, height, lock.view) {
            return false;
        }
        let min_stale_age = self
            .quorum_timeout(self.runtime_da_enabled())
            .max(self.frontier_slot_lag_window())
            .max(Duration::from_millis(1));
        let hard_stale_age = min_stale_age.saturating_mul(3);
        self.stale_same_height_recovery_age(height, lock.view, now)
            .is_some_and(|age| age >= hard_stale_age)
            || self.same_height_vote_recovery_escalation_view_gap_exhausted(
                lock.view,
                view,
                lock.total_validators,
            )
    }

    pub(super) fn frontier_recovery_ingress_override_active(
        &self,
        height: u64,
        view: u64,
        now: Instant,
        ingress_grace: Duration,
    ) -> bool {
        if !self.config.resilience.enabled
            || height != self.committed_height_snapshot().saturating_add(1)
            || view == 0
            || !self.frontier_proposal_or_view_starved_past_ingress_grace(
                height,
                now,
                ingress_grace,
            )
        {
            return false;
        }
        self.vote_locked_frontier_recovery_ready(height, view, now)
            || self.frontier_missing_qc_liveness_active(height, view)
            || self.exact_frontier_body_repair_active_at_height(height)
    }

    fn warn_resilience_frontier_proposal_deferred(
        &mut self,
        height: u64,
        view: u64,
        reason: &'static str,
        highest_qc: crate::sumeragi::consensus::QcHeaderRef,
        pending_queue_len: usize,
        now: Instant,
    ) {
        if !self.config.resilience.enabled
            || height != self.committed_height_snapshot().saturating_add(1)
            || view == 0
            || highest_qc.height.saturating_add(1) != height
        {
            return;
        }
        if let Some(suppressed_since_last) = self.proposal_defer_warning_log.allow(
            ProposalDeferWarningKind::FrontierRecoveryProposalDeferred,
            height,
            view,
            highest_qc.subject_block_hash,
            now,
            Duration::from_secs(2),
        ) {
            warn!(
                height,
                view,
                reason,
                queue_len = pending_queue_len,
                highest_height = highest_qc.height,
                highest_view = highest_qc.view,
                highest_hash = %highest_qc.subject_block_hash,
                suppressed_since_last,
                "resilience frontier recovery proposal deferred"
            );
        }
    }

    pub(super) fn internal_proposal_work(
        &mut self,
        proposal_height: u64,
        prev_block: Option<&SignedBlock>,
        certified_merge: bool,
    ) -> InternalProposalWork {
        let time_triggers = self.proposal_time_triggers_due(proposal_height, prev_block);
        let autoscale_maintenance = self.autoscale_maintenance_due();
        if !self.runtime_da_enabled() {
            return InternalProposalWork {
                time_triggers,
                da_commitments: false,
                da_receipts: false,
                da_pin_intents: false,
                certified_merge,
                autoscale_maintenance,
            };
        }
        let (da_commitments, da_receipts, da_pin_intents) = self.proposal_da_spool_work();
        InternalProposalWork {
            time_triggers,
            da_commitments,
            da_receipts,
            da_pin_intents,
            certified_merge,
            autoscale_maintenance,
        }
    }

    fn autoscale_maintenance_due(&self) -> bool {
        let nexus = self.state.nexus_snapshot();
        if !nexus.enabled || !nexus.autoscale.enabled {
            return false;
        }
        let min_lane = nexus.autoscale.min_lanes.get();
        let max_lane = nexus.autoscale.max_lanes.get();
        let policy = &nexus.routing_policy;
        nexus.lane_catalog.lanes().iter().any(|lane| {
            lane.id != policy.default_lane
                && lane.dataspace_id == policy.default_dataspace
                && (min_lane..max_lane).contains(&lane.id.as_u32())
                && lane.is_autoscale_managed_elastic()
        })
    }

    fn pending_certified_merge_entry_for_proposal(
        &self,
        proposal_height: u64,
        proposal_view: u64,
        prev_block: Option<&SignedBlock>,
    ) -> Option<iroha_data_model::merge::MergeLedgerEntry> {
        let expected_epoch = self
            .state
            .merge_ledger()
            .latest()
            .map_or(1, |latest| latest.epoch_id.saturating_add(1));
        let round_builder = BlockBuilder::new(Vec::new()).chain(proposal_view, prev_block);
        let round_header = round_builder.carrier_context_header();
        if round_header.height().get() != proposal_height {
            return None;
        }
        match self
            .state
            .select_pending_certified_merge_entry_for_round(&round_header, expected_epoch)
        {
            Ok(Some((_, entry, _))) => Some(entry),
            Ok(None) => None,
            Err(err) => {
                warn!(
                    ?err,
                    height = proposal_height,
                    view = proposal_view,
                    "pending certified merge sidecars could not be inspected; ordinary proposal liveness remains enabled"
                );
                None
            }
        }
    }

    pub(super) fn merge_preparation_grace_active(
        &self,
        proposal_height: u64,
        proposal_view: u64,
        now: Instant,
    ) -> bool {
        let Some(preparation) = self.subsystems.merge.committee.preparation else {
            return false;
        };
        if preparation.round.height != proposal_height
            || preparation.round.view != proposal_view
            || self.state.latest_block_hash_fast() != Some(preparation.round.parent_hash)
        {
            return false;
        }
        let grace = self
            .effective_timing
            .get()
            .commit_quorum_timeout
            .max(Duration::from_millis(1));
        now.saturating_duration_since(preparation.started_at) < grace
    }

    fn proposal_time_triggers_due(
        &self,
        proposal_height: u64,
        prev_block: Option<&SignedBlock>,
    ) -> bool {
        let now = iroha_primitives::time::TimeSource::new_system().get_unix_time();
        let prev_block_time = prev_block.map_or(std::time::Duration::ZERO, |block| {
            block.header().creation_time()
        });
        let creation_time = std::cmp::max(
            now,
            std::cmp::max(
                prev_block_time.saturating_add(PROPOSAL_TIME_PADDING),
                PROPOSAL_TIME_PADDING,
            ),
        );
        let since = NonZeroUsize::new(self.state.committed_height())
            .and_then(|height| self.state.block_by_height(height))
            .map_or(creation_time, |block| block.header().creation_time());
        let (since, length) = creation_time
            .checked_sub(since)
            .map_or((creation_time, std::time::Duration::ZERO), |length| {
                (since, length)
            });
        let event = iroha_data_model::events::time::TimeEvent {
            interval: iroha_data_model::events::time::TimeInterval::new(since, length),
        };
        let key_height = "__registered_block_height"
            .parse::<iroha_data_model::name::Name>()
            .ok();
        let world = self.state.world_view();
        world
            .triggers()
            .time_triggers()
            .iter()
            .filter(|(_, action)| {
                crate::smartcontracts::isi::triggers::trigger_is_enabled(action.metadata())
            })
            .any(|(_, action)| {
                let mut count = action.filter.count_matches(&event);
                if let Repeats::Exactly(repeats) = action.repeats {
                    count = std::cmp::min(repeats, count);
                }
                if count == 0 {
                    return false;
                }
                let registered_height = key_height
                    .as_ref()
                    .and_then(|key| action.metadata().get(key))
                    .and_then(|json| json.try_into_any_norito::<u64>().ok());
                registered_height.is_some_and(|height| height != proposal_height)
            })
    }

    fn proposal_da_spool_work(&mut self) -> (bool, bool, bool) {
        let (commitment_bundle, commitment_load_failed) = {
            let da_rbc = &mut self.subsystems.da_rbc;
            match da_rbc.spool_cache.load_commitment_bundle(&da_rbc.spool_dir) {
                Ok((value, cache_outcome)) => {
                    #[cfg(feature = "telemetry")]
                    self.telemetry.note_da_spool_cache(
                        crate::telemetry::DaSpoolCacheKind::Commitments,
                        cache_outcome.as_telemetry(),
                    );
                    #[cfg(not(feature = "telemetry"))]
                    let _ = cache_outcome;
                    (value, false)
                }
                Err(err) => {
                    warn!(
                        ?err,
                        spool = %da_rbc.spool_dir.display(),
                        "failed to load DA commitments during proposal preflight; scheduling assembly to surface the error"
                    );
                    (None, true)
                }
            }
        };
        let eligible_commitments = commitment_bundle.map_or_else(Vec::new, |bundle| {
            bundle
                .commitments
                .into_iter()
                .filter(|record| {
                    !self
                        .state
                        .da_commitments_contains_record_identity_cached(record)
                })
                .collect::<Vec<_>>()
        });

        let nexus = self.state.nexus_snapshot();
        let (receipt_plan, receipt_load_or_plan_failed) = if nexus.enabled {
            let cursor_snapshot = self.state.da_receipt_cursor_snapshot_cached();
            let da_rbc = &mut self.subsystems.da_rbc;
            match da_rbc.spool_cache.load_receipt_entries(&da_rbc.spool_dir) {
                Ok((entries, cache_outcome)) => {
                    #[cfg(feature = "telemetry")]
                    self.telemetry.note_da_spool_cache(
                        crate::telemetry::DaSpoolCacheKind::Receipts,
                        cache_outcome.as_telemetry(),
                    );
                    #[cfg(not(feature = "telemetry"))]
                    let _ = cache_outcome;
                    match crate::da::receipts::plan_committable_receipts(
                        &nexus.lane_config,
                        &cursor_snapshot,
                        entries,
                    ) {
                        Ok(plan) => (plan, false),
                        Err(err) => {
                            warn!(
                                ?err,
                                spool = %da_rbc.spool_dir.display(),
                                "failed to plan DA receipts during proposal preflight; scheduling assembly to surface the error"
                            );
                            (Vec::new(), true)
                        }
                    }
                }
                Err(err) => {
                    warn!(
                        ?err,
                        spool = %da_rbc.spool_dir.display(),
                        "failed to load DA receipts during proposal preflight; scheduling assembly to surface the error"
                    );
                    (Vec::new(), true)
                }
            }
        } else {
            (Vec::new(), false)
        };
        let receipt_has = receipt_load_or_plan_failed || !receipt_plan.is_empty();
        let commitment_has = if commitment_load_failed {
            true
        } else if !nexus.enabled {
            !eligible_commitments.is_empty()
        } else if receipt_load_or_plan_failed || receipt_plan.is_empty() {
            false
        } else {
            match crate::da::receipts::align_commitments_for_receipts(
                &receipt_plan,
                &eligible_commitments,
            ) {
                Ok(aligned) => !aligned.is_empty(),
                Err(err) => {
                    warn!(
                        ?err,
                        spool = %self.subsystems.da_rbc.spool_dir.display(),
                        "failed to align DA commitments and receipts during proposal preflight; scheduling assembly to surface the error"
                    );
                    false
                }
            }
        };

        let da_rbc = &mut self.subsystems.da_rbc;
        let pin_intent_has = match da_rbc.spool_cache.load_pin_bundle(&da_rbc.spool_dir) {
            Ok((value, cache_outcome)) => {
                #[cfg(feature = "telemetry")]
                self.telemetry.note_da_spool_cache(
                    crate::telemetry::DaSpoolCacheKind::PinIntents,
                    cache_outcome.as_telemetry(),
                );
                #[cfg(not(feature = "telemetry"))]
                let _ = cache_outcome;
                value.is_some_and(|bundle| {
                    bundle.intents.iter().any(|intent| {
                        !self
                            .state
                            .da_pin_intents_contains_intent_identity_cached(intent)
                    })
                })
            }
            Err(err) => {
                warn!(
                    ?err,
                    spool = %da_rbc.spool_dir.display(),
                    "failed to load DA pin intents during proposal preflight; scheduling assembly to surface the error"
                );
                true
            }
        };

        (commitment_has, receipt_has, pin_intent_has)
    }

    pub(super) fn max_tx_budget(
        queue_len: usize,
        block_param_limit: u64,
        config_cap: Option<NonZeroUsize>,
    ) -> (usize, NonZeroUsize) {
        let param_limit = usize::try_from(block_param_limit).unwrap_or_else(|_| {
            warn!(
                block_param_limit,
                "block max transactions exceeds usize; capping to usize::MAX"
            );
            usize::MAX
        });
        let max_tx_target = config_cap
            .map(NonZeroUsize::get)
            .map_or(param_limit, |cfg| cfg.min(param_limit));
        let max_in_block = NonZeroUsize::new(queue_len.min(max_tx_target).max(1))
            .expect("non-zero by construction");
        (max_tx_target, max_in_block)
    }

    pub(super) fn max_tx_budget_for_commit_time(
        queue_len: usize,
        block_param_limit: u64,
        config_cap: Option<NonZeroUsize>,
        fast_finality_config_cap: Option<NonZeroUsize>,
        commit_time_ms: u64,
        effective_commit_time_ms: u64,
    ) -> (usize, NonZeroUsize, bool) {
        let (configured_target, _) = Self::max_tx_budget(queue_len, block_param_limit, config_cap);
        let fast_cap = fast_finality_config_cap
            .filter(|_| Self::fast_finality_cap_applies(commit_time_ms, effective_commit_time_ms))
            .map(NonZeroUsize::get);
        let max_tx_target = fast_cap.map_or(configured_target, |cap| configured_target.min(cap));
        let max_in_block = NonZeroUsize::new(queue_len.min(max_tx_target).max(1))
            .expect("non-zero by construction");
        let fast_tx_capped = max_tx_target < configured_target;
        (max_tx_target, max_in_block, fast_tx_capped)
    }

    pub(super) fn fast_finality_cap_applies(
        commit_time_ms: u64,
        effective_commit_time_ms: u64,
    ) -> bool {
        let threshold = iroha_config::parameters::defaults::sumeragi::FAST_FINALITY_COMMIT_TIME_MS;
        commit_time_ms <= threshold || effective_commit_time_ms <= threshold
    }

    pub(super) fn cap_gas_limit_for_fast_commit(
        gas_limit_per_block: Option<NonZeroU64>,
        commit_time_ms: u64,
        effective_commit_time_ms: u64,
        fast_gas_limit_per_block: Option<NonZeroU64>,
    ) -> Option<NonZeroU64> {
        let Some(base_limit) = gas_limit_per_block else {
            return None;
        };
        let Some(cap) = fast_gas_limit_per_block else {
            return Some(base_limit);
        };
        if !Self::fast_finality_cap_applies(commit_time_ms, effective_commit_time_ms) {
            return Some(base_limit);
        }
        let capped = base_limit.get().min(cap.get());
        Some(NonZeroU64::new(capped).expect("non-zero by construction"))
    }

    pub(super) fn proposal_assembly_stale_window(base: Duration, tx_count: usize) -> Duration {
        let batches = tx_count.saturating_add(PROPOSAL_STALE_WINDOW_TX_QUANTUM - 1)
            / PROPOSAL_STALE_WINDOW_TX_QUANTUM;
        let full_batch_grace = usize::from(tx_count >= PROPOSAL_STALE_WINDOW_TX_QUANTUM);
        let consensus_multiplier = batches.saturating_add(full_batch_grace);
        let prep_multiplier = if tx_count >= PROPOSAL_STALE_WINDOW_TX_QUANTUM {
            tx_count.saturating_add(PROPOSAL_STALE_WINDOW_PREP_TX_QUANTUM - 1)
                / PROPOSAL_STALE_WINDOW_PREP_TX_QUANTUM
                + PROPOSAL_STALE_WINDOW_FULL_BATCH_PREP_GRACE
        } else {
            1
        };
        let multiplier = consensus_multiplier
            .max(prep_multiplier)
            .max(1)
            .min(PROPOSAL_STALE_WINDOW_MAX_MULTIPLIER as usize);
        let multiplier = u32::try_from(multiplier).expect("proposal stale multiplier fits u32");
        base.saturating_mul(multiplier)
    }

    pub(super) fn is_ivm_heavy_transaction(
        tx: &AcceptedTransaction<'_>,
        replay_ivm_proved: bool,
    ) -> bool {
        fn is_heavy_executable(
            executable: &iroha_data_model::transaction::Executable,
            replay_ivm_proved: bool,
        ) -> bool {
            matches!(
                executable,
                iroha_data_model::transaction::Executable::ContractCall(_)
                    | iroha_data_model::transaction::Executable::Ivm(_)
            ) || (replay_ivm_proved
                && matches!(
                    executable,
                    iroha_data_model::transaction::Executable::IvmProved(_)
                ))
        }

        match tx.entrypoint() {
            iroha_data_model::transaction::TransactionEntrypoint::External(signed) => {
                is_heavy_executable(signed.instructions(), replay_ivm_proved)
            }
            iroha_data_model::transaction::TransactionEntrypoint::SealedReveal(reveal) => {
                is_heavy_executable(
                    reveal.signed_transaction().instructions(),
                    replay_ivm_proved,
                )
            }
            iroha_data_model::transaction::TransactionEntrypoint::SealedCommitment(_)
            | iroha_data_model::transaction::TransactionEntrypoint::PrivateKaigi(_)
            | iroha_data_model::transaction::TransactionEntrypoint::Time(_) => false,
        }
    }

    pub(super) fn pull_transactions_for_proposal(
        &self,
        state: &State,
        max_in_block: NonZeroUsize,
        scan_budget: usize,
        gas_limit_per_block: Option<NonZeroU64>,
        max_ivm_transactions: Option<NonZeroUsize>,
        replay_ivm_proved: bool,
        tx_guards: &mut Vec<crate::queue::TransactionGuard>,
        height: u64,
        view: u64,
    ) -> ProposalTransactionGuards {
        #[derive(Clone, Copy)]
        enum GuardDecision {
            Accept { exceeds_gas_limit: bool },
            Defer,
        }

        let mut lane_consumption: BTreeMap<LaneId, u64> = BTreeMap::new();
        let mut deferred_accumulator =
            ProposalTransactionGuards::new(Arc::clone(&self.queue), Arc::clone(&self.state));
        let mut fetched_total = 0usize;
        let mut gas_used_in_block = 0u64;
        let gas_limit_per_block = gas_limit_per_block.map(NonZeroU64::get);
        let max_ivm_transactions = max_ivm_transactions.map(NonZeroUsize::get);
        let mut ivm_transactions_included = 0usize;
        let mut ivm_transactions_deferred = 0usize;
        let scan_budget = scan_budget.max(1);
        let committed_nexus = state.nexus_snapshot();
        if committed_nexus.enabled && self.queue.lane_reservation_journal_installed() {
            // In Nexus mode each active lane owns FIFO selection through its
            // crash-safe reservation journal. Letting the global proposer pop
            // the same queue concurrently would reintroduce cross-node double
            // execution; global blocks carry certified merge batches instead.
            debug!(
                height,
                view,
                "skipping global FIFO selection while independent lane producers own the queue"
            );
            return deferred_accumulator;
        }
        let (lane_domain_consensus_mode, lane_domain_mode_tag, _) =
            self.consensus_context_for_height(height);
        let mut planned_lane_payload_ownerships = Vec::new();
        let mut lane_domain_validators =
            self.roster_for_live_vote_with_mode(height, lane_domain_consensus_mode);
        if lane_domain_validators.is_empty() {
            lane_domain_validators = self.effective_commit_topology();
        }
        let use_shared_lane_domain_committee = !committed_nexus.enabled
            || !super::lane_scheduler::proposal_lookahead_enabled(&committed_nexus, height);
        if self.queue.reconfigure_nexus_with_state_if_needed(
            &committed_nexus,
            state,
            self.queue.lane_compliance_engine(),
        ) {
            info!(
                height,
                view, "proposal queue routing refreshed from committed Nexus state"
            );
        }
        let known_lane_block_tips = self.known_lane_block_tips_for_proposal(height);
        let blocked_lane_ids = self.unapplied_lane_block_lanes_for_proposal(state, height);
        let lane_reset_heights = BTreeMap::new();

        loop {
            let remaining_budget = scan_budget.saturating_sub(fetched_total);
            if remaining_budget == 0 {
                debug!(
                    height,
                    view, scan_budget, fetched_total, "proposal queue scan budget reached"
                );
                break;
            }
            let remaining_slots = max_in_block.get().saturating_sub(tx_guards.len());
            if remaining_slots == 0 {
                break;
            }
            if let Some(limit) = gas_limit_per_block {
                let remaining_gas = limit.saturating_sub(gas_used_in_block);
                if remaining_gas == 0 {
                    debug!(
                        height,
                        view,
                        gas_limit = limit,
                        gas_used = gas_used_in_block,
                        "proposal gas budget reached"
                    );
                    break;
                }
            }
            let fetch_cap = super::lane_scheduler::proposal_fetch_cap(
                &committed_nexus,
                height,
                remaining_budget,
                remaining_slots,
            );
            let fetch_cap = NonZeroUsize::new(fetch_cap).expect("non-zero by construction");
            let mut fetched =
                ProposalTransactionGuards::new(Arc::clone(&self.queue), Arc::clone(&self.state));
            self.queue
                .get_transactions_for_block_with_state(state, fetch_cap, &mut fetched);
            if fetched.is_empty() {
                break;
            }
            fetched_total = fetched_total.saturating_add(fetched.len());
            let mut deferred = ProposalTransactionGuards::from_vec(
                self.queue
                    .enforce_lane_teu_limits_with_consumption_and_routing_plans(
                        &mut fetched,
                        &mut lane_consumption,
                    ),
                Arc::clone(&self.queue),
                Arc::clone(&self.state),
            );
            if !deferred.is_empty() {
                deferred_accumulator.extend(deferred.take_all());
            }

            let fetched_routing: Vec<RoutingDecision> = fetched
                .iter()
                .map(crate::queue::TransactionGuard::routing)
                .collect();
            let fetched_hashes: Vec<_> = fetched
                .iter()
                .map(|guard| Hash::from(guard.hash_as_entrypoint()))
                .collect();
            let candidates: Vec<_> = fetched
                .iter()
                .map(|guard| super::lane_scheduler::ProposalAdmissionCandidate {
                    gas_cost: guard.gas_cost(),
                    is_ivm_heavy: Self::is_ivm_heavy_transaction(
                        guard.as_accepted(),
                        replay_ivm_proved,
                    ),
                })
                .collect();
            let mut schedule = super::lane_scheduler::schedule_proposal_batch(
                &fetched_routing,
                &candidates,
                super::lane_scheduler::ProposalAdmissionContext {
                    accepted_before_batch: tx_guards.len(),
                    accepted_in_batch: 0,
                    max_in_block: max_in_block.get(),
                    gas_limit_per_block,
                    gas_used_in_block,
                    max_ivm_transactions,
                    ivm_transactions_included,
                },
                height,
                view,
            )
            .expect("fetched proposal candidates and routing decisions must align");
            if !blocked_lane_ids.is_empty() {
                schedule = super::lane_scheduler::defer_accepted_proposal_actions_for_lanes(
                    &schedule,
                    &fetched_routing,
                    &candidates,
                    &blocked_lane_ids,
                    super::lane_scheduler::ProposalDeferralReason::LaneConsensus,
                );
            }
            let lane_consensus_deferral = !use_shared_lane_domain_committee;
            let mut defer_accepted_due_to_lane_consensus = false;
            for planning_attempt in 0..2 {
                let lane_domain_committees =
                    super::lane_scheduler::plan_lane_consensus_committees_with_authority(
                        &fetched_routing,
                        &schedule,
                        use_shared_lane_domain_committee
                            .then_some(lane_domain_validators.as_slice()),
                        |lane_id, _dataspace_id| {
                            if use_shared_lane_domain_committee {
                                Vec::new()
                            } else {
                                state.authoritative_lane_peer_ids_at_height(lane_id, height)
                            }
                        },
                    );
                let lane_domains = lane_domain_committees.and_then(|committees| {
                    super::lane_scheduler::plan_lane_consensus_domains(
                        &fetched_routing,
                        &schedule,
                        &committees,
                        lane_domain_mode_tag,
                    )
                });
                let domains = match lane_domains {
                    Ok(domains) => domains,
                    Err(error) => {
                        warn!(
                            height,
                            view,
                            ?error,
                            lane_consensus_deferral,
                            "failed to plan lane-local consensus domains for proposal batch"
                        );
                        if lane_consensus_deferral {
                            defer_accepted_due_to_lane_consensus = true;
                        }
                        break;
                    }
                };
                if domains.is_empty() {
                    break;
                }
                let lane_incarnations = domains
                    .iter()
                    .map(|domain| {
                        state
                            .lane_incarnation_at_height(domain.lane_id, height)
                            .filter(|incarnation| {
                                !incarnation.as_ref().iter().all(|byte| *byte == 0)
                            })
                            .map(|incarnation| (domain.lane_id, incarnation))
                    })
                    .collect::<Option<BTreeMap<_, _>>>();
                let Some(lane_incarnations) = lane_incarnations else {
                    warn!(
                        height,
                        view, "failed to plan lane payload: active lane incarnation is missing"
                    );
                    if lane_consensus_deferral {
                        defer_accepted_due_to_lane_consensus = true;
                    }
                    break;
                };

                let lane_payload_plan =
                    match super::lane_scheduler::plan_lane_payload_with_incarnations(
                        &domains,
                        &known_lane_block_tips,
                        &fetched_hashes,
                        height.saturating_sub(1),
                        &lane_reset_heights,
                        &lane_incarnations,
                        height,
                        Self::planned_lane_block_view_for_global_proposal(height, view),
                    ) {
                        Ok(lane_payload_plan) => lane_payload_plan,
                        Err(error) => {
                            warn!(
                                height,
                                view,
                                ?error,
                                lane_consensus_deferral,
                                "failed to plan lane-local payload for proposal batch"
                            );
                            if lane_consensus_deferral {
                                defer_accepted_due_to_lane_consensus = true;
                            }
                            break;
                        }
                    };

                let non_authoritative_lanes = if lane_consensus_deferral {
                    self.lane_payload_lanes_not_authorized_for_local_proposer(&lane_payload_plan)
                } else {
                    BTreeSet::new()
                };
                if !non_authoritative_lanes.is_empty() && planning_attempt == 0 {
                    debug!(
                        height,
                        view,
                        lane_ids = ?non_authoritative_lanes
                            .iter()
                            .map(|lane_id| lane_id.as_u32())
                            .collect::<Vec<_>>(),
                        "deferring lane-routed proposal work outside the local lane committee"
                    );
                    schedule = super::lane_scheduler::defer_accepted_proposal_actions_for_lanes(
                        &schedule,
                        &fetched_routing,
                        &candidates,
                        &non_authoritative_lanes,
                        super::lane_scheduler::ProposalDeferralReason::LaneConsensus,
                    );
                    continue;
                }
                if !non_authoritative_lanes.is_empty() {
                    warn!(
                        height,
                        view,
                        lane_ids = ?non_authoritative_lanes
                            .iter()
                            .map(|lane_id| lane_id.as_u32())
                            .collect::<Vec<_>>(),
                        lane_consensus_deferral,
                        "failed to remove lane-routed proposal work outside the local lane committee"
                    );
                    if lane_consensus_deferral {
                        defer_accepted_due_to_lane_consensus = true;
                    }
                    break;
                }

                planned_lane_payload_ownerships.extend(
                    lane_payload_plan
                        .entries
                        .iter()
                        .map(|entry| Self::lane_payload_ownership_to_wire(entry, height, view)),
                );
                trace!(
                    height,
                    view,
                    lane_domains = domains.len(),
                    lane_ids = ?domains
                        .iter()
                        .map(|domain| domain.lane_id.as_u32())
                        .collect::<Vec<_>>(),
                    dataspace_ids = ?domains
                        .iter()
                        .map(|domain| domain.dataspace_id.as_u64())
                        .collect::<Vec<_>>(),
                    accepted_lane_candidates = domains
                        .iter()
                        .map(|domain| domain.accepted_candidates)
                        .sum::<usize>(),
                    accepted_lane_candidate_indices = ?domains
                        .iter()
                        .map(|domain| domain.accepted_candidate_indices.clone())
                        .collect::<Vec<_>>(),
                    lane_tip_heights = ?lane_payload_plan
                        .entries
                        .iter()
                        .map(|entry| entry.tip.latest_lane_block_height)
                        .collect::<Vec<_>>(),
                    lane_slot_heights = ?lane_payload_plan
                        .entries
                        .iter()
                        .map(|entry| entry.slot.lane_block_height)
                        .collect::<Vec<_>>(),
                    lane_block_subjects = lane_payload_plan.entries.len(),
                    lane_block_subject_hashes = ?lane_payload_plan
                        .entries
                        .iter()
                        .map(|entry| hex::encode(entry.subject.subject_hash.as_ref()))
                        .collect::<Vec<_>>(),
                    lane_payload_ownerships = lane_payload_plan.entries.len(),
                    lane_payload_ownership_hashes = ?lane_payload_plan
                        .entries
                        .iter()
                        .map(|entry| hex::encode(entry.ownership.payload_ownership_hash.as_ref()))
                        .collect::<Vec<_>>(),
                    lane_rbc_instance_hashes = ?lane_payload_plan
                        .entries
                        .iter()
                        .map(|entry| hex::encode(entry.ownership.rbc_instance_hash.as_ref()))
                        .collect::<Vec<_>>(),
                    lane_block_descriptor_hashes = ?lane_payload_plan
                        .entries
                        .iter()
                        .map(|entry| hex::encode(entry.block_descriptor.descriptor_hash.as_ref()))
                        .collect::<Vec<_>>(),
                    validator_count = domains
                        .iter()
                        .map(|domain| domain.validator_set.len())
                        .max()
                        .unwrap_or_default(),
                    first_quorum = domains
                        .first()
                        .map_or(0, |domain| domain.quorum.min_quorum),
                    first_qc_mode_tag = domains
                        .first()
                        .map_or("", |domain| domain.qc_mode_tag.as_str()),
                    "planned lane-local consensus domains for proposal batch"
                );
                break;
            }
            if defer_accepted_due_to_lane_consensus {
                schedule = super::lane_scheduler::defer_accepted_proposal_actions(
                    &schedule,
                    super::lane_scheduler::ProposalDeferralReason::LaneConsensus,
                );
            }
            let mut decisions = vec![None; fetched.len()];
            for action in schedule.actions.iter().copied() {
                let (index, decision) = match action {
                    super::lane_scheduler::ProposalBatchAction::Accept {
                        index,
                        exceeds_gas_limit,
                    } => (index, GuardDecision::Accept { exceeds_gas_limit }),
                    super::lane_scheduler::ProposalBatchAction::Defer { index, reason: _ } => {
                        (index, GuardDecision::Defer)
                    }
                };
                if let Some(slot) = decisions.get_mut(index) {
                    if slot.replace(decision).is_some() {
                        warn!(
                            height,
                            view,
                            index,
                            "duplicate proposal scheduler action; deferring candidate fail-closed"
                        );
                        *slot = Some(GuardDecision::Defer);
                    }
                } else {
                    warn!(
                        height,
                        view,
                        index,
                        candidates = fetched.len(),
                        "out-of-range proposal scheduler action ignored"
                    );
                }
            }
            let mut accepted =
                ProposalTransactionGuards::new(Arc::clone(&self.queue), Arc::clone(&self.state));
            let release_lane_consumption =
                |guard: &crate::queue::TransactionGuard,
                 lane_consumption: &mut BTreeMap<LaneId, u64>| {
                    let lane_id = guard.routing().lane_id;
                    let teu = guard.teu_weight();
                    if let Some(used) = lane_consumption.get_mut(&lane_id) {
                        *used = used.saturating_sub(teu);
                    }
                };

            fetched.reverse();
            for decision in decisions {
                let Some(guard) = fetched.pop() else {
                    break;
                };
                match decision.unwrap_or(GuardDecision::Defer) {
                    GuardDecision::Accept { exceeds_gas_limit } => {
                        if exceeds_gas_limit {
                            debug!(
                                height,
                                view,
                                gas_cost = guard.gas_cost(),
                                gas_limit = gas_limit_per_block,
                                "proposal gas cap exceeded by single tx; admitting to avoid stall"
                            );
                        }
                        accepted.push(guard);
                    }
                    GuardDecision::Defer => {
                        release_lane_consumption(&guard, &mut lane_consumption);
                        deferred_accumulator.push(guard);
                    }
                }
            }
            // A malformed scheduler length must never make an unmentioned guard fall out of
            // scope. Preserve every extra guard fail-closed.
            deferred_accumulator.extend(fetched.take_all());
            gas_used_in_block = gas_used_in_block.saturating_add(schedule.gas_used_delta);
            ivm_transactions_included =
                ivm_transactions_included.saturating_add(schedule.ivm_transactions_included_delta);
            ivm_transactions_deferred =
                ivm_transactions_deferred.saturating_add(schedule.ivm_transactions_deferred);
            tx_guards.extend(accepted.take_all());

            if let Some(limit) = gas_limit_per_block {
                if gas_used_in_block >= limit {
                    break;
                }
            }
        }
        crate::sumeragi::status::set_lane_payload_ownerships(planned_lane_payload_ownerships);

        if ivm_transactions_deferred > 0 {
            debug!(
                height,
                view,
                max_ivm_transactions,
                ivm_transactions_included,
                ivm_transactions_deferred,
                "proposal IVM-heavy transaction budget reached"
            );
        }

        deferred_accumulator
    }

    fn lane_payload_ownership_to_wire(
        entry: &super::lane_scheduler::LanePayloadPlanEntry,
        proposal_height: u64,
        proposal_view: u64,
    ) -> SumeragiLanePayloadOwnership {
        let ownership = &entry.ownership;
        SumeragiLanePayloadOwnership {
            proposal_height,
            proposal_view,
            lane_id: ownership.lane_id,
            dataspace_id: ownership.dataspace_id,
            lane_incarnation: ownership.lane_incarnation,
            lane_block_height: ownership.lane_block_height,
            lane_block_view: ownership.lane_block_view,
            subject_hash: ownership.subject_hash,
            qc_mode_tag: ownership.qc_mode_tag.clone(),
            accepted_candidate_indices: ownership
                .accepted_candidate_indices
                .iter()
                .map(|index| {
                    u64::try_from(*index).expect("validated lane payload candidate index fits u64")
                })
                .collect(),
            accepted_transaction_hashes: ownership.accepted_transaction_hashes.clone(),
            previous_lane_block_height: entry.block_descriptor.previous_lane_block_height,
            previous_lane_block_descriptor_hash: entry
                .block_descriptor
                .previous_lane_block_descriptor_hash,
            lane_block_descriptor_hash: Some(entry.block_descriptor.descriptor_hash),
            lane_block_descriptor_validator_set: entry.block_descriptor.validator_set.clone(),
            lane_block_descriptor_validator_count: entry.block_descriptor.quorum.validator_count,
            lane_block_descriptor_min_quorum: entry.block_descriptor.quorum.min_quorum,
            payload_ownership_hash: ownership.payload_ownership_hash,
            rbc_instance_hash: ownership.rbc_instance_hash,
        }
    }

    fn lane_payload_lanes_not_authorized_for_local_proposer(
        &self,
        lane_payload_plan: &super::lane_scheduler::LanePayloadPlan,
    ) -> BTreeSet<LaneId> {
        let local_peer = self.common_config.peer.id();
        lane_payload_plan
            .entries
            .iter()
            .filter(|entry| {
                !entry
                    .domain
                    .validator_set
                    .iter()
                    .any(|peer| peer == local_peer)
            })
            .map(|entry| entry.domain.lane_id)
            .collect()
    }

    fn planned_lane_block_view_for_global_proposal(
        _proposal_height: u64,
        _proposal_view: u64,
    ) -> u64 {
        // The current standalone lane-block path does not run an independent
        // lane-local view-change protocol. Binding lane-local view to the
        // global Sumeragi view fragments votes when several global leaders
        // retry the same lane height, so newly planned lane blocks use a stable
        // initial lane view.
        0
    }

    fn plan_final_lane_payload(
        &self,
        state: &State,
        routing_batch: &[RoutingDecision],
        candidate_hashes: &[Hash],
        height: u64,
        view: u64,
    ) -> Result<FinalLanePayloadPlan> {
        if routing_batch.is_empty() {
            return Ok(FinalLanePayloadPlan::default());
        }

        let committed_nexus = state.nexus_snapshot();
        let (lane_domain_consensus_mode, lane_domain_mode_tag, _) =
            self.consensus_context_for_height(height);
        let mut lane_domain_validators =
            self.roster_for_live_vote_with_mode(height, lane_domain_consensus_mode);
        if lane_domain_validators.is_empty() {
            lane_domain_validators = self.effective_commit_topology();
        }
        let use_shared_lane_domain_committee = !committed_nexus.enabled
            || !super::lane_scheduler::proposal_lookahead_enabled(&committed_nexus, height);
        let schedule = super::lane_scheduler::ProposalBatchSchedule {
            actions: (0..routing_batch.len())
                .map(|index| super::lane_scheduler::ProposalBatchAction::Accept {
                    index,
                    exceeds_gas_limit: false,
                })
                .collect(),
            ..super::lane_scheduler::ProposalBatchSchedule::default()
        };
        let lane_domain_committees =
            super::lane_scheduler::plan_lane_consensus_committees_with_authority(
                routing_batch,
                &schedule,
                use_shared_lane_domain_committee.then_some(lane_domain_validators.as_slice()),
                |lane_id, _dataspace_id| {
                    if use_shared_lane_domain_committee {
                        Vec::new()
                    } else {
                        state.authoritative_lane_peer_ids_at_height(lane_id, height)
                    }
                },
            )
            .map_err(|error| {
                eyre!(
                    "failed to plan lane-local consensus committees for final proposal batch: {error:?}"
                )
            })?;
        let lane_domains = super::lane_scheduler::plan_lane_consensus_domains(
            routing_batch,
            &schedule,
            &lane_domain_committees,
            lane_domain_mode_tag,
        )
        .map_err(|error| {
            eyre!("failed to plan lane-local consensus domains for final proposal batch: {error:?}")
        })?;
        if lane_domains.is_empty() {
            return Ok(FinalLanePayloadPlan::default());
        }

        let known_lane_block_tips = self.known_lane_block_tips_for_proposal(height);
        let lane_reset_heights = BTreeMap::new();
        let lane_incarnations = lane_domains
            .iter()
            .map(|domain| {
                state
                    .lane_incarnation_at_height(domain.lane_id, height)
                    .filter(|incarnation| !incarnation.as_ref().iter().all(|byte| *byte == 0))
                    .map(|incarnation| (domain.lane_id, incarnation))
                    .ok_or_else(|| {
                        eyre!(
                            "missing active incarnation for lane {} at proposal height {height}",
                            domain.lane_id.as_u32()
                        )
                    })
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        let lane_payload_plan = super::lane_scheduler::plan_lane_payload_with_incarnations(
            &lane_domains,
            &known_lane_block_tips,
            candidate_hashes,
            height.saturating_sub(1),
            &lane_reset_heights,
            &lane_incarnations,
            height,
            Self::planned_lane_block_view_for_global_proposal(height, view),
        )
        .map_err(|error| {
            eyre!("failed to plan lane-local payload for final proposal batch: {error:?}")
        })?;
        let non_authoritative_lanes = if use_shared_lane_domain_committee {
            BTreeSet::new()
        } else {
            self.lane_payload_lanes_not_authorized_for_local_proposer(&lane_payload_plan)
        };
        if !non_authoritative_lanes.is_empty() {
            return Err(eyre!(
                "local proposer is not authorized for lane-local payloads on lanes: {:?}",
                non_authoritative_lanes
                    .iter()
                    .map(|lane_id| lane_id.as_u32())
                    .collect::<Vec<_>>()
            ));
        }

        let ownerships = lane_payload_plan
            .entries
            .iter()
            .map(|entry| Self::lane_payload_ownership_to_wire(entry, height, view))
            .collect();

        Ok(FinalLanePayloadPlan {
            ownerships,
            lane_block_proposal_artifacts: lane_payload_plan.lane_block_proposal_artifacts,
            lane_block_prepare_vote_plans: lane_payload_plan.lane_block_prepare_vote_plans,
        })
    }

    fn final_lane_payload_lanes_not_authorized_for_local_proposer(
        &self,
        state: &State,
        routing_batch: &[RoutingDecision],
        candidate_hashes: &[Hash],
        height: u64,
        view: u64,
    ) -> Result<BTreeSet<LaneId>> {
        if routing_batch.is_empty() {
            return Ok(BTreeSet::new());
        }

        let committed_nexus = state.nexus_snapshot();
        let use_shared_lane_domain_committee = !committed_nexus.enabled
            || !super::lane_scheduler::proposal_lookahead_enabled(&committed_nexus, height);
        if use_shared_lane_domain_committee {
            return Ok(BTreeSet::new());
        }

        let (lane_domain_consensus_mode, lane_domain_mode_tag, _) =
            self.consensus_context_for_height(height);
        let lane_domain_committees =
            super::lane_scheduler::plan_lane_consensus_committees_with_authority(
                routing_batch,
                &super::lane_scheduler::ProposalBatchSchedule {
                    actions: (0..routing_batch.len())
                        .map(|index| super::lane_scheduler::ProposalBatchAction::Accept {
                            index,
                            exceeds_gas_limit: false,
                        })
                        .collect(),
                    ..super::lane_scheduler::ProposalBatchSchedule::default()
                },
                None,
                |lane_id, _dataspace_id| state.authoritative_lane_peer_ids_at_height(lane_id, height),
            )
            .map_err(|error| {
                eyre!(
                    "failed to plan lane-local consensus committees for final proposal batch: {error:?}"
                )
            })?;
        let lane_domains = super::lane_scheduler::plan_lane_consensus_domains(
            routing_batch,
            &super::lane_scheduler::ProposalBatchSchedule {
                actions: (0..routing_batch.len())
                    .map(|index| super::lane_scheduler::ProposalBatchAction::Accept {
                        index,
                        exceeds_gas_limit: false,
                    })
                    .collect(),
                ..super::lane_scheduler::ProposalBatchSchedule::default()
            },
            &lane_domain_committees,
            lane_domain_mode_tag,
        )
        .map_err(|error| {
            eyre!("failed to plan lane-local consensus domains for final proposal batch: {error:?}")
        })?;
        if lane_domains.is_empty() {
            return Ok(BTreeSet::new());
        }

        let _ = lane_domain_consensus_mode;
        let known_lane_block_tips = self.known_lane_block_tips_for_proposal(height);
        let lane_reset_heights = state.da_shard_canonical_reset_heights_snapshot_cached();
        let lane_incarnations = lane_domains
            .iter()
            .map(|domain| {
                state
                    .lane_incarnation_at_height(domain.lane_id, height)
                    .filter(|incarnation| !incarnation.as_ref().iter().all(|byte| *byte == 0))
                    .map(|incarnation| (domain.lane_id, incarnation))
                    .ok_or_else(|| {
                        eyre!(
                            "missing active incarnation for lane {} at proposal height {height}",
                            domain.lane_id.as_u32()
                        )
                    })
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        let lane_payload_plan = super::lane_scheduler::plan_lane_payload_with_incarnations(
            &lane_domains,
            &known_lane_block_tips,
            candidate_hashes,
            height.saturating_sub(1),
            &lane_reset_heights,
            &lane_incarnations,
            height,
            Self::planned_lane_block_view_for_global_proposal(height, view),
        )
        .map_err(|error| {
            eyre!("failed to plan lane-local payload for final proposal batch: {error:?}")
        })?;

        Ok(self.lane_payload_lanes_not_authorized_for_local_proposer(&lane_payload_plan))
    }

    pub(super) fn known_lane_block_tips_for_proposal(
        &self,
        proposal_height: u64,
    ) -> Vec<super::lane_scheduler::LaneBlockTip> {
        let mut tips = known_lane_block_tips_for_proposal(self.state.as_ref(), proposal_height);
        tips.extend(
            self.subsystems
                .committed_lane_blocks
                .lane_block_tips_snapshot_for_admissible_lanes(
                    |lane_id,
                     dataspace_id,
                     lane_incarnation,
                     _lane_block_height,
                     tip_proposal_height| {
                        self.state
                            .da_lane_visible_after_reset(tip_proposal_height, lane_id)
                            && self.lane_block_artifact_targets_active_route(
                                lane_id,
                                dataspace_id,
                                lane_incarnation,
                                tip_proposal_height,
                            )
                            && self.lane_block_artifact_targets_active_route(
                                lane_id,
                                dataspace_id,
                                lane_incarnation,
                                proposal_height,
                            )
                    },
                ),
        );
        tips
    }

    pub(super) fn unapplied_lane_block_lanes_for_proposal(
        &self,
        state: &State,
        proposal_height: u64,
    ) -> BTreeSet<LaneId> {
        let mut blocked_lanes = state
            .unapplied_lane_block_artifact_heights_snapshot_cached()
            .into_iter()
            .chain(
                state
                    .unapplied_certified_lane_block_heights_snapshot_cached()
                    .into_iter(),
            )
            .filter_map(|((lane_id, dataspace_id), _lane_block_height)| {
                let lane_incarnation =
                    state.lane_incarnation_at_height(lane_id, proposal_height)?;
                self.lane_block_artifact_targets_active_route(
                    lane_id,
                    dataspace_id,
                    lane_incarnation,
                    proposal_height,
                )
                .then_some(lane_id)
            })
            .collect::<BTreeSet<_>>();
        blocked_lanes.extend(
            self.subsystems
                .committed_lane_blocks
                .unapplied_lane_ids_for_admissible_lanes_for_state(
                    state,
                    |lane_id, dataspace_id, _lane_block_height, artifact_proposal_height| {
                        state
                            .lane_incarnation_at_height(lane_id, artifact_proposal_height)
                            .is_some_and(|lane_incarnation| {
                                self.lane_block_artifact_targets_active_route(
                                    lane_id,
                                    dataspace_id,
                                    lane_incarnation,
                                    artifact_proposal_height,
                                )
                            })
                    },
                ),
        );
        blocked_lanes.extend(
            self.subsystems
                .lane_blocks
                .inflight_lane_ids_for_admissible_lanes(
                    |lane_id,
                     dataspace_id,
                     lane_incarnation,
                     _lane_block_height,
                     artifact_proposal_height,
                     has_consensus_evidence| {
                        self.lane_block_artifact_targets_active_route(
                            lane_id,
                            dataspace_id,
                            lane_incarnation,
                            artifact_proposal_height,
                        ) && (has_consensus_evidence || artifact_proposal_height == proposal_height)
                    },
                ),
        );
        blocked_lanes
    }

    pub(super) fn defer_batch_lanes_with_unapplied_lane_blocks<T, V>(
        &self,
        proposal_height: u64,
        tx_batch: &mut Vec<T>,
        routing_batch: &mut Vec<RoutingDecision>,
        routing_plan_batch: &mut Vec<V>,
        sizes: &mut Vec<usize>,
        removed: &mut Vec<(T, V)>,
    ) -> (BTreeSet<LaneId>, usize) {
        let blocked_lane_ids =
            self.unapplied_lane_block_lanes_for_proposal(self.state.as_ref(), proposal_height);
        let removed_count = defer_batch_lanes_with_plans(
            tx_batch,
            routing_batch,
            routing_plan_batch,
            sizes,
            &blocked_lane_ids,
            removed,
        );
        (blocked_lane_ids, removed_count)
    }

    pub(super) fn local_lane_block_prepare_vote(
        &self,
        plan: &super::lane_scheduler::LaneBlockVotePlan,
        proposal: &crate::sumeragi::consensus::LaneBlockProposalV1,
    ) -> Option<crate::lane_consensus::LaneBlockVoteV1> {
        if plan.phase != crate::sumeragi::consensus::Phase::Prepare {
            return None;
        }
        if proposal.proposal_hash != plan.proposal_hash
            || !self.lane_block_payload_available_for_vote(proposal, "prepare")
        {
            return None;
        }

        match self.common_config.key_pair.public_key().try_algorithm() {
            Ok(iroha_crypto::Algorithm::BlsNormal) => {}
            Ok(algorithm) => {
                warn!(
                    ?algorithm,
                    "skipping local lane-block prepare vote broadcast with non-BLS consensus key"
                );
                return None;
            }
            Err(err) => {
                warn!(
                    ?err,
                    "skipping local lane-block prepare vote broadcast with unrecognized consensus key"
                );
                return None;
            }
        }

        let local_peer = self.common_config.peer.id();
        let Some(vote) = plan.votes.iter().find(|vote| {
            vote.phase == crate::sumeragi::consensus::Phase::Prepare && &vote.signer == local_peer
        }) else {
            return None;
        };

        if vote.body.phase != crate::sumeragi::consensus::Phase::Prepare
            || vote.proposal_hash != plan.proposal_hash
            || vote.descriptor_hash != plan.descriptor_hash
            || vote.validator_set_hash != plan.validator_set_hash
        {
            warn!(
                lane = vote.lane_id.as_u32(),
                lane_block_height = vote.lane_block_height,
                lane_block_view = vote.lane_block_view,
                "skipping inconsistent local lane-block prepare vote plan"
            );
            return None;
        }

        if vote.signer.public_key() != self.common_config.key_pair.public_key() {
            warn!("skipping local lane-block prepare vote for mismatched local peer key");
            return None;
        }

        let signing_hash = Hash::new(vote.body.signature_preimage());
        if vote.signing_hash != signing_hash {
            warn!(
                lane = vote.lane_id.as_u32(),
                lane_block_height = vote.lane_block_height,
                lane_block_view = vote.lane_block_view,
                "skipping local lane-block prepare vote with stale signing hash"
            );
            return None;
        }

        if !self.lane_block_vote_body_targets_authorized_local_signer(&vote.body, local_peer) {
            warn!(
                lane = vote.lane_id.as_u32(),
                dataspace_id = vote.dataspace_id.as_u64(),
                lane_block_height = vote.lane_block_height,
                lane_block_view = vote.lane_block_view,
                validator_count = vote.body.validator_count,
                min_quorum = vote.body.min_quorum,
                "skipping local lane-block prepare vote for non-authoritative route or committee"
            );
            return None;
        }

        let signature = match Signature::try_new(
            self.common_config.key_pair.private_key(),
            &vote.body.signature_preimage(),
        ) {
            Ok(signature) => signature,
            Err(err) => {
                warn!(
                    ?err,
                    lane = vote.lane_id.as_u32(),
                    lane_block_height = vote.lane_block_height,
                    lane_block_view = vote.lane_block_view,
                    "skipping local lane-block prepare vote after signing failure"
                );
                return None;
            }
        };

        Some(crate::lane_consensus::LaneBlockVoteV1 {
            body: vote.body.clone(),
            payload_availability_vote: None,
            signer: vote.signer.clone(),
            bls_signature: signature.payload().to_vec(),
        })
    }

    pub(super) fn broadcast_lane_block_plan_artifacts(
        &mut self,
        proposals: &[crate::sumeragi::consensus::LaneBlockProposalV1],
        prepare_vote_plans: &[super::lane_scheduler::LaneBlockVotePlan],
        payload_block_hint: Option<crate::sumeragi::consensus::LaneBlockProposalPayloadHintV1>,
    ) -> usize {
        let mut scheduled = 0_usize;

        for proposal_artifact in proposals {
            let proposal = payload_block_hint.clone().map_or_else(
                || proposal_artifact.clone(),
                |hint| proposal_artifact.clone().with_payload_block_hint(hint),
            );
            let accepted_for_broadcast =
                self.lane_block_proposal_accepts_local_broadcast(&proposal);
            if accepted_for_broadcast {
                self.schedule_background(BackgroundRequest::Broadcast {
                    msg: BlockMessageWire::new(BlockMessage::LaneBlockProposal(proposal.clone())),
                });
                self.schedule_lane_block_message_to_validator_set(
                    BlockMessage::LaneBlockProposal(proposal.clone()),
                    proposal.descriptor.validator_set.as_slice(),
                );
                scheduled = scheduled.saturating_add(1);
            }
            if let Err(err) = self.handle_lane_block_proposal(proposal.clone()) {
                warn!(?err, "failed to cache locally produced lane-block proposal");
            }
        }

        let local_prepare_votes = prepare_vote_plans
            .iter()
            .filter_map(|plan| {
                let proposal = proposals
                    .iter()
                    .find(|proposal| proposal.proposal_hash == plan.proposal_hash)?;
                self.local_lane_block_prepare_vote(plan, proposal)
            })
            .collect::<Vec<_>>();
        let local_peer = self.common_config.peer.id().clone();
        for vote in local_prepare_votes {
            let accepted_for_broadcast =
                self.lane_block_vote_accepts_local_broadcast(&vote, &local_peer);
            if accepted_for_broadcast {
                self.schedule_background(BackgroundRequest::Broadcast {
                    msg: BlockMessageWire::new(BlockMessage::LaneBlockVote(vote.clone())),
                });
                self.schedule_lane_block_vote_to_known_validator_set(&vote);
                scheduled = scheduled.saturating_add(1);
            }
            if let Err(err) = self.handle_lane_block_vote(vote, Some(&local_peer)) {
                warn!(
                    ?err,
                    "failed to cache locally produced lane-block prepare vote"
                );
            }
        }

        if scheduled > 0 {
            debug!(
                lane_block_proposals = proposals.len(),
                lane_block_prepare_vote_plans = prepare_vote_plans.len(),
                scheduled,
                "scheduled finalized lane-block proposal artifacts for broadcast"
            );
        }

        scheduled
    }

    /// Retry guards retained after an earlier atomic-return invariant failure.
    pub(super) fn retry_quarantined_proposal_guards(&mut self) -> bool {
        if self.proposal_guard_return_quarantine.guards.is_empty() {
            return true;
        }
        match self.queue.return_transaction_guards(
            &mut self.proposal_guard_return_quarantine.guards,
            self.state.as_ref(),
        ) {
            Ok(report) => {
                info!(
                    ?report,
                    "restored quarantined proposal transaction guards before new selection"
                );
                true
            }
            Err(err) => {
                error!(
                    ?err,
                    guard_count = self.proposal_guard_return_quarantine.guards.len(),
                    "proposal transaction-guard quarantine remains blocked"
                );
                false
            }
        }
    }

    /// Persist or securely hand off canonical lane-owned executable payloads
    /// before lane voting.
    ///
    /// When the global proposer is in the independently selected lane
    /// committee it signs and disseminates the canonical payload directly.
    /// Otherwise it signs a non-executable handoff so active committee members
    /// can verify the exact bytes and re-sign them under committee authority.
    fn persist_lane_executable_payloads(
        &mut self,
        proposals: &[crate::sumeragi::consensus::LaneBlockProposalV1],
        transactions: &[crate::tx::AcceptedTransaction<'_>],
        epoch: u64,
        global_proposal_hint: LaneBlockProposalPayloadHintV1,
    ) -> usize {
        let local_peer = self.common_config.peer.id().clone();
        let mut scheduled = 0_usize;
        for proposal in proposals {
            if proposal.descriptor.lane_block_view != 0 {
                continue;
            }
            let mut anchored_proposal = proposal.clone();
            anchored_proposal.payload_block_hint = Some(global_proposal_hint);
            let mut entrypoints =
                Vec::with_capacity(proposal.descriptor.accepted_candidate_indices.len());
            let mut complete = true;
            for raw_index in &proposal.descriptor.accepted_candidate_indices {
                let Some(entrypoint) = usize::try_from(*raw_index)
                    .ok()
                    .and_then(|index| transactions.get(index))
                    .map(|transaction| transaction.entrypoint().clone())
                else {
                    complete = false;
                    break;
                };
                entrypoints.push(entrypoint);
            }
            if !complete {
                warn!(
                    lane_id = proposal.descriptor.lane_id.as_u32(),
                    lane_block_height = proposal.descriptor.lane_block_height,
                    "skipping autonomous lane payload with out-of-range entrypoint index"
                );
                continue;
            }
            if super::lane_scheduler::lane_block_redrive_leader(&anchored_proposal, 0)
                == Some(&local_peer)
            {
                let payload = match crate::lane_consensus::LaneExecutablePayloadV1::new_signed(
                    self.chain_hash,
                    epoch,
                    anchored_proposal,
                    entrypoints,
                    local_peer.clone(),
                    self.common_config.key_pair.private_key(),
                ) {
                    Ok(payload) => payload,
                    Err(err) => {
                        warn!(
                            ?err,
                            lane_id = proposal.descriptor.lane_id.as_u32(),
                            lane_block_height = proposal.descriptor.lane_block_height,
                            "failed to authenticate autonomous lane payload"
                        );
                        continue;
                    }
                };
                if let Err(err) = self.handle_lane_executable_payload(payload, Some(&local_peer)) {
                    warn!(
                        ?err,
                        lane_id = proposal.descriptor.lane_id.as_u32(),
                        lane_block_height = proposal.descriptor.lane_block_height,
                        "failed to persist or disseminate autonomous lane payload"
                    );
                    continue;
                }
                scheduled = scheduled.saturating_add(1);
                continue;
            }

            let handoff = match crate::lane_consensus::LaneExecutablePayloadHandoffV1::new_signed(
                self.chain_hash,
                epoch,
                anchored_proposal,
                entrypoints,
                local_peer.clone(),
                self.common_config.key_pair.private_key(),
            ) {
                Ok(handoff) => handoff,
                Err(err) => {
                    warn!(
                        ?err,
                        lane_id = proposal.descriptor.lane_id.as_u32(),
                        lane_block_height = proposal.descriptor.lane_block_height,
                        "failed to authenticate autonomous lane payload handoff"
                    );
                    continue;
                }
            };
            let message = BlockMessage::LaneExecutablePayloadHandoff(handoff);
            let wire_len = consensus_block_wire_len(&local_peer, &message);
            if wire_len > self.consensus_payload_frame_cap {
                warn!(
                    lane_id = proposal.descriptor.lane_id.as_u32(),
                    lane_block_height = proposal.descriptor.lane_block_height,
                    wire_len,
                    cap = self.consensus_payload_frame_cap,
                    "lane payload handoff exceeds configured consensus payload frame cap"
                );
                continue;
            }
            self.schedule_lane_block_message_to_validator_set(
                message.clone(),
                proposal.descriptor.validator_set.as_slice(),
            );
            scheduled = scheduled.saturating_add(1);
        }
        scheduled
    }

    fn requeue_accepted_transaction(
        &self,
        tx: AcceptedTransaction<'static>,
        routing_plan: crate::queue::RoutingPlan,
        warn_context: &'static str,
    ) {
        let tx_hash = tx.as_ref().hash();
        if let Err(err) =
            self.queue
                .push_requeued_with_routing_plan(tx, routing_plan, self.state.as_ref())
        {
            match err.err {
                crate::queue::Error::IsInQueue => {
                    trace!(
                        tx = %tx_hash,
                        "transaction already in queue during proposal requeue"
                    );
                }
                crate::queue::Error::InBlockchain => {
                    trace!(
                        tx = %tx_hash,
                        "transaction already committed during proposal requeue"
                    );
                }
                err => {
                    warn!(?err, "{warn_context}");
                }
            }
        }
    }

    fn nudge_proposal_guard_return_retry(&mut self) {
        self.subsystems.propose.pacemaker.next_deadline = Instant::now();
        if let Some(wake) = self.wake_tx.as_ref() {
            let _ = wake.try_send(());
        }
    }

    #[cfg(test)]
    fn run_proposal_guard_return_admission_flood(&self) {
        let transactions =
            PROPOSAL_GUARD_RETURN_ADMISSION_FLOOD.with(|flood| flood.borrow_mut().take());
        let Some(transactions) = transactions else {
            return;
        };
        let attempted = transactions.len();
        let mut admitted = 0usize;
        let mut full = 0usize;
        for transaction in transactions {
            match self.queue.push(transaction, self.state.view()) {
                Ok(_) => admitted = admitted.saturating_add(1),
                Err(failure) if matches!(failure.err, crate::queue::Error::Full) => {
                    full = full.saturating_add(1);
                }
                Err(_) => {}
            }
        }
        PROPOSAL_GUARD_RETURN_ADMISSION_FLOOD_REPORT.with(|report| {
            report.set(ProposalGuardReturnAdmissionFloodReport {
                attempted,
                admitted,
                full,
            });
        });
    }

    /// Return popped proposal guards without re-running admission.
    ///
    /// An invariant failure leaves the complete atomic batch live. Move it into actor-owned
    /// quarantine so unwinding or an ordinary branch return cannot invoke guard `Drop` and remove
    /// accepted transactions. The next proposal attempt retries quarantine before selecting work.
    pub(super) fn return_proposal_guards_or_quarantine(
        &mut self,
        guards: &mut Vec<crate::queue::TransactionGuard>,
        context: &'static str,
    ) -> bool {
        if guards.is_empty() {
            return true;
        }
        #[cfg(test)]
        self.run_proposal_guard_return_admission_flood();
        match self
            .queue
            .return_transaction_guards(guards, self.state.as_ref())
        {
            Ok(report) => {
                trace!(?report, context, "returned proposal transaction guards");
                true
            }
            Err(err) => {
                error!(
                    ?err,
                    guard_count = guards.len(),
                    context,
                    "quarantining proposal transaction guards after atomic-return invariant failure"
                );
                self.proposal_guard_return_quarantine.guards.append(guards);
                self.nudge_proposal_guard_return_retry();
                false
            }
        }
    }

    /// Move a still-live local batch behind an already quarantined return batch.
    fn quarantine_proposal_guards_without_return(
        &mut self,
        guards: &mut Vec<crate::queue::TransactionGuard>,
        context: &'static str,
    ) {
        if guards.is_empty() {
            return;
        }
        error!(
            guard_count = guards.len(),
            context, "retaining additional proposal transaction guards behind blocked quarantine"
        );
        self.proposal_guard_return_quarantine.guards.append(guards);
        self.nudge_proposal_guard_return_retry();
    }

    pub(super) fn drop_stale_pending_block(
        &mut self,
        pending_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
    ) -> Option<(usize, usize, usize, usize, bool)> {
        self.drop_stale_pending_block_skipping_known_committed(
            pending_hash,
            height,
            view,
            true,
            None,
        )
    }

    pub(super) fn drop_stale_pending_block_skipping_committed_txs(
        &mut self,
        pending_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        known_committed_hashes: Option<&BTreeSet<HashOf<SignedTransaction>>>,
    ) -> Option<(usize, usize, usize, usize, bool)> {
        self.drop_stale_pending_block_skipping_known_committed(
            pending_hash,
            height,
            view,
            true,
            known_committed_hashes,
        )
    }

    pub(super) fn drop_stale_pending_block_for_fresh_proposal(
        &mut self,
        pending_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
    ) -> Option<(usize, usize, usize, usize, bool)> {
        self.drop_stale_pending_block_skipping_known_committed(
            pending_hash,
            height,
            view,
            true,
            None,
        )
    }

    pub(super) fn drop_pending_block_for_memory_cap(
        &mut self,
        pending_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
    ) -> Option<(usize, usize, usize, usize, bool)> {
        self.drop_stale_pending_block_skipping_known_committed(
            pending_hash,
            height,
            view,
            false,
            None,
        )
    }

    fn drop_stale_pending_block_skipping_known_committed(
        &mut self,
        pending_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        retain_for_body_repair: bool,
        known_committed_hashes: Option<&BTreeSet<HashOf<SignedTransaction>>>,
    ) -> Option<(usize, usize, usize, usize, bool)> {
        if retain_for_body_repair
            && self.should_retain_stale_pending_payload_for_body_repair(pending_hash, height, view)
        {
            let mut pending = self
                .pending
                .pending_blocks
                .remove(&pending_hash)
                .expect("checked pending block exists");
            if !pending.is_retired_same_height() {
                pending.retire_same_height();
            }
            let frontier_info =
                self.authoritative_slot_frontier_info(pending.height, pending.view, pending_hash);
            self.slot_tracker.note_retained_branch(
                pending.height,
                pending.view,
                pending_hash,
                frontier_info,
                true,
                Instant::now(),
            );
            self.pending.pending_fetch_requests.remove(&pending_hash);
            self.pending
                .pending_block_body_requests
                .remove(&pending_hash);
            self.clear_validation_ownership_for_block(pending_hash);
            self.subsystems
                .propose
                .proposal_cache
                .pop_hint(height, view);
            self.subsystems
                .propose
                .proposal_cache
                .pop_proposal(height, view);
            self.pending.pending_blocks.insert(pending_hash, pending);
            debug!(
                height,
                view,
                block = %pending_hash,
                "retired stale vote-backed pending payload for exact body repair"
            );
            return Some((0, 0, 0, 0, true));
        }

        if self.active_commit_inflight_blocks_stale_owner_clear(pending_hash, height, view, true) {
            return None;
        }

        let (tx_count, requeued, failures, duplicate_failures, retained_for_retry) =
            super::drop_pending_block_and_requeue_skipping_known_committed(
                &mut self.pending.pending_blocks,
                pending_hash,
                self.queue.as_ref(),
                self.state.as_ref(),
                known_committed_hashes,
            )?;

        self.clear_validation_ownership_for_block(pending_hash);
        self.clean_rbc_sessions_for_block(pending_hash, height);
        self.qc_cache
            .retain(|(_, hash, _, _, _, _, _), _| hash != &pending_hash);
        self.qc_signer_tally
            .retain(|(_, hash, _, _, _, _, _), _| hash != &pending_hash);
        self.subsystems
            .propose
            .proposal_cache
            .pop_hint(height, view);
        self.subsystems
            .propose
            .proposal_cache
            .pop_proposal(height, view);
        // Keep proposals_seen so we don't re-propose in the same view after dropping a stale block.
        let _ =
            self.active_commit_inflight_blocks_stale_owner_clear(pending_hash, height, view, false);

        Some((
            tx_count,
            requeued,
            failures,
            duplicate_failures,
            retained_for_retry,
        ))
    }

    fn should_retain_stale_pending_payload_for_body_repair(
        &self,
        pending_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
    ) -> bool {
        if !self.runtime_da_enabled() || self.kura.get_block_height_by_hash(pending_hash).is_some()
        {
            return false;
        }
        let Some(pending) = self.pending.pending_blocks.get(&pending_hash) else {
            return false;
        };
        if pending.height != height
            || pending.view != view
            || pending.is_retired_same_height()
            || matches!(pending.validation_status, ValidationStatus::Invalid)
        {
            return false;
        }
        let committed_height = self.state.committed_height();
        let tip_hash = self.state.latest_block_hash_fast();
        if !super::pending_extends_tip(
            pending.height,
            pending.block.header().prev_block_hash(),
            committed_height,
            tip_hash,
        ) {
            return false;
        }
        pending.local_commit_vote_emitted()
            || pending.commit_qc_observed()
            || self.pending_block_has_commit_votes(pending_hash, pending.height, pending.view)
            || self.pending_block_has_qc(pending_hash, pending.height, pending.view)
    }

    fn active_commit_inflight_blocks_stale_owner_clear(
        &self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        requeue_transactions: bool,
    ) -> bool {
        let Some(inflight) = self.subsystems.commit.inflight.as_ref().filter(|inflight| {
            inflight.block_hash == block_hash
                && inflight.pending.height == height
                && inflight.pending.view == view
        }) else {
            return false;
        };
        let tx_count = inflight.pending.block.external_entrypoints_cloned().count();
        warn!(
            height,
            view,
            block = %block_hash,
            commit_id = inflight.id,
            requeue_transactions,
            tx_count,
            elapsed_ms = Instant::now()
                .saturating_duration_since(inflight.enqueue_time)
                .as_millis(),
            "stale commit-inflight owner is still running; waiting for commit worker result"
        );
        true
    }

    fn request_frontier_owner_body_repair(
        &mut self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        now: Instant,
    ) -> bool {
        if self.frontier_block_materialized_locally(block_hash) {
            return false;
        }

        let (consensus_mode, _, _) = self.consensus_context_for_height(height);
        let mut roster = self.roster_for_vote_with_mode(block_hash, height, view, consensus_mode);
        if roster.is_empty() {
            roster = self.rbc_roster_for_session((block_hash, height, view));
        }
        if roster.is_empty() {
            roster = self.roster_for_live_vote_with_mode(height, consensus_mode);
        }
        if roster.is_empty() {
            roster = self.effective_commit_topology();
        }
        if roster.is_empty() {
            return false;
        }

        let topology = super::network_topology::Topology::new(roster);
        let seeded = self.handle_frontier_body_gap_with_topology(
            block_hash,
            height,
            view,
            &BTreeSet::new(),
            &topology,
            true,
            now,
        );
        let fetch_requested = self.emit_frontier_block_body_fetch_urgent(now);
        if seeded || fetch_requested {
            debug!(
                height,
                view,
                block = %block_hash,
                seeded_frontier_body_repair = seeded,
                fetch_requested,
                "requested exact frontier body repair for protected owner"
            );
        }
        seeded || fetch_requested
    }

    fn escalate_stale_vote_locked_frontier_owner_recovery(
        &mut self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        now: Instant,
        trigger: &'static str,
    ) -> bool {
        let mut progress = self.request_frontier_owner_body_repair(block_hash, height, view, now);
        if self.frontier_block_materialized_locally(block_hash) {
            let targets =
                self.known_block_commit_qc_recovery_targets(block_hash, height, view, &[]);
            progress |= self.maybe_request_known_block_commit_qc_recovery(
                block_hash, height, view, &targets, None, trigger,
            );
        }
        if self.frontier_slot.as_ref().is_some_and(|slot| {
            slot.height == height && slot.view == view && slot.block_hash == block_hash
        }) {
            progress |= matches!(
                self.handle_frontier_slot_event(
                    now,
                    FrontierSlotEvent::OnLagWindowExpired {
                        reason: "frontier_stall_reset",
                    },
                ),
                FrontierRecoveryAdvance::CatchUp | FrontierRecoveryAdvance::Rotate
            );
        }
        if !progress {
            progress |=
                self.request_range_pull_from_anchor(height, "frontier_stall_reset_fallback", now);
        }
        progress
    }

    pub(super) fn maybe_yield_stale_frontier_owner_for_fresh_proposal(
        &mut self,
        height: u64,
        view: u64,
        owner_hash: HashOf<BlockHeader>,
        owner_view: u64,
        now: Instant,
        pending_queue_len: usize,
    ) -> bool {
        let missing_qc_liveness_active = self.frontier_missing_qc_liveness_active(height, view);
        let committed_height = self.committed_height_snapshot();
        let same_height_recovery_view =
            height == committed_height.saturating_add(1) && owner_view < view && view > 0;
        if !self.config.resilience.enabled
            || (pending_queue_len == 0 && !missing_qc_liveness_active && !same_height_recovery_view)
            || height != committed_height.saturating_add(1)
            || owner_view >= view
        {
            return false;
        }
        let quorum_yield_age = self
            .quorum_timeout(self.runtime_da_enabled())
            .max(Duration::from_millis(1));
        let fast_recovery_yield_age =
            super::reschedule::near_quorum_payload_timeout(self.rebroadcast_cooldown())
                .min(quorum_yield_age)
                .max(Duration::from_millis(1));
        let standard_yield_age = quorum_yield_age
            .max(self.frontier_slot_lag_window())
            .max(Duration::from_millis(1));
        let hard_yield_age = standard_yield_age.saturating_mul(3);
        let owner_qc_observed =
            self.same_height_block_has_recoverable_qc(owner_hash, height, owner_view);
        let owner_slot_evidence = self
            .frontier_slot
            .as_ref()
            .filter(|slot| {
                slot.height == height && slot.view == owner_view && slot.block_hash == owner_hash
            })
            .map(|slot| {
                (
                    now.saturating_duration_since(slot.timers.last_progress_at)
                        .max(now.saturating_duration_since(slot.timers.observed_at)),
                    slot.quorum_progress.commit_qc_observed,
                    self.frontier_slot_competing_quorum_locked_for_view(slot, view),
                )
            });
        let local_vote_consensus_locked = self
            .local_same_height_vote(height, self.epoch_for_height(height))
            .as_ref()
            .is_some_and(|vote| self.local_same_height_vote_has_consensus_lock(height, vote));
        let local_commit_vote_blocks_fresh_branch = self
            .local_same_height_vote(height, self.epoch_for_height(height))
            .as_ref()
            .is_some_and(|vote| {
                matches!(vote.phase, crate::sumeragi::consensus::Phase::Commit)
                    && !self.local_same_height_vote_is_committed_parent_marker(height, view, vote)
            });
        let commit_inflight_live =
            self.subsystems
                .commit
                .inflight
                .as_ref()
                .is_some_and(|inflight| {
                    inflight.block_hash == owner_hash
                        && !inflight.pending.aborted
                        && inflight.pending.validation_status != ValidationStatus::Invalid
                        && inflight.pending.height == height
                        && inflight.pending.view == owner_view
                });
        let owner_pending_commit_qc_observed = self
            .pending
            .pending_blocks
            .get(&owner_hash)
            .is_some_and(|pending| {
                pending.height == height
                    && pending.view == owner_view
                    && !pending.aborted
                    && pending.validation_status != ValidationStatus::Invalid
                    && pending.commit_qc_observed()
            });
        let commit_quorum_timeout_owner_clear = self
            .stale_frontier_owner_commit_quorum_timeout_allows_clear(
                height,
                view,
                owner_hash,
                owner_view,
                now,
                pending_queue_len,
            );
        let new_view_qc_supersedes_owner = self.latest_committed_qc().is_some_and(|highest_qc| {
            self.new_view_qc_supersedes_noncommit_same_height_vote_conflict(
                height,
                view,
                highest_qc,
                owner_hash,
                owner_view,
                crate::sumeragi::consensus::Phase::Prepare,
            )
        });
        let Some(owner_pending) = self
            .pending
            .pending_blocks
            .get(&owner_hash)
            .filter(|pending| {
                pending.height == height
                    && pending.view == owner_view
                    && !pending.commit_qc_observed()
            })
        else {
            let mut body_repair_requested = false;
            let mut passive_catchup_requested = false;
            let mut stale_vote_locked_recovery_requested = false;
            if let Some((owner_age, frontier_commit_qc_observed, competing_quorum_locked)) =
                owner_slot_evidence
            {
                let slot_commit_qc_repairable =
                    frontier_commit_qc_observed && owner_age < hard_yield_age;
                let protected_owner = owner_qc_observed
                    || slot_commit_qc_repairable
                    || local_vote_consensus_locked
                    || (local_commit_vote_blocks_fresh_branch && !new_view_qc_supersedes_owner)
                    || (competing_quorum_locked && !new_view_qc_supersedes_owner)
                    || commit_inflight_live;
                body_repair_requested = protected_owner
                    && self.request_frontier_owner_body_repair(owner_hash, height, owner_view, now);
                let owner_body_repair_active = body_repair_requested
                    || self.frontier_slot.as_ref().is_some_and(|slot| {
                        slot.height == height
                            && slot.view == owner_view
                            && slot.block_hash == owner_hash
                            && slot.exact_fetch_armed
                            && slot.body_missing()
                    });
                let owner_body_repair_lagged = owner_age
                    >= self
                        .frontier_slot_lag_window()
                        .max(Duration::from_millis(1));
                if protected_owner
                    && owner_body_repair_active
                    && owner_body_repair_lagged
                    && !self.frontier_block_materialized_locally(owner_hash)
                {
                    let _ = self.handoff_contiguous_frontier_to_passive_catchup(
                        height,
                        now,
                        "stale_frontier_owner_body_repair",
                    );
                    passive_catchup_requested = self.request_range_pull_from_anchor(
                        height,
                        "stale_frontier_owner_body_repair",
                        now,
                    );
                    if passive_catchup_requested {
                        info!(
                            height,
                            view,
                            owner_view,
                            owner = %owner_hash,
                            owner_age_ms = owner_age.as_millis(),
                            frontier_lag_window_ms = self.frontier_slot_lag_window().as_millis(),
                            queue_len = pending_queue_len,
                            frontier_commit_qc_observed,
                            competing_quorum_locked,
                            new_view_qc_supersedes_owner,
                            "escalated stale frontier owner body repair to passive catch-up"
                        );
                    }
                }
                let stale_vote_locked_owner = protected_owner
                    && owner_age >= hard_yield_age
                    && (local_vote_consensus_locked
                        || (local_commit_vote_blocks_fresh_branch
                            && !new_view_qc_supersedes_owner)
                        || (competing_quorum_locked && !new_view_qc_supersedes_owner))
                    && !owner_qc_observed
                    && !owner_pending_commit_qc_observed
                    && !commit_inflight_live;
                if stale_vote_locked_owner {
                    stale_vote_locked_recovery_requested = self
                        .escalate_stale_vote_locked_frontier_owner_recovery(
                            owner_hash,
                            height,
                            owner_view,
                            now,
                            "stale_vote_locked_frontier_owner",
                        );
                    if stale_vote_locked_recovery_requested {
                        info!(
                            height,
                            view,
                            owner_view,
                            owner = %owner_hash,
                            owner_age_ms = owner_age.as_millis(),
                            hard_yield_age_ms = hard_yield_age.as_millis(),
                            local_vote_consensus_locked,
                            competing_quorum_locked,
                            new_view_qc_supersedes_owner,
                            "escalated stale vote-locked frontier owner to committed-anchor catch-up"
                        );
                    }
                }
                let yield_age = if same_height_recovery_view
                    && !owner_qc_observed
                    && !frontier_commit_qc_observed
                    && !owner_pending_commit_qc_observed
                {
                    fast_recovery_yield_age
                } else {
                    standard_yield_age
                };
                let stale_unprotected_owner = owner_age >= yield_age && !protected_owner;
                let recovery_exhausted = owner_age >= hard_yield_age;
                let new_view_superseded_unrepairable_owner = new_view_qc_supersedes_owner
                    && !owner_qc_observed
                    && !frontier_commit_qc_observed
                    && !owner_pending_commit_qc_observed
                    && !local_vote_consensus_locked
                    && !commit_inflight_live;
                let stale_vote_lock_allows_owner_clear =
                    self.latest_committed_qc().is_some_and(|highest_qc| {
                        self.same_height_vote_lock_blocking_candidate(height, view, None)
                            .is_some_and(|lock| {
                                lock.block_hash == owner_hash
                                    && lock.view == owner_view
                                    && self.stale_same_height_vote_lock_allows_proposal_rotation(
                                        height, view, &lock, now, highest_qc,
                                    )
                            })
                    });
                if (stale_unprotected_owner
                    || recovery_exhausted
                    || new_view_superseded_unrepairable_owner
                    || stale_vote_lock_allows_owner_clear
                    || commit_quorum_timeout_owner_clear.is_some())
                    && !owner_qc_observed
                    && !owner_pending_commit_qc_observed
                    && !local_vote_consensus_locked
                    && !(local_commit_vote_blocks_fresh_branch
                        && !new_view_qc_supersedes_owner
                        && !stale_vote_lock_allows_owner_clear
                        && commit_quorum_timeout_owner_clear.is_none())
                    && (!competing_quorum_locked
                        || new_view_qc_supersedes_owner
                        || recovery_exhausted
                        || stale_vote_lock_allows_owner_clear
                        || commit_quorum_timeout_owner_clear.is_some())
                    && !commit_inflight_live
                {
                    self.frontier_slot = None;
                    let (
                        commit_timeout_votes,
                        commit_timeout_required,
                        commit_timeout_age,
                        commit_timeout,
                    ) = commit_quorum_timeout_owner_clear
                        .map(|(votes, required, age, timeout)| {
                            (
                                Some(votes),
                                Some(required),
                                Some(age.as_millis()),
                                Some(timeout.as_millis()),
                            )
                        })
                        .unwrap_or((None, None, None, None));
                    info!(
                        height,
                        view,
                        owner_view,
                        owner = %owner_hash,
                        owner_age_ms = owner_age.as_millis(),
                        min_yield_age_ms = yield_age.as_millis(),
                        fast_yield_age_ms = fast_recovery_yield_age.as_millis(),
                        standard_yield_age_ms = standard_yield_age.as_millis(),
                        hard_yield_age_ms = hard_yield_age.as_millis(),
                        queue_len = pending_queue_len,
                        frontier_commit_qc_observed,
                        owner_pending_commit_qc_observed,
                        competing_quorum_locked,
                        new_view_qc_supersedes_owner,
                        new_view_superseded_unrepairable_owner,
                        stale_vote_lock_allows_owner_clear,
                        commit_timeout_votes,
                        commit_timeout_required,
                        commit_timeout_age_ms = commit_timeout_age,
                        commit_timeout_ms = commit_timeout,
                        "cleared no-pending stale frontier owner for fresh resilience proposal"
                    );
                    return true;
                }
            }
            if let Some(suppressed_since_last) = self.proposal_defer_warning_log.allow(
                ProposalDeferWarningKind::FrontierOwnerYieldBlocked,
                height,
                view,
                owner_hash,
                now,
                Duration::from_secs(3),
            ) {
                let pending_snapshot = self.pending.pending_blocks.get(&owner_hash);
                warn!(
                    height,
                    view,
                    owner_view,
                    owner = %owner_hash,
                    pending_present = pending_snapshot.is_some(),
                    pending_aborted = pending_snapshot.is_some_and(|pending| pending.aborted),
                    pending_retired = pending_snapshot
                        .is_some_and(PendingBlock::is_retired_same_height),
                    pending_validation = ?pending_snapshot.map(|pending| pending.validation_status),
                    pending_commit_qc_observed = pending_snapshot
                        .is_some_and(PendingBlock::commit_qc_observed),
                    owner_slot_present = owner_slot_evidence.is_some(),
                    owner_slot_age_ms = ?owner_slot_evidence.map(|(age, _, _)| age.as_millis()),
                    fast_yield_age_ms = fast_recovery_yield_age.as_millis(),
                    standard_yield_age_ms = standard_yield_age.as_millis(),
                    hard_yield_age_ms = hard_yield_age.as_millis(),
                    owner_qc_observed,
                    owner_pending_commit_qc_observed,
                    local_vote_consensus_locked,
                    local_commit_vote_blocks_fresh_branch,
                    commit_inflight_live,
                    body_repair_requested,
                    passive_catchup_requested,
                    stale_vote_locked_recovery_requested,
                    new_view_qc_supersedes_owner,
                    frontier_commit_qc_observed = owner_slot_evidence
                        .is_some_and(|(_, observed, _)| observed),
                    competing_quorum_locked = owner_slot_evidence
                        .is_some_and(|(_, _, locked)| locked),
                    suppressed_since_last,
                    "stale frontier owner yield blocked: owner pending is not yieldable"
                );
            }
            return false;
        };
        let owner_age = owner_pending
            .progress_age(now)
            .max(now.saturating_duration_since(owner_pending.inserted_at));
        let recovery_age = self.stale_same_height_recovery_age(height, owner_view, now);
        let recovery_exhausted =
            owner_age >= hard_yield_age || recovery_age.is_some_and(|age| age >= hard_yield_age);
        let local_vote = self.local_same_height_vote(height, self.epoch_for_height(height));
        let local_vote_new_view_qc_supersedes = local_vote.as_ref().is_some_and(|vote| {
            self.latest_committed_qc().is_some_and(|highest_qc| {
                self.new_view_qc_supersedes_noncommit_same_height_vote_conflict(
                    height,
                    view,
                    highest_qc,
                    vote.block_hash,
                    vote.view,
                    vote.phase,
                )
            })
        });
        let local_vote_blocks = local_vote.as_ref().is_some_and(|vote| {
            !local_vote_new_view_qc_supersedes
                && !matches!(vote.phase, crate::sumeragi::consensus::Phase::Commit)
                && self.local_same_height_vote_blocks_fresh_proposal(height, view, vote, now, false)
        });
        let local_commit_vote_matches_owner = local_vote.as_ref().is_some_and(|vote| {
            matches!(vote.phase, crate::sumeragi::consensus::Phase::Commit)
                && vote.block_hash == owner_hash
                && vote.view == owner_view
                && !self.local_same_height_vote_is_committed_parent_marker(height, view, vote)
        });
        let local_commit_vote_present = local_vote.as_ref().is_some_and(|vote| {
            !local_vote_new_view_qc_supersedes
                && matches!(vote.phase, crate::sumeragi::consensus::Phase::Commit)
                && !self.local_same_height_vote_is_committed_parent_marker(height, view, vote)
        });
        let (frontier_commit_qc_observed, competing_quorum_locked) = self
            .frontier_slot
            .as_ref()
            .filter(|slot| {
                slot.height == height && slot.view == owner_view && slot.block_hash == owner_hash
            })
            .map_or((false, false), |slot| {
                (
                    slot.quorum_progress.commit_qc_observed,
                    self.frontier_slot_competing_quorum_locked_for_view(slot, view),
                )
            });
        let yield_age = if same_height_recovery_view
            && !owner_qc_observed
            && !frontier_commit_qc_observed
            && !owner_pending_commit_qc_observed
        {
            fast_recovery_yield_age
        } else {
            standard_yield_age
        };
        let frontier_commit_qc_blocks_yield = frontier_commit_qc_observed && !recovery_exhausted;
        let competing_quorum_blocks_yield = competing_quorum_locked
            && !new_view_qc_supersedes_owner
            && owner_age < yield_age
            && commit_quorum_timeout_owner_clear.is_none();
        let local_commit_vote_superseded_by_owner_new_view = local_commit_vote_matches_owner
            && new_view_qc_supersedes_owner
            && recovery_exhausted
            && !owner_qc_observed
            && !frontier_commit_qc_observed
            && !owner_pending_commit_qc_observed
            && !local_vote_consensus_locked
            && !commit_inflight_live;
        let local_commit_vote_blocks_yield = local_commit_vote_present
            && recovery_exhausted
            && !local_commit_vote_superseded_by_owner_new_view
            && commit_quorum_timeout_owner_clear.is_none();
        if owner_qc_observed
            || frontier_commit_qc_blocks_yield
            || local_vote_consensus_locked
            || competing_quorum_blocks_yield
            || local_commit_vote_blocks_yield
            || (local_vote_blocks && !recovery_exhausted)
        {
            if let Some(suppressed_since_last) = self.proposal_defer_warning_log.allow(
                ProposalDeferWarningKind::FrontierOwnerYieldBlocked,
                height,
                view,
                owner_hash,
                now,
                Duration::from_secs(3),
            ) {
                warn!(
                    height,
                    view,
                    owner_view,
                    owner = %owner_hash,
                    owner_age_ms = owner_age.as_millis(),
                    recovery_age_ms = recovery_age.map(|age| age.as_millis()),
                    min_yield_age_ms = yield_age.as_millis(),
                    fast_yield_age_ms = fast_recovery_yield_age.as_millis(),
                    standard_yield_age_ms = standard_yield_age.as_millis(),
                    hard_yield_age_ms = hard_yield_age.as_millis(),
                    recovery_exhausted,
                    owner_qc_observed,
                    frontier_commit_qc_observed,
                    frontier_commit_qc_blocks_yield,
                    local_vote_consensus_locked,
                    local_commit_vote_present,
                    local_commit_vote_blocks_yield,
                    local_vote_blocks,
                    competing_quorum_locked,
                    competing_quorum_blocks_yield,
                    new_view_qc_supersedes_owner,
                    local_vote_new_view_qc_supersedes,
                    local_commit_vote_superseded_by_owner_new_view,
                    commit_quorum_timeout_owner_clear = commit_quorum_timeout_owner_clear.is_some(),
                    suppressed_since_last,
                    "stale frontier owner yield blocked by consensus evidence"
                );
            }
            return false;
        }
        if owner_age < yield_age
            && !recovery_exhausted
            && commit_quorum_timeout_owner_clear.is_none()
        {
            if let Some(suppressed_since_last) = self.proposal_defer_warning_log.allow(
                ProposalDeferWarningKind::FrontierOwnerYieldBlocked,
                height,
                view,
                owner_hash,
                now,
                Duration::from_secs(3),
            ) {
                warn!(
                    height,
                    view,
                    owner_view,
                    owner = %owner_hash,
                    owner_age_ms = owner_age.as_millis(),
                    recovery_age_ms = recovery_age.map(|age| age.as_millis()),
                    min_yield_age_ms = yield_age.as_millis(),
                    fast_yield_age_ms = fast_recovery_yield_age.as_millis(),
                    standard_yield_age_ms = standard_yield_age.as_millis(),
                    suppressed_since_last,
                    "stale frontier owner yield blocked: owner still inside yield grace"
                );
            }
            return false;
        }

        if self
            .active_commit_inflight_blocks_stale_owner_clear(owner_hash, height, owner_view, true)
        {
            return false;
        }

        let dropped =
            self.drop_stale_pending_block_for_fresh_proposal(owner_hash, height, owner_view);
        if self.frontier_slot.as_ref().is_some_and(|slot| {
            slot.height == height && slot.view == owner_view && slot.block_hash == owner_hash
        }) {
            self.frontier_slot = None;
        }
        if let Some((tx_count, requeued, failures, duplicate_failures, retained_for_retry)) =
            dropped
        {
            let (commit_timeout_votes, commit_timeout_required, commit_timeout_age, commit_timeout) =
                commit_quorum_timeout_owner_clear
                    .map(|(votes, required, age, timeout)| {
                        (
                            Some(votes),
                            Some(required),
                            Some(age.as_millis()),
                            Some(timeout.as_millis()),
                        )
                    })
                    .unwrap_or((None, None, None, None));
            info!(
                height,
                view,
                owner_view,
                owner = %owner_hash,
                owner_age_ms = owner_age.as_millis(),
                recovery_age_ms = recovery_age.map(|age| age.as_millis()),
                min_yield_age_ms = yield_age.as_millis(),
                fast_yield_age_ms = fast_recovery_yield_age.as_millis(),
                standard_yield_age_ms = standard_yield_age.as_millis(),
                tx_count,
                requeued,
                failures,
                duplicate_failures,
                retained_for_retry,
                queue_len = pending_queue_len,
                commit_timeout_votes,
                commit_timeout_required,
                commit_timeout_age_ms = commit_timeout_age,
                commit_timeout_ms = commit_timeout,
                "yielded stale frontier owner for fresh resilience proposal"
            );
        } else {
            let (commit_timeout_votes, commit_timeout_required, commit_timeout_age, commit_timeout) =
                commit_quorum_timeout_owner_clear
                    .map(|(votes, required, age, timeout)| {
                        (
                            Some(votes),
                            Some(required),
                            Some(age.as_millis()),
                            Some(timeout.as_millis()),
                        )
                    })
                    .unwrap_or((None, None, None, None));
            info!(
                height,
                view,
                owner_view,
                owner = %owner_hash,
                owner_age_ms = owner_age.as_millis(),
                recovery_age_ms = recovery_age.map(|age| age.as_millis()),
                min_yield_age_ms = yield_age.as_millis(),
                fast_yield_age_ms = fast_recovery_yield_age.as_millis(),
                standard_yield_age_ms = standard_yield_age.as_millis(),
                cleared_inflight = false,
                queue_len = pending_queue_len,
                commit_timeout_votes,
                commit_timeout_required,
                commit_timeout_age_ms = commit_timeout_age,
                commit_timeout_ms = commit_timeout,
                "cleared stale frontier owner for fresh resilience proposal"
            );
        }
        true
    }

    fn local_same_height_vote_has_consensus_lock(
        &self,
        proposal_height: u64,
        existing_vote: &crate::sumeragi::consensus::Vote,
    ) -> bool {
        self.locked_qc
            .is_some_and(|locked| locked.height >= proposal_height)
            || self.same_height_block_has_recoverable_qc(
                existing_vote.block_hash,
                proposal_height,
                existing_vote.view,
            )
    }

    fn stale_frontier_owner_commit_quorum_timeout_allows_clear(
        &self,
        height: u64,
        view: u64,
        owner_hash: HashOf<BlockHeader>,
        owner_view: u64,
        now: Instant,
        pending_queue_len: usize,
    ) -> Option<(usize, usize, Duration, Duration)> {
        if !self.config.resilience.enabled
            || pending_queue_len == 0
            || height != self.committed_height_snapshot().saturating_add(1)
            || owner_view >= view
            || self.same_height_has_recoverable_qc(height)
            || self.same_height_block_has_observed_qc(owner_hash, height, owner_view)
            || self.active_commit_inflight_blocks_stale_owner_clear(
                owner_hash, height, owner_view, false,
            )
            || !self.latest_committed_qc().is_some_and(|highest_qc| {
                highest_qc.height.saturating_add(1) == height
                    && self.highest_qc_is_canonical_committed_tip(highest_qc)
            })
            || !self
                .same_height_vote_lock_blocking_candidate(height, view, None)
                .is_some_and(|lock| {
                    lock.block_hash == owner_hash
                        && lock.view == owner_view
                        && lock.commit_vote_observed
                })
        {
            return None;
        }

        let (consensus_mode, _, _) = self.consensus_context_for_height(height);
        let mut commit_roster =
            self.roster_for_vote_with_mode(owner_hash, height, owner_view, consensus_mode);
        if commit_roster.is_empty() {
            commit_roster = self.roster_for_live_vote_with_mode(height, consensus_mode);
        }
        if commit_roster.is_empty() {
            commit_roster = self.effective_commit_topology();
        }
        if commit_roster.is_empty() {
            return None;
        }
        let required = super::network_topology::Topology::new(commit_roster)
            .min_votes_for_commit()
            .max(1);
        let vote_status =
            self.commit_vote_quorum_status_for_block_detail(owner_hash, height, owner_view);
        if vote_status.vote_count == 0
            || vote_status.vote_count >= required
            || vote_status.quorum_reached
        {
            return None;
        }

        let pending_age = self
            .pending
            .pending_blocks
            .get(&owner_hash)
            .and_then(|pending| {
                (pending.height == height
                    && pending.view == owner_view
                    && !pending.aborted
                    && !pending.is_retired_same_height()
                    && pending.validation_status != ValidationStatus::Invalid)
                    .then(|| {
                        pending
                            .progress_age(now)
                            .max(now.saturating_duration_since(pending.inserted_at))
                    })
            });
        let slot_age = self.frontier_slot.as_ref().and_then(|slot| {
            (slot.height == height && slot.view == owner_view && slot.block_hash == owner_hash)
                .then(|| {
                    now.saturating_duration_since(slot.timers.last_progress_at)
                        .max(now.saturating_duration_since(slot.timers.observed_at))
                })
        });
        let owner_age = pending_age
            .into_iter()
            .chain(slot_age)
            .max()
            .unwrap_or_default();
        let timeout = self.commit_quorum_timeout().max(Duration::from_millis(1));
        super::missing_quorum_stale(owner_age, timeout, vote_status.quorum_reached).then_some((
            vote_status.vote_count,
            required,
            owner_age,
            timeout,
        ))
    }

    pub(super) fn same_height_block_has_observed_qc(
        &self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
    ) -> bool {
        self.same_height_block_has_recoverable_qc(block_hash, height, view)
            || self.frontier_slot.as_ref().is_some_and(|slot| {
                slot.height == height
                    && slot.view == view
                    && slot.block_hash == block_hash
                    && slot.quorum_progress.commit_qc_observed
            })
    }

    fn local_same_height_vote_has_hard_lock(
        &self,
        proposal_height: u64,
        existing_vote: &crate::sumeragi::consensus::Vote,
    ) -> bool {
        self.local_same_height_vote_has_consensus_lock(proposal_height, existing_vote)
            || self
                .subsystems
                .commit
                .inflight
                .as_ref()
                .is_some_and(|inflight| {
                    inflight.block_hash == existing_vote.block_hash
                        && !inflight.pending.aborted
                        && inflight.pending.validation_status != ValidationStatus::Invalid
                })
    }

    pub(super) fn stale_same_height_recovery_age(
        &self,
        proposal_height: u64,
        subject_view: u64,
        now: Instant,
    ) -> Option<Duration> {
        self.frontier_recovery
            .as_ref()
            .filter(|state| {
                state.frontier_height == proposal_height
                    && matches!(state.last_cause, "missing_qc" | "quorum_timeout")
                    && state
                        .last_rotation_view
                        .is_some_and(|view| view >= subject_view)
            })
            .map(|state| now.saturating_duration_since(state.entered_at))
    }

    pub(super) fn same_height_vote_recovery_view_gap_exhausted(
        &self,
        subject_view: u64,
        proposal_view: u64,
        total_validators: usize,
    ) -> bool {
        let view_gap = proposal_view.saturating_sub(subject_view);
        let min_view_gap = u64::try_from(total_validators.saturating_mul(8))
            .unwrap_or(u64::MAX)
            .max(8);
        view_gap >= min_view_gap
    }

    pub(super) fn same_height_vote_recovery_escalation_view_gap_exhausted(
        &self,
        subject_view: u64,
        proposal_view: u64,
        total_validators: usize,
    ) -> bool {
        let view_gap = proposal_view.saturating_sub(subject_view);
        // Escalation is still an assembly-only liveness path: raw local re-voting continues
        // to use the longer recovery_exhausted gate.
        let min_view_gap = u64::try_from(total_validators.saturating_mul(2))
            .unwrap_or(u64::MAX)
            .max(8);
        view_gap >= min_view_gap
    }

    fn same_height_stale_vote_recovery_exhausted(
        &self,
        proposal_height: u64,
        subject_view: u64,
        proposal_view: u64,
        total_validators: usize,
        now: Instant,
    ) -> bool {
        let hard_stale_age = self
            .quorum_timeout(self.runtime_da_enabled())
            .max(self.frontier_slot_lag_window())
            .max(Duration::from_millis(1))
            .saturating_mul(3);
        self.stale_same_height_recovery_age(proposal_height, subject_view, now)
            .is_some_and(|age| age >= hard_stale_age)
            || self.same_height_vote_recovery_view_gap_exhausted(
                subject_view,
                proposal_view,
                total_validators,
            )
    }

    fn stale_same_height_vote_lock_allows_proposal_rotation(
        &self,
        proposal_height: u64,
        proposal_view: u64,
        lock: &SameHeightVoteLock,
        now: Instant,
        highest_qc: crate::sumeragi::consensus::QcHeaderRef,
    ) -> bool {
        self.config.resilience.enabled
            && proposal_height == self.committed_height_snapshot().saturating_add(1)
            && proposal_view > lock.view
            && highest_qc.height.saturating_add(1) == proposal_height
            && self.highest_qc_is_canonical_committed_tip(highest_qc)
            && !self.same_height_has_recoverable_qc(proposal_height)
            && !self.same_height_block_has_observed_qc(lock.block_hash, proposal_height, lock.view)
            && !self
                .local_same_height_vote_has_live_proposal_material(proposal_height, lock.block_hash)
            && (self.same_height_stale_vote_recovery_exhausted(
                proposal_height,
                lock.view,
                proposal_view,
                lock.total_validators,
                now,
            ) || self.same_height_vote_recovery_escalation_view_gap_exhausted(
                lock.view,
                proposal_view,
                lock.total_validators,
            ))
    }

    pub(super) fn local_same_height_vote_has_live_proposal_material(
        &self,
        proposal_height: u64,
        block_hash: HashOf<BlockHeader>,
    ) -> bool {
        self.pending
            .pending_blocks
            .get(&block_hash)
            .is_some_and(|pending| {
                pending.height == proposal_height
                    && !pending.aborted
                    && !pending.is_retired_same_height()
                    && pending.validation_status != ValidationStatus::Invalid
            })
            || self
                .subsystems
                .commit
                .inflight
                .as_ref()
                .is_some_and(|inflight| {
                    inflight.block_hash == block_hash
                        && inflight.pending.height == proposal_height
                        && !inflight.pending.aborted
                })
            || self
                .pending
                .pending_processing
                .get()
                .is_some_and(|pending| pending == block_hash)
            || self
                .kura
                .get_block_height_by_hash(block_hash)
                .is_some_and(|height| {
                    u64::try_from(height.get()).is_ok_and(|height| height == proposal_height)
                })
    }

    pub(super) fn local_same_height_vote_blocks_fresh_proposal(
        &self,
        proposal_height: u64,
        proposal_view: u64,
        existing_vote: &crate::sumeragi::consensus::Vote,
        now: Instant,
        block_active_tip_owner: bool,
    ) -> bool {
        if existing_vote.view > proposal_view {
            return true;
        }
        if existing_vote.view == proposal_view {
            return self.local_same_height_vote_has_hard_lock(proposal_height, existing_vote)
                || self.local_same_height_vote_has_live_proposal_material(
                    proposal_height,
                    existing_vote.block_hash,
                );
        }
        if !self.config.resilience.enabled {
            return true;
        }
        if self.local_same_height_vote_has_hard_lock(proposal_height, existing_vote) {
            return true;
        }
        let min_stale_age = self
            .quorum_timeout(self.runtime_da_enabled())
            .max(self.frontier_slot_lag_window())
            .max(Duration::from_millis(1));
        let recovery_exhausted = self.same_height_stale_vote_recovery_exhausted(
            proposal_height,
            existing_vote.view,
            proposal_view,
            self.effective_commit_topology().len(),
            now,
        );
        if let Some(pending) = self
            .pending
            .pending_blocks
            .get(&existing_vote.block_hash)
            .filter(|pending| {
                pending.height == proposal_height
                    && pending.view == existing_vote.view
                    && !pending.aborted
                    && pending.validation_status != ValidationStatus::Invalid
            })
        {
            let tip_height = self.state.committed_height();
            let tip_hash = self.state.latest_block_hash_fast();
            let pending_age = pending
                .progress_age(now)
                .max(now.saturating_duration_since(pending.inserted_at));
            let active_tip_owner = block_active_tip_owner
                && self.pending_block_is_active_for_tip(
                    existing_vote.block_hash,
                    pending,
                    tip_height,
                    tip_hash,
                );
            // Keep old-view active-owner protection through the hard stale window, but do
            // not let a no-QC branch anchor fresh proposal assembly forever.
            let stale_age = if active_tip_owner {
                min_stale_age.saturating_mul(3)
            } else {
                min_stale_age
            };
            if pending_age < stale_age && !recovery_exhausted {
                return true;
            }
        }
        if matches!(
            existing_vote.phase,
            crate::sumeragi::consensus::Phase::Commit
        ) && !recovery_exhausted
        {
            return true;
        }
        if self.frontier_slot.as_ref().is_some_and(|slot| {
            slot.height == proposal_height
                && slot.view == existing_vote.view
                && slot.block_hash == existing_vote.block_hash
                && (slot.quorum_progress.commit_qc_observed
                    || (self.frontier_slot_competing_quorum_locked_for_view(slot, proposal_view)
                        && !recovery_exhausted))
        }) {
            return true;
        }
        false
    }

    pub(super) fn local_same_height_vote_blocks_fresh_proposal_assembly(
        &self,
        proposal_height: u64,
        proposal_view: u64,
        existing_vote: &crate::sumeragi::consensus::Vote,
        now: Instant,
        block_active_tip_owner: bool,
        highest_qc: crate::sumeragi::consensus::QcHeaderRef,
    ) -> bool {
        if !self.local_same_height_vote_blocks_fresh_proposal(
            proposal_height,
            proposal_view,
            existing_vote,
            now,
            block_active_tip_owner,
        ) {
            return false;
        }
        !self.stale_local_commit_vote_allows_proposal_assembly_after_missing_qc_repair(
            proposal_height,
            proposal_view,
            existing_vote,
            now,
            highest_qc,
        )
    }

    fn stale_local_commit_vote_allows_proposal_assembly_after_missing_qc_repair(
        &self,
        proposal_height: u64,
        proposal_view: u64,
        existing_vote: &crate::sumeragi::consensus::Vote,
        now: Instant,
        highest_qc: crate::sumeragi::consensus::QcHeaderRef,
    ) -> bool {
        if !self.config.resilience.enabled
            || !matches!(
                existing_vote.phase,
                crate::sumeragi::consensus::Phase::Commit
            )
            || proposal_view <= existing_vote.view
            || proposal_height != self.committed_height_snapshot().saturating_add(1)
            || highest_qc.height.saturating_add(1) != proposal_height
            || !self.highest_qc_is_canonical_committed_tip(highest_qc)
        {
            return false;
        }
        if self.local_same_height_vote_is_committed_parent_marker(
            proposal_height,
            proposal_view,
            existing_vote,
        ) {
            return true;
        }
        if self.local_same_height_vote_has_consensus_lock(proposal_height, existing_vote)
            || self.same_height_block_has_observed_qc(
                existing_vote.block_hash,
                proposal_height,
                existing_vote.view,
            )
            || self.same_height_has_recoverable_qc(proposal_height)
            || self
                .same_height_vote_lock_blocking_candidate(proposal_height, proposal_view, None)
                .is_some_and(|lock| {
                    !self.stale_same_height_vote_lock_allows_proposal_rotation(
                        proposal_height,
                        proposal_view,
                        &lock,
                        now,
                        highest_qc,
                    )
                })
            || self
                .subsystems
                .commit
                .inflight
                .as_ref()
                .is_some_and(|inflight| {
                    inflight.block_hash == existing_vote.block_hash
                        && inflight.pending.height == proposal_height
                        && inflight.pending.view == existing_vote.view
                        && !inflight.pending.aborted
                        && inflight.pending.validation_status != ValidationStatus::Invalid
                })
        {
            return false;
        }

        let commit_qc_repair_window = self
            .known_block_commit_qc_recovery_view_change_window()
            .max(self.quorum_timeout(self.runtime_da_enabled()))
            .max(Duration::from_millis(1));
        let hard_stale_age = self
            .quorum_timeout(self.runtime_da_enabled())
            .max(self.frontier_slot_lag_window())
            .max(Duration::from_millis(1))
            .saturating_mul(3);
        let recovery_exhausted = self
            .stale_same_height_recovery_age(proposal_height, existing_vote.view, now)
            .is_some_and(|age| age >= hard_stale_age)
            || self.same_height_vote_recovery_view_gap_exhausted(
                existing_vote.view,
                proposal_view,
                self.effective_commit_topology().len(),
            );
        let missing_qc_liveness_active =
            self.frontier_missing_qc_liveness_active(proposal_height, proposal_view);
        let repair_window = if missing_qc_liveness_active {
            super::reschedule::near_quorum_payload_timeout(self.rebroadcast_cooldown())
                .min(commit_qc_repair_window)
                .max(Duration::from_millis(1))
        } else {
            commit_qc_repair_window
        };
        let stale_branch_terminal = self
            .pending
            .pending_blocks
            .get(&existing_vote.block_hash)
            .filter(|pending| {
                pending.height == proposal_height && pending.view == existing_vote.view
            })
            .is_some_and(|pending| {
                pending.is_retired_same_height()
                    || pending.is_retry_aborted()
                    || pending.validation_status == ValidationStatus::Invalid
            });
        let stale_branch_absent_after_recovery_exhausted = recovery_exhausted
            && !self.local_same_height_vote_has_live_proposal_material(
                proposal_height,
                existing_vote.block_hash,
            );
        let stale_pending_repair_window_elapsed = self
            .pending
            .pending_blocks
            .get(&existing_vote.block_hash)
            .filter(|pending| {
                pending.height == proposal_height && pending.view == existing_vote.view
            })
            .is_some_and(|pending| {
                !pending.commit_qc_observed()
                    && pending
                        .progress_age(now)
                        .max(now.saturating_duration_since(pending.inserted_at))
                        >= repair_window
            });
        let pending_allows_stale_branch_rotation = self
            .pending
            .pending_blocks
            .get(&existing_vote.block_hash)
            .filter(|pending| {
                pending.height == proposal_height && pending.view == existing_vote.view
            })
            .is_none_or(|pending| {
                pending.is_retired_same_height()
                    || pending.is_retry_aborted()
                    || pending.validation_status == ValidationStatus::Invalid
                    || (!pending.commit_qc_observed()
                        && pending
                            .progress_age(now)
                            .max(now.saturating_duration_since(pending.inserted_at))
                            >= repair_window)
            });
        (stale_branch_terminal
            || stale_branch_absent_after_recovery_exhausted
            || missing_qc_liveness_active
            || stale_pending_repair_window_elapsed)
            && pending_allows_stale_branch_rotation
    }

    fn stale_local_commit_vote_allows_frontier_owner_clear_for_proposal_assembly(
        &self,
        proposal_height: u64,
        proposal_view: u64,
        owner_hash: HashOf<BlockHeader>,
        owner_view: u64,
        now: Instant,
        highest_qc: crate::sumeragi::consensus::QcHeaderRef,
    ) -> bool {
        self.local_same_height_vote(proposal_height, self.epoch_for_height(proposal_height))
            .as_ref()
            .is_some_and(|existing_vote| {
                existing_vote.block_hash == owner_hash
                    && existing_vote.view == owner_view
                    && self
                        .stale_local_commit_vote_allows_proposal_assembly_after_missing_qc_repair(
                            proposal_height,
                            proposal_view,
                            existing_vote,
                            now,
                            highest_qc,
                        )
            })
    }

    pub(super) fn local_same_height_vote_is_committed_parent_marker(
        &self,
        proposal_height: u64,
        proposal_view: u64,
        existing_vote: &crate::sumeragi::consensus::Vote,
    ) -> bool {
        existing_vote.view == proposal_view
            && proposal_height > 0
            && self.committed_block_hash_for_height(proposal_height.saturating_sub(1))
                == Some(existing_vote.block_hash)
    }

    fn proposal_has_exact_primary_block_owner(
        &self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
    ) -> bool {
        self.pending
            .pending_blocks
            .get(&block_hash)
            .is_some_and(|pending| {
                !pending.aborted
                    && !pending.is_retry_aborted()
                    && !pending.is_retired_same_height()
                    && !matches!(pending.validation_status, ValidationStatus::Invalid)
                    && pending.height == height
                    && pending.view == view
                    && pending.block.hash() == block_hash
            })
            || self
                .subsystems
                .commit
                .inflight
                .as_ref()
                .is_some_and(|inflight| {
                    inflight.block_hash == block_hash
                        && !inflight.pending.aborted
                        && !inflight.pending.is_retry_aborted()
                        && !inflight.pending.is_retired_same_height()
                        && !matches!(
                            inflight.pending.validation_status,
                            ValidationStatus::Invalid
                        )
                        && inflight.pending.height == height
                        && inflight.pending.view == view
                })
            || self
                .kura
                .get_block_height_by_hash(block_hash)
                .and_then(|block_height| self.kura.get_block(block_height))
                .is_some_and(|block| {
                    block.hash() == block_hash
                        && block.header().height().get() == height
                        && block.header().view_change_index() == view
                })
    }

    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(super) fn assemble_and_broadcast_proposal(
        &mut self,
        height: u64,
        view: u64,
        highest_qc: crate::sumeragi::consensus::QcHeaderRef,
        topology: &mut super::network_topology::Topology,
        leader_index: usize,
        local_validator_index: u32,
        view_snapshot: Option<StateView<'_>>,
        now: Instant,
    ) -> Result<bool> {
        self.assemble_and_broadcast_proposal_with_recovery_heartbeat(
            height,
            view,
            highest_qc,
            topology,
            leader_index,
            local_validator_index,
            view_snapshot,
            now,
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_lines)]
    fn assemble_and_broadcast_proposal_with_recovery_heartbeat(
        &mut self,
        height: u64,
        view: u64,
        mut highest_qc: crate::sumeragi::consensus::QcHeaderRef,
        topology: &mut super::network_topology::Topology,
        leader_index: usize,
        local_validator_index: u32,
        view_snapshot: Option<StateView<'_>>,
        now: Instant,
        allow_recovery_heartbeat: bool,
    ) -> Result<bool> {
        if !self.retry_quarantined_proposal_guards() {
            return Ok(false);
        }
        let _ = self.retry_pending_block_requeues(now, 4);
        if self.is_observer() {
            return Ok(false);
        }
        if view == u64::MAX {
            warn!(
                height,
                view, "skipping proposal assembly: view-change index overflow"
            );
            return Ok(false);
        }
        if self.runtime_da_enabled() {
            match self.state.da_indexes_hydration_result_cached() {
                Some(Ok(())) => {}
                Some(Err(err)) => {
                    return Err(eyre!(
                        "cannot assemble DA proposal because canonical DA index hydration failed: {err}"
                    ));
                }
                None => {
                    return Err(eyre!(
                        "cannot assemble DA proposal before canonical DA indexes are hydrated"
                    ));
                }
            }
        }
        super::status::set_leader_index(leader_index as u64);
        let required_for_commit = topology.min_votes_for_commit();
        debug!(
            height,
            view,
            topology_size = topology.as_ref().len(),
            required_for_commit,
            "proposal topology snapshot"
        );
        let proposal_height = height;
        let proposal_epoch = self.epoch_for_height(proposal_height);
        let committed_height = u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
        self.prune_highest_qc_missing_defer_markers(committed_height);
        self.init_collector_plan(topology, proposal_height, view);
        if let Some(lock) =
            self.same_height_vote_lock_blocking_candidate(proposal_height, view, None)
        {
            if self.new_view_qc_supersedes_same_height_vote_lock(
                proposal_height,
                view,
                highest_qc,
                &lock,
            ) {
                info!(
                    height = proposal_height,
                    view,
                    epoch = proposal_epoch,
                    locked_block = %lock.block_hash,
                    locked_view = lock.view,
                    locked_votes = lock.vote_count,
                    conflicting_voters = lock.conflicting_voters,
                    candidate_possible_votes = lock.candidate_possible_votes,
                    required = lock.required,
                    total_validators = lock.total_validators,
                    "allowing proposal assembly: NEW_VIEW QC supersedes raw same-height vote lock"
                );
            } else {
                let min_stale_age = self
                    .quorum_timeout(self.runtime_da_enabled())
                    .max(self.frontier_slot_lag_window())
                    .max(Duration::from_millis(1));
                let hard_stale_age = min_stale_age.saturating_mul(3);
                let recovery_exhausted = self
                    .stale_same_height_recovery_age(proposal_height, lock.view, now)
                    .is_some_and(|age| age >= hard_stale_age)
                    || self.same_height_vote_recovery_view_gap_exhausted(
                        lock.view,
                        view,
                        lock.total_validators,
                    );
                let recovery_escalation_due = recovery_exhausted
                    || self.same_height_vote_recovery_escalation_view_gap_exhausted(
                        lock.view,
                        view,
                        lock.total_validators,
                    );
                let qc_observed = self.same_height_block_has_observed_qc(
                    lock.block_hash,
                    proposal_height,
                    lock.view,
                );
                let stale_vote_lock_can_rotate = self
                    .stale_same_height_vote_lock_allows_proposal_rotation(
                        proposal_height,
                        view,
                        &lock,
                        now,
                        highest_qc,
                    );
                if stale_vote_lock_can_rotate {
                    info!(
                        height = proposal_height,
                        view,
                        epoch = proposal_epoch,
                        locked_block = %lock.block_hash,
                        locked_view = lock.view,
                        locked_votes = lock.vote_count,
                        conflicting_voters = lock.conflicting_voters,
                        candidate_possible_votes = lock.candidate_possible_votes,
                        required = lock.required,
                        total_validators = lock.total_validators,
                        recovery_exhausted,
                        qc_observed,
                        highest_qc_height = highest_qc.height,
                        highest_qc_view = highest_qc.view,
                        highest_qc_block = %highest_qc.subject_block_hash,
                        "allowing proposal assembly: stale same-height vote lock has no recoverable branch"
                    );
                } else {
                    let _ = self.seed_frontier_slot_from_same_height_evidence(
                        proposal_height,
                        view,
                        now,
                        "vote_locked_same_height",
                        false,
                    );
                    let highest_qc_dependency_repair_requested = if recovery_escalation_due
                        && highest_qc.height.saturating_add(1) == proposal_height
                        && !self.block_known_locally(highest_qc.subject_block_hash)
                    {
                        let _ = self.mark_highest_qc_missing_defer_for_round(
                            proposal_height,
                            view,
                            highest_qc,
                        );
                        self.observe_new_view_highest_qc_exact_repair(highest_qc)
                    } else {
                        false
                    };
                    let stale_vote_locked_recovery_requested = recovery_escalation_due
                        && !qc_observed
                        && self.escalate_stale_vote_locked_frontier_owner_recovery(
                            lock.block_hash,
                            proposal_height,
                            lock.view,
                            now,
                            "exhausted_vote_locked_same_height",
                        );
                    let all_validator_vote_lock_new_view_requested = lock.candidate_possible_votes
                        == 0
                        && !qc_observed
                        && proposal_height == self.committed_height_snapshot().saturating_add(1);
                    if all_validator_vote_lock_new_view_requested {
                        self.maybe_rebroadcast_new_view_votes(proposal_height, now);
                        self.trigger_view_change_with_cause(
                            proposal_height,
                            view,
                            ViewChangeCause::MissingQc,
                        );
                    }
                    warn!(
                        height = proposal_height,
                        view,
                        epoch = proposal_epoch,
                        locked_block = %lock.block_hash,
                        locked_view = lock.view,
                        locked_votes = lock.vote_count,
                        conflicting_voters = lock.conflicting_voters,
                        candidate_possible_votes = lock.candidate_possible_votes,
                        required = lock.required,
                        total_validators = lock.total_validators,
                        recovery_exhausted,
                        recovery_escalation_due,
                        qc_observed,
                        highest_qc_dependency_repair_requested,
                        highest_qc_height = highest_qc.height,
                        highest_qc_view = highest_qc.view,
                        highest_qc_block = %highest_qc.subject_block_hash,
                        stale_vote_locked_recovery_requested,
                        all_validator_vote_lock_new_view_requested,
                        "deferring proposal assembly: same-height vote history makes a fresh branch non-viable"
                    );
                    return Ok(false);
                }
            }
        }
        if let Some(existing_vote) = self.local_same_height_vote(proposal_height, proposal_epoch) {
            let new_view_qc_supersedes = self.new_view_qc_supersedes_same_height_vote_conflict(
                proposal_height,
                view,
                highest_qc,
                existing_vote.block_hash,
                existing_vote.view,
            );
            if new_view_qc_supersedes
                || !self.local_same_height_vote_blocks_fresh_proposal_assembly(
                    proposal_height,
                    view,
                    &existing_vote,
                    now,
                    true,
                    highest_qc,
                )
            {
                debug!(
                    height = proposal_height,
                    view,
                    epoch = proposal_epoch,
                    voted_view = existing_vote.view,
                    voted_phase = ?existing_vote.phase,
                    voted_block = %existing_vote.block_hash,
                    new_view_qc_supersedes,
                    "allowing fresh proposal after stale prior-view local same-height vote"
                );
            } else {
                warn!(
                    height = proposal_height,
                    view,
                    epoch = proposal_epoch,
                    voted_view = existing_vote.view,
                    voted_phase = ?existing_vote.phase,
                    voted_block = %existing_vote.block_hash,
                    "deferring proposal assembly: local same-height vote already anchors another branch"
                );
                return Ok(false);
            }
        }
        if self.same_height_vote_verification_pending_at_or_before_view(
            proposal_height,
            view,
            proposal_epoch,
        ) {
            debug!(
                height = proposal_height,
                view,
                epoch = proposal_epoch,
                "deferring proposal assembly: same-height vote verification is pending"
            );
            return Ok(false);
        }
        if let Err(LockedQcRejection::HeightRegressed { locked, highest }) =
            ensure_locked_qc_allows(self.locked_qc, highest_qc)
        {
            let Some(lock) = self.promote_locked_qc_to_highest_if_needed("proposal_assembly")
            else {
                return Ok(false);
            };
            info!(
                locked_height = locked,
                highest_height = highest,
                height = proposal_height,
                view,
                lock_hash = %lock.subject_block_hash,
                "replacing regressed highest QC with locked QC for direct proposal assembly"
            );
            highest_qc = lock;
        }
        let _lock_lag_highest_qc_deferred = !self.highest_qc_extends_locked(highest_qc)
            && self.defer_highest_qc_update_for_lock_catchup(
                height, view, highest_qc, now, "proposal",
            );
        let parent_height = proposal_height.saturating_sub(1);
        let hash_only_parent_hash =
            if proposal_height > 1 && highest_qc.height.saturating_add(1) == proposal_height {
                usize::try_from(parent_height)
                    .ok()
                    .and_then(NonZeroUsize::new)
                    .and_then(|height| {
                        self.kura
                            .block_hash_at_height(height)
                            .or_else(|| self.kura.get_durable_block_hash(height))
                    })
                    .filter(|hash| *hash == highest_qc.subject_block_hash)
            } else {
                None
            };
        if proposal_height > 1
            && !self.block_known_locally(highest_qc.subject_block_hash)
            && hash_only_parent_hash.is_none()
        {
            if self.mark_highest_qc_missing_defer_for_round(proposal_height, view, highest_qc) {
                self.observe_new_view_highest_qc_exact_repair(highest_qc);
            }
            if let Some(suppressed_since_last) = self.proposal_defer_warning_log.allow(
                ProposalDeferWarningKind::HighestQcMissing,
                proposal_height,
                view,
                highest_qc.subject_block_hash,
                now,
                Duration::from_secs(5),
            ) {
                warn!(
                    height = proposal_height,
                    view,
                    highest_height = highest_qc.height,
                    highest_hash = %highest_qc.subject_block_hash,
                    suppressed_since_last,
                    "deferring proposal assembly: highest QC block not available locally"
                );
            }
            return Ok(false);
        }
        let prev_block = resolve_prev_block_for_proposal(
            proposal_height,
            &highest_qc,
            &self.kura,
            &self.pending.pending_blocks,
        );
        if prev_block.is_none() && proposal_height > 1 && hash_only_parent_hash.is_none() {
            if !self.block_known_locally(highest_qc.subject_block_hash) {
                if self.mark_highest_qc_missing_defer_for_round(proposal_height, view, highest_qc) {
                    self.observe_new_view_highest_qc_exact_repair(highest_qc);
                }
            }
            if let Some(suppressed_since_last) = self.proposal_defer_warning_log.allow(
                ProposalDeferWarningKind::ParentMissing,
                proposal_height,
                view,
                highest_qc.subject_block_hash,
                now,
                Duration::from_secs(5),
            ) {
                warn!(
                    height = proposal_height,
                    view,
                    parent_height,
                    highest_height = highest_qc.height,
                    highest_hash = %highest_qc.subject_block_hash,
                    suppressed_since_last,
                    "deferring proposal assembly: parent block not available locally"
                );
            }
            return Ok(false);
        }
        let mut pending_certified_merge_entry = self.pending_certified_merge_entry_for_proposal(
            proposal_height,
            view,
            prev_block.as_deref(),
        );

        let preflight_elapsed_ms = now.elapsed().as_millis();
        let queue_len = self.queue.queued_len();
        let mut tx_guards =
            ProposalTransactionGuards::new(Arc::clone(&self.queue), Arc::clone(&self.state));
        let tx_select_started_at = Instant::now();
        let (
            _block_digest,
            conf_features,
            mut transactions,
            mut routing_decisions,
            mut routing_plans,
            mut tx_sizes,
            deferred_transactions,
        ) = {
            let (block_max_param, commit_time_ms, effective_commit_time_ms, base_gas_limit, digest) =
                if let Some(state_view) = view_snapshot.as_ref() {
                    let block_max_param =
                        state_view.world().parameters().block().max_transactions();
                    let sumeragi_params = state_view.world().parameters().sumeragi();
                    let commit_time_ms = sumeragi_params.commit_time_ms();
                    let effective_commit_time_ms = sumeragi_params.effective_commit_time_ms();
                    let base_gas_limit = NonZeroU64::new(crate::state::gas_limit_from_parameters(
                        state_view.world().parameters(),
                    ));
                    let committed_height = u64::try_from(state_view.height())
                        .expect("committed height exceeds u64::MAX");
                    let next_height = committed_height
                        .checked_add(1)
                        .expect("block height exceeds u64::MAX");
                    let digest = self.state.cached_confidential_feature_digest(
                        state_view.world(),
                        &state_view.zk,
                        next_height,
                    );
                    (
                        block_max_param,
                        commit_time_ms,
                        effective_commit_time_ms,
                        base_gas_limit,
                        digest,
                    )
                } else {
                    let world = self.state.world_view();
                    let block_max_param = world.parameters().block().max_transactions();
                    let sumeragi_params = world.parameters().sumeragi();
                    let commit_time_ms = sumeragi_params.commit_time_ms();
                    let effective_commit_time_ms = sumeragi_params.effective_commit_time_ms();
                    let base_gas_limit = NonZeroU64::new(crate::state::gas_limit_from_parameters(
                        world.parameters(),
                    ));
                    let committed_height = u64::try_from(self.state.committed_height())
                        .expect("committed height exceeds u64::MAX");
                    let next_height = committed_height
                        .checked_add(1)
                        .expect("block height exceeds u64::MAX");
                    let zk = self.state.zk_snapshot();
                    let digest =
                        self.state
                            .cached_confidential_feature_digest(&world, &zk, next_height);
                    (
                        block_max_param,
                        commit_time_ms,
                        effective_commit_time_ms,
                        base_gas_limit,
                        digest,
                    )
                };
            let (max_tx_target, max_in_block, fast_tx_capped) = Self::max_tx_budget_for_commit_time(
                queue_len,
                block_max_param.get(),
                self.config.block.max_transactions,
                self.config.block.fast_finality_max_transactions,
                commit_time_ms,
                effective_commit_time_ms,
            );
            let fast_gas_limit_per_block = self.config.block.fast_gas_limit_per_block;
            let proposal_gas_limit = Self::cap_gas_limit_for_fast_commit(
                base_gas_limit,
                commit_time_ms,
                effective_commit_time_ms,
                fast_gas_limit_per_block,
            );
            let fast_gas_capped = proposal_gas_limit != base_gas_limit;
            let scan_budget = self.proposal_scan_budget(max_in_block);
            let max_ivm_transactions = self.config.block.max_ivm_transactions;
            let replay_ivm_proved = {
                let state_view = self.state.view();
                let pipeline = state_view.pipeline();
                pipeline.ivm_proved.enabled && !pipeline.ivm_proved.skip_replay
            };
            info!(
                height,
                view,
                queue_len,
                max_tx_param = block_max_param.get(),
                max_tx_target,
                max_in_block = max_in_block.get(),
                max_ivm_transactions = max_ivm_transactions.map(NonZeroUsize::get),
                scan_budget,
                scan_multiplier = self.config.block.proposal_queue_scan_multiplier.get(),
                commit_time_ms,
                effective_commit_time_ms,
                gas_limit_per_block = base_gas_limit.map(NonZeroU64::get),
                fast_gas_limit_per_block = fast_gas_limit_per_block.map(NonZeroU64::get),
                proposal_gas_limit = proposal_gas_limit.map(NonZeroU64::get),
                fast_gas_capped,
                fast_tx_capped,
                "proposal assembly budget"
            );
            // Bound queue scanning to keep proposal assembly from stalling under sustained load.
            let deferred_accumulator = self.pull_transactions_for_proposal(
                self.state.as_ref(),
                max_in_block,
                scan_budget,
                proposal_gas_limit,
                max_ivm_transactions,
                replay_ivm_proved,
                &mut tx_guards,
                height,
                view,
            );
            let transactions: Vec<AcceptedTransaction<'static>> = tx_guards
                .iter()
                .map(crate::queue::TransactionGuard::clone_accepted)
                .collect();
            let routing_decisions: Vec<RoutingDecision> = tx_guards
                .iter()
                .map(crate::queue::TransactionGuard::routing)
                .collect();
            let routing_plans: Vec<crate::queue::RoutingPlan> = tx_guards
                .iter()
                .map(crate::queue::TransactionGuard::routing_plan)
                .collect();
            let tx_sizes: Vec<usize> = tx_guards
                .iter()
                .map(crate::queue::TransactionGuard::encoded_len)
                .collect();
            let conf_features = if digest.is_empty() {
                None
            } else {
                Some(digest)
            };
            (
                digest,
                conf_features,
                transactions,
                routing_decisions,
                routing_plans,
                tx_sizes,
                deferred_accumulator,
            )
        };
        let tx_select_ms = tx_select_started_at.elapsed().as_millis();

        let tx_prepare_started_at = Instant::now();
        if let Err(err) = Self::filter_committed_transactions_for_proposal(
            self.state.as_ref(),
            &mut tx_guards,
            &mut transactions,
            &mut routing_decisions,
            &mut routing_plans,
            &mut tx_sizes,
            height,
            view,
        ) {
            let _ = self.return_proposal_guards_or_quarantine(
                &mut tx_guards,
                "proposal committed-filter vector mismatch",
            );
            return Err(err);
        }

        if transactions.len() > 1 {
            // Lane interleaving is a budget-selection policy only. The default block builder
            // still canonicalizes normal-lane payload order by entrypoint hash for consensus.
            let order = interleave_lane_indices_for_slot(&routing_decisions, height, view);

            if order.iter().enumerate().any(|(idx, &value)| idx != value) {
                reorder_vec_by_indices(&mut transactions, &order);
                reorder_vec_by_indices(&mut routing_decisions, &order);
                reorder_vec_by_indices(&mut routing_plans, &order);
                reorder_vec_by_indices(&mut tx_sizes, &order);
            }
        }

        let mut deferred_transactions = deferred_transactions;
        if !self.return_proposal_guards_or_quarantine(
            &mut deferred_transactions,
            "proposal TEU or scheduler deferral",
        ) {
            self.quarantine_proposal_guards_without_return(
                &mut tx_guards,
                "selected proposal guards held behind TEU/scheduler return failure",
            );
            return Ok(false);
        }

        let queue_len_after_pop = self.queue.queued_len();
        let mut internal_work = if transactions.is_empty() {
            if allow_recovery_heartbeat {
                let heartbeat = match self.build_recovery_heartbeat_transaction(proposal_height) {
                    Ok(heartbeat) => heartbeat,
                    Err(err) => {
                        let _ = self.return_proposal_guards_or_quarantine(
                            &mut tx_guards,
                            "recovery-heartbeat construction failure",
                        );
                        return Err(err);
                    }
                };
                let encoded_len = heartbeat.encoded_len();
                transactions.push(heartbeat);
                routing_decisions.push(RoutingDecision::default());
                routing_plans.push(crate::queue::RoutingPlan::single(RoutingDecision::default()));
                tx_sizes.push(encoded_len);
                info!(
                    height = proposal_height,
                    view,
                    queue_len = queue_len_after_pop,
                    "injecting recovery heartbeat transaction for empty leader queue"
                );
                None
            } else {
                let work = self.internal_proposal_work(
                    proposal_height,
                    prev_block.as_deref(),
                    pending_certified_merge_entry.is_some(),
                );
                if !work.has_work() {
                    let _ = self.return_proposal_guards_or_quarantine(
                        &mut tx_guards,
                        "empty proposal after committed-transaction filtering",
                    );
                    info!(
                        height,
                        view,
                        queue_len = queue_len_after_pop,
                        "skipping empty proposal; empty blocks are disallowed"
                    );
                    return Ok(false);
                }
                if work.autoscale_maintenance && !work.has_non_autoscale_work() {
                    let heartbeat = self.build_recovery_heartbeat_transaction(proposal_height)?;
                    let encoded_len = heartbeat.encoded_len();
                    transactions.push(heartbeat);
                    routing_decisions.push(RoutingDecision::default());
                    routing_plans
                        .push(crate::queue::RoutingPlan::single(RoutingDecision::default()));
                    tx_sizes.push(encoded_len);
                    info!(
                        height = proposal_height,
                        view,
                        queue_len = queue_len_after_pop,
                        "injecting view-0 autoscale maintenance heartbeat until elastic capacity reaches its floor"
                    );
                    None
                } else {
                    Some(work)
                }
            }
        } else {
            None
        };

        let da_enabled = self.runtime_da_enabled();
        let mut overflow_transactions: Vec<(
            AcceptedTransaction<'static>,
            crate::queue::RoutingPlan,
        )> = Vec::new();
        let mut oversized_frame_len: Option<usize> = None;
        let tx_sizes_in = tx_sizes;
        let routing_plans_in = routing_plans;
        let mut tx_batch;
        let mut routing_batch;
        let mut routing_plan_batch;
        let mut tx_sizes;
        if da_enabled {
            let mut remaining_budget = da_payload_budget(
                self.config.rbc.chunk_max_bytes,
                self.config.rbc.pending_max_bytes,
                self.config.rbc.pending_max_chunks,
                self.config.block.max_payload_bytes,
            );
            tx_batch = Vec::with_capacity(transactions.len());
            routing_batch = Vec::with_capacity(routing_decisions.len());
            routing_plan_batch = Vec::with_capacity(routing_plans_in.len());
            let mut tx_sizes_out = Vec::with_capacity(transactions.len());
            for (((tx, routing), routing_plan), encoded_len) in transactions
                .into_iter()
                .zip(routing_decisions.into_iter())
                .zip(routing_plans_in.into_iter())
                .zip(tx_sizes_in.into_iter())
            {
                if encoded_len > self.consensus_payload_frame_cap {
                    oversized_frame_len =
                        Some(oversized_frame_len.map_or(encoded_len, |prev| prev.max(encoded_len)));
                }
                if encoded_len > remaining_budget {
                    overflow_transactions.push((tx, routing_plan));
                    continue;
                }
                remaining_budget = remaining_budget.saturating_sub(encoded_len);
                tx_sizes_out.push(encoded_len);
                tx_batch.push(tx);
                routing_batch.push(routing);
                routing_plan_batch.push(routing_plan);
            }
            tx_sizes = tx_sizes_out;
        } else {
            let mut payload_budget = Some(non_rbc_payload_budget(
                self.config.block.max_payload_bytes,
                self.consensus_payload_frame_cap,
            ));
            tx_batch = Vec::with_capacity(transactions.len());
            routing_batch = Vec::with_capacity(routing_decisions.len());
            routing_plan_batch = Vec::with_capacity(routing_plans_in.len());
            let mut tx_sizes_out = Vec::with_capacity(transactions.len());
            for (((tx, routing), routing_plan), encoded_len) in transactions
                .into_iter()
                .zip(routing_decisions.into_iter())
                .zip(routing_plans_in.into_iter())
                .zip(tx_sizes_in.into_iter())
            {
                if let Some(budget) = payload_budget {
                    if encoded_len > budget {
                        overflow_transactions.push((tx, routing_plan));
                        continue;
                    }
                    payload_budget = Some(budget.saturating_sub(encoded_len));
                    tx_sizes_out.push(encoded_len);
                }
                tx_batch.push(tx);
                routing_batch.push(routing);
                routing_plan_batch.push(routing_plan);
            }
            tx_sizes = tx_sizes_out;
        }

        if tx_batch.len() > 1 {
            if let Some(admin_idx) = tx_batch.iter().position(Self::is_peer_admin_transaction) {
                let admin_tx = tx_batch.remove(admin_idx);
                let admin_route = routing_batch.remove(admin_idx);
                let admin_plan = routing_plan_batch.remove(admin_idx);
                let admin_size = tx_sizes.remove(admin_idx);
                overflow_transactions.extend(tx_batch.drain(..).zip(routing_plan_batch.drain(..)));
                routing_batch.clear();
                tx_sizes.clear();
                tx_batch.push(admin_tx);
                routing_batch.push(admin_route);
                routing_plan_batch.push(admin_plan);
                tx_sizes.push(admin_size);
            }
        }
        canonicalize_proposal_batch_with_plans(
            &mut tx_batch,
            &mut routing_batch,
            &mut routing_plan_batch,
            &mut tx_sizes,
        );

        if let Some(entry) = pending_certified_merge_entry.as_ref()
            && let Some(batch) = entry.execution_batch.as_ref()
        {
            let merge_entrypoints = batch
                .lanes
                .iter()
                .flat_map(|execution| execution.entrypoint_hashes.iter().copied())
                .collect::<BTreeSet<_>>();
            let application_time = batch.application_block_header.creation_time();
            let mut index = 0;
            while index < tx_batch.len() {
                let entrypoint_hash = Hash::from(tx_batch[index].hash_as_entrypoint());
                if merge_entrypoints.contains(&entrypoint_hash)
                    || tx_batch[index].creation_time() >= application_time
                {
                    let transaction = tx_batch.remove(index);
                    let _routing = routing_batch.remove(index);
                    let plan = routing_plan_batch.remove(index);
                    let _size = tx_sizes.remove(index);
                    overflow_transactions.push((transaction, plan));
                } else {
                    index += 1;
                }
            }
        }

        if let Some(entry) = pending_certified_merge_entry.as_ref() {
            let merge_probe_builder = if let Some(parent) = prev_block.as_deref() {
                BlockBuilder::new(tx_batch.clone()).chain(view, Some(parent))
            } else if let Some(parent_hash) = hash_only_parent_hash {
                BlockBuilder::new(tx_batch.clone()).chain_with_parent_hash(
                    view,
                    parent_height,
                    parent_hash,
                )
            } else {
                BlockBuilder::new(tx_batch.clone()).chain(view, None)
            };
            let merge_probe_builder = if let Some(batch) = entry.execution_batch.as_ref() {
                merge_probe_builder
                    .bind_certified_merge_application_context(&batch.application_block_header)
                    .map_err(str::to_owned)
            } else {
                Ok(merge_probe_builder)
            };
            let stage_result = merge_probe_builder.and_then(|merge_probe_builder| {
                self.state
                    .block_with_certified_merge_entry(
                        merge_probe_builder.carrier_context_header(),
                        entry,
                    )
                    .map(drop)
                    .map_err(|err| err.to_string())
            });
            if let Err(reason) = stage_result {
                warn!(
                    height = proposal_height,
                    view,
                    merge_epoch = entry.epoch_id,
                    reason,
                    "certified merge sidecar is not eligible for this proposal; continuing without it"
                );
                pending_certified_merge_entry = None;
                if let Some(work) = internal_work.as_mut() {
                    work.certified_merge = false;
                }
            }
        }
        let tx_prepare_ms = tx_prepare_started_at.elapsed().as_millis();

        let native_precheck_started_at = Instant::now();
        if let Err(reason) =
            self.native_amx_receipts_for_batch(&tx_batch, &routing_plan_batch, proposal_height)
        {
            let _ = self.return_proposal_guards_or_quarantine(
                &mut tx_guards,
                "native AMX participant-attestation deferral",
            );
            info!(
                height = proposal_height,
                view,
                reason,
                "deferring proposal while native AMX participant attestations are collected"
            );
            return Ok(false);
        }
        let native_precheck_ms = native_precheck_started_at.elapsed().as_millis();

        if tx_batch.is_empty() {
            if !self.return_proposal_guards_or_quarantine(
                &mut tx_guards,
                "no external transaction fits the proposal payload budget",
            ) {
                return Ok(false);
            }
            let has_internal_work = internal_work
                .get_or_insert_with(|| {
                    self.internal_proposal_work(
                        proposal_height,
                        prev_block.as_deref(),
                        pending_certified_merge_entry.is_some(),
                    )
                })
                .has_work();
            if !has_internal_work {
                if let Some(frame_len) = oversized_frame_len {
                    return Err(eyre!(
                        "proposal frame size {frame_len} exceeds consensus payload cap {}",
                        self.consensus_payload_frame_cap
                    ));
                }
                info!(
                    height = proposal_height,
                    view,
                    queue_len = queue_len_after_pop,
                    "deferring proposal: no transactions fit within payload budget"
                );
                return Ok(false);
            }
            debug!(
                height = proposal_height,
                view, "assembling proposal without external transactions"
            );
        }

        let previous_roster_started_at = Instant::now();
        let previous_roster_evidence = prev_block
            .as_deref()
            .and_then(|parent| {
                previous_roster_evidence_for_parent(
                    self.state.as_ref(),
                    self.kura.as_ref(),
                    self.consensus_context_for_height(parent.header().height().get())
                        .0,
                    parent,
                )
            })
            .or_else(|| {
                let parent_hash = hash_only_parent_hash?;
                let (consensus_mode, _, _) = self.consensus_context_for_height(parent_height);
                let roster = self.roster_for_live_vote_with_mode(parent_height, consensus_mode);
                previous_roster_evidence_for_hash_only_parent(
                    self.state.as_ref(),
                    consensus_mode,
                    parent_height,
                    parent_hash,
                    &roster,
                )
            });
        let previous_roster_ms = previous_roster_started_at.elapsed().as_millis();
        let mut removed_for_chunk_cap: Vec<(
            AcceptedTransaction<'static>,
            crate::queue::RoutingPlan,
        )> = Vec::new();
        let mut removed_for_frame_cap: Vec<(
            AcceptedTransaction<'static>,
            crate::queue::RoutingPlan,
        )> = Vec::new();
        let mut removed_for_lane_authority: Vec<(
            AcceptedTransaction<'static>,
            crate::queue::RoutingPlan,
        )> = Vec::new();
        let mut removed_for_lane_readiness: Vec<(
            AcceptedTransaction<'static>,
            crate::queue::RoutingPlan,
        )> = Vec::new();
        let mut lane_authority_deferred = false;
        let mut lane_readiness_deferred = false;
        let mut no_effective_work_deferred = false;
        let mut proposal_block_hash_for_cleanup = None;
        let mut proposal_exposed_to_remote = false;
        let mut exposed_proposal_hint: Option<super::message::ProposalHint> = None;
        let mut exposed_proposal: Option<crate::sumeragi::consensus::Proposal> = None;
        let mut exposed_payload_hash: Option<Hash> = None;
        let mut last_sidecar_ms = 0_u128;
        let mut last_block_build_ms = 0_u128;
        let mut last_payload_encode_ms = 0_u128;
        let mut last_frontier_wire_ms = 0_u128;
        let block_loop_started_at = Instant::now();
        let assembly_result: Result<()> = (|| {
            if tx_sizes.len() < tx_batch.len() {
                for tx in tx_batch.iter().skip(tx_sizes.len()) {
                    tx_sizes.push(tx.encoded_len());
                }
            }
            canonicalize_proposal_batch_with_plans(
                &mut tx_batch,
                &mut routing_batch,
                &mut routing_plan_batch,
                &mut tx_sizes,
            );
            let (
                signed_block,
                block_created_msg,
                payload_bytes,
                payload_hash,
                proposal,
                proposal_hint,
                block_hash,
                block_created_frame_len,
                final_lane_payload_plan,
            ) = loop {
                let sidecar_started_at = Instant::now();
                let mut da_stage = ProposalDaStage::default();
                let nexus = self.state.nexus_snapshot();
                let nexus_enabled = nexus.enabled;
                let lane_config = nexus.lane_config.clone();
                let mut builder = if let Some(parent) = prev_block.as_deref() {
                    BlockBuilder::new(tx_batch.clone()).chain(view, Some(parent))
                } else if let Some(parent_hash) = hash_only_parent_hash {
                    BlockBuilder::new(tx_batch.clone()).chain_with_parent_hash(
                        view,
                        parent_height,
                        parent_hash,
                    )
                } else {
                    BlockBuilder::new(tx_batch.clone()).chain(view, None)
                };
                if let Some(batch) = pending_certified_merge_entry
                    .as_ref()
                    .and_then(|entry| entry.execution_batch.as_ref())
                {
                    builder = builder
                        .bind_certified_merge_application_context(&batch.application_block_header)
                        .map_err(|reason| eyre!(reason))?;
                }
                let routing_ledger_time_ms =
                    u64::try_from(builder.creation_time().as_millis()).unwrap_or(u64::MAX);
                {
                    let state_view = self.state.view();
                    if refresh_proposal_routing_from_state(
                        &tx_batch,
                        &mut routing_batch,
                        &mut routing_plan_batch,
                        &state_view,
                        routing_ledger_time_ms,
                        proposal_height,
                    )? {
                        info!(
                            height = proposal_height,
                            view,
                            tx_count = tx_batch.len(),
                            "proposal routing refreshed from committed Nexus state before sidecar assembly"
                        );
                    }
                }
                if proposal_height > 2 && previous_roster_evidence.is_none() {
                    return Err(eyre!(
                        "missing previous-roster evidence for parent block at height {}",
                        proposal_height.saturating_sub(1),
                    ));
                }
                builder = builder.with_previous_roster_evidence(previous_roster_evidence.clone());
                let npos_effects =
                    self.build_npos_consensus_effects_for_proposal(proposal_height)?;
                builder = builder.with_npos_consensus_effects(npos_effects);

                let receipt_plan = if nexus_enabled {
                    let cursor_snapshot = self.state.da_receipt_cursor_snapshot_cached();
                    let (receipts, cache_outcome) = {
                        let da_rbc = &mut self.subsystems.da_rbc;
                        let prune_report =
                            crate::da::receipts::prune_spool(&da_rbc.spool_dir, &cursor_snapshot);
                        if prune_report.has_failures() {
                            warn!(
                                ?prune_report,
                                path = %da_rbc.spool_dir.display(),
                                "DA receipt spool cleanup encountered filesystem failures"
                            );
                        }
                        da_rbc
                            .spool_cache
                            .load_receipt_entries(&da_rbc.spool_dir)
                            .map_err(|err| eyre!(err))?
                    };
                    #[cfg(feature = "telemetry")]
                    self.telemetry.note_da_spool_cache(
                        crate::telemetry::DaSpoolCacheKind::Receipts,
                        cache_outcome.as_telemetry(),
                    );
                    #[cfg(not(feature = "telemetry"))]
                    let _ = cache_outcome;
                    crate::da::receipts::plan_committable_receipts(
                        &lane_config,
                        &cursor_snapshot,
                        receipts,
                    )
                    .map_err(|err| eyre!(err))?
                } else {
                    Vec::new()
                };

                let mut bundle_opt = {
                    let da_rbc = &mut self.subsystems.da_rbc;
                    match da_rbc.spool_cache.load_commitment_bundle(&da_rbc.spool_dir) {
                        Ok((value, cache_outcome)) => {
                            #[cfg(feature = "telemetry")]
                            self.telemetry.note_da_spool_cache(
                                crate::telemetry::DaSpoolCacheKind::Commitments,
                                cache_outcome.as_telemetry(),
                            );
                            #[cfg(not(feature = "telemetry"))]
                            let _ = cache_outcome;
                            value
                        }
                        Err(err) => {
                            return Err(eyre!(
                                "failed to load DA commitments from spool `{}`: {err}",
                                da_rbc.spool_dir.display()
                            ));
                        }
                    }
                };

                if bundle_opt.is_none() && nexus_enabled && !receipt_plan.is_empty() {
                    return Err(eyre!(
                        "DA receipts are present but no commitment records are available in the spool"
                    ));
                }

                if let Some(bundle) = bundle_opt.as_mut() {
                    // Drop commitments already present in canonical state before aligning the
                    // remaining records with receipt evidence. Manifest availability is checked
                    // after alignment so a temporarily missing strict manifest defers a known
                    // commitment instead of being misreported as a missing commitment record.
                    bundle.commitments.retain(|record| {
                        let already_committed = self
                            .state
                            .da_commitments_contains_record_identity_cached(record);
                        if already_committed {
                            warn!(
                                lane = record.lane_id.as_u32(),
                                epoch = record.epoch,
                                sequence = record.sequence,
                                "dropping DA commitment already present in the committed index before proposal sidecar assembly"
                            );
                        }
                        !already_committed
                    });

                    if nexus_enabled {
                        bundle.commitments = crate::da::receipts::align_commitments_for_receipts(
                            &receipt_plan,
                            &bundle.commitments,
                        )
                        .map_err(|err| eyre!(err))?;
                    }

                    let filtered = {
                        let da_rbc = &mut self.subsystems.da_rbc;
                        let mut kept = Vec::with_capacity(bundle.commitments.len());
                        for record in &bundle.commitments {
                            let policy = lane_config.manifest_policy(record.lane_id);
                            let (available, cache_outcome) =
                                crate::sumeragi::main_loop::manifest_available_for_commitment(
                                    &mut da_rbc.manifest_cache,
                                    &da_rbc.spool_dir,
                                    record,
                                    policy,
                                );
                            #[cfg(feature = "telemetry")]
                            self.telemetry
                                .note_da_manifest_cache(cache_outcome.as_telemetry());
                            #[cfg(not(feature = "telemetry"))]
                            let _ = cache_outcome;
                            match available {
                                Ok(true) => kept.push(record.clone()),
                                Ok(false) => {}
                                Err(err) => {
                                    return Err(eyre!(
                                        "DA manifest guard failed before including commitment in proposal for lane {} epoch {} seq {}: {err}",
                                        record.lane_id.as_u32(),
                                        record.epoch,
                                        record.sequence
                                    ));
                                }
                            }
                        }
                        kept
                    };
                    bundle.commitments = filtered;

                    if bundle.is_empty() {
                        bundle_opt = None;
                    } else {
                        self.validate_da_bundle(bundle, proposal_height)?;
                    }

                    if let Some(bundle) = bundle_opt.take() {
                        // Validate proposal-local monotonicity against a cloned canonical cursor.
                        // Only block commit may advance the State cursor or its Kura journal.
                        let mut proposal_cursors =
                            self.state.da_shard_cursor_index_snapshot_cached();
                        proposal_cursors
                            .record_bundle(&lane_config, &bundle, proposal_height)
                            .map_err(|err| {
                                eyre!("failed to validate DA shard cursors before proposal: {err}")
                            })?;
                        da_stage.commitments = Some(bundle);
                    }
                }

                let has_da_commitments = da_stage.commitments.is_some();
                if let Some(bundle) = da_stage.commitments.as_ref() {
                    builder = builder.with_da_commitments(Some(bundle.clone()));
                }

                let pin_bundle_opt = {
                    let da_rbc = &mut self.subsystems.da_rbc;
                    match da_rbc.spool_cache.load_pin_bundle(&da_rbc.spool_dir) {
                        Ok((value, cache_outcome)) => {
                            #[cfg(feature = "telemetry")]
                            self.telemetry.note_da_spool_cache(
                                crate::telemetry::DaSpoolCacheKind::PinIntents,
                                cache_outcome.as_telemetry(),
                            );
                            #[cfg(not(feature = "telemetry"))]
                            let _ = cache_outcome;
                            value
                        }
                        Err(err) => {
                            return Err(eyre!(
                                "failed to load DA pin intents from spool `{}`: {err}",
                                da_rbc.spool_dir.display()
                            ));
                        }
                    }
                };

                if let Some(bundle) = pin_bundle_opt {
                    let world = self.state.world_view();
                    let account_exists = |account: &iroha_data_model::account::AccountId| -> bool {
                        world.accounts().get(account).is_some()
                    };
                    let (mut intents, rejected) =
                        crate::da::sanitize_pin_intents_against_nexus_at_height(
                            bundle.intents,
                            &nexus,
                            proposal_height,
                            account_exists,
                        );
                    if let Some(first_rejection) = rejected.first().cloned() {
                        for reason in &rejected {
                            #[cfg(feature = "telemetry")]
                            self.telemetry.note_da_pin_intent_spool(
                                crate::telemetry::PinIntentSpoolResult::Dropped,
                                crate::telemetry::PinIntentSpoolReason::from(reason),
                            );
                            warn!(
                                height = proposal_height,
                                ?reason,
                                "rejecting invalid DA pin intent before including proposal sidecar"
                            );
                        }
                        return Err(eyre!(
                            "invalid DA pin intent in spool `{}`: {first_rejection} ({} rejection(s))",
                            self.subsystems.da_rbc.spool_dir.display(),
                            rejected.len()
                        ));
                    }
                    intents.retain(|intent| {
                        !self
                            .state
                            .da_pin_intents_contains_intent_identity_cached(intent)
                    });
                    if !intents.is_empty() {
                        let sanitized_bundle = DaPinIntentBundle::new(intents);
                        builder = builder.with_da_pin_intents(Some(sanitized_bundle.clone()));
                        da_stage.pins = Some(sanitized_bundle);
                    }
                }

                let has_da_pin_intents = da_stage.pins.is_some();
                let has_due_time_trigger = internal_work.is_some_and(|work| work.time_triggers);
                if tx_batch.is_empty()
                    && !has_due_time_trigger
                    && !has_da_commitments
                    && !has_da_pin_intents
                {
                    no_effective_work_deferred = true;
                    debug!(
                        height = proposal_height,
                        view,
                        "deferring proposal after DA spool filtering removed all effective work"
                    );
                    return Ok(());
                }

                let proof_policy_bundle =
                    crate::da::active_proof_policy_bundle_at_height(&nexus, proposal_height);
                builder = builder.with_da_proof_policies(Some(proof_policy_bundle));

                if !tx_batch.is_empty() {
                    let before_routes = routing_plan_batch
                        .iter()
                        .map(|plan| {
                            let route = plan.coordinator_route();
                            (route.lane_id.as_u32(), route.dataspace_id.as_u64())
                        })
                        .collect::<Vec<_>>();
                    let (state_height, refreshed) = {
                        let state_view = self.state.view();
                        let state_height = state_view.height();
                        let refreshed = refresh_proposal_routing_from_state(
                            &tx_batch,
                            &mut routing_batch,
                            &mut routing_plan_batch,
                            &state_view,
                            routing_ledger_time_ms,
                            proposal_height,
                        )?;
                        (state_height, refreshed)
                    };
                    let after_routes = routing_plan_batch
                        .iter()
                        .map(|plan| {
                            let route = plan.coordinator_route();
                            (route.lane_id.as_u32(), route.dataspace_id.as_u64())
                        })
                        .collect::<Vec<_>>();
                    info!(
                        height = proposal_height,
                        view,
                        state_height,
                        tx_count = tx_batch.len(),
                        refreshed,
                        before_routes = ?before_routes,
                        after_routes = ?after_routes,
                        "proposal routing resolved from committed Nexus state before execution context assembly"
                    );
                }

                let (blocked_lane_ids, removed) = self
                    .defer_batch_lanes_with_unapplied_lane_blocks(
                        proposal_height,
                        &mut tx_batch,
                        &mut routing_batch,
                        &mut routing_plan_batch,
                        &mut tx_sizes,
                        &mut removed_for_lane_readiness,
                    );
                if removed > 0 {
                    debug!(
                        height = proposal_height,
                        view,
                        removed,
                        remaining = tx_batch.len(),
                        lane_ids = ?blocked_lane_ids
                            .iter()
                            .map(|lane_id| lane_id.as_u32())
                            .collect::<Vec<_>>(),
                        "deferring final proposal transactions for lanes with unapplied lane-block artifacts"
                    );
                    if tx_batch.is_empty() {
                        lane_readiness_deferred = true;
                        return Ok(());
                    }
                    continue;
                }

                let tx_hashes: Vec<_> = tx_batch
                    .iter()
                    .map(|tx| Hash::from(tx.hash_as_entrypoint()))
                    .collect();
                let non_authoritative_lanes = self
                    .final_lane_payload_lanes_not_authorized_for_local_proposer(
                        self.state.as_ref(),
                        &routing_batch,
                        &tx_hashes,
                        proposal_height,
                        view,
                    )?;
                if !non_authoritative_lanes.is_empty() {
                    let removed = defer_batch_lanes_with_plans(
                        &mut tx_batch,
                        &mut routing_batch,
                        &mut routing_plan_batch,
                        &mut tx_sizes,
                        &non_authoritative_lanes,
                        &mut removed_for_lane_authority,
                    );
                    if removed > 0 {
                        debug!(
                            height = proposal_height,
                            view,
                            removed,
                            remaining = tx_batch.len(),
                            lane_ids = ?non_authoritative_lanes
                                .iter()
                                .map(|lane_id| lane_id.as_u32())
                                .collect::<Vec<_>>(),
                            "deferring final proposal transactions outside the local lane committee"
                        );
                        if tx_batch.is_empty() {
                            lane_authority_deferred = true;
                            return Ok(());
                        }
                        continue;
                    }
                }
                let final_lane_payload_plan = self.plan_final_lane_payload(
                    self.state.as_ref(),
                    &routing_batch,
                    &tx_hashes,
                    proposal_height,
                    view,
                )?;
                let native_amx_receipts = self
                    .native_amx_receipts_for_batch(&tx_batch, &routing_plan_batch, proposal_height)
                    .map_err(|reason| {
                        eyre!("native AMX participant attestations unavailable: {reason}")
                    })?;
                let execution_context = tx_batch
                    .iter()
                    .zip(routing_plan_batch.iter())
                    .zip(native_amx_receipts.into_iter())
                    .map(|((tx, plan), receipt)| {
                        let context = crate::queue::execution_context_for_routing_plan(
                            tx.hash_as_entrypoint(),
                            plan,
                        );
                        if let Some(receipt) = receipt {
                            context.with_native_amx_receipt(receipt)
                        } else {
                            context
                        }
                    })
                    .collect::<Vec<_>>();
                let mut execution_context = BlockExecutionContextBundle::new(execution_context)
                    .with_lane_payload_ownerships(final_lane_payload_plan.ownerships.clone());
                if let Some(entry) = pending_certified_merge_entry.as_ref() {
                    execution_context = execution_context
                        .with_merge_entry(CertifiedMergeLedgerReference::new(entry));
                }
                if !execution_context.is_empty() {
                    builder = builder.with_execution_context(Some(execution_context));
                }
                builder = builder.with_confidential_features(conf_features);
                let proposal_may_record_sccp_messages = {
                    let world_view = self.state.world_view();
                    !collect_sccp_messages_for_active_proposal_routes(
                        &tx_batch,
                        &routing_batch,
                        &nexus,
                        proposal_height,
                        |key| world_view.sccp_outbound_messages().get(key).is_some(),
                    )?
                    .is_empty()
                };
                let sccp_commitment_root = if proposal_may_record_sccp_messages {
                    let private_key = self.common_config.key_pair.private_key();
                    let initial_root = proposal_sccp_commitment_root_after_execution(
                        self.state.as_ref(),
                        &builder,
                        None,
                        private_key,
                        local_validator_index,
                    )?;
                    let stable_root = proposal_sccp_commitment_root_after_execution(
                        self.state.as_ref(),
                        &builder,
                        initial_root,
                        private_key,
                        local_validator_index,
                    )?;
                    if stable_root != initial_root {
                        return Err(eyre!(
                            "unstable SCCP commitment root after proposal execution: initial={:?} stable={:?}",
                            initial_root,
                            stable_root
                        ));
                    }
                    stable_root
                } else {
                    None
                };
                builder = builder.with_sccp_commitment_root(sccp_commitment_root);
                last_sidecar_ms = sidecar_started_at.elapsed().as_millis();

                let block_build_started_at = Instant::now();
                let new_block = builder
                    .try_sign_with_index(
                        self.common_config.key_pair.private_key(),
                        u64::from(local_validator_index),
                    )
                    .map_err(|err| eyre!("failed to sign proposed block: {err}"))?
                    .unpack(|event| self.emit_pipeline_event(event));
                let signed_block: SignedBlock = new_block.into();
                let built_height = signed_block.header().height().get();
                if built_height != proposal_height {
                    debug!(
                        expected = proposal_height,
                        actual = built_height,
                        "constructed block height differs from NEW_VIEW target"
                    );
                }
                let block_hash = signed_block.hash();
                last_block_build_ms = block_build_started_at.elapsed().as_millis();
                let payload_encode_started_at = Instant::now();
                let payload_bytes = block_payload_bytes(&signed_block);
                last_payload_encode_ms = payload_encode_started_at.elapsed().as_millis();
                if da_enabled {
                    let payload_cap = da_payload_budget(
                        self.config.rbc.chunk_max_bytes,
                        self.config.rbc.pending_max_bytes,
                        self.config.rbc.pending_max_chunks,
                        self.config.block.max_payload_bytes,
                    );
                    if payload_bytes.len() > payload_cap {
                        if tx_batch.is_empty()
                            || (tx_batch.len() == 1 && pending_certified_merge_entry.is_none())
                        {
                            return Err(eyre!(
                                "proposal payload size {} exceeds DA/RBC payload cap {payload_cap}",
                                payload_bytes.len()
                            ));
                        }
                        let excess = payload_bytes.len().saturating_sub(payload_cap);
                        let removed = trim_batch_for_size_cap_with_plans(
                            &mut tx_batch,
                            &mut routing_batch,
                            &mut routing_plan_batch,
                            &mut tx_sizes,
                            &mut removed_for_chunk_cap,
                            excess,
                        );
                        if removed == 0
                            && let Some(removed_tx) = tx_batch.pop()
                        {
                            let _removed_routing = routing_batch
                                .pop()
                                .expect("routing batch should align with tx batch");
                            let removed_plan = routing_plan_batch
                                .pop()
                                .expect("routing plan batch should align with tx batch");
                            let _ = tx_sizes.pop();
                            removed_for_chunk_cap.push((removed_tx, removed_plan));
                        }
                        #[cfg(test)]
                        record_proposal_inner_rebuild();
                        continue;
                    }
                    let total_chunks =
                        rbc::chunk_count(payload_bytes.len(), self.config.rbc.chunk_max_bytes);
                    if total_chunks > usize::try_from(RBC_MAX_TOTAL_CHUNKS).expect("fits in usize")
                    {
                        if tx_batch.is_empty()
                            || (tx_batch.len() == 1 && pending_certified_merge_entry.is_none())
                        {
                            warn!(
                                height = proposal_height,
                                view,
                                total_chunks,
                                max_chunks = RBC_MAX_TOTAL_CHUNKS,
                                "block payload exceeds RBC chunk cap; unable to assemble proposal"
                            );
                            return Err(eyre!(
                                "proposal payload requires {total_chunks} chunks, exceeding cap {}",
                                RBC_MAX_TOTAL_CHUNKS
                            ));
                        }
                        if let Some(removed_tx) = tx_batch.pop() {
                            let _removed_routing = routing_batch
                                .pop()
                                .expect("routing batch should align with tx batch");
                            let removed_plan = routing_plan_batch
                                .pop()
                                .expect("routing plan batch should align with tx batch");
                            let _ = tx_sizes.pop();
                            removed_for_chunk_cap.push((removed_tx, removed_plan));
                            #[cfg(test)]
                            record_proposal_inner_rebuild();
                            continue;
                        }
                    }
                }
                let frontier_wire_started_at = Instant::now();
                let payload_hash = Hash::new(&payload_bytes);
                let proposal = Self::build_consensus_proposal(
                    &signed_block,
                    payload_hash,
                    highest_qc,
                    local_validator_index,
                    view,
                    proposal_epoch,
                );

                let proposal_hint = super::message::ProposalHint {
                    block_hash,
                    height: proposal_height,
                    view,
                    highest_qc,
                };
                let block_created = if let Some(block_created) = self
                    .frontier_block_created_for_local_proposal_wire_with_payload(
                        &signed_block,
                        &proposal,
                        topology.as_ref(),
                        &payload_bytes,
                        payload_hash,
                    ) {
                    block_created
                } else {
                    warn!(
                        height = proposal_height,
                        view,
                        block = %block_hash,
                        "aborting active proposal because frontier metadata could not be rebuilt"
                    );
                    return Err(eyre!(
                        "failed to rebuild authoritative frontier metadata for active proposal"
                    ));
                };
                let block_created_msg = BlockMessage::BlockCreated(block_created);
                let frame_len = super::consensus_block_wire_len(
                    self.common_config.peer.id(),
                    &block_created_msg,
                );
                if frame_len > self.consensus_payload_frame_cap && !da_enabled {
                    if tx_batch.is_empty()
                        || (tx_batch.len() == 1 && pending_certified_merge_entry.is_none())
                    {
                        warn!(
                            height = proposal_height,
                            view,
                            frame_len,
                            cap = self.consensus_payload_frame_cap,
                            da_enabled,
                            "BlockCreated frame exceeds consensus payload cap; unable to assemble proposal"
                        );
                        return Err(eyre!(
                            "proposal frame size {frame_len} exceeds consensus payload cap {}",
                            self.consensus_payload_frame_cap
                        ));
                    }
                    let excess = frame_len.saturating_sub(self.consensus_payload_frame_cap);
                    let removed = trim_batch_for_size_cap_with_plans(
                        &mut tx_batch,
                        &mut routing_batch,
                        &mut routing_plan_batch,
                        &mut tx_sizes,
                        &mut removed_for_frame_cap,
                        excess,
                    );
                    if removed == 0 {
                        if let Some(removed_tx) = tx_batch.pop() {
                            let _removed_routing = routing_batch
                                .pop()
                                .expect("routing batch should align with tx batch");
                            let removed_plan = routing_plan_batch
                                .pop()
                                .expect("routing plan batch should align with tx batch");
                            let _ = tx_sizes.pop();
                            removed_for_frame_cap.push((removed_tx, removed_plan));
                            continue;
                        }
                    }
                    continue;
                }
                last_frontier_wire_ms = frontier_wire_started_at.elapsed().as_millis();

                break (
                    signed_block,
                    block_created_msg,
                    payload_bytes,
                    payload_hash,
                    proposal,
                    proposal_hint,
                    block_hash,
                    frame_len,
                    final_lane_payload_plan,
                );
            };
            proposal_block_hash_for_cleanup = Some(block_hash);
            let block_loop_ms = block_loop_started_at.elapsed().as_millis();

            let elapsed = now.elapsed();
            let base_stale_window = self
                .quorum_timeout(da_enabled)
                .max(Duration::from_millis(1));
            let stale_window =
                Self::proposal_assembly_stale_window(base_stale_window, tx_batch.len());
            if elapsed >= stale_window {
                self.subsystems
                    .propose
                    .proposal_cache
                    .pop_hint(proposal_height, view);
                self.subsystems
                    .propose
                    .proposal_cache
                    .pop_proposal(proposal_height, view);
                warn!(
                    height = proposal_height,
                    view,
                    elapsed_ms = elapsed.as_millis(),
                    base_stale_window_ms = base_stale_window.as_millis(),
                    stale_window_ms = stale_window.as_millis(),
                    tx_count = tx_batch.len(),
                    block_hash = %block_hash,
                    preflight_elapsed_ms,
                    tx_select_ms,
                    tx_prepare_ms,
                    native_precheck_ms,
                    previous_roster_ms,
                    block_loop_ms,
                    sidecar_ms = last_sidecar_ms,
                    block_build_ms = last_block_build_ms,
                    payload_encode_ms = last_payload_encode_ms,
                    frontier_wire_ms = last_frontier_wire_ms,
                    "aborting stale proposal assembly before broadcast"
                );
                return Err(eyre!(
                    "proposal assembly exceeded view window: elapsed={}ms window={}ms",
                    elapsed.as_millis(),
                    stale_window.as_millis()
                ));
            }
            let frontier_block_created_ready = matches!(
                &block_created_msg,
                BlockMessage::BlockCreated(created) if created.frontier.is_some()
            );
            let block_created_frame_fits =
                block_created_frame_len <= self.consensus_payload_frame_cap;
            let frontier_rbc_transport_needed = frontier_block_created_ready
                && (rbc::chunk_count(payload_bytes.len(), self.config.rbc.chunk_max_bytes) > 1
                    || !block_created_frame_fits);
            let inline_frontier_block_created_transport =
                frontier_block_created_ready && !frontier_rbc_transport_needed;
            let seed_frontier_backup_transport = should_seed_frontier_backup_transport(
                da_enabled,
                inline_frontier_block_created_transport,
                self.config.rbc.inline_block_created_backup,
            );
            let use_rbc_transport = da_enabled
                && (!inline_frontier_block_created_transport || seed_frontier_backup_transport);
            let mut rbc_plan = if use_rbc_transport {
                // Inline single-frame frontier BlockCreated messages can skip RBC body backup
                // when configured to favor low-latency steady-state transport. Multi-chunk,
                // oversized, and non-frontier recovery payloads continue to rely on RBC as
                // their primary DA body transport.
                self.prepare_rbc_plan(rbc::RbcPlanInputs {
                    signed_block: &signed_block,
                    transactions: &tx_batch,
                    routing: &routing_batch,
                    tx_sizes: &tx_sizes,
                    payload: &payload_bytes,
                    payload_hash,
                    height: proposal_height,
                    view,
                    epoch: proposal_epoch,
                    local_validator_index,
                })?
            } else {
                None
            };
            let block_created_wire = block_created_frame_fits.then(|| {
                let wire = Arc::new(block_created_msg.clone());
                let encoded = Arc::new(BlockMessageWire::encode_message(wire.as_ref()));
                (wire, encoded)
            });
            drop(payload_bytes);

            let topology_peers = topology.as_ref();
            let local_peer_id = self.common_config.peer.id().clone();

            crate::sumeragi::status::set_lane_payload_ownerships(
                final_lane_payload_plan.ownerships.clone(),
            );
            self.subsystems
                .propose
                .proposal_cache
                .insert_hint(proposal_hint);
            self.subsystems
                .propose
                .proposal_cache
                .insert_proposal(proposal);
            exposed_proposal_hint = Some(proposal_hint);
            exposed_proposal = Some(proposal);
            exposed_payload_hash = Some(payload_hash);

            if let Some(plan) = rbc_plan.as_mut() {
                // Non-frontier recovery always uses RBC transport. Frontier proposals keep the
                // inline fast path only when the exact body fits a consensus frame; multi-chunk
                // or otherwise oversized BlockCreated bodies use Proposal + RBC.
                self.install_rbc_session_plan(&mut plan.primary);
                if let Some(dup) = plan.duplicate.as_mut() {
                    self.install_rbc_session_plan(dup);
                }
                self.publish_rbc_backlog_snapshot();
            }

            // Put the exact body on the wire before local self-processing can emit READY/QC
            // evidence. Multi-chunk frontier payloads still use Proposal + RBC for DA transport,
            // but the body companion prevents a single missed chunk from pushing peers onto the
            // slower recovery path.
            if let Some((block_created_wire, block_created_encoded)) = block_created_wire.as_ref() {
                for peer in topology_peers {
                    if peer == &local_peer_id {
                        continue;
                    }
                    self.schedule_background(BackgroundRequest::Post {
                        peer: peer.clone(),
                        msg: BlockMessageWire::with_encoded(
                            Arc::clone(block_created_wire),
                            Arc::clone(block_created_encoded),
                        ),
                    });
                }
            } else {
                debug!(
                    height = proposal_height,
                    view,
                    frame_len = block_created_frame_len,
                    cap = self.consensus_payload_frame_cap,
                    "skipping exact BlockCreated companion; Proposal + RBC will carry the payload"
                );
            }
            self.post_proposal_metadata_to_topology(
                topology_peers,
                &local_peer_id,
                proposal_hint,
                proposal,
            );
            proposal_exposed_to_remote = topology_peers.iter().any(|peer| peer != &local_peer_id);
            let lane_block_payload_hint =
                crate::sumeragi::consensus::LaneBlockProposalPayloadHintV1 {
                    proposal_height,
                    proposal_view: view,
                    proposal_block_hash: block_hash,
                };
            self.persist_lane_executable_payloads(
                &final_lane_payload_plan.lane_block_proposal_artifacts,
                &tx_batch,
                proposal_epoch,
                lane_block_payload_hint,
            );
            self.broadcast_lane_block_plan_artifacts(
                &final_lane_payload_plan.lane_block_proposal_artifacts,
                &final_lane_payload_plan.lane_block_prepare_vote_plans,
                Some(lane_block_payload_hint),
            );
            if let Some(plan) = rbc_plan.take() {
                self.broadcast_rbc_session_plan(plan.primary)?;
                if let Some(dup) = plan.duplicate {
                    self.broadcast_rbc_session_plan(dup)?;
                }
            }

            if let BlockMessage::BlockCreated(block_msg) = block_created_msg {
                self.handle_block_created(block_msg, None)?;
            }
            if !inline_frontier_block_created_transport {
                self.handle_proposal(proposal)?;
            }

            #[cfg(test)]
            if take_proposal_publication_tail_failpoint() {
                return Err(eyre!(
                    "injected proposal processing-tail failure before ownership transfer"
                ));
            }

            if !self.proposal_has_exact_primary_block_owner(block_hash, proposal_height, view) {
                return Err(eyre!(
                    "proposal processing did not retain an exact active block owner for {block_hash} at height {proposal_height} view {view}"
                ));
            }

            // Local handling can consume cache entries while validating or finalizing the slot.
            // Reinsert advisory metadata only after exact block ownership is established.
            self.subsystems
                .propose
                .proposal_cache
                .insert_hint(proposal_hint);
            self.subsystems
                .propose
                .proposal_cache
                .insert_proposal(proposal);
            self.note_proposal_seen(proposal_height, view, payload_hash);

            #[cfg(feature = "telemetry")]
            if let Some(bundle) = signed_block.da_pin_intents() {
                for _ in &bundle.intents {
                    self.telemetry.note_da_pin_intent_spool(
                        crate::telemetry::PinIntentSpoolResult::Kept,
                        crate::telemetry::PinIntentSpoolReason::Kept,
                    );
                }
            }

            // From this point onward an exact active local block owns every included transaction
            // and DA sidecar. There are no production-fallible operations after this boundary.

            let relay_envelopes = crate::sumeragi::status::lane_relay_envelopes_snapshot();
            if !relay_envelopes.is_empty() {
                self.subsystems.merge.lane_relay.broadcast(relay_envelopes);
            }

            self.record_phase_sample(PipelinePhase::Propose, proposal_height, view);

            let tx_count = tx_batch.len();
            iroha_logger::info!(
                height = proposal_height,
                view,
                tx_count,
                queue_len,
                leader_index,
                block_hash = %block_hash,
                "assembled proposal"
            );

            Ok(())
        })();

        if let Err(err) = assembly_result {
            let concrete_owner = proposal_block_hash_for_cleanup.is_some_and(|block_hash| {
                self.proposal_has_exact_primary_block_owner(block_hash, proposal_height, view)
            });
            if !concrete_owner && proposal_exposed_to_remote {
                if let Some(hint) = exposed_proposal_hint {
                    self.subsystems.propose.proposal_cache.insert_hint(hint);
                }
                if let Some(proposal) = exposed_proposal {
                    self.subsystems
                        .propose
                        .proposal_cache
                        .insert_proposal(proposal);
                }
                if let Some(payload_hash) = exposed_payload_hash {
                    self.note_proposal_seen(proposal_height, view, payload_hash);
                }
            } else if !concrete_owner {
                self.subsystems
                    .propose
                    .proposal_cache
                    .pop_hint(proposal_height, view);
                self.subsystems
                    .propose
                    .proposal_cache
                    .pop_proposal(proposal_height, view);
                self.slot_tracker
                    .proposals_seen
                    .remove(&(proposal_height, view));
                if let Some(block_hash) = proposal_block_hash_for_cleanup {
                    self.pending.pending_blocks.remove(&block_hash);
                    self.deferred_block_sync_updates
                        .remove(&(proposal_height, view, block_hash));
                    self.clean_rbc_sessions_for_block(block_hash, proposal_height);
                }
            }
            if concrete_owner {
                let _ = self.return_proposal_guards_or_quarantine(
                    &mut tx_guards,
                    "proposal processing failure after concrete local ownership",
                );
                error!(
                    height = proposal_height,
                    view,
                    block = ?proposal_block_hash_for_cleanup,
                    error = %err,
                    "proposal processing failed after exact local ownership; retaining included transactions and signed DA sidecars"
                );
                return Ok(true);
            }
            let _ = self.return_proposal_guards_or_quarantine(
                &mut tx_guards,
                "proposal assembly failure without concrete local ownership",
            );
            if proposal_exposed_to_remote {
                warn!(
                    height = proposal_height,
                    view,
                    block = ?proposal_block_hash_for_cleanup,
                    error = %err,
                    "proposal body was exposed without a local owner; returned all transaction guards and retained the occupied slot"
                );
                return Ok(true);
            }
            return Err(err);
        }
        if lane_readiness_deferred {
            let _ = self.return_proposal_guards_or_quarantine(
                &mut tx_guards,
                "lane-block readiness proposal deferral",
            );
            info!(
                height = proposal_height,
                view, "deferring proposal: lane-block artifacts are not yet applied for this batch"
            );
            return Ok(false);
        }
        if lane_authority_deferred {
            let _ = self.return_proposal_guards_or_quarantine(
                &mut tx_guards,
                "lane-authority proposal deferral",
            );
            info!(
                height = proposal_height,
                view,
                "deferring proposal: no transactions are authorable by the local lane committee"
            );
            return Ok(false);
        }
        if no_effective_work_deferred {
            let _ = self.return_proposal_guards_or_quarantine(
                &mut tx_guards,
                "proposal deferred after effective-work filtering",
            );
            return Ok(false);
        }
        let _ = self.return_proposal_guards_or_quarantine(
            &mut tx_guards,
            "successfully published proposal",
        );

        Ok(true)
    }

    fn build_recovery_heartbeat_transaction(
        &self,
        proposal_height: u64,
    ) -> Result<AcceptedTransaction<'static>> {
        let (max_clock_drift, tx_limits) = {
            let world = self.state.world_view();
            let params = world.parameters();
            (params.sumeragi().max_clock_drift(), params.transaction())
        };
        let time_source = iroha_primitives::time::TimeSource::new_system();
        let signed = crate::tx::try_build_heartbeat_transaction_with_time_source(
            self.state.chain_id_ref().clone(),
            &self.common_config.key_pair,
            &tx_limits,
            proposal_height,
            &time_source,
        )
        .map_err(|err| eyre!("failed to sign recovery heartbeat transaction: {err}"))?;
        let crypto = self.state.crypto();
        AcceptedTransaction::accept_with_time_source(
            signed,
            self.state.chain_id_ref(),
            max_clock_drift,
            tx_limits,
            &crypto,
            &time_source,
        )
        .map_err(|err| eyre!("failed to build recovery heartbeat transaction: {err}"))
    }

    /// Enforce DA proof/commitment caps before embedding them into a block.
    ///
    /// The current `PoR` proof bundle is tracked by commitments only; we bound proof
    /// openings by the same count until proof summaries are threaded through the
    /// consensus path.
    pub(super) fn validate_da_bundle(
        &mut self,
        bundle: &DaCommitmentBundle,
        proposal_height: u64,
    ) -> Result<()> {
        let nexus = self.state.nexus_snapshot();
        let lane_config = nexus.lane_config.clone();
        validate_da_bundle_caps(
            bundle,
            self.config.da.max_commitments_per_block,
            self.config.da.max_proof_openings_per_block,
        )?;

        for record in &bundle.commitments {
            crate::da::active_lane_proof_policy_at_height(&nexus, record.lane_id, proposal_height)
                .map_err(|err| {
                eyre!(
                    "DA commitment active lane validation failed for lane {} epoch {} seq {}: {err}",
                    record.lane_id.as_u32(),
                    record.epoch,
                    record.sequence
                )
            })?;
            let policy = lane_config.manifest_policy(record.lane_id);
            let (outcome, cache_outcome) = {
                let da_rbc = &mut self.subsystems.da_rbc;
                manifest_guard_outcome(
                    &mut da_rbc.manifest_cache,
                    &da_rbc.spool_dir,
                    record,
                    policy,
                )
            };
            #[cfg(feature = "telemetry")]
            self.telemetry
                .note_da_manifest_cache(cache_outcome.as_telemetry());
            #[cfg(not(feature = "telemetry"))]
            let _ = cache_outcome;
            match outcome {
                ManifestGuardOutcome::Pass => {}
                ManifestGuardOutcome::Warn(err) => warn!(
                    ?err,
                    ?policy,
                    lane = record.lane_id.as_u32(),
                    epoch = record.epoch,
                    sequence = record.sequence,
                    "audit-only lane missing DA manifest; including commitment in proposal with warning"
                ),
                ManifestGuardOutcome::Reject(err) => {
                    return Err(eyre!(
                        "DA manifest guard failed for lane {} epoch {} seq {}: {err}",
                        record.lane_id.as_u32(),
                        record.epoch,
                        record.sequence
                    ));
                }
            }

            crate::da::validate_confidential_compute_record(&lane_config, record).map_err(
                |err| {
                    eyre!(
                        "confidential-compute validation failed for lane {} epoch {} seq {}: {err}",
                        record.lane_id.as_u32(),
                        record.epoch,
                        record.sequence
                    )
                },
            )?;
        }

        crate::da::validate_commitment_bundle_against_nexus_at_height(
            bundle,
            &nexus,
            proposal_height,
        )
        .map_err(|err| eyre!("DA commitment bundle failed validation: {err}"))?;

        Ok(())
    }

    pub(super) fn post_proposal_metadata_to_topology(
        &mut self,
        topology_peers: &[PeerId],
        local_peer_id: &PeerId,
        proposal_hint: super::message::ProposalHint,
        proposal: crate::sumeragi::consensus::Proposal,
    ) -> usize {
        let proposal_hint_msg = Arc::new(BlockMessage::ProposalHint(proposal_hint));
        let proposal_hint_encoded =
            Arc::new(BlockMessageWire::encode_message(proposal_hint_msg.as_ref()));
        let proposal_msg = Arc::new(BlockMessage::Proposal(proposal));
        let proposal_encoded = Arc::new(BlockMessageWire::encode_message(proposal_msg.as_ref()));
        let mut scheduled = 0usize;

        for peer in topology_peers {
            if peer == local_peer_id {
                continue;
            }
            self.schedule_background(BackgroundRequest::Post {
                peer: peer.clone(),
                msg: BlockMessageWire::with_encoded(
                    Arc::clone(&proposal_hint_msg),
                    Arc::clone(&proposal_hint_encoded),
                ),
            });
            self.schedule_background(BackgroundRequest::Post {
                peer: peer.clone(),
                msg: BlockMessageWire::with_encoded(
                    Arc::clone(&proposal_msg),
                    Arc::clone(&proposal_encoded),
                ),
            });
            scheduled = scheduled.saturating_add(1);
        }

        scheduled
    }

    pub(super) fn build_consensus_proposal(
        block: &SignedBlock,
        payload_hash: Hash,
        highest_qc: crate::sumeragi::consensus::QcHeaderRef,
        proposer: u32,
        view: u64,
        epoch: u64,
    ) -> crate::sumeragi::consensus::Proposal {
        let header = block.header();
        let parent_hash = header.prev_block_hash().unwrap_or_else(|| {
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0; Hash::LENGTH]))
        });
        let tx_root = header
            .merkle_root()
            .map_or_else(|| Hash::prehashed([0; Hash::LENGTH]), Hash::from);
        let state_root = header
            .result_merkle_root()
            .map_or_else(|| Hash::prehashed([0; Hash::LENGTH]), Hash::from);
        let block_height = header.height().get();

        crate::sumeragi::consensus::Proposal {
            header: crate::sumeragi::consensus::ConsensusBlockHeader {
                parent_hash,
                tx_root,
                state_root,
                proposer,
                height: block_height,
                view,
                epoch,
                highest_qc,
            },
            payload_hash,
        }
    }

    #[cfg(test)]
    pub(super) fn proposal_backpressure(&mut self) -> ProposalBackpressure {
        self.proposal_backpressure_at(Instant::now())
    }

    fn missing_qc_frontier_backpressure_override_active(&self, now: Instant) -> bool {
        if !self.config.resilience.enabled || self.queue.active_len() == 0 {
            return false;
        }
        let committed_height = u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
        let height = committed_height.saturating_add(1);
        let Some(view) = self.phase_tracker.current_view(height) else {
            return false;
        };
        if view == 0 || !self.frontier_missing_qc_liveness_active(height, view) {
            return false;
        }
        self.frontier_proposal_or_view_starved_past_ingress_grace(
            height,
            now,
            self.frontier_ingress_drain_grace(self.runtime_da_enabled()),
        )
    }

    pub(super) fn proposal_backpressure_at(&mut self, now: Instant) -> ProposalBackpressure {
        self.subsystems.propose.backpressure_gate.refresh();
        let mut queue_state = self.subsystems.propose.backpressure_gate.state();
        let queue_pressure = self.queue.pressure_snapshot();
        if queue_pressure.saturated_by_age && !queue_pressure.saturated_by_count {
            // Age-only pressure means transactions have waited too long; consensus should keep
            // proposing instead of treating that condition as a reason to wait even longer.
            queue_state = BackpressureState::Healthy {
                queued: queue_state.queued(),
                capacity: queue_state.capacity(),
            };
        }
        let blocking_pending = self.blocking_pending_blocks_len_with_progress(now);
        let queue_depths = status::worker_queue_depth_snapshot();
        let mut consensus_queue_backpressure = consensus_queue_backpressure(
            queue_depths,
            self.config.queues.block_payload,
            self.config.queues.rbc_chunks,
        );
        let backpressure_override_due = self.backpressure_override_due(now);
        let missing_qc_frontier_override =
            self.missing_qc_frontier_backpressure_override_active(now);
        let tip_height = self.state.committed_height();
        let tip_hash = self.state.latest_block_hash_fast();
        let ingress_grace = self.frontier_ingress_drain_grace(self.runtime_da_enabled());
        let quorum_timeout = self.quorum_timeout(self.runtime_da_enabled());
        let (pending_votes_or_qc, live_pending_under_congestion, recent_pending_consensus_progress) =
            self.pending.pending_blocks.values().fold(
                (false, false, false),
                |(has_votes_or_qc, has_live_pending, has_recent_progress), pending| {
                    if has_votes_or_qc && has_live_pending && has_recent_progress {
                        return (has_votes_or_qc, has_live_pending, has_recent_progress);
                    }
                    if pending.aborted || pending.validation_status == ValidationStatus::Invalid {
                        return (has_votes_or_qc, has_live_pending, has_recent_progress);
                    }
                    let block_hash = pending.block.hash();
                    let extends_tip = super::pending_extends_tip(
                        pending.height,
                        pending.block.header().prev_block_hash(),
                        tip_height,
                        tip_hash,
                    );
                    let has_consensus_progress = pending.local_commit_vote_emitted()
                        || pending.commit_qc_observed()
                        || self.pending_block_has_votes(block_hash, pending.height, pending.view)
                        || self.pending_block_has_qc(block_hash, pending.height, pending.view);
                    let recent_consensus_progress =
                        has_consensus_progress && pending.progress_age(now) < ingress_grace;
                    let consensus_evidence_blocks_proposals = self
                        .pending_consensus_evidence_blocks_proposals(pending, now, quorum_timeout);
                    (
                        has_votes_or_qc || consensus_evidence_blocks_proposals,
                        // In normal operation, payload-only pending blocks stay on the fast path.
                        // Under saturation, live pending blocks at or beyond the frontier become
                        // a proposal pacing signal so targeted load cannot churn around recovery.
                        has_live_pending
                            || extends_tip
                            || pending.height
                                > u64::try_from(tip_height.saturating_add(1)).unwrap_or(u64::MAX),
                        has_recent_progress || recent_consensus_progress,
                    )
                },
            );
        let ingress_starvation_override = self.config.resilience.enabled
            && self.queue.active_len() > 0
            && (backpressure_override_due
                || missing_qc_frontier_override
                || self.frontier_proposal_starved_past_ingress_grace(now, ingress_grace));
        let congested_tip_pending = (queue_state.is_saturated() || consensus_queue_backpressure)
            && live_pending_under_congestion
            && !ingress_starvation_override;
        let mut active_pending = pending_votes_or_qc
            || congested_tip_pending
            || blocking_pending > self.config.pacemaker.active_pending_soft_limit;
        if age_starved_queue_allows_stale_pending_override(
            queue_pressure.saturated_by_age,
            queue_pressure.saturated_by_count,
            ingress_starvation_override,
            recent_pending_consensus_progress,
        ) {
            active_pending = false;
        }
        let rbc_backlog_summary = self.proposal_rbc_backlog_summary();
        let mut rbc_backlog = self.rbc_backlog_exceeds_pacemaker_soft_limits(rbc_backlog_summary);
        let liveness_backpressure_override =
            backpressure_override_due || missing_qc_frontier_override;
        let relay_backpressure = if liveness_backpressure_override {
            // Liveness override: don't let prolonged relay/RBC backpressure stall proposals.
            rbc_backlog = false;
            false
        } else {
            self.relay_backpressure_active(now, self.rebroadcast_cooldown())
        };
        if missing_qc_frontier_override {
            active_pending = false;
            consensus_queue_backpressure = false;
            if queue_state.is_saturated() {
                queue_state = BackpressureState::Healthy {
                    queued: queue_state.queued(),
                    capacity: queue_state.capacity(),
                };
            }
        }
        let queue_only_saturation = queue_state.is_saturated()
            && !active_pending
            && !rbc_backlog
            && !relay_backpressure
            && !consensus_queue_backpressure;
        let queue_only_starved = ingress_starvation_override;
        if queue_only_saturation && queue_only_starved {
            queue_state = BackpressureState::Healthy {
                queued: queue_state.queued(),
                capacity: queue_state.capacity(),
            };
        }
        ProposalBackpressure {
            queue_state,
            active_pending,
            rbc_backlog,
            relay_backpressure,
            consensus_queue_backpressure,
        }
    }

    pub(super) fn proposal_scan_budget(&self, max_in_block: NonZeroUsize) -> usize {
        max_in_block
            .get()
            .saturating_mul(self.config.block.proposal_queue_scan_multiplier.get())
    }

    pub(super) fn filter_committed_transactions_for_proposal(
        state: &State,
        tx_guards: &mut Vec<crate::queue::TransactionGuard>,
        transactions: &mut Vec<AcceptedTransaction<'static>>,
        routing_decisions: &mut Vec<RoutingDecision>,
        routing_plans: &mut Vec<crate::queue::RoutingPlan>,
        tx_sizes: &mut Vec<usize>,
        height: u64,
        view: u64,
    ) -> Result<usize> {
        if tx_guards.len() != transactions.len()
            || transactions.len() != routing_decisions.len()
            || transactions.len() != routing_plans.len()
            || transactions.len() != tx_sizes.len()
        {
            return Err(eyre!(
                "proposal committed-filter vector length mismatch: guards={} txs={} routes={} plans={} sizes={}",
                tx_guards.len(),
                transactions.len(),
                routing_decisions.len(),
                routing_plans.len(),
                tx_sizes.len()
            ));
        }
        if let Some((index, (guard, tx))) = tx_guards
            .iter()
            .zip(transactions.iter())
            .enumerate()
            .find(|(_, (guard, tx))| guard.as_ref().hash() != tx.as_ref().hash())
        {
            return Err(eyre!(
                "proposal committed-filter guard/transaction hash mismatch at index {index}: guard={} tx={}",
                guard.as_ref().hash(),
                tx.as_ref().hash(),
            ));
        }

        let mut retained_transactions = Vec::with_capacity(transactions.len());
        let mut retained_routing = Vec::with_capacity(routing_decisions.len());
        let mut retained_routing_plans = Vec::with_capacity(routing_plans.len());
        let mut retained_sizes = Vec::with_capacity(tx_sizes.len());
        let mut dropped = 0usize;

        for (((tx, routing), routing_plan), size) in std::mem::take(transactions)
            .into_iter()
            .zip(std::mem::take(routing_decisions))
            .zip(std::mem::take(routing_plans))
            .zip(std::mem::take(tx_sizes))
        {
            if state.has_committed_transaction(tx.hash()) {
                dropped = dropped.saturating_add(1);
                continue;
            }
            retained_transactions.push(tx);
            retained_routing.push(routing);
            retained_routing_plans.push(routing_plan);
            retained_sizes.push(size);
        }

        if dropped > 0 {
            debug!(
                height,
                view, dropped, "dropping committed transactions from proposal batch"
            );
        }

        *transactions = retained_transactions;
        *routing_decisions = retained_routing;
        *routing_plans = retained_routing_plans;
        *tx_sizes = retained_sizes;

        Ok(dropped)
    }

    pub(super) fn maybe_rebroadcast_cached_proposal(
        &mut self,
        height: u64,
        view: u64,
        pending_queue_len: usize,
        now: Instant,
    ) -> Option<HashOf<BlockHeader>> {
        let cached_proposal = self
            .subsystems
            .propose
            .proposal_cache
            .get_proposal(height, view)
            .cloned();
        let cached_hint = self
            .subsystems
            .propose
            .proposal_cache
            .get_hint(height, view)
            .copied();
        let owner_hint = cached_hint
            .map(|hint| hint.block_hash)
            .or_else(|| self.authoritative_slot_owner_hash(height, view))
            .or_else(|| {
                self.frontier_slot
                    .as_ref()
                    .filter(|slot| slot.height == height && slot.view == view)
                    .map(|slot| slot.block_hash)
            });
        if cached_proposal.is_none() && owner_hint.is_none() {
            return None;
        }

        let (pending_block, block_hash, pending_payload_hash) = {
            let Some(pending) = self.pending.pending_blocks.values().find(|pending| {
                !pending.aborted
                    && pending.height == height
                    && pending.view == view
                    && cached_proposal
                        .as_ref()
                        .is_none_or(|proposal| pending.payload_hash == proposal.payload_hash)
                    && owner_hint.is_none_or(|hint| pending.block.hash() == hint)
            }) else {
                trace!(
                    height,
                    view,
                    payload = ?cached_proposal.as_ref().map(|proposal| proposal.payload_hash),
                    block = ?owner_hint,
                    "skipping cached proposal rebroadcast: pending block not found"
                );
                return None;
            };
            (
                pending.block.clone(),
                pending.block.hash(),
                pending.payload_hash,
            )
        };

        let (consensus_mode, _, _) = self.consensus_context_for_height(height);
        let proposal_roster = self.roster_for_live_vote_with_mode(height, consensus_mode);
        if proposal_roster.is_empty() {
            trace!(
                height,
                view, "skipping cached proposal rebroadcast: empty commit topology"
            );
            return None;
        }
        let mut topology = super::network_topology::Topology::new(proposal_roster.clone());
        let leader_index = match self.leader_index_for(&mut topology, height, view) {
            Ok(idx) => idx,
            Err(err) => {
                warn!(
                    ?err,
                    height, view, "failed to compute leader index for cached proposal rebroadcast"
                );
                return None;
            }
        };

        let Some(local_pos) = topology.position(self.common_config.peer.id().public_key()) else {
            trace!(
                height,
                view, "skipping cached proposal rebroadcast: local peer not in validator set"
            );
            return None;
        };
        let locally_authoritative_frontier = self
            .locally_authoritative_frontier_info_for_block(&pending_block)
            .filter(|frontier| {
                frontier.payload_hash == pending_payload_hash
                    && usize::try_from(frontier.proposer) == Ok(leader_index)
                    && frontier.epoch == self.epoch_for_height(height)
            });
        let proposal = if let Some(proposal) = cached_proposal {
            if usize::try_from(proposal.header.proposer) != Ok(leader_index) {
                warn!(
                    height,
                    view,
                    block = %block_hash,
                    proposer = proposal.header.proposer,
                    leader_index,
                    "skipping cached proposal rebroadcast: cached proposer is not the selected leader"
                );
                return None;
            }
            proposal
        } else if let Some(hint) = cached_hint.filter(|hint| hint.block_hash == block_hash) {
            let proposer = match u32::try_from(leader_index) {
                Ok(proposer) => proposer,
                Err(_) => {
                    warn!(
                        height,
                        view,
                        block = %block_hash,
                        leader_index,
                        "skipping cached proposal rebroadcast: leader index exceeds proposal field"
                    );
                    return None;
                }
            };
            let proposal = Self::build_consensus_proposal(
                &pending_block,
                pending_payload_hash,
                hint.highest_qc,
                proposer,
                view,
                self.epoch_for_height(height),
            );
            self.subsystems
                .propose
                .proposal_cache
                .insert_proposal(proposal.clone());
            proposal
        } else if let Some(frontier) = locally_authoritative_frontier.clone() {
            let proposal = Self::build_consensus_proposal(
                &pending_block,
                pending_payload_hash,
                frontier.highest_qc,
                frontier.proposer,
                view,
                frontier.epoch,
            );
            self.subsystems
                .propose
                .proposal_cache
                .insert_proposal(proposal.clone());
            proposal
        } else {
            trace!(
                height,
                view,
                block = %block_hash,
                "skipping cached proposal rebroadcast: no proposal metadata retained"
            );
            return None;
        };

        let frontier_recovery_cached = self.config.resilience.enabled
            && height == self.committed_height_snapshot().saturating_add(1)
            && view > 0
            && pending_queue_len > 0;
        let prior_body_rebroadcast = self
            .proposal_rebroadcast_log
            .last_sent_at(&block_hash)
            .is_some();
        let payload_cooldown = self.payload_rebroadcast_cooldown();
        let mut cooldown = if frontier_recovery_cached {
            self.targeted_payload_rescue_cooldown()
        } else {
            payload_cooldown
        };
        cooldown = cooldown.max(CACHED_PROPOSAL_REBROADCAST_COOLDOWN_FLOOR);
        if self.relay_backpressure_active(now, payload_cooldown)
            && (!frontier_recovery_cached || prior_body_rebroadcast)
        {
            trace!(
                height,
                view,
                block = %block_hash,
                frontier_recovery_cached,
                prior_body_rebroadcast,
                "skipping cached proposal rebroadcast due to relay backpressure"
            );
            return None;
        }
        let queue_drop_backpressure = self.queue_drop_backpressure_active(now, payload_cooldown);
        let queue_block_backpressure = self.queue_block_backpressure_active(now, payload_cooldown);
        if (queue_drop_backpressure || queue_block_backpressure) && prior_body_rebroadcast {
            let queue_depths = super::status::worker_queue_depth_snapshot();
            trace!(
                height,
                view,
                block = %block_hash,
                frontier_recovery_cached,
                queue_drop_backpressure,
                queue_block_backpressure,
                block_payload_rx_depth = queue_depths.block_payload_rx,
                rbc_chunk_rx_depth = queue_depths.rbc_chunk_rx,
                "skipping cached proposal rebroadcast due to consensus queue backpressure"
            );
            return None;
        }
        let tx_queue_pressure = self.queue.pressure_snapshot();
        let tx_queue_capacity_backpressure =
            tx_queue_pressure.saturated_by_count || tx_queue_pressure.saturated_by_bytes;
        if tx_queue_capacity_backpressure && prior_body_rebroadcast {
            let slow_cooldown = self.rbc_deliver_commit_qc_recovery_cooldown().max(cooldown);
            if slow_cooldown > cooldown {
                trace!(
                    height,
                    view,
                    block = %block_hash,
                    frontier_recovery_cached,
                    queued = tx_queue_pressure.queued_tx_count,
                    tracked = tx_queue_pressure.tracked_tx_count,
                    capacity = tx_queue_pressure.capacity.get(),
                    retained_bytes = tx_queue_pressure.retained_bytes,
                    max_retained_bytes = tx_queue_pressure.max_retained_bytes.get(),
                    saturated_by_count = tx_queue_pressure.saturated_by_count,
                    saturated_by_bytes = tx_queue_pressure.saturated_by_bytes,
                    cooldown_ms = slow_cooldown.as_millis(),
                    "slowing cached proposal rebroadcast due to transaction queue backpressure"
                );
                cooldown = slow_cooldown;
            }
        }
        if !self
            .proposal_rebroadcast_log
            .allow(block_hash, now, cooldown)
        {
            trace!(
                height,
                view,
                block = %block_hash,
                local_idx = local_pos,
                leader_index,
                cooldown_ms = cooldown.as_millis(),
                "skipping cached proposal rebroadcast due to cooldown"
            );
            return None;
        }

        let local_peer_id = self.common_config.peer.id().clone();
        let block_created = {
            let Some(pending) = self
                .pending
                .pending_blocks
                .get(&block_hash)
                .filter(|pending| {
                    !pending.aborted
                        && pending.height == height
                        && pending.view == view
                        && pending.payload_hash == pending_payload_hash
                })
            else {
                trace!(
                    height,
                    view,
                    block = %block_hash,
                    "skipping cached proposal rebroadcast: pending block changed before wire build"
                );
                return None;
            };
            let pending_payload_bytes = pending.payload_bytes();
            self.frontier_block_created_for_local_proposal_wire_with_payload(
                &pending.block,
                &proposal,
                &proposal_roster,
                pending_payload_bytes,
                pending_payload_hash,
            )
            .unwrap_or_else(|| {
                warn!(
                    height,
                    view,
                    block = %block_hash,
                    "rebroadcasting cached proposal with plain block-created fallback"
                );
                super::message::BlockCreated::from(&pending.block)
            })
        };
        let block_msg = Arc::new(BlockMessage::BlockCreated(block_created));
        let block_encoded = Arc::new(BlockMessageWire::encode_message(block_msg.as_ref()));
        let proposal_hint = super::message::ProposalHint {
            block_hash,
            height,
            view,
            highest_qc: proposal.header.highest_qc,
        };
        let mut scheduled = 0usize;
        for peer in topology.iter() {
            if peer == &local_peer_id {
                continue;
            }
            self.schedule_background(BackgroundRequest::Post {
                peer: peer.clone(),
                msg: BlockMessageWire::with_encoded(
                    Arc::clone(&block_msg),
                    Arc::clone(&block_encoded),
                ),
            });
            scheduled = scheduled.saturating_add(1);
        }
        self.post_proposal_metadata_to_topology(
            topology.as_ref(),
            &local_peer_id,
            proposal_hint,
            proposal,
        );
        if scheduled == 0 {
            trace!(
                height,
                view,
                block = %block_hash,
                "skipping cached proposal rebroadcast: no remote validators"
            );
            return None;
        }
        if pending_queue_len > 0 {
            iroha_logger::info!(
                height,
                view,
                block = %block_hash,
                frontier_recovery_cached,
                local_idx = local_pos,
                leader_index,
                cooldown_ms = cooldown.as_millis(),
                "rebroadcasting cached proposal"
            );
        } else {
            debug!(
                height,
                view,
                block = %block_hash,
                frontier_recovery_cached,
                local_idx = local_pos,
                leader_index,
                cooldown_ms = cooldown.as_millis(),
                "rebroadcasting cached proposal"
            );
        }
        Some(block_hash)
    }

    fn nudge_frontier_recovery_proposal_retry(&mut self, now: Instant) {
        if let Some(next_deadline) = now.checked_add(PACEMAKER_QUEUE_NUDGE_MIN_INTERVAL)
            && self.subsystems.propose.pacemaker.next_deadline > next_deadline
        {
            self.subsystems.propose.pacemaker.next_deadline = next_deadline;
        }
    }

    pub(super) fn recent_pending_validation_for_slot(
        &self,
        height: u64,
        view: u64,
        expected_hash: Option<HashOf<BlockHeader>>,
        now: Instant,
        freshness_window: Duration,
    ) -> Option<(HashOf<BlockHeader>, Duration)> {
        if freshness_window == Duration::ZERO {
            return None;
        }

        let mut youngest = None;
        for (block_hash, pending) in &self.pending.pending_blocks {
            if pending.aborted
                || pending.is_retired_same_height()
                || pending.validation_status != ValidationStatus::Pending
                || pending.height != height
                || pending.view != view
                || expected_hash.is_some_and(|expected| expected != *block_hash)
            {
                continue;
            }

            let age = pending
                .progress_age(now)
                .max(now.saturating_duration_since(pending.inserted_at));
            if age < freshness_window && youngest.is_none_or(|(_, current_age)| age < current_age) {
                youngest = Some((*block_hash, age));
            }
        }
        youngest
    }

    pub(super) fn validation_inflight_for_slot(
        &self,
        height: u64,
        view: u64,
        expected_hash: Option<HashOf<BlockHeader>>,
    ) -> bool {
        self.subsystems
            .validation
            .inflight
            .keys()
            .any(|block_hash| {
                expected_hash.is_none_or(|expected| expected == *block_hash)
                    && self
                        .pending
                        .pending_blocks
                        .get(block_hash)
                        .is_some_and(|pending| {
                            !pending.aborted
                                && !pending.is_retired_same_height()
                                && pending.validation_status != ValidationStatus::Invalid
                                && pending.height == height
                                && pending.view == view
                        })
            })
    }

    pub(super) fn maybe_progress_existing_slot_proposal(
        &mut self,
        height: u64,
        view: u64,
        pending_queue_len: usize,
        now: Instant,
        trigger: &'static str,
    ) -> bool {
        let mut progressed = self
            .maybe_rebroadcast_cached_proposal(height, view, pending_queue_len, now)
            .is_some();

        let owner_hint = self
            .authoritative_slot_owner_hash(height, view)
            .or_else(|| {
                self.frontier_slot
                    .as_ref()
                    .filter(|slot| slot.height == height && slot.view == view)
                    .map(|slot| slot.block_hash)
            });
        if let Some(block_hash) = owner_hint
            && !self.frontier_block_materialized_locally(block_hash)
        {
            progressed |= self.request_frontier_owner_body_repair(block_hash, height, view, now);
        }

        let pending_hashes: Vec<_> = self
            .pending
            .pending_blocks
            .iter()
            .filter_map(|(block_hash, pending)| {
                (!pending.aborted
                    && !pending.is_retired_same_height()
                    && pending.validation_status == ValidationStatus::Valid
                    && pending.height == height
                    && pending.view == view)
                    .then_some(*block_hash)
            })
            .collect();
        if pending_hashes.is_empty() {
            if progressed {
                self.nudge_frontier_recovery_proposal_retry(now);
            }
            return progressed;
        }

        let commit_topology = self.effective_commit_topology();
        for block_hash in pending_hashes {
            if self.maybe_emit_local_commit_vote_for_pending_event(
                block_hash,
                height,
                view,
                &commit_topology,
                trigger,
            ) {
                progressed = true;
                continue;
            }

            if self.maybe_replay_known_block_commit_evidence(
                block_hash,
                height,
                view,
                &commit_topology,
                trigger,
            ) {
                self.request_commit_pipeline_for_pending(
                    block_hash,
                    super::status::RoundEventCauseTrace::VoteReceived,
                    None,
                );
                progressed = true;
            }
        }

        if progressed {
            self.nudge_frontier_recovery_proposal_retry(now);
        }
        progressed
    }

    pub(super) fn same_height_frontier_owner_blocks_proposal(
        &mut self,
        height: u64,
        view_idx: u64,
        pending_queue_len: usize,
        now: Instant,
        highest_qc: crate::sumeragi::consensus::QcHeaderRef,
    ) -> bool {
        let Some((owner_hash, owner_view)) = self
            .frontier_slot_live_local_owner_for_round(height, view_idx)
            .filter(|(_, owner_view)| *owner_view < view_idx)
        else {
            return false;
        };
        let stale_local_commit_vote_allows_owner_clear = self
            .stale_local_commit_vote_allows_frontier_owner_clear_for_proposal_assembly(
                height, view_idx, owner_hash, owner_view, now, highest_qc,
            );
        if self.maybe_yield_stale_frontier_owner_for_fresh_proposal(
            height,
            view_idx,
            owner_hash,
            owner_view,
            now,
            pending_queue_len,
        ) {
            debug!(
                height,
                view = view_idx,
                owner = %owner_hash,
                owner_view,
                queue_len = pending_queue_len,
                "stale same-height frontier owner yielded; continuing fresh proposal assembly"
            );
            false
        } else if stale_local_commit_vote_allows_owner_clear {
            let dropped =
                self.drop_stale_pending_block_for_fresh_proposal(owner_hash, height, owner_view);
            if self.frontier_slot.as_ref().is_some_and(|slot| {
                slot.height == height && slot.view == owner_view && slot.block_hash == owner_hash
            }) {
                self.frontier_slot = None;
            }
            info!(
                height,
                view = view_idx,
                owner = %owner_hash,
                owner_view,
                queue_len = pending_queue_len,
                dropped_tx_count = dropped.map(|(tx_count, _, _, _, _)| tx_count),
                "cleared stale same-height frontier owner for fresh proposal assembly after missing-QC repair"
            );
            false
        } else {
            let progressed = self.maybe_progress_existing_slot_proposal(
                height,
                owner_view,
                pending_queue_len,
                now,
                "same_height_owner_live",
            );
            if !progressed {
                self.nudge_frontier_recovery_proposal_retry(now);
            }
            if pending_queue_len > 0 {
                debug!(
                    height,
                    view = view_idx,
                    owner = %owner_hash,
                    owner_view,
                    queue_len = pending_queue_len,
                    "same-height frontier owner is still locally live for this round; deferring reassembly"
                );
            } else {
                trace!(
                    height,
                    view = view_idx,
                    owner = %owner_hash,
                    owner_view,
                    "same-height frontier owner is still locally live for this round; deferring reassembly"
                );
            }
            self.warn_resilience_frontier_proposal_deferred(
                height,
                view_idx,
                "same_height_owner_live",
                highest_qc,
                pending_queue_len,
                now,
            );
            true
        }
    }

    fn stale_proposals_seen_only_slot_allows_recovery_rotation(
        &self,
        height: u64,
        view: u64,
        epoch: u64,
        now: Instant,
        precommit_votes_at_view: usize,
    ) -> Option<(Duration, Duration)> {
        if !self.config.resilience.enabled
            || height != self.committed_height_snapshot().saturating_add(1)
            || view == 0
            || precommit_votes_at_view > 0
            || !self.slot_tracker.proposals_seen.contains(&(height, view))
            || self
                .subsystems
                .propose
                .proposal_cache
                .get_proposal(height, view)
                .is_some()
            || self
                .subsystems
                .propose
                .proposal_cache
                .get_hint(height, view)
                .is_some()
            || self.authoritative_slot_owner_hash(height, view).is_some()
            || self
                .frontier_slot_live_local_owner_for_round(height, view)
                .is_some()
            || self.slot_has_actionable_vote_backed_proposal_evidence(height, view, epoch)
            || self.same_height_has_recoverable_qc(height)
            || self.pending.pending_blocks.values().any(|pending| {
                !pending.aborted
                    && !pending.is_retired_same_height()
                    && pending.validation_status != ValidationStatus::Invalid
                    && pending.height == height
                    && pending.view == view
            })
        {
            return None;
        }

        if let Some(existing_vote) = self.local_same_height_vote(height, epoch)
            && existing_vote.view >= view
            && self.local_same_height_vote_blocks_fresh_proposal(
                height,
                view,
                &existing_vote,
                now,
                true,
            )
        {
            return None;
        }

        let stale_window = self
            .quorum_timeout(self.runtime_da_enabled())
            .max(PACEMAKER_QUEUE_NUDGE_MIN_INTERVAL)
            .max(Duration::from_millis(1));
        let view_age = self.phase_tracker.view_age(height, now)?;
        (view_age >= stale_window).then_some((view_age, stale_window))
    }

    pub(super) fn stale_slot_proposal_evidence_allows_recovery_rotation(
        &self,
        height: u64,
        view: u64,
        epoch: u64,
        now: Instant,
        precommit_votes_at_view: usize,
        pending_queue_len: usize,
        highest_qc: crate::sumeragi::consensus::QcHeaderRef,
    ) -> Option<(Duration, Duration)> {
        let full_stale_window = self
            .quorum_timeout(self.runtime_da_enabled())
            .max(PACEMAKER_QUEUE_NUDGE_MIN_INTERVAL)
            .max(self.frontier_slot_lag_window())
            .max(Duration::from_millis(1));
        let stale_window = if pending_queue_len > 0 && precommit_votes_at_view == 0 {
            self.cap_active_block_production_gap(full_stale_window, true)
                .max(PACEMAKER_QUEUE_NUDGE_MIN_INTERVAL)
                .max(Duration::from_millis(1))
        } else {
            full_stale_window
        };
        if !self.config.resilience.enabled
            || height != self.committed_height_snapshot().saturating_add(1)
            || view == 0
            || precommit_votes_at_view > 0
            || !self.frontier_missing_qc_liveness_active(height, view)
            || self.same_height_has_recoverable_qc(height)
            || self.validation_inflight_for_slot(height, view, None)
            || self
                .recent_pending_validation_for_slot(height, view, None, now, stale_window)
                .is_some()
            || self
                .subsystems
                .commit
                .inflight
                .as_ref()
                .is_some_and(|inflight| {
                    !inflight.pending.aborted
                        && inflight.pending.validation_status != ValidationStatus::Invalid
                        && inflight.pending.height == height
                        && inflight.pending.view == view
                })
            || self.pending.pending_blocks.values().any(|pending| {
                !pending.aborted
                    && pending.validation_status != ValidationStatus::Invalid
                    && pending.height == height
                    && pending.view == view
                    && pending.commit_qc_observed()
            })
            || self.frontier_slot.as_ref().is_some_and(|slot| {
                slot.height == height
                    && slot.view == view
                    && slot.quorum_progress.commit_qc_observed
            })
        {
            return None;
        }

        if let Some(existing_vote) = self.local_same_height_vote(height, epoch)
            && existing_vote.view >= view
            && self.local_same_height_vote_blocks_fresh_proposal_assembly(
                height,
                view,
                &existing_vote,
                now,
                true,
                highest_qc,
            )
        {
            return None;
        }

        let view_age = self.phase_tracker.view_age(height, now)?;
        (view_age >= stale_window).then_some((view_age, stale_window))
    }

    pub(super) fn clear_exhausted_frontier_proposal_marker(
        &mut self,
        height: u64,
        view: u64,
    ) -> bool {
        let removed = self.slot_tracker.proposals_seen.remove(&(height, view));
        if self
            .subsystems
            .propose
            .proposal_liveness
            .is_some_and(|slot| slot.height == height && slot.view == view)
        {
            self.subsystems.propose.proposal_liveness = None;
        }
        removed
    }

    pub(super) fn on_pacemaker_backpressure_deferral(
        &mut self,
        now: Instant,
        state: BackpressureState,
    ) {
        let blocking_pending = self.blocking_pending_blocks_len_with_progress(now);
        let active_pending = blocking_pending > self.config.pacemaker.active_pending_soft_limit;
        let rbc_backlog_summary = self.rbc_backlog_summary();
        let rbc_backlog = self.rbc_backlog_exceeds_pacemaker_soft_limits(rbc_backlog_summary);
        let relay_backpressure = self.relay_backpressure_active(now, self.rebroadcast_cooldown());
        super::status::inc_pacemaker_backpressure_deferrals();
        #[cfg(feature = "telemetry")]
        {
            self.telemetry.inc_pacemaker_backpressure_deferrals();
        }
        debug!(
            ?now,
            tx_queue_depth = state.queued(),
            tx_queue_capacity = state.capacity().get(),
            active_pending,
            blocking_pending,
            pending_soft_limit = self.config.pacemaker.active_pending_soft_limit,
            rbc_backlog,
            rbc_sessions = rbc_backlog_summary.sessions_pending,
            rbc_missing_chunks = rbc_backlog_summary.missing_chunks_total,
            rbc_session_soft_limit = self.config.pacemaker.rbc_backlog_session_soft_limit,
            rbc_chunk_soft_limit = self.config.pacemaker.rbc_backlog_chunk_soft_limit,
            relay_backpressure,
            "Pacemaker deferred proposal assembly due to backpressure"
        );
    }

    pub(super) fn maybe_rebroadcast_new_view_votes(&mut self, height: u64, now: Instant) {
        if self.is_observer() {
            return;
        }
        let target = self
            .subsystems
            .propose
            .new_view_tracker
            .entries
            .iter()
            .rev()
            .find(|((entry_height, _), _)| *entry_height == height)
            .map(|(key, entry)| (*key, entry.highest_qc.subject_block_hash));
        let Some(((target_height, target_view), block_hash)) = target else {
            return;
        };
        let frontier_recovery_new_view = self.config.resilience.enabled
            && target_height == self.committed_height_snapshot().saturating_add(1)
            && target_view > 0;
        let cooldown = if frontier_recovery_new_view {
            self.frontier_recovery_new_view_rebroadcast_cooldown()
        } else {
            self.rebroadcast_cooldown()
        };
        if !self.new_view_rebroadcast_log.allow(
            block_hash,
            target_height,
            target_view,
            now,
            cooldown,
        ) {
            trace!(
                height = target_height,
                view = target_view,
                block = ?block_hash,
                cooldown_ms = cooldown.as_millis(),
                "skipping NEW_VIEW vote rebroadcast due to cooldown"
            );
            return;
        }
        let targets = self.new_view_rebroadcast_targets(block_hash, target_height, target_view);
        let rebroadcasted = self.rebroadcast_block_votes_to_targets_with_backpressure(
            crate::sumeragi::consensus::Phase::NewView,
            block_hash,
            target_height,
            target_view,
            &targets,
            true,
            "new_view_convergence_rebroadcast",
        );
        if rebroadcasted == 0 {
            debug!(
                height = target_height,
                view = target_view,
                block = ?block_hash,
                "no NEW_VIEW votes available for rebroadcast"
            );
        }
    }

    pub(super) fn precommit_vote_blocks_proposal_assembly(
        &self,
        vote: &crate::sumeragi::consensus::Vote,
        height: u64,
        view: u64,
        epoch: u64,
    ) -> bool {
        vote.phase == crate::sumeragi::consensus::Phase::Commit
            && vote.height == height
            && vote.view == view
            && vote.epoch == epoch
            && self.vote_payload_actionable_for_proposal(vote.block_hash, height, view)
    }

    fn slot_has_actionable_vote_backed_proposal_evidence(
        &self,
        height: u64,
        view: u64,
        epoch: u64,
    ) -> bool {
        self.stored_votes().any(|vote| {
            matches!(
                vote.phase,
                crate::sumeragi::consensus::Phase::Prepare
                    | crate::sumeragi::consensus::Phase::Commit
            ) && vote.height == height
                && vote.view == view
                && vote.epoch == epoch
                && self.vote_payload_actionable_for_proposal(vote.block_hash, height, view)
        }) || self.qc_cache.values().any(|qc| {
            matches!(
                qc.phase,
                crate::sumeragi::consensus::Phase::Prepare
                    | crate::sumeragi::consensus::Phase::Commit
            ) && qc.height == height
                && qc.view == view
                && qc.epoch == epoch
                && self.vote_payload_actionable_for_proposal(qc.subject_block_hash, height, view)
        })
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn on_pacemaker_propose_ready(&mut self, now: Instant) -> bool {
        self.on_pacemaker_propose_ready_with_dependency_override(now, false)
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn on_pacemaker_propose_ready_with_dependency_override(
        &mut self,
        now: Instant,
        allow_dependency_gated_reproposal: bool,
    ) -> bool {
        trace!(?now, "pacemaker evaluating NEW_VIEW gating");
        if !self.retry_quarantined_proposal_guards() {
            return false;
        }
        if self.round_liveness_isolated() {
            self.subsystems.propose.pacemaker.next_deadline = now
                .checked_add(
                    self.subsystems
                        .propose
                        .pacemaker
                        .propose_interval
                        .max(Duration::from_millis(1)),
                )
                .unwrap_or(now);
            debug!("suppressing proposal path while round liveness catch-up isolation is active");
            return false;
        }
        let prev_attempt = self.subsystems.propose.last_pacemaker_attempt.replace(now);
        let tip_height = self.state.committed_height();
        let tip_hash = self.state.latest_block_hash_fast();
        let pending_queue_len = self.queue.queued_len();
        let active_pending = self.active_pending_blocks_len_for_tip(tip_height, tip_hash);
        let view_height = tip_height;
        let committed_height = view_height as u64;

        // Drop NEW_VIEW entries that point at already-committed heights so the pacemaker
        // cannot re-propose a finalized height after a commit.
        self.subsystems
            .propose
            .new_view_tracker
            .prune(committed_height);

        self.promote_locked_qc_to_highest_if_needed("pacemaker");

        let da_enabled = self.runtime_da_enabled();
        let committed_qc = self.latest_committed_qc();
        let precommit_qc = precommit_qc_for_view_change(self.highest_qc, committed_qc);
        let desired_height = active_round_height(self.highest_qc, committed_qc, committed_height);
        let tracked_height = desired_height.min(committed_height.saturating_add(1));
        if tracked_height != desired_height {
            debug!(
                desired_height,
                tracked_height,
                committed_height,
                "clamping pacemaker active round height to local commit horizon"
            );
        }
        let (consensus_mode, _, _) = self.consensus_context_for_height(tracked_height);
        let topology_peers = self.roster_for_live_vote_with_mode(tracked_height, consensus_mode);
        let active_topology_peers = topology_peers.clone();
        let tracked_view = self.phase_tracker.current_view(tracked_height).unwrap_or(0);
        let queue_depths = super::status::worker_queue_depth_snapshot();
        let ingress_grace = self.frontier_ingress_drain_grace(self.runtime_da_enabled());
        let frontier_recovery_ingress_override = self.frontier_recovery_ingress_override_active(
            tracked_height,
            tracked_view,
            now,
            ingress_grace,
        );
        let payload_or_block_ingress_queued = queue_depths.block_payload_rx > 0
            || queue_depths.rbc_chunk_rx > 0
            || queue_depths.block_rx > 0;
        let frontier_proposal_ingress_deferring = self.config.resilience.enabled
            && tracked_height == committed_height.saturating_add(1)
            && tracked_view > 0
            && (!frontier_recovery_ingress_override || payload_or_block_ingress_queued)
            && self.frontier_proposal_ingress_defer_active(
                tracked_height,
                tracked_view,
                now,
                queue_depths,
                ingress_grace,
            );
        let active_cached_frontier_slot = self.config.resilience.enabled
            && tracked_height == committed_height.saturating_add(1)
            && self
                .subsystems
                .propose
                .proposal_cache
                .get_proposal(tracked_height, tracked_view)
                .is_some();
        // Drop stale NEW_VIEW entries before any proposal-path early return so stale future
        // evidence cannot keep re-triggering catch-up after the local pacemaker has advanced.
        self.subsystems
            .propose
            .new_view_tracker
            .drop_below_height(tracked_height);
        let stale_future_new_view_entries: Vec<_> = self
            .subsystems
            .propose
            .new_view_tracker
            .entries
            .keys()
            .filter_map(|(entry_height, entry_view)| {
                if *entry_height <= tracked_height {
                    return None;
                }
                let local_view = self.phase_tracker.current_view(*entry_height)?;
                (*entry_view < local_view).then_some((*entry_height, *entry_view, local_view))
            })
            .collect();
        for (entry_height, entry_view, local_view) in stale_future_new_view_entries {
            self.subsystems
                .propose
                .new_view_tracker
                .remove(entry_height, entry_view);
            debug!(
                height = entry_height,
                view = entry_view,
                local_view,
                tracked_height,
                "pruned stale future NEW_VIEW entry before proposal selection"
            );
        }
        if frontier_proposal_ingress_deferring && !active_cached_frontier_slot {
            if frontier_recovery_ingress_override {
                let _ = self.seed_frontier_slot_from_same_height_evidence(
                    tracked_height,
                    tracked_view,
                    now,
                    "vote_locked_ingress_override",
                    false,
                );
            }
            self.subsystems.propose.pacemaker.next_deadline = now
                .checked_add(PACEMAKER_QUEUE_NUDGE_MIN_INTERVAL)
                .unwrap_or(now);
            debug!(
                height = tracked_height,
                view = tracked_view,
                block_payload_rx_depth = queue_depths.block_payload_rx,
                rbc_chunk_rx_depth = queue_depths.rbc_chunk_rx,
                block_rx_depth = queue_depths.block_rx,
                "deferring proposal while frontier payload ingress drains"
            );
            return false;
        }
        if self.proposal_gated_by_missing_dependencies(tracked_height)
            && !allow_dependency_gated_reproposal
            && !active_cached_frontier_slot
        {
            self.subsystems.propose.pacemaker.next_deadline = now
                .checked_add(
                    self.subsystems
                        .propose
                        .pacemaker
                        .propose_interval
                        .max(Duration::from_millis(1)),
                )
                .unwrap_or(now);
            debug!(
                height = tracked_height,
                view = tracked_view,
                "deferring proposal while canonical dependencies are still recovering"
            );
            return false;
        }
        if topology_peers.is_empty() {
            let _ = self.handle_roster_unavailable_recovery(
                tracked_height,
                tracked_view,
                tip_hash,
                pending_queue_len,
                now,
                ProposalDeferWarningKind::EmptyCommitTopologyProposal,
                "pacemaker_propose_ready",
            );
            return false;
        }

        let mut topology = super::network_topology::Topology::new(topology_peers);
        let mut required = topology.min_votes_for_commit();
        let local_peer_id = self.common_config.peer.id().clone();
        let local_idx = self.local_validator_index_for_topology(&topology);
        let local_peer = local_idx.map(|_| local_peer_id.clone());
        let frontier_partial_new_view_support_detected = self.config.resilience.enabled
            && tracked_height == committed_height.saturating_add(1)
            && tracked_view > 0
            && (self
                .subsystems
                .propose
                .new_view_tracker
                .entries
                .get(&(tracked_height, tracked_view))
                .is_some_and(|entry| {
                    let roster_set: BTreeSet<_> = topology.as_ref().iter().cloned().collect();
                    let support = entry.count_in_roster(&roster_set, local_peer.as_ref());
                    support > 0 && support < required
                })
                || self.frontier_partial_new_view_vote_support(
                    tracked_height,
                    tracked_view,
                    required,
                    &topology,
                ));
        let frontier_partial_new_view_support = frontier_partial_new_view_support_detected
            && self.frontier_partial_new_view_support_still_converging(tracked_height, now);
        if frontier_partial_new_view_support_detected && !frontier_partial_new_view_support {
            debug!(
                height = tracked_height,
                view = tracked_view,
                committed_height,
                convergence_grace_ms = self
                    .frontier_partial_new_view_convergence_grace()
                    .as_millis(),
                "partial NEW_VIEW support did not converge before the grace window; allowing committed-QC frontier fallback"
            );
        }

        let has_queue_work = pending_queue_len > 0;
        if da_enabled && !has_queue_work {
            trace!(
                da_enabled,
                "DA enabled and transaction queue is empty; checking internal work"
            );
        }

        let online_peers = self
            .network
            .online_peers(|peers| super::count_online_validators(peers, topology.as_ref()));
        // `online_peers` counts only remote peers in the current validator roster; include the
        // local node if it is part of the commit topology so we do not stall when exactly
        // `required` validators are up.
        let online_total = online_peers + usize::from(local_idx.is_some());
        let mut view_age = self.phase_tracker.view_age(tracked_height, now);
        if view_age.is_none() {
            self.phase_tracker.start_new_round(tracked_height, now);
            view_age = self.phase_tracker.view_age(tracked_height, now);
        }
        let current_view = self.phase_tracker.current_view(tracked_height);
        self.clear_consensus_recovery_for_round(tracked_height, current_view.unwrap_or(0));
        let bootstrap_view = current_view.is_none_or(|view| view == 0);
        let missing_qc_frontier_self_proposal_qc = self
            .missing_qc_liveness_allows_frontier_self_proposal(
                tracked_height,
                tracked_view,
                committed_height,
                pending_queue_len,
                precommit_qc,
            );
        let missing_qc_frontier_self_proposal_ready =
            missing_qc_frontier_self_proposal_qc.is_some();
        let missing_qc_frontier_self_proposal_blocked_by_ingress =
            missing_qc_frontier_self_proposal_ready && frontier_proposal_ingress_deferring;
        let frontier_partial_new_view_support_blocks_missing_qc_fallback =
            missing_qc_frontier_self_proposal_qc.is_some_and(|qc| {
                self.frontier_partial_new_view_support_blocks_qc_fallback(
                    tracked_height,
                    tracked_view,
                    required,
                    &topology,
                    local_peer.as_ref(),
                    qc,
                    frontier_partial_new_view_support,
                )
            });
        let frontier_partial_new_view_support_blocks_precommit_qc_fallback = precommit_qc
            .is_some_and(|qc| {
                self.frontier_partial_new_view_support_blocks_qc_fallback(
                    tracked_height,
                    tracked_view,
                    required,
                    &topology,
                    local_peer.as_ref(),
                    qc,
                    frontier_partial_new_view_support,
                )
            });
        if frontier_partial_new_view_support
            && !frontier_partial_new_view_support_blocks_precommit_qc_fallback
        {
            debug!(
                height = tracked_height,
                view = tracked_view,
                committed_height,
                "partial NEW_VIEW support is compatible with the committed-QC frontier fallback"
            );
        }
        let missing_qc_committed_frontier_fallback_allowed = missing_qc_frontier_self_proposal_qc
            .is_some_and(|qc| {
                self.config.resilience.enabled
                    && tracked_view > 0
                    && pending_queue_len > 0
                    && tracked_height == committed_height.saturating_add(1)
                    && qc.phase == crate::sumeragi::consensus::Phase::Commit
                    && qc.height == committed_height
                    && !frontier_partial_new_view_support_blocks_missing_qc_fallback
                    && !missing_qc_frontier_self_proposal_blocked_by_ingress
                    && (active_cached_frontier_slot
                        || !self.proposal_gated_by_missing_dependencies(tracked_height))
            });
        let queued_committed_frontier_fallback_preconditions =
            queued_committed_frontier_fallback_allowed(
                self.config.resilience.enabled,
                tracked_view,
                pending_queue_len,
                active_pending,
                tracked_height,
                committed_height,
                precommit_qc.is_some_and(|qc| {
                    qc.phase == crate::sumeragi::consensus::Phase::Commit
                        && qc.height == committed_height
                }),
                frontier_partial_new_view_support_blocks_precommit_qc_fallback,
                missing_qc_frontier_self_proposal_blocked_by_ingress,
                true,
            );
        let queued_committed_frontier_fallback_allowed =
            queued_committed_frontier_fallback_preconditions
                && (active_cached_frontier_slot
                    || !self.proposal_gated_by_missing_dependencies(tracked_height));
        let new_view_quorum_free_frontier_proposal_allowed = tracked_view == 0
            || required <= 1
            || missing_qc_committed_frontier_fallback_allowed
            || queued_committed_frontier_fallback_allowed;
        if let Some(view) = current_view {
            // Avoid proposing stale views by pruning NEW_VIEW entries below the local view.
            self.subsystems
                .propose
                .new_view_tracker
                .drop_below_view(tracked_height, view);
        }
        self.reconcile_new_view_tracker_with_local_blocks();
        if pending_queue_len > 0
            && self
                .subsystems
                .propose
                .propose_attempt_monitor
                .should_log(now)
        {
            let since_last_attempt = prev_attempt.map(|ts| now.saturating_duration_since(ts));
            let since_last_success = self
                .subsystems
                .propose
                .last_successful_proposal
                .map(|ts| now.saturating_duration_since(ts));
            iroha_logger::info!(
                height = tracked_height,
                view = current_view,
                view_age_ms = view_age.map(|d| d.as_millis()),
                pending_blocks = self.pending.pending_blocks.len(),
                queue_len = pending_queue_len,
                topology_len = topology.as_ref().len(),
                required,
                online_total,
                since_last_attempt_ms = since_last_attempt.map(|d| d.as_millis()),
                since_last_success_ms = since_last_success.map(|d| d.as_millis()),
                "pacemaker attempt with queued transactions"
            );
        }
        if online_total < required {
            let throttle_hash = tip_hash.unwrap_or_else(|| {
                HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0; Hash::LENGTH]))
            });
            if let Some(suppressed_since_last) = self.proposal_defer_warning_log.allow(
                ProposalDeferWarningKind::InsufficientOnlinePeers,
                tracked_height,
                current_view.unwrap_or_default(),
                throttle_hash,
                now,
                Duration::from_secs(5),
            ) {
                warn!(
                    queue_len = pending_queue_len,
                    height = tracked_height,
                    view = current_view,
                    required,
                    online_peers,
                    online_total,
                    age_ms = view_age.map(|age| age.as_millis()),
                    suppressed_since_last,
                    "online peer count below commit quorum; continuing proposal flow"
                );
            }
            let roster_set: BTreeSet<_> = topology.as_ref().iter().cloned().collect();
            let current_view_idx = current_view.unwrap_or_default();
            let new_view_quorum_at_tracked_height =
                self.subsystems.propose.new_view_tracker.entries.iter().any(
                    |((entry_height, _), entry)| {
                        *entry_height == tracked_height
                            && entry.count_in_roster(&roster_set, local_peer.as_ref()) >= required
                    },
                );
            let exact_new_view_quorum_ready = new_view_quorum_at_tracked_height
                && (pending_queue_len == 0
                    || view_age.is_some_and(|age| age >= self.commit_quorum_timeout()));
            let future_new_view_quorum_observed = tracked_height < desired_height
                && self.subsystems.propose.new_view_tracker.entries.iter().any(
                    |((entry_height, _), entry)| {
                        *entry_height >= tracked_height.saturating_add(1)
                            && entry.count_in_roster(&roster_set, local_peer.as_ref()) >= required
                    },
                );
            let cached_current_slot = self
                .subsystems
                .propose
                .proposal_cache
                .get_proposal(tracked_height, current_view_idx)
                .is_some();
            let missing_qc_recovery_active =
                self.subsystems
                    .propose
                    .proposal_liveness
                    .is_some_and(|slot| {
                        slot.height == tracked_height
                            && slot.view == current_view_idx
                            && matches!(
                                slot.state,
                                ProposalLivenessState::AwaitingProposalAfterMissingQc
                                    | ProposalLivenessState::RecoveryAcquireDependencies
                            )
                    });
            let forced_recovery_view = self
                .subsystems
                .propose
                .forced_view_after_timeout
                .is_some_and(|(forced_height, forced_view)| {
                    forced_height == tracked_height && forced_view >= current_view_idx
                });
            let committed_qc_frontier_recovery_candidate = self.config.resilience.enabled
                && tracked_height == committed_height.saturating_add(1)
                && current_view_idx > 0
                && self
                    .subsystems
                    .propose
                    .new_view_tracker
                    .entries
                    .get(&(tracked_height, current_view_idx))
                    .is_none_or(|entry| entry.senders.is_empty())
                && !self.proposal_gated_by_missing_dependencies(tracked_height)
                && precommit_qc.is_some_and(|qc| tracked_height == qc.height.saturating_add(1));
            let empty_recovery_view = pending_queue_len == 0 && current_view_idx > 0;
            let recovery_or_quorum_evidence = empty_recovery_view
                || exact_new_view_quorum_ready
                || future_new_view_quorum_observed
                || cached_current_slot
                || missing_qc_recovery_active
                || committed_qc_frontier_recovery_candidate
                || forced_recovery_view
                || new_view_quorum_free_frontier_proposal_allowed;
            if required > 1
                && tracked_height == committed_height.saturating_add(1)
                && self.subsystems.propose.last_successful_proposal.is_none()
                && !recovery_or_quorum_evidence
            {
                self.subsystems.propose.pacemaker.next_deadline = now
                    .checked_add(
                        self.subsystems
                            .propose
                            .pacemaker
                            .propose_interval
                            .max(Duration::from_millis(1)),
                    )
                    .unwrap_or(now);
                self.maybe_rebroadcast_new_view_votes(tracked_height, now);
                return false;
            }
        }

        if required == 1 && topology.as_ref().len() == 1 {
            if let Some(local_peer) = local_peer.as_ref() {
                if let Some(qc) = precommit_qc {
                    // Seed NEW_VIEW tracker so single-validator networks can progress.
                    if self.highest_qc.is_none_or(|current| {
                        let incoming = (qc.height, qc.view);
                        let existing = (current.height, current.view);
                        incoming > existing
                            || (incoming == existing
                                && current.phase != crate::sumeragi::consensus::Phase::Commit)
                    }) {
                        self.highest_qc = Some(qc);
                        super::status::set_highest_qc(qc.height, qc.view);
                        super::status::set_highest_qc_hash(qc.subject_block_hash);
                    }
                    self.subsystems.propose.new_view_tracker.record(
                        qc.height.saturating_add(1),
                        0,
                        local_peer.clone(),
                        qc,
                    );
                }
            }
        }

        if pending_queue_len > 0 {
            iroha_logger::debug!(
                queue_len = pending_queue_len,
                topology_len = topology.as_ref().len(),
                required,
                height = tracked_height,
                "pacemaker evaluating proposal assembly with queued transactions"
            );
            if active_pending > 0 {
                iroha_logger::debug!(
                    height = tracked_height,
                    pending = active_pending,
                    "pending block already assembled for current slot; waiting for view-change"
                );
            }
        }
        let new_view_summary: Vec<String> = self
            .subsystems
            .propose
            .new_view_tracker
            .entries
            .iter()
            .map(|((h, v), entry)| format!("{h}:{v}={}", entry.senders.len()))
            .collect();
        debug!(
            height = tracked_height,
            required,
            local_idx = ?local_idx,
            forced = ?self.subsystems.propose.forced_view_after_timeout,
            new_view_slots = ?new_view_summary,
            "pacemaker NEW_VIEW snapshot before selection"
        );

        let mut candidate = self
            .subsystems
            .propose
            .new_view_tracker
            .select_with_quorum_for_height(
                tracked_height,
                required,
                local_peer.as_ref(),
                topology.as_ref(),
            );
        if pending_queue_len > 0 {
            if let Some((forced_height, forced_view)) =
                self.subsystems.propose.forced_view_after_timeout
            {
                if let Some(qc) = precommit_qc {
                    let should_override = candidate.as_ref().is_none_or(|selection| {
                        selection.key.0 != forced_height || selection.key.1 < forced_view
                    });
                    if forced_height == qc.height.saturating_add(1) && should_override {
                        candidate = Some(NewViewSelection {
                            key: (forced_height, forced_view),
                            quorum: required,
                            highest_qc: qc,
                        });
                        self.subsystems.propose.forced_view_after_timeout = None;
                    }
                }
            }
        }

        if candidate.is_none() {
            if let Some((forced_height, forced_view)) =
                self.subsystems.propose.forced_view_after_timeout
            {
                if let Some(qc) = precommit_qc {
                    if forced_height == qc.height.saturating_add(1) {
                        candidate = Some(NewViewSelection {
                            key: (forced_height, forced_view),
                            quorum: required,
                            highest_qc: qc,
                        });
                        self.subsystems.propose.forced_view_after_timeout = None;
                    }
                }
            }
        }

        if candidate.is_none() && frontier_proposal_ingress_deferring {
            self.maybe_rebroadcast_new_view_votes(tracked_height, now);
            self.subsystems.propose.pacemaker.next_deadline = now
                .checked_add(PACEMAKER_QUEUE_NUDGE_MIN_INTERVAL)
                .unwrap_or(now);
            debug!(
                height = tracked_height,
                view = tracked_view,
                committed_height,
                queue_len = pending_queue_len,
                vote_rx_depth = queue_depths.vote_rx,
                block_payload_rx_depth = queue_depths.block_payload_rx,
                rbc_chunk_rx_depth = queue_depths.rbc_chunk_rx,
                block_rx_depth = queue_depths.block_rx,
                "deferring committed-QC frontier fallback while proposal ingress drains"
            );
            return false;
        }

        if candidate.is_none()
            && !frontier_partial_new_view_support_blocks_missing_qc_fallback
            && let Some(qc) = self.missing_qc_liveness_allows_frontier_self_proposal(
                tracked_height,
                tracked_view,
                committed_height,
                pending_queue_len,
                precommit_qc,
            )
        {
            candidate = Some(NewViewSelection {
                key: (tracked_height, tracked_view),
                quorum: required,
                highest_qc: qc,
            });
            debug!(
                height = tracked_height,
                view = tracked_view,
                committed_height,
                qc_height = qc.height,
                qc_view = qc.view,
                "using committed-QC frontier candidate after missing-QC no-proposal liveness timeout"
            );
        }
        if candidate.is_none()
            && self.config.resilience.enabled
            && tracked_height == committed_height.saturating_add(1)
            && tracked_view > 0
            && !frontier_partial_new_view_support_blocks_precommit_qc_fallback
            && self
                .subsystems
                .propose
                .new_view_tracker
                .entries
                .get(&(tracked_height, tracked_view))
                .is_none_or(|entry| entry.senders.is_empty())
            && !self.proposal_gated_by_missing_dependencies(tracked_height)
            && let Some(qc) = precommit_qc
            && tracked_height == qc.height.saturating_add(1)
        {
            self.maybe_rebroadcast_new_view_votes(tracked_height, now);
            candidate = Some(NewViewSelection {
                key: (tracked_height, tracked_view),
                quorum: required,
                highest_qc: qc,
            });
            if let Some(suppressed_since_last) = self.proposal_defer_warning_log.allow(
                ProposalDeferWarningKind::CommittedQcNoNewViewFallback,
                tracked_height,
                tracked_view,
                qc.subject_block_hash,
                now,
                Duration::from_secs(2),
            ) {
                warn!(
                    height = tracked_height,
                    view = tracked_view,
                    committed_height,
                    qc_height = qc.height,
                    qc_view = qc.view,
                    queue_len = pending_queue_len,
                    suppressed_since_last,
                    "using committed-QC frontier candidate without NEW_VIEW quorum under resilience liveness pressure"
                );
            }
        }

        if candidate.is_none()
            && tracked_height < desired_height
            && let Some(future_selection) = self
                .subsystems
                .propose
                .new_view_tracker
                .select_with_quorum_at_or_above_height(
                    tracked_height.saturating_add(1),
                    required,
                    local_peer.as_ref(),
                    topology.as_ref(),
                )
        {
            let highest_qc_observed =
                self.observe_new_view_highest_qc_exact_repair(future_selection.highest_qc);
            let reanchor_requested = self.request_range_pull_from_anchor(
                tracked_height,
                FUTURE_NEW_VIEW_FRONTIER_REANCHOR_REASON,
                now,
            );
            info!(
                height = tracked_height,
                desired_height,
                future_height = future_selection.key.0,
                future_view = future_selection.key.1,
                future_quorum = future_selection.quorum,
                future_highest_qc_height = future_selection.highest_qc.height,
                future_highest_qc_view = future_selection.highest_qc.view,
                highest_qc_observed,
                reanchor_requested,
                "deferring proposal: future NEW_VIEW quorum observed, reanchoring local frontier"
            );
            self.maybe_rebroadcast_new_view_votes(tracked_height, now);
            return false;
        }

        let Some(selection) = candidate.or_else(|| {
            // Fallback: bootstrap the first view using the latest committed QC when no NEW_VIEW
            // quorum has been observed yet. This prevents the pacemaker from stalling indefinitely
            // at startup before any view changes occur.
            if !new_view_quorum_free_frontier_proposal_allowed {
                return None;
            }
            let _local_idx = local_idx?;
            let qc = missing_qc_frontier_self_proposal_qc.or_else(|| self.latest_committed_qc())?;
            Some(NewViewSelection {
                key: (
                    qc.height.saturating_add(1),
                    if bootstrap_view { 0 } else { tracked_view },
                ),
                quorum: required,
                highest_qc: qc,
            })
        }) else {
            debug!(
                queue_len = pending_queue_len,
                height = tracked_height,
                required,
                local_idx = ?local_idx,
                new_view_slots = ?new_view_summary,
                "deferring proposal: awaiting NEW_VIEW quorum"
            );
            self.maybe_rebroadcast_new_view_votes(tracked_height, now);
            return false;
        };

        let (height, view_idx) = selection.key;
        let quorum = selection.quorum;
        let mut highest_qc = selection.highest_qc;
        let _ = self.promote_committed_qc_for_frontier_selection(
            height,
            &mut highest_qc,
            "proposal_selection",
        );

        debug!(
            height,
            view = view_idx,
            quorum,
            local_idx = ?local_idx,
            highest_height = highest_qc.height,
            highest_view = highest_qc.view,
            new_view_slots = ?new_view_summary,
            "selected NEW_VIEW candidate"
        );
        if height == self.committed_height_snapshot().saturating_add(1)
            && let Some(current_view) = self.phase_tracker.current_view(height)
            && view_idx > current_view
        {
            let future_window = self.config.gating.future_view_window;
            if future_window == 0 || view_idx <= current_view.saturating_add(future_window) {
                self.phase_tracker.on_view_change(height, view_idx, now);
                self.subsystems.propose.pacemaker.next_deadline = now;
                let min_view = if future_window == 0 {
                    view_idx
                } else {
                    view_idx.saturating_sub(future_window)
                };
                self.subsystems
                    .propose
                    .new_view_tracker
                    .drop_below_view(height, min_view);
                self.prune_stale_view_state(height, view_idx);
                super::status::set_view_change_index(view_idx);
                if let Some(telemetry) = self.telemetry_handle() {
                    telemetry.set_view_changes(view_idx);
                    telemetry.inc_view_change_install();
                }
                super::status::inc_view_change_install();
                info!(
                    height,
                    selected_view = view_idx,
                    local_view = current_view,
                    quorum,
                    highest_height = highest_qc.height,
                    highest_view = highest_qc.view,
                    highest_block = %highest_qc.subject_block_hash,
                    "adopting selected NEW_VIEW quorum before proposal leader evaluation"
                );
            } else {
                debug!(
                    height,
                    selected_view = view_idx,
                    local_view = current_view,
                    future_window,
                    "selected NEW_VIEW quorum is outside the configured future-view window"
                );
            }
        }
        let epoch = self.epoch_for_height(height);
        let precommit_votes_at_view = self
            .vote_log
            .values()
            .filter(|vote| {
                self.precommit_vote_blocks_proposal_assembly(vote, height, view_idx, epoch)
            })
            .count();

        // Avoid rebuilding multiple proposals for the same (height, view) slot. Reassembly in the
        // same view causes double-voting and scatters QC collection; wait for the existing
        // proposal to gather votes or transition via a view change instead.
        if let Some(cached_payload_hash) = self
            .subsystems
            .propose
            .proposal_cache
            .get_proposal(height, view_idx)
            .map(|proposal| proposal.payload_hash)
        {
            let cached_owner_hint = self
                .subsystems
                .propose
                .proposal_cache
                .get_hint(height, view_idx)
                .map(|hint| hint.block_hash)
                .or_else(|| self.authoritative_slot_owner_hash(height, view_idx))
                .or_else(|| {
                    self.frontier_slot
                        .as_ref()
                        .filter(|slot| slot.height == height && slot.view == view_idx)
                        .map(|slot| slot.block_hash)
                });
            // Rebroadcast cached proposals when the leader is still responsible for the slot so
            // peers that missed the initial messages can recover without forcing a view change.
            let _ =
                self.maybe_rebroadcast_cached_proposal(height, view_idx, pending_queue_len, now);
            if precommit_votes_at_view > 0 {
                debug!(
                    height,
                    view = view_idx,
                    precommit_votes = precommit_votes_at_view,
                    queue_len = pending_queue_len,
                    "proposal already cached for this slot; precommit votes observed, continuing cached-slot liveness checks"
                );
            }
            let quorum_timeout = self.quorum_timeout(da_enabled);
            let queue_depths = super::status::worker_queue_depth_snapshot();
            let consensus_queue_backlog =
                self.consensus_queue_backlog_blocks_near_quorum_timeout(queue_depths);
            let (consensus_mode, _, _) = self.consensus_context_for_height(height);
            let mut commit_roster = self.roster_for_live_vote_with_mode(height, consensus_mode);
            if commit_roster.is_empty() {
                commit_roster = self.effective_commit_topology();
            }
            let commit_topology = super::network_topology::Topology::new(commit_roster);
            let mut missing_local_data = false;
            let mut rbc_session_incomplete = false;
            for pending in self.pending.pending_blocks.values().filter(|pending| {
                !pending.aborted && pending.height == height && pending.view == view_idx
            }) {
                let payload_available = da_enabled && self.payload_available_for_da(pending);
                if !da_enabled || payload_available {
                    continue;
                }
                missing_local_data = true;
                let rbc_key = (pending.block.hash(), pending.height, pending.view);
                let pending_entry = self.subsystems.da_rbc.rbc.pending.contains_key(&rbc_key);
                let required_ready = Self::rbc_protocol_deliver_quorum(&commit_topology);
                rbc_session_incomplete |= self
                    .subsystems
                    .da_rbc
                    .rbc
                    .sessions
                    .get(&rbc_key)
                    .is_some_and(|session| {
                        rbc_session_availability_incomplete(session, pending_entry, required_ready)
                    });
                if rbc_session_incomplete {
                    break;
                }
            }
            let mut effective_quorum_timeout = cached_slot_effective_quorum_timeout(
                quorum_timeout,
                self.rebroadcast_cooldown(),
                precommit_votes_at_view,
                quorum,
                missing_local_data,
                consensus_queue_backlog,
                rbc_session_incomplete,
            );
            if pending_queue_len > 0 && precommit_votes_at_view == 0 {
                let capped = self.cap_active_block_production_gap(effective_quorum_timeout, true);
                if capped < effective_quorum_timeout {
                    debug!(
                        height,
                        view = view_idx,
                        queue_len = pending_queue_len,
                        quorum_timeout_ms = effective_quorum_timeout.as_millis(),
                        capped_timeout_ms = capped.as_millis(),
                        "capping cached proposal wait under active transaction backlog"
                    );
                    effective_quorum_timeout = capped;
                }
            }
            let mut live_same_slot_pending = 0usize;
            let cached_wait_age = self
                .pending
                .pending_blocks
                .values()
                .filter(|pending| {
                    if pending.aborted || pending.height != height || pending.view != view_idx {
                        return false;
                    }
                    live_same_slot_pending = live_same_slot_pending.saturating_add(1);
                    pending.payload_hash == cached_payload_hash
                        && cached_owner_hint.is_none_or(|hint| pending.block.hash() == hint)
                })
                .map(|pending| {
                    pending
                        .progress_age(now)
                        .max(now.saturating_duration_since(pending.inserted_at))
                })
                .max();
            let cached_pending_present = cached_wait_age.is_some();
            if !cached_pending_present && live_same_slot_pending > 0 {
                let dropped_proposal = self
                    .subsystems
                    .propose
                    .proposal_cache
                    .pop_proposal(height, view_idx)
                    .is_some();
                let dropped_hint = self
                    .subsystems
                    .propose
                    .proposal_cache
                    .pop_hint(height, view_idx)
                    .is_some();
                let progressed = self.maybe_progress_existing_slot_proposal(
                    height,
                    view_idx,
                    pending_queue_len,
                    now,
                    "cached_proposal_metadata_mismatch",
                );
                warn!(
                    height,
                    view = view_idx,
                    queue_len = pending_queue_len,
                    live_same_slot_pending,
                    cached_payload = ?cached_payload_hash,
                    cached_block = ?cached_owner_hint,
                    dropped_proposal,
                    dropped_hint,
                    progressed_existing_slot = progressed,
                    "cached proposal metadata does not match live pending body; dropping stale metadata"
                );
                self.warn_resilience_frontier_proposal_deferred(
                    height,
                    view_idx,
                    "cached_proposal_metadata_mismatch",
                    highest_qc,
                    pending_queue_len,
                    now,
                );
                return false;
            }
            if !cached_pending_present
                && self.config.resilience.enabled
                && height == self.committed_height_snapshot().saturating_add(1)
                && (view_idx > 0 || pending_queue_len > 0)
            {
                let cached_hint = self
                    .subsystems
                    .propose
                    .proposal_cache
                    .get_hint(height, view_idx)
                    .cloned();
                let base_repair_window = self
                    .frontier_slot_lag_window()
                    .max(self.recovery_deferred_qc_ttl())
                    .max(quorum_timeout)
                    .max(self.rebroadcast_cooldown())
                    .max(PACEMAKER_QUEUE_NUDGE_MIN_INTERVAL);
                let cached_body_recovery_active = cached_hint.as_ref().is_some_and(|hint| {
                    let session_key = Self::session_key(&hint.block_hash, height, view_idx);
                    let exact_repair_active = self.frontier_slot.as_ref().is_some_and(|slot| {
                        slot.height == height
                            && slot.view == view_idx
                            && slot.block_hash == hint.block_hash
                            && slot.exact_fetch_armed
                            && !slot.body_present()
                    });
                    let rbc_pending_active = self
                        .subsystems
                        .da_rbc
                        .rbc
                        .pending
                        .contains_key(&session_key);
                    let rbc_session_active = self
                        .subsystems
                        .da_rbc
                        .rbc
                        .sessions
                        .get(&session_key)
                        .is_some_and(|session| {
                            !session.is_invalid()
                                && (session.received_chunks() > 0
                                    || !session.ready_signatures.is_empty()
                                    || session.sent_ready
                                    || session.delivered
                                    || session.progress_stage()
                                        > RbcProgressStage::CollectingChunks)
                        });
                    exact_repair_active || rbc_pending_active || rbc_session_active
                });
                let active_backlog_without_precommit =
                    pending_queue_len > 0 && precommit_votes_at_view == 0;
                let repair_window = if cached_body_recovery_active
                    && !active_backlog_without_precommit
                {
                    base_repair_window
                } else {
                    self.cap_active_block_production_gap(base_repair_window, pending_queue_len > 0)
                };
                if repair_window < base_repair_window {
                    debug!(
                        height,
                        view = view_idx,
                        queue_len = pending_queue_len,
                        precommit_votes_at_view,
                        repair_window_ms = base_repair_window.as_millis(),
                        capped_repair_window_ms = repair_window.as_millis(),
                        "capping cached proposal body repair window under active transaction backlog"
                    );
                } else if cached_body_recovery_active && pending_queue_len > 0 {
                    debug!(
                        height,
                        view = view_idx,
                        queue_len = pending_queue_len,
                        repair_window_ms = repair_window.as_millis(),
                        "using full cached proposal body repair window while recovery is active"
                    );
                }
                let cache_age = self
                    .subsystems
                    .propose
                    .proposal_cache
                    .observed_at(height, view_idx)
                    .map(|observed_at| now.saturating_duration_since(observed_at));
                if let Some(hint) = cached_hint.as_ref() {
                    let validation_inflight = self
                        .subsystems
                        .validation
                        .inflight
                        .contains_key(&hint.block_hash);
                    let commit_inflight =
                        self.subsystems
                            .commit
                            .inflight
                            .as_ref()
                            .is_some_and(|inflight| {
                                inflight.block_hash == hint.block_hash
                                    && !inflight.pending.aborted
                                    && inflight.pending.height == height
                                    && inflight.pending.view == view_idx
                            });
                    let pending_processing = self
                        .pending
                        .pending_processing
                        .get()
                        .is_some_and(|processing| processing == hint.block_hash);
                    let deferred_body = self.deferred_block_sync_updates.keys().any(
                        |(deferred_height, deferred_view, deferred_hash)| {
                            *deferred_height == height
                                && *deferred_view == view_idx
                                && *deferred_hash == hint.block_hash
                        },
                    );
                    let pending_validation = self.recent_pending_validation_for_slot(
                        height,
                        view_idx,
                        Some(hint.block_hash),
                        now,
                        repair_window,
                    );
                    let pending_processing_only = pending_processing
                        && !validation_inflight
                        && !commit_inflight
                        && !deferred_body;
                    let stale_pending_processing_only = pending_processing_only
                        && cache_age.is_some_and(|age| {
                            repair_window != Duration::ZERO && age >= repair_window
                        });
                    if (validation_inflight
                        || commit_inflight
                        || pending_processing
                        || deferred_body
                        || pending_validation.is_some())
                        && !stale_pending_processing_only
                    {
                        self.subsystems.propose.pacemaker.next_deadline = now
                            .checked_add(PACEMAKER_QUEUE_NUDGE_MIN_INTERVAL)
                            .unwrap_or(now);
                        debug!(
                            height,
                            view = view_idx,
                            block = %hint.block_hash,
                            validation_inflight,
                            commit_inflight,
                            pending_processing,
                            deferred_body,
                            pending_validation_age_ms =
                                pending_validation.map(|(_, age)| age.as_millis()),
                            queue_len = pending_queue_len,
                            "cached proposal has no live pending body but local processing still owns it; deferring rotation"
                        );
                        self.maybe_rebroadcast_new_view_votes(height, now);
                        self.warn_resilience_frontier_proposal_deferred(
                            height,
                            view_idx,
                            "cached_proposal_local_processing",
                            highest_qc,
                            pending_queue_len,
                            now,
                        );
                        return false;
                    }
                    if stale_pending_processing_only {
                        warn!(
                            height,
                            view = view_idx,
                            block = %hint.block_hash,
                            queue_len = pending_queue_len,
                            repair_window_ms = repair_window.as_millis(),
                            cache_age_ms = cache_age.map(|age| age.as_millis()),
                            "cached proposal has no live pending body; ignoring stale pending-processing marker before rotation"
                        );
                    }
                }
                let repair_age = cached_hint.as_ref().and_then(|hint| {
                    self.frontier_slot.as_ref().and_then(|slot| {
                        (slot.height == height
                            && slot.view == view_idx
                            && slot.block_hash == hint.block_hash
                            && slot.exact_fetch_armed
                            && !slot.body_present())
                        .then(|| now.saturating_duration_since(slot.lag_started_at()))
                    })
                });
                let repair_exhausted = repair_age
                    .is_some_and(|age| repair_window != Duration::ZERO && age >= repair_window);
                if let Some(hint) = cached_hint.as_ref().filter(|_| !repair_exhausted) {
                    let seeded = self.handle_frontier_body_gap_with_topology(
                        hint.block_hash,
                        height,
                        view_idx,
                        &BTreeSet::new(),
                        &commit_topology,
                        true,
                        now,
                    );
                    let fetch_requested = self.emit_frontier_block_body_fetch_urgent(now);
                    let repair_active = repair_age.is_some()
                        || self.frontier_slot.as_ref().is_some_and(|slot| {
                            slot.height == height
                                && slot.view == view_idx
                                && slot.block_hash == hint.block_hash
                                && slot.exact_fetch_armed
                                && !slot.body_present()
                        });
                    if repair_active {
                        debug!(
                            height,
                            view = view_idx,
                            block = %hint.block_hash,
                            queue_len = pending_queue_len,
                            repair_window_ms = repair_window.as_millis(),
                            repair_age_ms = repair_age.map(|age| age.as_millis()),
                            seeded_frontier_body_repair = seeded,
                            fetch_requested,
                            "cached proposal has no live pending body; requesting exact body repair before rotating"
                        );
                        self.maybe_rebroadcast_new_view_votes(height, now);
                        self.warn_resilience_frontier_proposal_deferred(
                            height,
                            view_idx,
                            "cached_proposal_body_repair",
                            highest_qc,
                            pending_queue_len,
                            now,
                        );
                        return false;
                    }
                    debug!(
                        height,
                        view = view_idx,
                        block = %hint.block_hash,
                        queue_len = pending_queue_len,
                        repair_window_ms = repair_window.as_millis(),
                        repair_age_ms = repair_age.map(|age| age.as_millis()),
                        seeded_frontier_body_repair = seeded,
                        fetch_requested,
                        "cached proposal has no live pending body and no active exact repair; rotating recovery view"
                    );
                }
                if (cached_hint.is_some() || pending_queue_len == 0)
                    && !repair_exhausted
                    && cache_age.is_some_and(|age| age < repair_window)
                {
                    self.subsystems.propose.pacemaker.next_deadline = now
                        .checked_add(PACEMAKER_QUEUE_NUDGE_MIN_INTERVAL)
                        .unwrap_or(now);
                    debug!(
                        height,
                        view = view_idx,
                        queue_len = pending_queue_len,
                        repair_window_ms = repair_window.as_millis(),
                        cache_age_ms = cache_age.map(|age| age.as_millis()),
                        "cached proposal has no live pending body but was just observed; deferring rotation"
                    );
                    self.maybe_rebroadcast_new_view_votes(height, now);
                    self.warn_resilience_frontier_proposal_deferred(
                        height,
                        view_idx,
                        "cached_proposal_body_materializing",
                        highest_qc,
                        pending_queue_len,
                        now,
                    );
                    return false;
                }
                let dropped_proposal = self
                    .subsystems
                    .propose
                    .proposal_cache
                    .pop_proposal(height, view_idx)
                    .is_some();
                let dropped_hint = self
                    .subsystems
                    .propose
                    .proposal_cache
                    .pop_hint(height, view_idx)
                    .is_some();
                let dropped_proposal_seen =
                    self.clear_exhausted_frontier_proposal_marker(height, view_idx);
                warn!(
                    height,
                    view = view_idx,
                    queue_len = pending_queue_len,
                    dropped_proposal,
                    dropped_hint,
                    dropped_proposal_seen,
                    repair_window_ms = repair_window.as_millis(),
                    repair_age_ms = repair_age.map(|age| age.as_millis()),
                    cache_age_ms = cache_age.map(|age| age.as_millis()),
                    "cached proposal has no live pending body; rotating recovery view"
                );
                self.apply_view_change_after_exhausted_frontier_recovery(
                    height,
                    view_idx,
                    ViewChangeCause::QuorumTimeout,
                );
                self.maybe_rebroadcast_new_view_votes(height, now);
                self.warn_resilience_frontier_proposal_deferred(
                    height,
                    view_idx,
                    "cached_proposal_without_pending",
                    highest_qc,
                    pending_queue_len,
                    now,
                );
                return false;
            }
            if effective_quorum_timeout != Duration::ZERO
                && cached_wait_age.is_some_and(|age| age >= effective_quorum_timeout)
            {
                let wait_age_ms = cached_wait_age.map(|age| age.as_millis()).unwrap_or(0);
                let already_forced = self
                    .subsystems
                    .propose
                    .forced_view_after_timeout
                    .is_some_and(|(forced_height, forced_view)| {
                        forced_height == height && forced_view > view_idx
                    });
                if already_forced {
                    debug!(
                        height,
                        view = view_idx,
                        queue_len = pending_queue_len,
                        wait_age_ms,
                        quorum_timeout_ms = effective_quorum_timeout.as_millis(),
                        base_quorum_timeout_ms = quorum_timeout.as_millis(),
                        forced = ?self.subsystems.propose.forced_view_after_timeout,
                        "cached proposal slot stalled past quorum timeout; awaiting scheduled view change"
                    );
                } else if let Some(wait_remaining) = cached_slot_timeout_hysteresis_remaining(
                    consensus_mode,
                    effective_quorum_timeout,
                    self.subsystems.propose.last_cached_slot_timeout_trigger,
                    height,
                    view_idx,
                    now,
                ) {
                    let next_streak = next_cached_slot_timeout_streak(
                        self.subsystems.propose.last_cached_slot_timeout_trigger,
                        height,
                        view_idx,
                    );
                    debug!(
                        height,
                        view = view_idx,
                        queue_len = pending_queue_len,
                        wait_age_ms,
                        quorum_timeout_ms = effective_quorum_timeout.as_millis(),
                        base_quorum_timeout_ms = quorum_timeout.as_millis(),
                        hysteresis_wait_ms = wait_remaining.as_millis(),
                        timeout_streak = next_streak,
                        "cached proposal slot stalled past quorum timeout; waiting for NPoS timeout hysteresis"
                    );
                } else {
                    let timeout_streak = next_cached_slot_timeout_streak(
                        self.subsystems.propose.last_cached_slot_timeout_trigger,
                        height,
                        view_idx,
                    );
                    let committed_height = self.committed_height_snapshot();
                    let contiguous_frontier = height == committed_height.saturating_add(1);
                    let same_slot_recovery_active = contiguous_frontier
                        && self.frontier_recovery_quorum_timeout_same_height_recovery_active(
                            height,
                            view_idx,
                            now,
                            queue_depths,
                        );
                    if same_slot_recovery_active {
                        let seeded = self
                            .seed_frontier_recovery_for_quorum_timeout_without_local_pending(
                                height, view_idx, now,
                            );
                        let recovery_advance = self.advance_frontier_recovery(
                            "quorum_timeout",
                            height,
                            view_idx,
                            false,
                            true,
                            true,
                            now,
                        );
                        debug!(
                            height,
                            view = view_idx,
                            queue_len = pending_queue_len,
                            wait_age_ms,
                            quorum_timeout_ms = effective_quorum_timeout.as_millis(),
                            base_quorum_timeout_ms = quorum_timeout.as_millis(),
                            timeout_streak,
                            seeded_frontier_owner = seeded,
                            ?recovery_advance,
                            "cached proposal slot quorum-timeout routed through same-slot frontier recovery"
                        );
                    } else if contiguous_frontier {
                        warn!(
                            height,
                            view = view_idx,
                            queue_len = pending_queue_len,
                            wait_age_ms,
                            quorum_timeout_ms = effective_quorum_timeout.as_millis(),
                            base_quorum_timeout_ms = quorum_timeout.as_millis(),
                            timeout_streak,
                            "cached proposal slot stalled past quorum timeout; routing through unified frontier recovery"
                        );
                        self.seed_frontier_recovery_for_quorum_timeout_without_local_pending(
                            height, view_idx, now,
                        );
                        let _ = self.advance_frontier_recovery(
                            "quorum_timeout",
                            height,
                            view_idx,
                            false,
                            true,
                            true,
                            now,
                        );
                    } else {
                        warn!(
                            height,
                            view = view_idx,
                            queue_len = pending_queue_len,
                            wait_age_ms,
                            quorum_timeout_ms = effective_quorum_timeout.as_millis(),
                            base_quorum_timeout_ms = quorum_timeout.as_millis(),
                            timeout_streak,
                            "cached proposal slot stalled past quorum timeout; forcing view change"
                        );
                        self.trigger_view_change_with_cause(
                            height,
                            view_idx,
                            ViewChangeCause::QuorumTimeout,
                        );
                    }
                    self.subsystems.propose.last_cached_slot_timeout_trigger =
                        Some(CachedSlotTimeoutTrigger {
                            height,
                            view: view_idx,
                            at: now,
                            streak: timeout_streak,
                        });
                }
                self.maybe_rebroadcast_new_view_votes(height, now);
                self.warn_resilience_frontier_proposal_deferred(
                    height,
                    view_idx,
                    "cached_proposal_quorum_timeout",
                    highest_qc,
                    pending_queue_len,
                    now,
                );
                return false;
            }
            if pending_queue_len > 0 {
                debug!(
                    height,
                    view = view_idx,
                    queue_len = pending_queue_len,
                    "proposal already cached for this slot; waiting for votes/view change"
                );
                // Provide one cooldown-gated NEW_VIEW assist while waiting so peers that
                // missed prior messages can converge without immediate view rotation.
                self.maybe_rebroadcast_new_view_votes(height, now);
            } else {
                trace!(
                    height,
                    view = view_idx,
                    "proposal already cached for this slot; deferring reassembly"
                );
            }
            self.warn_resilience_frontier_proposal_deferred(
                height,
                view_idx,
                "cached_proposal_waiting",
                highest_qc,
                pending_queue_len,
                now,
            );
            return false;
        }

        if precommit_votes_at_view > 0 {
            debug!(
                height,
                view = view_idx,
                precommit_votes = precommit_votes_at_view,
                queue_len = pending_queue_len,
                "deferring proposal: precommit votes already observed for this view"
            );
            self.warn_resilience_frontier_proposal_deferred(
                height,
                view_idx,
                "precommit_votes_observed",
                highest_qc,
                pending_queue_len,
                now,
            );
            return false;
        }

        if height == self.committed_height_snapshot().saturating_add(1) && view_idx > 0 {
            let queue_depths = super::status::worker_queue_depth_snapshot();
            let ingress_grace = self.frontier_ingress_drain_grace(da_enabled);
            let selected_frontier_recovery_candidate = self.config.resilience.enabled
                && highest_qc.phase == crate::sumeragi::consensus::Phase::Commit
                && highest_qc.height.saturating_add(1) == height;
            let vote_ingress_starvation_override = selected_frontier_recovery_candidate
                && queue_depths.vote_rx > 0
                && queue_depths.block_payload_rx == 0
                && queue_depths.rbc_chunk_rx == 0
                && queue_depths.block_rx == 0
                && self.frontier_proposal_or_view_starved_past_ingress_grace(
                    height,
                    now,
                    ingress_grace,
                );
            let frontier_recovery_ingress_override = self
                .frontier_recovery_ingress_override_active(height, view_idx, now, ingress_grace)
                || vote_ingress_starvation_override;
            if Self::frontier_consensus_ingress_queued(queue_depths) {
                if !frontier_recovery_ingress_override
                    && self.frontier_proposal_ingress_defer_active(
                        height,
                        view_idx,
                        now,
                        queue_depths,
                        ingress_grace,
                    )
                {
                    self.subsystems.propose.pacemaker.next_deadline = now
                        .checked_add(PACEMAKER_QUEUE_NUDGE_MIN_INTERVAL)
                        .unwrap_or(now);
                    debug!(
                        height,
                        view = view_idx,
                        vote_rx_depth = queue_depths.vote_rx,
                        block_payload_rx_depth = queue_depths.block_payload_rx,
                        rbc_chunk_rx_depth = queue_depths.rbc_chunk_rx,
                        block_rx_depth = queue_depths.block_rx,
                        queue_len = pending_queue_len,
                        "deferring fresh frontier proposal while proposal ingress drains"
                    );
                    self.warn_resilience_frontier_proposal_deferred(
                        height,
                        view_idx,
                        "proposal_ingress_draining",
                        highest_qc,
                        pending_queue_len,
                        now,
                    );
                    return false;
                }
                let view_age = self.phase_tracker.view_age(height, now).unwrap_or_default();
                let proposal_starved =
                    self.frontier_proposal_starved_past_ingress_grace(now, ingress_grace);
                if view_age < ingress_grace && !proposal_starved {
                    self.subsystems.propose.pacemaker.next_deadline = now
                        .checked_add(PACEMAKER_QUEUE_NUDGE_MIN_INTERVAL)
                        .unwrap_or(now);
                    debug!(
                        height,
                        view = view_idx,
                        view_age_ms = view_age.as_millis(),
                        ingress_grace_ms = ingress_grace.as_millis(),
                        vote_rx_depth = queue_depths.vote_rx,
                        block_payload_rx_depth = queue_depths.block_payload_rx,
                        rbc_chunk_rx_depth = queue_depths.rbc_chunk_rx,
                        block_rx_depth = queue_depths.block_rx,
                        queue_len = pending_queue_len,
                        "deferring fresh frontier proposal while consensus ingress drains"
                    );
                    self.warn_resilience_frontier_proposal_deferred(
                        height,
                        view_idx,
                        "consensus_ingress_draining",
                        highest_qc,
                        pending_queue_len,
                        now,
                    );
                    return false;
                }
                if view_age < ingress_grace {
                    debug!(
                        height,
                        view = view_idx,
                        view_age_ms = view_age.as_millis(),
                        ingress_grace_ms = ingress_grace.as_millis(),
                        vote_rx_depth = queue_depths.vote_rx,
                        block_payload_rx_depth = queue_depths.block_payload_rx,
                        rbc_chunk_rx_depth = queue_depths.rbc_chunk_rx,
                        block_rx_depth = queue_depths.block_rx,
                        queue_len = pending_queue_len,
                        "allowing fresh frontier proposal after proposal starvation despite queued consensus ingress"
                    );
                }
            }
        }

        if height == self.committed_height_snapshot().saturating_add(1)
            && (self.slot_has_proposal_evidence(height, view_idx)
                || self
                    .frontier_slot_live_local_owner_for_round(height, view_idx)
                    .is_some()
                || self.slot_has_actionable_vote_backed_proposal_evidence(height, view_idx, epoch))
        {
            let _ = self.seed_frontier_slot_from_same_height_evidence(
                height,
                view_idx,
                now,
                "missing_qc",
                true,
            );
        }

        if self.same_height_frontier_owner_blocks_proposal(
            height,
            view_idx,
            pending_queue_len,
            now,
            highest_qc,
        ) {
            return false;
        }

        if let Some(block_hash) = self.authoritative_slot_owner_hash(height, view_idx) {
            let progressed = self.maybe_progress_existing_slot_proposal(
                height,
                view_idx,
                pending_queue_len,
                now,
                "authoritative_slot_owner",
            );
            if !progressed {
                self.nudge_frontier_recovery_proposal_retry(now);
            }
            if pending_queue_len > 0 {
                debug!(
                    height,
                    view = view_idx,
                    queue_len = pending_queue_len,
                    block = %block_hash,
                    "authoritative BlockCreated already owns this slot; waiting for progress"
                );
            } else {
                trace!(
                    height,
                    view = view_idx,
                    block = %block_hash,
                    "authoritative BlockCreated already owns this slot; deferring reassembly"
                );
            }
            self.warn_resilience_frontier_proposal_deferred(
                height,
                view_idx,
                "authoritative_slot_owner",
                highest_qc,
                pending_queue_len,
                now,
            );
            return false;
        }

        if self.slot_has_proposal_evidence(height, view_idx) {
            if let Some((view_age, stale_window)) = self
                .stale_proposals_seen_only_slot_allows_recovery_rotation(
                    height,
                    view_idx,
                    epoch,
                    now,
                    precommit_votes_at_view,
                )
            {
                self.clear_exhausted_frontier_proposal_marker(height, view_idx);
                warn!(
                    height,
                    view = view_idx,
                    queue_len = pending_queue_len,
                    view_age_ms = view_age.as_millis(),
                    stale_window_ms = stale_window.as_millis(),
                    "proposal-seen marker has no materialized slot owner; rotating recovery view"
                );
                self.apply_view_change_after_exhausted_frontier_recovery(
                    height,
                    view_idx,
                    ViewChangeCause::QuorumTimeout,
                );
                self.maybe_rebroadcast_new_view_votes(height, now);
                self.warn_resilience_frontier_proposal_deferred(
                    height,
                    view_idx,
                    "proposal_seen_without_materialized_owner",
                    highest_qc,
                    pending_queue_len,
                    now,
                );
                return false;
            }
            let progressed = self.maybe_progress_existing_slot_proposal(
                height,
                view_idx,
                pending_queue_len,
                now,
                "slot_has_proposal_evidence",
            );
            if !progressed {
                if let Some((view_age, stale_window)) = self
                    .stale_slot_proposal_evidence_allows_recovery_rotation(
                        height,
                        view_idx,
                        epoch,
                        now,
                        precommit_votes_at_view,
                        pending_queue_len,
                        highest_qc,
                    )
                {
                    self.clear_exhausted_frontier_proposal_marker(height, view_idx);
                    warn!(
                        height,
                        view = view_idx,
                        queue_len = pending_queue_len,
                        view_age_ms = view_age.as_millis(),
                        stale_window_ms = stale_window.as_millis(),
                        "proposal evidence made no local progress; rotating recovery view"
                    );
                    self.apply_view_change_after_exhausted_frontier_recovery(
                        height,
                        view_idx,
                        ViewChangeCause::QuorumTimeout,
                    );
                    self.maybe_rebroadcast_new_view_votes(height, now);
                    self.warn_resilience_frontier_proposal_deferred(
                        height,
                        view_idx,
                        "stale_slot_proposal_evidence",
                        highest_qc,
                        pending_queue_len,
                        now,
                    );
                    return false;
                }
                self.nudge_frontier_recovery_proposal_retry(now);
            }
            if pending_queue_len > 0 {
                debug!(
                    height,
                    view = view_idx,
                    queue_len = pending_queue_len,
                    "proposal already observed for this slot; waiting for progress"
                );
            } else {
                trace!(
                    height,
                    view = view_idx,
                    "proposal already observed for this slot; deferring reassembly"
                );
            }
            self.warn_resilience_frontier_proposal_deferred(
                height,
                view_idx,
                "slot_has_proposal_evidence",
                highest_qc,
                pending_queue_len,
                now,
            );
            return false;
        }

        if height == self.committed_height_snapshot().saturating_add(1)
            && let Some(existing_vote) =
                self.local_same_height_vote(height, self.epoch_for_height(height))
        {
            let new_view_qc_supersedes = self.new_view_qc_supersedes_same_height_vote_conflict(
                height,
                view_idx,
                highest_qc,
                existing_vote.block_hash,
                existing_vote.view,
            );
            if new_view_qc_supersedes
                || !self.local_same_height_vote_blocks_fresh_proposal_assembly(
                    height,
                    view_idx,
                    &existing_vote,
                    now,
                    true,
                    highest_qc,
                )
            {
                debug!(
                    height,
                    view = view_idx,
                    queue_len = pending_queue_len,
                    voted_view = existing_vote.view,
                    voted_phase = ?existing_vote.phase,
                    voted_block = %existing_vote.block_hash,
                    new_view_qc_supersedes,
                    "allowing fresh proposal after stale prior-view local same-height vote"
                );
            } else {
                if pending_queue_len > 0 {
                    debug!(
                        height,
                        view = view_idx,
                        queue_len = pending_queue_len,
                        voted_view = existing_vote.view,
                        voted_phase = ?existing_vote.phase,
                        voted_block = %existing_vote.block_hash,
                        "same-height local vote history already anchors the frontier; deferring fresh proposal assembly"
                    );
                } else {
                    trace!(
                        height,
                        view = view_idx,
                        voted_view = existing_vote.view,
                        voted_phase = ?existing_vote.phase,
                        voted_block = %existing_vote.block_hash,
                        "same-height local vote history already anchors the frontier; deferring fresh proposal assembly"
                    );
                }
                self.warn_resilience_frontier_proposal_deferred(
                    height,
                    view_idx,
                    "local_same_height_vote_blocks",
                    highest_qc,
                    pending_queue_len,
                    now,
                );
                return false;
            }
        }

        if let Some((pending_age, pending_view)) = self
            .pending
            .pending_blocks
            .values()
            .filter(|pending| {
                !pending.aborted && pending.height == height && pending.view == view_idx
            })
            .map(|pending| {
                (
                    now.saturating_duration_since(pending.inserted_at),
                    pending.view,
                )
            })
            .min_by_key(|(age, _)| *age)
        {
            let quorum_timeout = self.quorum_timeout(da_enabled);
            if quorum_timeout != Duration::ZERO && pending_age < quorum_timeout {
                debug!(
                    height,
                    pending_view,
                    target_view = view_idx,
                    age_ms = pending_age.as_millis(),
                    quorum_timeout_ms = quorum_timeout.as_millis(),
                    queue_len = pending_queue_len,
                    "deferring proposal: pending block still within quorum timeout window"
                );
                self.warn_resilience_frontier_proposal_deferred(
                    height,
                    view_idx,
                    "pending_block_quorum_window",
                    highest_qc,
                    pending_queue_len,
                    now,
                );
                return false;
            }
        }

        if let Some((pending_hash, pending_parent)) = self
            .pending
            .pending_blocks
            .iter()
            .find(|(_, pending)| {
                !pending.aborted && pending.height == height && pending.view == view_idx
            })
            .map(|(hash, pending)| (*hash, pending.block.header().prev_block_hash()))
        {
            if pending_block_stale_for_tip(height, pending_parent, tip_height, tip_hash) {
                iroha_logger::info!(
                    height,
                    view = view_idx,
                    queue_len = pending_queue_len,
                    pending_hash = %pending_hash,
                    pending_parent = ?pending_parent,
                    committed_hash = ?tip_hash,
                    "dropping stale pending proposal that no longer builds on committed chain"
                );
                if let Some((
                    tx_count,
                    requeued,
                    failures,
                    duplicate_failures,
                    retained_for_retry,
                )) = self.drop_stale_pending_block(pending_hash, height, view_idx)
                {
                    if tx_count > 0 {
                        iroha_logger::info!(
                            height,
                            view = view_idx,
                            tx_count,
                            requeued,
                            failures,
                            duplicate_failures,
                            retained_for_retry,
                            "requeued transactions from stale pending proposal"
                        );
                    }
                }
            } else {
                iroha_logger::info!(
                    height,
                    view = view_idx,
                    queue_len = pending_queue_len,
                    "proposal already pending for this slot; deferring reassembly"
                );
                self.warn_resilience_frontier_proposal_deferred(
                    height,
                    view_idx,
                    "pending_block_same_slot",
                    highest_qc,
                    pending_queue_len,
                    now,
                );
                return false;
            }
        }

        if self.highest_qc.is_none_or(|current| {
            (highest_qc.height, highest_qc.view) > (current.height, current.view)
        }) {
            self.highest_qc = Some(highest_qc);
            super::status::set_highest_qc(highest_qc.height, highest_qc.view);
            super::status::set_highest_qc_hash(highest_qc.subject_block_hash);
        }

        if let Err(reason) = ensure_locked_qc_allows(self.locked_qc, highest_qc) {
            match reason {
                LockedQcRejection::HeightRegressed { locked, highest } => {
                    let Some(lock) = self.promote_locked_qc_to_highest_if_needed("proposal") else {
                        return false;
                    };
                    iroha_logger::info!(
                        locked_height = locked,
                        highest_height = highest,
                        height,
                        view = view_idx,
                        queue_len = pending_queue_len,
                        lock_hash = %lock.subject_block_hash,
                        "replacing regressed highest QC with locked QC for proposal assembly"
                    );
                    highest_qc = lock;
                }
                LockedQcRejection::HashMismatch { .. } => {
                    let locked_hash = self.locked_qc.map(|qc| qc.subject_block_hash);
                    let locked_missing =
                        locked_hash.is_some_and(|hash| !self.block_known_for_lock(hash));
                    let highest_missing =
                        !self.block_payload_available_for_progress(highest_qc.subject_block_hash);
                    if locked_missing {
                        iroha_logger::warn!(
                            ?reason,
                            height,
                            view = view_idx,
                            queue_len = pending_queue_len,
                            locked_hash = ?locked_hash,
                            "clearing locked QC that is missing from kura"
                        );
                        self.locked_qc = Some(highest_qc);
                        super::status::set_locked_qc(
                            highest_qc.height,
                            highest_qc.view,
                            Some(highest_qc.subject_block_hash),
                        );
                    }
                    if highest_missing {
                        if self
                            .suppress_committed_edge_conflicting_highest_qc(highest_qc, "proposal")
                        {
                            self.clear_missing_block_view_change(&highest_qc.subject_block_hash);
                            return false;
                        }
                        let first_defer_in_round = self
                            .mark_highest_qc_missing_defer_for_round(height, view_idx, highest_qc);
                        if first_defer_in_round {
                            self.observe_new_view_highest_qc_exact_repair(highest_qc);
                        }
                        if let Some(suppressed_since_last) = self.proposal_defer_warning_log.allow(
                            ProposalDeferWarningKind::HighestQcMissing,
                            height,
                            view_idx,
                            highest_qc.subject_block_hash,
                            now,
                            Duration::from_secs(5),
                        ) {
                            iroha_logger::warn!(
                                ?reason,
                                height,
                                view = view_idx,
                                queue_len = pending_queue_len,
                                highest_hash = ?highest_qc.subject_block_hash,
                                locked_hash = ?locked_hash,
                                suppressed_since_last,
                                first_defer_in_round,
                                "highest QC block missing locally; deferring proposal"
                            );
                        }
                        return false;
                    }
                    if !locked_missing {
                        let lock_lag_deferred = self.defer_highest_qc_update_for_lock_catchup(
                            height, view_idx, highest_qc, now, "proposal",
                        );
                        iroha_logger::info!(
                            ?reason,
                            height,
                            view = view_idx,
                            queue_len = pending_queue_len,
                            lock_lag_deferred,
                            "deferring proposal: locked QC prevents proposal"
                        );
                        return false;
                    }
                }
            }
        }

        if !self.highest_qc_extends_locked(highest_qc) {
            if let Some(new_lock) = realign_locked_to_committed_if_extends(
                self.locked_qc,
                self.latest_committed_qc(),
                highest_qc,
                |hash, height| self.parent_hash_for(hash, height),
            ) {
                if self.locked_qc != Some(new_lock) {
                    info!(
                        height,
                        view = view_idx,
                        highest_height = highest_qc.height,
                        highest_hash = %highest_qc.subject_block_hash,
                        locked_height = new_lock.height,
                        locked_hash = %new_lock.subject_block_hash,
                        "resetting locked QC to committed chain to unblock proposal"
                    );
                    self.locked_qc = Some(new_lock);
                    super::status::set_locked_qc(
                        new_lock.height,
                        new_lock.view,
                        Some(new_lock.subject_block_hash),
                    );
                }
            }
        }

        self.maybe_realign_locked_to_committed_tip();

        if !self.highest_qc_extends_locked(highest_qc) {
            let _ = self.defer_highest_qc_update_for_lock_catchup(
                height, view_idx, highest_qc, now, "proposal",
            );
            if let Some(lock) = self.locked_qc
                && (highest_qc.height, highest_qc.view) <= (lock.height, lock.view)
            {
                iroha_logger::info!(
                    height,
                    view = view_idx,
                    highest_height = highest_qc.height,
                    highest_hash = %highest_qc.subject_block_hash,
                    locked_height = lock.height,
                    locked_hash = %lock.subject_block_hash,
                    queue_len = pending_queue_len,
                    "replacing non-extending highest QC with locked QC"
                );
                highest_qc = lock;
            } else if let Some(new_lock) = realign_locked_to_committed_if_extends(
                self.locked_qc,
                self.latest_committed_qc(),
                highest_qc,
                |hash, height| self.parent_hash_for(hash, height),
            ) {
                if self.locked_qc != Some(new_lock) {
                    iroha_logger::info!(
                        height,
                        view = view_idx,
                        locked_height = new_lock.height,
                        locked_hash = %new_lock.subject_block_hash,
                        highest_height = highest_qc.height,
                        highest_hash = %highest_qc.subject_block_hash,
                        queue_len = pending_queue_len,
                        "realigning locked QC to committed chain for proposal assembly"
                    );
                    self.locked_qc = Some(new_lock);
                    super::status::set_locked_qc(
                        new_lock.height,
                        new_lock.view,
                        Some(new_lock.subject_block_hash),
                    );
                }
                if !self.highest_qc_extends_locked(highest_qc) {
                    iroha_logger::info!(
                        height,
                        view = view_idx,
                        highest_height = highest_qc.height,
                        highest_hash = ?highest_qc.subject_block_hash,
                        locked_height = ?self.locked_qc.map(|qc| qc.height),
                        locked_hash = ?self.locked_qc.map(|qc| qc.subject_block_hash),
                        queue_len = pending_queue_len,
                        "deferring proposal: highest QC does not extend locked chain"
                    );
                    return false;
                }
            } else {
                iroha_logger::info!(
                    height,
                    view = view_idx,
                    highest_height = highest_qc.height,
                    highest_hash = ?highest_qc.subject_block_hash,
                    locked_height = ?self.locked_qc.map(|qc| qc.height),
                    locked_hash = ?self.locked_qc.map(|qc| qc.subject_block_hash),
                    queue_len = pending_queue_len,
                    "deferring proposal: highest QC does not extend locked chain"
                );
                return false;
            }
        }

        let proposal_roster = active_topology_peers;
        if proposal_roster.is_empty() {
            let _ = self.handle_roster_unavailable_recovery(
                height,
                view_idx,
                Some(highest_qc.subject_block_hash),
                pending_queue_len,
                now,
                ProposalDeferWarningKind::EmptyCommitTopologyProposal,
                "proposal_roster_selected_empty",
            );
            return false;
        }
        topology = super::network_topology::Topology::new(proposal_roster);
        required = topology.min_votes_for_commit();

        let leader_index = match self.leader_index_for(&mut topology, height, view_idx) {
            Ok(idx) => idx,
            Err(err) => {
                warn!(
                    ?err,
                    height,
                    view = view_idx,
                    "failed to compute leader index"
                );
                return false;
            }
        };

        let Some(local_pos) = topology.position(self.common_config.peer.id().public_key()) else {
            if pending_queue_len > 0 {
                iroha_logger::info!(
                    height,
                    view = view_idx,
                    queue_len = pending_queue_len,
                    "deferring proposal: local node not part of validator set"
                );
            } else {
                trace!(
                    height,
                    view = view_idx,
                    "local node not part of validator set"
                );
            }
            return false;
        };
        if local_pos != leader_index {
            let leader_peer = topology.iter().next().cloned();
            if self.maybe_rotate_missing_qc_nonleader_after_proposal_timeout(
                height,
                view_idx,
                pending_queue_len,
                now,
            ) {
                self.warn_resilience_frontier_proposal_deferred(
                    height,
                    view_idx,
                    "local_not_leader_timeout_rotate",
                    highest_qc,
                    pending_queue_len,
                    now,
                );
                return false;
            }
            let missing_qc_reacquire_requested = if self.config.resilience.enabled
                && self.frontier_missing_qc_liveness_active(height, view_idx)
                && height == self.committed_height_snapshot().saturating_add(1)
            {
                let requested = self.reacquire_missing_qc_dependencies(height, view_idx, now, true);
                self.maybe_rebroadcast_new_view_votes(height, now);
                self.nudge_frontier_recovery_proposal_retry(now);
                requested
            } else {
                false
            };
            if pending_queue_len > 0 {
                iroha_logger::debug!(
                    height,
                    view = view_idx,
                    local_idx = local_pos,
                    leader_index,
                    leader = ?leader_peer,
                    queue_len = pending_queue_len,
                    missing_qc_reacquire_requested,
                    "deferring proposal: local node is not leader for this round"
                );
            } else {
                trace!(
                    height,
                    view = view_idx,
                    local_idx = local_pos,
                    leader_index,
                    leader = ?leader_peer,
                    missing_qc_reacquire_requested,
                    "local node is not leader for this round"
                );
            }
            self.warn_resilience_frontier_proposal_deferred(
                height,
                view_idx,
                if missing_qc_reacquire_requested {
                    "local_not_leader_reacquire"
                } else {
                    "local_not_leader"
                },
                highest_qc,
                pending_queue_len,
                now,
            );
            return false;
        }

        let Ok(local_idx_val) = u32::try_from(local_pos) else {
            warn!(local_pos, "local validator index exceeds u32 limits");
            return false;
        };

        let prev_block = resolve_prev_block_for_proposal(
            height,
            &highest_qc,
            &self.kura,
            &self.pending.pending_blocks,
        );
        let certified_merge = self
            .pending_certified_merge_entry_for_proposal(height, view_idx, prev_block.as_deref())
            .is_some();
        if should_defer_ordinary_proposal_for_merge(
            has_queue_work,
            certified_merge,
            self.merge_preparation_grace_active(height, view_idx, now),
        ) {
            trace!(
                height,
                view = view_idx,
                queue_len = pending_queue_len,
                "deferring ordinary proposal during bounded merge-candidate preparation grace"
            );
            return false;
        }
        let has_internal_work = if has_queue_work {
            false
        } else {
            self.internal_proposal_work(height, prev_block.as_deref(), certified_merge)
                .has_work()
        };
        let allow_recovery_heartbeat = view_idx > 0 && height == committed_height.saturating_add(1);
        if !has_queue_work && !has_internal_work && !allow_recovery_heartbeat {
            trace!(
                height,
                view = view_idx,
                "deferring proposal: no queued transactions or internal work"
            );
            self.warn_resilience_frontier_proposal_deferred(
                height,
                view_idx,
                "no_queue_or_internal_work",
                highest_qc,
                pending_queue_len,
                now,
            );
            return false;
        }

        debug!(
            height,
            view = view_idx,
            quorum,
            required,
            leader_index,
            "NEW_VIEW quorum satisfied; assembling proposal"
        );

        let leader_peer = topology.iter().next().cloned();

        iroha_logger::info!(
            height,
            view = view_idx,
            leader_index,
            leader = ?leader_peer,
            local_idx = local_pos,
            quorum,
            highest_height = highest_qc.height,
            highest_view = highest_qc.view,
            "starting proposal assembly"
        );

        let view_snapshot = None;
        let assembled = match self.assemble_and_broadcast_proposal_with_recovery_heartbeat(
            height,
            view_idx,
            highest_qc,
            &mut topology,
            leader_index,
            local_idx_val,
            view_snapshot,
            now,
            allow_recovery_heartbeat,
        ) {
            Ok(assembled) => assembled,
            Err(err) => {
                warn!(?err, height, view = view_idx, "failed to assemble proposal");
                return false;
            }
        };

        if !assembled {
            return false;
        }

        iroha_logger::info!(
            height,
            view = view_idx,
            local_idx = local_idx_val,
            leader_index,
            quorum,
            highest_height = highest_qc.height,
            highest_view = highest_qc.view,
            highest_hash = %highest_qc.subject_block_hash,
            "proposal assembly succeeded"
        );

        self.subsystems
            .propose
            .new_view_tracker
            .remove(height, view_idx);
        self.subsystems.propose.last_cached_slot_timeout_trigger = None;
        self.subsystems.propose.last_missing_qc_timeout_trigger = None;
        self.subsystems.propose.last_successful_proposal = Some(now);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ProposalBackpressure, age_starved_queue_allows_stale_pending_override,
        cached_slot_timeout_hysteresis_remaining, canonicalize_parallel_batch_by_key,
        canonicalize_proposal_batch, canonicalize_proposal_batch_with_plans,
        collect_sccp_messages_after_ordered_preflight,
        collect_sccp_messages_for_active_proposal_routes,
        collect_sccp_messages_for_committable_proposal_routes, consensus_queue_backpressure,
        da_payload_budget, next_cached_slot_timeout_streak, refresh_proposal_routing_from_state,
        relay_tip_descriptor_hash_for_proposal, reorder_vec_by_indices,
        should_defer_ordinary_proposal_for_merge, trim_batch_for_size_cap,
        trim_batch_for_size_cap_with_plans,
    };
    use crate::queue::{
        BackpressureState, ConfigLaneRouter, LaneRouter, RoutingDecision, RoutingPlan,
    };
    use crate::sumeragi::status;
    use crate::tx::AcceptedTransaction;
    use iroha_config::parameters::actual::LaneRoutingPolicy;
    use iroha_crypto::{Hash, HashOf, KeyPair};
    use iroha_data_model::{
        ChainId, Level,
        block::{BlockHeader, consensus::LaneBlockCommitment},
        consensus::{CertPhase, Qc, QcAggregate, VALIDATOR_SET_HASH_VERSION_V1},
        domain::{Domain, DomainId},
        isi::{Log, Register},
        nexus::{
            AUTOSCALE_META_CREATED_HEIGHT, AUTOSCALE_META_MANAGED, DataSpaceCatalog, DataSpaceId,
            DataSpaceMetadata, LaneCatalog, LaneConfig, LaneFastpqProofMaterial, LaneId,
            LaneRelayEnvelope,
        },
        peer::PeerId,
        prelude::{AccountId, InstructionBox, TransactionBuilder},
        transaction::{Executable, IvmBytecode, IvmProved},
    };
    use std::borrow::Cow;
    use std::num::{NonZeroU32, NonZeroU64, NonZeroUsize};
    use std::time::{Duration, Instant};

    #[test]
    fn merge_preparation_grace_is_bounded_and_ready_merge_wins() {
        assert!(should_defer_ordinary_proposal_for_merge(true, false, true));
        assert!(
            !should_defer_ordinary_proposal_for_merge(true, false, false),
            "grace timeout must release ordinary proposal liveness"
        );
        assert!(
            !should_defer_ordinary_proposal_for_merge(true, true, true),
            "a ready certified merge must proceed without further preparation delay"
        );
        assert!(!should_defer_ordinary_proposal_for_merge(
            false, false, true
        ));
    }

    fn checked_key_pair() -> KeyPair {
        KeyPair::try_random().expect("proposal fixture key generation should succeed")
    }

    fn accepted_log_transaction(message: &str) -> AcceptedTransaction<'static> {
        let chain: ChainId = "proposal-canonicalization".parse().expect("chain id");
        let key_pair = checked_key_pair();
        let (_, private_key) = key_pair.clone().into_parts();
        let authority = AccountId::new(key_pair.public_key().clone());
        let tx = TransactionBuilder::new(chain, authority)
            .with_instructions([Log::new(Level::INFO, message.to_owned())])
            .sign(&private_key);

        AcceptedTransaction::new_unchecked(Cow::Owned(tx))
    }

    fn proposal_sccp_transfer_payload(nonce: u64) -> iroha_sccp::SccpPayloadV1 {
        iroha_sccp::SccpPayloadV1::Transfer(iroha_sccp::TransferPayloadV1 {
            version: 1,
            source_domain: iroha_sccp::SCCP_DOMAIN_SORA,
            dest_domain: iroha_sccp::SCCP_DOMAIN_ETH,
            nonce,
            asset_home_domain: iroha_sccp::SCCP_DOMAIN_SORA,
            asset_id_codec: iroha_sccp::SCCP_CODEC_TEXT_UTF8,
            asset_id: b"xor".to_vec(),
            amount: 77,
            sender_codec: iroha_sccp::SCCP_CODEC_TEXT_UTF8,
            sender: b"sora:bridge".to_vec(),
            recipient_codec: iroha_sccp::SCCP_CODEC_EVM_HEX,
            recipient: b"0x1111111111111111111111111111111111111111".to_vec(),
            route_id_codec: iroha_sccp::SCCP_CODEC_TEXT_UTF8,
            route_id: b"nexus:eth:xor".to_vec(),
        })
    }

    fn accepted_sccp_record_transaction(nonce: u64) -> AcceptedTransaction<'static> {
        let chain: ChainId = "proposal-sccp-root".parse().expect("chain id");
        let key_pair = checked_key_pair();
        let (_, private_key) = key_pair.clone().into_parts();
        let authority = AccountId::new(key_pair.public_key().clone());
        let payload =
            iroha_sccp::canonical_sccp_payload_bytes(&proposal_sccp_transfer_payload(nonce));
        let mut bytecode = ivm::ProgramMetadata {
            version_major: 1,
            version_minor: 0,
            mode: ivm::ivm_mode::ZK,
            vector_length: 0,
            max_cycles: 1,
            abi_version: 1,
        }
        .encode();
        bytecode.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        let executable = Executable::IvmProved(IvmProved {
            bytecode: IvmBytecode::from_compiled(bytecode),
            overlay: vec![InstructionBox::from(
                iroha_data_model::isi::bridge::RecordSccpMessage::new(payload),
            )]
            .into(),
            events_commitment: Hash::new(b"proposal-sccp-events"),
            gas_policy_commitment: Hash::new(b"proposal-sccp-gas"),
        });
        let tx = TransactionBuilder::new(chain, authority)
            .with_executable(executable)
            .sign(&private_key);
        AcceptedTransaction::new_unchecked(Cow::Owned(tx))
    }

    fn blank_state() -> crate::state::State {
        let world = crate::state::World::default();
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query = crate::query::store::LiveQueryStore::start_test();
        #[cfg(feature = "telemetry")]
        {
            let telemetry = crate::telemetry::StateTelemetry::default();
            crate::state::State::with_telemetry(world, kura, query, telemetry)
        }
        #[cfg(not(feature = "telemetry"))]
        {
            crate::state::State::new(world, kura, query)
        }
    }

    fn proposal_test_header() -> iroha_data_model::block::BlockHeader {
        iroha_data_model::block::BlockHeader::new(
            nonzero_ext::nonzero!(1_u64),
            None,
            None,
            None,
            1,
            0,
        )
    }

    fn proposal_lane_relay_settlement(height: u64) -> LaneBlockCommitment {
        LaneBlockCommitment {
            block_height: height,
            lane_id: LaneId::SINGLE,
            lane_incarnation: iroha_crypto::Hash::new(b"lane-block-commitment-incarnation"),
            dataspace_id: DataSpaceId::UNIVERSAL,
            tx_count: 0,
            total_local_micro: 0,
            total_xor_due_micro: 0,
            total_xor_after_haircut_micro: 0,
            total_xor_variance_micro: 0,
            swap_metadata: None,
            receipts: Vec::new(),
            nexus_fee_receipts: Vec::new(),
            native_amx_receipts: Vec::new(),
        }
    }

    fn proposal_lane_relay_qc(header: &BlockHeader) -> Qc {
        let validator_set: Vec<PeerId> = Vec::new();
        Qc {
            phase: CertPhase::Commit,
            subject_block_hash: header.hash(),
            parent_state_root: Hash::new(b"proposal relay parent state"),
            post_state_root: Hash::new(b"proposal relay post state"),
            height: header.height().get(),
            view: header.view_change_index(),
            epoch: 0,
            chain_order_hash: iroha_data_model::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            mode_tag: iroha_data_model::block::consensus::PERMISSIONED_TAG.to_string(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set,
            aggregate: QcAggregate {
                signers_bitmap: vec![0b0000_0001],
                bls_aggregate_signature: vec![0xA5; 48],
            },
        }
    }

    fn proposal_lane_relay_envelope(
        descriptor_hash: Option<Hash>,
        include_qc: bool,
        fastpq_proof: Option<LaneFastpqProofMaterial>,
    ) -> LaneRelayEnvelope {
        let header = proposal_test_header();
        let qc = include_qc.then(|| proposal_lane_relay_qc(&header));
        LaneRelayEnvelope::new(header, qc, None, proposal_lane_relay_settlement(1), 0)
            .expect("proposal lane relay envelope")
            .with_lane_block_descriptor_hash(descriptor_hash)
            .with_fastpq_proof_material(fastpq_proof)
    }

    #[test]
    fn relay_tip_descriptor_hash_for_proposal_requires_merge_admissible_relay() {
        let descriptor_hash = Hash::new(b"proposal relay descriptor");
        let fastpq_proof = LaneFastpqProofMaterial {
            proof_digest: Hash::new(b"proposal relay fastpq proof"),
            verified_at_height: 1,
        };
        let pending = proposal_lane_relay_envelope(Some(descriptor_hash), true, None);
        assert_eq!(
            relay_tip_descriptor_hash_for_proposal(&pending),
            None,
            "QC-only pending relay metadata must not become proposal lineage"
        );

        let fastpq_without_qc =
            proposal_lane_relay_envelope(Some(descriptor_hash), false, Some(fastpq_proof));
        assert_eq!(
            relay_tip_descriptor_hash_for_proposal(&fastpq_without_qc),
            None,
            "FastPQ metadata without lane finality must not become proposal lineage"
        );

        let merge_admissible =
            proposal_lane_relay_envelope(Some(descriptor_hash), true, Some(fastpq_proof));
        assert_eq!(
            relay_tip_descriptor_hash_for_proposal(&merge_admissible),
            Some(descriptor_hash)
        );
    }

    #[test]
    fn proposal_sccp_collection_ignores_records_when_nexus_disabled() {
        let mut state = blank_state();
        state.nexus.get_mut().enabled = false;
        let tx = accepted_sccp_record_transaction(1);
        let routing = vec![RoutingDecision::default()];
        let nexus = state.nexus_snapshot();

        let messages =
            collect_sccp_messages_for_active_proposal_routes(&[tx], &routing, &nexus, 1, |_| false)
                .expect("disabled Nexus should not be a routing error");

        assert!(
            messages.is_empty(),
            "proposal roots must not commit SCCP records that disabled Nexus execution will reject"
        );
    }

    #[test]
    fn proposal_sccp_collection_includes_active_route_records() {
        let mut state = blank_state();
        state.nexus.get_mut().enabled = true;
        let tx = accepted_sccp_record_transaction(2);
        let routing = vec![RoutingDecision::default()];
        let nexus = state.nexus_snapshot();

        let messages =
            collect_sccp_messages_for_active_proposal_routes(&[tx], &routing, &nexus, 1, |_| false)
                .expect("active default route should collect");

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].tx_index, 0);
        assert_eq!(messages[0].instruction_index, 0);
    }

    #[test]
    fn proposal_sccp_collection_filters_inactive_routes_without_renumbering() {
        let mut state = blank_state();
        state.nexus.get_mut().enabled = true;
        let skipped = accepted_sccp_record_transaction(3);
        let included = accepted_sccp_record_transaction(4);
        let routing = vec![
            RoutingDecision::new(LaneId::new(99), DataSpaceId::UNIVERSAL),
            RoutingDecision::default(),
        ];
        let nexus = state.nexus_snapshot();

        let messages = collect_sccp_messages_for_active_proposal_routes(
            &[skipped, included],
            &routing,
            &nexus,
            1,
            |_| false,
        )
        .expect("inactive routed entries should be filtered, not fatal");

        assert_eq!(messages.len(), 1);
        assert_eq!(
            messages[0].tx_index, 1,
            "route filtering must preserve canonical entrypoint indices"
        );
        assert_eq!(messages[0].instruction_index, 0);
    }

    #[test]
    fn proposal_sccp_collection_filters_future_created_autoscale_route() {
        let mut state = blank_state();
        let future_lane = LaneId::new(1);
        let mut elastic = LaneConfig {
            id: future_lane,
            dataspace_id: DataSpaceId::UNIVERSAL,
            alias: "elastic-lane-1".to_string(),
            ..LaneConfig::default()
        };
        elastic
            .metadata
            .insert(AUTOSCALE_META_MANAGED.to_string(), "true".to_string());
        elastic
            .metadata
            .insert(AUTOSCALE_META_CREATED_HEIGHT.to_string(), "7".to_string());
        let lane_catalog = LaneCatalog::new(
            NonZeroU32::new(2).expect("nonzero lane count"),
            vec![LaneConfig::default(), elastic],
        )
        .expect("future autoscale lane catalog");
        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.autoscale.enabled = true;
            nexus.autoscale.min_lanes = NonZeroU32::new(1).expect("nonzero min lanes");
            nexus.autoscale.max_lanes = NonZeroU32::new(8).expect("nonzero max lanes");
            nexus.lane_catalog = lane_catalog;
            nexus.lane_config =
                iroha_config::parameters::actual::LaneConfig::from_catalog(&nexus.lane_catalog);
        }
        let routing = vec![RoutingDecision::new(future_lane, DataSpaceId::UNIVERSAL)];
        let nexus = state.nexus_snapshot();

        let messages_before_creation = collect_sccp_messages_for_active_proposal_routes(
            &[accepted_sccp_record_transaction(6)],
            &routing,
            &nexus,
            6,
            |_| false,
        )
        .expect("future-created autoscale route should be filtered before creation height");
        assert!(
            messages_before_creation.is_empty(),
            "proposal roots must not commit SCCP records before the autoscale lane creation height"
        );

        let messages_at_creation = collect_sccp_messages_for_active_proposal_routes(
            &[accepted_sccp_record_transaction(7)],
            &routing,
            &nexus,
            7,
            |_| false,
        )
        .expect("autoscale route should collect at creation height");
        assert_eq!(messages_at_creation.len(), 1);
        assert_eq!(messages_at_creation[0].tx_index, 0);
    }

    #[test]
    fn proposal_sccp_collection_filters_stale_retired_lane_route() {
        let mut state = blank_state();
        let retired_lane = LaneId::new(5);
        let stale_catalog = LaneCatalog::new(
            NonZeroU32::new(6).expect("nonzero lane count"),
            vec![
                LaneConfig::default(),
                LaneConfig {
                    id: retired_lane,
                    alias: "retired-sccp-lane".to_owned(),
                    ..LaneConfig::default()
                },
            ],
        )
        .expect("stale retired lane catalog");
        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.lane_config =
                iroha_config::parameters::actual::LaneConfig::from_catalog(&stale_catalog);
            nexus.lane_catalog = LaneCatalog::default();
        }
        let routing = vec![RoutingDecision::new(retired_lane, DataSpaceId::UNIVERSAL)];
        let nexus = state.nexus_snapshot();

        let messages = collect_sccp_messages_for_active_proposal_routes(
            &[accepted_sccp_record_transaction(11)],
            &routing,
            &nexus,
            11,
            |_| false,
        )
        .expect("stale retired lane route should filter without aborting proposal assembly");

        assert!(
            messages.is_empty(),
            "proposal roots must not commit SCCP records for routes whose lane id was retired"
        );
    }

    #[test]
    fn proposal_sccp_collection_requires_recreated_lane_dataspace_match() {
        let mut state = blank_state();
        let recreated_lane = LaneId::new(4);
        let retired_dataspace = DataSpaceId::new(20);
        let recreated_dataspace = DataSpaceId::new(21);
        let lane_catalog = LaneCatalog::new(
            NonZeroU32::new(recreated_lane.as_u32() + 1).expect("nonzero lane count"),
            vec![
                LaneConfig::default(),
                LaneConfig {
                    id: recreated_lane,
                    dataspace_id: recreated_dataspace,
                    alias: "recreated-sccp-lane".to_owned(),
                    ..LaneConfig::default()
                },
            ],
        )
        .expect("recreated lane catalog");
        let dataspace_catalog = DataSpaceCatalog::new(vec![
            DataSpaceMetadata::default(),
            DataSpaceMetadata {
                id: recreated_dataspace,
                alias: "recreated-sccp-dataspace".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("recreated dataspace catalog");
        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.lane_catalog = lane_catalog;
            nexus.lane_config =
                iroha_config::parameters::actual::LaneConfig::from_catalog(&nexus.lane_catalog);
            nexus.dataspace_catalog = dataspace_catalog;
        }
        let nexus = state.nexus_snapshot();

        let stale_messages = collect_sccp_messages_for_active_proposal_routes(
            &[accepted_sccp_record_transaction(12)],
            &[RoutingDecision::new(recreated_lane, retired_dataspace)],
            &nexus,
            12,
            |_| false,
        )
        .expect("stale recreated-lane dataspace should filter without aborting proposal assembly");
        assert!(
            stale_messages.is_empty(),
            "proposal roots must not commit SCCP records for a recreated lane under its retired dataspace"
        );

        let fresh_messages = collect_sccp_messages_for_active_proposal_routes(
            &[accepted_sccp_record_transaction(13)],
            &[RoutingDecision::new(recreated_lane, recreated_dataspace)],
            &nexus,
            13,
            |_| false,
        )
        .expect("fresh recreated-lane route should collect");
        assert_eq!(
            fresh_messages.len(),
            1,
            "recreated lane ids remain usable only with their current dataspace binding"
        );
        assert_eq!(fresh_messages[0].tx_index, 0);
    }

    #[test]
    fn proposal_sccp_ordered_preflight_replays_regular_transaction_before_candidate() {
        let regular = accepted_log_transaction("regular-before-sccp");
        let sccp = accepted_sccp_record_transaction(9);
        let tx_batch = vec![regular, sccp];
        let routing = vec![RoutingDecision::default(), RoutingDecision::default()];
        let sccp_messages =
            crate::bridge::collect_sccp_messages_from_accepted_transaction(1, &tx_batch[1]);
        assert_eq!(sccp_messages.len(), 1);
        let candidate_messages = vec![None, Some(sccp_messages)];
        let mut preflight_order = Vec::new();
        let mut prior_regular_applied = false;

        let committable = collect_sccp_messages_after_ordered_preflight(
            &tx_batch,
            &routing,
            candidate_messages,
            |tx_index, _, _| {
                preflight_order.push(tx_index);
                match tx_index {
                    0 => {
                        prior_regular_applied = true;
                        Ok(true)
                    }
                    1 if prior_regular_applied => Ok(true),
                    1 => Err("regular transaction state was not replayed first".to_owned()),
                    _ => Ok(true),
                }
            },
        );

        assert_eq!(
            preflight_order,
            vec![0, 1],
            "proposal SCCP preflight must replay regular signed entrypoints before later SCCP candidates"
        );
        assert_eq!(committable.len(), 1);
        assert_eq!(
            committable[0].tx_index, 1,
            "ordered preflight must preserve the canonical SCCP transaction index"
        );
    }

    #[test]
    fn proposal_sccp_ordered_preflight_does_not_apply_failed_regular_transaction() {
        let regular = accepted_log_transaction("failed-regular-before-sccp");
        let sccp = accepted_sccp_record_transaction(10);
        let tx_batch = vec![regular, sccp];
        let routing = vec![RoutingDecision::default(), RoutingDecision::default()];
        let sccp_messages =
            crate::bridge::collect_sccp_messages_from_accepted_transaction(1, &tx_batch[1]);
        assert_eq!(sccp_messages.len(), 1);
        let candidate_messages = vec![None, Some(sccp_messages)];
        let mut preflight_order = Vec::new();
        let mut prior_regular_applied = false;

        let committable = collect_sccp_messages_after_ordered_preflight(
            &tx_batch,
            &routing,
            candidate_messages,
            |tx_index, _, _| {
                preflight_order.push(tx_index);
                match tx_index {
                    0 => Err("regular transaction failed".to_owned()),
                    1 if prior_regular_applied => Ok(true),
                    1 => Err("regular transaction state was correctly absent".to_owned()),
                    _ => {
                        prior_regular_applied = true;
                        Ok(true)
                    }
                }
            },
        );

        assert_eq!(
            preflight_order,
            vec![0, 1],
            "failed regular entrypoints must still be evaluated before later SCCP candidates"
        );
        assert!(
            committable.is_empty(),
            "SCCP candidate must be excluded when it depends on a prior regular transaction that failed preflight"
        );
    }

    #[test]
    fn proposal_sccp_preflight_excludes_ivm_proved_overlay_build_failure() {
        let mut state = blank_state();
        state.nexus.get_mut().enabled = true;
        let tx = accepted_sccp_record_transaction(8);
        let routing = vec![RoutingDecision::default()];
        let nexus = state.nexus_snapshot();

        let raw_messages = collect_sccp_messages_for_active_proposal_routes(
            &[tx.clone()],
            &routing,
            &nexus,
            1,
            |_| false,
        )
        .expect("raw proposal SCCP collection should still see the record");
        assert_eq!(raw_messages.len(), 1);

        let committable = collect_sccp_messages_for_committable_proposal_routes(
            &[tx],
            &routing,
            &nexus,
            1,
            &state,
            proposal_test_header(),
        )
        .expect("failed SCCP preflight should filter, not abort proposal assembly");

        assert!(
            committable.is_empty(),
            "SCCP records from a failed IvmProved preflight must not be signed into the root"
        );
    }

    #[test]
    fn proposal_sccp_collection_rejects_routing_vector_length_drift() {
        let mut state = blank_state();
        state.nexus.get_mut().enabled = true;
        let tx = accepted_sccp_record_transaction(5);
        let nexus = state.nexus_snapshot();

        let err =
            collect_sccp_messages_for_active_proposal_routes(&[tx], &[], &nexus, 1, |_| false)
                .expect_err("routing length drift must reject before root computation");

        assert!(
            err.to_string()
                .contains("SCCP routing vector length mismatch"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn refresh_proposal_routing_from_state_replaces_stale_vectors() {
        let state = blank_state();
        let tx_batch = vec![
            accepted_log_transaction("refresh-route-a"),
            accepted_log_transaction("refresh-route-b"),
        ];
        let stale_route = RoutingDecision::new(LaneId::new(7), DataSpaceId::new(7));
        let mut routing_batch = vec![stale_route; tx_batch.len()];
        let mut routing_plan_batch = vec![RoutingPlan::single(stale_route); tx_batch.len()];
        let default_route = RoutingDecision::default();
        let default_plan = RoutingPlan::single(default_route);

        let changed = refresh_proposal_routing_from_state(
            &tx_batch,
            &mut routing_batch,
            &mut routing_plan_batch,
            &state.view(),
            0,
            1,
        )
        .expect("refresh should use committed state routing");

        assert!(changed, "stale route vectors should be replaced");
        assert_eq!(routing_batch, vec![default_route; tx_batch.len()]);
        assert_eq!(routing_plan_batch, vec![default_plan; tx_batch.len()]);

        let changed_again = refresh_proposal_routing_from_state(
            &tx_batch,
            &mut routing_batch,
            &mut routing_plan_batch,
            &state.view(),
            0,
            1,
        )
        .expect("second refresh should remain valid");

        assert!(
            !changed_again,
            "already-current route vectors should not report another refresh"
        );
    }

    #[test]
    fn refresh_proposal_routing_from_state_uses_live_autoscale_elastic_range() {
        let mut state = blank_state();
        let mut elastic = LaneConfig {
            id: LaneId::new(1),
            dataspace_id: DataSpaceId::UNIVERSAL,
            alias: "elastic-lane-1".to_string(),
            ..LaneConfig::default()
        };
        elastic
            .metadata
            .insert(AUTOSCALE_META_MANAGED.to_string(), "true".to_string());
        elastic
            .metadata
            .insert(AUTOSCALE_META_CREATED_HEIGHT.to_string(), "2".to_string());
        let lane_catalog = LaneCatalog::new(
            NonZeroU32::new(2).expect("nonzero lane count"),
            vec![LaneConfig::default(), elastic],
        )
        .expect("autoscale lane catalog");
        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.autoscale.enabled = true;
            nexus.autoscale.min_lanes = NonZeroU32::new(1).expect("nonzero min lanes");
            nexus.autoscale.max_lanes = NonZeroU32::new(8).expect("nonzero max lanes");
            nexus.lane_catalog = lane_catalog;
            nexus.lane_config =
                iroha_config::parameters::actual::LaneConfig::from_catalog(&nexus.lane_catalog);
        }

        let mut elastic_tx = None;
        for idx in 0..512 {
            let tx = accepted_log_transaction(&format!("proposal-autoscale-{idx}"));
            let mut routing_batch = vec![RoutingDecision::default()];
            let mut routing_plan_batch = vec![RoutingPlan::single(RoutingDecision::default())];
            refresh_proposal_routing_from_state(
                core::slice::from_ref(&tx),
                &mut routing_batch,
                &mut routing_plan_batch,
                &state.view(),
                0,
                2,
            )
            .expect("proposal refresh should resolve autoscale candidates");
            if routing_batch[0].lane_id == LaneId::new(1) {
                elastic_tx = Some(tx);
                break;
            }
        }
        let tx = elastic_tx.expect("fixture should find a transaction for the elastic shard");
        let mut routing_batch = vec![RoutingDecision::default()];
        let mut routing_plan_batch = vec![RoutingPlan::single(RoutingDecision::default())];

        let changed = refresh_proposal_routing_from_state(
            &[tx],
            &mut routing_batch,
            &mut routing_plan_batch,
            &state.view(),
            0,
            2,
        )
        .expect("proposal refresh should use live Nexus autoscale range");

        assert!(
            changed,
            "stale single-lane proposal vectors should be refreshed"
        );
        assert_eq!(
            routing_batch,
            vec![RoutingDecision::new(LaneId::new(1), DataSpaceId::UNIVERSAL)]
        );
        assert_eq!(
            routing_plan_batch,
            vec![RoutingPlan::single(RoutingDecision::new(
                LaneId::new(1),
                DataSpaceId::UNIVERSAL
            ))]
        );
    }

    #[test]
    fn refresh_proposal_routing_from_state_ignores_autoscale_when_nexus_disabled() {
        let mut state = blank_state();
        let mut elastic = LaneConfig {
            id: LaneId::new(1),
            dataspace_id: DataSpaceId::UNIVERSAL,
            alias: "elastic-lane-1".to_string(),
            ..LaneConfig::default()
        };
        elastic
            .metadata
            .insert(AUTOSCALE_META_MANAGED.to_string(), "true".to_string());
        elastic
            .metadata
            .insert(AUTOSCALE_META_CREATED_HEIGHT.to_string(), "2".to_string());
        let lane_catalog = LaneCatalog::new(
            NonZeroU32::new(2).expect("nonzero lane count"),
            vec![LaneConfig::default(), elastic],
        )
        .expect("autoscale lane catalog");
        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.autoscale.enabled = true;
            nexus.autoscale.min_lanes = NonZeroU32::new(1).expect("nonzero min lanes");
            nexus.autoscale.max_lanes = NonZeroU32::new(8).expect("nonzero max lanes");
            nexus.lane_catalog = lane_catalog;
            nexus.lane_config =
                iroha_config::parameters::actual::LaneConfig::from_catalog(&nexus.lane_catalog);
        }

        let stale_elastic_route = RoutingDecision::new(LaneId::new(1), DataSpaceId::UNIVERSAL);
        let mut elastic_tx = None;
        for idx in 0..512 {
            let tx = accepted_log_transaction(&format!("proposal-disabled-autoscale-{idx}"));
            let mut routing_batch = vec![RoutingDecision::default()];
            let mut routing_plan_batch = vec![RoutingPlan::single(RoutingDecision::default())];
            refresh_proposal_routing_from_state(
                core::slice::from_ref(&tx),
                &mut routing_batch,
                &mut routing_plan_batch,
                &state.view(),
                0,
                2,
            )
            .expect("enabled Nexus should resolve autoscale candidates");
            if routing_batch == vec![stale_elastic_route] {
                elastic_tx = Some(tx);
                break;
            }
        }
        let tx = elastic_tx.expect("fixture should find a transaction for the elastic shard");

        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = false;
        }

        let mut routing_batch = vec![stale_elastic_route];
        let mut routing_plan_batch = vec![RoutingPlan::single(stale_elastic_route)];
        let default_route = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);

        let changed = refresh_proposal_routing_from_state(
            &[tx],
            &mut routing_batch,
            &mut routing_plan_batch,
            &state.view(),
            0,
            2,
        )
        .expect("disabled Nexus should refresh stale elastic proposal vectors");

        assert!(
            changed,
            "stale elastic vectors must be replaced once Nexus is disabled"
        );
        assert_eq!(routing_batch, vec![default_route]);
        assert_eq!(routing_plan_batch, vec![RoutingPlan::single(default_route)]);
    }

    #[test]
    fn refresh_proposal_routing_from_state_refreshes_native_amx_participant_legs() {
        let mut state = blank_state();
        let first_dataspace = DataSpaceId::new(7);
        let second_dataspace = DataSpaceId::new(8);
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: vec![],
        };
        let dataspace_catalog = DataSpaceCatalog::new(vec![
            DataSpaceMetadata {
                id: DataSpaceId::UNIVERSAL,
                alias: "universal".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
            DataSpaceMetadata {
                id: first_dataspace,
                alias: "acme".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
            DataSpaceMetadata {
                id: second_dataspace,
                alias: "bank".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("dataspace catalog");
        let stale_lane_catalog = LaneCatalog::new(
            NonZeroU32::new(4).expect("nonzero lane count"),
            vec![
                LaneConfig::default(),
                LaneConfig {
                    id: LaneId::new(1),
                    dataspace_id: first_dataspace,
                    alias: "acme-primary".to_owned(),
                    ..LaneConfig::default()
                },
                LaneConfig {
                    id: LaneId::new(2),
                    dataspace_id: second_dataspace,
                    alias: "bank-primary".to_owned(),
                    ..LaneConfig::default()
                },
                LaneConfig {
                    id: LaneId::new(3),
                    dataspace_id: second_dataspace,
                    alias: "bank-secondary".to_owned(),
                    ..LaneConfig::default()
                },
            ],
        )
        .expect("stale lane catalog");
        let mut current_lanes = stale_lane_catalog.lanes().to_vec();
        let stale_participant_lane = current_lanes
            .iter_mut()
            .find(|lane| lane.id == LaneId::new(2))
            .expect("stale participant lane");
        stale_participant_lane.alias = "elastic-lane-2".to_owned();
        stale_participant_lane
            .metadata
            .insert(AUTOSCALE_META_MANAGED.to_string(), "true".to_string());
        stale_participant_lane
            .metadata
            .insert(AUTOSCALE_META_CREATED_HEIGHT.to_string(), "10".to_string());
        let current_lane_catalog = LaneCatalog::new(stale_lane_catalog.lane_count(), current_lanes)
            .expect("current lane catalog");

        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.routing_policy = policy.clone();
            nexus.lane_catalog = current_lane_catalog.clone();
            nexus.dataspace_catalog = dataspace_catalog.clone();
            nexus.lane_config =
                iroha_config::parameters::actual::LaneConfig::from_catalog(&nexus.lane_catalog);
        }

        let chain: ChainId = "proposal-native-amx-refresh".parse().expect("chain id");
        let key_pair = checked_key_pair();
        let (_, private_key) = key_pair.clone().into_parts();
        let authority = AccountId::new(key_pair.public_key().clone());
        let tx = AcceptedTransaction::new_unchecked(Cow::Owned(
            TransactionBuilder::new(chain, authority)
                .with_instructions([
                    InstructionBox::from(Register::domain(Domain::new(
                        DomainId::try_new("merchant", "acme").expect("domain id"),
                    ))),
                    InstructionBox::from(Register::domain(Domain::new(
                        DomainId::try_new("treasury", "bank").expect("domain id"),
                    ))),
                ])
                .sign(&private_key),
        ));
        let stale_plan = ConfigLaneRouter::new(
            policy.clone(),
            dataspace_catalog.clone(),
            stale_lane_catalog,
        )
        .try_route_plan(&tx)
        .expect("stale Native AMX plan should resolve");
        let current_plan = ConfigLaneRouter::new(policy, dataspace_catalog, current_lane_catalog)
            .try_route_plan(&tx)
            .expect("current Native AMX plan should resolve");
        assert_eq!(
            stale_plan.coordinator_route(),
            current_plan.coordinator_route()
        );
        assert_ne!(stale_plan, current_plan);

        let mut routing_batch = vec![stale_plan.coordinator_route()];
        let mut routing_plan_batch = vec![stale_plan];
        let changed = refresh_proposal_routing_from_state(
            &[tx],
            &mut routing_batch,
            &mut routing_plan_batch,
            &state.view(),
            0,
            1,
        )
        .expect("proposal refresh should replace stale Native AMX participant legs");

        assert!(
            changed,
            "participant-only Native AMX drift should refresh proposal plan vectors"
        );
        assert_eq!(routing_batch, vec![current_plan.coordinator_route()]);
        assert_eq!(routing_plan_batch, vec![current_plan]);
    }

    #[test]
    fn refresh_proposal_routing_from_state_rejects_vector_length_drift() {
        let state = blank_state();
        let tx_batch = vec![accepted_log_transaction("refresh-route-drift")];
        let mut routing_batch = Vec::new();
        let mut routing_plan_batch = vec![RoutingPlan::single(RoutingDecision::default())];

        let err = refresh_proposal_routing_from_state(
            &tx_batch,
            &mut routing_batch,
            &mut routing_plan_batch,
            &state.view(),
            0,
            1,
        )
        .expect_err("routing vector drift must fail closed");

        assert!(
            err.to_string()
                .contains("proposal routing vector length mismatch"),
            "unexpected error: {err}"
        );
        assert!(
            routing_batch.is_empty(),
            "failed refresh must not mutate route vector"
        );
        assert_eq!(
            routing_plan_batch,
            vec![RoutingPlan::single(RoutingDecision::default())],
            "failed refresh must not mutate plan vector"
        );
    }

    #[test]
    fn filter_committed_transactions_for_proposal_rejects_vector_length_drift() {
        let state = blank_state();
        let tx = accepted_log_transaction("committed-filter-route-drift");
        let route = RoutingDecision::default();
        let plan = RoutingPlan::single(route);
        let size = tx.encoded_len();
        let mut guards = Vec::new();
        let mut transactions = vec![tx];
        let mut routing = Vec::new();
        let mut routing_plans = vec![plan];
        let mut sizes = vec![size];

        let err = match super::Actor::filter_committed_transactions_for_proposal(
            &state,
            &mut guards,
            &mut transactions,
            &mut routing,
            &mut routing_plans,
            &mut sizes,
            1,
            0,
        ) {
            Ok(_) => panic!("proposal metadata vector drift must fail closed"),
            Err(err) => err,
        };

        assert!(
            err.to_string()
                .contains("proposal committed-filter vector length mismatch"),
            "unexpected error: {err}"
        );
        assert!(guards.is_empty());
        assert_eq!(
            transactions.len(),
            1,
            "failed filter must retain tx ownership"
        );
        assert!(routing.is_empty());
        assert_eq!(routing_plans.len(), 1);
        assert_eq!(sizes, vec![size]);
    }

    #[test]
    fn reorder_vec_by_indices_moves_non_clone_values() {
        #[derive(Debug, PartialEq, Eq)]
        struct NonClone(u8);

        let mut values = vec![NonClone(1), NonClone(2), NonClone(3), NonClone(4)];

        reorder_vec_by_indices(&mut values, &[2, 0, 3, 1]);

        assert_eq!(
            values,
            vec![NonClone(3), NonClone(1), NonClone(4), NonClone(2)]
        );
    }

    #[test]
    fn proposal_batch_formal_gate_matrix() {
        fn routes_for(txs: &[u32]) -> Vec<u32> {
            txs.iter().map(|tx| tx + 10).collect()
        }

        fn plans_for(txs: &[u32]) -> Vec<u32> {
            txs.iter().map(|tx| tx + 20).collect()
        }

        fn sizes_for(txs: &[u32]) -> Vec<usize> {
            txs.iter()
                .map(|tx| usize::try_from(tx + 30).expect("fits"))
                .collect()
        }

        fn assert_trim_case(
            name: &str,
            txs: Vec<u32>,
            sizes: Vec<usize>,
            excess_bytes: usize,
            expected_txs: &[u32],
            expected_removed: &[(u32, u32)],
        ) {
            let mut tx_batch = txs.clone();
            let mut routing_batch = routes_for(&txs);
            let mut size_batch = sizes;
            let mut removed = Vec::new();

            let removed_count = trim_batch_for_size_cap(
                &mut tx_batch,
                &mut routing_batch,
                &mut size_batch,
                &mut removed,
                excess_bytes,
            );

            assert_eq!(tx_batch, expected_txs, "{name} txs");
            assert_eq!(routing_batch, routes_for(expected_txs), "{name} routes");
            assert_eq!(
                size_batch.len(),
                expected_txs.len(),
                "{name} sizes stay aligned"
            );
            assert_eq!(removed, expected_removed, "{name} removed");
            assert_eq!(
                removed_count,
                expected_removed.len(),
                "{name} removed_count"
            );
        }

        assert_trim_case(
            "trim_no_excess",
            vec![1, 2, 3],
            vec![10, 10, 10],
            0,
            &[1, 2, 3],
            &[],
        );
        assert_trim_case(
            "trim_remove_one",
            vec![1, 2, 3],
            vec![10, 10, 10],
            5,
            &[1, 2],
            &[(3, 13)],
        );
        assert_trim_case(
            "trim_remove_multiple",
            vec![1, 2, 3, 4],
            vec![10, 10, 10, 10],
            15,
            &[1, 2],
            &[(4, 14), (3, 13)],
        );
        assert_trim_case(
            "trim_keeps_single",
            vec![1, 2, 3],
            vec![5, 5, 5],
            100,
            &[1],
            &[(3, 13), (2, 12)],
        );
        assert_trim_case(
            "trim_zero_size_floor",
            vec![1, 2, 3],
            vec![10, 10, 0],
            1,
            &[1, 2],
            &[(3, 13)],
        );

        let mut tx_batch = vec![1, 2, 3];
        let mut routing_batch = routes_for(&tx_batch);
        let mut routing_plan_batch = plans_for(&tx_batch);
        let mut size_batch = vec![10, 10, 10];
        let mut removed = Vec::new();
        let removed_count = trim_batch_for_size_cap_with_plans(
            &mut tx_batch,
            &mut routing_batch,
            &mut routing_plan_batch,
            &mut size_batch,
            &mut removed,
            5,
        );
        assert_eq!(removed_count, 1, "trim_with_plans_align removed_count");
        assert_eq!(tx_batch, vec![1, 2], "trim_with_plans_align txs");
        assert_eq!(routing_batch, vec![11, 12], "trim_with_plans_align routes");
        assert_eq!(
            routing_plan_batch,
            vec![21, 22],
            "trim_with_plans_align plans"
        );
        assert_eq!(size_batch, vec![10, 10], "trim_with_plans_align sizes");
        assert_eq!(removed, vec![(3, 23)], "trim_with_plans_align removed");

        let first_route = RoutingDecision::new(LaneId::new(1), DataSpaceId::new(7));
        let second_route = RoutingDecision::new(LaneId::new(2), DataSpaceId::new(8));
        let deferred_route = RoutingDecision::new(LaneId::new(3), DataSpaceId::new(9));
        let native_plan = RoutingPlan::native_amx(
            deferred_route,
            vec![
                crate::queue::RouteLeg::new(first_route, crate::queue::RouteLegRole::Participant),
                crate::queue::RouteLeg::new(second_route, crate::queue::RouteLegRole::Participant),
            ],
        );
        let mut tx_batch = vec![1_u32, 2_u32];
        let mut routing_batch = vec![first_route, deferred_route];
        let mut routing_plan_batch = vec![RoutingPlan::single(first_route), native_plan.clone()];
        let mut size_batch = vec![10_usize, 10_usize];
        let mut removed = Vec::new();
        let removed_count = trim_batch_for_size_cap_with_plans(
            &mut tx_batch,
            &mut routing_batch,
            &mut routing_plan_batch,
            &mut size_batch,
            &mut removed,
            1,
        );
        assert_eq!(removed_count, 1, "trim_native_amx removed_count");
        assert_eq!(tx_batch, vec![1], "trim_native_amx txs");
        assert_eq!(routing_batch, vec![first_route], "trim_native_amx routes");
        assert_eq!(
            routing_plan_batch,
            vec![RoutingPlan::single(first_route)],
            "trim_native_amx retained plan"
        );
        assert_eq!(
            removed,
            vec![(2, native_plan)],
            "trim_native_amx removed plan preserves participants"
        );

        fn assert_canon_case<K, F>(name: &str, mut tx_batch: Vec<u32>, key: F, expected_txs: &[u32])
        where
            K: Ord,
            F: Fn(&u32) -> K,
        {
            let mut routing_batch = routes_for(&tx_batch);
            let mut size_batch = sizes_for(&tx_batch);
            canonicalize_parallel_batch_by_key(
                &mut tx_batch,
                &mut routing_batch,
                &mut size_batch,
                key,
            );
            assert_eq!(tx_batch, expected_txs, "{name} txs");
            assert_eq!(routing_batch, routes_for(expected_txs), "{name} routes");
            assert_eq!(size_batch, sizes_for(expected_txs), "{name} sizes");
        }

        assert_canon_case("canon_empty", Vec::new(), |tx| *tx, &[]);
        assert_canon_case("canon_single", vec![1], |tx| *tx, &[1]);
        assert_canon_case("canon_already_sorted", vec![1, 2, 3], |tx| *tx, &[1, 2, 3]);
        assert_canon_case(
            "canon_reverse_keys",
            vec![1, 2, 3],
            |tx| 4 - *tx,
            &[3, 2, 1],
        );
        assert_canon_case(
            "canon_duplicate_keys_stable",
            vec![1, 2, 3, 4],
            |tx| match *tx {
                2 | 4 => 0,
                3 => 1,
                1 => 2,
                _ => 3,
            },
            &[2, 4, 3, 1],
        );

        let txs = ["canon-plan-a", "canon-plan-b", "canon-plan-c"]
            .into_iter()
            .map(accepted_log_transaction)
            .collect::<Vec<_>>();
        let mut entries = txs
            .into_iter()
            .enumerate()
            .map(|(idx, tx)| {
                let route = RoutingDecision::new(
                    LaneId::new(u32::try_from(idx + 1).expect("lane id")),
                    DataSpaceId::new(u64::try_from(idx + 20).expect("dataspace id")),
                );
                let plan = RoutingPlan::single(route);
                let size = 100 + idx;
                (tx.as_ref().hash_as_entrypoint(), tx, route, plan, size)
            })
            .collect::<Vec<_>>();
        entries.sort_unstable_by(|left, right| right.0.cmp(&left.0));
        let mut expected = entries.clone();
        expected.sort_unstable_by(|left, right| left.0.cmp(&right.0));

        let mut tx_batch = entries
            .iter()
            .map(|(_, tx, _, _, _)| tx.clone())
            .collect::<Vec<_>>();
        let mut routing_batch = entries
            .iter()
            .map(|(_, _, route, _, _)| *route)
            .collect::<Vec<_>>();
        let mut routing_plan_batch = entries
            .iter()
            .map(|(_, _, _, plan, _)| plan.clone())
            .collect::<Vec<_>>();
        let mut size_batch = entries
            .iter()
            .map(|(_, _, _, _, size)| *size)
            .collect::<Vec<_>>();

        canonicalize_proposal_batch_with_plans(
            &mut tx_batch,
            &mut routing_batch,
            &mut routing_plan_batch,
            &mut size_batch,
        );

        let actual_hashes = tx_batch
            .iter()
            .map(|tx| tx.as_ref().hash_as_entrypoint())
            .collect::<Vec<_>>();
        let expected_hashes = expected
            .iter()
            .map(|(hash, _, _, _, _)| *hash)
            .collect::<Vec<_>>();
        let expected_routes = expected
            .iter()
            .map(|(_, _, route, _, _)| *route)
            .collect::<Vec<_>>();
        let expected_plans = expected
            .iter()
            .map(|(_, _, _, plan, _)| plan.clone())
            .collect::<Vec<_>>();
        let expected_sizes = expected
            .iter()
            .map(|(_, _, _, _, size)| *size)
            .collect::<Vec<_>>();

        assert_eq!(actual_hashes, expected_hashes, "canon_with_plans hashes");
        assert_eq!(routing_batch, expected_routes, "canon_with_plans routes");
        assert_eq!(routing_plan_batch, expected_plans, "canon_with_plans plans");
        assert_eq!(size_batch, expected_sizes, "canon_with_plans sizes");
    }

    #[test]
    fn proposal_budget_formal_gate_matrix() {
        fn assert_queue_case(
            name: &str,
            block_depth: u64,
            rbc_depth: u64,
            block_cap: usize,
            rbc_cap: usize,
            expected: bool,
        ) {
            let depths = status::WorkerQueueDepthSnapshot {
                block_payload_rx: block_depth,
                rbc_chunk_rx: rbc_depth,
                ..status::WorkerQueueDepthSnapshot::default()
            };
            assert_eq!(
                consensus_queue_backpressure(depths, block_cap, rbc_cap),
                expected,
                "{name}"
            );
        }

        assert_queue_case("queue_block_cap_floor", 1, 0, 0, 5, true);
        assert_queue_case("queue_rbc_cap_floor", 0, 1, 5, 0, true);
        assert_queue_case("queue_below_caps", 1, 1, 2, 2, false);
        assert_queue_case("queue_at_block_cap", 2, 0, 2, 5, true);
        assert_queue_case("queue_at_rbc_cap", 0, 2, 5, 2, true);

        let rbc_max_total_chunks =
            usize::try_from(super::super::RBC_MAX_TOTAL_CHUNKS).expect("fits in usize");
        assert_eq!(da_payload_budget(0, 10, 10, None), 10);
        assert_eq!(da_payload_budget(5, 50, 10, NonZeroUsize::new(12)), 12);
        assert_eq!(da_payload_budget(5, 7, 10, None), 7);
        assert_eq!(da_payload_budget(5, 50, 0, None), 5);
        assert_eq!(
            da_payload_budget(3, 10_000, 2_000, None),
            3 * rbc_max_total_chunks
        );

        let tx_cases = [
            ("tx_no_config_empty_queue", 0, 9, None, 9, 1),
            ("tx_config_caps_param", 10, 9, NonZeroUsize::new(3), 3, 3),
            ("tx_param_caps_config", 10, 4, NonZeroUsize::new(8), 4, 4),
            ("tx_queue_caps_target", 2, 9, None, 9, 2),
        ];
        for (name, queue_len, param_limit, config_cap, expected_target, expected_max) in tx_cases {
            let (target, max_in_block) =
                super::Actor::max_tx_budget(queue_len, param_limit, config_cap);
            assert_eq!(target, expected_target, "{name} target");
            assert_eq!(max_in_block.get(), expected_max, "{name} max_in_block");
        }

        let fast_threshold =
            iroha_config::parameters::defaults::sumeragi::FAST_FINALITY_COMMIT_TIME_MS;
        let fast_tx_cases = [
            (
                "fast_tx_cap_commit_time",
                Some(NonZeroUsize::new(6).expect("non-zero")),
                fast_threshold,
                fast_threshold + 1,
                6,
                6,
                true,
                true,
            ),
            (
                "fast_tx_cap_effective_time",
                Some(NonZeroUsize::new(6).expect("non-zero")),
                fast_threshold + 1,
                fast_threshold,
                6,
                6,
                true,
                true,
            ),
            (
                "fast_tx_cap_not_applicable",
                Some(NonZeroUsize::new(6).expect("non-zero")),
                fast_threshold + 1,
                fast_threshold + 1,
                15,
                15,
                false,
                false,
            ),
            (
                "fast_tx_no_cap",
                None,
                fast_threshold,
                fast_threshold,
                15,
                15,
                false,
                true,
            ),
        ];
        for (
            name,
            fast_cap,
            commit_time_ms,
            effective_commit_time_ms,
            expected_target,
            expected_max,
            expected_capped,
            expected_applies,
        ) in fast_tx_cases
        {
            let (target, max_in_block, capped) = super::Actor::max_tx_budget_for_commit_time(
                20,
                20,
                NonZeroUsize::new(15),
                fast_cap,
                commit_time_ms,
                effective_commit_time_ms,
            );
            assert_eq!(target, expected_target, "{name} target");
            assert_eq!(max_in_block.get(), expected_max, "{name} max_in_block");
            assert_eq!(capped, expected_capped, "{name} capped");
            assert_eq!(
                super::Actor::fast_finality_cap_applies(commit_time_ms, effective_commit_time_ms),
                expected_applies,
                "{name} cap applies"
            );
        }

        let gas_base = NonZeroU64::new(10).expect("non-zero");
        let gas_fast_cap = NonZeroU64::new(4).expect("non-zero");
        let gas_cases = [
            (
                "gas_no_base",
                None,
                Some(gas_fast_cap),
                fast_threshold,
                fast_threshold,
                None,
                true,
            ),
            (
                "gas_no_fast_cap",
                Some(gas_base),
                None,
                fast_threshold,
                fast_threshold,
                Some(10),
                true,
            ),
            (
                "gas_fast_cap_applies",
                Some(gas_base),
                Some(gas_fast_cap),
                fast_threshold,
                fast_threshold,
                Some(4),
                true,
            ),
            (
                "gas_fast_cap_not_applicable",
                Some(gas_base),
                Some(gas_fast_cap),
                fast_threshold + 1,
                fast_threshold + 1,
                Some(10),
                false,
            ),
        ];
        for (
            name,
            gas_limit,
            fast_gas_limit,
            commit_time_ms,
            effective_commit_time_ms,
            expected_limit,
            expected_applies,
        ) in gas_cases
        {
            let actual = super::Actor::cap_gas_limit_for_fast_commit(
                gas_limit,
                commit_time_ms,
                effective_commit_time_ms,
                fast_gas_limit,
            )
            .map(NonZeroU64::get);
            assert_eq!(actual, expected_limit, "{name} limit");
            assert_eq!(
                super::Actor::fast_finality_cap_applies(commit_time_ms, effective_commit_time_ms),
                expected_applies,
                "{name} cap applies"
            );
        }

        let base = Duration::from_millis(10);
        assert_eq!(super::Actor::proposal_assembly_stale_window(base, 0), base);
        assert_eq!(super::Actor::proposal_assembly_stale_window(base, 50), base);
        assert_eq!(
            super::Actor::proposal_assembly_stale_window(
                base,
                super::PROPOSAL_STALE_WINDOW_TX_QUANTUM
            ),
            base.saturating_mul(6)
        );
        assert_eq!(
            super::Actor::proposal_assembly_stale_window(base, 140),
            base.saturating_mul(7)
        );
        assert_eq!(
            super::Actor::proposal_assembly_stale_window(
                base,
                super::PROPOSAL_STALE_WINDOW_TX_QUANTUM * 100
            ),
            base.saturating_mul(super::PROPOSAL_STALE_WINDOW_MAX_MULTIPLIER)
        );
    }

    #[test]
    fn da_payload_budget_caps_to_rbc_budget() {
        let budget = da_payload_budget(1, 8 * 1024, 1024, None);
        let rbc_budget =
            usize::try_from(super::super::RBC_MAX_TOTAL_CHUNKS).expect("fits in usize");
        assert_eq!(budget, rbc_budget.min(8 * 1024));
    }

    #[test]
    fn da_payload_budget_honors_block_payload_cap() {
        let cap = NonZeroUsize::new(4096).expect("non-zero");
        let budget = da_payload_budget(256 * 1024, 32 * 1024, 1024, Some(cap));
        assert_eq!(budget, 4096);
    }

    #[test]
    fn da_payload_budget_honors_pending_caps() {
        let budget = da_payload_budget(256 * 1024, 4 * 1024, 1, None);
        assert_eq!(budget, 4 * 1024);
    }

    #[test]
    fn da_payload_budget_is_not_limited_by_single_consensus_frame() {
        let budget = da_payload_budget(256 * 1024, 512 * 1024, 1024, None);
        assert_eq!(budget, 512 * 1024);
    }

    #[test]
    fn consensus_queue_backpressure_flags_full_queues() {
        let mut depths = status::WorkerQueueDepthSnapshot::default();
        depths.block_payload_rx = 2;
        assert!(consensus_queue_backpressure(depths, 2, 10));

        depths.block_payload_rx = 1;
        depths.rbc_chunk_rx = 5;
        assert!(consensus_queue_backpressure(depths, 10, 5));

        depths.block_payload_rx = 1;
        depths.rbc_chunk_rx = 4;
        assert!(!consensus_queue_backpressure(depths, 10, 5));
    }

    #[test]
    fn trim_batch_for_size_cap_removes_multiple_entries() {
        let mut txs = vec![1, 2, 3, 4];
        let mut routes = vec![10, 11, 12, 13];
        let mut sizes = vec![10, 10, 10, 10];
        let mut removed = Vec::new();

        let removed_count =
            trim_batch_for_size_cap(&mut txs, &mut routes, &mut sizes, &mut removed, 15);

        assert_eq!(removed_count, 2);
        assert_eq!(txs, vec![1, 2]);
        assert_eq!(routes, vec![10, 11]);
        assert_eq!(sizes, vec![10, 10]);
        assert_eq!(removed.len(), 2);
    }

    #[test]
    fn trim_batch_for_size_cap_keeps_single_entry() {
        let mut txs = vec![1, 2, 3];
        let mut routes = vec![10, 11, 12];
        let mut sizes = vec![5, 5, 5];
        let mut removed = Vec::new();

        let removed_count =
            trim_batch_for_size_cap(&mut txs, &mut routes, &mut sizes, &mut removed, 100);

        assert_eq!(removed_count, 2);
        assert_eq!(txs.len(), 1);
        assert_eq!(routes.len(), 1);
        assert_eq!(sizes.len(), 1);
    }

    #[test]
    fn canonicalize_parallel_batch_by_key_reorders_companions_stably() {
        let mut txs = vec![30, 10, 20, 10];
        let mut routes = vec!["c", "a", "b", "a2"];
        let mut sizes = vec![3, 1, 2, 4];

        canonicalize_parallel_batch_by_key(&mut txs, &mut routes, &mut sizes, |tx| *tx);

        assert_eq!(txs, vec![10, 10, 20, 30]);
        assert_eq!(routes, vec!["a", "a2", "b", "c"]);
        assert_eq!(sizes, vec![1, 4, 2, 3]);
    }

    #[test]
    fn canonicalize_proposal_batch_keeps_routing_aligned_with_transaction_order() {
        let txs = ["first", "second", "third", "fourth"]
            .into_iter()
            .map(accepted_log_transaction)
            .collect::<Vec<_>>();
        let mut entries = txs
            .into_iter()
            .enumerate()
            .map(|(idx, tx)| {
                let route = RoutingDecision::new(
                    LaneId::new(u32::try_from(idx + 1).expect("lane id")),
                    DataSpaceId::new(u64::try_from(idx + 10).expect("dataspace id")),
                );
                let size = 100 + idx;
                (tx.as_ref().hash_as_entrypoint(), tx, route, size)
            })
            .collect::<Vec<_>>();

        entries.sort_unstable_by(|left, right| right.0.cmp(&left.0));
        let mut expected = entries.clone();
        expected.sort_unstable_by(|left, right| left.0.cmp(&right.0));

        let mut tx_batch = entries
            .iter()
            .map(|(_, tx, _, _)| tx.clone())
            .collect::<Vec<_>>();
        let mut routing_batch = entries
            .iter()
            .map(|(_, _, route, _)| *route)
            .collect::<Vec<_>>();
        let mut sizes = entries
            .iter()
            .map(|(_, _, _, size)| *size)
            .collect::<Vec<_>>();

        canonicalize_proposal_batch(&mut tx_batch, &mut routing_batch, &mut sizes);

        let actual_hashes = tx_batch
            .iter()
            .map(|tx| tx.as_ref().hash_as_entrypoint())
            .collect::<Vec<_>>();
        let expected_hashes = expected
            .iter()
            .map(|(hash, _, _, _)| *hash)
            .collect::<Vec<_>>();
        let expected_routes = expected
            .iter()
            .map(|(_, _, route, _)| *route)
            .collect::<Vec<_>>();
        let expected_sizes = expected
            .iter()
            .map(|(_, _, _, size)| *size)
            .collect::<Vec<_>>();

        assert_eq!(actual_hashes, expected_hashes);
        assert_eq!(routing_batch, expected_routes);
        assert_eq!(sizes, expected_sizes);
    }

    #[test]
    fn consensus_queue_backpressure_trips_on_payload_or_rbc_queue() {
        let depths = super::status::WorkerQueueDepthSnapshot {
            block_payload_rx: 4,
            ..super::status::WorkerQueueDepthSnapshot::default()
        };
        assert!(consensus_queue_backpressure(depths, 4, 8));

        let depths = super::status::WorkerQueueDepthSnapshot {
            rbc_chunk_rx: 8,
            ..super::status::WorkerQueueDepthSnapshot::default()
        };
        assert!(consensus_queue_backpressure(depths, 4, 8));

        let depths = super::status::WorkerQueueDepthSnapshot {
            block_payload_rx: 3,
            rbc_chunk_rx: 7,
            ..super::status::WorkerQueueDepthSnapshot::default()
        };
        assert!(!consensus_queue_backpressure(depths, 4, 8));
    }

    #[test]
    fn proposal_backpressure_defers_on_consensus_queue_backpressure() {
        let backpressure = ProposalBackpressure {
            queue_state: BackpressureState::Healthy {
                queued: 0,
                capacity: NonZeroUsize::new(1).expect("non-zero"),
            },
            active_pending: false,
            rbc_backlog: false,
            relay_backpressure: false,
            consensus_queue_backpressure: true,
        };
        assert!(backpressure.should_defer());
        assert!(backpressure.only_pacing_backpressure());
    }

    #[test]
    fn age_starved_queue_override_keeps_fresh_pending_hard() {
        assert!(
            age_starved_queue_allows_stale_pending_override(true, false, true, false),
            "age-only queued ingress plus stale pending progress should bypass hard pending backpressure"
        );
        assert!(
            !age_starved_queue_allows_stale_pending_override(true, false, true, true),
            "fresh pending consensus progress should remain hard backpressure"
        );
        assert!(
            !age_starved_queue_allows_stale_pending_override(true, true, true, false),
            "capacity saturation should stay on the existing pacing path"
        );
        assert!(
            !age_starved_queue_allows_stale_pending_override(true, false, false, false),
            "the override must only activate after ingress starvation is due"
        );
    }

    #[test]
    fn timeout_streak_advances_for_repeated_height_views() {
        let now = Instant::now();
        let trigger = super::CachedSlotTimeoutTrigger {
            height: 10,
            view: 2,
            at: now,
            streak: 1,
        };

        assert_eq!(
            next_cached_slot_timeout_streak(Some(trigger), 10, 3),
            2,
            "next view at same height should increase streak"
        );
        assert_eq!(
            next_cached_slot_timeout_streak(Some(trigger), 11, 0),
            0,
            "new height should reset streak"
        );
    }

    #[test]
    fn npos_timeout_hysteresis_applies_after_previous_trigger() {
        let now = Instant::now();
        let previous = super::CachedSlotTimeoutTrigger {
            height: 42,
            view: 1,
            at: now,
            streak: 1,
        };
        let quorum_timeout = Duration::from_secs(2);
        let remaining = cached_slot_timeout_hysteresis_remaining(
            super::ConsensusMode::Npos,
            quorum_timeout,
            Some(previous),
            42,
            2,
            now + Duration::from_secs(1),
        );
        assert!(
            remaining.is_some(),
            "NPoS repeated timeout should be delayed by hysteresis window"
        );
        let no_hysteresis = cached_slot_timeout_hysteresis_remaining(
            super::ConsensusMode::Permissioned,
            quorum_timeout,
            Some(previous),
            42,
            2,
            now + Duration::from_secs(1),
        );
        assert!(
            no_hysteresis.is_none(),
            "permissioned mode should not apply NPoS hysteresis"
        );
    }
}
