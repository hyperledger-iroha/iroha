//! Deterministic per-lane proposal scheduling helpers.

use std::collections::{BTreeMap, BTreeSet, VecDeque, btree_map::Entry};

use crate::queue::RoutingDecision;
use iroha_config::parameters::actual::Nexus;
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    block::consensus::{
        CertPhase, LaneBlockDescriptorV1, LaneBlockProposalV1, LaneBlockVoteBodyV1,
        SumeragiLanePayloadOwnership,
    },
    consensus::VALIDATOR_SET_HASH_VERSION_V1,
    nexus::{DataSpaceId, LaneId, LaneRelayEnvelope, LaneRelayQuorumContext},
    peer::PeerId,
};

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
    /// quorum for the validator set length is used. When supplied, it must
    /// match that deterministic threshold.
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
    /// Stable transaction hashes committed by this subject in accepted-candidate order.
    pub(super) accepted_transaction_hashes: Vec<Hash>,
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
    /// Descriptor hash of the latest committed lane-local block, when known.
    pub(super) latest_lane_block_descriptor_hash: Option<Hash>,
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
    /// Stable transaction hashes owned by this lane payload in accepted-candidate order.
    pub(super) accepted_transaction_hashes: Vec<Hash>,
    /// Stable digest naming lane-local payload ownership.
    pub(super) payload_ownership_hash: Hash,
    /// Stable digest naming the lane-local RBC instance for this payload.
    pub(super) rbc_instance_hash: Hash,
}

/// Replayable lane-local block descriptor for standalone lane scheduling.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct LaneBlockDescriptor {
    /// Lane whose local block is described.
    pub(super) lane_id: LaneId,
    /// Dataspace bound to the lane-local block.
    pub(super) dataspace_id: DataSpaceId,
    /// Latest committed lane-local height used as this block's predecessor tip.
    pub(super) previous_lane_block_height: u64,
    /// Descriptor hash of the predecessor tip, when the predecessor is known.
    pub(super) previous_lane_block_descriptor_hash: Option<Hash>,
    /// Lane-local block height assigned to the descriptor.
    pub(super) lane_block_height: u64,
    /// Lane-local view assigned to the descriptor.
    pub(super) lane_block_view: u64,
    /// Subject hash signed by lane-local voters.
    pub(super) subject_hash: Hash,
    /// Payload ownership hash used for DA/RBC handoff.
    pub(super) payload_ownership_hash: Hash,
    /// RBC instance hash used for DA/RBC handoff.
    pub(super) rbc_instance_hash: Hash,
    /// Accepted fetched-batch candidate indices in scheduler order.
    pub(super) accepted_candidate_indices: Vec<usize>,
    /// Accepted transaction hashes in scheduler order.
    pub(super) accepted_transaction_hashes: Vec<Hash>,
    /// Canonical validator order eligible to sign the lane-local block.
    pub(super) validator_set: Vec<PeerId>,
    /// Quorum context required for the lane-local block.
    pub(super) quorum: LaneRelayQuorumContext,
    /// Domain-separated QC mode tag used for lane-local votes.
    pub(super) qc_mode_tag: String,
    /// Stable descriptor digest binding predecessor, work, ownership, committee, and quorum.
    pub(super) descriptor_hash: Hash,
}

/// Standalone lane-local block proposal artifact ready for lane voting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct LaneBlockProposal {
    /// Replayable block descriptor proposed to the lane committee.
    pub(super) block_descriptor: LaneBlockDescriptor,
    /// Canonical lane-local vote/DA subject carried by the descriptor.
    pub(super) subject: LaneBlockSubject,
    /// DA/RBC ownership identity carried by the descriptor.
    pub(super) ownership: LanePayloadOwnership,
    /// Stable proposal digest binding descriptor, subject, ownership, and committee.
    pub(super) proposal_hash: Hash,
    /// Canonical public proposal artifact ready for broadcast.
    pub(super) artifact: LaneBlockProposalV1,
}

/// Lane-local vote record over a standalone lane block proposal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct LaneBlockVote {
    /// Lane-local QC phase this vote contributes to.
    pub(super) phase: CertPhase,
    /// Lane whose proposal is being voted on.
    pub(super) lane_id: LaneId,
    /// Dataspace bound to the lane proposal.
    pub(super) dataspace_id: DataSpaceId,
    /// Lane-local block height of the proposal.
    pub(super) lane_block_height: u64,
    /// Lane-local view of the proposal.
    pub(super) lane_block_view: u64,
    /// Proposal digest being signed.
    pub(super) proposal_hash: Hash,
    /// Descriptor digest carried by the proposal.
    pub(super) descriptor_hash: Hash,
    /// Stable digest of the validator set bound into the descriptor.
    pub(super) validator_set_hash: HashOf<Vec<PeerId>>,
    /// Signer index within the descriptor's canonical validator set.
    pub(super) signer_index: u32,
    /// Signer peer identity.
    pub(super) signer: PeerId,
    /// Canonical body to be signed by the lane validator.
    pub(super) body: LaneBlockVoteBodyV1,
    /// Common digest to sign for this lane proposal and phase.
    ///
    /// This intentionally excludes signer-local transport fields so every
    /// validator signs the same message and later BLS aggregation remains
    /// possible.
    pub(super) signing_hash: Hash,
}

/// Quorum-ready collection of lane-local votes for one proposal phase.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct LaneBlockVotePlan {
    /// Lane-local QC phase being assembled.
    pub(super) phase: CertPhase,
    /// Proposal digest certified by the vote set.
    pub(super) proposal_hash: Hash,
    /// Descriptor digest certified by the vote set.
    pub(super) descriptor_hash: Hash,
    /// Stable digest of the descriptor validator set.
    pub(super) validator_set_hash: HashOf<Vec<PeerId>>,
    /// Minimum distinct signer count required for quorum.
    pub(super) min_quorum: u32,
    /// Votes sorted by descriptor signer index.
    pub(super) votes: Vec<LaneBlockVote>,
}

/// One lane-local block payload descriptor for standalone lane scheduling.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct LanePayloadPlanEntry {
    /// Consensus domain, committee, and accepted work for this lane block.
    pub(super) domain: LaneConsensusDomain,
    /// Latest committed tip used to assign the next lane-local slot.
    pub(super) tip: LaneBlockTip,
    /// Next lane-local height/view selected for this lane block.
    pub(super) slot: LaneBlockSlot,
    /// Canonical lane-local vote/DA subject.
    pub(super) subject: LaneBlockSubject,
    /// DA/RBC ownership identity derived from the subject.
    pub(super) ownership: LanePayloadOwnership,
    /// Stable transaction hashes owned by this lane payload in accepted-candidate order.
    pub(super) accepted_transaction_hashes: Vec<Hash>,
    /// Replayable standalone lane-local block descriptor.
    pub(super) block_descriptor: LaneBlockDescriptor,
    /// Standalone lane-local block proposal artifact.
    pub(super) lane_block_proposal: LaneBlockProposal,
}

