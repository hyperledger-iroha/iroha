//! Deterministic per-lane proposal scheduling helpers.

use std::collections::{BTreeMap, BTreeSet, VecDeque, btree_map::Entry};

use iroha_config::parameters::actual::Nexus;
use iroha_crypto::Hash;
use iroha_data_model::{
    nexus::{DataSpaceId, LaneId, LaneRelayEnvelope, LaneRelayQuorumContext},
    peer::PeerId,
};
use norito::codec::Encode;

use crate::queue::RoutingDecision;

/// Return true when proposal assembly should look beyond the remaining block
/// slots to discover work from other currently routable lanes.
///
/// The gate intentionally uses the same height-aware routing surface as the
/// queue router. Sidecar lanes that no policy path can select, future-created
/// autoscale lanes, malformed autoscale anchors, and disabled Nexus state do not
/// enable broader scans.
#[must_use]
pub(super) fn proposal_lookahead_enabled(nexus: &Nexus, block_height: u64) -> bool {
    crate::queue::routable_lane_ids_for_nexus_at_height(nexus, block_height).len() > 1
}

/// Compute how many queued transactions proposal assembly may inspect next.
///
/// Single-lane slots scan only up to the remaining block capacity. Multi-lane
/// slots may scan the full remaining scan budget so schedulable work on later
/// lanes can be found even when the first lane is already saturated or over a
/// per-lane TEU limit. The caller is still responsible for enforcing block
/// capacity before admitting transactions.
#[must_use]
pub(super) fn proposal_fetch_cap(
    nexus: &Nexus,
    block_height: u64,
    remaining_budget: usize,
    remaining_slots: usize,
) -> usize {
    if remaining_budget == 0 || remaining_slots == 0 {
        return 0;
    }
    if proposal_lookahead_enabled(nexus, block_height) {
        remaining_budget
    } else {
        remaining_budget.min(remaining_slots)
    }
}

/// Scheduler-visible properties for a fetched proposal candidate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ProposalAdmissionCandidate {
    /// Gas cost charged to the global proposal gas budget.
    pub(super) gas_cost: u64,
    /// Whether this transaction consumes one IVM-heavy proposal slot.
    pub(super) is_ivm_heavy: bool,
}

/// Current proposal resource usage at a candidate admission point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ProposalAdmissionContext {
    /// Transactions already accepted before the current fetched batch.
    pub(super) accepted_before_batch: usize,
    /// Transactions accepted from the current fetched batch.
    pub(super) accepted_in_batch: usize,
    /// Maximum number of transactions allowed in the proposal block.
    pub(super) max_in_block: usize,
    /// Optional global gas budget for the proposal block.
    pub(super) gas_limit_per_block: Option<u64>,
    /// Gas already consumed by accepted proposal candidates.
    pub(super) gas_used_in_block: u64,
    /// Optional cap for IVM-heavy transactions in the proposal block.
    pub(super) max_ivm_transactions: Option<usize>,
    /// IVM-heavy transactions already accepted into the proposal block.
    pub(super) ivm_transactions_included: usize,
}

/// Reason a fetched proposal candidate should be deferred.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ProposalDeferralReason {
    /// The block has no remaining transaction slots.
    BlockFull,
    /// The proposal already reached its IVM-heavy transaction cap.
    IvmLimit,
    /// The candidate does not fit the remaining gas budget.
    GasLimit,
    /// Lane-local consensus metadata could not be planned securely.
    LaneConsensus,
}

/// Admission decision for a fetched proposal candidate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ProposalAdmissionDecision {
    /// Admit the candidate. `exceeds_gas_limit` is true only for the
    /// oversized-first fallback that avoids proposal stalls.
    Accept { exceeds_gas_limit: bool },
    /// Defer the candidate and requeue it with its current routing plan.
    Defer { reason: ProposalDeferralReason },
}

/// Error returned when proposal batch scheduling inputs are internally inconsistent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ProposalBatchScheduleError {
    /// Candidate and routing vectors do not describe the same fetched batch.
    CandidateRoutingLengthMismatch {
        /// Number of candidate resource records.
        candidates: usize,
        /// Number of routing decisions.
        routing_decisions: usize,
    },
}

/// Scheduler action for one fetched proposal candidate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ProposalBatchAction {
    /// Admit the fetched candidate at `index`.
    Accept {
        /// Candidate index in the fetched batch.
        index: usize,
        /// Whether this candidate used the oversized-first liveness fallback.
        exceeds_gas_limit: bool,
    },
    /// Defer the fetched candidate at `index`.
    Defer {
        /// Candidate index in the fetched batch.
        index: usize,
        /// Resource boundary that caused deferral.
        reason: ProposalDeferralReason,
    },
}

/// Deterministic action plan for a fetched proposal batch.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct ProposalBatchSchedule {
    /// Actions to apply in scheduler order.
    pub(super) actions: Vec<ProposalBatchAction>,
    /// Gas added by accepted candidates in this batch.
    pub(super) gas_used_delta: u64,
    /// IVM-heavy transactions accepted from this batch.
    pub(super) ivm_transactions_included_delta: usize,
    /// IVM-heavy transactions deferred by this batch.
    pub(super) ivm_transactions_deferred: usize,
}

/// Validator committee declared for a lane-local consensus domain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct LaneConsensusCommittee {
    /// Lane certified by this committee.
    pub(super) lane_id: LaneId,
    /// Dataspace bound to the lane for this committee.
    pub(super) dataspace_id: DataSpaceId,
    /// Validator peers eligible to sign lane-local votes.
    pub(super) validators: Vec<PeerId>,
    /// Optional explicit quorum threshold. When omitted, the standard commit
    /// quorum for the validator set length is used.
    pub(super) min_quorum: Option<u32>,
}

/// Deterministic lane-local vote/QC domain for accepted proposal work.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct LaneConsensusDomain {
    /// Lane certified by this domain.
    pub(super) lane_id: LaneId,
    /// Dataspace bound to the lane for this domain.
    pub(super) dataspace_id: DataSpaceId,
    /// Accepted proposal candidates assigned to this lane in the scheduled batch.
    pub(super) accepted_candidates: usize,
    /// Fetched-batch candidate indices assigned to this lane, in scheduler order.
    pub(super) accepted_candidate_indices: Vec<usize>,
    /// Canonical validator order used by signer bitmaps.
    pub(super) validator_set: Vec<PeerId>,
    /// Quorum context for validating lane relay QCs.
    pub(super) quorum: LaneRelayQuorumContext,
    /// Domain-separated mode tag used for lane-local vote signatures.
    pub(super) qc_mode_tag: String,
}

/// Deterministic subject for lane-local block votes and DA ownership.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct LaneBlockSubject {
    /// Lane whose work is bound by this subject.
    pub(super) lane_id: LaneId,
    /// Dataspace bound to the lane work.
    pub(super) dataspace_id: DataSpaceId,
    /// Lane-local block height assigned by the caller.
    pub(super) lane_block_height: u64,
    /// Lane-local view assigned by the caller.
    pub(super) lane_block_view: u64,
    /// Fetched-batch candidate indices committed by this subject.
    pub(super) accepted_candidate_indices: Vec<usize>,
    /// Domain-separated QC mode tag used for lane-local votes.
    pub(super) qc_mode_tag: String,
    /// Stable Norito-backed digest of the subject preimage.
    pub(super) subject_hash: Hash,
}

/// Committed lane-local block tip known before planning the next slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct LaneBlockTip {
    /// Lane whose tip is being supplied.
    pub(super) lane_id: LaneId,
    /// Dataspace bound to the lane tip.
    pub(super) dataspace_id: DataSpaceId,
    /// Latest committed lane-local block height. Use zero for a newly created
    /// lane with no committed lane-local block yet.
    pub(super) latest_lane_block_height: u64,
}

/// Lane-local slot coordinates assigned before subject derivation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct LaneBlockSlot {
    /// Lane whose next block slot is being planned.
    pub(super) lane_id: LaneId,
    /// Dataspace expected for the lane slot.
    pub(super) dataspace_id: DataSpaceId,
    /// Lane-local block height for this slot.
    pub(super) lane_block_height: u64,
    /// Lane-local view for this slot.
    pub(super) lane_block_view: u64,
}

/// Deterministic DA/RBC ownership identity for one lane-local block subject.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct LanePayloadOwnership {
    /// Lane whose payload ownership is bound by this identity.
    pub(super) lane_id: LaneId,
    /// Dataspace bound to the lane payload.
    pub(super) dataspace_id: DataSpaceId,
    /// Lane-local block height for the payload.
    pub(super) lane_block_height: u64,
    /// Lane-local view for the payload.
    pub(super) lane_block_view: u64,
    /// Subject digest validated before deriving ownership identity.
    pub(super) subject_hash: Hash,
    /// Domain-separated QC mode tag used for lane-local votes.
    pub(super) qc_mode_tag: String,
    /// Fetched-batch candidate indices owned by this lane payload.
    pub(super) accepted_candidate_indices: Vec<usize>,
    /// Stable digest naming lane-local payload ownership.
    pub(super) payload_ownership_hash: Hash,
    /// Stable digest naming the lane-local RBC instance for this payload.
    pub(super) rbc_instance_hash: Hash,
}

/// Full deterministic lane payload plan derived for accepted proposal work.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct LanePayloadPlan {
    /// Latest lane tips selected for accepted work after reset filtering.
    pub(super) lane_tips: Vec<LaneBlockTip>,
    /// Next lane-local block slots derived from the selected tips.
    pub(super) slots: Vec<LaneBlockSlot>,
    /// Lane-local vote/DA subjects for the selected slots.
    pub(super) subjects: Vec<LaneBlockSubject>,
    /// DA/RBC ownership identities for the selected subjects.
    pub(super) ownerships: Vec<LanePayloadOwnership>,
}

/// Error returned when lane-local slots cannot be derived from lane tips.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LaneBlockSlotPlanError {
    /// More than one accepted consensus domain was provided for the same lane.
    DuplicateLaneDomain {
        /// Duplicated lane identifier.
        lane_id: LaneId,
        /// Dataspace selected by the first accepted domain.
        dataspace_id: DataSpaceId,
    },
    /// More than one latest-tip descriptor was provided for the same lane.
    DuplicateLaneTip {
        /// Duplicated lane identifier.
        lane_id: LaneId,
    },
    /// Accepted work references a lane without an explicit latest-tip descriptor.
    MissingLaneTip {
        /// Lane missing latest-tip coordinates.
        lane_id: LaneId,
    },
    /// Latest-tip dataspace does not match the accepted lane work.
    LaneTipDataspaceMismatch {
        /// Lane being planned.
        lane_id: LaneId,
        /// Dataspace selected by accepted work.
        expected: DataSpaceId,
        /// Dataspace declared by the latest-tip descriptor.
        actual: DataSpaceId,
    },
    /// Advancing the lane-local block height would overflow.
    LaneBlockHeightOverflow {
        /// Lane being planned.
        lane_id: LaneId,
        /// Latest committed lane-local block height.
        latest_lane_block_height: u64,
    },
}

/// Error returned when latest lane tips cannot be reduced from known tip candidates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LaneBlockTipPlanError {
    /// More than one accepted consensus domain was provided for the same lane.
    DuplicateLaneDomain {
        /// Duplicated lane identifier.
        lane_id: LaneId,
        /// Dataspace selected by the first accepted domain.
        dataspace_id: DataSpaceId,
    },
    /// A known tip for an accepted lane carries a different dataspace.
    LaneTipDataspaceMismatch {
        /// Lane being planned.
        lane_id: LaneId,
        /// Dataspace selected by accepted work.
        expected: DataSpaceId,
        /// Dataspace declared by the known tip.
        actual: DataSpaceId,
    },
}

/// Error returned when lane-local block subjects cannot be derived safely.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LaneBlockSubjectError {
    /// Consensus domain has a blank mode tag.
    BlankQcModeTag {
        /// Lane being planned.
        lane_id: LaneId,
    },
    /// Consensus domain has no accepted work to bind.
    EmptyCandidateSet {
        /// Lane being planned.
        lane_id: LaneId,
    },
    /// Consensus domain count disagrees with its candidate index list.
    CandidateCountMismatch {
        /// Lane being planned.
        lane_id: LaneId,
        /// Accepted-candidate count advertised by the domain.
        accepted_candidates: usize,
        /// Number of candidate indices carried by the domain.
        candidate_indices: usize,
    },
    /// Consensus domain repeats a fetched-batch candidate index.
    DuplicateCandidateIndex {
        /// Lane being planned.
        lane_id: LaneId,
        /// Duplicated fetched-batch index.
        index: usize,
    },
    /// More than one domain was provided for the same lane.
    DuplicateLaneDomain {
        /// Duplicated lane identifier.
        lane_id: LaneId,
    },
    /// More than one slot descriptor was provided for the same lane.
    DuplicateLaneSlot {
        /// Duplicated lane identifier.
        lane_id: LaneId,
    },
    /// Accepted work references a lane without a lane-local slot descriptor.
    MissingLaneSlot {
        /// Lane missing slot coordinates.
        lane_id: LaneId,
    },
    /// A slot descriptor was provided for a lane without accepted work.
    UnexpectedLaneSlot {
        /// Unexpected lane identifier.
        lane_id: LaneId,
    },
    /// Slot dataspace does not match the accepted lane work.
    LaneSlotDataspaceMismatch {
        /// Lane being planned.
        lane_id: LaneId,
        /// Dataspace selected by accepted work.
        expected: DataSpaceId,
        /// Dataspace declared by the slot descriptor.
        actual: DataSpaceId,
    },
    /// Candidate index does not fit the architecture-neutral digest preimage.
    CandidateIndexOverflow {
        /// Lane being planned.
        lane_id: LaneId,
        /// Candidate index that could not be represented as `u64`.
        index: usize,
    },
    /// Canonical subject preimage encoding failed.
    Encode,
}

