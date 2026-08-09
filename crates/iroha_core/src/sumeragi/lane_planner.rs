//! Deterministic lane-local proposal scheduling and payload ownership planning.
//!
//! This module is independent of the global consensus reducer. Sumeragi v2
//! invokes it only to derive bounded lane-local artifacts which become inputs
//! to the authoritative reducer-owned block candidate.

use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};

#[cfg(test)]
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use crate::queue::{LaneQueueReservationScopeV1, RoutingDecision};
use iroha_config::parameters::actual::Nexus;
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    block::consensus::{
        CertPhase, LaneBlockDescriptorV1, LaneBlockProposalV1, LaneBlockVoteBodyV1,
        SumeragiLanePayloadOwnership,
    },
    block::consensus_v2 as wire,
    consensus::VALIDATOR_SET_HASH_VERSION_V1,
    nexus::{DataSpaceId, LaneId, LaneRelayEnvelope, LaneRelayQuorumContext},
    peer::PeerId,
};
use norito::codec::Encode;
use thiserror::Error;

use crate::{kura::Kura, state::State};

/// Resolve an autoscaled lane's immutable, incarnation-bound PoPs in exact
/// validator-set order.
///
/// `Some(None)` identifies an operator-managed lane, whose live-roster policy
/// remains applicable. `Some(Some(_))` is the exact pinned autoscale vector.
/// `None` means a missing or malformed pin, an absent lane, or a validator-set
/// mismatch and therefore fails closed.
pub(in crate::sumeragi) fn pinned_autoscale_validator_pops_for_set(
    state: &State,
    lane_id: LaneId,
    validator_set: &[PeerId],
) -> Option<Option<Vec<Vec<u8>>>> {
    let nexus = state.nexus_snapshot();
    let lane = nexus
        .lane_catalog
        .lanes()
        .iter()
        .find(|lane| lane.id == lane_id)?;
    if !lane.claims_autoscale_managed() {
        return Some(None);
    }
    let pinned = crate::state::autoscale_lane_pinned_committee_with_pops(lane)?;
    align_exact_pinned_validator_pops(pinned, validator_set).map(Some)
}

fn align_exact_pinned_validator_pops(
    pinned: Vec<(PeerId, Vec<u8>)>,
    validator_set: &[PeerId],
) -> Option<Vec<Vec<u8>>> {
    if pinned.len() != validator_set.len()
        || pinned
            .iter()
            .zip(validator_set)
            .any(|((pinned_peer, _), validator)| pinned_peer != validator)
    {
        return None;
    }
    Some(pinned.into_iter().map(|(_, pop)| pop).collect())
}

/// Return true when proposal assembly should look beyond the remaining block
/// slots to discover work from other currently routable lanes.
///
/// The gate intentionally uses the same height-aware routing surface as the
/// queue router. Sidecar lanes that no policy path can select, future-created
/// autoscale lanes, malformed autoscale anchors, and disabled Nexus state do not
/// enable broader scans.
#[must_use]
pub(crate) fn proposal_lookahead_enabled(nexus: &Nexus, block_height: u64) -> bool {
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
#[cfg(test)]
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
#[cfg(test)]
pub(super) struct ProposalAdmissionCandidate {
    /// Gas cost charged to the global proposal gas budget.
    pub(super) gas_cost: u64,
    /// Whether this transaction consumes one IVM-heavy proposal slot.
    pub(super) is_ivm_heavy: bool,
}

/// Current proposal resource usage at a candidate admission point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg(test)]
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
#[cfg(test)]
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
#[cfg(test)]
pub(super) enum ProposalAdmissionDecision {
    /// Admit the candidate. `exceeds_gas_limit` is true only for the
    /// oversized-first fallback that avoids proposal stalls.
    Accept { exceeds_gas_limit: bool },
    /// Defer the candidate and requeue it with its current routing plan.
    Defer { reason: ProposalDeferralReason },
}

/// Error returned when proposal batch scheduling inputs are internally inconsistent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg(test)]
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
    #[cfg(test)]
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
    #[cfg(test)]
    pub(super) gas_used_delta: u64,
    /// IVM-heavy transactions accepted from this batch.
    #[cfg(test)]
    pub(super) ivm_transactions_included_delta: usize,
    /// IVM-heavy transactions deferred by this batch.
    #[cfg(test)]
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
    /// Exact active lane incarnation commitment.
    pub(super) lane_incarnation: Hash,
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
    /// Exact active lane incarnation commitment.
    pub(super) lane_incarnation: Hash,
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
    /// Exact active lane incarnation commitment.
    pub(super) lane_incarnation: Hash,
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
    /// Exact active lane incarnation commitment.
    pub(super) lane_incarnation: Hash,
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
    /// Exact active lane incarnation commitment.
    pub(super) lane_incarnation: Hash,
    /// Global proposal height that planned this lane-local block.
    pub(super) proposal_height: u64,
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

/// Stable identity used to pace proposal redrive independently for each lane slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg(test)]
struct LaneBlockRedriveIdentity {
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_incarnation: Hash,
    lane_block_height: u64,
    lane_block_view: u64,
    proposal_hash: Hash,
}

#[cfg(test)]
impl LaneBlockRedriveIdentity {
    fn from_proposal(proposal: &LaneBlockProposalV1) -> Self {
        Self {
            lane_id: proposal.descriptor.lane_id,
            dataspace_id: proposal.descriptor.dataspace_id,
            lane_incarnation: proposal.descriptor.lane_incarnation,
            lane_block_height: proposal.descriptor.lane_block_height,
            lane_block_view: proposal.descriptor.lane_block_view,
            proposal_hash: proposal.proposal_hash,
        }
    }

    fn same_height(self, other: Self) -> bool {
        self.lane_id == other.lane_id
            && self.dataspace_id == other.dataspace_id
            && self.lane_incarnation == other.lane_incarnation
            && self.lane_block_height == other.lane_block_height
    }
}

/// Result of admitting a canonical proposal into the per-lane redrive clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg(test)]
pub(super) enum LaneBlockRedriveObservation {
    /// A new lane height/view started its independent redrive clock.
    Inserted,
    /// The exact proposal was already tracked; its original clock was preserved.
    Duplicate,
    /// A canonical higher view replaced the tracked lower view for this lane height.
    Superseded {
        /// Previously tracked lane view.
        previous_view: u64,
    },
    /// The proposal is older than the canonical view already tracked for this lane height.
    Stale {
        /// Newest tracked lane view.
        current_view: u64,
    },
    /// A different proposal attempted to claim an already tracked lane height/view.
    Conflicting,
    /// The proposal failed canonical stateless validation.
    Invalid,
}

/// Bounded per-lane proposal-redrive scheduler.
///
/// The persisted lane proposal is immutable: its lane view, descriptor, and DA/RBC
/// ownership are all part of the canonical hash. This scheduler therefore rotates
/// only the transport coordinator for that exact artifact. Each timeout advances
/// to the next committee member. After one full committee cycle, every committee
/// member may redrive; that bounded fallback preserves liveness even when peers
/// observed the artifact at different local instants. This allows an available
/// Kura-backed proposal to be recovered when its original producer or the global
/// block leader disappears, without fabricating a new payload identity.
#[derive(Debug)]
#[cfg(test)]
pub(super) struct LaneBlockRedriveTracker {
    capacity: usize,
    observed_at: BTreeMap<LaneBlockRedriveIdentity, Instant>,
    order: VecDeque<LaneBlockRedriveIdentity>,
}

#[cfg(test)]
impl LaneBlockRedriveTracker {
    /// Construct a tracker with a hard bound on retained lane proposal identities.
    #[must_use]
    pub(super) fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            observed_at: BTreeMap::new(),
            order: VecDeque::new(),
        }
    }

    /// Record an internally canonical proposal without resetting duplicate clocks.
    pub(super) fn observe(
        &mut self,
        proposal: &LaneBlockProposalV1,
        now: Instant,
    ) -> LaneBlockRedriveObservation {
        if crate::lane_consensus::validate_lane_block_proposal(proposal).is_err() {
            return LaneBlockRedriveObservation::Invalid;
        }

        let identity = LaneBlockRedriveIdentity::from_proposal(proposal);
        if self.observed_at.contains_key(&identity) {
            return LaneBlockRedriveObservation::Duplicate;
        }

        let current = self
            .observed_at
            .keys()
            .copied()
            .filter(|candidate| candidate.same_height(identity))
            .max_by_key(|candidate| candidate.lane_block_view);
        let observation = match current {
            Some(current) if identity.lane_block_view < current.lane_block_view => {
                return LaneBlockRedriveObservation::Stale {
                    current_view: current.lane_block_view,
                };
            }
            Some(current)
                if identity.lane_block_view == current.lane_block_view
                    && identity.proposal_hash != current.proposal_hash =>
            {
                return LaneBlockRedriveObservation::Conflicting;
            }
            Some(current) => {
                self.observed_at
                    .retain(|candidate, _| !candidate.same_height(identity));
                self.order
                    .retain(|candidate| !candidate.same_height(identity));
                LaneBlockRedriveObservation::Superseded {
                    previous_view: current.lane_block_view,
                }
            }
            None => LaneBlockRedriveObservation::Inserted,
        };

        self.observed_at.insert(identity, now);
        self.order.push_back(identity);
        self.enforce_capacity();
        observation
    }

    /// Return the timeout round for the exact current proposal, if tracked.
    #[must_use]
    pub(super) fn redrive_round(
        &self,
        proposal: &LaneBlockProposalV1,
        now: Instant,
        timeout: Duration,
    ) -> Option<u64> {
        let observed_at = self
            .observed_at
            .get(&LaneBlockRedriveIdentity::from_proposal(proposal))?;
        if timeout.is_zero() {
            return Some(0);
        }
        let elapsed = now.saturating_duration_since(*observed_at).as_nanos();
        let rounds = elapsed / timeout.as_nanos();
        Some(u64::try_from(rounds).unwrap_or(u64::MAX))
    }

    /// Return true when `peer` is the deterministic coordinator for the exact
    /// proposal's current redrive round.
    #[must_use]
    pub(super) fn peer_may_redrive(
        &self,
        proposal: &LaneBlockProposalV1,
        peer: &PeerId,
        now: Instant,
        timeout: Duration,
    ) -> bool {
        let Some(round) = self.redrive_round(proposal, now, timeout) else {
            return false;
        };
        let Ok(validator_count) = u64::try_from(proposal.descriptor.validator_set.len()) else {
            return false;
        };
        if validator_count > 0 && round >= validator_count {
            // Local monotonic time controls transport retries only; it never
            // changes proposal identity, vote validity, or consensus admission.
            // Opening the exact canonical artifact to the full committee after
            // one cycle prevents clock/observation skew from creating a silent
            // schedule in which every peer believes a different peer should send.
            return proposal.descriptor.validator_set.contains(peer);
        }
        lane_block_redrive_leader(proposal, round) == Some(peer)
    }

    fn enforce_capacity(&mut self) {
        while self.observed_at.len() > self.capacity {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            self.observed_at.remove(&oldest);
        }
        self.order
            .retain(|identity| self.observed_at.contains_key(identity));
    }
}

