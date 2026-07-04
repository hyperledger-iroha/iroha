//! Deterministic per-lane proposal scheduling helpers.

use std::collections::{BTreeMap, VecDeque, btree_map::Entry};

use iroha_config::parameters::actual::Nexus;
use iroha_data_model::{
    nexus::{DataSpaceId, LaneId, LaneRelayEnvelope, LaneRelayQuorumContext},
    peer::PeerId,
};

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

#[derive(Clone, Debug, PartialEq, Eq)]
struct LaneAcceptedWork {
    dataspace_id: DataSpaceId,
    candidate_indices: Vec<usize>,
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

/// Derive lane-local domains using one shared validator set for every accepted lane.
///
/// This is the compatibility bridge for the current global proposal path. Fully
/// independent lane consensus can replace the shared committee with
/// manifest/stake-derived lane committees while preserving the same validation
/// and domain-separation behavior.
pub(super) fn plan_lane_consensus_domains_with_shared_committee(
    routing_decisions: &[RoutingDecision],
    schedule: &ProposalBatchSchedule,
    validators: &[PeerId],
    base_mode_tag: &str,
) -> Result<Vec<LaneConsensusDomain>, LaneConsensusDomainError> {
    let committees: Vec<_> = accepted_work_by_lane(routing_decisions, schedule)?
        .into_iter()
        .map(|(lane_id, work)| LaneConsensusCommittee {
            lane_id,
            dataspace_id: work.dataspace_id,
            validators: validators.to_vec(),
            min_quorum: None,
        })
        .collect();
    plan_lane_consensus_domains(routing_decisions, schedule, &committees, base_mode_tag)
}

fn accepted_work_by_lane(
    routing_decisions: &[RoutingDecision],
    schedule: &ProposalBatchSchedule,
) -> Result<BTreeMap<LaneId, LaneAcceptedWork>, LaneConsensusDomainError> {
    let mut accepted_work = BTreeMap::new();
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