/// Error returned when lane-local DA/RBC ownership identities cannot be planned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LanePayloadOwnershipError {
    /// Block subject has a blank QC mode tag.
    BlankQcModeTag {
        /// Lane being planned.
        lane_id: LaneId,
    },
    /// Block subject has no accepted work to bind.
    EmptyCandidateSet {
        /// Lane being planned.
        lane_id: LaneId,
    },
    /// Block subject repeats a fetched-batch candidate index.
    DuplicateCandidateIndex {
        /// Lane being planned.
        lane_id: LaneId,
        /// Duplicated fetched-batch index.
        index: usize,
    },
    /// Candidate index does not fit the architecture-neutral digest preimage.
    CandidateIndexOverflow {
        /// Lane being planned.
        lane_id: LaneId,
        /// Candidate index that could not be represented as `u64`.
        index: usize,
    },
    /// Block subject digest does not match its canonical preimage.
    SubjectHashMismatch {
        /// Lane being planned.
        lane_id: LaneId,
        /// Digest recomputed from the canonical preimage.
        expected: Hash,
        /// Digest carried by the subject.
        actual: Hash,
    },
    /// More than one subject was provided for the same lane-local slot.
    DuplicateLaneSlot {
        /// Duplicated lane identifier.
        lane_id: LaneId,
        /// Duplicated dataspace identifier.
        dataspace_id: DataSpaceId,
        /// Duplicated lane-local block height.
        lane_block_height: u64,
        /// Duplicated lane-local block view.
        lane_block_view: u64,
    },
    /// Two subjects produced the same payload ownership digest.
    DuplicatePayloadOwnershipHash {
        /// Duplicated payload ownership digest.
        payload_ownership_hash: Hash,
    },
    /// Two subjects produced the same RBC instance digest.
    DuplicateRbcInstanceHash {
        /// Duplicated RBC instance digest.
        rbc_instance_hash: Hash,
    },
    /// Canonical ownership preimage encoding failed.
    Encode,
}

/// Error returned when a full lane payload plan cannot be derived.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LanePayloadPlanError {
    /// Latest lane tips could not be reduced for the accepted domains.
    Tips(LaneBlockTipPlanError),
    /// Next lane-local slots could not be derived from the selected tips.
    Slots(LaneBlockSlotPlanError),
    /// Lane-local block subjects could not be derived from the selected slots.
    Subjects(LaneBlockSubjectError),
    /// DA/RBC ownership identities could not be derived from the subjects.
    Ownerships(LanePayloadOwnershipError),
}

/// Error returned when a lane-local consensus domain cannot be derived safely.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LaneConsensusDomainError {
    /// Base consensus mode tag is empty or whitespace.
    BlankBaseModeTag,
    /// Scheduler action references a candidate outside the fetched routing batch.
    ActionIndexOutOfBounds {
        /// Referenced candidate index.
        index: usize,
        /// Number of fetched routing decisions.
        routing_decisions: usize,
    },
    /// Accepted candidates for one lane disagree on dataspace ownership.
    AcceptedLaneDataspaceMismatch {
        /// Lane with inconsistent routing decisions.
        lane_id: LaneId,
        /// Dataspace first accepted for the lane.
        expected: DataSpaceId,
        /// Later accepted dataspace for the same lane.
        actual: DataSpaceId,
    },
    /// More than one committee descriptor was provided for the same lane.
    DuplicateLaneCommittee {
        /// Duplicated lane identifier.
        lane_id: LaneId,
    },
    /// Accepted work references a lane without a committee descriptor.
    MissingLaneCommittee {
        /// Lane missing a committee.
        lane_id: LaneId,
    },
    /// Committee dataspace does not match the accepted lane work.
    CommitteeDataspaceMismatch {
        /// Lane being planned.
        lane_id: LaneId,
        /// Dataspace selected by accepted work.
        expected: DataSpaceId,
        /// Dataspace declared by the committee descriptor.
        actual: DataSpaceId,
    },
    /// Committee validator set is empty.
    EmptyValidatorSet {
        /// Lane with no validators.
        lane_id: LaneId,
    },
    /// Committee validator set contains a duplicate peer.
    DuplicateValidator {
        /// Lane with duplicate validators.
        lane_id: LaneId,
    },
    /// Committee validator count does not fit the relay quorum format.
    ValidatorCountOverflow {
        /// Lane whose validator set is too large.
        lane_id: LaneId,
    },
    /// Committee quorum is zero or larger than the validator set.
    InvalidQuorum {
        /// Lane with the invalid quorum.
        lane_id: LaneId,
        /// Number of validators in the committee.
        validator_count: u32,
        /// Required quorum threshold.
        min_quorum: u32,
    },
}

/// Decide whether a fetched candidate should enter the current proposal.
///
/// The decision is deterministic and side-effect free so the global proposal
/// path and future lane-local schedulers can share the same resource-boundary
/// behavior. In particular, an oversized first candidate is only admitted when
/// no later still-eligible candidate fits the remaining gas budget.
#[must_use]
pub(super) fn decide_proposal_candidate_admission<I>(
    candidate: ProposalAdmissionCandidate,
    later_candidates: I,
    context: ProposalAdmissionContext,
) -> ProposalAdmissionDecision
where
    I: IntoIterator<Item = ProposalAdmissionCandidate>,
{
    if context
        .accepted_before_batch
        .saturating_add(context.accepted_in_batch)
        >= context.max_in_block
    {
        return ProposalAdmissionDecision::Defer {
            reason: ProposalDeferralReason::BlockFull,
        };
    }

    if let Some(limit) = context.max_ivm_transactions
        && candidate.is_ivm_heavy
        && context.ivm_transactions_included >= limit
    {
        return ProposalAdmissionDecision::Defer {
            reason: ProposalDeferralReason::IvmLimit,
        };
    }

    if let Some(limit) = context.gas_limit_per_block {
        let remaining_gas = limit.saturating_sub(context.gas_used_in_block);
        let would_exceed = candidate.gas_cost > remaining_gas && candidate.gas_cost > 0;
        let allow_oversized = context.gas_used_in_block == 0
            && context.accepted_before_batch == 0
            && context.accepted_in_batch == 0;
        let fitting_later_candidate = would_exceed
            && allow_oversized
            && later_candidates.into_iter().any(|candidate| {
                candidate_fits_remaining_resources(candidate, remaining_gas, context)
            });

        if would_exceed && (!allow_oversized || fitting_later_candidate) {
            return ProposalAdmissionDecision::Defer {
                reason: ProposalDeferralReason::GasLimit,
            };
        }

        if would_exceed {
            return ProposalAdmissionDecision::Accept {
                exceeds_gas_limit: true,
            };
        }
    }

    ProposalAdmissionDecision::Accept {
        exceeds_gas_limit: false,
    }
}

fn candidate_fits_remaining_resources(
    candidate: ProposalAdmissionCandidate,
    remaining_gas: u64,
    context: ProposalAdmissionContext,
) -> bool {
    context
        .max_ivm_transactions
        .is_none_or(|max| !candidate.is_ivm_heavy || context.ivm_transactions_included < max)
        && candidate.gas_cost <= remaining_gas
}

/// Build a deterministic action plan for an already-fetched proposal batch.
///
/// This combines lane interleaving with block-slot, gas, and IVM-heavy admission
/// policy while staying side-effect free. Queue guard ownership, lane-TEU release,
/// and requeue persistence remain with the caller.
pub(super) fn schedule_proposal_batch(
    routing_decisions: &[RoutingDecision],
    candidates: &[ProposalAdmissionCandidate],
    context: ProposalAdmissionContext,
    height: u64,
    view: u64,
) -> Result<ProposalBatchSchedule, ProposalBatchScheduleError> {
    if routing_decisions.len() != candidates.len() {
        return Err(ProposalBatchScheduleError::CandidateRoutingLengthMismatch {
            candidates: candidates.len(),
            routing_decisions: routing_decisions.len(),
        });
    }

    let order = LaneProposalBatch::from_routing_decisions(routing_decisions)
        .interleaved_indices_for_slot(height, view);
    let mut context = context;
    let mut schedule = ProposalBatchSchedule {
        actions: Vec::with_capacity(order.len()),
        ..ProposalBatchSchedule::default()
    };
    for (order_pos, idx) in order.iter().copied().enumerate() {
        let candidate = candidates[idx];
        let later_candidates = order
            .iter()
            .skip(order_pos + 1)
            .map(|candidate_idx| candidates[*candidate_idx]);
        match decide_proposal_candidate_admission(candidate, later_candidates, context) {
            ProposalAdmissionDecision::Accept { exceeds_gas_limit } => {
                schedule.actions.push(ProposalBatchAction::Accept {
                    index: idx,
                    exceeds_gas_limit,
                });
                context.accepted_in_batch = context.accepted_in_batch.saturating_add(1);
                if context.gas_limit_per_block.is_some() {
                    context.gas_used_in_block =
                        context.gas_used_in_block.saturating_add(candidate.gas_cost);
                    schedule.gas_used_delta =
                        schedule.gas_used_delta.saturating_add(candidate.gas_cost);
                }
                if candidate.is_ivm_heavy {
                    context.ivm_transactions_included =
                        context.ivm_transactions_included.saturating_add(1);
                    schedule.ivm_transactions_included_delta =
                        schedule.ivm_transactions_included_delta.saturating_add(1);
                }
            }
            ProposalAdmissionDecision::Defer { reason } => {
                if reason == ProposalDeferralReason::IvmLimit {
                    schedule.ivm_transactions_deferred =
                        schedule.ivm_transactions_deferred.saturating_add(1);
                }
                schedule
                    .actions
                    .push(ProposalBatchAction::Defer { index: idx, reason });
            }
        }
    }

    Ok(schedule)
}