/// Select the deterministic lane proposal transport coordinator for a redrive round.
///
/// The seed deliberately excludes transaction and proposal hashes, preventing a
/// proposer from grinding payload contents to select itself. Lane view and redrive
/// round are additive rotations over the canonical committee: a genuine future
/// view change and a local timeout both move responsibility to the next validator.
#[must_use]
#[cfg(test)]
pub(in crate::sumeragi) fn lane_block_redrive_leader(
    proposal: &LaneBlockProposalV1,
    redrive_round: u64,
) -> Option<&PeerId> {
    if crate::lane_consensus::validate_lane_block_proposal(proposal).is_err() {
        return None;
    }
    let descriptor = &proposal.descriptor;
    lane_block_slot_leader(
        descriptor.lane_id,
        descriptor.dataspace_id,
        descriptor.lane_incarnation,
        descriptor.proposal_height,
        descriptor.previous_lane_block_height,
        descriptor.lane_block_height,
        descriptor.lane_block_view,
        descriptor.validator_set_hash,
        &descriptor.validator_set,
        redrive_round,
    )
}

/// Select the deterministic producer for a lane slot before transaction
/// selection. The seed excludes all candidate and payload hashes, preventing a
/// producer from grinding queue contents to win ownership.
#[allow(clippy::too_many_arguments)]
#[must_use]
#[cfg(test)]
pub(super) fn lane_block_slot_leader<'a>(
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_incarnation: Hash,
    proposal_height: u64,
    previous_lane_block_height: u64,
    lane_block_height: u64,
    lane_block_view: u64,
    validator_set_hash: HashOf<Vec<PeerId>>,
    validator_set: &'a [PeerId],
    redrive_round: u64,
) -> Option<&'a PeerId> {
    let validator_count = u64::try_from(validator_set.len()).ok()?;
    if validator_count == 0 {
        return None;
    }

    let mut seed = Vec::with_capacity(128);
    seed.extend_from_slice(b"iroha:nexus:lane-block-redrive-leader:v1");
    seed.extend_from_slice(&lane_id.as_u32().to_be_bytes());
    seed.extend_from_slice(&dataspace_id.as_u64().to_be_bytes());
    seed.extend_from_slice(lane_incarnation.as_ref());
    seed.extend_from_slice(&proposal_height.to_be_bytes());
    seed.extend_from_slice(&previous_lane_block_height.to_be_bytes());
    seed.extend_from_slice(&lane_block_height.to_be_bytes());
    seed.extend_from_slice(validator_set_hash.as_ref());
    let digest = Hash::new(seed);
    let mut prefix = [0_u8; 8];
    prefix.copy_from_slice(&digest.as_ref()[..8]);
    let base = u64::from_be_bytes(prefix) % validator_count;
    let rotation =
        (lane_block_view % validator_count + redrive_round % validator_count) % validator_count;
    let index = usize::try_from((base + rotation) % validator_count).ok()?;
    validator_set.get(index)
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
    /// Active lane has no non-zero incarnation commitment.
    MissingLaneIncarnation {
        /// Lane missing its active incarnation.
        lane_id: LaneId,
    },
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
#[cfg(test)]
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

#[cfg(test)]
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
#[cfg(test)]
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
#[cfg(test)]
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