/// Full deterministic lane payload plan derived for accepted proposal work.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct LanePayloadPlan {
    /// Per-lane payload descriptors, sorted by lane id.
    pub(super) entries: Vec<LanePayloadPlanEntry>,
    /// Latest lane tips selected for accepted work after reset filtering.
    pub(super) lane_tips: Vec<LaneBlockTip>,
    /// Next lane-local block slots derived from the selected tips.
    pub(super) slots: Vec<LaneBlockSlot>,
    /// Lane-local vote/DA subjects for the selected slots.
    pub(super) subjects: Vec<LaneBlockSubject>,
    /// DA/RBC ownership identities for the selected subjects.
    pub(super) ownerships: Vec<LanePayloadOwnership>,
    /// Standalone lane block proposals derived from the entries.
    pub(super) lane_block_proposals: Vec<LaneBlockProposal>,
    /// Canonical public proposal artifacts ready for per-lane broadcast.
    pub(super) lane_block_proposal_artifacts: Vec<LaneBlockProposalV1>,
    /// Full-committee prepare vote templates for each standalone lane proposal.
    pub(super) lane_block_prepare_vote_plans: Vec<LaneBlockVotePlan>,
    /// Full-committee commit vote templates for each standalone lane proposal.
    pub(super) lane_block_commit_vote_plans: Vec<LaneBlockVotePlan>,
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
    /// Two known tips at the same lane-local height carry different descriptor hashes.
    ConflictingLaneTipDescriptorHash {
        /// Lane being planned.
        lane_id: LaneId,
        /// Conflicting lane-local tip height.
        latest_lane_block_height: u64,
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
    /// Accepted candidate index does not have a matching transaction hash.
    CandidateHashIndexOutOfBounds {
        /// Lane being planned.
        lane_id: LaneId,
        /// Referenced accepted candidate index.
        index: usize,
        /// Number of transaction hashes supplied for the fetched candidate batch.
        candidate_hashes: usize,
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
    /// Candidate index and transaction-hash lists have different lengths.
    CandidateHashCountMismatch {
        /// Lane being planned.
        lane_id: LaneId,
        /// Number of candidate indices carried by the subject.
        candidate_indices: usize,
        /// Number of transaction hashes carried by the subject.
        candidate_hashes: usize,
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
    /// Planner stages produced mismatched per-lane descriptors.
    InconsistentEntry {
        /// Lane whose descriptor could not be assembled consistently.
        lane_id: LaneId,
    },
    /// Accepted candidate index does not have a matching transaction hash.
    CandidateHashIndexOutOfBounds {
        /// Lane whose accepted work references a missing hash.
        lane_id: LaneId,
        /// Referenced accepted candidate index.
        index: usize,
        /// Number of transaction hashes supplied for the fetched candidate batch.
        candidate_hashes: usize,
    },
    /// Lane-local vote templates could not be derived from a proposal artifact.
    VotePlans(LaneBlockVotePlanError),
}

/// Error returned when lane-local votes cannot be planned safely.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LaneBlockVotePlanError {
    /// Lane proposal votes only certify prepare or commit phases.
    InvalidPhase {
        /// Rejected certificate phase.
        phase: CertPhase,
    },
    /// Proposal fields do not agree with the embedded descriptor, subject, or ownership.
    InconsistentProposal {
        /// Lane whose proposal is internally inconsistent.
        lane_id: LaneId,
    },
    /// Descriptor validator set is empty.
    EmptyValidatorSet {
        /// Lane whose descriptor has no validators.
        lane_id: LaneId,
    },
    /// Descriptor validator set is not sorted canonically.
    ValidatorSetNotCanonical {
        /// Lane whose descriptor has a non-canonical validator set.
        lane_id: LaneId,
    },
    /// Descriptor validator set contains a duplicate peer.
    DuplicateValidator {
        /// Lane whose descriptor repeats a validator.
        lane_id: LaneId,
    },
    /// Descriptor validator count does not fit the relay quorum format.
    ValidatorCountOverflow {
        /// Lane whose validator set is too large.
        lane_id: LaneId,
    },
    /// Descriptor quorum does not match the validator set.
    InvalidQuorum {
        /// Lane with the invalid quorum.
        lane_id: LaneId,
        /// Number of validators in the descriptor set.
        validator_count: u32,
        /// Required quorum threshold.
        min_quorum: u32,
    },
    /// Descriptor hash does not match the embedded descriptor fields.
    DescriptorHashMismatch {
        /// Lane whose descriptor hash mismatched.
        lane_id: LaneId,
        /// Digest recomputed from the descriptor fields.
        expected: Hash,
        /// Digest carried by the descriptor.
        actual: Hash,
    },
    /// Proposal hash does not match the embedded proposal fields.
    ProposalHashMismatch {
        /// Lane whose proposal hash mismatched.
        lane_id: LaneId,
        /// Digest recomputed from the proposal fields.
        expected: Hash,
        /// Digest carried by the proposal.
        actual: Hash,
    },
    /// Signer is not a member of the descriptor validator set.
    SignerNotInCommittee {
        /// Lane whose committee rejected the signer.
        lane_id: LaneId,
    },
    /// A signer was supplied more than once for the same proposal phase.
    DuplicateSigner {
        /// Lane whose vote set repeated a signer.
        lane_id: LaneId,
    },
    /// Distinct signer count is below the descriptor quorum.
    InsufficientVoteQuorum {
        /// Lane whose vote set is below quorum.
        lane_id: LaneId,
        /// Distinct votes observed.
        observed: u32,
        /// Required quorum threshold.
        min_quorum: u32,
    },
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
    /// Committee quorum does not match the deterministic validator-set quorum.
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

#[derive(Clone, Debug)]
struct LaneBlockProposalVoteContext {
    candidate_indices: Vec<u64>,
    candidate_hashes: Vec<Hash>,
    validator_set_hash: HashOf<Vec<PeerId>>,
    validator_count: u32,
    min_quorum: u32,
}

fn canonical_lane_commit_quorum(validator_set_len: usize) -> Option<u32> {
    u32::try_from(
        crate::sumeragi::network_topology::commit_quorum_from_len(validator_set_len).max(1),
    )
    .ok()
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
        let expected_min_quorum = canonical_lane_commit_quorum(validator_set.len())
            .ok_or(LaneConsensusDomainError::ValidatorCountOverflow { lane_id })?;
        let min_quorum = committee.min_quorum.unwrap_or(expected_min_quorum);
        if min_quorum != expected_min_quorum {
            return Err(LaneConsensusDomainError::InvalidQuorum {
                lane_id,
                validator_count,
                min_quorum,
            });
        }
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
/// incarnation. Planning floors known tips at that watermark and uses the
/// watermark as the missing-tip baseline for reset lanes, so recreated lanes
/// resume at `reset_height + 1` instead of inheriting global-height
/// compatibility coordinates from the old incarnation.
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
        let latest_lane_block_height = tip.latest_lane_block_height.max(floor_height);
        let latest_lane_block_descriptor_hash = if latest_lane_block_height
            == tip.latest_lane_block_height
            && reset_height.is_none_or(|height| tip.latest_lane_block_height > height)
        {
            tip.latest_lane_block_descriptor_hash
        } else {
            None
        };
        let floored_tip = LaneBlockTip {
            latest_lane_block_height,
            latest_lane_block_descriptor_hash,
            ..*tip
        };
        match latest_by_lane.entry(tip.lane_id) {
            Entry::Occupied(mut entry) => {
                if floored_tip.latest_lane_block_height > entry.get().latest_lane_block_height {
                    entry.insert(floored_tip);
                } else if floored_tip.latest_lane_block_height
                    == entry.get().latest_lane_block_height
                {
                    match (
                        entry.get().latest_lane_block_descriptor_hash,
                        floored_tip.latest_lane_block_descriptor_hash,
                    ) {
                        (Some(existing), Some(incoming)) if existing != incoming => {
                            return Err(LaneBlockTipPlanError::ConflictingLaneTipDescriptorHash {
                                lane_id: tip.lane_id,
                                latest_lane_block_height: floored_tip.latest_lane_block_height,
                            });
                        }
                        (None, Some(_)) => {
                            entry.insert(floored_tip);
                        }
                        _ => {}
                    }
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
            let baseline = reset_heights
                .get(&lane_id)
                .copied()
                .unwrap_or(compatibility_latest_lane_block_height);
            latest_by_lane
                .get(&lane_id)
                .copied()
                .unwrap_or(LaneBlockTip {
                    lane_id,
                    dataspace_id: domain.dataspace_id,
                    latest_lane_block_height: baseline,
                    latest_lane_block_descriptor_hash: None,
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
    candidate_hashes: &[Hash],
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
    plan_lane_block_subjects_for_slots(domains, candidate_hashes, &slots)
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
    candidate_hashes: &[Hash],
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
        let mut accepted_transaction_hashes =
            Vec::with_capacity(domain.accepted_candidate_indices.len());
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
            let hash = candidate_hashes.get(index).copied().ok_or(
                LaneBlockSubjectError::CandidateHashIndexOutOfBounds {
                    lane_id: domain.lane_id,
                    index,
                    candidate_hashes: candidate_hashes.len(),
                },
            )?;
            accepted_transaction_hashes.push(hash);
        }

        let subject_hash = SumeragiLanePayloadOwnership::compute_replay_subject_hash(
            domain.lane_id,
            domain.dataspace_id,
            slot.lane_block_height,
            slot.lane_block_view,
            &candidate_indices,
            &accepted_transaction_hashes,
            &domain.qc_mode_tag,
        )
        .map_err(|_| LaneBlockSubjectError::Encode)?;
        subjects.push(LaneBlockSubject {
            lane_id: domain.lane_id,
            dataspace_id: domain.dataspace_id,
            lane_block_height: slot.lane_block_height,
            lane_block_view: slot.lane_block_view,
            accepted_candidate_indices: domain.accepted_candidate_indices.clone(),
            accepted_transaction_hashes,
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
        if subject.accepted_candidate_indices.len() != subject.accepted_transaction_hashes.len() {
            return Err(LanePayloadOwnershipError::CandidateHashCountMismatch {
                lane_id: subject.lane_id,
                candidate_indices: subject.accepted_candidate_indices.len(),
                candidate_hashes: subject.accepted_transaction_hashes.len(),
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

        let expected_subject_hash = SumeragiLanePayloadOwnership::compute_replay_subject_hash(
            subject.lane_id,
            subject.dataspace_id,
            subject.lane_block_height,
            subject.lane_block_view,
            &candidate_indices,
            &subject.accepted_transaction_hashes,
            &subject.qc_mode_tag,
        )
        .map_err(|_| LanePayloadOwnershipError::Encode)?;
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

        let payload_ownership_hash =
            SumeragiLanePayloadOwnership::compute_replay_payload_ownership_hash(
                subject.lane_id,
                subject.dataspace_id,
                subject.lane_block_height,
                subject.lane_block_view,
                subject.subject_hash,
                &candidate_indices,
                &subject.accepted_transaction_hashes,
                &subject.qc_mode_tag,
            )
            .map_err(|_| LanePayloadOwnershipError::Encode)?;
        if !seen_payload_ownership_hashes.insert(payload_ownership_hash) {
            return Err(LanePayloadOwnershipError::DuplicatePayloadOwnershipHash {
                payload_ownership_hash,
            });
        }

        let rbc_instance_hash = SumeragiLanePayloadOwnership::compute_replay_rbc_instance_hash(
            subject.lane_id,
            subject.dataspace_id,
            subject.lane_block_height,
            subject.lane_block_view,
            subject.subject_hash,
            payload_ownership_hash,
        )
        .map_err(|_| LanePayloadOwnershipError::Encode)?;
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
            accepted_transaction_hashes: subject.accepted_transaction_hashes.clone(),
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
    candidate_hashes: &[Hash],
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
    let subjects = plan_lane_block_subjects_for_slots(domains, candidate_hashes, &slots)
        .map_err(LanePayloadPlanError::Subjects)?;
    let ownerships =
        plan_lane_payload_ownership(&subjects).map_err(LanePayloadPlanError::Ownerships)?;
    let entries = build_lane_payload_plan_entries(
        domains,
        &lane_tips,
        &slots,
        &subjects,
        &ownerships,
        candidate_hashes,
    )?;
    let lane_block_proposals = entries
        .iter()
        .map(|entry| entry.lane_block_proposal.clone())
        .collect();
    let lane_block_proposal_artifacts = entries
        .iter()
        .map(|entry| entry.lane_block_proposal.artifact.clone())
        .collect();
    let lane_block_prepare_vote_plans = entries
        .iter()
        .map(|entry| {
            plan_lane_block_vote_quorum(
                &entry.lane_block_proposal,
                CertPhase::Prepare,
                &entry.block_descriptor.validator_set,
            )
            .map_err(LanePayloadPlanError::VotePlans)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let lane_block_commit_vote_plans = entries
        .iter()
        .map(|entry| {
            plan_lane_block_vote_quorum(
                &entry.lane_block_proposal,
                CertPhase::Commit,
                &entry.block_descriptor.validator_set,
            )
            .map_err(LanePayloadPlanError::VotePlans)
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(LanePayloadPlan {
        entries,
        lane_tips,
        slots,
        subjects,
        ownerships,
        lane_block_proposals,
        lane_block_proposal_artifacts,
        lane_block_prepare_vote_plans,
        lane_block_commit_vote_plans,
    })
}

fn build_lane_payload_plan_entries(
    domains: &[LaneConsensusDomain],
    lane_tips: &[LaneBlockTip],
    slots: &[LaneBlockSlot],
    subjects: &[LaneBlockSubject],
    ownerships: &[LanePayloadOwnership],
    candidate_hashes: &[Hash],
) -> Result<Vec<LanePayloadPlanEntry>, LanePayloadPlanError> {
    let tips_by_lane = lane_tips
        .iter()
        .map(|tip| (tip.lane_id, tip))
        .collect::<BTreeMap<_, _>>();
    let slots_by_lane = slots
        .iter()
        .map(|slot| (slot.lane_id, slot))
        .collect::<BTreeMap<_, _>>();
    let subjects_by_lane = subjects
        .iter()
        .map(|subject| (subject.lane_id, subject))
        .collect::<BTreeMap<_, _>>();
    let ownerships_by_lane = ownerships
        .iter()
        .map(|ownership| (ownership.lane_id, ownership))
        .collect::<BTreeMap<_, _>>();

    let mut entries = Vec::with_capacity(domains.len());
    for domain in domains {
        let tip = tips_by_lane.get(&domain.lane_id).copied().ok_or(
            LanePayloadPlanError::InconsistentEntry {
                lane_id: domain.lane_id,
            },
        )?;
        let slot = slots_by_lane.get(&domain.lane_id).copied().ok_or(
            LanePayloadPlanError::InconsistentEntry {
                lane_id: domain.lane_id,
            },
        )?;
        let subject = subjects_by_lane.get(&domain.lane_id).copied().ok_or(
            LanePayloadPlanError::InconsistentEntry {
                lane_id: domain.lane_id,
            },
        )?;
        let ownership = ownerships_by_lane.get(&domain.lane_id).copied().ok_or(
            LanePayloadPlanError::InconsistentEntry {
                lane_id: domain.lane_id,
            },
        )?;

        let expected_next_height = tip.latest_lane_block_height.checked_add(1).ok_or(
            LanePayloadPlanError::InconsistentEntry {
                lane_id: domain.lane_id,
            },
        )?;
        let accepted_transaction_hashes = domain
            .accepted_candidate_indices
            .iter()
            .map(|index| {
                candidate_hashes.get(*index).copied().ok_or(
                    LanePayloadPlanError::CandidateHashIndexOutOfBounds {
                        lane_id: domain.lane_id,
                        index: *index,
                        candidate_hashes: candidate_hashes.len(),
                    },
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let is_consistent = tip.dataspace_id == domain.dataspace_id
            && slot.dataspace_id == domain.dataspace_id
            && slot.lane_block_height == expected_next_height
            && subject.dataspace_id == domain.dataspace_id
            && subject.lane_block_height == slot.lane_block_height
            && subject.lane_block_view == slot.lane_block_view
            && subject.accepted_candidate_indices == domain.accepted_candidate_indices
            && subject.accepted_transaction_hashes == accepted_transaction_hashes
            && subject.qc_mode_tag == domain.qc_mode_tag
            && ownership.dataspace_id == subject.dataspace_id
            && ownership.lane_block_height == subject.lane_block_height
            && ownership.lane_block_view == subject.lane_block_view
            && ownership.subject_hash == subject.subject_hash
            && ownership.qc_mode_tag == subject.qc_mode_tag
            && ownership.accepted_candidate_indices == subject.accepted_candidate_indices
            && ownership.accepted_transaction_hashes == subject.accepted_transaction_hashes;
        if !is_consistent {
            return Err(LanePayloadPlanError::InconsistentEntry {
                lane_id: domain.lane_id,
            });
        }
        for index in domain.accepted_candidate_indices.iter().copied() {
            u64::try_from(index).map_err(|_| LanePayloadPlanError::InconsistentEntry {
                lane_id: domain.lane_id,
            })?;
        }
        let mut block_descriptor = LaneBlockDescriptor {
            lane_id: domain.lane_id,
            dataspace_id: domain.dataspace_id,
            previous_lane_block_height: tip.latest_lane_block_height,
            previous_lane_block_descriptor_hash: tip.latest_lane_block_descriptor_hash,
            lane_block_height: slot.lane_block_height,
            lane_block_view: slot.lane_block_view,
            subject_hash: subject.subject_hash,
            payload_ownership_hash: ownership.payload_ownership_hash,
            rbc_instance_hash: ownership.rbc_instance_hash,
            accepted_candidate_indices: domain.accepted_candidate_indices.clone(),
            accepted_transaction_hashes: accepted_transaction_hashes.clone(),
            validator_set: domain.validator_set.clone(),
            quorum: domain.quorum,
            qc_mode_tag: domain.qc_mode_tag.clone(),
            descriptor_hash: Hash::prehashed([0_u8; Hash::LENGTH]),
        };
        block_descriptor.descriptor_hash =
            lane_block_descriptor_artifact(&block_descriptor).computed_descriptor_hash();
        let lane_block_proposal =
            build_lane_block_proposal(domain.lane_id, &block_descriptor, subject, ownership)?;

        entries.push(LanePayloadPlanEntry {
            domain: domain.clone(),
            tip: *tip,
            slot: *slot,
            subject: (*subject).clone(),
            ownership: (*ownership).clone(),
            accepted_transaction_hashes,
            block_descriptor,
            lane_block_proposal,
        });
    }

    entries.sort_by_key(|entry| entry.domain.lane_id);
    Ok(entries)
}

fn build_lane_block_proposal(
    lane_id: LaneId,
    block_descriptor: &LaneBlockDescriptor,
    subject: &LaneBlockSubject,
    ownership: &LanePayloadOwnership,
) -> Result<LaneBlockProposal, LanePayloadPlanError> {
    let descriptor_candidate_hashes = block_descriptor
        .accepted_transaction_hashes
        .iter()
        .copied()
        .collect::<Vec<_>>();
    for index in block_descriptor.accepted_candidate_indices.iter().copied() {
        u64::try_from(index).map_err(|_| LanePayloadPlanError::InconsistentEntry { lane_id })?;
    }
    let is_consistent = block_descriptor.lane_id == subject.lane_id
        && block_descriptor.dataspace_id == subject.dataspace_id
        && block_descriptor.lane_block_height == subject.lane_block_height
        && block_descriptor.lane_block_view == subject.lane_block_view
        && block_descriptor.subject_hash == subject.subject_hash
        && block_descriptor.payload_ownership_hash == ownership.payload_ownership_hash
        && block_descriptor.rbc_instance_hash == ownership.rbc_instance_hash
        && block_descriptor.accepted_candidate_indices == subject.accepted_candidate_indices
        && block_descriptor.accepted_candidate_indices == ownership.accepted_candidate_indices
        && descriptor_candidate_hashes == subject.accepted_transaction_hashes
        && descriptor_candidate_hashes == ownership.accepted_transaction_hashes
        && block_descriptor.qc_mode_tag == subject.qc_mode_tag
        && block_descriptor.qc_mode_tag == ownership.qc_mode_tag
        && ownership.lane_id == subject.lane_id
        && ownership.dataspace_id == subject.dataspace_id
        && ownership.lane_block_height == subject.lane_block_height
        && ownership.lane_block_view == subject.lane_block_view
        && ownership.subject_hash == subject.subject_hash;
    if !is_consistent {
        return Err(LanePayloadPlanError::InconsistentEntry { lane_id });
    }

    let artifact_descriptor = lane_block_descriptor_artifact(block_descriptor);
    let mut artifact = LaneBlockProposalV1 {
        descriptor: artifact_descriptor,
        proposal_hash: Hash::prehashed([0_u8; Hash::LENGTH]),
    };
    let proposal_hash = artifact.computed_proposal_hash();
    artifact.proposal_hash = proposal_hash;

    Ok(LaneBlockProposal {
        block_descriptor: block_descriptor.clone(),
        subject: subject.clone(),
        ownership: ownership.clone(),
        proposal_hash,
        artifact,
    })
}

fn lane_block_descriptor_artifact(descriptor: &LaneBlockDescriptor) -> LaneBlockDescriptorV1 {
    let accepted_candidate_indices = descriptor
        .accepted_candidate_indices
        .iter()
        .copied()
        .map(|index| u64::try_from(index).expect("descriptor candidate index already checked"))
        .collect::<Vec<_>>();
    LaneBlockDescriptorV1 {
        lane_id: descriptor.lane_id,
        dataspace_id: descriptor.dataspace_id,
        previous_lane_block_height: descriptor.previous_lane_block_height,
        previous_lane_block_descriptor_hash: descriptor.previous_lane_block_descriptor_hash,
        lane_block_height: descriptor.lane_block_height,
        lane_block_view: descriptor.lane_block_view,
        subject_hash: descriptor.subject_hash,
        payload_ownership_hash: descriptor.payload_ownership_hash,
        rbc_instance_hash: descriptor.rbc_instance_hash,
        accepted_candidate_indices,
        accepted_transaction_hashes: descriptor.accepted_transaction_hashes.clone(),
        validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
        validator_set_hash: HashOf::new(&descriptor.validator_set),
        validator_set: descriptor.validator_set.clone(),
        validator_count: descriptor.quorum.validator_count,
        min_quorum: descriptor.quorum.min_quorum,
        qc_mode_tag: descriptor.qc_mode_tag.clone(),
        descriptor_hash: descriptor.descriptor_hash,
    }
}

/// Build a single lane-local vote for a standalone proposal and signer.
pub(super) fn plan_lane_block_vote(
    proposal: &LaneBlockProposal,
    phase: CertPhase,
    signer: &PeerId,
) -> Result<LaneBlockVote, LaneBlockVotePlanError> {
    let (mut votes, _) =
        plan_lane_block_votes_with_context(proposal, phase, std::slice::from_ref(signer))?;
    votes
        .pop()
        .ok_or(LaneBlockVotePlanError::SignerNotInCommittee {
            lane_id: proposal.block_descriptor.lane_id,
        })
}

/// Build deterministic lane-local votes for the supplied signers.
///
/// Returned votes are sorted by descriptor signer index. The signable digest is
/// common across all returned votes for a given proposal and phase.
pub(super) fn plan_lane_block_votes(
    proposal: &LaneBlockProposal,
    phase: CertPhase,
    signers: &[PeerId],
) -> Result<Vec<LaneBlockVote>, LaneBlockVotePlanError> {
    if let [signer] = signers {
        return plan_lane_block_vote(proposal, phase, signer).map(|vote| vec![vote]);
    }
    let (votes, _) = plan_lane_block_votes_with_context(proposal, phase, signers)?;
    Ok(votes)
}

/// Build a quorum-ready lane-local vote plan for a proposal phase.
pub(super) fn plan_lane_block_vote_quorum(
    proposal: &LaneBlockProposal,
    phase: CertPhase,
    signers: &[PeerId],
) -> Result<LaneBlockVotePlan, LaneBlockVotePlanError> {
    let votes = plan_lane_block_votes(proposal, phase, signers)?;
    let context = validate_lane_block_proposal_for_vote(proposal)?;
    let observed = u32::try_from(votes.len()).unwrap_or(context.validator_count);
    if observed < context.min_quorum {
        return Err(LaneBlockVotePlanError::InsufficientVoteQuorum {
            lane_id: proposal.block_descriptor.lane_id,
            observed,
            min_quorum: context.min_quorum,
        });
    }

    Ok(LaneBlockVotePlan {
        phase,
        proposal_hash: proposal.proposal_hash,
        descriptor_hash: proposal.block_descriptor.descriptor_hash,
        validator_set_hash: context.validator_set_hash,
        min_quorum: context.min_quorum,
        votes,
    })
}

fn plan_lane_block_votes_with_context(
    proposal: &LaneBlockProposal,
    phase: CertPhase,
    signers: &[PeerId],
) -> Result<(Vec<LaneBlockVote>, LaneBlockProposalVoteContext), LaneBlockVotePlanError> {
    validate_lane_block_vote_phase(phase)?;
    let context = validate_lane_block_proposal_for_vote(proposal)?;
    let body = lane_block_vote_body(proposal, phase, &context);
    let signing_hash = Hash::new(body.signature_preimage());
    let mut seen_signers = BTreeSet::new();
    let mut votes = Vec::with_capacity(signers.len());

    for signer in signers {
        if !seen_signers.insert(signer) {
            return Err(LaneBlockVotePlanError::DuplicateSigner {
                lane_id: proposal.block_descriptor.lane_id,
            });
        }
        let signer_index = proposal
            .block_descriptor
            .validator_set
            .iter()
            .position(|validator| validator == signer)
            .ok_or(LaneBlockVotePlanError::SignerNotInCommittee {
                lane_id: proposal.block_descriptor.lane_id,
            })
            .and_then(|index| {
                u32::try_from(index).map_err(|_| LaneBlockVotePlanError::ValidatorCountOverflow {
                    lane_id: proposal.block_descriptor.lane_id,
                })
            })?;
        votes.push(LaneBlockVote {
            phase,
            lane_id: proposal.block_descriptor.lane_id,
            dataspace_id: proposal.block_descriptor.dataspace_id,
            lane_block_height: proposal.block_descriptor.lane_block_height,
            lane_block_view: proposal.block_descriptor.lane_block_view,
            proposal_hash: proposal.proposal_hash,
            descriptor_hash: proposal.block_descriptor.descriptor_hash,
            validator_set_hash: context.validator_set_hash,
            signer_index,
            signer: signer.clone(),
            body: body.clone(),
            signing_hash,
        });
    }

    votes.sort_by_key(|vote| vote.signer_index);
    Ok((votes, context))
}

fn validate_lane_block_vote_phase(phase: CertPhase) -> Result<(), LaneBlockVotePlanError> {
    match phase {
        CertPhase::Prepare | CertPhase::Commit => Ok(()),
        CertPhase::NewView => Err(LaneBlockVotePlanError::InvalidPhase { phase }),
    }
}

fn validate_lane_block_proposal_for_vote(
    proposal: &LaneBlockProposal,
) -> Result<LaneBlockProposalVoteContext, LaneBlockVotePlanError> {
    let descriptor = &proposal.block_descriptor;
    let lane_id = descriptor.lane_id;
    let descriptor_candidate_hashes = descriptor
        .accepted_transaction_hashes
        .iter()
        .copied()
        .map(Hash::from)
        .collect::<Vec<_>>();
    let candidate_indices = descriptor
        .accepted_candidate_indices
        .iter()
        .copied()
        .map(|index| {
            u64::try_from(index)
                .map_err(|_| LaneBlockVotePlanError::InconsistentProposal { lane_id })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let is_consistent = descriptor.lane_id == proposal.subject.lane_id
        && descriptor.dataspace_id == proposal.subject.dataspace_id
        && descriptor.lane_block_height == proposal.subject.lane_block_height
        && descriptor.lane_block_view == proposal.subject.lane_block_view
        && descriptor.subject_hash == proposal.subject.subject_hash
        && descriptor.payload_ownership_hash == proposal.ownership.payload_ownership_hash
        && descriptor.rbc_instance_hash == proposal.ownership.rbc_instance_hash
        && descriptor.accepted_candidate_indices == proposal.subject.accepted_candidate_indices
        && descriptor.accepted_candidate_indices == proposal.ownership.accepted_candidate_indices
        && descriptor_candidate_hashes == proposal.subject.accepted_transaction_hashes
        && descriptor_candidate_hashes == proposal.ownership.accepted_transaction_hashes
        && descriptor.qc_mode_tag == proposal.subject.qc_mode_tag
        && descriptor.qc_mode_tag == proposal.ownership.qc_mode_tag
        && proposal.ownership.lane_id == proposal.subject.lane_id
        && proposal.ownership.dataspace_id == proposal.subject.dataspace_id
        && proposal.ownership.lane_block_height == proposal.subject.lane_block_height
        && proposal.ownership.lane_block_view == proposal.subject.lane_block_view
        && proposal.ownership.subject_hash == proposal.subject.subject_hash;
    if !is_consistent {
        return Err(LaneBlockVotePlanError::InconsistentProposal { lane_id });
    }

    if descriptor.validator_set.is_empty() {
        return Err(LaneBlockVotePlanError::EmptyValidatorSet { lane_id });
    }
    let validator_count = u32::try_from(descriptor.validator_set.len())
        .map_err(|_| LaneBlockVotePlanError::ValidatorCountOverflow { lane_id })?;
    let mut canonical_validator_set = descriptor.validator_set.clone();
    canonical_validator_set.sort();
    if canonical_validator_set != descriptor.validator_set {
        return Err(LaneBlockVotePlanError::ValidatorSetNotCanonical { lane_id });
    }
    for pair in canonical_validator_set.windows(2) {
        if pair[0] == pair[1] {
            return Err(LaneBlockVotePlanError::DuplicateValidator { lane_id });
        }
    }
    let expected_min_quorum = canonical_lane_commit_quorum(descriptor.validator_set.len())
        .ok_or(LaneBlockVotePlanError::ValidatorCountOverflow { lane_id })?;
    if descriptor.quorum.validator_count != validator_count
        || descriptor.quorum.min_quorum != expected_min_quorum
    {
        return Err(LaneBlockVotePlanError::InvalidQuorum {
            lane_id,
            validator_count,
            min_quorum: descriptor.quorum.min_quorum,
        });
    }

    let expected_descriptor_hash =
        lane_block_descriptor_artifact(descriptor).computed_descriptor_hash();
    if expected_descriptor_hash != descriptor.descriptor_hash {
        return Err(LaneBlockVotePlanError::DescriptorHashMismatch {
            lane_id,
            expected: expected_descriptor_hash,
            actual: descriptor.descriptor_hash,
        });
    }
    let expected_artifact_descriptor = lane_block_descriptor_artifact(descriptor);
    if proposal.artifact.descriptor != expected_artifact_descriptor {
        return Err(LaneBlockVotePlanError::InconsistentProposal { lane_id });
    }
    let artifact_descriptor_hash = proposal.artifact.descriptor.computed_descriptor_hash();
    if artifact_descriptor_hash != descriptor.descriptor_hash {
        return Err(LaneBlockVotePlanError::DescriptorHashMismatch {
            lane_id,
            expected: artifact_descriptor_hash,
            actual: descriptor.descriptor_hash,
        });
    }

    let expected_proposal_hash = proposal.artifact.computed_proposal_hash();
    if expected_proposal_hash != proposal.proposal_hash {
        return Err(LaneBlockVotePlanError::ProposalHashMismatch {
            lane_id,
            expected: expected_proposal_hash,
            actual: proposal.proposal_hash,
        });
    }
    if proposal.artifact.proposal_hash != proposal.proposal_hash {
        return Err(LaneBlockVotePlanError::ProposalHashMismatch {
            lane_id,
            expected: proposal.proposal_hash,
            actual: proposal.artifact.proposal_hash,
        });
    }
    let artifact_proposal_hash = proposal.artifact.computed_proposal_hash();
    if artifact_proposal_hash != proposal.proposal_hash {
        return Err(LaneBlockVotePlanError::ProposalHashMismatch {
            lane_id,
            expected: artifact_proposal_hash,
            actual: proposal.proposal_hash,
        });
    }

    let validator_set_hash = HashOf::new(&descriptor.validator_set);

    Ok(LaneBlockProposalVoteContext {
        candidate_indices,
        candidate_hashes: descriptor_candidate_hashes,
        validator_set_hash,
        validator_count,
        min_quorum: descriptor.quorum.min_quorum,
    })
}

fn lane_block_vote_body(
    proposal: &LaneBlockProposal,
    phase: CertPhase,
    context: &LaneBlockProposalVoteContext,
) -> LaneBlockVoteBodyV1 {
    let body = proposal.artifact.vote_body(phase);
    debug_assert_eq!(body.accepted_candidate_indices, context.candidate_indices);
    debug_assert_eq!(body.accepted_transaction_hashes, context.candidate_hashes);
    debug_assert_eq!(body.validator_set_hash, context.validator_set_hash);
    debug_assert_eq!(body.validator_count, context.validator_count);
    debug_assert_eq!(body.min_quorum, context.min_quorum);
    body
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
    use iroha_data_model::{
        block::consensus::SumeragiLanePayloadOwnership,
        nexus::{
            AUTOSCALE_META_CREATED_HEIGHT, AUTOSCALE_META_MANAGED, DataSpaceCatalog, DataSpaceId,
            LaneCatalog, LaneConfig, LaneId,
        },
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

    fn tx_hash(seed: u8) -> Hash {
        Hash::prehashed([seed; Hash::LENGTH])
    }

    fn tx_hashes(count: usize) -> Vec<Hash> {
        (0..count)
            .map(|index| tx_hash(u8::try_from(index + 1).expect("test hash seed fits u8")))
            .collect()
    }

    fn lane_tip(lane: u32, dataspace: u64, latest_lane_block_height: u64) -> LaneBlockTip {
        LaneBlockTip {
            lane_id: LaneId::new(lane),
            dataspace_id: DataSpaceId::new(dataspace),
            latest_lane_block_height,
            latest_lane_block_descriptor_hash: None,
        }
    }

    fn lane_tip_with_descriptor(
        lane: u32,
        dataspace: u64,
        latest_lane_block_height: u64,
        descriptor_seed: u8,
    ) -> LaneBlockTip {
        LaneBlockTip {
            latest_lane_block_descriptor_hash: Some(Hash::prehashed(
                [descriptor_seed; Hash::LENGTH],
            )),
            ..lane_tip(lane, dataspace, latest_lane_block_height)
        }
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

    fn lane_block_proposal_with_committee(
        validators: Vec<PeerId>,
        min_quorum: Option<u32>,
    ) -> LaneBlockProposal {
        let routing = routing_for_lane_dataspaces(&[(1, 11)]);
        let domains = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[0]),
            &[committee(1, 11, validators, min_quorum)],
            "permissioned",
        )
        .expect("lane consensus domain");
        let plan = plan_lane_payload(
            &domains,
            &[lane_tip_with_descriptor(1, 11, 3, 0xA7)],
            &[tx_hash(0xC7)],
            3,
            &BTreeMap::new(),
            2,
        )
        .expect("lane payload plan");
        plan.entries[0].lane_block_proposal.clone()
    }

    fn refresh_lane_block_proposal_hashes(proposal: &mut LaneBlockProposal) {
        proposal.block_descriptor.descriptor_hash =
            lane_block_descriptor_artifact(&proposal.block_descriptor).computed_descriptor_hash();
        proposal.artifact.descriptor = lane_block_descriptor_artifact(&proposal.block_descriptor);
        let proposal_hash = proposal.artifact.computed_proposal_hash();
        proposal.artifact.proposal_hash = proposal_hash;
        proposal.proposal_hash = proposal_hash;
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

        let subjects =
            plan_lane_block_subjects(&domains, &tx_hashes(4), 42, 7).expect("lane block subjects");

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

        let view_drift = plan_lane_block_subjects(&domains, &tx_hashes(4), 42, 8)
            .expect("lane block subjects with view drift");
        assert_ne!(subjects[0].subject_hash, view_drift[0].subject_hash);

        let mut reordered_work = domains.clone();
        reordered_work[0].accepted_candidate_indices.reverse();
        let reordered_subjects = plan_lane_block_subjects(&reordered_work, &tx_hashes(4), 42, 7)
            .expect("reordered subjects");
        assert_ne!(subjects[0].subject_hash, reordered_subjects[0].subject_hash);

        let mut mode_drift = domains.clone();
        mode_drift[0].qc_mode_tag.push_str("::tampered");
        let mode_drift_subjects = plan_lane_block_subjects(&mode_drift, &tx_hashes(4), 42, 7)
            .expect("mode drift subjects");
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

        let subjects =
            plan_lane_block_subjects(&domains, &tx_hashes(2), 3, 4).expect("lane block subjects");
        let reversed_subjects = plan_lane_block_subjects(&reversed_domains, &tx_hashes(2), 3, 4)
            .expect("reversed subjects");

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

        let subjects = plan_lane_block_subjects_for_slots(&domains, &tx_hashes(4), &slots)
            .expect("slotted subjects");

        assert_eq!(subjects.len(), 2);
        assert_eq!(subjects[0].lane_id, LaneId::new(1));
        assert_eq!(subjects[0].lane_block_height, 10);
        assert_eq!(subjects[0].lane_block_view, 1);
        assert_eq!(subjects[1].lane_id, LaneId::new(2));
        assert_eq!(subjects[1].lane_block_height, 4);
        assert_eq!(subjects[1].lane_block_view, 8);

        let global_subjects = plan_lane_block_subjects(&domains, &tx_hashes(4), 10, 1)
            .expect("global compatibility subjects");
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
        let lane_tips = vec![lane_tip(2, 22, 3), lane_tip(7, 77, 31), lane_tip(1, 11, 9)];

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

        let subjects = plan_lane_block_subjects_for_slots(&domains, &tx_hashes(4), &slots)
            .expect("slotted subjects");
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
        let new_lane_slots =
            plan_next_lane_block_slots(&[new_lane_domain], &[lane_tip(8, 88, 0)], 0)
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
        let known_tips = vec![lane_tip(2, 22, 0), lane_tip_with_descriptor(1, 11, 7, 0x71)];
        let candidate_hashes = vec![tx_hash(0xA0), tx_hash(0xA1), tx_hash(0xA2)];

        let plan = plan_lane_payload(
            &domains,
            &known_tips,
            &candidate_hashes,
            99,
            &BTreeMap::new(),
            5,
        )
        .expect("lane plan");

        assert_eq!(
            plan.lane_tips,
            vec![
                LaneBlockTip {
                    lane_id: LaneId::new(1),
                    dataspace_id: DataSpaceId::new(11),
                    latest_lane_block_height: 7,
                    latest_lane_block_descriptor_hash: Some(Hash::prehashed([0x71; Hash::LENGTH])),
                },
                LaneBlockTip {
                    lane_id: LaneId::new(2),
                    dataspace_id: DataSpaceId::new(22),
                    latest_lane_block_height: 0,
                    latest_lane_block_descriptor_hash: None,
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
            plan.entries
                .iter()
                .map(|entry| (
                    entry.domain.lane_id,
                    entry.tip.latest_lane_block_height,
                    entry.slot.lane_block_height,
                    entry.subject.subject_hash,
                    entry.ownership.subject_hash,
                    entry.accepted_transaction_hashes.clone(),
                ))
                .collect::<Vec<_>>(),
            vec![
                (
                    LaneId::new(1),
                    7,
                    8,
                    plan.subjects[0].subject_hash,
                    plan.ownerships[0].subject_hash,
                    vec![candidate_hashes[2], candidate_hashes[0]],
                ),
                (
                    LaneId::new(2),
                    0,
                    1,
                    plan.subjects[1].subject_hash,
                    plan.ownerships[1].subject_hash,
                    vec![candidate_hashes[1]],
                ),
            ],
            "standalone lane descriptors must group matching tip, slot, subject, ownership, and transaction hashes"
        );
        assert_eq!(
            plan.lane_block_proposals,
            plan.entries
                .iter()
                .map(|entry| entry.lane_block_proposal.clone())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            plan.lane_block_proposal_artifacts,
            plan.entries
                .iter()
                .map(|entry| entry.lane_block_proposal.artifact.clone())
                .collect::<Vec<_>>()
        );
        assert_eq!(plan.lane_block_prepare_vote_plans.len(), plan.entries.len());
        assert_eq!(plan.lane_block_commit_vote_plans.len(), plan.entries.len());
        assert_eq!(
            plan.entries[0].domain.validator_set,
            domains[0].validator_set
        );
        assert_eq!(plan.entries[0].subject, plan.subjects[0]);
        assert_eq!(plan.entries[0].ownership, plan.ownerships[0]);
        let first_descriptor = &plan.entries[0].block_descriptor;
        assert_eq!(first_descriptor.lane_id, LaneId::new(1));
        assert_eq!(first_descriptor.dataspace_id, DataSpaceId::new(11));
        assert_eq!(first_descriptor.previous_lane_block_height, 7);
        assert_eq!(
            first_descriptor.previous_lane_block_descriptor_hash,
            Some(Hash::prehashed([0x71; Hash::LENGTH]))
        );
        assert_eq!(first_descriptor.lane_block_height, 8);
        assert_eq!(first_descriptor.lane_block_view, 5);
        assert_eq!(first_descriptor.subject_hash, plan.subjects[0].subject_hash);
        assert_eq!(
            first_descriptor.payload_ownership_hash,
            plan.ownerships[0].payload_ownership_hash
        );
        assert_eq!(
            first_descriptor.rbc_instance_hash,
            plan.ownerships[0].rbc_instance_hash
        );
        assert_eq!(first_descriptor.accepted_candidate_indices, vec![2, 0]);
        assert_eq!(
            first_descriptor.accepted_transaction_hashes,
            vec![candidate_hashes[2], candidate_hashes[0]]
        );
        assert_eq!(first_descriptor.validator_set, domains[0].validator_set);
        assert_eq!(first_descriptor.quorum, domains[0].quorum);
        assert_eq!(first_descriptor.qc_mode_tag, domains[0].qc_mode_tag);
        let first_proposal = &plan.entries[0].lane_block_proposal;
        assert_eq!(&first_proposal.block_descriptor, first_descriptor);
        assert_eq!(first_proposal.subject, plan.subjects[0]);
        assert_eq!(first_proposal.ownership, plan.ownerships[0]);
        assert_eq!(
            first_proposal.artifact.descriptor.descriptor_hash,
            first_descriptor.descriptor_hash
        );
        assert_eq!(
            first_proposal.artifact.computed_proposal_hash(),
            first_proposal.proposal_hash
        );
        assert_ne!(
            first_proposal.proposal_hash,
            first_descriptor.descriptor_hash
        );
        assert_ne!(first_proposal.proposal_hash, first_descriptor.subject_hash);
        assert_ne!(
            first_descriptor.descriptor_hash,
            first_descriptor.subject_hash
        );
        let first_prepare_votes = &plan.lane_block_prepare_vote_plans[0];
        let first_commit_votes = &plan.lane_block_commit_vote_plans[0];
        assert_eq!(first_prepare_votes.phase, CertPhase::Prepare);
        assert_eq!(first_commit_votes.phase, CertPhase::Commit);
        assert_eq!(
            first_prepare_votes.proposal_hash,
            first_proposal.proposal_hash
        );
        assert_eq!(
            first_commit_votes.proposal_hash,
            first_proposal.proposal_hash
        );
        assert_eq!(
            first_prepare_votes.descriptor_hash,
            first_descriptor.descriptor_hash
        );
        assert_eq!(
            first_prepare_votes.votes.len(),
            first_descriptor.validator_set.len()
        );
        assert_eq!(
            first_prepare_votes.votes[0].body,
            first_proposal.artifact.vote_body(CertPhase::Prepare)
        );
        assert_eq!(
            first_commit_votes.votes[0].body,
            first_proposal.artifact.vote_body(CertPhase::Commit)
        );
        assert_eq!(
            first_prepare_votes
                .votes
                .iter()
                .map(|vote| vote.signer_index)
                .collect::<Vec<_>>(),
            vec![0, 1, 2],
            "full-committee vote templates should follow canonical signer order"
        );
        assert!(
            first_prepare_votes
                .votes
                .windows(2)
                .all(|votes| votes[0].signing_hash == votes[1].signing_hash),
            "prepare vote signing hash must be common across signers"
        );
        assert_ne!(
            first_prepare_votes.votes[0].signing_hash, first_commit_votes.votes[0].signing_hash,
            "prepare and commit vote templates must not share a signable digest"
        );
        assert_eq!(
            plan.subjects[0].subject_hash,
            plan.ownerships[0].subject_hash
        );
        assert_ne!(
            plan.ownerships[0].payload_ownership_hash,
            plan.ownerships[0].rbc_instance_hash
        );

        for entry in &plan.entries {
            let ownership = &entry.ownership;
            let wire_ownership = SumeragiLanePayloadOwnership {
                proposal_height: 10,
                proposal_view: entry.slot.lane_block_view,
                lane_id: ownership.lane_id,
                dataspace_id: ownership.dataspace_id,
                lane_block_height: ownership.lane_block_height,
                lane_block_view: ownership.lane_block_view,
                subject_hash: ownership.subject_hash,
                qc_mode_tag: ownership.qc_mode_tag.clone(),
                accepted_candidate_indices: ownership
                    .accepted_candidate_indices
                    .iter()
                    .map(|index| u64::try_from(*index).expect("candidate index fits u64"))
                    .collect(),
                accepted_transaction_hashes: ownership.accepted_transaction_hashes.clone(),
                previous_lane_block_height: entry.block_descriptor.previous_lane_block_height,
                previous_lane_block_descriptor_hash: entry
                    .block_descriptor
                    .previous_lane_block_descriptor_hash,
                lane_block_descriptor_hash: Some(entry.block_descriptor.descriptor_hash),
                lane_block_descriptor_validator_set: entry.block_descriptor.validator_set.clone(),
                lane_block_descriptor_validator_count: entry
                    .block_descriptor
                    .quorum
                    .validator_count,
                lane_block_descriptor_min_quorum: entry.block_descriptor.quorum.min_quorum,
                payload_ownership_hash: ownership.payload_ownership_hash,
                rbc_instance_hash: ownership.rbc_instance_hash,
            };

            wire_ownership
                .validate_replay_material()
                .expect("scheduler wire ownership should validate replay material");
        }
    }

    #[test]
    fn lane_block_descriptor_binds_committee_without_changing_payload_identity() {
        let routing = routing_for_lane_dataspaces(&[(1, 11)]);
        let candidate_hashes = vec![tx_hash(0xA9)];
        let known_tips = vec![lane_tip(1, 11, 3)];
        let domains_a = plan_lane_consensus_domains(
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
        .expect("lane consensus domain");
        let domains_b = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[0]),
            &[committee(
                1,
                11,
                vec![test_peer(4), test_peer(5), test_peer(6)],
                None,
            )],
            "permissioned",
        )
        .expect("lane consensus domain");

        let plan_a = plan_lane_payload(
            &domains_a,
            &known_tips,
            &candidate_hashes,
            99,
            &BTreeMap::new(),
            2,
        )
        .expect("lane plan with first committee");
        let plan_b = plan_lane_payload(
            &domains_b,
            &known_tips,
            &candidate_hashes,
            99,
            &BTreeMap::new(),
            2,
        )
        .expect("lane plan with second committee");

        assert_eq!(
            plan_a.entries[0].subject.subject_hash, plan_b.entries[0].subject.subject_hash,
            "lane-local payload identity does not include committee membership"
        );
        assert_eq!(
            plan_a.entries[0].ownership.payload_ownership_hash,
            plan_b.entries[0].ownership.payload_ownership_hash
        );
        assert_ne!(
            plan_a.entries[0].block_descriptor.validator_set,
            plan_b.entries[0].block_descriptor.validator_set
        );
        assert_ne!(
            plan_a.entries[0].block_descriptor.descriptor_hash,
            plan_b.entries[0].block_descriptor.descriptor_hash,
            "standalone descriptor must bind the lane-local voting committee"
        );
        assert_ne!(
            plan_a.entries[0].lane_block_proposal.proposal_hash,
            plan_b.entries[0].lane_block_proposal.proposal_hash,
            "standalone proposal identity must bind the voting committee through the descriptor"
        );
    }

    #[test]
    fn lane_block_descriptor_binds_predecessor_descriptor_without_changing_payload_identity() {
        let routing = routing_for_lane_dataspaces(&[(1, 11)]);
        let candidate_hashes = vec![tx_hash(0xAA)];
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
        .expect("lane consensus domain");

        let plan_a = plan_lane_payload(
            &domains,
            &[lane_tip_with_descriptor(1, 11, 3, 0xA1)],
            &candidate_hashes,
            99,
            &BTreeMap::new(),
            2,
        )
        .expect("lane plan with first predecessor descriptor");
        let plan_b = plan_lane_payload(
            &domains,
            &[lane_tip_with_descriptor(1, 11, 3, 0xA2)],
            &candidate_hashes,
            99,
            &BTreeMap::new(),
            2,
        )
        .expect("lane plan with second predecessor descriptor");

        assert_eq!(
            plan_a.entries[0].subject.subject_hash, plan_b.entries[0].subject.subject_hash,
            "lane-local payload identity is independent of predecessor descriptor material"
        );
        assert_eq!(
            plan_a.entries[0].ownership.payload_ownership_hash,
            plan_b.entries[0].ownership.payload_ownership_hash
        );
        assert_ne!(
            plan_a.entries[0]
                .block_descriptor
                .previous_lane_block_descriptor_hash,
            plan_b.entries[0]
                .block_descriptor
                .previous_lane_block_descriptor_hash
        );
        assert_ne!(
            plan_a.entries[0].block_descriptor.descriptor_hash,
            plan_b.entries[0].block_descriptor.descriptor_hash,
            "standalone descriptor must bind the predecessor lane descriptor"
        );
        assert_ne!(
            plan_a.entries[0].lane_block_proposal.proposal_hash,
            plan_b.entries[0].lane_block_proposal.proposal_hash,
            "standalone proposal identity must bind predecessor descriptor lineage"
        );
    }

    #[test]
    fn lane_payload_plan_entries_reject_internal_stage_drift() {
        let routing = routing_for_lane_dataspaces(&[(1, 11)]);
        let validators = vec![test_peer(1), test_peer(2), test_peer(3)];
        let domains = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[0]),
            &[committee(1, 11, validators, None)],
            "permissioned",
        )
        .expect("lane consensus domain");
        let candidate_hashes = vec![tx_hash(0xB0)];
        let plan = plan_lane_payload(&domains, &[], &candidate_hashes, 4, &BTreeMap::new(), 2)
            .expect("lane plan");
        let mut tampered_ownerships = plan.ownerships.clone();
        tampered_ownerships[0].accepted_candidate_indices.push(99);

        let err = build_lane_payload_plan_entries(
            &domains,
            &plan.lane_tips,
            &plan.slots,
            &plan.subjects,
            &tampered_ownerships,
            &candidate_hashes,
        )
        .expect_err("entry builder must reject mismatched ownership descriptors");

        assert_eq!(
            err,
            LanePayloadPlanError::InconsistentEntry {
                lane_id: LaneId::new(1)
            }
        );
    }

    #[test]
    fn lane_block_proposal_rejects_descriptor_subject_drift() {
        let routing = routing_for_lane_dataspaces(&[(1, 11)]);
        let validators = vec![test_peer(1), test_peer(2), test_peer(3)];
        let domains = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[0]),
            &[committee(1, 11, validators, None)],
            "permissioned",
        )
        .expect("lane consensus domain");
        let candidate_hashes = vec![tx_hash(0xB1)];
        let plan = plan_lane_payload(&domains, &[], &candidate_hashes, 4, &BTreeMap::new(), 2)
            .expect("lane plan");
        let mut descriptor = plan.entries[0].block_descriptor.clone();
        descriptor.subject_hash = Hash::prehashed([0xE1; Hash::LENGTH]);

        let err = build_lane_block_proposal(
            descriptor.lane_id,
            &descriptor,
            &plan.entries[0].subject,
            &plan.entries[0].ownership,
        )
        .expect_err("proposal builder must reject descriptor/subject drift");

        assert_eq!(
            err,
            LanePayloadPlanError::InconsistentEntry {
                lane_id: LaneId::new(1)
            }
        );
    }

    #[test]
    fn lane_block_vote_plan_sorts_signers_and_uses_common_signing_hash() {
        let proposal = lane_block_proposal_with_committee(
            vec![test_peer(3), test_peer(1), test_peer(4), test_peer(2)],
            Some(3),
        );
        let validators = proposal.block_descriptor.validator_set.clone();
        let vote_plan = plan_lane_block_vote_quorum(
            &proposal,
            CertPhase::Prepare,
            &[
                validators[2].clone(),
                validators[0].clone(),
                validators[1].clone(),
            ],
        )
        .expect("prepare vote quorum");

        assert_eq!(vote_plan.phase, CertPhase::Prepare);
        assert_eq!(vote_plan.proposal_hash, proposal.proposal_hash);
        assert_eq!(
            vote_plan.descriptor_hash,
            proposal.block_descriptor.descriptor_hash
        );
        assert_eq!(vote_plan.min_quorum, 3);
        assert_eq!(
            vote_plan
                .votes
                .iter()
                .map(|vote| vote.signer_index)
                .collect::<Vec<_>>(),
            vec![0, 1, 2],
            "votes must be sorted by descriptor signer index, not input order"
        );
        assert_eq!(
            vote_plan.votes[0].signing_hash,
            vote_plan.votes[1].signing_hash
        );
        assert_eq!(
            vote_plan.votes[0].validator_set_hash,
            vote_plan.validator_set_hash
        );
        assert_eq!(vote_plan.votes[0].lane_id, LaneId::new(1));
        assert_eq!(vote_plan.votes[0].dataspace_id, DataSpaceId::new(11));
        assert_eq!(vote_plan.votes[0].lane_block_height, 4);
        assert_eq!(vote_plan.votes[0].lane_block_view, 2);

        let single_vote = plan_lane_block_vote(&proposal, CertPhase::Prepare, &validators[1])
            .expect("single lane vote");
        assert_eq!(
            single_vote.signing_hash, vote_plan.votes[0].signing_hash,
            "signer-local transport fields must stay outside the signable digest"
        );

        let commit_votes = plan_lane_block_votes(
            &proposal,
            CertPhase::Commit,
            &[validators[0].clone(), validators[1].clone()],
        )
        .expect("commit votes");
        assert_ne!(
            commit_votes[0].signing_hash, vote_plan.votes[0].signing_hash,
            "prepare and commit votes must be domain-separated"
        );
    }

    #[test]
    fn lane_block_vote_plan_rejects_invalid_phase_and_under_quorum() {
        let proposal = lane_block_proposal_with_committee(
            vec![test_peer(1), test_peer(2), test_peer(3)],
            Some(3),
        );
        let validators = proposal.block_descriptor.validator_set.clone();

        assert_eq!(
            plan_lane_block_vote(&proposal, CertPhase::NewView, &validators[0]),
            Err(LaneBlockVotePlanError::InvalidPhase {
                phase: CertPhase::NewView,
            })
        );
        assert_eq!(
            plan_lane_block_vote_quorum(
                &proposal,
                CertPhase::Prepare,
                std::slice::from_ref(&validators[0]),
            ),
            Err(LaneBlockVotePlanError::InsufficientVoteQuorum {
                lane_id: LaneId::new(1),
                observed: 1,
                min_quorum: 3,
            })
        );
    }

    #[test]
    fn lane_block_vote_plan_rejects_noncanonical_descriptor_quorum() {
        for min_quorum in [2, 4] {
            let mut proposal = lane_block_proposal_with_committee(
                vec![test_peer(1), test_peer(2), test_peer(3), test_peer(4)],
                Some(3),
            );
            proposal.block_descriptor.quorum.min_quorum = min_quorum;
            refresh_lane_block_proposal_hashes(&mut proposal);
            let signer = proposal.block_descriptor.validator_set[0].clone();

            assert_eq!(
                plan_lane_block_vote(&proposal, CertPhase::Prepare, &signer),
                Err(LaneBlockVotePlanError::InvalidQuorum {
                    lane_id: LaneId::new(1),
                    validator_count: 4,
                    min_quorum,
                }),
                "lane validators must not sign descriptors whose quorum diverges from canonical 3-of-4"
            );
        }
    }

    #[test]
    fn lane_block_vote_plan_rejects_duplicate_and_unknown_signers() {
        let proposal = lane_block_proposal_with_committee(
            vec![test_peer(1), test_peer(2), test_peer(3)],
            Some(3),
        );
        let validators = proposal.block_descriptor.validator_set.clone();

        assert_eq!(
            plan_lane_block_votes(
                &proposal,
                CertPhase::Prepare,
                &[validators[0].clone(), validators[0].clone()],
            ),
            Err(LaneBlockVotePlanError::DuplicateSigner {
                lane_id: LaneId::new(1),
            })
        );
        assert_eq!(
            plan_lane_block_vote(&proposal, CertPhase::Prepare, &test_peer(99)),
            Err(LaneBlockVotePlanError::SignerNotInCommittee {
                lane_id: LaneId::new(1),
            })
        );
    }

    #[test]
    fn lane_block_vote_plan_rejects_tampered_descriptor_and_proposal_hashes() {
        let proposal = lane_block_proposal_with_committee(
            vec![test_peer(1), test_peer(2), test_peer(3)],
            Some(3),
        );
        let signer = proposal.block_descriptor.validator_set[0].clone();

        let mut descriptor_tampered = proposal.clone();
        let actual_descriptor = Hash::prehashed([0xD1; Hash::LENGTH]);
        descriptor_tampered.block_descriptor.descriptor_hash = actual_descriptor;
        let descriptor_err =
            plan_lane_block_vote(&descriptor_tampered, CertPhase::Prepare, &signer)
                .expect_err("descriptor hash drift must be rejected");
        assert!(matches!(
            descriptor_err,
            LaneBlockVotePlanError::DescriptorHashMismatch {
                lane_id,
                actual,
                ..
            } if lane_id == LaneId::new(1) && actual == actual_descriptor
        ));

        let mut proposal_tampered = proposal;
        let actual_proposal = Hash::prehashed([0xD2; Hash::LENGTH]);
        proposal_tampered.proposal_hash = actual_proposal;
        let proposal_err = plan_lane_block_vote(&proposal_tampered, CertPhase::Prepare, &signer)
            .expect_err("proposal hash drift must be rejected");
        assert!(matches!(
            proposal_err,
            LaneBlockVotePlanError::ProposalHashMismatch {
                lane_id,
                actual,
                ..
            } if lane_id == LaneId::new(1) && actual == actual_proposal
        ));
    }

    #[test]
    fn lane_block_vote_plan_rejects_tampered_public_artifact() {
        let proposal = lane_block_proposal_with_committee(
            vec![test_peer(1), test_peer(2), test_peer(3)],
            Some(3),
        );
        let signer = proposal.block_descriptor.validator_set[0].clone();

        let mut descriptor_artifact_tampered = proposal.clone();
        descriptor_artifact_tampered
            .artifact
            .descriptor
            .validator_set_hash =
            HashOf::from_untyped_unchecked(Hash::prehashed([0xE1; Hash::LENGTH]));
        assert_eq!(
            plan_lane_block_vote(&descriptor_artifact_tampered, CertPhase::Prepare, &signer),
            Err(LaneBlockVotePlanError::InconsistentProposal {
                lane_id: LaneId::new(1),
            })
        );

        let mut proposal_artifact_tampered = proposal;
        let actual = Hash::prehashed([0xE2; Hash::LENGTH]);
        proposal_artifact_tampered.artifact.proposal_hash = actual;
        assert_eq!(
            plan_lane_block_vote(&proposal_artifact_tampered, CertPhase::Prepare, &signer),
            Err(LaneBlockVotePlanError::ProposalHashMismatch {
                lane_id: LaneId::new(1),
                expected: proposal_artifact_tampered.proposal_hash,
                actual,
            })
        );
    }

    #[test]
    fn lane_block_vote_plan_rejects_noncanonical_descriptor_validator_set() {
        let mut proposal = lane_block_proposal_with_committee(
            vec![test_peer(1), test_peer(2), test_peer(3)],
            Some(3),
        );
        let signer = proposal.block_descriptor.validator_set[0].clone();
        proposal.block_descriptor.validator_set.swap(0, 1);

        assert_eq!(
            plan_lane_block_vote(&proposal, CertPhase::Prepare, &signer),
            Err(LaneBlockVotePlanError::ValidatorSetNotCanonical {
                lane_id: LaneId::new(1),
            })
        );
    }

    #[test]
    fn lane_payload_plan_rejects_missing_candidate_hash_for_accepted_index() {
        let routing = routing_for_lane_dataspaces(&[(1, 11), (1, 11)]);
        let validators = vec![test_peer(1), test_peer(2), test_peer(3)];
        let domains = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[1]),
            &[committee(1, 11, validators, None)],
            "permissioned",
        )
        .expect("lane consensus domain");

        let err = plan_lane_payload(&domains, &[], &[tx_hash(0xC0)], 4, &BTreeMap::new(), 2)
            .expect_err("accepted candidate without transaction hash must fail closed");

        assert_eq!(
            err,
            LanePayloadPlanError::Subjects(LaneBlockSubjectError::CandidateHashIndexOutOfBounds {
                lane_id: LaneId::new(1),
                index: 1,
                candidate_hashes: 1,
            })
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
            &[lane_tip(1, 99, 4)],
            &[tx_hash(0xD0)],
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
            lane_tip(1, 11, 4),
            lane_tip(7, 77, 99),
            lane_tip_with_descriptor(1, 11, 8, 0x81),
            lane_tip(3, 33, 0),
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
                lane_tip_with_descriptor(1, 11, 8, 0x81),
                lane_tip(2, 22, 41),
                lane_tip(3, 33, 0),
            ],
            "tip reducer should keep the latest active-lane tip, ignore idle-lane tips, and fill compatibility tips"
        );
    }

    #[test]
    fn latest_lane_block_tips_reject_conflicting_descriptor_hashes_at_same_height() {
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
                &domains,
                &[
                    lane_tip_with_descriptor(1, 11, 8, 0xB1),
                    lane_tip_with_descriptor(1, 11, 8, 0xB2),
                ],
                3,
                &BTreeMap::new(),
            ),
            Err(LaneBlockTipPlanError::ConflictingLaneTipDescriptorHash {
                lane_id: LaneId::new(1),
                latest_lane_block_height: 8,
            }),
            "same-height lane tips with different predecessor descriptors must fail closed"
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
            lane_tip_with_descriptor(1, 11, 4, 0x91),
            lane_tip_with_descriptor(2, 99, 5, 0x92),
            lane_tip_with_descriptor(3, 33, 12, 0x93),
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
                lane_tip(1, 11, 9),
                lane_tip(2, 22, 6),
                lane_tip_with_descriptor(3, 33, 12, 0x93),
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
    fn latest_lane_block_tips_use_reset_watermark_for_missing_recreated_lane_tip() {
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
        let reset_heights = BTreeMap::from([(LaneId::new(1), 9)]);

        let tips =
            plan_latest_lane_block_tips_with_reset_heights(&domains, &[], 41, &reset_heights)
                .expect("reset-aware latest lane block tips");

        assert_eq!(
            tips,
            vec![lane_tip(1, 11, 9), lane_tip(2, 22, 41)],
            "missing tips for reset lanes resume from the reset watermark, while non-reset lanes keep the compatibility baseline"
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
                &[lane_tip(1, 99, 7)],
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
                &[lane_tip(1, 99, 7)],
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
        let tip = lane_tip(1, 11, 7);

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
            plan_lane_block_subjects_for_slots(&domains, &tx_hashes(1), &[]),
            Err(LaneBlockSubjectError::MissingLaneSlot {
                lane_id: LaneId::new(1),
            })
        );

        assert_eq!(
            plan_lane_block_subjects_for_slots(&domains, &tx_hashes(1), &[slot, slot]),
            Err(LaneBlockSubjectError::DuplicateLaneSlot {
                lane_id: LaneId::new(1),
            })
        );

        let mismatched_dataspace = LaneBlockSlot {
            dataspace_id: DataSpaceId::new(99),
            ..slot
        };
        assert_eq!(
            plan_lane_block_subjects_for_slots(&domains, &tx_hashes(1), &[mismatched_dataspace]),
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
            plan_lane_block_subjects_for_slots(&domains, &tx_hashes(1), &[slot, unexpected]),
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
        let candidate_hashes = tx_hashes(4);
        let subjects = plan_lane_block_subjects(&domains, &candidate_hashes, 42, 7)
            .expect("lane block subjects");

        let ownerships = plan_lane_payload_ownership(&subjects).expect("lane payload ownership");

        assert_eq!(ownerships.len(), 2);
        assert_eq!(ownerships[0].lane_id, LaneId::new(1));
        assert_eq!(ownerships[0].dataspace_id, DataSpaceId::new(11));
        assert_eq!(ownerships[0].lane_block_height, 42);
        assert_eq!(ownerships[0].lane_block_view, 7);
        assert_eq!(ownerships[0].subject_hash, subjects[0].subject_hash);
        assert_eq!(ownerships[0].accepted_candidate_indices, vec![2, 0]);
        assert_eq!(
            ownerships[0].accepted_transaction_hashes,
            vec![candidate_hashes[2], candidate_hashes[0]]
        );
        assert_ne!(
            ownerships[0].payload_ownership_hash,
            ownerships[0].rbc_instance_hash
        );

        let view_drift_subjects = plan_lane_block_subjects(&domains, &candidate_hashes, 42, 8)
            .expect("lane block subjects with view drift");
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

        let hash_drift_candidate_hashes =
            vec![tx_hash(0xE0), tx_hash(0xE1), tx_hash(0xE2), tx_hash(0xE3)];
        let hash_drift_subjects =
            plan_lane_block_subjects(&domains, &hash_drift_candidate_hashes, 42, 7)
                .expect("hash drift subjects");
        let hash_drift_ownerships =
            plan_lane_payload_ownership(&hash_drift_subjects).expect("hash drift ownership");
        assert_ne!(
            subjects[0].subject_hash,
            hash_drift_subjects[0].subject_hash
        );
        assert_ne!(
            ownerships[0].payload_ownership_hash,
            hash_drift_ownerships[0].payload_ownership_hash
        );
        assert_ne!(
            ownerships[0].rbc_instance_hash,
            hash_drift_ownerships[0].rbc_instance_hash
        );

        let mut reordered_work = domains.clone();
        reordered_work[0].accepted_candidate_indices.reverse();
        let reordered_subjects =
            plan_lane_block_subjects(&reordered_work, &candidate_hashes, 42, 7)
                .expect("reordered subjects");
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
        let subjects =
            plan_lane_block_subjects(&domains, &tx_hashes(2), 3, 4).expect("lane block subjects");
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
        let subjects =
            plan_lane_block_subjects(&domains, &tx_hashes(1), 9, 2).expect("lane block subjects");
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
        malformed.accepted_transaction_hashes.clear();
        assert_eq!(
            plan_lane_payload_ownership(&[malformed.clone()]),
            Err(LanePayloadOwnershipError::CandidateHashCountMismatch {
                lane_id: LaneId::new(1),
                candidate_indices: 1,
                candidate_hashes: 0,
            })
        );

        malformed = subjects[0].clone();
        malformed.accepted_candidate_indices.push(0);
        malformed.accepted_transaction_hashes.push(tx_hash(0xF0));
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
            plan_lane_block_subjects(&[malformed.clone()], &tx_hashes(1), 1, 0),
            Err(LaneBlockSubjectError::BlankQcModeTag {
                lane_id: LaneId::new(1),
            })
        );

        malformed = domains[0].clone();
        malformed.accepted_candidate_indices.clear();
        malformed.accepted_candidates = 0;
        assert_eq!(
            plan_lane_block_subjects(&[malformed.clone()], &tx_hashes(1), 1, 0),
            Err(LaneBlockSubjectError::EmptyCandidateSet {
                lane_id: LaneId::new(1),
            })
        );

        malformed = domains[0].clone();
        malformed.accepted_candidates = 2;
        assert_eq!(
            plan_lane_block_subjects(&[malformed.clone()], &tx_hashes(1), 1, 0),
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
            plan_lane_block_subjects(&[malformed.clone()], &tx_hashes(1), 1, 0),
            Err(LaneBlockSubjectError::DuplicateCandidateIndex {
                lane_id: LaneId::new(1),
                index: 0,
            })
        );

        assert_eq!(
            plan_lane_block_subjects(&[domains[0].clone()], &[], 1, 0),
            Err(LaneBlockSubjectError::CandidateHashIndexOutOfBounds {
                lane_id: LaneId::new(1),
                index: 0,
                candidate_hashes: 0,
            })
        );

        assert_eq!(
            plan_lane_block_subjects(
                &[domains[0].clone(), domains[0].clone()],
                &tx_hashes(1),
                1,
                0
            ),
            Err(LaneBlockSubjectError::DuplicateLaneDomain {
                lane_id: LaneId::new(1),
            })
        );
    }

    #[test]
    fn lane_consensus_domains_use_explicit_quorum() {
        let routing = routing_for_lane_dataspaces(&[(7, 70)]);
        let validators = vec![test_peer(1), test_peer(2), test_peer(3), test_peer(4)];

        let domains = plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[0]),
            &[committee(7, 70, validators, Some(3))],
            "npos",
        )
        .expect("lane consensus domains");

        assert_eq!(domains[0].quorum.validator_count, 4);
        assert_eq!(domains[0].quorum.min_quorum, 3);
        assert_eq!(
            domains[0].qc_mode_tag,
            "npos::lane-relay:v1:70:7".to_string()
        );
    }

    #[test]
    fn lane_consensus_domains_reject_noncanonical_explicit_quorum() {
        let routing = routing_for_lane_dataspaces(&[(7, 70)]);
        let validators = vec![test_peer(1), test_peer(2), test_peer(3), test_peer(4)];

        for min_quorum in [1, 2, 4] {
            assert_eq!(
                plan_lane_consensus_domains(
                    &routing,
                    &accepted_schedule(&[0]),
                    &[committee(7, 70, validators.clone(), Some(min_quorum))],
                    "npos",
                ),
                Err(LaneConsensusDomainError::InvalidQuorum {
                    lane_id: LaneId::new(7),
                    validator_count: 4,
                    min_quorum,
                }),
                "four-validator lane committees must use canonical 3-of-4 commit quorum"
            );
        }
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