/// Convert accepted batch actions into deferrals without charging accepted
/// resource counters.
///
/// This is used when a later consensus-critical planning stage determines that
/// accepted candidates cannot be proposed safely. Existing deferrals are
/// preserved with their original resource-boundary reasons so queue reordering
/// remains stable and diagnostics stay precise.
#[must_use]
pub(super) fn defer_accepted_proposal_actions(
    schedule: &ProposalBatchSchedule,
    reason: ProposalDeferralReason,
) -> ProposalBatchSchedule {
    ProposalBatchSchedule {
        actions: schedule
            .actions
            .iter()
            .map(|action| match *action {
                ProposalBatchAction::Accept { index, .. } => {
                    ProposalBatchAction::Defer { index, reason }
                }
                action @ ProposalBatchAction::Defer { .. } => action,
            })
            .collect(),
        gas_used_delta: 0,
        ivm_transactions_included_delta: 0,
        ivm_transactions_deferred: schedule.ivm_transactions_deferred,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LaneAcceptedWork {
    dataspace_id: DataSpaceId,
    candidate_indices: Vec<usize>,
}

#[derive(Clone, Debug, Encode)]
struct LaneBlockSubjectPreimage {
    version: u8,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_block_height: u64,
    lane_block_view: u64,
    candidate_indices: Vec<u64>,
    qc_mode_tag: String,
}

#[derive(Clone, Debug, Encode)]
struct LanePayloadOwnershipPreimage {
    purpose: String,
    version: u8,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_block_height: u64,
    lane_block_view: u64,
    subject_hash: Hash,
    candidate_indices: Vec<u64>,
    qc_mode_tag: String,
}

#[derive(Clone, Debug, Encode)]
struct LaneRbcInstancePreimage {
    purpose: String,
    version: u8,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_block_height: u64,
    lane_block_view: u64,
    subject_hash: Hash,
    payload_ownership_hash: Hash,
}

/// Derive lane-local vote/QC domains for accepted work in a scheduled batch.
///
/// The returned domains are sorted by lane id, include only accepted candidates,
/// and use the same lane relay mode-tag helper as [`LaneRelayEnvelope`] so
/// later relay QCs cannot accidentally share the global consensus domain.
pub(super) fn plan_lane_consensus_domains(
    routing_decisions: &[RoutingDecision],
    schedule: &ProposalBatchSchedule,
    committees: &[LaneConsensusCommittee],
    base_mode_tag: &str,
) -> Result<Vec<LaneConsensusDomain>, LaneConsensusDomainError> {
    if base_mode_tag.trim().is_empty() {
        return Err(LaneConsensusDomainError::BlankBaseModeTag);
    }

    let accepted_work = accepted_work_by_lane(routing_decisions, schedule)?;
    if accepted_work.is_empty() {
        return Ok(Vec::new());
    }

    let committees = committees_by_lane(committees)?;
    let mut domains = Vec::with_capacity(accepted_work.len());
    for (lane_id, work) in accepted_work {
        let committee = committees
            .get(&lane_id)
            .ok_or(LaneConsensusDomainError::MissingLaneCommittee { lane_id })?;
        if committee.dataspace_id != work.dataspace_id {
            return Err(LaneConsensusDomainError::CommitteeDataspaceMismatch {
                lane_id,
                expected: work.dataspace_id,
                actual: committee.dataspace_id,
            });
        }
        let validator_set = canonical_validator_set(lane_id, &committee.validators)?;
        let validator_count = u32::try_from(validator_set.len())
            .map_err(|_| LaneConsensusDomainError::ValidatorCountOverflow { lane_id })?;
        let min_quorum = committee.min_quorum.unwrap_or_else(|| {
            u32::try_from(crate::sumeragi::network_topology::commit_quorum_from_len(
                validator_set.len(),
            ))
            .expect("commit quorum for u32-sized validator set fits u32")
        });
        let quorum = LaneRelayQuorumContext::new(validator_count, min_quorum).map_err(|_| {
            LaneConsensusDomainError::InvalidQuorum {
                lane_id,
                validator_count,
                min_quorum,
            }
        })?;
        domains.push(LaneConsensusDomain {
            lane_id,
            dataspace_id: work.dataspace_id,
            accepted_candidates: work.candidate_indices.len(),
            accepted_candidate_indices: work.candidate_indices,
            validator_set,
            quorum,
            qc_mode_tag: LaneRelayEnvelope::lane_qc_mode_tag_for(
                lane_id,
                work.dataspace_id,
                base_mode_tag,
            ),
        });
    }

    Ok(domains)
}

/// Derive lane-local committee descriptors for accepted proposal work.
///
/// The authority callback is evaluated only for lanes with accepted work. If it
/// returns no validators, `fallback_validators` are used only when explicitly
/// supplied by the caller; enabled multi-lane paths should pass `None` so stale
/// or missing lane authority fails closed instead of inheriting global
/// topology accidentally.
pub(super) fn plan_lane_consensus_committees_with_authority<F>(
    routing_decisions: &[RoutingDecision],
    schedule: &ProposalBatchSchedule,
    fallback_validators: Option<&[PeerId]>,
    mut validators_for_lane: F,
) -> Result<Vec<LaneConsensusCommittee>, LaneConsensusDomainError>
where
    F: FnMut(LaneId, DataSpaceId) -> Vec<PeerId>,
{
    accepted_work_by_lane(routing_decisions, schedule)?
        .into_iter()
        .map(|(lane_id, work)| {
            let mut validators = validators_for_lane(lane_id, work.dataspace_id);
            if validators.is_empty() {
                let Some(fallback) = fallback_validators else {
                    return Err(LaneConsensusDomainError::MissingLaneCommittee { lane_id });
                };
                validators = fallback.to_vec();
            }
            Ok(LaneConsensusCommittee {
                lane_id,
                dataspace_id: work.dataspace_id,
                validators,
                min_quorum: None,
            })
        })
        .collect()
}

/// Reduce known lane-local tips while honoring lane reset watermarks.
///
/// Accepted lanes without a known tip receive the compatibility latest height
/// supplied by the caller. The current global proposal path uses
/// `global_height - 1` for that value so missing lane-local relay history keeps
/// the existing global-height anchor, while lanes with relay history can
/// advance independently.
/// A reset watermark is treated as the latest height of the previous lane
/// incarnation. Planning floors both known and compatibility tips at that
/// watermark so recreated lanes resume at `reset_height + 1` and never reuse
/// old lane-local block coordinates.
pub(super) fn plan_latest_lane_block_tips_with_reset_heights(
    domains: &[LaneConsensusDomain],
    known_tips: &[LaneBlockTip],
    compatibility_latest_lane_block_height: u64,
    reset_heights: &BTreeMap<LaneId, u64>,
) -> Result<Vec<LaneBlockTip>, LaneBlockTipPlanError> {
    let mut domains_by_lane = BTreeMap::new();
    for domain in domains {
        if domains_by_lane.insert(domain.lane_id, domain).is_some() {
            return Err(LaneBlockTipPlanError::DuplicateLaneDomain {
                lane_id: domain.lane_id,
                dataspace_id: domain.dataspace_id,
            });
        }
    }

    let mut latest_by_lane: BTreeMap<LaneId, LaneBlockTip> = BTreeMap::new();
    for tip in known_tips {
        let Some(domain) = domains_by_lane.get(&tip.lane_id) else {
            continue;
        };
        let reset_height = reset_heights.get(&tip.lane_id).copied();
        if tip.dataspace_id != domain.dataspace_id {
            if reset_height.is_some_and(|height| tip.latest_lane_block_height <= height) {
                continue;
            }
            return Err(LaneBlockTipPlanError::LaneTipDataspaceMismatch {
                lane_id: tip.lane_id,
                expected: domain.dataspace_id,
                actual: tip.dataspace_id,
            });
        }
        let floor_height = reset_height.unwrap_or(0);
        let floored_tip = LaneBlockTip {
            latest_lane_block_height: tip.latest_lane_block_height.max(floor_height),
            ..*tip
        };
        match latest_by_lane.entry(tip.lane_id) {
            Entry::Occupied(mut entry) => {
                if floored_tip.latest_lane_block_height > entry.get().latest_lane_block_height {
                    entry.insert(floored_tip);
                }
            }
            Entry::Vacant(entry) => {
                entry.insert(floored_tip);
            }
        }
    }

    Ok(domains_by_lane
        .into_iter()
        .map(|(lane_id, domain)| {
            let baseline = compatibility_latest_lane_block_height
                .max(reset_heights.get(&lane_id).copied().unwrap_or(0));
            latest_by_lane
                .get(&lane_id)
                .copied()
                .unwrap_or(LaneBlockTip {
                    lane_id,
                    dataspace_id: domain.dataspace_id,
                    latest_lane_block_height: baseline,
                })
        })
        .collect())
}

/// Derive the next lane-local block slots from explicit latest lane tips.
///
/// Every accepted lane must have exactly one latest-tip descriptor. A newly
/// created lane that has not committed a lane-local block yet must still be
/// represented explicitly with `latest_lane_block_height = 0`, which prevents
/// missing state from silently resetting an established lane to height 1.
pub(super) fn plan_next_lane_block_slots(
    domains: &[LaneConsensusDomain],
    lane_tips: &[LaneBlockTip],
    lane_block_view: u64,
) -> Result<Vec<LaneBlockSlot>, LaneBlockSlotPlanError> {
    let mut tips_by_lane = BTreeMap::new();
    for tip in lane_tips {
        if tips_by_lane.insert(tip.lane_id, tip).is_some() {
            return Err(LaneBlockSlotPlanError::DuplicateLaneTip {
                lane_id: tip.lane_id,
            });
        }
    }

    let mut seen_lanes = BTreeSet::new();
    let mut slots = Vec::with_capacity(domains.len());
    for domain in domains {
        if !seen_lanes.insert(domain.lane_id) {
            return Err(LaneBlockSlotPlanError::DuplicateLaneDomain {
                lane_id: domain.lane_id,
                dataspace_id: domain.dataspace_id,
            });
        }
        let tip =
            tips_by_lane
                .get(&domain.lane_id)
                .ok_or(LaneBlockSlotPlanError::MissingLaneTip {
                    lane_id: domain.lane_id,
                })?;
        if tip.dataspace_id != domain.dataspace_id {
            return Err(LaneBlockSlotPlanError::LaneTipDataspaceMismatch {
                lane_id: domain.lane_id,
                expected: domain.dataspace_id,
                actual: tip.dataspace_id,
            });
        }
        let lane_block_height = tip.latest_lane_block_height.checked_add(1).ok_or(
            LaneBlockSlotPlanError::LaneBlockHeightOverflow {
                lane_id: domain.lane_id,
                latest_lane_block_height: tip.latest_lane_block_height,
            },
        )?;
        slots.push(LaneBlockSlot {
            lane_id: domain.lane_id,
            dataspace_id: domain.dataspace_id,
            lane_block_height,
            lane_block_view,
        });
    }

    slots.sort_by_key(|slot| {
        (
            slot.lane_id,
            slot.dataspace_id,
            slot.lane_block_height,
            slot.lane_block_view,
        )
    });
    Ok(slots)
}

/// Derive deterministic lane block subjects from lane-local consensus domains.
///
/// Subjects are sorted by lane id/dataspace id and bind the lane coordinates,
/// caller-supplied lane height/view, exact fetched-batch candidate order, and
/// lane QC mode tag into a stable Norito-backed digest. The current global
/// proposal path can call this with global height/view as a compatibility
/// anchor; a full per-lane scheduler can later supply independent lane-local
/// heights without changing the subject validation rules.
#[cfg(test)]
fn plan_lane_block_subjects(
    domains: &[LaneConsensusDomain],
    lane_block_height: u64,
    lane_block_view: u64,
) -> Result<Vec<LaneBlockSubject>, LaneBlockSubjectError> {
    let mut seen_lanes = BTreeSet::new();
    for domain in domains {
        if !seen_lanes.insert(domain.lane_id) {
            return Err(LaneBlockSubjectError::DuplicateLaneDomain {
                lane_id: domain.lane_id,
            });
        }
    }
    let slots = domains
        .iter()
        .map(|domain| LaneBlockSlot {
            lane_id: domain.lane_id,
            dataspace_id: domain.dataspace_id,
            lane_block_height,
            lane_block_view,
        })
        .collect::<Vec<_>>();
    plan_lane_block_subjects_for_slots(domains, &slots)
}

/// Derive deterministic lane block subjects from explicit lane-local slots.
///
/// Unlike [`plan_lane_block_subjects`], this accepts independent height/view
/// coordinates per lane. That lets the future independent lane scheduler plan
/// subjects for lanes that advance at different rates while preserving the
/// same canonical digest and validation rules used by the current global
/// compatibility path.
pub(super) fn plan_lane_block_subjects_for_slots(
    domains: &[LaneConsensusDomain],
    slots: &[LaneBlockSlot],
) -> Result<Vec<LaneBlockSubject>, LaneBlockSubjectError> {
    let mut slots_by_lane = BTreeMap::new();
    for slot in slots {
        if slots_by_lane.insert(slot.lane_id, slot).is_some() {
            return Err(LaneBlockSubjectError::DuplicateLaneSlot {
                lane_id: slot.lane_id,
            });
        }
    }

    let mut seen_lanes = BTreeSet::new();
    let mut subjects = Vec::with_capacity(domains.len());
    for domain in domains {
        if domain.qc_mode_tag.trim().is_empty() {
            return Err(LaneBlockSubjectError::BlankQcModeTag {
                lane_id: domain.lane_id,
            });
        }
        if domain.accepted_candidate_indices.is_empty() {
            return Err(LaneBlockSubjectError::EmptyCandidateSet {
                lane_id: domain.lane_id,
            });
        }
        if domain.accepted_candidates != domain.accepted_candidate_indices.len() {
            return Err(LaneBlockSubjectError::CandidateCountMismatch {
                lane_id: domain.lane_id,
                accepted_candidates: domain.accepted_candidates,
                candidate_indices: domain.accepted_candidate_indices.len(),
            });
        }
        if !seen_lanes.insert(domain.lane_id) {
            return Err(LaneBlockSubjectError::DuplicateLaneDomain {
                lane_id: domain.lane_id,
            });
        }
        let slot =
            slots_by_lane
                .get(&domain.lane_id)
                .ok_or(LaneBlockSubjectError::MissingLaneSlot {
                    lane_id: domain.lane_id,
                })?;
        if slot.dataspace_id != domain.dataspace_id {
            return Err(LaneBlockSubjectError::LaneSlotDataspaceMismatch {
                lane_id: domain.lane_id,
                expected: domain.dataspace_id,
                actual: slot.dataspace_id,
            });
        }

        let mut seen_indices = BTreeSet::new();
        let mut candidate_indices = Vec::with_capacity(domain.accepted_candidate_indices.len());
        for index in domain.accepted_candidate_indices.iter().copied() {
            if !seen_indices.insert(index) {
                return Err(LaneBlockSubjectError::DuplicateCandidateIndex {
                    lane_id: domain.lane_id,
                    index,
                });
            }
            candidate_indices.push(u64::try_from(index).map_err(|_| {
                LaneBlockSubjectError::CandidateIndexOverflow {
                    lane_id: domain.lane_id,
                    index,
                }
            })?);
        }

        let preimage = LaneBlockSubjectPreimage {
            version: 1,
            lane_id: domain.lane_id,
            dataspace_id: domain.dataspace_id,
            lane_block_height: slot.lane_block_height,
            lane_block_view: slot.lane_block_view,
            candidate_indices,
            qc_mode_tag: domain.qc_mode_tag.clone(),
        };
        let subject_hash =
            Hash::new(norito::to_bytes(&preimage).map_err(|_| LaneBlockSubjectError::Encode)?);
        subjects.push(LaneBlockSubject {
            lane_id: domain.lane_id,
            dataspace_id: domain.dataspace_id,
            lane_block_height: slot.lane_block_height,
            lane_block_view: slot.lane_block_view,
            accepted_candidate_indices: domain.accepted_candidate_indices.clone(),
            qc_mode_tag: domain.qc_mode_tag.clone(),
            subject_hash,
        });
    }

    if let Some(slot) = slots
        .iter()
        .find(|slot| !seen_lanes.contains(&slot.lane_id))
    {
        return Err(LaneBlockSubjectError::UnexpectedLaneSlot {
            lane_id: slot.lane_id,
        });
    }

    subjects.sort_by_key(|subject| {
        (
            subject.lane_id,
            subject.dataspace_id,
            subject.lane_block_height,
            subject.lane_block_view,
        )
    });
    Ok(subjects)
}

/// Derive deterministic DA/RBC ownership identities from lane block subjects.
///
/// The planner validates each subject against its canonical subject digest
/// before deriving payload ownership and RBC instance digests. The current
/// proposal path uses this as preflight metadata while it still broadcasts a
/// global block payload; future lane-local DA/RBC sessions can use these
/// digests as stable, hardware-independent instance names.
pub(super) fn plan_lane_payload_ownership(
    subjects: &[LaneBlockSubject],
) -> Result<Vec<LanePayloadOwnership>, LanePayloadOwnershipError> {
    let mut seen_slots = BTreeSet::new();
    let mut seen_payload_ownership_hashes = BTreeSet::new();
    let mut seen_rbc_instance_hashes = BTreeSet::new();
    let mut ownerships = Vec::with_capacity(subjects.len());

    for subject in subjects {
        if subject.qc_mode_tag.trim().is_empty() {
            return Err(LanePayloadOwnershipError::BlankQcModeTag {
                lane_id: subject.lane_id,
            });
        }
        if subject.accepted_candidate_indices.is_empty() {
            return Err(LanePayloadOwnershipError::EmptyCandidateSet {
                lane_id: subject.lane_id,
            });
        }

        let mut seen_indices = BTreeSet::new();
        let mut candidate_indices = Vec::with_capacity(subject.accepted_candidate_indices.len());
        for index in subject.accepted_candidate_indices.iter().copied() {
            if !seen_indices.insert(index) {
                return Err(LanePayloadOwnershipError::DuplicateCandidateIndex {
                    lane_id: subject.lane_id,
                    index,
                });
            }
            candidate_indices.push(u64::try_from(index).map_err(|_| {
                LanePayloadOwnershipError::CandidateIndexOverflow {
                    lane_id: subject.lane_id,
                    index,
                }
            })?);
        }

        let expected_subject_hash = Hash::new(
            norito::to_bytes(&LaneBlockSubjectPreimage {
                version: 1,
                lane_id: subject.lane_id,
                dataspace_id: subject.dataspace_id,
                lane_block_height: subject.lane_block_height,
                lane_block_view: subject.lane_block_view,
                candidate_indices: candidate_indices.clone(),
                qc_mode_tag: subject.qc_mode_tag.clone(),
            })
            .map_err(|_| LanePayloadOwnershipError::Encode)?,
        );
        if expected_subject_hash != subject.subject_hash {
            return Err(LanePayloadOwnershipError::SubjectHashMismatch {
                lane_id: subject.lane_id,
                expected: expected_subject_hash,
                actual: subject.subject_hash,
            });
        }

        let slot = (
            subject.lane_id,
            subject.dataspace_id,
            subject.lane_block_height,
            subject.lane_block_view,
        );
        if !seen_slots.insert(slot) {
            return Err(LanePayloadOwnershipError::DuplicateLaneSlot {
                lane_id: subject.lane_id,
                dataspace_id: subject.dataspace_id,
                lane_block_height: subject.lane_block_height,
                lane_block_view: subject.lane_block_view,
            });
        }

        let payload_ownership_hash = Hash::new(
            norito::to_bytes(&LanePayloadOwnershipPreimage {
                purpose: "nexus:lane-payload-ownership:v1".to_string(),
                version: 1,
                lane_id: subject.lane_id,
                dataspace_id: subject.dataspace_id,
                lane_block_height: subject.lane_block_height,
                lane_block_view: subject.lane_block_view,
                subject_hash: subject.subject_hash,
                candidate_indices,
                qc_mode_tag: subject.qc_mode_tag.clone(),
            })
            .map_err(|_| LanePayloadOwnershipError::Encode)?,
        );
        if !seen_payload_ownership_hashes.insert(payload_ownership_hash) {
            return Err(LanePayloadOwnershipError::DuplicatePayloadOwnershipHash {
                payload_ownership_hash,
            });
        }

        let rbc_instance_hash = Hash::new(
            norito::to_bytes(&LaneRbcInstancePreimage {
                purpose: "nexus:lane-rbc-instance:v1".to_string(),
                version: 1,
                lane_id: subject.lane_id,
                dataspace_id: subject.dataspace_id,
                lane_block_height: subject.lane_block_height,
                lane_block_view: subject.lane_block_view,
                subject_hash: subject.subject_hash,
                payload_ownership_hash,
            })
            .map_err(|_| LanePayloadOwnershipError::Encode)?,
        );
        if !seen_rbc_instance_hashes.insert(rbc_instance_hash) {
            return Err(LanePayloadOwnershipError::DuplicateRbcInstanceHash { rbc_instance_hash });
        }

        ownerships.push(LanePayloadOwnership {
            lane_id: subject.lane_id,
            dataspace_id: subject.dataspace_id,
            lane_block_height: subject.lane_block_height,
            lane_block_view: subject.lane_block_view,
            subject_hash: subject.subject_hash,
            qc_mode_tag: subject.qc_mode_tag.clone(),
            accepted_candidate_indices: subject.accepted_candidate_indices.clone(),
            payload_ownership_hash,
            rbc_instance_hash,
        });
    }

    ownerships.sort_by_key(|ownership| {
        (
            ownership.lane_id,
            ownership.dataspace_id,
            ownership.lane_block_height,
            ownership.lane_block_view,
        )
    });
    Ok(ownerships)
}

/// Derive the full lane-local payload plan for accepted consensus domains.
///
/// This is the common planning boundary for the current global proposal path
/// and the future standalone lane proposal scheduler: known committed tips are
/// reduced, reset watermarks are applied, next lane-local slots are assigned,
/// canonical lane block subjects are derived, and DA/RBC ownership identities
/// are validated before being returned together.
pub(super) fn plan_lane_payload(
    domains: &[LaneConsensusDomain],
    known_tips: &[LaneBlockTip],
    compatibility_latest_lane_block_height: u64,
    reset_heights: &BTreeMap<LaneId, u64>,
    lane_block_view: u64,
) -> Result<LanePayloadPlan, LanePayloadPlanError> {
    let lane_tips = plan_latest_lane_block_tips_with_reset_heights(
        domains,
        known_tips,
        compatibility_latest_lane_block_height,
        reset_heights,
    )
    .map_err(LanePayloadPlanError::Tips)?;
    let slots = plan_next_lane_block_slots(domains, &lane_tips, lane_block_view)
        .map_err(LanePayloadPlanError::Slots)?;
    let subjects = plan_lane_block_subjects_for_slots(domains, &slots)
        .map_err(LanePayloadPlanError::Subjects)?;
    let ownerships =
        plan_lane_payload_ownership(&subjects).map_err(LanePayloadPlanError::Ownerships)?;

    Ok(LanePayloadPlan {
        lane_tips,
        slots,
        subjects,
        ownerships,
    })
}

fn accepted_work_by_lane(
    routing_decisions: &[RoutingDecision],
    schedule: &ProposalBatchSchedule,
) -> Result<BTreeMap<LaneId, LaneAcceptedWork>, LaneConsensusDomainError> {
    let mut accepted_work: BTreeMap<LaneId, LaneAcceptedWork> = BTreeMap::new();
    for action in &schedule.actions {
        let ProposalBatchAction::Accept { index, .. } = *action else {
            continue;
        };
        let routing = *routing_decisions.get(index).ok_or(
            LaneConsensusDomainError::ActionIndexOutOfBounds {
                index,
                routing_decisions: routing_decisions.len(),
            },
        )?;
        match accepted_work.entry(routing.lane_id) {
            Entry::Occupied(mut entry) => {
                let work = entry.get_mut();
                if work.dataspace_id != routing.dataspace_id {
                    return Err(LaneConsensusDomainError::AcceptedLaneDataspaceMismatch {
                        lane_id: routing.lane_id,
                        expected: work.dataspace_id,
                        actual: routing.dataspace_id,
                    });
                }
                work.candidate_indices.push(index);
            }
            Entry::Vacant(entry) => {
                entry.insert(LaneAcceptedWork {
                    dataspace_id: routing.dataspace_id,
                    candidate_indices: vec![index],
                });
            }
        }
    }
    Ok(accepted_work)
}

fn committees_by_lane(
    committees: &[LaneConsensusCommittee],
) -> Result<BTreeMap<LaneId, &LaneConsensusCommittee>, LaneConsensusDomainError> {
    let mut by_lane = BTreeMap::new();
    for committee in committees {
        if by_lane.insert(committee.lane_id, committee).is_some() {
            return Err(LaneConsensusDomainError::DuplicateLaneCommittee {
                lane_id: committee.lane_id,
            });
        }
    }
    Ok(by_lane)
}

fn canonical_validator_set(
    lane_id: LaneId,
    validators: &[PeerId],
) -> Result<Vec<PeerId>, LaneConsensusDomainError> {
    if validators.is_empty() {
        return Err(LaneConsensusDomainError::EmptyValidatorSet { lane_id });
    }
    let mut validator_set = validators.to_vec();
    validator_set.sort();
    for pair in validator_set.windows(2) {
        if pair[0] == pair[1] {
            return Err(LaneConsensusDomainError::DuplicateValidator { lane_id });
        }
    }
    Ok(validator_set)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LaneProposalWork {
    lane_id: LaneId,
    indices: VecDeque<usize>,
}

/// Batch-local lane scheduler for proposal assembly.
///
/// The scheduler only includes lanes that have fetched transaction work in the
/// current batch. Configured lanes with no queued work are intentionally absent,
/// which keeps idle lanes from blocking active lanes as Nexus moves toward fully
/// independent lane proposal/vote scheduling.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct LaneProposalBatch {
    lanes: Vec<LaneProposalWork>,
    total: usize,
}

impl LaneProposalBatch {
    /// Build a scheduler batch from fetched routing decisions.
    #[must_use]
    pub(super) fn from_routing_decisions(routing_decisions: &[RoutingDecision]) -> Self {
        let mut per_lane: BTreeMap<LaneId, VecDeque<usize>> = BTreeMap::new();
        for (idx, decision) in routing_decisions.iter().enumerate() {
            per_lane.entry(decision.lane_id).or_default().push_back(idx);
        }
        let lanes = per_lane
            .into_iter()
            .map(|(lane_id, indices)| LaneProposalWork { lane_id, indices })
            .collect();
        Self {
            lanes,
            total: routing_decisions.len(),
        }
    }

    /// Number of lanes with fetched work in this proposal batch.
    #[must_use]
    pub(super) fn active_lane_count(&self) -> usize {
        self.lanes.len()
    }

    /// Return true when this batch contains work for more than one lane.
    #[must_use]
    pub(super) fn has_parallel_work(&self) -> bool {
        self.active_lane_count() > 1
    }

    /// Return a deterministic interleaving for the slot identified by height/view.
    #[must_use]
    pub(super) fn interleaved_indices_for_slot(&self, height: u64, view: u64) -> Vec<usize> {
        if !self.has_parallel_work() {
            return self.interleaved_indices_from_offset(0);
        }
        let start_offset = u64::try_from(self.active_lane_count())
            .ok()
            .and_then(|lane_count| {
                usize::try_from(height.wrapping_add(view) % lane_count.max(1)).ok()
            })
            .unwrap_or_default();
        self.interleaved_indices_from_offset(start_offset)
    }

    /// Return a deterministic interleaving starting at the provided lane offset.
    #[must_use]
    pub(super) fn interleaved_indices_from_offset(&self, start_offset: usize) -> Vec<usize> {
        if self.total <= 1 {
            return (0..self.total).collect();
        }
        if self.lanes.is_empty() {
            return (0..self.total).collect();
        }

        let mut lanes = self.lanes.clone();
        let lane_count = lanes.len();
        let start_offset = start_offset % lane_count;
        let mut order = Vec::with_capacity(self.total);
        while order.len() < self.total {
            let mut progress = false;
            for offset in 0..lane_count {
                let lane_idx = (start_offset + offset) % lane_count;
                if let Some(idx) = lanes[lane_idx].indices.pop_front() {
                    order.push(idx);
                    progress = true;
                    if order.len() == self.total {
                        break;
                    }
                }
            }
            if !progress {
                break;
            }
        }

        if order.len() == self.total {
            order
        } else {
            (0..self.total).collect()
        }
    }

    #[cfg(test)]
    fn active_lane_ids(&self) -> Vec<LaneId> {
        self.lanes.iter().map(|lane| lane.lane_id).collect()
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, num::NonZeroU32};

    use iroha_config::parameters::actual::{
        LaneConfig as ActualLaneConfig, LaneRoutingMatcher, LaneRoutingPolicy, LaneRoutingRule,
    };
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::nexus::{
        AUTOSCALE_META_CREATED_HEIGHT, AUTOSCALE_META_MANAGED, DataSpaceCatalog, DataSpaceId,
        LaneCatalog, LaneConfig, LaneId,
    };

    use super::*;

    fn routing_for_lanes(lanes: &[u32]) -> Vec<RoutingDecision> {
        lanes
            .iter()
            .enumerate()
            .map(|(idx, lane)| {
                RoutingDecision::new(
                    LaneId::new(*lane),
                    DataSpaceId::new(u64::try_from(idx + 1).expect("dataspace id fits")),
                )
            })
            .collect()
    }

    fn routing_for_lane_dataspaces(routes: &[(u32, u64)]) -> Vec<RoutingDecision> {
        routes
            .iter()
            .map(|(lane, dataspace)| {
                RoutingDecision::new(LaneId::new(*lane), DataSpaceId::new(*dataspace))
            })
            .collect()
    }

    fn lane_catalog_from_configs(lanes: Vec<LaneConfig>) -> LaneCatalog {
        let max_lane = lanes.iter().map(|lane| lane.id.as_u32()).max().unwrap_or(0);
        let lane_count = NonZeroU32::new(max_lane.saturating_add(1))
            .expect("lane catalog requires nonzero lane count");
        LaneCatalog::new(lane_count, lanes).expect("valid lane catalog")
    }

    fn nexus_with_routing(routing_policy: LaneRoutingPolicy, lane_catalog: LaneCatalog) -> Nexus {
        let lane_config = ActualLaneConfig::from_catalog(&lane_catalog);
        Nexus {
            enabled: true,
            routing_policy,
            lane_catalog,
            lane_config,
            dataspace_catalog: DataSpaceCatalog::default(),
            ..Nexus::default()
        }
    }

    fn default_routing_policy() -> LaneRoutingPolicy {
        LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: Vec::new(),
        }
    }

    fn default_lane_config() -> LaneConfig {
        LaneConfig::default()
    }

    fn sidecar_lane_config(lane_id: LaneId) -> LaneConfig {
        LaneConfig {
            id: lane_id,
            dataspace_id: DataSpaceId::UNIVERSAL,
            alias: format!("sidecar-{}", lane_id.as_u32()),
            ..LaneConfig::default()
        }
    }

    fn autoscale_elastic_lane_config(lane_id: LaneId, created_height: u64) -> LaneConfig {
        let mut metadata = BTreeMap::new();
        metadata.insert(AUTOSCALE_META_MANAGED.to_string(), "true".to_string());
        metadata.insert(
            AUTOSCALE_META_CREATED_HEIGHT.to_string(),
            created_height.to_string(),
        );
        LaneConfig {
            id: lane_id,
            dataspace_id: DataSpaceId::UNIVERSAL,
            alias: format!("elastic-lane-{}", lane_id.as_u32()),
            metadata,
            ..LaneConfig::default()
        }
    }

    fn proposal_candidate(gas_cost: u64, is_ivm_heavy: bool) -> ProposalAdmissionCandidate {
        ProposalAdmissionCandidate {
            gas_cost,
            is_ivm_heavy,
        }
    }

    fn proposal_context() -> ProposalAdmissionContext {
        ProposalAdmissionContext {
            accepted_before_batch: 0,
            accepted_in_batch: 0,
            max_in_block: 4,
            gas_limit_per_block: Some(10),
            gas_used_in_block: 0,
            max_ivm_transactions: Some(1),
            ivm_transactions_included: 0,
        }
    }

    fn accepted_schedule(indices: &[usize]) -> ProposalBatchSchedule {
        ProposalBatchSchedule {
            actions: indices
                .iter()
                .copied()
                .map(|index| ProposalBatchAction::Accept {
                    index,
                    exceeds_gas_limit: false,
                })
                .collect(),
            ..ProposalBatchSchedule::default()
        }
    }

    fn mixed_schedule(actions: Vec<ProposalBatchAction>) -> ProposalBatchSchedule {
        ProposalBatchSchedule {
            actions,
            ..ProposalBatchSchedule::default()
        }
    }

    fn test_peer(seed: u8) -> PeerId {
        let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("deterministic peer key");
        PeerId::new(key_pair.public_key().clone())
    }

    fn committee(
        lane: u32,
        dataspace: u64,
        validators: Vec<PeerId>,
        min_quorum: Option<u32>,
    ) -> LaneConsensusCommittee {
        LaneConsensusCommittee {
            lane_id: LaneId::new(lane),
            dataspace_id: DataSpaceId::new(dataspace),
            validators,
            min_quorum,
        }
    }

    #[test]
    fn lane_proposal_batch_tracks_only_lanes_with_work() {
        let batch = LaneProposalBatch::from_routing_decisions(&routing_for_lanes(&[3, 1, 3]));

        assert_eq!(
            batch.active_lane_ids(),
            vec![LaneId::new(1), LaneId::new(3)]
        );
        assert_eq!(batch.active_lane_count(), 2);
        assert!(batch.has_parallel_work());
    }

    #[test]
    fn lane_proposal_batch_rotates_start_lane_by_height_and_view() {
        let batch = LaneProposalBatch::from_routing_decisions(&routing_for_lanes(&[1, 2, 1, 2]));

        assert_eq!(batch.interleaved_indices_for_slot(0, 0), vec![0, 1, 2, 3]);
        assert_eq!(batch.interleaved_indices_for_slot(1, 0), vec![1, 0, 3, 2]);
        assert_eq!(batch.interleaved_indices_for_slot(1, 1), vec![0, 1, 2, 3]);
    }

    #[test]
    fn lane_proposal_batch_does_not_wait_for_idle_lane_ids() {
        let batch = LaneProposalBatch::from_routing_decisions(&routing_for_lanes(&[1, 3, 1]));

        assert_eq!(
            batch.active_lane_ids(),
            vec![LaneId::new(1), LaneId::new(3)]
        );
        assert_eq!(batch.interleaved_indices_from_offset(0), vec![0, 1, 2]);
        assert_eq!(batch.interleaved_indices_from_offset(1), vec![1, 0, 2]);
    }

    #[test]
    fn lane_proposal_batch_preserves_serial_order_for_empty_or_single_lane_work() {
        let empty = LaneProposalBatch::from_routing_decisions(&[]);
        assert_eq!(empty.active_lane_count(), 0);
        assert_eq!(
            empty.interleaved_indices_for_slot(7, 9),
            Vec::<usize>::new()
        );

        let single_lane = LaneProposalBatch::from_routing_decisions(&routing_for_lanes(&[7, 7]));
        assert_eq!(single_lane.active_lane_count(), 1);
        assert!(!single_lane.has_parallel_work());
        assert_eq!(single_lane.interleaved_indices_for_slot(7, 9), vec![0, 1]);
    }

    #[test]
    fn proposal_lookahead_ignores_unrouted_sidecar_lane() {
        let lane_catalog = lane_catalog_from_configs(vec![
            default_lane_config(),
            sidecar_lane_config(LaneId::new(1)),
        ]);
        let nexus = nexus_with_routing(default_routing_policy(), lane_catalog);

        assert!(!proposal_lookahead_enabled(&nexus, 1));
    }

    #[test]
    fn proposal_lookahead_enables_for_explicit_rule_lane() {
        let routing_policy = LaneRoutingPolicy {
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(1),
                dataspace: Some(DataSpaceId::UNIVERSAL),
                matcher: LaneRoutingMatcher {
                    account: Some("alice".to_string()),
                    instruction: None,
                    description: None,
                },
            }],
            ..default_routing_policy()
        };
        let lane_catalog = lane_catalog_from_configs(vec![
            default_lane_config(),
            sidecar_lane_config(LaneId::new(1)),
        ]);
        let nexus = nexus_with_routing(routing_policy, lane_catalog);

        assert!(proposal_lookahead_enabled(&nexus, 1));
    }

    #[test]
    fn proposal_lookahead_respects_autoscale_creation_height() {
        let lane_catalog = lane_catalog_from_configs(vec![
            default_lane_config(),
            autoscale_elastic_lane_config(LaneId::new(1), 7),
        ]);
        let mut nexus = nexus_with_routing(default_routing_policy(), lane_catalog);
        nexus.autoscale.enabled = true;
        nexus.autoscale.min_lanes = NonZeroU32::new(1).expect("nonzero min");
        nexus.autoscale.max_lanes = NonZeroU32::new(4).expect("nonzero max");

        assert!(!proposal_lookahead_enabled(&nexus, 6));
        assert!(proposal_lookahead_enabled(&nexus, 7));
    }

    #[test]
    fn proposal_lookahead_fails_closed_when_nexus_is_disabled() {
        let routing_policy = LaneRoutingPolicy {
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(1),
                dataspace: Some(DataSpaceId::UNIVERSAL),
                matcher: LaneRoutingMatcher {
                    account: Some("alice".to_string()),
                    instruction: None,
                    description: None,
                },
            }],
            ..default_routing_policy()
        };
        let lane_catalog = lane_catalog_from_configs(vec![
            default_lane_config(),
            sidecar_lane_config(LaneId::new(1)),
        ]);
        let mut nexus = nexus_with_routing(routing_policy, lane_catalog);
        nexus.enabled = false;

        assert!(!proposal_lookahead_enabled(&nexus, 1));
    }

    #[test]
    fn proposal_fetch_cap_widens_only_for_schedulable_multilane_routes() {
        let single_lane = nexus_with_routing(
            default_routing_policy(),
            lane_catalog_from_configs(vec![
                default_lane_config(),
                sidecar_lane_config(LaneId::new(1)),
            ]),
        );
        assert_eq!(
            proposal_fetch_cap(&single_lane, 1, 8, 2),
            2,
            "unrouted sidecars must not widen queue scans"
        );

        let explicit_lane_policy = LaneRoutingPolicy {
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(1),
                dataspace: Some(DataSpaceId::UNIVERSAL),
                matcher: LaneRoutingMatcher {
                    account: Some("alice".to_string()),
                    instruction: None,
                    description: None,
                },
            }],
            ..default_routing_policy()
        };
        let explicit_lane = nexus_with_routing(
            explicit_lane_policy,
            lane_catalog_from_configs(vec![
                default_lane_config(),
                sidecar_lane_config(LaneId::new(1)),
            ]),
        );
        assert_eq!(
            proposal_fetch_cap(&explicit_lane, 1, 8, 2),
            8,
            "reachable sidecar lanes may widen scans to find lane-local work"
        );
    }

    #[test]
    fn proposal_fetch_cap_respects_budget_slot_and_autoscale_activation_bounds() {
        let lane_catalog = lane_catalog_from_configs(vec![
            default_lane_config(),
            autoscale_elastic_lane_config(LaneId::new(1), 7),
        ]);
        let mut nexus = nexus_with_routing(default_routing_policy(), lane_catalog);
        nexus.autoscale.enabled = true;
        nexus.autoscale.min_lanes = NonZeroU32::new(1).expect("nonzero min");
        nexus.autoscale.max_lanes = NonZeroU32::new(4).expect("nonzero max");

        assert_eq!(
            proposal_fetch_cap(&nexus, 6, 8, 2),
            2,
            "future-created autoscale lanes must not widen scans"
        );
        assert_eq!(
            proposal_fetch_cap(&nexus, 7, 8, 2),
            8,
            "active autoscale lanes may widen scans"
        );
        assert_eq!(proposal_fetch_cap(&nexus, 7, 0, 2), 0);
        assert_eq!(proposal_fetch_cap(&nexus, 7, 8, 0), 0);
    }

    #[test]
    fn proposal_admission_defers_when_block_slots_are_full() {
        let mut context = proposal_context();
        context.accepted_before_batch = 3;
        context.accepted_in_batch = 1;
        context.max_in_block = 4;

        assert_eq!(
            decide_proposal_candidate_admission(
                proposal_candidate(1, false),
                std::iter::empty::<ProposalAdmissionCandidate>(),
                context
            ),
            ProposalAdmissionDecision::Defer {
                reason: ProposalDeferralReason::BlockFull
            }
        );
    }

    #[test]
    fn proposal_admission_enforces_ivm_cap_without_blocking_non_ivm_work() {
        let mut context = proposal_context();
        context.ivm_transactions_included = 1;

        assert_eq!(
            decide_proposal_candidate_admission(
                proposal_candidate(1, true),
                std::iter::empty::<ProposalAdmissionCandidate>(),
                context
            ),
            ProposalAdmissionDecision::Defer {
                reason: ProposalDeferralReason::IvmLimit
            }
        );
        assert_eq!(
            decide_proposal_candidate_admission(
                proposal_candidate(1, false),
                std::iter::empty::<ProposalAdmissionCandidate>(),
                context
            ),
            ProposalAdmissionDecision::Accept {
                exceeds_gas_limit: false
            }
        );
    }

    #[test]
    fn proposal_admission_defers_gas_overflow_after_first_acceptance() {
        let mut context = proposal_context();
        context.gas_used_in_block = 7;
        context.accepted_before_batch = 1;

        assert_eq!(
            decide_proposal_candidate_admission(
                proposal_candidate(4, false),
                std::iter::empty::<ProposalAdmissionCandidate>(),
                context
            ),
            ProposalAdmissionDecision::Defer {
                reason: ProposalDeferralReason::GasLimit
            }
        );
    }

    #[test]
    fn proposal_admission_defers_oversized_first_when_later_candidate_fits() {
        let context = proposal_context();

        assert_eq!(
            decide_proposal_candidate_admission(
                proposal_candidate(11, false),
                [proposal_candidate(3, false)],
                context
            ),
            ProposalAdmissionDecision::Defer {
                reason: ProposalDeferralReason::GasLimit
            }
        );
    }

    #[test]
    fn proposal_admission_accepts_oversized_first_when_no_later_candidate_fits() {
        let context = proposal_context();

        assert_eq!(
            decide_proposal_candidate_admission(
                proposal_candidate(11, false),
                [proposal_candidate(12, false), proposal_candidate(11, true)],
                context
            ),
            ProposalAdmissionDecision::Accept {
                exceeds_gas_limit: true
            }
        );
    }

    #[test]
    fn proposal_admission_ignores_later_candidate_that_would_exceed_ivm_cap() {
        let mut context = proposal_context();
        context.ivm_transactions_included = 1;

        assert_eq!(
            decide_proposal_candidate_admission(
                proposal_candidate(11, false),
                [proposal_candidate(3, true)],
                context
            ),
            ProposalAdmissionDecision::Accept {
                exceeds_gas_limit: true
            }
        );
    }

    #[test]
    fn schedule_proposal_batch_interleaves_and_accumulates_resources() {
        let routing = routing_for_lanes(&[1, 2, 1]);
        let candidates = vec![
            proposal_candidate(2, false),
            proposal_candidate(3, true),
            proposal_candidate(5, false),
        ];
        let mut context = proposal_context();
        context.max_ivm_transactions = Some(2);

        let schedule = schedule_proposal_batch(&routing, &candidates, context, 0, 0)
            .expect("schedule proposal batch");

        assert_eq!(
            schedule.actions,
            vec![
                ProposalBatchAction::Accept {
                    index: 0,
                    exceeds_gas_limit: false
                },
                ProposalBatchAction::Accept {
                    index: 1,
                    exceeds_gas_limit: false
                },
                ProposalBatchAction::Accept {
                    index: 2,
                    exceeds_gas_limit: false
                },
            ]
        );
        assert_eq!(schedule.gas_used_delta, 10);
        assert_eq!(schedule.ivm_transactions_included_delta, 1);
        assert_eq!(schedule.ivm_transactions_deferred, 0);
    }

    #[test]
    fn schedule_proposal_batch_rotates_and_defers_after_block_full() {
        let routing = routing_for_lanes(&[1, 2, 1, 2]);
        let candidates = vec![
            proposal_candidate(1, false),
            proposal_candidate(1, false),
            proposal_candidate(1, false),
            proposal_candidate(1, false),
        ];
        let mut context = proposal_context();
        context.max_in_block = 2;

        let schedule = schedule_proposal_batch(&routing, &candidates, context, 1, 0)
            .expect("schedule proposal batch");

        assert_eq!(
            schedule.actions,
            vec![
                ProposalBatchAction::Accept {
                    index: 1,
                    exceeds_gas_limit: false
                },
                ProposalBatchAction::Accept {
                    index: 0,
                    exceeds_gas_limit: false
                },
                ProposalBatchAction::Defer {
                    index: 3,
                    reason: ProposalDeferralReason::BlockFull
                },
                ProposalBatchAction::Defer {
                    index: 2,
                    reason: ProposalDeferralReason::BlockFull
                },
            ]
        );
        assert_eq!(schedule.gas_used_delta, 2);
    }

    #[test]
    fn schedule_proposal_batch_prefers_later_fitting_candidate_over_oversized_first() {
        let routing = routing_for_lanes(&[1, 2]);
        let candidates = vec![proposal_candidate(11, false), proposal_candidate(3, false)];
        let context = proposal_context();

        let schedule = schedule_proposal_batch(&routing, &candidates, context, 0, 0)
            .expect("schedule proposal batch");

        assert_eq!(
            schedule.actions,
            vec![
                ProposalBatchAction::Defer {
                    index: 0,
                    reason: ProposalDeferralReason::GasLimit
                },
                ProposalBatchAction::Accept {
                    index: 1,
                    exceeds_gas_limit: false
                },
            ]
        );
        assert_eq!(schedule.gas_used_delta, 3);
    }

    #[test]
    fn schedule_proposal_batch_counts_ivm_deferrals() {
        let routing = routing_for_lanes(&[1, 2]);
        let candidates = vec![proposal_candidate(1, true), proposal_candidate(2, false)];
        let mut context = proposal_context();
        context.ivm_transactions_included = 1;

        let schedule = schedule_proposal_batch(&routing, &candidates, context, 0, 0)
            .expect("schedule proposal batch");

        assert_eq!(
            schedule.actions,
            vec![
                ProposalBatchAction::Defer {
                    index: 0,
                    reason: ProposalDeferralReason::IvmLimit
                },
                ProposalBatchAction::Accept {
                    index: 1,
                    exceeds_gas_limit: false
                },
            ]
        );
        assert_eq!(schedule.gas_used_delta, 2);
        assert_eq!(schedule.ivm_transactions_included_delta, 0);
        assert_eq!(schedule.ivm_transactions_deferred, 1);
    }

    #[test]
    fn defer_accepted_proposal_actions_preserves_existing_deferrals_without_resource_deltas() {
        let schedule = ProposalBatchSchedule {
            actions: vec![
                ProposalBatchAction::Accept {
                    index: 0,
                    exceeds_gas_limit: false,
                },
                ProposalBatchAction::Defer {
                    index: 1,
                    reason: ProposalDeferralReason::GasLimit,
                },
                ProposalBatchAction::Accept {
                    index: 2,
                    exceeds_gas_limit: true,
                },
            ],
            gas_used_delta: 13,
            ivm_transactions_included_delta: 1,
            ivm_transactions_deferred: 1,
        };

        let deferred =
            defer_accepted_proposal_actions(&schedule, ProposalDeferralReason::LaneConsensus);

        assert_eq!(
            deferred.actions,
            vec![
                ProposalBatchAction::Defer {
                    index: 0,
                    reason: ProposalDeferralReason::LaneConsensus,
                },
                ProposalBatchAction::Defer {
                    index: 1,
                    reason: ProposalDeferralReason::GasLimit,
                },
                ProposalBatchAction::Defer {
                    index: 2,
                    reason: ProposalDeferralReason::LaneConsensus,
                },
            ]
        );
        assert_eq!(deferred.gas_used_delta, 0);
        assert_eq!(deferred.ivm_transactions_included_delta, 0);
        assert_eq!(deferred.ivm_transactions_deferred, 1);
    }

    #[test]
    fn schedule_proposal_batch_rejects_mismatched_candidates_and_routes() {
        let routing = routing_for_lanes(&[1, 2]);
        let candidates = vec![proposal_candidate(1, false)];

        assert_eq!(
            schedule_proposal_batch(&routing, &candidates, proposal_context(), 0, 0),
            Err(ProposalBatchScheduleError::CandidateRoutingLengthMismatch {
                candidates: 1,
                routing_decisions: 2
            })
        );
    }

    #[test]
    fn lane_consensus_domains_include_only_accepted_work_and_canonicalize_validators() {
        let routing = routing_for_lane_dataspaces(&[(1, 11), (2, 22), (1, 11)]);
        let validators = vec![test_peer(3), test_peer(1), test_peer(4), test_peer(2)];
        let mut expected_validators = validators.clone();
        expected_validators.sort();
        let schedule = mixed_schedule(vec![
            ProposalBatchAction::Accept {
                index: 0,
                exceeds_gas_limit: false,
            },
            ProposalBatchAction::Defer {
                index: 1,
                reason: ProposalDeferralReason::GasLimit,
            },
            ProposalBatchAction::Accept {
                index: 2,
                exceeds_gas_limit: false,
            },
        ]);

        let domains = plan_lane_consensus_domains(
            &routing,
            &schedule,
            &[committee(1, 11, validators, None)],
            "permissioned",
        )
        .expect("lane consensus domains");

        assert_eq!(domains.len(), 1);
        let domain = &domains[0];
        assert_eq!(domain.lane_id, LaneId::new(1));
        assert_eq!(domain.dataspace_id, DataSpaceId::new(11));
        assert_eq!(domain.accepted_candidates, 2);
        assert_eq!(domain.accepted_candidate_indices, vec![0, 2]);
        assert_eq!(domain.validator_set, expected_validators);
        assert_eq!(domain.quorum.validator_count, 4);
        assert_eq!(domain.quorum.min_quorum, 3);
        assert_eq!(
            domain.qc_mode_tag,
            LaneRelayEnvelope::lane_qc_mode_tag_for(
                LaneId::new(1),
                DataSpaceId::new(11),
                "permissioned"
            )
        );
    }

    #[test]
    fn lane_consensus_domains_preserve_scheduler_candidate_order_per_lane() {
        let routing = routing_for_lane_dataspaces(&[(1, 11), (2, 22), (1, 11), (2, 22)]);
        let schedule = accepted_schedule(&[2, 1, 0, 3]);
        let validators = vec![test_peer(1), test_peer(2), test_peer(3)];

        let domains = plan_lane_consensus_domains(
            &routing,
            &schedule,
            &[
                committee(1, 11, validators.clone(), None),
                committee(2, 22, validators, None),
            ],
            "permissioned",
        )
        .expect("lane consensus domains");

        assert_eq!(domains.len(), 2);
        assert_eq!(domains[0].lane_id, LaneId::new(1));
        assert_eq!(domains[0].accepted_candidate_indices, vec![2, 0]);
        assert_eq!(domains[1].lane_id, LaneId::new(2));
        assert_eq!(domains[1].accepted_candidate_indices, vec![1, 3]);
    }

    #[test]
    fn lane_consensus_committees_use_authority_for_accepted_lanes_only() {
        let routing = routing_for_lane_dataspaces(&[(1, 11), (2, 22), (1, 11), (3, 33)]);
        let schedule = mixed_schedule(vec![
            ProposalBatchAction::Accept {
                index: 2,
                exceeds_gas_limit: false,
            },
            ProposalBatchAction::Defer {
                index: 1,
                reason: ProposalDeferralReason::GasLimit,
            },
            ProposalBatchAction::Accept {
                index: 0,
                exceeds_gas_limit: false,
            },
        ]);
        let lane1_authority = vec![test_peer(3), test_peer(1), test_peer(2)];
        let fallback = vec![test_peer(9), test_peer(10), test_peer(11)];
        let mut requested = Vec::new();

        let committees = plan_lane_consensus_committees_with_authority(
            &routing,
            &schedule,
            Some(&fallback),
            |lane_id, dataspace_id| {
                requested.push((lane_id, dataspace_id));
                if lane_id == LaneId::new(1) {
                    lane1_authority.clone()
                } else {
                    Vec::new()
                }
            },
        )
        .expect("lane consensus committees");

        assert_eq!(requested, vec![(LaneId::new(1), DataSpaceId::new(11))]);
        assert_eq!(committees.len(), 1);
        assert_eq!(committees[0].lane_id, LaneId::new(1));
        assert_eq!(committees[0].dataspace_id, DataSpaceId::new(11));
        assert_eq!(committees[0].validators, lane1_authority);
    }

    #[test]
    fn lane_consensus_committees_require_authority_without_fallback() {
        let routing = routing_for_lane_dataspaces(&[(1, 11), (2, 22)]);
        let schedule = accepted_schedule(&[0, 1]);

        assert_eq!(
            plan_lane_consensus_committees_with_authority(
                &routing,
                &schedule,
                None,
                |lane_id, _| {
                    if lane_id == LaneId::new(1) {
                        vec![test_peer(1), test_peer(2), test_peer(3)]
                    } else {
                        Vec::new()
                    }
                }
            ),
            Err(LaneConsensusDomainError::MissingLaneCommittee {
                lane_id: LaneId::new(2),
            })
        );
    }

    #[test]
    fn lane_consensus_committees_use_explicit_fallback_for_compatibility() {
        let routing = routing_for_lane_dataspaces(&[(1, 11), (2, 22)]);
        let schedule = accepted_schedule(&[0, 1]);
        let fallback = vec![test_peer(4), test_peer(5), test_peer(6)];

        let committees = plan_lane_consensus_committees_with_authority(
            &routing,
            &schedule,
            Some(&fallback),
            |_, _| Vec::new(),
        )
        .expect("fallback committees");

        assert_eq!(
            committees
                .iter()
                .map(|committee| (
                    committee.lane_id,
                    committee.dataspace_id,
                    committee.validators.clone()
                ))
                .collect::<Vec<_>>(),
            vec![
                (LaneId::new(1), DataSpaceId::new(11), fallback.clone()),
                (LaneId::new(2), DataSpaceId::new(22), fallback),
            ]
        );
    }

    #[test]
    fn lane_block_subjects_bind_coordinates_mode_tag_and_candidate_order() {
        let routing = routing_for_lane_dataspaces(&[(1, 11), (2, 22), (1, 11), (2, 22)]);
        let validators = vec![test_peer(1), test_peer(2), test_peer(3)];
        let domains = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[2, 1, 0, 3]),
            &[
                committee(1, 11, validators.clone(), None),
                committee(2, 22, validators, None),
            ],
            "permissioned",
        )
        .expect("lane consensus domains");

        let subjects = plan_lane_block_subjects(&domains, 42, 7).expect("lane block subjects");

        assert_eq!(subjects.len(), 2);
        assert_eq!(subjects[0].lane_id, LaneId::new(1));
        assert_eq!(subjects[0].dataspace_id, DataSpaceId::new(11));
        assert_eq!(subjects[0].lane_block_height, 42);
        assert_eq!(subjects[0].lane_block_view, 7);
        assert_eq!(subjects[0].accepted_candidate_indices, vec![2, 0]);
        assert_eq!(
            subjects[0].qc_mode_tag,
            LaneRelayEnvelope::lane_qc_mode_tag_for(
                LaneId::new(1),
                DataSpaceId::new(11),
                "permissioned"
            )
        );

        let view_drift =
            plan_lane_block_subjects(&domains, 42, 8).expect("lane block subjects with view drift");
        assert_ne!(subjects[0].subject_hash, view_drift[0].subject_hash);

        let mut reordered_work = domains.clone();
        reordered_work[0].accepted_candidate_indices.reverse();
        let reordered_subjects =
            plan_lane_block_subjects(&reordered_work, 42, 7).expect("reordered subjects");
        assert_ne!(subjects[0].subject_hash, reordered_subjects[0].subject_hash);

        let mut mode_drift = domains.clone();
        mode_drift[0].qc_mode_tag.push_str("::tampered");
        let mode_drift_subjects =
            plan_lane_block_subjects(&mode_drift, 42, 7).expect("mode drift subjects");
        assert_ne!(
            subjects[0].subject_hash,
            mode_drift_subjects[0].subject_hash
        );
    }

    #[test]
    fn lane_block_subjects_are_sorted_independent_of_domain_input_order() {
        let routing = routing_for_lane_dataspaces(&[(1, 11), (2, 22)]);
        let validators = vec![test_peer(1), test_peer(2), test_peer(3)];
        let domains = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[0, 1]),
            &[
                committee(1, 11, validators.clone(), None),
                committee(2, 22, validators, None),
            ],
            "permissioned",
        )
        .expect("lane consensus domains");
        let mut reversed_domains = domains.clone();
        reversed_domains.reverse();

        let subjects = plan_lane_block_subjects(&domains, 3, 4).expect("lane block subjects");
        let reversed_subjects =
            plan_lane_block_subjects(&reversed_domains, 3, 4).expect("reversed subjects");

        assert_eq!(
            subjects
                .iter()
                .map(|subject| subject.lane_id)
                .collect::<Vec<_>>(),
            vec![LaneId::new(1), LaneId::new(2)]
        );
        assert_eq!(
            subjects
                .iter()
                .map(|subject| subject.subject_hash)
                .collect::<Vec<_>>(),
            reversed_subjects
                .iter()
                .map(|subject| subject.subject_hash)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn lane_block_subjects_for_slots_bind_independent_lane_heights_and_views() {
        let routing = routing_for_lane_dataspaces(&[(1, 11), (2, 22), (1, 11), (2, 22)]);
        let validators = vec![test_peer(1), test_peer(2), test_peer(3)];
        let domains = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[2, 1, 0, 3]),
            &[
                committee(1, 11, validators.clone(), None),
                committee(2, 22, validators, None),
            ],
            "permissioned",
        )
        .expect("lane consensus domains");
        let slots = vec![
            LaneBlockSlot {
                lane_id: LaneId::new(2),
                dataspace_id: DataSpaceId::new(22),
                lane_block_height: 4,
                lane_block_view: 8,
            },
            LaneBlockSlot {
                lane_id: LaneId::new(1),
                dataspace_id: DataSpaceId::new(11),
                lane_block_height: 10,
                lane_block_view: 1,
            },
        ];

        let subjects =
            plan_lane_block_subjects_for_slots(&domains, &slots).expect("slotted subjects");

        assert_eq!(subjects.len(), 2);
        assert_eq!(subjects[0].lane_id, LaneId::new(1));
        assert_eq!(subjects[0].lane_block_height, 10);
        assert_eq!(subjects[0].lane_block_view, 1);
        assert_eq!(subjects[1].lane_id, LaneId::new(2));
        assert_eq!(subjects[1].lane_block_height, 4);
        assert_eq!(subjects[1].lane_block_view, 8);

        let global_subjects =
            plan_lane_block_subjects(&domains, 10, 1).expect("global compatibility subjects");
        assert_eq!(subjects[0].subject_hash, global_subjects[0].subject_hash);
        assert_ne!(subjects[1].subject_hash, global_subjects[1].subject_hash);
    }

    #[test]
    fn lane_block_slots_from_tips_advance_active_lanes_independently() {
        let routing = routing_for_lane_dataspaces(&[(1, 11), (2, 22), (1, 11), (2, 22)]);
        let validators = vec![test_peer(1), test_peer(2), test_peer(3)];
        let domains = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[2, 1, 0, 3]),
            &[
                committee(1, 11, validators.clone(), None),
                committee(2, 22, validators, None),
            ],
            "permissioned",
        )
        .expect("lane consensus domains");
        let lane_tips = vec![
            LaneBlockTip {
                lane_id: LaneId::new(2),
                dataspace_id: DataSpaceId::new(22),
                latest_lane_block_height: 3,
            },
            LaneBlockTip {
                lane_id: LaneId::new(7),
                dataspace_id: DataSpaceId::new(77),
                latest_lane_block_height: 31,
            },
            LaneBlockTip {
                lane_id: LaneId::new(1),
                dataspace_id: DataSpaceId::new(11),
                latest_lane_block_height: 9,
            },
        ];

        let slots =
            plan_next_lane_block_slots(&domains, &lane_tips, 5).expect("next lane block slots");

        assert_eq!(
            slots,
            vec![
                LaneBlockSlot {
                    lane_id: LaneId::new(1),
                    dataspace_id: DataSpaceId::new(11),
                    lane_block_height: 10,
                    lane_block_view: 5,
                },
                LaneBlockSlot {
                    lane_id: LaneId::new(2),
                    dataspace_id: DataSpaceId::new(22),
                    lane_block_height: 4,
                    lane_block_view: 5,
                },
            ],
            "slot planning must ignore idle lane tips and sort active slots deterministically"
        );

        let subjects =
            plan_lane_block_subjects_for_slots(&domains, &slots).expect("slotted subjects");
        assert_eq!(subjects[0].lane_block_height, 10);
        assert_eq!(subjects[1].lane_block_height, 4);
        assert_ne!(
            subjects[0].subject_hash, subjects[1].subject_hash,
            "independent lane heights should produce distinct subject identities"
        );

        let new_lane_domain = LaneConsensusDomain {
            lane_id: LaneId::new(8),
            dataspace_id: DataSpaceId::new(88),
            accepted_candidates: 1,
            accepted_candidate_indices: vec![0],
            validator_set: vec![test_peer(4), test_peer(5), test_peer(6)],
            quorum: LaneRelayQuorumContext::new(3, 2).expect("valid quorum"),
            qc_mode_tag: LaneRelayEnvelope::lane_qc_mode_tag_for(
                LaneId::new(8),
                DataSpaceId::new(88),
                "permissioned",
            ),
        };
        let new_lane_slots = plan_next_lane_block_slots(
            &[new_lane_domain],
            &[LaneBlockTip {
                lane_id: LaneId::new(8),
                dataspace_id: DataSpaceId::new(88),
                latest_lane_block_height: 0,
            }],
            0,
        )
        .expect("new lane first slot");
        assert_eq!(new_lane_slots[0].lane_block_height, 1);
    }

    #[test]
    fn lane_payload_plan_derives_tips_slots_subjects_and_ownerships() {
        let routing = routing_for_lane_dataspaces(&[(1, 11), (2, 22), (1, 11)]);
        let validators = vec![test_peer(1), test_peer(2), test_peer(3)];
        let domains = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[2, 1, 0]),
            &[
                committee(1, 11, validators.clone(), None),
                committee(2, 22, validators, None),
            ],
            "permissioned",
        )
        .expect("lane consensus domains");
        let known_tips = vec![
            LaneBlockTip {
                lane_id: LaneId::new(2),
                dataspace_id: DataSpaceId::new(22),
                latest_lane_block_height: 0,
            },
            LaneBlockTip {
                lane_id: LaneId::new(1),
                dataspace_id: DataSpaceId::new(11),
                latest_lane_block_height: 7,
            },
        ];

        let plan =
            plan_lane_payload(&domains, &known_tips, 99, &BTreeMap::new(), 5).expect("lane plan");

        assert_eq!(
            plan.lane_tips,
            vec![
                LaneBlockTip {
                    lane_id: LaneId::new(1),
                    dataspace_id: DataSpaceId::new(11),
                    latest_lane_block_height: 7,
                },
                LaneBlockTip {
                    lane_id: LaneId::new(2),
                    dataspace_id: DataSpaceId::new(22),
                    latest_lane_block_height: 0,
                },
            ]
        );
        assert_eq!(
            plan.slots,
            vec![
                LaneBlockSlot {
                    lane_id: LaneId::new(1),
                    dataspace_id: DataSpaceId::new(11),
                    lane_block_height: 8,
                    lane_block_view: 5,
                },
                LaneBlockSlot {
                    lane_id: LaneId::new(2),
                    dataspace_id: DataSpaceId::new(22),
                    lane_block_height: 1,
                    lane_block_view: 5,
                },
            ]
        );
        assert_eq!(
            plan.subjects
                .iter()
                .map(|subject| (
                    subject.lane_id,
                    subject.lane_block_height,
                    subject.accepted_candidate_indices.clone()
                ))
                .collect::<Vec<_>>(),
            vec![
                (LaneId::new(1), 8, vec![2, 0]),
                (LaneId::new(2), 1, vec![1]),
            ]
        );
        assert_eq!(
            plan.ownerships
                .iter()
                .map(|ownership| (
                    ownership.lane_id,
                    ownership.lane_block_height,
                    ownership.accepted_candidate_indices.clone()
                ))
                .collect::<Vec<_>>(),
            vec![
                (LaneId::new(1), 8, vec![2, 0]),
                (LaneId::new(2), 1, vec![1]),
            ]
        );
        assert_eq!(
            plan.subjects[0].subject_hash,
            plan.ownerships[0].subject_hash
        );
        assert_ne!(
            plan.ownerships[0].payload_ownership_hash,
            plan.ownerships[0].rbc_instance_hash
        );
    }

    #[test]
    fn lane_payload_plan_wraps_tip_dataspace_mismatch() {
        let routing = routing_for_lane_dataspaces(&[(1, 11)]);
        let validators = vec![test_peer(1), test_peer(2), test_peer(3)];
        let domains = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[0]),
            &[committee(1, 11, validators, None)],
            "permissioned",
        )
        .expect("lane consensus domain");

        let err = plan_lane_payload(
            &domains,
            &[LaneBlockTip {
                lane_id: LaneId::new(1),
                dataspace_id: DataSpaceId::new(99),
                latest_lane_block_height: 4,
            }],
            3,
            &BTreeMap::new(),
            0,
        )
        .expect_err("foreign-dataspace tip must fail closed");

        assert_eq!(
            err,
            LanePayloadPlanError::Tips(LaneBlockTipPlanError::LaneTipDataspaceMismatch {
                lane_id: LaneId::new(1),
                expected: DataSpaceId::new(11),
                actual: DataSpaceId::new(99),
            })
        );
    }

    #[test]
    fn latest_lane_block_tips_use_latest_known_or_compatibility_tip() {
        let routing = routing_for_lane_dataspaces(&[(1, 11), (2, 22), (3, 33)]);
        let validators = vec![test_peer(1), test_peer(2), test_peer(3)];
        let domains = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[0, 1, 2]),
            &[
                committee(1, 11, validators.clone(), None),
                committee(2, 22, validators.clone(), None),
                committee(3, 33, validators, None),
            ],
            "permissioned",
        )
        .expect("lane consensus domains");
        let known_tips = vec![
            LaneBlockTip {
                lane_id: LaneId::new(1),
                dataspace_id: DataSpaceId::new(11),
                latest_lane_block_height: 4,
            },
            LaneBlockTip {
                lane_id: LaneId::new(7),
                dataspace_id: DataSpaceId::new(77),
                latest_lane_block_height: 99,
            },
            LaneBlockTip {
                lane_id: LaneId::new(1),
                dataspace_id: DataSpaceId::new(11),
                latest_lane_block_height: 8,
            },
            LaneBlockTip {
                lane_id: LaneId::new(3),
                dataspace_id: DataSpaceId::new(33),
                latest_lane_block_height: 0,
            },
        ];

        let tips = plan_latest_lane_block_tips_with_reset_heights(
            &domains,
            &known_tips,
            41,
            &BTreeMap::new(),
        )
        .expect("latest lane block tips");

        assert_eq!(
            tips,
            vec![
                LaneBlockTip {
                    lane_id: LaneId::new(1),
                    dataspace_id: DataSpaceId::new(11),
                    latest_lane_block_height: 8,
                },
                LaneBlockTip {
                    lane_id: LaneId::new(2),
                    dataspace_id: DataSpaceId::new(22),
                    latest_lane_block_height: 41,
                },
                LaneBlockTip {
                    lane_id: LaneId::new(3),
                    dataspace_id: DataSpaceId::new(33),
                    latest_lane_block_height: 0,
                },
            ],
            "tip reducer should keep the latest active-lane tip, ignore idle-lane tips, and fill compatibility tips"
        );
    }

    #[test]
    fn latest_lane_block_tips_floor_recreated_lanes_by_reset_watermark() {
        let routing = routing_for_lane_dataspaces(&[(1, 11), (2, 22), (3, 33)]);
        let validators = vec![test_peer(1), test_peer(2), test_peer(3)];
        let domains = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[0, 1, 2]),
            &[
                committee(1, 11, validators.clone(), None),
                committee(2, 22, validators.clone(), None),
                committee(3, 33, validators, None),
            ],
            "permissioned",
        )
        .expect("lane consensus domains");
        let known_tips = vec![
            LaneBlockTip {
                lane_id: LaneId::new(1),
                dataspace_id: DataSpaceId::new(11),
                latest_lane_block_height: 4,
            },
            LaneBlockTip {
                lane_id: LaneId::new(2),
                dataspace_id: DataSpaceId::new(99),
                latest_lane_block_height: 5,
            },
            LaneBlockTip {
                lane_id: LaneId::new(3),
                dataspace_id: DataSpaceId::new(33),
                latest_lane_block_height: 12,
            },
        ];
        let reset_heights = BTreeMap::from([
            (LaneId::new(1), 9),
            (LaneId::new(2), 6),
            (LaneId::new(3), 8),
        ]);

        let tips = plan_latest_lane_block_tips_with_reset_heights(
            &domains,
            &known_tips,
            3,
            &reset_heights,
        )
        .expect("reset-aware latest lane block tips");

        assert_eq!(
            tips,
            vec![
                LaneBlockTip {
                    lane_id: LaneId::new(1),
                    dataspace_id: DataSpaceId::new(11),
                    latest_lane_block_height: 9,
                },
                LaneBlockTip {
                    lane_id: LaneId::new(2),
                    dataspace_id: DataSpaceId::new(22),
                    latest_lane_block_height: 6,
                },
                LaneBlockTip {
                    lane_id: LaneId::new(3),
                    dataspace_id: DataSpaceId::new(33),
                    latest_lane_block_height: 12,
                },
            ],
            "reset watermarks floor stale same-dataspace tips, ignore stale old-incarnation mismatches, and preserve newer tips"
        );

        let slots = plan_next_lane_block_slots(&domains, &tips, 7)
            .expect("slots should advance from reset-aware tips");
        assert_eq!(
            slots
                .iter()
                .map(|slot| (slot.lane_id, slot.lane_block_height))
                .collect::<Vec<_>>(),
            vec![
                (LaneId::new(1), 10),
                (LaneId::new(2), 7),
                (LaneId::new(3), 13),
            ],
            "recreated lanes must resume after the reset watermark"
        );
    }

    #[test]
    fn latest_lane_block_tips_reject_future_dataspace_mismatch_after_reset() {
        let routing = routing_for_lane_dataspaces(&[(1, 11)]);
        let domains = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[0]),
            &[committee(
                1,
                11,
                vec![test_peer(1), test_peer(2), test_peer(3)],
                None,
            )],
            "permissioned",
        )
        .expect("lane consensus domains");
        let reset_heights = BTreeMap::from([(LaneId::new(1), 6)]);

        assert_eq!(
            plan_latest_lane_block_tips_with_reset_heights(
                &domains,
                &[LaneBlockTip {
                    lane_id: LaneId::new(1),
                    dataspace_id: DataSpaceId::new(99),
                    latest_lane_block_height: 7,
                }],
                3,
                &reset_heights,
            ),
            Err(LaneBlockTipPlanError::LaneTipDataspaceMismatch {
                lane_id: LaneId::new(1),
                expected: DataSpaceId::new(11),
                actual: DataSpaceId::new(99),
            }),
            "a mismatched tip above the reset watermark belongs to the current incarnation and must not be ignored"
        );
    }

    #[test]
    fn latest_lane_block_tips_reject_duplicate_domains_and_dataspace_drift() {
        let routing = routing_for_lane_dataspaces(&[(1, 11)]);
        let domains = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[0]),
            &[committee(
                1,
                11,
                vec![test_peer(1), test_peer(2), test_peer(3)],
                None,
            )],
            "permissioned",
        )
        .expect("lane consensus domains");

        assert_eq!(
            plan_latest_lane_block_tips_with_reset_heights(
                &[domains[0].clone(), domains[0].clone()],
                &[],
                9,
                &BTreeMap::new(),
            ),
            Err(LaneBlockTipPlanError::DuplicateLaneDomain {
                lane_id: LaneId::new(1),
                dataspace_id: DataSpaceId::new(11),
            })
        );

        assert_eq!(
            plan_latest_lane_block_tips_with_reset_heights(
                &domains,
                &[LaneBlockTip {
                    lane_id: LaneId::new(1),
                    dataspace_id: DataSpaceId::new(99),
                    latest_lane_block_height: 7,
                }],
                9,
                &BTreeMap::new(),
            ),
            Err(LaneBlockTipPlanError::LaneTipDataspaceMismatch {
                lane_id: LaneId::new(1),
                expected: DataSpaceId::new(11),
                actual: DataSpaceId::new(99),
            })
        );
    }

    #[test]
    fn lane_block_slots_from_tips_reject_malformed_inputs() {
        let routing = routing_for_lane_dataspaces(&[(1, 11)]);
        let domains = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[0]),
            &[committee(
                1,
                11,
                vec![test_peer(1), test_peer(2), test_peer(3)],
                None,
            )],
            "permissioned",
        )
        .expect("lane consensus domains");
        let tip = LaneBlockTip {
            lane_id: LaneId::new(1),
            dataspace_id: DataSpaceId::new(11),
            latest_lane_block_height: 7,
        };

        assert_eq!(
            plan_next_lane_block_slots(&domains, &[], 0),
            Err(LaneBlockSlotPlanError::MissingLaneTip {
                lane_id: LaneId::new(1),
            })
        );

        assert_eq!(
            plan_next_lane_block_slots(&domains, &[tip, tip], 0),
            Err(LaneBlockSlotPlanError::DuplicateLaneTip {
                lane_id: LaneId::new(1),
            })
        );

        let mismatched_dataspace = LaneBlockTip {
            dataspace_id: DataSpaceId::new(99),
            ..tip
        };
        assert_eq!(
            plan_next_lane_block_slots(&domains, &[mismatched_dataspace], 0),
            Err(LaneBlockSlotPlanError::LaneTipDataspaceMismatch {
                lane_id: LaneId::new(1),
                expected: DataSpaceId::new(11),
                actual: DataSpaceId::new(99),
            })
        );

        let overflow = LaneBlockTip {
            latest_lane_block_height: u64::MAX,
            ..tip
        };
        assert_eq!(
            plan_next_lane_block_slots(&domains, &[overflow], 0),
            Err(LaneBlockSlotPlanError::LaneBlockHeightOverflow {
                lane_id: LaneId::new(1),
                latest_lane_block_height: u64::MAX,
            })
        );

        assert_eq!(
            plan_next_lane_block_slots(&[domains[0].clone(), domains[0].clone()], &[tip], 0),
            Err(LaneBlockSlotPlanError::DuplicateLaneDomain {
                lane_id: LaneId::new(1),
                dataspace_id: DataSpaceId::new(11),
            })
        );
    }

    #[test]
    fn lane_block_subjects_for_slots_reject_malformed_slots() {
        let routing = routing_for_lane_dataspaces(&[(1, 11)]);
        let domains = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[0]),
            &[committee(
                1,
                11,
                vec![test_peer(1), test_peer(2), test_peer(3)],
                None,
            )],
            "permissioned",
        )
        .expect("lane consensus domains");
        let slot = LaneBlockSlot {
            lane_id: LaneId::new(1),
            dataspace_id: DataSpaceId::new(11),
            lane_block_height: 7,
            lane_block_view: 3,
        };

        assert_eq!(
            plan_lane_block_subjects_for_slots(&domains, &[]),
            Err(LaneBlockSubjectError::MissingLaneSlot {
                lane_id: LaneId::new(1),
            })
        );

        assert_eq!(
            plan_lane_block_subjects_for_slots(&domains, &[slot, slot]),
            Err(LaneBlockSubjectError::DuplicateLaneSlot {
                lane_id: LaneId::new(1),
            })
        );

        let mismatched_dataspace = LaneBlockSlot {
            dataspace_id: DataSpaceId::new(99),
            ..slot
        };
        assert_eq!(
            plan_lane_block_subjects_for_slots(&domains, &[mismatched_dataspace]),
            Err(LaneBlockSubjectError::LaneSlotDataspaceMismatch {
                lane_id: LaneId::new(1),
                expected: DataSpaceId::new(11),
                actual: DataSpaceId::new(99),
            })
        );

        let unexpected = LaneBlockSlot {
            lane_id: LaneId::new(2),
            dataspace_id: DataSpaceId::new(22),
            lane_block_height: 1,
            lane_block_view: 0,
        };
        assert_eq!(
            plan_lane_block_subjects_for_slots(&domains, &[slot, unexpected]),
            Err(LaneBlockSubjectError::UnexpectedLaneSlot {
                lane_id: LaneId::new(2),
            })
        );
    }

    #[test]
    fn lane_payload_ownership_binds_subject_hash_coordinates_and_candidate_order() {
        let routing = routing_for_lane_dataspaces(&[(1, 11), (2, 22), (1, 11), (2, 22)]);
        let validators = vec![test_peer(1), test_peer(2), test_peer(3)];
        let domains = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[2, 1, 0, 3]),
            &[
                committee(1, 11, validators.clone(), None),
                committee(2, 22, validators, None),
            ],
            "permissioned",
        )
        .expect("lane consensus domains");
        let subjects = plan_lane_block_subjects(&domains, 42, 7).expect("lane block subjects");

        let ownerships = plan_lane_payload_ownership(&subjects).expect("lane payload ownership");

        assert_eq!(ownerships.len(), 2);
        assert_eq!(ownerships[0].lane_id, LaneId::new(1));
        assert_eq!(ownerships[0].dataspace_id, DataSpaceId::new(11));
        assert_eq!(ownerships[0].lane_block_height, 42);
        assert_eq!(ownerships[0].lane_block_view, 7);
        assert_eq!(ownerships[0].subject_hash, subjects[0].subject_hash);
        assert_eq!(ownerships[0].accepted_candidate_indices, vec![2, 0]);
        assert_ne!(
            ownerships[0].payload_ownership_hash,
            ownerships[0].rbc_instance_hash
        );

        let view_drift_subjects =
            plan_lane_block_subjects(&domains, 42, 8).expect("lane block subjects with view drift");
        let view_drift_ownerships =
            plan_lane_payload_ownership(&view_drift_subjects).expect("view drift ownership");
        assert_ne!(
            ownerships[0].payload_ownership_hash,
            view_drift_ownerships[0].payload_ownership_hash
        );
        assert_ne!(
            ownerships[0].rbc_instance_hash,
            view_drift_ownerships[0].rbc_instance_hash
        );

        let mut reordered_work = domains.clone();
        reordered_work[0].accepted_candidate_indices.reverse();
        let reordered_subjects =
            plan_lane_block_subjects(&reordered_work, 42, 7).expect("reordered subjects");
        let reordered_ownerships =
            plan_lane_payload_ownership(&reordered_subjects).expect("reordered ownership");
        assert_ne!(
            ownerships[0].payload_ownership_hash,
            reordered_ownerships[0].payload_ownership_hash
        );
        assert_ne!(
            ownerships[0].rbc_instance_hash,
            reordered_ownerships[0].rbc_instance_hash
        );
    }

    #[test]
    fn lane_payload_ownership_is_sorted_independent_of_subject_input_order() {
        let routing = routing_for_lane_dataspaces(&[(1, 11), (2, 22)]);
        let validators = vec![test_peer(1), test_peer(2), test_peer(3)];
        let domains = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[0, 1]),
            &[
                committee(1, 11, validators.clone(), None),
                committee(2, 22, validators, None),
            ],
            "permissioned",
        )
        .expect("lane consensus domains");
        let subjects = plan_lane_block_subjects(&domains, 3, 4).expect("lane block subjects");
        let mut reversed_subjects = subjects.clone();
        reversed_subjects.reverse();

        let ownerships = plan_lane_payload_ownership(&subjects).expect("lane payload ownership");
        let reversed_ownerships =
            plan_lane_payload_ownership(&reversed_subjects).expect("reversed ownership");

        assert_eq!(
            ownerships
                .iter()
                .map(|ownership| ownership.lane_id)
                .collect::<Vec<_>>(),
            vec![LaneId::new(1), LaneId::new(2)]
        );
        assert_eq!(
            ownerships
                .iter()
                .map(|ownership| {
                    (
                        ownership.payload_ownership_hash,
                        ownership.rbc_instance_hash,
                    )
                })
                .collect::<Vec<_>>(),
            reversed_ownerships
                .iter()
                .map(|ownership| {
                    (
                        ownership.payload_ownership_hash,
                        ownership.rbc_instance_hash,
                    )
                })
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn lane_payload_ownership_rejects_malformed_subjects() {
        let routing = routing_for_lane_dataspaces(&[(1, 11)]);
        let domains = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[0]),
            &[committee(
                1,
                11,
                vec![test_peer(1), test_peer(2), test_peer(3)],
                None,
            )],
            "permissioned",
        )
        .expect("lane consensus domains");
        let subjects = plan_lane_block_subjects(&domains, 9, 2).expect("lane block subjects");
        let mut malformed = subjects[0].clone();

        malformed.qc_mode_tag = " ".to_string();
        assert_eq!(
            plan_lane_payload_ownership(&[malformed.clone()]),
            Err(LanePayloadOwnershipError::BlankQcModeTag {
                lane_id: LaneId::new(1),
            })
        );

        malformed = subjects[0].clone();
        malformed.accepted_candidate_indices.clear();
        assert_eq!(
            plan_lane_payload_ownership(&[malformed.clone()]),
            Err(LanePayloadOwnershipError::EmptyCandidateSet {
                lane_id: LaneId::new(1),
            })
        );

        malformed = subjects[0].clone();
        malformed.accepted_candidate_indices.push(0);
        assert_eq!(
            plan_lane_payload_ownership(&[malformed.clone()]),
            Err(LanePayloadOwnershipError::DuplicateCandidateIndex {
                lane_id: LaneId::new(1),
                index: 0,
            })
        );

        malformed = subjects[0].clone();
        malformed.subject_hash = Hash::new(b"tampered lane block subject");
        assert_eq!(
            plan_lane_payload_ownership(&[malformed.clone()]),
            Err(LanePayloadOwnershipError::SubjectHashMismatch {
                lane_id: LaneId::new(1),
                expected: subjects[0].subject_hash,
                actual: malformed.subject_hash,
            })
        );

        assert_eq!(
            plan_lane_payload_ownership(&[subjects[0].clone(), subjects[0].clone()]),
            Err(LanePayloadOwnershipError::DuplicateLaneSlot {
                lane_id: LaneId::new(1),
                dataspace_id: DataSpaceId::new(11),
                lane_block_height: 9,
                lane_block_view: 2,
            })
        );
    }

    #[test]
    fn lane_block_subjects_reject_malformed_domains() {
        let routing = routing_for_lane_dataspaces(&[(1, 11)]);
        let domains = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[0]),
            &[committee(
                1,
                11,
                vec![test_peer(1), test_peer(2), test_peer(3)],
                None,
            )],
            "permissioned",
        )
        .expect("lane consensus domains");
        let mut malformed = domains[0].clone();

        malformed.qc_mode_tag = " ".to_string();
        assert_eq!(
            plan_lane_block_subjects(&[malformed.clone()], 1, 0),
            Err(LaneBlockSubjectError::BlankQcModeTag {
                lane_id: LaneId::new(1),
            })
        );

        malformed = domains[0].clone();
        malformed.accepted_candidate_indices.clear();
        malformed.accepted_candidates = 0;
        assert_eq!(
            plan_lane_block_subjects(&[malformed.clone()], 1, 0),
            Err(LaneBlockSubjectError::EmptyCandidateSet {
                lane_id: LaneId::new(1),
            })
        );

        malformed = domains[0].clone();
        malformed.accepted_candidates = 2;
        assert_eq!(
            plan_lane_block_subjects(&[malformed.clone()], 1, 0),
            Err(LaneBlockSubjectError::CandidateCountMismatch {
                lane_id: LaneId::new(1),
                accepted_candidates: 2,
                candidate_indices: 1,
            })
        );

        malformed = domains[0].clone();
        malformed.accepted_candidate_indices.push(0);
        malformed.accepted_candidates = malformed.accepted_candidate_indices.len();
        assert_eq!(
            plan_lane_block_subjects(&[malformed.clone()], 1, 0),
            Err(LaneBlockSubjectError::DuplicateCandidateIndex {
                lane_id: LaneId::new(1),
                index: 0,
            })
        );

        assert_eq!(
            plan_lane_block_subjects(&[domains[0].clone(), domains[0].clone()], 1, 0),
            Err(LaneBlockSubjectError::DuplicateLaneDomain {
                lane_id: LaneId::new(1),
            })
        );
    }

    #[test]
    fn lane_consensus_domains_use_explicit_quorum() {
        let routing = routing_for_lane_dataspaces(&[(7, 70)]);
        let validators = vec![test_peer(1), test_peer(2), test_peer(3)];

        let domains = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[0]),
            &[committee(7, 70, validators, Some(2))],
            "npos",
        )
        .expect("lane consensus domains");

        assert_eq!(domains[0].quorum.validator_count, 3);
        assert_eq!(domains[0].quorum.min_quorum, 2);
        assert_eq!(
            domains[0].qc_mode_tag,
            "npos::lane-relay:v1:70:7".to_string()
        );
    }

    #[test]
    fn lane_consensus_domains_ignore_missing_committee_for_deferred_lane() {
        let routing = routing_for_lane_dataspaces(&[(1, 11), (2, 22)]);
        let schedule = mixed_schedule(vec![
            ProposalBatchAction::Accept {
                index: 0,
                exceeds_gas_limit: false,
            },
            ProposalBatchAction::Defer {
                index: 1,
                reason: ProposalDeferralReason::GasLimit,
            },
        ]);

        let domains = plan_lane_consensus_domains(
            &routing,
            &schedule,
            &[committee(1, 11, vec![test_peer(1)], None)],
            "permissioned",
        )
        .expect("lane consensus domains");

        assert_eq!(domains.len(), 1);
        assert_eq!(domains[0].lane_id, LaneId::new(1));
    }

    #[test]
    fn lane_consensus_domains_reject_blank_base_mode_tag() {
        assert_eq!(
            plan_lane_consensus_domains(
                &routing_for_lane_dataspaces(&[(1, 11)]),
                &accepted_schedule(&[0]),
                &[committee(1, 11, vec![test_peer(1)], None)],
                "  ",
            ),
            Err(LaneConsensusDomainError::BlankBaseModeTag)
        );
    }

    #[test]
    fn lane_consensus_domains_reject_action_index_out_of_bounds() {
        assert_eq!(
            plan_lane_consensus_domains(
                &routing_for_lane_dataspaces(&[(1, 11)]),
                &accepted_schedule(&[1]),
                &[committee(1, 11, vec![test_peer(1)], None)],
                "permissioned",
            ),
            Err(LaneConsensusDomainError::ActionIndexOutOfBounds {
                index: 1,
                routing_decisions: 1
            })
        );
    }

    #[test]
    fn lane_consensus_domains_reject_inconsistent_accepted_lane_dataspace() {
        assert_eq!(
            plan_lane_consensus_domains(
                &routing_for_lane_dataspaces(&[(1, 11), (1, 12)]),
                &accepted_schedule(&[0, 1]),
                &[committee(1, 11, vec![test_peer(1)], None)],
                "permissioned",
            ),
            Err(LaneConsensusDomainError::AcceptedLaneDataspaceMismatch {
                lane_id: LaneId::new(1),
                expected: DataSpaceId::new(11),
                actual: DataSpaceId::new(12),
            })
        );
    }

    #[test]
    fn lane_consensus_domains_reject_duplicate_committee() {
        assert_eq!(
            plan_lane_consensus_domains(
                &routing_for_lane_dataspaces(&[(1, 11)]),
                &accepted_schedule(&[0]),
                &[
                    committee(1, 11, vec![test_peer(1)], None),
                    committee(1, 11, vec![test_peer(2)], None),
                ],
                "permissioned",
            ),
            Err(LaneConsensusDomainError::DuplicateLaneCommittee {
                lane_id: LaneId::new(1)
            })
        );
    }

    #[test]
    fn lane_consensus_domains_reject_missing_committee_for_accepted_lane() {
        assert_eq!(
            plan_lane_consensus_domains(
                &routing_for_lane_dataspaces(&[(1, 11)]),
                &accepted_schedule(&[0]),
                &[],
                "permissioned",
            ),
            Err(LaneConsensusDomainError::MissingLaneCommittee {
                lane_id: LaneId::new(1)
            })
        );
    }

    #[test]
    fn lane_consensus_domains_reject_committee_dataspace_mismatch() {
        assert_eq!(
            plan_lane_consensus_domains(
                &routing_for_lane_dataspaces(&[(1, 11)]),
                &accepted_schedule(&[0]),
                &[committee(1, 12, vec![test_peer(1)], None)],
                "permissioned",
            ),
            Err(LaneConsensusDomainError::CommitteeDataspaceMismatch {
                lane_id: LaneId::new(1),
                expected: DataSpaceId::new(11),
                actual: DataSpaceId::new(12),
            })
        );
    }

    #[test]
    fn lane_consensus_domains_reject_empty_duplicate_and_invalid_quorum_committees() {
        let routing = routing_for_lane_dataspaces(&[(1, 11)]);
        assert_eq!(
            plan_lane_consensus_domains(
                &routing,
                &accepted_schedule(&[0]),
                &[committee(1, 11, Vec::new(), None)],
                "permissioned",
            ),
            Err(LaneConsensusDomainError::EmptyValidatorSet {
                lane_id: LaneId::new(1)
            })
        );

        let duplicate = test_peer(1);
        assert_eq!(
            plan_lane_consensus_domains(
                &routing,
                &accepted_schedule(&[0]),
                &[committee(1, 11, vec![duplicate.clone(), duplicate], None)],
                "permissioned",
            ),
            Err(LaneConsensusDomainError::DuplicateValidator {
                lane_id: LaneId::new(1)
            })
        );

        assert_eq!(
            plan_lane_consensus_domains(
                &routing,
                &accepted_schedule(&[0]),
                &[committee(1, 11, vec![test_peer(1), test_peer(2)], Some(3))],
                "permissioned",
            ),
            Err(LaneConsensusDomainError::InvalidQuorum {
                lane_id: LaneId::new(1),
                validator_count: 2,
                min_quorum: 3,
            })
        );
    }
}