/// Convert accepted batch actions for blocked lanes into deferrals while
/// preserving resource counters for candidates that remain accepted.
///
/// Existing deferrals retain their original reason. Accepted work routed to a
/// blocked lane is treated as a lane-consensus deferral before lane payload
/// ownerships are planned, which prevents proposal assembly from extending a
/// lane whose previous certified block has not been applied yet.
#[must_use]
#[cfg(test)]
pub(super) fn defer_accepted_proposal_actions_for_lanes(
    schedule: &ProposalBatchSchedule,
    routing_decisions: &[RoutingDecision],
    candidates: &[ProposalAdmissionCandidate],
    blocked_lanes: &BTreeSet<LaneId>,
    reason: ProposalDeferralReason,
) -> ProposalBatchSchedule {
    if blocked_lanes.is_empty() {
        return schedule.clone();
    }

    let mut deferred = ProposalBatchSchedule {
        actions: Vec::with_capacity(schedule.actions.len()),
        ivm_transactions_deferred: schedule.ivm_transactions_deferred,
        ..ProposalBatchSchedule::default()
    };
    let account_gas = schedule.gas_used_delta > 0;
    for action in &schedule.actions {
        match *action {
            ProposalBatchAction::Accept {
                index,
                exceeds_gas_limit,
            } => {
                let routing = routing_decisions
                    .get(index)
                    .expect("schedule action index must reference a routing decision");
                if blocked_lanes.contains(&routing.lane_id) {
                    deferred
                        .actions
                        .push(ProposalBatchAction::Defer { index, reason });
                    continue;
                }

                let candidate = candidates
                    .get(index)
                    .expect("schedule action index must reference an admission candidate");
                deferred.actions.push(ProposalBatchAction::Accept {
                    index,
                    exceeds_gas_limit,
                });
                if account_gas {
                    deferred.gas_used_delta =
                        deferred.gas_used_delta.saturating_add(candidate.gas_cost);
                }
                if candidate.is_ivm_heavy {
                    deferred.ivm_transactions_included_delta =
                        deferred.ivm_transactions_included_delta.saturating_add(1);
                }
            }
            action @ ProposalBatchAction::Defer { .. } => deferred.actions.push(action),
        }
    }
    deferred
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
/// returns no validators, the frozen shared-domain roster is used only when
/// explicitly supplied by the caller. Enabled multi-lane paths pass `None` so
/// stale or missing lane authority fails closed instead of inheriting global
/// topology accidentally.
pub(super) fn plan_lane_consensus_committees_with_authority<F>(
    routing_decisions: &[RoutingDecision],
    schedule: &ProposalBatchSchedule,
    shared_domain_validators: Option<&[PeerId]>,
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
                let Some(shared_validators) = shared_domain_validators else {
                    return Err(LaneConsensusDomainError::MissingLaneCommittee { lane_id });
                };
                validators = shared_validators.to_vec();
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

/// Reduce known lane-local tips within exact lane-incarnation namespaces.
///
/// Accepted lanes without a known tip start from lane-local height zero. Missing
/// lane-local relay history never inherits global or DA-reset coordinates,
/// because standalone lane-block execution requires contiguous predecessor
/// application receipts.
/// Tips from retired incarnations are ignored, and a recreated lane starts at
/// lane-local height zero regardless of prior DA/global reset coordinates.
pub(super) fn plan_latest_lane_block_tips_with_incarnations(
    domains: &[LaneConsensusDomain],
    known_tips: &[LaneBlockTip],
    lane_incarnations: &BTreeMap<LaneId, Hash>,
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
        let Some(expected_incarnation) = lane_incarnations.get(&tip.lane_id).copied() else {
            return Err(LaneBlockTipPlanError::MissingLaneIncarnation {
                lane_id: tip.lane_id,
            });
        };
        if expected_incarnation.as_ref().iter().all(|byte| *byte == 0) {
            return Err(LaneBlockTipPlanError::MissingLaneIncarnation {
                lane_id: tip.lane_id,
            });
        }
        if tip.lane_incarnation != expected_incarnation {
            // Durable history from a retired incarnation is inert regardless
            // of its local height. The current incarnation starts from zero
            // unless a matching tip is also present.
            continue;
        }
        if tip.dataspace_id != domain.dataspace_id {
            return Err(LaneBlockTipPlanError::LaneTipDataspaceMismatch {
                lane_id: tip.lane_id,
                expected: domain.dataspace_id,
                actual: tip.dataspace_id,
            });
        }
        let floored_tip = *tip;
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
            let lane_incarnation = lane_incarnations
                .get(&lane_id)
                .copied()
                .ok_or(LaneBlockTipPlanError::MissingLaneIncarnation { lane_id })?;
            if lane_incarnation.as_ref().iter().all(|byte| *byte == 0) {
                return Err(LaneBlockTipPlanError::MissingLaneIncarnation { lane_id });
            }
            latest_by_lane.get(&lane_id).copied().map_or_else(
                || {
                    Ok(LaneBlockTip {
                        lane_id,
                        dataspace_id: domain.dataspace_id,
                        lane_incarnation,
                        latest_lane_block_height: 0,
                        latest_lane_block_descriptor_hash: None,
                    })
                },
                Ok,
            )
        })
        .collect::<Result<Vec<_>, _>>()?)
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
            lane_incarnation: tip.lane_incarnation,
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
/// lane QC mode tag into a stable Norito-backed digest. This test helper assigns
/// one uniform slot to every lane so tests can compare it with independently
/// advancing slots.
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
            lane_incarnation: Hash::new(
                [
                    b"lane-subject-test-incarnation:".as_slice(),
                    &domain.lane_id.as_u32().to_be_bytes(),
                    &domain.dataspace_id.as_u64().to_be_bytes(),
                ]
                .concat(),
            ),
            lane_block_height,
            lane_block_view,
        })
        .collect::<Vec<_>>();
    plan_lane_block_subjects_for_slots(domains, candidate_hashes, &slots)
}

/// Derive deterministic lane block subjects from explicit lane-local slots.
///
/// Unlike [`plan_lane_block_subjects`], this accepts independent height/view
/// coordinates per lane. The live v2 planner uses this to advance lanes at
/// different rates while preserving one canonical digest and validation rule.
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
            slot.lane_incarnation,
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
            lane_incarnation: slot.lane_incarnation,
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
            subject.lane_incarnation,
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
                subject.lane_incarnation,
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
            subject.lane_incarnation,
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
            lane_incarnation: subject.lane_incarnation,
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
/// Known committed tips are reduced inside exact lane-incarnation namespaces,
/// next lane-local slots are assigned, canonical lane block subjects are
/// derived, and DA/RBC ownership identities are validated before being returned
/// together.
pub(super) fn plan_lane_payload_with_incarnations(
    domains: &[LaneConsensusDomain],
    known_tips: &[LaneBlockTip],
    candidate_hashes: &[Hash],
    lane_incarnations: &BTreeMap<LaneId, Hash>,
    proposal_height: u64,
    lane_block_view: u64,
) -> Result<LanePayloadPlan, LanePayloadPlanError> {
    let lane_tips =
        plan_latest_lane_block_tips_with_incarnations(domains, known_tips, lane_incarnations)
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
        proposal_height,
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

/// Bounded lane-local plan returned to the Sumeragi v2 candidate adapter.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct V2LanePayloadPlan {
    /// Replayable ownership commitments covering every available candidate.
    pub(crate) ownerships: Vec<SumeragiLanePayloadOwnership>,
    /// Standalone lane-local proposals corresponding to `ownerships`.
    pub(crate) proposals: Vec<LaneBlockProposalV1>,
    /// Candidate indices whose lane authority or predecessor is unavailable.
    pub(crate) unavailable_indices: BTreeSet<usize>,
}

/// Transaction-independent ownership coordinates for the next autonomous lane slot.
///
/// This plan is derived only from committed state and one frozen global height
/// context. Queue selection happens after the plan is fixed, so neither the
/// rotating author nor either reservation identity can be ground by changing
/// transaction contents.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AutonomousLaneReservationSlotPlan {
    /// Exact frozen height-context identity used to derive both reservation hashes.
    pub(crate) height_context_id: wire::HeightContextId,
    /// Lane allowed to reserve the queued work.
    pub(crate) lane_id: LaneId,
    /// Dataspace bound to the active lane route.
    pub(crate) dataspace_id: DataSpaceId,
    /// Exact active incarnation at `proposal_height`.
    pub(crate) lane_incarnation: Hash,
    /// Frozen global height that authorizes this reservation.
    pub(crate) proposal_height: u64,
    /// Latest durably applied lane-local height.
    pub(crate) previous_lane_block_height: u64,
    /// Exact descriptor hash of the latest applied lane-local block.
    pub(crate) previous_lane_block_descriptor_hash: Option<Hash>,
    /// Contiguous lane-local height reserved for the next batch.
    pub(crate) lane_block_height: u64,
    /// Fresh autonomous slots always begin at lane view zero.
    pub(crate) lane_block_view: u64,
    /// Canonically ordered frozen committee.
    pub(crate) validator_set: Vec<PeerId>,
    /// Hash of the exact canonical validator order.
    pub(crate) validator_set_hash: HashOf<Vec<PeerId>>,
    /// Canonical quorum for `validator_set`.
    pub(crate) quorum: LaneRelayQuorumContext,
    /// Height-context-separated lane QC domain.
    pub(crate) qc_mode_tag: String,
    /// Height-rotated deterministic producer for this slot.
    pub(crate) author: PeerId,
    /// Stable identity of the author/session taking queue ownership.
    pub(crate) reservation_owner_hash: Hash,
    /// Stable provisional slot identity, independent of selected transactions.
    pub(crate) proposal_identity_hash: Hash,
}

/// Move-only authority for the QueuePlan-conjunction and reservation-fsync
/// production trace steps of one canonical autonomous slot.
///
/// Production code can obtain this value only from a fully assembled slot
/// plan. Keeping the committee geometry beside the exact queue scope prevents
/// Queue from accepting an unbound validator count or proposer bit merely to
/// manufacture a formal projection.
#[allow(missing_copy_implementations)]
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct AutonomousLaneReservationSelectionAuthorization {
    scope: LaneQueueReservationScopeV1,
    height_context_id: wire::HeightContextId,
    validator_count: u8,
    producer: u128,
}

impl AutonomousLaneReservationSelectionAuthorization {
    /// Return the exact queue scope frozen by the canonical slot plan.
    #[must_use]
    pub(crate) const fn scope(&self) -> LaneQueueReservationScopeV1 {
        self.scope
    }

    /// Return the frozen height-context identity which committed the exact
    /// predecessor, committee, quorum, QC domain, and producer into the slot's
    /// reservation hashes.
    #[must_use]
    pub(crate) const fn height_context_id(&self) -> wire::HeightContextId {
        self.height_context_id
    }

    /// Return the canonical committee width represented by the producer bit.
    #[must_use]
    pub(crate) const fn validator_count(&self) -> u8 {
        self.validator_count
    }

    /// Return the one-hot index of the deterministic producer.
    #[must_use]
    pub(crate) const fn producer(&self) -> u128 {
        self.producer
    }

    #[cfg(test)]
    pub(crate) fn single_validator_for_test(scope: LaneQueueReservationScopeV1) -> Self {
        Self {
            scope,
            height_context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                b"iroha:test:single-validator-reservation-height-context:v1",
            ))),
            validator_count: 1,
            producer: 1,
        }
    }
}

impl AutonomousLaneReservationSlotPlan {
    /// Derive the exact committee geometry consumed by Queue's first-release
    /// selection/refinement gate.
    ///
    /// # Errors
    /// Returns [`AutonomousLaneReservationSlotPlanError::InvalidQuorum`] if a
    /// caller somehow presents a slot whose canonical committee/author
    /// invariants no longer hold.
    pub(crate) fn selection_authorization(
        &self,
    ) -> Result<
        AutonomousLaneReservationSelectionAuthorization,
        AutonomousLaneReservationSlotPlanError,
    > {
        let validator_count = u8::try_from(self.validator_set.len())
            .map_err(|_| AutonomousLaneReservationSlotPlanError::InvalidQuorum)?;
        if validator_count == 0 || validator_count > 128 {
            return Err(AutonomousLaneReservationSlotPlanError::InvalidQuorum);
        }
        let producer_index = self
            .validator_set
            .iter()
            .position(|peer| peer == &self.author)
            .ok_or(AutonomousLaneReservationSlotPlanError::InvalidQuorum)?;
        let producer = 1_u128
            .checked_shl(
                u32::try_from(producer_index)
                    .map_err(|_| AutonomousLaneReservationSlotPlanError::InvalidQuorum)?,
            )
            .ok_or(AutonomousLaneReservationSlotPlanError::InvalidQuorum)?;
        Ok(AutonomousLaneReservationSelectionAuthorization {
            scope: self.reservation_scope(),
            height_context_id: self.height_context_id,
            validator_count,
            producer,
        })
    }

    /// Convert the plan into the exact scope accepted by the durable queue.
    #[must_use]
    pub(crate) fn reservation_scope(&self) -> LaneQueueReservationScopeV1 {
        LaneQueueReservationScopeV1 {
            lane_id: self.lane_id,
            dataspace_id: self.dataspace_id,
            lane_incarnation: self.lane_incarnation,
            proposal_height: self.proposal_height,
            lane_block_height: self.lane_block_height,
            lane_block_view: self.lane_block_view,
            reservation_owner_hash: self.reservation_owner_hash,
            proposal_identity_hash: self.proposal_identity_hash,
        }
    }
}

/// Failure while deriving an autonomous queue-reservation slot.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub(crate) enum AutonomousLaneReservationSlotPlanError {
    /// The supplied global context is structurally invalid.
    #[error("invalid frozen height context: {reason}")]
    InvalidHeightContext {
        /// Structural validation failure.
        reason: String,
    },
    /// The context belongs to a different exact network than committed state.
    #[error("frozen height context belongs to a different network")]
    NetworkIdMismatch,
    /// The context is not the exact next height over committed state.
    #[error(
        "stale autonomous reservation context at height {context_height}; committed height is {committed_height}"
    )]
    StaleHeightContext {
        /// Height carried by the frozen context.
        context_height: u64,
        /// Current committed state height.
        committed_height: u64,
    },
    /// The requested lane has no active non-zero incarnation.
    #[error("active lane incarnation is unavailable")]
    MissingLaneIncarnation {
        /// Lane requested by the scheduler.
        lane_id: LaneId,
    },
    /// The lane/dataspace/incarnation tuple is not active at the frozen height.
    #[error("lane reservation route is inactive at the frozen height")]
    InactiveRoute {
        /// Requested lane.
        lane_id: LaneId,
        /// Requested dataspace.
        dataspace_id: DataSpaceId,
    },
    /// A predecessor artifact or certificate has not crossed its application boundary.
    #[error("lane reservation predecessor is not durably applied")]
    BlockedPredecessor {
        /// Blocked lane.
        lane_id: LaneId,
        /// Blocked dataspace.
        dataspace_id: DataSpaceId,
    },
    /// Durable sources disagree about the latest predecessor identity.
    #[error("lane reservation predecessor identity is conflicting")]
    ConflictingPredecessor {
        /// Conflicting lane.
        lane_id: LaneId,
        /// Conflicting dataspace.
        dataspace_id: DataSpaceId,
    },
    /// A non-genesis predecessor lacks its exact descriptor hash.
    #[error("lane reservation predecessor at height {previous_lane_block_height} has no hash")]
    MissingPredecessorHash {
        /// Height whose descriptor identity is unavailable.
        previous_lane_block_height: u64,
    },
    /// A height-zero lane unexpectedly carries a predecessor descriptor hash.
    #[error("fresh lane reservation carries an unexpected predecessor hash")]
    UnexpectedGenesisPredecessorHash,
    /// The next contiguous lane-local height cannot be represented.
    #[error("lane reservation height overflow")]
    LaneBlockHeightOverflow,
    /// No authoritative validator set exists for the active route.
    #[error("lane reservation committee is unavailable")]
    MissingCommittee {
        /// Lane missing an authoritative committee.
        lane_id: LaneId,
    },
    /// The authoritative validator set is empty, duplicated, or otherwise malformed.
    #[error("invalid lane reservation committee: {reason}")]
    InvalidCommittee {
        /// Committee validation failure.
        reason: String,
    },
    /// The canonical quorum cannot be represented for this validator set.
    #[error("lane reservation quorum is unavailable")]
    InvalidQuorum,
    /// The state or Kura frontier changed while the plan was being assembled.
    #[error("lane reservation inputs changed during planning")]
    PlanningSnapshotChanged,
    /// A versioned identity preimage could not be encoded.
    #[error("lane reservation identity encoding failed")]
    IdentityEncode,
}

#[derive(Encode)]
struct AutonomousLaneReservationSlotIdentityV1 {
    identity_version: u16,
    chain_id_hash: Hash,
    height_context_id: wire::HeightContextId,
    epoch: u64,
    proposal_height: u64,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_incarnation: Hash,
    previous_lane_block_height: u64,
    previous_lane_block_descriptor_hash: Option<Hash>,
    lane_block_height: u64,
    lane_block_view: u64,
    validator_set_hash: HashOf<Vec<PeerId>>,
    validator_count: u32,
    min_quorum: u32,
    qc_mode_tag: String,
}

#[derive(Encode)]
struct AutonomousLaneReservationOwnerIdentityV1 {
    identity_version: u16,
    proposal_identity_hash: Hash,
    author: PeerId,
}

const AUTONOMOUS_LANE_RESERVATION_SLOT_IDENTITY_VERSION_V1: u16 = 1;
const AUTONOMOUS_LANE_RESERVATION_OWNER_IDENTITY_VERSION_V1: u16 = 1;

fn encode_autonomous_lane_reservation_identity_hashes(
    identity: AutonomousLaneReservationSlotIdentityV1,
    author: &PeerId,
) -> Result<(Hash, Hash), AutonomousLaneReservationSlotPlanError> {
    let proposal_identity_hash = Hash::new(
        norito::to_bytes(&identity)
            .map_err(|_| AutonomousLaneReservationSlotPlanError::IdentityEncode)?,
    );
    let reservation_owner_hash = Hash::new(
        norito::to_bytes(&AutonomousLaneReservationOwnerIdentityV1 {
            identity_version: AUTONOMOUS_LANE_RESERVATION_OWNER_IDENTITY_VERSION_V1,
            proposal_identity_hash,
            author: author.clone(),
        })
        .map_err(|_| AutonomousLaneReservationSlotPlanError::IdentityEncode)?,
    );
    Ok((reservation_owner_hash, proposal_identity_hash))
}

/// Recompute the canonical queue-ownership identities advertised by an
/// autonomous proposal.
///
/// This verifier-side entry point contains no proposer-local state. Admission
/// callers first validate the proposal's route, incarnation, predecessor,
/// committee, quorum, QC tag, and deterministic author against their own
/// frozen height context, then compare every reservation key with the returned
/// pair.
pub(crate) fn autonomous_lane_reservation_identity_hashes_for_proposal(
    chain_id_hash: Hash,
    height_context_id: wire::HeightContextId,
    epoch: u64,
    proposal: &LaneBlockProposalV1,
    author: &PeerId,
) -> Result<(Hash, Hash), AutonomousLaneReservationSlotPlanError> {
    let descriptor = &proposal.descriptor;
    encode_autonomous_lane_reservation_identity_hashes(
        AutonomousLaneReservationSlotIdentityV1 {
            identity_version: AUTONOMOUS_LANE_RESERVATION_SLOT_IDENTITY_VERSION_V1,
            chain_id_hash,
            height_context_id,
            epoch,
            proposal_height: descriptor.proposal_height,
            lane_id: descriptor.lane_id,
            dataspace_id: descriptor.dataspace_id,
            lane_incarnation: descriptor.lane_incarnation,
            previous_lane_block_height: descriptor.previous_lane_block_height,
            previous_lane_block_descriptor_hash: descriptor.previous_lane_block_descriptor_hash,
            lane_block_height: descriptor.lane_block_height,
            lane_block_view: descriptor.lane_block_view,
            validator_set_hash: descriptor.validator_set_hash,
            validator_count: descriptor.validator_count,
            min_quorum: descriptor.min_quorum,
            qc_mode_tag: descriptor.qc_mode_tag.clone(),
        },
        author,
    )
}

/// Failure while deriving lane-local work from one frozen v2 context.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("{message}")]
pub(crate) struct V2LanePayloadPlanError {
    message: String,
}

impl V2LanePayloadPlanError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

fn v2_lane_context_mode_tag(context: &wire::HeightContext) -> String {
    let base_mode_tag = match context.mode {
        wire::ConsensusMode::Permissioned => wire::PERMISSIONED_TAG,
        wire::ConsensusMode::Npos => wire::NPOS_TAG,
    };
    format!(
        "{base_mode_tag}::height-context:{}::epoch:{}",
        hex::encode(context.id().0.as_ref()),
        context.epoch
    )
}

fn autonomous_lane_predecessor_blocked(
    state: &State,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
) -> bool {
    state
        .unapplied_lane_block_artifact_heights_snapshot_cached()
        .contains_key(&(lane_id, dataspace_id))
        || state
            .unapplied_certified_lane_block_heights_snapshot_cached()
            .contains_key(&(lane_id, dataspace_id))
}

fn validate_autonomous_lane_reservation_eligibility(
    context_height: u64,
    committed_height: u64,
    route_active: bool,
    predecessor_blocked: bool,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
) -> Result<(), AutonomousLaneReservationSlotPlanError> {
    if committed_height.checked_add(1) != Some(context_height) {
        return Err(AutonomousLaneReservationSlotPlanError::StaleHeightContext {
            context_height,
            committed_height,
        });
    }
    if !route_active {
        return Err(AutonomousLaneReservationSlotPlanError::InactiveRoute {
            lane_id,
            dataspace_id,
        });
    }
    if predecessor_blocked {
        return Err(AutonomousLaneReservationSlotPlanError::BlockedPredecessor {
            lane_id,
            dataspace_id,
        });
    }
    Ok(())
}

fn autonomous_lane_reservation_committee(
    state: &State,
    context: &wire::HeightContext,
    lane_id: LaneId,
) -> Result<Vec<PeerId>, AutonomousLaneReservationSlotPlanError> {
    let nexus = state.nexus_snapshot();
    let shared_committee = !nexus.enabled || !proposal_lookahead_enabled(&nexus, context.height);
    let validators = if shared_committee {
        context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<Vec<_>>()
    } else {
        state.authoritative_lane_peer_ids_at_height(lane_id, context.height)
    };
    if validators.is_empty() {
        return Err(AutonomousLaneReservationSlotPlanError::MissingCommittee { lane_id });
    }
    canonical_validator_set(lane_id, &validators).map_err(|error| {
        AutonomousLaneReservationSlotPlanError::InvalidCommittee {
            reason: format!("{error:?}"),
        }
    })
}

#[allow(clippy::too_many_arguments)]
fn assemble_autonomous_lane_reservation_slot(
    context: &wire::HeightContext,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_incarnation: Hash,
    previous_lane_block_height: u64,
    previous_lane_block_descriptor_hash: Option<Hash>,
    validator_set: Vec<PeerId>,
) -> Result<AutonomousLaneReservationSlotPlan, AutonomousLaneReservationSlotPlanError> {
    if previous_lane_block_height == 0 && previous_lane_block_descriptor_hash.is_some() {
        return Err(AutonomousLaneReservationSlotPlanError::UnexpectedGenesisPredecessorHash);
    }
    if previous_lane_block_height > 0 && previous_lane_block_descriptor_hash.is_none() {
        return Err(
            AutonomousLaneReservationSlotPlanError::MissingPredecessorHash {
                previous_lane_block_height,
            },
        );
    }
    let lane_block_height = previous_lane_block_height
        .checked_add(1)
        .ok_or(AutonomousLaneReservationSlotPlanError::LaneBlockHeightOverflow)?;
    let lane_block_view = 0;
    let validator_set = canonical_validator_set(lane_id, &validator_set).map_err(|error| {
        AutonomousLaneReservationSlotPlanError::InvalidCommittee {
            reason: format!("{error:?}"),
        }
    })?;
    let validator_count = u32::try_from(validator_set.len())
        .map_err(|_| AutonomousLaneReservationSlotPlanError::InvalidQuorum)?;
    let min_quorum = canonical_lane_commit_quorum(validator_set.len())
        .ok_or(AutonomousLaneReservationSlotPlanError::InvalidQuorum)?;
    let quorum = LaneRelayQuorumContext::new(validator_count, min_quorum)
        .map_err(|_| AutonomousLaneReservationSlotPlanError::InvalidQuorum)?;
    let validator_set_hash = HashOf::new(&validator_set);
    let author =
        crate::lane_consensus::deterministic_lane_author(&validator_set, lane_block_height)
            .cloned()
            .ok_or(AutonomousLaneReservationSlotPlanError::InvalidQuorum)?;
    let qc_mode_tag = LaneRelayEnvelope::lane_qc_mode_tag_for(
        lane_id,
        dataspace_id,
        &v2_lane_context_mode_tag(context),
    );
    let (reservation_owner_hash, proposal_identity_hash) =
        encode_autonomous_lane_reservation_identity_hashes(
            AutonomousLaneReservationSlotIdentityV1 {
                identity_version: AUTONOMOUS_LANE_RESERVATION_SLOT_IDENTITY_VERSION_V1,
                chain_id_hash: Hash::prehashed(*context.network_id.as_bytes()),
                height_context_id: context.id(),
                epoch: context.epoch,
                proposal_height: context.height,
                lane_id,
                dataspace_id,
                lane_incarnation,
                previous_lane_block_height,
                previous_lane_block_descriptor_hash,
                lane_block_height,
                lane_block_view,
                validator_set_hash,
                validator_count,
                min_quorum,
                qc_mode_tag: qc_mode_tag.clone(),
            },
            &author,
        )?;

    Ok(AutonomousLaneReservationSlotPlan {
        height_context_id: context.id(),
        lane_id,
        dataspace_id,
        lane_incarnation,
        proposal_height: context.height,
        previous_lane_block_height,
        previous_lane_block_descriptor_hash,
        lane_block_height,
        lane_block_view,
        validator_set,
        validator_set_hash,
        quorum,
        qc_mode_tag,
        author,
        reservation_owner_hash,
        proposal_identity_hash,
    })
}

/// Plan the exact next autonomous lane slot before selecting queue contents.
///
/// The state must still be at the parent of `context.height`. Any unapplied
/// lane artifact/certificate blocks planning, and a non-genesis predecessor
/// must have one exact descriptor hash. The returned author rotates by
/// `(lane_block_height - 1) mod validator_count`.
///
/// # Errors
///
/// Returns [`AutonomousLaneReservationSlotPlanError`] when the context is
/// stale, the route/incarnation/committee is not authoritative, the predecessor
/// is unavailable, or the observed state changes while the plan is assembled.
pub(crate) fn plan_autonomous_lane_reservation_slot(
    state: &State,
    kura: &Kura,
    context: &wire::HeightContext,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
) -> Result<AutonomousLaneReservationSlotPlan, AutonomousLaneReservationSlotPlanError> {
    context.validate().map_err(|error| {
        AutonomousLaneReservationSlotPlanError::InvalidHeightContext {
            reason: error.to_string(),
        }
    })?;
    if state.network_id_ref() != &context.network_id {
        return Err(AutonomousLaneReservationSlotPlanError::NetworkIdMismatch);
    }
    let committed_height = u64::try_from(state.committed_height()).map_err(|_| {
        AutonomousLaneReservationSlotPlanError::StaleHeightContext {
            context_height: context.height,
            committed_height: u64::MAX,
        }
    })?;
    let lane_incarnation = state
        .lane_incarnation_at_height(lane_id, context.height)
        .ok_or(AutonomousLaneReservationSlotPlanError::MissingLaneIncarnation { lane_id })?;
    let route_active = state.lane_route_and_incarnation_active_at_height(
        lane_id,
        dataspace_id,
        lane_incarnation,
        context.height,
    );
    let predecessor_blocked = autonomous_lane_predecessor_blocked(state, lane_id, dataspace_id);
    validate_autonomous_lane_reservation_eligibility(
        context.height,
        committed_height,
        route_active,
        predecessor_blocked,
        lane_id,
        dataspace_id,
    )?;
    let (previous_lane_block_height, previous_lane_block_descriptor_hash) =
        v2_known_lane_tip_for_route(
            state,
            kura,
            context.height,
            lane_id,
            dataspace_id,
            lane_incarnation,
        )
        .ok_or(
            AutonomousLaneReservationSlotPlanError::ConflictingPredecessor {
                lane_id,
                dataspace_id,
            },
        )?;
    let validator_set = autonomous_lane_reservation_committee(state, context, lane_id)?;
    let plan = assemble_autonomous_lane_reservation_slot(
        context,
        lane_id,
        dataspace_id,
        lane_incarnation,
        previous_lane_block_height,
        previous_lane_block_descriptor_hash,
        validator_set,
    )?;

    let committed_height_after = u64::try_from(state.committed_height())
        .map_err(|_| AutonomousLaneReservationSlotPlanError::PlanningSnapshotChanged)?;
    let exact_tip_after = v2_known_lane_tip_for_route(
        state,
        kura,
        context.height,
        lane_id,
        dataspace_id,
        lane_incarnation,
    );
    let exact_committee_after = autonomous_lane_reservation_committee(state, context, lane_id).ok();
    if committed_height_after != committed_height
        || state.lane_incarnation_at_height(lane_id, context.height) != Some(lane_incarnation)
        || !state.lane_route_and_incarnation_active_at_height(
            lane_id,
            dataspace_id,
            lane_incarnation,
            context.height,
        )
        || autonomous_lane_predecessor_blocked(state, lane_id, dataspace_id)
        || exact_tip_after
            != Some((
                previous_lane_block_height,
                previous_lane_block_descriptor_hash,
            ))
        || exact_committee_after.as_ref() != Some(&plan.validator_set)
    {
        return Err(AutonomousLaneReservationSlotPlanError::PlanningSnapshotChanged);
    }
    Ok(plan)
}

/// Derive deterministic lane-local RBC ownership and proposal artifacts for a
/// v2 candidate without invoking the legacy global actor.
///
/// The frozen context roster is used only for the single-lane/shared-domain
/// profile. Enabled multi-lane Nexus routes must have an authoritative lane
/// committee in committed state. A global leader need not also be the rotating
/// author for every selected lane: it commits the exact ownership and hands
/// executable bytes to the independently selected lane author. A lane whose
/// predecessor is not durably applied remains unavailable.
///
/// # Errors
///
/// Returns [`V2LanePayloadPlanError`] when aligned candidate inputs cannot be
/// reduced into canonical lane descriptors.
pub(crate) fn prepare_v2_lane_payload_plan(
    state: &State,
    kura: &Kura,
    context: &wire::HeightContext,
    view: wire::View,
    _local_peer: &PeerId,
    routing_decisions: &[RoutingDecision],
    candidate_hashes: &[Hash],
) -> Result<V2LanePayloadPlan, V2LanePayloadPlanError> {
    prepare_v2_lane_payload_plan_inner(
        state,
        kura,
        context,
        view,
        _local_peer,
        routing_decisions,
        candidate_hashes,
        false,
    )
}

/// Recompute a received proposal's lane plan while allowing one exact
/// canonical predecessor whose lane certificate/application receipt is still
/// catching up locally.
///
/// Proposal production must continue to use [`prepare_v2_lane_payload_plan`]
/// so a node never extends its own unapplied lane work. Validation is
/// different: another quorum may already have finalized the predecessor while
/// this node is completing the independently durable lane sidecars. In that
/// case the authenticated canonical block body is immutable planning input;
/// rejecting its exact successor would turn local recovery lag into consensus
/// divergence.
pub(crate) fn prepare_v2_lane_payload_validation_plan(
    state: &State,
    kura: &Kura,
    context: &wire::HeightContext,
    view: wire::View,
    local_peer: &PeerId,
    routing_decisions: &[RoutingDecision],
    candidate_hashes: &[Hash],
) -> Result<V2LanePayloadPlan, V2LanePayloadPlanError> {
    prepare_v2_lane_payload_plan_inner(
        state,
        kura,
        context,
        view,
        local_peer,
        routing_decisions,
        candidate_hashes,
        true,
    )
}

#[allow(clippy::too_many_arguments)]
fn prepare_v2_lane_payload_plan_inner(
    state: &State,
    kura: &Kura,
    context: &wire::HeightContext,
    view: wire::View,
    _local_peer: &PeerId,
    routing_decisions: &[RoutingDecision],
    candidate_hashes: &[Hash],
    allow_canonical_unapplied_predecessor: bool,
) -> Result<V2LanePayloadPlan, V2LanePayloadPlanError> {
    if routing_decisions.len() != candidate_hashes.len() {
        return Err(V2LanePayloadPlanError::new(
            "lane routing and candidate hash lengths differ",
        ));
    }
    if routing_decisions.is_empty() {
        return Ok(V2LanePayloadPlan::default());
    }

    let native_amx_blocked_routes =
        state.unapplied_native_amx_participant_control_heights_snapshot_cached();
    let mut blocked_routes = state.unapplied_lane_block_artifact_heights_snapshot_cached();
    for (route, height) in state.unapplied_certified_lane_block_heights_snapshot_cached() {
        blocked_routes
            .entry(route)
            .and_modify(|blocked_height| *blocked_height = (*blocked_height).max(height))
            .or_insert(height);
    }
    let committed_height = u64::try_from(state.committed_height()).map_err(|_| {
        V2LanePayloadPlanError::new("committed height does not fit the v2 lane planner")
    })?;
    let canonical_unapplied_tips = if allow_canonical_unapplied_predecessor {
        routing_decisions
            .iter()
            .filter_map(|route| {
                let artifact =
                    kura.latest_lane_block_artifact_matching(route.lane_id, |artifact| {
                        let ownership = &artifact.ownership;
                        ownership.dataspace_id == route.dataspace_id
                            && ownership.proposal_height <= committed_height
                            && ownership.proposal_height < context.height
                            && state.lane_route_and_incarnation_active_at_height(
                                ownership.lane_id,
                                ownership.dataspace_id,
                                ownership.lane_incarnation,
                                ownership.proposal_height,
                            )
                            && state.lane_route_and_incarnation_active_at_height(
                                ownership.lane_id,
                                ownership.dataspace_id,
                                ownership.lane_incarnation,
                                context.height,
                            )
                    })?;
                // A matching global block hash is not enough: bind the raw
                // sidecar to the exact ownership embedded in that canonical
                // block body before it can affect deterministic planning.
                let canonical = kura.canonical_lane_block_artifacts_at_proposal_height_matching(
                    artifact.ownership.proposal_height,
                    1,
                    |ownership| ownership == &artifact.ownership,
                );
                if canonical.first() != Some(&artifact) || canonical.len() != 1 {
                    return None;
                }
                let ownership = artifact.ownership;
                Some((
                    (ownership.lane_id, ownership.dataspace_id),
                    LaneBlockTip {
                        lane_id: ownership.lane_id,
                        dataspace_id: ownership.dataspace_id,
                        lane_incarnation: ownership.lane_incarnation,
                        latest_lane_block_height: ownership.lane_block_height,
                        latest_lane_block_descriptor_hash: ownership.lane_block_descriptor_hash,
                    },
                ))
            })
            .collect::<BTreeMap<_, _>>()
    } else {
        BTreeMap::new()
    };
    if allow_canonical_unapplied_predecessor {
        blocked_routes.retain(|route, blocked_height| {
            if native_amx_blocked_routes.contains_key(route) {
                return true;
            }
            canonical_unapplied_tips
                .get(route)
                .is_none_or(|tip| tip.latest_lane_block_height < *blocked_height)
        });
    }
    let unavailable_indices = routing_decisions
        .iter()
        .enumerate()
        .filter_map(|(index, route)| {
            blocked_routes
                .contains_key(&(route.lane_id, route.dataspace_id))
                .then_some(index)
        })
        .collect::<BTreeSet<_>>();
    if !unavailable_indices.is_empty() {
        return Ok(V2LanePayloadPlan {
            unavailable_indices,
            ..V2LanePayloadPlan::default()
        });
    }

    let schedule = ProposalBatchSchedule {
        actions: (0..routing_decisions.len())
            .map(|index| ProposalBatchAction::Accept {
                index,
                exceeds_gas_limit: false,
            })
            .collect(),
        ..ProposalBatchSchedule::default()
    };
    let nexus = state.nexus_snapshot();
    let shared_committee = !nexus.enabled || !proposal_lookahead_enabled(&nexus, context.height);
    let frozen_voters = context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();
    let committees = plan_lane_consensus_committees_with_authority(
        routing_decisions,
        &schedule,
        shared_committee.then_some(frozen_voters.as_slice()),
        |lane_id, _| {
            if shared_committee {
                Vec::new()
            } else {
                state.authoritative_lane_peer_ids_at_height(lane_id, context.height)
            }
        },
    )
    .map_err(|error| {
        V2LanePayloadPlanError::new(format!("lane committee planning failed: {error:?}"))
    })?;
    let context_mode_tag = v2_lane_context_mode_tag(context);
    let domains =
        plan_lane_consensus_domains(routing_decisions, &schedule, &committees, &context_mode_tag)
            .map_err(|error| {
            V2LanePayloadPlanError::new(format!("lane consensus-domain planning failed: {error:?}"))
        })?;
    let lane_incarnations = domains
        .iter()
        .map(|domain| {
            state
                .lane_incarnation_at_height(domain.lane_id, context.height)
                .map(|incarnation| (domain.lane_id, incarnation))
                .ok_or_else(|| {
                    V2LanePayloadPlanError::new(format!(
                        "lane {} has no active incarnation at height {}",
                        domain.lane_id.as_u32(),
                        context.height
                    ))
                })
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let tips = domains
        .iter()
        .map(|domain| {
            let lane_incarnation = *lane_incarnations.get(&domain.lane_id).ok_or_else(|| {
                V2LanePayloadPlanError::new(format!(
                    "lane {} has no planned incarnation at height {}",
                    domain.lane_id.as_u32(),
                    context.height
                ))
            })?;
            let ordinary_tip = v2_known_lane_tip_for_route(
                state,
                kura,
                context.height,
                domain.lane_id,
                domain.dataspace_id,
                lane_incarnation,
            )
            .ok_or_else(|| {
                V2LanePayloadPlanError::new(format!(
                    "lane {} has conflicting durable predecessor evidence at height {}",
                    domain.lane_id.as_u32(),
                    context.height
                ))
            })?;
            let canonical_tip = canonical_unapplied_tips
                .get(&(domain.lane_id, domain.dataspace_id))
                .filter(|tip| tip.lane_incarnation == lane_incarnation)
                .map(|tip| {
                    (
                        tip.latest_lane_block_height,
                        tip.latest_lane_block_descriptor_hash,
                    )
                });
            let (latest_lane_block_height, latest_lane_block_descriptor_hash) = match canonical_tip
            {
                Some(canonical) if canonical.0 > ordinary_tip.0 => canonical,
                Some(canonical)
                    if canonical.0 == ordinary_tip.0 && canonical.1 != ordinary_tip.1 =>
                {
                    return Err(V2LanePayloadPlanError::new(format!(
                        "lane {} has conflicting canonical predecessor evidence at height {}",
                        domain.lane_id.as_u32(),
                        context.height
                    )));
                }
                _ => ordinary_tip,
            };
            Ok(LaneBlockTip {
                lane_id: domain.lane_id,
                dataspace_id: domain.dataspace_id,
                lane_incarnation,
                latest_lane_block_height,
                latest_lane_block_descriptor_hash,
            })
        })
        .collect::<Result<Vec<_>, V2LanePayloadPlanError>>()?;
    // A fresh lane height always originates at lane view zero. The global
    // proposal view is carried separately in the ownership/hint below; binding
    // it into the lane view would make every global reproposal look like an
    // unauthenticated lane NewView jump and would make the executable payload
    // impossible to persist.
    let plan = plan_lane_payload_with_incarnations(
        &domains,
        &tips,
        candidate_hashes,
        &lane_incarnations,
        context.height,
        0,
    )
    .map_err(|error| {
        V2LanePayloadPlanError::new(format!("lane payload planning failed: {error:?}"))
    })?;

    let ownerships = plan
        .entries
        .iter()
        .map(|entry| v2_lane_payload_ownership(entry, context.height, view))
        .collect::<Vec<_>>();
    Ok(V2LanePayloadPlan {
        ownerships,
        proposals: plan.lane_block_proposal_artifacts,
        unavailable_indices: BTreeSet::new(),
    })
}

fn v2_known_lane_tips(state: &State, proposal_height: u64) -> Vec<LaneBlockTip> {
    let nexus = state.nexus_snapshot();
    let reset_heights = state.da_shard_canonical_reset_heights_snapshot_cached();
    let mut tips = state
        .lane_block_artifact_tips_snapshot_cached()
        .into_iter()
        .map(
            |(
                lane_id,
                dataspace_id,
                lane_incarnation,
                latest_lane_block_height,
                descriptor_hash,
            )| LaneBlockTip {
                lane_id,
                dataspace_id,
                lane_incarnation,
                latest_lane_block_height,
                latest_lane_block_descriptor_hash: descriptor_hash,
            },
        )
        .collect::<Vec<_>>();
    tips.extend(
        state
            .lane_relay_snapshot()
            .into_iter()
            .filter(|relay| {
                relay.has_merge_admission_material()
                    && relay.lane_block_descriptor_hash.is_some()
                    && state.lane_incarnation_at_height(relay.lane_id, proposal_height)
                        == Some(relay.lane_incarnation)
                    && (!nexus.enabled
                        || crate::state::nexus_active_lane_dataspace_at_height(
                            relay.lane_id,
                            &nexus,
                            proposal_height,
                        ) == Some(relay.dataspace_id))
                    && reset_heights
                        .get(&relay.lane_id)
                        .is_none_or(|reset_height| relay.block_height > *reset_height)
            })
            .map(|relay| LaneBlockTip {
                lane_id: relay.lane_id,
                dataspace_id: relay.dataspace_id,
                lane_incarnation: relay.lane_incarnation,
                latest_lane_block_height: relay.block_height,
                latest_lane_block_descriptor_hash: relay.lane_block_descriptor_hash,
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
                )| LaneBlockTip {
                    lane_id,
                    dataspace_id,
                    lane_incarnation,
                    latest_lane_block_height,
                    latest_lane_block_descriptor_hash: descriptor_hash,
                },
            ),
    );
    tips
}

/// Resolve the exact latest lane-local frontier for a participant-only AMX proposal.
pub(crate) fn v2_known_lane_tip_for_route(
    state: &State,
    kura: &Kura,
    proposal_height: u64,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_incarnation: Hash,
) -> Option<(u64, Option<Hash>)> {
    let mut matching = v2_known_lane_tips(state, proposal_height)
        .into_iter()
        .filter(|tip| {
            tip.lane_id == lane_id
                && tip.dataspace_id == dataspace_id
                && tip.lane_incarnation == lane_incarnation
        })
        .collect::<Vec<_>>();
    if let Some(receipt) = kura.latest_native_amx_participant_application_receipt_matching(
        lane_id,
        dataspace_id,
        lane_incarnation,
        |receipt| receipt.application_block_height < proposal_height,
    ) {
        let descriptor = &receipt.participant_proposal.descriptor;
        matching.push(LaneBlockTip {
            lane_id: descriptor.lane_id,
            dataspace_id: descriptor.dataspace_id,
            lane_incarnation: descriptor.lane_incarnation,
            latest_lane_block_height: descriptor.lane_block_height,
            latest_lane_block_descriptor_hash: Some(descriptor.descriptor_hash),
        });
    }
    if matching.is_empty() {
        return Some((0, None));
    }
    matching.sort_by_key(|tip| tip.latest_lane_block_height);
    let latest_height = matching.last()?.latest_lane_block_height;
    let mut hashes = matching
        .iter()
        .filter(|tip| tip.latest_lane_block_height == latest_height)
        .filter_map(|tip| tip.latest_lane_block_descriptor_hash)
        .collect::<BTreeSet<_>>();
    if hashes.len() > 1 {
        return None;
    }
    Some((latest_height, hashes.pop_first()))
}

fn v2_lane_payload_ownership(
    entry: &LanePayloadPlanEntry,
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
            .map(|index| u64::try_from(*index).expect("validated candidate index fits u64"))
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

fn build_lane_payload_plan_entries(
    domains: &[LaneConsensusDomain],
    lane_tips: &[LaneBlockTip],
    slots: &[LaneBlockSlot],
    subjects: &[LaneBlockSubject],
    ownerships: &[LanePayloadOwnership],
    candidate_hashes: &[Hash],
    proposal_height: u64,
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
            && subject.lane_incarnation == slot.lane_incarnation
            && ownership.dataspace_id == subject.dataspace_id
            && ownership.lane_incarnation == subject.lane_incarnation
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
            lane_incarnation: slot.lane_incarnation,
            proposal_height,
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
        && block_descriptor.lane_incarnation == subject.lane_incarnation
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
        && ownership.lane_incarnation == subject.lane_incarnation
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
        payload_block_hint: None,
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
        lane_incarnation: descriptor.lane_incarnation,
        proposal_height: descriptor.proposal_height,
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
        let index = match *action {
            ProposalBatchAction::Accept { index, .. } => index,
            #[cfg(test)]
            ProposalBatchAction::Defer { .. } => continue,
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
#[cfg(test)]
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
#[cfg(test)]
pub(super) struct LaneProposalBatch {
    lanes: Vec<LaneProposalWork>,
    total: usize,
}

#[cfg(test)]
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
fn test_lane_incarnations(
    domains: &[LaneConsensusDomain],
    known_tips: &[LaneBlockTip],
) -> BTreeMap<LaneId, Hash> {
    domains
        .iter()
        .map(|domain| {
            let incarnation = known_tips
                .iter()
                .find(|tip| tip.lane_id == domain.lane_id)
                .map_or_else(
                    || {
                        Hash::new(
                            format!(
                                "lane-tip-incarnation:{}:{}",
                                domain.lane_id.as_u32(),
                                domain.dataspace_id.as_u64()
                            )
                            .as_bytes(),
                        )
                    },
                    |tip| tip.lane_incarnation,
                );
            (domain.lane_id, incarnation)
        })
        .collect()
}

#[cfg(test)]
fn plan_latest_lane_block_tips_for_tests(
    domains: &[LaneConsensusDomain],
    known_tips: &[LaneBlockTip],
) -> Result<Vec<LaneBlockTip>, LaneBlockTipPlanError> {
    plan_latest_lane_block_tips_with_incarnations(
        domains,
        known_tips,
        &test_lane_incarnations(domains, known_tips),
    )
}

#[cfg(test)]
fn plan_lane_payload(
    domains: &[LaneConsensusDomain],
    known_tips: &[LaneBlockTip],
    candidate_hashes: &[Hash],
    proposal_height: u64,
    lane_block_view: u64,
) -> Result<LanePayloadPlan, LanePayloadPlanError> {
    plan_lane_payload_with_incarnations(
        domains,
        known_tips,
        candidate_hashes,
        &test_lane_incarnations(domains, known_tips),
        proposal_height,
        lane_block_view,
    )
}

include!("lane_planner_tests.rs");
