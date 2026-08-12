//! Inert coordinator staging for adjacent direct body-pipeline successors.

use super::{
    AdapterEffectAdmissionError, AdmissionDecision, AdmissionRequest, CandidateAdmission,
    CapacityClass, CoordinatorFault, InitialLifecycleState, LifecycleCoordinator, LifecyclePhase,
    LifecycleStageKind, LifecycleState, LifecycleWorkClass, OwnerId, PhysicalSlotId,
    PredecessorScope, TerminalOutcome, TurnLease,
    schema::{DurableContinuation, DurableContinuationEdge, DurablePayloadReference},
    work_registry::{
        LiveValidateSignRegistryPublicationError, PreparedCertifiedFetchStoreSuccessor,
        PreparedDurableStoreValidateSuccessor, PreparedInvalidBodyReportReplayPreAdmission,
        PreparedLiveValidateSignRegistryPublication, PreparedReadyDurableValidateAdapterPreview,
        PreparedReadyDurableValidatePersistedSignPreAdmission,
        PreparedRecoveredDecisionFetchStoreSuccessor,
        PreparedRecoveredLifecycleSignBroadcastSuccessor, SealedBodySuccessorProjectionError,
        SealedValidateTerminalProjectionError,
    },
};
use crate::sumeragi::v2::VerifiedHeightContext;

#[cfg(test)]
use crate::sumeragi::{
    v2::{AdapterEffect, SignRequest},
    v2_body_store::{DurableBodyReceipt, ValidatedBodyReceipt},
    v2_runtime::PendingRuntimeEffectBinding,
};

impl DurableContinuationEdge {
    const fn parent(self) -> (LifecycleWorkClass, LifecyclePhase, LifecycleStageKind) {
        match self {
            Self::FetchToStore => (
                LifecycleWorkClass::Fetch,
                LifecyclePhase::Fetch,
                LifecycleStageKind::FetchBody,
            ),
            Self::StoreToValidate => (
                LifecycleWorkClass::Store,
                LifecyclePhase::Store,
                LifecycleStageKind::StoreBody,
            ),
            Self::ValidateToApply => (
                LifecycleWorkClass::Validate,
                LifecyclePhase::Validate,
                LifecycleStageKind::ValidateBody,
            ),
            Self::ValidateToInvalidBodyReport
            | Self::ValidateToSignPrepare
            | Self::ValidateToSignCommit => (
                LifecycleWorkClass::Validate,
                LifecyclePhase::Validate,
                LifecycleStageKind::ValidateBody,
            ),
            Self::SignProposalToBroadcast => (
                LifecycleWorkClass::SignProposal,
                LifecyclePhase::Proposal,
                LifecycleStageKind::SignProposal,
            ),
            Self::SignPrepareToBroadcast => (
                LifecycleWorkClass::SignVote,
                LifecyclePhase::Prepare,
                LifecycleStageKind::SignPrepareVote,
            ),
            Self::SignCommitToBroadcast => (
                LifecycleWorkClass::SignVote,
                LifecyclePhase::Commit,
                LifecycleStageKind::SignCommitVote,
            ),
            Self::SignTimeoutToBroadcast => (
                LifecycleWorkClass::SignTimeout,
                LifecyclePhase::Timeout,
                LifecycleStageKind::SignTimeoutVote,
            ),
        }
    }

    const fn child(self) -> (LifecycleWorkClass, LifecyclePhase, LifecycleStageKind) {
        match self {
            Self::FetchToStore => (
                LifecycleWorkClass::Store,
                LifecyclePhase::Store,
                LifecycleStageKind::StoreBody,
            ),
            Self::StoreToValidate => (
                LifecycleWorkClass::Validate,
                LifecyclePhase::Validate,
                LifecycleStageKind::ValidateBody,
            ),
            Self::ValidateToApply => (
                LifecycleWorkClass::Apply,
                LifecyclePhase::Apply,
                LifecycleStageKind::ApplyDecision,
            ),
            Self::ValidateToInvalidBodyReport => (
                LifecycleWorkClass::InvalidBodyReport,
                LifecyclePhase::DiagnosticInvalidBody,
                LifecycleStageKind::ReportInvalidBody,
            ),
            Self::ValidateToSignPrepare => (
                LifecycleWorkClass::SignVote,
                LifecyclePhase::Prepare,
                LifecycleStageKind::SignPrepareVote,
            ),
            Self::ValidateToSignCommit => (
                LifecycleWorkClass::SignVote,
                LifecyclePhase::Commit,
                LifecycleStageKind::SignCommitVote,
            ),
            Self::SignProposalToBroadcast => (
                LifecycleWorkClass::Broadcast,
                LifecyclePhase::BroadcastProposal,
                LifecycleStageKind::BroadcastProposal,
            ),
            Self::SignPrepareToBroadcast => (
                LifecycleWorkClass::Broadcast,
                LifecyclePhase::BroadcastPrepareVote,
                LifecycleStageKind::BroadcastPrepareVote,
            ),
            Self::SignCommitToBroadcast => (
                LifecycleWorkClass::Broadcast,
                LifecyclePhase::BroadcastCommitVote,
                LifecycleStageKind::BroadcastCommitVote,
            ),
            Self::SignTimeoutToBroadcast => (
                LifecycleWorkClass::Broadcast,
                LifecyclePhase::BroadcastTimeoutVote,
                LifecycleStageKind::BroadcastTimeoutVote,
            ),
        }
    }

    fn preserves_lineage(self, parent: super::LifecycleKey, child: super::LifecycleKey) -> bool {
        if child.context() != parent.context()
            || child.round() != parent.round()
            || child.proposal_round() != parent.proposal_round()
            || child.subject() != parent.subject()
        {
            return false;
        }
        match self {
            Self::FetchToStore | Self::StoreToValidate => {
                child.execution_commitment() == parent.execution_commitment()
            }
            Self::ValidateToApply
            | Self::ValidateToInvalidBodyReport
            | Self::ValidateToSignPrepare
            | Self::ValidateToSignCommit => {
                child.execution_commitment().is_some()
                    && parent
                        .execution_commitment()
                        .is_none_or(|commitment| child.execution_commitment() == Some(commitment))
            }
            Self::SignProposalToBroadcast
            | Self::SignPrepareToBroadcast
            | Self::SignCommitToBroadcast
            | Self::SignTimeoutToBroadcast => {
                child.execution_commitment() == parent.execution_commitment()
            }
        }
    }
}

/// Return whether two durable rows form one exact adjacent body-pipeline edge.
///
/// This is the shared authority for live transition staging and LedgerV1
/// restart validation. It deliberately compares semantic lineage only; the
/// ledger separately authenticates owner, ordinal, and reconstruction source.
pub(super) fn durable_continuation_successor_is_exact(
    edge: DurableContinuationEdge,
    parent_work_class: LifecycleWorkClass,
    parent_key: super::LifecycleKey,
    parent_stage: super::LifecycleStage,
    child_work_class: LifecycleWorkClass,
    child_key: super::LifecycleKey,
    child_stage: super::LifecycleStage,
) -> bool {
    let required_lineage_is_present = match edge {
        DurableContinuationEdge::SignTimeoutToBroadcast => true,
        DurableContinuationEdge::FetchToStore
        | DurableContinuationEdge::StoreToValidate
        | DurableContinuationEdge::ValidateToApply
        | DurableContinuationEdge::ValidateToInvalidBodyReport
        | DurableContinuationEdge::ValidateToSignPrepare
        | DurableContinuationEdge::ValidateToSignCommit
        | DurableContinuationEdge::SignProposalToBroadcast
        | DurableContinuationEdge::SignPrepareToBroadcast
        | DurableContinuationEdge::SignCommitToBroadcast => {
            parent_key.proposal_round().is_some() && parent_key.subject().is_some()
        }
    };
    required_lineage_is_present
        && parent_stage.predecessor_scope() == PredecessorScope::Independent
        && child_stage.predecessor_scope() == PredecessorScope::Independent
        && {
            let (expected_parent, parent_phase, parent_kind) = edge.parent();
            let (expected_child, child_phase, child_kind) = edge.child();
            parent_work_class == expected_parent
                && parent_key.phase() == parent_phase
                && parent_stage.kind() == parent_kind
                && child_work_class == expected_child
                && child_key.phase() == child_phase
                && child_stage.kind() == child_kind
                && edge.preserves_lineage(parent_key, child_key)
        }
}

/// Return whether one durable continuation preserves its exact body frame.
///
/// A Ready Fetch and its Store successor retain the exact same fsynced frame.
/// Store→Validate and Validate→Apply likewise preserve that frame byte-for-byte,
/// and a payload-free, mixed, or substituted pair is never recoverable. Sign
/// and diagnostic successors own separate replay authority and therefore
/// retain no body-frame payload themselves.
pub(super) fn durable_continuation_payload_is_exact(
    edge: DurableContinuationEdge,
    parent: DurablePayloadReference,
    child: DurablePayloadReference,
) -> bool {
    match edge {
        DurableContinuationEdge::FetchToStore
        | DurableContinuationEdge::StoreToValidate
        | DurableContinuationEdge::ValidateToApply => match (parent, child) {
            (
                DurablePayloadReference::BodyFrame(parent),
                DurablePayloadReference::BodyFrame(child),
            ) => parent == child,
            _ => false,
        },
        DurableContinuationEdge::ValidateToInvalidBodyReport
        | DurableContinuationEdge::ValidateToSignPrepare
        | DurableContinuationEdge::ValidateToSignCommit => {
            matches!(parent, DurablePayloadReference::BodyFrame(_))
                && child == DurablePayloadReference::None
        }
        DurableContinuationEdge::SignProposalToBroadcast
        | DurableContinuationEdge::SignPrepareToBroadcast
        | DurableContinuationEdge::SignCommitToBroadcast
        | DurableContinuationEdge::SignTimeoutToBroadcast => {
            parent == DurablePayloadReference::None && child == DurablePayloadReference::None
        }
    }
}

/// Return whether a Validate row retains one key-bound body frame.
///
/// Live, waiting, claimed, and terminal Validate rows all name the same frame
/// consumed by validation. A payload-free or non-Validate-shaped reference is
/// never executable or restartable.
pub(super) fn durable_validate_payload_is_exact(
    key: super::LifecycleKey,
    payload: DurablePayloadReference,
) -> bool {
    matches!(payload, DurablePayloadReference::BodyFrame(frame) if frame.matches_key(key))
        && key.phase() == LifecyclePhase::Validate
}

/// Closed pre-commit failure inventory for one staged body-pipeline edge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum BodyStageTransitionError {
    /// The supplied lease is not the edge's exact one-slot Effect parent.
    WrongParentShape,
    /// The coordinator no longer owns the supplied claimed parent lease.
    StaleLease,
    /// The child effect and sealed binding did not project in this height.
    Projection(AdapterEffectAdmissionError),
    /// The durable validation receipt does not bind the Apply certificate.
    #[cfg(test)]
    InvalidValidationReceipt,
    /// The durable body receipt does not reproduce this row's exact frame.
    InvalidBodyFrameReference,
    /// The projected child is not the edge's exact ready one-slot shape.
    InvalidChildProjection,
    /// Parent and child do not retain the same immutable causal owner.
    ForeignSuccessorOwner,
    /// Parent and child disagree on the edge's authenticated lineage coordinates.
    ForeignSuccessorLineage,
    /// No strictly new lifecycle ordinal remains available.
    OrdinalExhausted,
    /// Pure parent settlement failed on the staged coordinator copy.
    ParentSettlement(CoordinatorFault),
    /// Child admission did not produce one new admitted record.
    ChildAdmission(Box<AdmissionDecision>),
    /// The admitted child did not retain the exact parent owner.
    InvalidChildOwner,
    /// The admitted child did not receive the sole expected new ordinal.
    InvalidChildOrdinal,
    /// The staged parent tombstone or child record is not exact.
    InvalidStagedRecords,
    /// The edge was not net-zero with one Effect generation advance.
    InvalidCapacityTransition,
    /// The active lease does not carry the branch's exact output reservation.
    InvalidOutputReservation,
}

/// Fully checked staged state shared by adjacent body-stage wrappers.
struct StagedBodyStageTransition {
    staged: LifecycleCoordinator,
    parent_ordinal: u128,
    child_ordinal: u128,
    owner: OwnerId,
    child_slot: PhysicalSlotId,
    child_digest: super::LifecycleDigest,
}

/// Fully checked staged state for one consumed Validate with no successor.
struct StagedBodyNoSuccessorTransition {
    staged: LifecycleCoordinator,
    parent_ordinal: u128,
    released_consensus_reservation: bool,
}

/// One-shot authority minted only by the sealed no-successor entrypoint.
///
/// The registry may consume this permit to bind its closed completion to an
/// opaque transition projection, but no sibling module can construct another
/// permit or extract body authority from it.
pub(super) struct SealedValidateNoSuccessorProjectionPermit {
    _linearity: SealedValidateNoSuccessorProjectionLinearity,
}

struct SealedValidateNoSuccessorProjectionLinearity;

impl Drop for SealedValidateNoSuccessorProjectionLinearity {
    fn drop(&mut self) {}
}

impl SealedValidateNoSuccessorProjectionPermit {
    fn new() -> Self {
        Self {
            _linearity: SealedValidateNoSuccessorProjectionLinearity,
        }
    }
}

/// One-shot authority minted only by the sealed invalid-report entrypoint.
///
/// Candidate projection receives this non-Copy proof by shared reference
/// while it remains nested inside the exact adapter/registry token. The
/// registry then consumes the permit when it closes the opaque projection.
pub(in crate::sumeragi) struct SealedInvalidBodyReportProjectionPermit {
    _linearity: SealedInvalidBodyReportProjectionLinearity,
}

struct SealedInvalidBodyReportProjectionLinearity;

impl Drop for SealedInvalidBodyReportProjectionLinearity {
    fn drop(&mut self) {}
}

impl SealedInvalidBodyReportProjectionPermit {
    fn new() -> Self {
        Self {
            _linearity: SealedInvalidBodyReportProjectionLinearity,
        }
    }
}

/// One-shot authority for projecting the already-bound post-WAL Sign child.
///
/// The pre-WAL ordinary-to-Commit refinement has already consumed its opaque
/// registered-Prepare capability. This permit lets the fixed transition read
/// only the resulting nested replay authority; it cannot ask the runtime to
/// derive another pending owner.
pub(in crate::sumeragi) struct SealedValidateSignProjectionPermit {
    _linearity: SealedValidateSignProjectionLinearity,
}

struct SealedValidateSignProjectionLinearity;

impl Drop for SealedValidateSignProjectionLinearity {
    fn drop(&mut self) {}
}

impl SealedValidateSignProjectionPermit {
    fn new() -> Self {
        Self {
            _linearity: SealedValidateSignProjectionLinearity,
        }
    }
}

/// Opaque registry-derived inputs for one no-successor Validate cut.
///
/// Fields are private to this module. The registry can construct the value
/// only by returning the transition module's one-shot permit.
#[must_use = "a sealed no-successor projection has not entered coordinator staging"]
pub(super) struct SealedValidateNoSuccessorProjection {
    lease: TurnLease,
    parent_payload: DurablePayloadReference,
    release_consensus_reservation: bool,
}

impl SealedValidateNoSuccessorProjection {
    /// Close registry-derived coordinates under the transition permit.
    pub(super) fn from_registry(
        _permit: SealedValidateNoSuccessorProjectionPermit,
        lease: TurnLease,
        parent_payload: DurablePayloadReference,
        release_consensus_reservation: bool,
    ) -> Self {
        Self {
            lease,
            parent_payload,
            release_consensus_reservation,
        }
    }
}

/// Opaque registry/adapter-derived inputs for one invalid-body report cut.
///
/// The candidate and body frame have no accessor surface. They are visible
/// only to the transition module after the sealed registry join consumes the
/// one-shot permit.
#[must_use = "a sealed invalid-body report projection has not entered coordinator staging"]
pub(super) struct SealedInvalidBodyReportProjection {
    lease: TurnLease,
    candidate: CandidateAdmission,
    parent_payload: DurablePayloadReference,
}

impl SealedInvalidBodyReportProjection {
    /// Close registry/adapter-derived coordinates under the transition permit.
    pub(super) fn from_registry(
        _permit: SealedInvalidBodyReportProjectionPermit,
        lease: TurnLease,
        candidate: CandidateAdmission,
        parent_payload: DurablePayloadReference,
    ) -> Self {
        Self {
            lease,
            candidate,
            parent_payload,
        }
    }
}

/// Opaque registry/adapter projection of one exact post-WAL Sign successor.
///
/// Candidate and BodyFrame authority are visible only inside this transition
/// module. The owning pre-admission remains intact beside the staged
/// coordinator until the registry reservation and LedgerV1 transaction take
/// over.
#[must_use = "a sealed Validate Sign projection has not entered coordinator staging"]
pub(super) struct SealedValidateSignProjection {
    lease: TurnLease,
    candidate: CandidateAdmission,
    parent_payload: DurablePayloadReference,
}

impl SealedValidateSignProjection {
    /// Close registry-derived coordinates under the transition permit.
    pub(super) fn from_registry(
        _permit: SealedValidateSignProjectionPermit,
        lease: TurnLease,
        candidate: CandidateAdmission,
        parent_payload: DurablePayloadReference,
    ) -> Self {
        Self {
            lease,
            candidate,
            parent_payload,
        }
    }
}

/// Stage one exact Validate terminal with no emitted successor.
///
/// Rejected Validate rows conservatively reserve one Consensus slot before
/// claim. When the reducer emits no report, this staged cut releases only that
/// transient overlay by advancing the Consensus generation once; it never
/// decrements durable Consensus occupancy. Validated rows carry no output
/// reservation. Both branches release the parent Effect capacity exactly once.
#[allow(clippy::too_many_lines)]
fn stage_validate_no_successor_transition(
    coordinator: &LifecycleCoordinator,
    lease: &TurnLease,
    parent_payload: DurablePayloadReference,
    release_consensus_reservation: bool,
) -> Result<StagedBodyNoSuccessorTransition, BodyStageTransitionError> {
    if lease.work_class() != LifecycleWorkClass::Validate
        || lease.key().phase() != LifecyclePhase::Validate
        || lease.stage().kind() != LifecycleStageKind::ValidateBody
        || lease.stage().predecessor_scope() != PredecessorScope::Independent
        || lease.physical_slots().len() != 1
        || !lease
            .physical_slots()
            .keys()
            .all(|slot| slot.capacity_class() == Some(CapacityClass::Effect))
    {
        return Err(BodyStageTransitionError::WrongParentShape);
    }
    if coordinator.active_lease.as_ref() != Some(lease) {
        return Err(BodyStageTransitionError::StaleLease);
    }
    if !durable_validate_payload_is_exact(lease.key(), parent_payload) {
        return Err(BodyStageTransitionError::InvalidBodyFrameReference);
    }
    let expected_reservation = lease.output_reservation();
    match (release_consensus_reservation, expected_reservation) {
        (false, None) => {}
        (true, Some(reservation))
            if reservation.class() == CapacityClass::Consensus
                && reservation.wait_token().observed_generation()
                    == coordinator.capacity_generation[&CapacityClass::Consensus]
                && coordinator.capacity_used[&CapacityClass::Consensus]
                    .checked_add(1)
                    .is_some_and(|reserved| {
                        reserved
                            <= coordinator
                                .capacity_geometry
                                .limit(CapacityClass::Consensus)
                    }) => {}
        (false, Some(_)) | (true, None | Some(_)) => {
            return Err(BodyStageTransitionError::InvalidOutputReservation);
        }
    }

    let parent = coordinator
        .records
        .get(&lease.ordinal())
        .ok_or(BodyStageTransitionError::StaleLease)?;
    let parent_metadata = coordinator
        .durable_records
        .get(&lease.ordinal())
        .ok_or(BodyStageTransitionError::StaleLease)?;
    if parent_metadata.payload != parent_payload {
        return Err(BodyStageTransitionError::InvalidBodyFrameReference);
    }
    let parent_slots = lease
        .physical_slots()
        .keys()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    if parent.ordinal != lease.ordinal()
        || parent.owner != lease.owner()
        || parent.key != lease.key()
        || parent.work_class != LifecycleWorkClass::Validate
        || parent.stage != lease.stage()
        || parent.state != LifecycleState::Claimed(lease.id())
        || parent.physical_slots != *lease.physical_slots()
        || parent.episode.slot_universe != parent_slots
        || parent.episode.consumed_slots != parent_slots
        || !parent.episode.frozen_predecessors.is_empty()
        || coordinator
            .episode_authority
            .universe_for(parent.key)
            .as_ref()
            != Some(&parent.episode.universe)
        || !coordinator.episode_authority.admits_slots(
            LifecycleWorkClass::Validate.capacity_class(),
            &parent.episode.slot_universe,
        )
        || coordinator.key_index.get(&parent.key) != Some(&parent.ordinal)
        || coordinator.owner_index.get(&parent.owner.causal_root()) != Some(&parent.owner)
        || coordinator
            .records
            .values()
            .filter(|candidate| candidate.ordinal == parent.ordinal)
            .count()
            != 1
        || coordinator
            .records
            .values()
            .filter(|candidate| candidate.key == parent.key)
            .count()
            != 1
        || coordinator
            .key_index
            .values()
            .filter(|ordinal| **ordinal == parent.ordinal)
            .count()
            != 1
        || coordinator
            .owner_index
            .values()
            .filter(|owner| **owner == parent.owner)
            .count()
            != 1
        || coordinator.ready_index.contains(&parent.ordinal)
        || parent_metadata.reconstruction_source != parent.owner.causal_root().digest()
        || parent_metadata.continuation != DurableContinuation::None
    {
        return Err(BodyStageTransitionError::StaleLease);
    }

    let effect_used_before = coordinator.capacity_used[&CapacityClass::Effect];
    let expected_effect_used = effect_used_before
        .checked_sub(1)
        .ok_or(BodyStageTransitionError::InvalidCapacityTransition)?;
    let expected_effect_generation = coordinator.capacity_generation[&CapacityClass::Effect]
        .checked_add(1)
        .ok_or(BodyStageTransitionError::InvalidCapacityTransition)?;
    let expected_consensus_generation = coordinator.capacity_generation[&CapacityClass::Consensus]
        .checked_add(u64::from(release_consensus_reservation))
        .ok_or(BodyStageTransitionError::InvalidCapacityTransition)?;
    let records_before = coordinator.records.len();
    let durable_records_before = coordinator.durable_records.len();
    let mut staged = coordinator.stage_durable_transaction();
    let mut settlement_lease = lease.clone();
    settlement_lease.output_reservation = None;
    staged.active_lease = Some(settlement_lease.clone());
    staged.reduce_settle_body_parent_for_continuation(settlement_lease);
    if let Some(fault) = staged.fault {
        return Err(BodyStageTransitionError::ParentSettlement(fault));
    }
    let Some(metadata) = staged.durable_records.get_mut(&lease.ordinal()) else {
        return Err(BodyStageTransitionError::InvalidStagedRecords);
    };
    if metadata.continuation != DurableContinuation::None {
        return Err(BodyStageTransitionError::InvalidStagedRecords);
    }
    metadata.continuation = DurableContinuation::AdvancedNoSuccessor;
    if release_consensus_reservation {
        staged
            .capacity_generation
            .insert(CapacityClass::Consensus, expected_consensus_generation);
    }

    if staged.fault.is_some()
        || staged.active_lease.is_some()
        || staged.high_water != coordinator.high_water
        || staged.records.len() != records_before
        || staged.durable_records.len() != durable_records_before
        || staged.ready_index.contains(&lease.ordinal())
        || staged.key_index.get(&lease.key()) != Some(&lease.ordinal())
        || staged.owner_index.get(&lease.owner().causal_root()) != Some(&lease.owner())
        || !staged.records.get(&lease.ordinal()).is_some_and(|record| {
            record.owner == lease.owner()
                && record.key == lease.key()
                && record.work_class == LifecycleWorkClass::Validate
                && record.stage == lease.stage()
                && record.state == LifecycleState::Terminal(TerminalOutcome::Advanced)
                && record.physical_slots == *lease.physical_slots()
                && record.episode.slot_universe == parent_slots
                && record.episode.consumed_slots == parent_slots
        })
        || !staged
            .durable_records
            .get(&lease.ordinal())
            .is_some_and(|metadata| {
                metadata.reconstruction_source == lease.owner().causal_root().digest()
                    && metadata.payload == parent_payload
                    && metadata.continuation == DurableContinuation::AdvancedNoSuccessor
            })
        || staged.capacity_used[&CapacityClass::Effect] != expected_effect_used
        || staged.capacity_generation[&CapacityClass::Effect] != expected_effect_generation
        || staged.capacity_used[&CapacityClass::Consensus]
            != coordinator.capacity_used[&CapacityClass::Consensus]
        || staged.capacity_generation[&CapacityClass::Consensus] != expected_consensus_generation
        || CapacityClass::ALL
            .into_iter()
            .filter(|class| !matches!(class, CapacityClass::Effect | CapacityClass::Consensus))
            .any(|class| {
                staged.capacity_used[&class] != coordinator.capacity_used[&class]
                    || staged.capacity_generation[&class] != coordinator.capacity_generation[&class]
            })
    {
        return Err(BodyStageTransitionError::InvalidCapacityTransition);
    }

    Ok(StagedBodyNoSuccessorTransition {
        staged,
        parent_ordinal: lease.ordinal(),
        released_consensus_reservation: release_consensus_reservation,
    })
}

/// Stage one already-authorized adjacent body lifecycle transition on a coordinator copy.
///
/// The candidate must arrive with its canonical payload and replay authority
/// already attached. Every exactness check precedes cloning. Same-class Effect
/// continuations terminalize the parent before admitting the child. The
/// rejected-report branch instead admits its Consensus child after removing
/// the lease overlay, then terminalizes the parent, so the staged cut converts
/// reserved occupancy into durable occupancy without changing its generation.
#[allow(clippy::too_many_lines)]
#[derive(Clone, Copy)]
enum BodyStagePayloadRelationV1 {
    OrdinaryBodyFrame,
    RecoveredDecisionFetch,
    RecoveredLifecycleSign,
}

fn stage_body_stage_transition(
    coordinator: &LifecycleCoordinator,
    lease: &TurnLease,
    candidate: CandidateAdmission,
    parent_payload: DurablePayloadReference,
    edge: DurableContinuationEdge,
) -> Result<StagedBodyStageTransition, BodyStageTransitionError> {
    stage_body_stage_transition_with_payload_relation(
        coordinator,
        lease,
        candidate,
        parent_payload,
        edge,
        BodyStagePayloadRelationV1::OrdinaryBodyFrame,
    )
}

fn stage_recovered_decision_fetch_store_transition(
    coordinator: &LifecycleCoordinator,
    lease: &TurnLease,
    candidate: CandidateAdmission,
) -> Result<StagedBodyStageTransition, BodyStageTransitionError> {
    stage_body_stage_transition_with_payload_relation(
        coordinator,
        lease,
        candidate,
        DurablePayloadReference::None,
        DurableContinuationEdge::FetchToStore,
        BodyStagePayloadRelationV1::RecoveredDecisionFetch,
    )
}

fn stage_recovered_lifecycle_sign_broadcast_transition(
    coordinator: &LifecycleCoordinator,
    lease: &TurnLease,
    candidate: CandidateAdmission,
) -> Result<StagedBodyStageTransition, BodyStageTransitionError> {
    let edge = match (
        lease.work_class(),
        lease.key().phase(),
        lease.stage().kind(),
    ) {
        (
            LifecycleWorkClass::SignProposal,
            LifecyclePhase::Proposal,
            LifecycleStageKind::SignProposal,
        ) => DurableContinuationEdge::SignProposalToBroadcast,
        (
            LifecycleWorkClass::SignVote,
            LifecyclePhase::Prepare,
            LifecycleStageKind::SignPrepareVote,
        ) => DurableContinuationEdge::SignPrepareToBroadcast,
        (
            LifecycleWorkClass::SignVote,
            LifecyclePhase::Commit,
            LifecycleStageKind::SignCommitVote,
        ) => DurableContinuationEdge::SignCommitToBroadcast,
        (
            LifecycleWorkClass::SignTimeout,
            LifecyclePhase::Timeout,
            LifecycleStageKind::SignTimeoutVote,
        ) => DurableContinuationEdge::SignTimeoutToBroadcast,
        _ => return Err(BodyStageTransitionError::WrongParentShape),
    };
    stage_body_stage_transition_with_payload_relation(
        coordinator,
        lease,
        candidate,
        DurablePayloadReference::None,
        edge,
        BodyStagePayloadRelationV1::RecoveredLifecycleSign,
    )
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn stage_body_stage_transition_with_payload_relation(
    coordinator: &LifecycleCoordinator,
    lease: &TurnLease,
    candidate: CandidateAdmission,
    parent_payload: DurablePayloadReference,
    edge: DurableContinuationEdge,
    payload_relation: BodyStagePayloadRelationV1,
) -> Result<StagedBodyStageTransition, BodyStageTransitionError> {
    let (parent_work_class, parent_phase, parent_stage) = edge.parent();
    let (child_work_class, child_phase, child_stage) = edge.child();
    let child_capacity = child_work_class.capacity_class();
    if parent_work_class.capacity_class() != CapacityClass::Effect
        || !matches!(
            child_capacity,
            CapacityClass::Effect | CapacityClass::Consensus
        )
        || lease.work_class() != parent_work_class
        || lease.key().phase() != parent_phase
        || !lease
            .work_class()
            .accepts_stage(lease.key().phase(), lease.stage())
        || lease.stage().kind() != parent_stage
        || lease.stage().predecessor_scope() != PredecessorScope::Independent
        || lease.physical_slots().len() != 1
        || !lease
            .physical_slots()
            .keys()
            .all(|slot| slot.capacity_class() == Some(CapacityClass::Effect))
    {
        return Err(BodyStageTransitionError::WrongParentShape);
    }
    if coordinator.active_lease.as_ref() != Some(lease) {
        return Err(BodyStageTransitionError::StaleLease);
    }
    if matches!(parent_payload, DurablePayloadReference::BodyFrame(frame) if !frame.matches_key(lease.key()))
    {
        return Err(BodyStageTransitionError::InvalidBodyFrameReference);
    }
    match (child_capacity, lease.output_reservation()) {
        (CapacityClass::Effect, None) => {}
        (CapacityClass::Consensus, Some(reservation))
            if reservation.class() == CapacityClass::Consensus
                && reservation.wait_token().observed_generation()
                    == coordinator.capacity_generation[&CapacityClass::Consensus]
                && coordinator.capacity_used[&CapacityClass::Consensus]
                    .checked_add(1)
                    .is_some_and(|reserved| {
                        reserved
                            <= coordinator
                                .capacity_geometry
                                .limit(CapacityClass::Consensus)
                    }) => {}
        _ => return Err(BodyStageTransitionError::InvalidOutputReservation),
    }
    let parent = coordinator
        .records
        .get(&lease.ordinal())
        .ok_or(BodyStageTransitionError::StaleLease)?;
    let parent_metadata = coordinator
        .durable_records
        .get(&lease.ordinal())
        .ok_or(BodyStageTransitionError::StaleLease)?;
    if parent_metadata.payload != parent_payload {
        return Err(BodyStageTransitionError::InvalidBodyFrameReference);
    }
    let parent_slots = lease
        .physical_slots()
        .keys()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    if parent.ordinal != lease.ordinal()
        || parent.owner != lease.owner()
        || parent.key != lease.key()
        || parent.work_class != lease.work_class()
        || parent.stage != lease.stage()
        || parent.state != LifecycleState::Claimed(lease.id())
        || parent.physical_slots != *lease.physical_slots()
        || parent.episode.slot_universe != parent_slots
        || parent.episode.consumed_slots != parent_slots
        || coordinator.key_index.get(&parent.key) != Some(&parent.ordinal)
        || coordinator.owner_index.get(&parent.owner.causal_root()) != Some(&parent.owner)
        || coordinator.ready_index.contains(&parent.ordinal)
        || parent_metadata.reconstruction_source != parent.owner.causal_root().digest()
        || parent_metadata.continuation != DurableContinuation::None
    {
        return Err(BodyStageTransitionError::StaleLease);
    }

    let child_payload = candidate.payload;
    let child_slot = PhysicalSlotId::for_capacity(child_capacity, 0);
    let (projected_slots, projected_universe, projected_consumed) = candidate
        .physical_geometry
        .normalized()
        .map_err(|_| BodyStageTransitionError::InvalidChildProjection)?;
    let Some(&child_digest) = projected_slots.get(&child_slot) else {
        return Err(BodyStageTransitionError::InvalidChildProjection);
    };
    if candidate.work_class != child_work_class
        || candidate.key.phase() != child_phase
        || candidate.stage.kind() != child_stage
        || candidate.stage.predecessor_scope() != PredecessorScope::Independent
        || candidate.initial_state != InitialLifecycleState::Ready
        || !candidate.replay_authority_is_exact(coordinator.active_context)
        || candidate.producer_turn.is_some()
        || projected_slots.len() != 1
        || projected_universe.len() != 1
        || !projected_universe.contains(&child_slot)
        || projected_consumed.len() != 1
        || !projected_consumed.contains(&child_slot)
    {
        return Err(BodyStageTransitionError::InvalidChildProjection);
    }
    if matches!(child_payload, DurablePayloadReference::BodyFrame(frame) if !frame.matches_key(candidate.key))
    {
        return Err(BodyStageTransitionError::InvalidBodyFrameReference);
    }
    let payload_is_exact = match payload_relation {
        BodyStagePayloadRelationV1::OrdinaryBodyFrame => {
            durable_continuation_payload_is_exact(edge, parent_payload, child_payload)
        }
        BodyStagePayloadRelationV1::RecoveredDecisionFetch => {
            edge == DurableContinuationEdge::FetchToStore
                && parent_payload == DurablePayloadReference::None
                && super::replay_authority::recovered_decision_body_continuation_is_exact(
                    edge,
                    &parent_metadata.replay_authority,
                    parent_payload,
                    &candidate.replay_authority,
                    child_payload,
                ) == Some(true)
        }
        BodyStagePayloadRelationV1::RecoveredLifecycleSign => {
            matches!(
                edge,
                DurableContinuationEdge::SignProposalToBroadcast
                    | DurableContinuationEdge::SignPrepareToBroadcast
                    | DurableContinuationEdge::SignCommitToBroadcast
                    | DurableContinuationEdge::SignTimeoutToBroadcast
            ) && parent_payload == DurablePayloadReference::None
                && super::replay_authority::signed_broadcast_continuation_is_exact(
                    edge,
                    &parent_metadata.replay_authority,
                    parent_payload,
                    &candidate.replay_authority,
                    child_payload,
                ) == Some(true)
        }
    };
    if !payload_is_exact {
        return Err(BodyStageTransitionError::InvalidBodyFrameReference);
    }
    if candidate.causal_root != lease.owner().causal_root() {
        return Err(BodyStageTransitionError::ForeignSuccessorOwner);
    }
    if !durable_continuation_successor_is_exact(
        edge,
        lease.work_class(),
        lease.key(),
        lease.stage(),
        candidate.work_class,
        candidate.key,
        candidate.stage,
    ) {
        return Err(BodyStageTransitionError::ForeignSuccessorLineage);
    }

    let expected_child_ordinal = coordinator
        .high_water
        .checked_add(1)
        .ok_or(BodyStageTransitionError::OrdinalExhausted)?;
    let capacity_used_before = coordinator.capacity_used.clone();
    let capacity_generation_before = coordinator.capacity_generation.clone();
    let expected_consensus_used = if child_capacity == CapacityClass::Consensus {
        capacity_used_before[&CapacityClass::Consensus]
            .checked_add(1)
            .ok_or(BodyStageTransitionError::InvalidCapacityTransition)?
    } else {
        capacity_used_before[&CapacityClass::Consensus]
    };
    let expected_effect_generation = coordinator.capacity_generation[&CapacityClass::Effect]
        .checked_add(1)
        .ok_or(BodyStageTransitionError::InvalidCapacityTransition)?;
    let projected_candidate = candidate.clone();
    let request = AdmissionRequest::Candidate(candidate);
    let records_before = coordinator.records.len();
    let durable_records_before = coordinator.durable_records.len();
    let mut staged = coordinator.stage_durable_transaction();
    let decision = if child_capacity == CapacityClass::Effect {
        staged.reduce_settle_body_parent_for_continuation(lease.clone());
        if let Some(fault) = staged.fault {
            return Err(BodyStageTransitionError::ParentSettlement(fault));
        }
        staged.reduce_admit(request)
    } else {
        // Convert the pre-claim Consensus reservation into the exact child row
        // before releasing the parent Effect. This is one invisible staged
        // cut: no capacity generation changes while overlay occupancy becomes
        // durable row occupancy.
        let mut settlement_lease = lease.clone();
        settlement_lease.output_reservation = None;
        staged.active_lease = Some(settlement_lease.clone());
        let decision = staged.reduce_admit(request);
        if matches!(decision, AdmissionDecision::Admitted { .. }) {
            staged.reduce_settle_body_parent_for_continuation(settlement_lease);
            if let Some(fault) = staged.fault {
                return Err(BodyStageTransitionError::ParentSettlement(fault));
            }
        }
        decision
    };
    let AdmissionDecision::Admitted {
        owner,
        ordinal: child_ordinal,
        producer_turn_ordinal: None,
    } = decision
    else {
        return Err(BodyStageTransitionError::ChildAdmission(Box::new(decision)));
    };
    if owner != lease.owner() {
        return Err(BodyStageTransitionError::InvalidChildOwner);
    }
    if child_ordinal != expected_child_ordinal || child_ordinal == lease.ordinal() {
        return Err(BodyStageTransitionError::InvalidChildOrdinal);
    }
    let Some(parent_metadata) = staged.durable_records.get_mut(&lease.ordinal()) else {
        return Err(BodyStageTransitionError::InvalidStagedRecords);
    };
    if parent_metadata.continuation != DurableContinuation::None {
        return Err(BodyStageTransitionError::InvalidStagedRecords);
    }
    parent_metadata.continuation = DurableContinuation::successor(edge, child_ordinal);

    let parent_is_advanced = staged.records.get(&lease.ordinal()).is_some_and(|record| {
        record.ordinal == lease.ordinal()
            && record.owner == lease.owner()
            && record.key == lease.key()
            && record.work_class == parent_work_class
            && record.stage.kind() == parent_stage
            && record.stage.predecessor_scope() == PredecessorScope::Independent
            && record.state == LifecycleState::Terminal(TerminalOutcome::Advanced)
            && record.physical_slots == *lease.physical_slots()
            && record.episode.slot_universe == parent_slots
            && record.episode.consumed_slots == parent_slots
    });
    let child_is_exact = staged.records.get(&child_ordinal).is_some_and(|record| {
        record.owner == owner
            && record.ordinal == child_ordinal
            && record.key == projected_candidate.key
            && record.work_class == child_work_class
            && record.stage.kind() == child_stage
            && record.stage.predecessor_scope() == PredecessorScope::Independent
            && record.state == LifecycleState::Ready
            && record.physical_slots == projected_slots
            && record.episode.slot_universe == projected_universe
            && record.episode.consumed_slots == projected_consumed
    });
    if !parent_is_advanced
        || !child_is_exact
        || staged.active_lease.is_some()
        || staged.high_water != child_ordinal
        || staged.records.len() != records_before.saturating_add(1)
        || staged.durable_records.len() != durable_records_before.saturating_add(1)
        || staged.key_index.get(&lease.key()) != Some(&lease.ordinal())
        || staged.key_index.get(&projected_candidate.key) != Some(&child_ordinal)
        || staged.owner_index.get(&projected_candidate.causal_root) != Some(&owner)
        || staged.ready_index.contains(&lease.ordinal())
        || !staged.ready_index.contains(&child_ordinal)
        || staged
            .durable_records
            .get(&lease.ordinal())
            .is_none_or(|metadata| {
                metadata.reconstruction_source != lease.owner().causal_root().digest()
                    || metadata.payload != parent_payload
                    || metadata.continuation != DurableContinuation::successor(edge, child_ordinal)
            })
        || !staged
            .durable_records
            .get(&child_ordinal)
            .is_some_and(|metadata| {
                metadata.matches_admission(&projected_candidate)
                    && metadata.continuation == DurableContinuation::None
            })
    {
        return Err(BodyStageTransitionError::InvalidStagedRecords);
    }
    let capacity_is_exact = match child_capacity {
        CapacityClass::Effect => {
            staged.capacity_used[&CapacityClass::Effect]
                == capacity_used_before[&CapacityClass::Effect]
                && staged.capacity_generation[&CapacityClass::Effect] == expected_effect_generation
                && CapacityClass::ALL
                    .into_iter()
                    .filter(|class| *class != CapacityClass::Effect)
                    .all(|class| {
                        staged.capacity_used[&class] == capacity_used_before[&class]
                            && staged.capacity_generation[&class]
                                == capacity_generation_before[&class]
                    })
        }
        CapacityClass::Consensus => {
            staged.capacity_used[&CapacityClass::Effect].checked_add(1)
                == Some(capacity_used_before[&CapacityClass::Effect])
                && staged.capacity_generation[&CapacityClass::Effect] == expected_effect_generation
                && staged.capacity_used[&CapacityClass::Consensus] == expected_consensus_used
                && staged.capacity_generation[&CapacityClass::Consensus]
                    == capacity_generation_before[&CapacityClass::Consensus]
                && CapacityClass::ALL
                    .into_iter()
                    .filter(|class| {
                        !matches!(class, CapacityClass::Effect | CapacityClass::Consensus)
                    })
                    .all(|class| {
                        staged.capacity_used[&class] == capacity_used_before[&class]
                            && staged.capacity_generation[&class]
                                == capacity_generation_before[&class]
                    })
        }
        CapacityClass::Serve | CapacityClass::Producer => false,
    };
    if !capacity_is_exact {
        return Err(BodyStageTransitionError::InvalidCapacityTransition);
    }

    Ok(StagedBodyStageTransition {
        staged,
        parent_ordinal: lease.ordinal(),
        child_ordinal,
        owner,
        child_slot,
        child_digest,
    })
}

/// Fully reduced coordinator copy for one adjacent body-pipeline transition.
///
/// The live coordinator remains exclusively borrowed and untouched. The
/// staged copy has already terminalized the claimed parent as `Advanced` and
/// admitted its ready child, but this tranche deliberately exposes no commit,
/// persistence, or state-extraction method.
///
/// TODO: Add the sole consuming publication only with the future composite
/// registry/adapter transaction; dropping this token must remain a pure abort.
#[must_use = "a staged body-pipeline coordinator cut has not been published"]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct PreparedBodyStageTransition<'a> {
    _coordinator: &'a mut LifecycleCoordinator,
    staged: LifecycleCoordinator,
    edge: DurableContinuationEdge,
    parent_ordinal: u128,
    child_ordinal: u128,
    owner: OwnerId,
    child_slot: PhysicalSlotId,
    child_digest: super::LifecycleDigest,
}

#[allow(dead_code, variant_size_differences, clippy::large_enum_variant)]
enum SealedBodyStageSuccessor<'registry> {
    CertifiedFetchStore(PreparedCertifiedFetchStoreSuccessor<'registry>),
    DurableStoreValidate(PreparedDurableStoreValidateSuccessor<'registry>),
}

/// Fully reduced coordinator copy retaining its move-only registry successor.
///
/// The candidate was projected inside the closed successor token before the
/// coordinator copy was cloned. This value keeps both exclusive borrows alive
/// and exposes no commit, candidate, receipt, or state-extraction surface.
#[must_use = "a sealed body-pipeline coordinator cut has not been published"]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct PreparedSealedBodyStageTransition<'coordinator, 'registry> {
    _coordinator: &'coordinator mut LifecycleCoordinator,
    _successor: SealedBodyStageSuccessor<'registry>,
    staged: LifecycleCoordinator,
    edge: DurableContinuationEdge,
    parent_ordinal: u128,
    child_ordinal: u128,
    owner: OwnerId,
    child_slot: PhysicalSlotId,
    child_digest: super::LifecycleDigest,
}

/// Fully staged recovered WAL Fetch-to-Store publication.
///
/// The Fetch parent remains payload-free; only the Store child owns the exact
/// BodyFrame. Registry and adapter borrows are retained beside the cloned
/// coordinator until one LedgerV1 fsync succeeds.
#[must_use = "recovered Decision Fetch-to-Store transition has not been published"]
pub(super) struct PreparedRecoveredDecisionFetchStoreTransition<'coordinator, 'registry, 'adapter> {
    coordinator: &'coordinator mut LifecycleCoordinator,
    successor: PreparedRecoveredDecisionFetchStoreSuccessor<'registry, 'adapter>,
    staged: LifecycleCoordinator,
    parent_ordinal: u128,
    child_ordinal: u128,
    child_slot: PhysicalSlotId,
    child_digest: super::LifecycleDigest,
}

impl PreparedRecoveredDecisionFetchStoreTransition<'_, '_, '_> {
    /// Fsync the exact staged LedgerV1 successor while all volatile owners remain borrowed.
    pub(super) fn persist_exact_successor(
        &self,
    ) -> Result<(), super::ledger::LifecycleLedgerError> {
        self.coordinator
            .persist_exact_staged_successor(&self.staged)
    }

    /// Publish the already-fsynced coordinator, registry, and adapter tail.
    pub(super) fn commit_after_publication(self) {
        let Self {
            coordinator,
            successor,
            staged,
            parent_ordinal,
            child_ordinal,
            child_slot,
            child_digest,
        } = self;
        assert!(staged.records.get(&parent_ordinal).is_some_and(|record| {
            record.state == LifecycleState::Terminal(TerminalOutcome::Advanced)
        }));
        assert!(staged.records.get(&child_ordinal).is_some_and(|record| {
            record.state == LifecycleState::Ready
                && record.physical_slots.get(&child_slot) == Some(&child_digest)
        }));
        let adapter = successor.commit_after_publication();
        *coordinator = staged;
        adapter.commit_after_durable_settlement();
    }
}

/// Fully staged recovered Sign-to-Broadcast publication.
///
/// The original Sign, adapter preview, and signed Broadcast projection remain
/// borrowed beside a cloned coordinator until the exact LedgerV1 successor is
/// fsynced. Dropping this value is a pure pre-publication abort.
#[must_use = "recovered Sign-to-Broadcast transition has not been published"]
pub(super) struct PreparedRecoveredLifecycleSignBroadcastTransition<
    'coordinator,
    'registry,
    'adapter,
> {
    coordinator: &'coordinator mut LifecycleCoordinator,
    successor: PreparedRecoveredLifecycleSignBroadcastSuccessor<'registry, 'adapter>,
    staged: LifecycleCoordinator,
    parent_ordinal: u128,
    child_ordinal: u128,
    child_slot: PhysicalSlotId,
    child_digest: super::LifecycleDigest,
}

impl PreparedRecoveredLifecycleSignBroadcastTransition<'_, '_, '_> {
    /// Fsync the exact staged LedgerV1 successor while all volatile owners remain borrowed.
    pub(super) fn persist_exact_successor(
        &self,
    ) -> Result<(), super::ledger::LifecycleLedgerError> {
        self.coordinator
            .persist_exact_staged_successor(&self.staged)
    }

    /// Publish the already-fsynced coordinator, registry, and adapter tail.
    pub(super) fn commit_after_publication(self) {
        let Self {
            coordinator,
            successor,
            staged,
            parent_ordinal,
            child_ordinal,
            child_slot,
            child_digest,
        } = self;
        assert!(staged.records.get(&parent_ordinal).is_some_and(|record| {
            record.state == LifecycleState::Terminal(TerminalOutcome::Advanced)
        }));
        assert!(staged.records.get(&child_ordinal).is_some_and(|record| {
            record.state == LifecycleState::Ready
                && record.physical_slots.get(&child_slot) == Some(&child_digest)
        }));
        let adapter = successor.commit_after_publication();
        *coordinator = staged;
        adapter.commit_after_durable_broadcast();
    }
}

fn map_sealed_successor_projection_error(
    error: SealedBodySuccessorProjectionError,
) -> BodyStageTransitionError {
    match error {
        SealedBodySuccessorProjectionError::ForeignParent => BodyStageTransitionError::StaleLease,
        SealedBodySuccessorProjectionError::InvalidCarrier => {
            BodyStageTransitionError::InvalidChildProjection
        }
        SealedBodySuccessorProjectionError::Projection(error) => {
            BodyStageTransitionError::Projection(error)
        }
    }
}

/// Fully reduced coordinator copy retaining the consumed no-successor preview.
///
/// The staged copy has released the parent Effect capacity and records the
/// typed `AdvancedNoSuccessor` tombstone. A rejected non-report branch has
/// also released its transient Consensus reservation by advancing that
/// generation exactly once. The live coordinator, registry, and adapter stay
/// exclusively borrowed, and this inert tranche exposes no publication API.
#[must_use = "a sealed no-successor Validate cut has not been published"]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct PreparedSealedValidateNoSuccessorTransition<'coordinator, 'registry, 'adapter> {
    _coordinator: &'coordinator mut LifecycleCoordinator,
    _preview: PreparedReadyDurableValidateAdapterPreview<'registry, 'adapter>,
    staged: LifecycleCoordinator,
    parent_ordinal: u128,
    released_consensus_reservation: bool,
}

#[allow(dead_code, variant_size_differences, clippy::large_enum_variant)]
enum SealedValidateNoSuccessorTransitionFailure {
    Projection(SealedValidateTerminalProjectionError),
    Stage(BodyStageTransitionError),
}

/// Fail-stop no-successor staging error retaining both authority borrows.
#[must_use = "failed no-successor staging still owns the sealed Validate preview"]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct SealedValidateNoSuccessorTransitionError<'registry, 'adapter> {
    _preview: PreparedReadyDurableValidateAdapterPreview<'registry, 'adapter>,
    _failure: SealedValidateNoSuccessorTransitionFailure,
}

/// Fully reduced Validate-to-report copy retaining every sealed authority cut.
///
/// The report candidate was projected while nested inside the exact adapter
/// and registry token. The live coordinator and both authority providers stay
/// exclusively borrowed; no candidate, effect, pending binding, receipt,
/// commit, persistence, or installation surface exists on this value.
#[must_use = "a sealed invalid-body report cut has not been published"]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct PreparedSealedValidateReportTransition<'coordinator, 'registry, 'adapter> {
    _coordinator: &'coordinator mut LifecycleCoordinator,
    _report: PreparedInvalidBodyReportReplayPreAdmission<'registry, 'adapter>,
    staged: LifecycleCoordinator,
    edge: DurableContinuationEdge,
    parent_ordinal: u128,
    child_ordinal: u128,
    owner: OwnerId,
    child_slot: PhysicalSlotId,
    child_digest: super::LifecycleDigest,
}

/// Fully staged live Validate-to-Sign transaction retaining every authority
/// provider until LedgerV1 and in-memory publication commit as one cut.
///
/// The child candidate came only from the nested post-WAL replay/pending seal.
/// No registry address is detached and no live coordinator state is changed
/// during preparation.
#[must_use = "a sealed live Validate-to-Sign transition has not been published"]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct PreparedSealedValidateSignTransition<'coordinator, 'registry, 'adapter> {
    coordinator: &'coordinator mut LifecycleCoordinator,
    publication: PreparedReadyDurableValidatePersistedSignPreAdmission<'registry, 'adapter>,
    staged: LifecycleCoordinator,
    lease: TurnLease,
    edge: DurableContinuationEdge,
    parent_ordinal: u128,
    child_ordinal: u128,
    child_slot: PhysicalSlotId,
    child_digest: super::LifecycleDigest,
}

#[allow(dead_code, variant_size_differences, clippy::large_enum_variant)]
enum SealedValidateSignTransitionFailure {
    MissingLedgerStore,
    Projection(SealedValidateTerminalProjectionError),
    Stage(BodyStageTransitionError),
}

/// Pre-publication error retaining the complete post-WAL fixed join.
#[must_use = "failed Validate-to-Sign staging still owns post-WAL authority"]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct SealedValidateSignTransitionError<'registry, 'adapter> {
    _publication: PreparedReadyDurableValidatePersistedSignPreAdmission<'registry, 'adapter>,
    _failure: SealedValidateSignTransitionFailure,
}

#[allow(dead_code, variant_size_differences, clippy::large_enum_variant)]
enum LiveValidateSignPublicationFailure<'registry, 'adapter> {
    Registry(LiveValidateSignRegistryPublicationError<'registry, 'adapter>),
    Ledger {
        _error: super::ledger::LifecycleLedgerError,
        _publication: PreparedLiveValidateSignRegistryPublication<'registry, 'adapter>,
    },
}

/// Restart-only publication error retaining the staged coordinator and every
/// detached registry/adapter authority.
///
/// If LedgerV1 persistence was ambiguous, the detached parent remains
/// non-restoring and adapter Drop latches fail-closed. Recovery must reopen the
/// WAL and LedgerV1 instead of reconstructing the old volatile parent.
#[must_use = "failed live Validate-to-Sign publication requires restart"]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct LiveValidateSignPublicationError<'coordinator, 'registry, 'adapter> {
    _coordinator: &'coordinator mut LifecycleCoordinator,
    _staged: LifecycleCoordinator,
    _failure: LiveValidateSignPublicationFailure<'registry, 'adapter>,
}

#[allow(dead_code, variant_size_differences, clippy::large_enum_variant)]
enum SealedValidateReportTransitionFailure {
    Projection(SealedValidateTerminalProjectionError),
    Stage(BodyStageTransitionError),
}

/// Fail-stop report staging error retaining the registry and adapter seals.
#[must_use = "failed invalid-body report staging still owns all sealed authority"]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct SealedValidateReportTransitionError<'registry, 'adapter> {
    _report: PreparedInvalidBodyReportReplayPreAdmission<'registry, 'adapter>,
    _failure: SealedValidateReportTransitionFailure,
}

impl LifecycleCoordinator {
    /// Stage the recovered payload-free Fetch and body-backed Store successor.
    pub(super) fn prepare_recovered_decision_fetch_store_transition<
        'coordinator,
        'registry,
        'adapter,
    >(
        &'coordinator mut self,
        lease: &TurnLease,
        verified: &VerifiedHeightContext,
        successor: PreparedRecoveredDecisionFetchStoreSuccessor<'registry, 'adapter>,
    ) -> Result<
        PreparedRecoveredDecisionFetchStoreTransition<'coordinator, 'registry, 'adapter>,
        BodyStageTransitionError,
    > {
        if self.ledger_store.is_none() {
            return Err(BodyStageTransitionError::InvalidStagedRecords);
        }
        let candidate = successor
            .project_for_body_transition(lease, verified)
            .map_err(map_sealed_successor_projection_error)?;
        let transition = stage_recovered_decision_fetch_store_transition(self, lease, candidate)?;
        Ok(PreparedRecoveredDecisionFetchStoreTransition {
            coordinator: self,
            successor,
            staged: transition.staged,
            parent_ordinal: transition.parent_ordinal,
            child_ordinal: transition.child_ordinal,
            child_slot: transition.child_slot,
            child_digest: transition.child_digest,
        })
    }

    /// Stage one recovered Sign and its adapter-authenticated Broadcast child.
    pub(super) fn prepare_recovered_lifecycle_sign_broadcast_transition<
        'coordinator,
        'registry,
        'adapter,
    >(
        &'coordinator mut self,
        lease: &TurnLease,
        verified: &VerifiedHeightContext,
        successor: PreparedRecoveredLifecycleSignBroadcastSuccessor<'registry, 'adapter>,
    ) -> Result<
        PreparedRecoveredLifecycleSignBroadcastTransition<'coordinator, 'registry, 'adapter>,
        BodyStageTransitionError,
    > {
        if self.ledger_store.is_none() {
            return Err(BodyStageTransitionError::InvalidStagedRecords);
        }
        let candidate = successor
            .project_for_transition(lease, verified)
            .map_err(map_sealed_successor_projection_error)?;
        let transition =
            stage_recovered_lifecycle_sign_broadcast_transition(self, lease, candidate)?;
        Ok(PreparedRecoveredLifecycleSignBroadcastTransition {
            coordinator: self,
            successor,
            staged: transition.staged,
            parent_ordinal: transition.parent_ordinal,
            child_ordinal: transition.child_ordinal,
            child_slot: transition.child_slot,
            child_digest: transition.child_digest,
        })
    }

    /// Stage the sole live post-WAL Validate-to-Sign transaction.
    ///
    /// A first-release node must have an attached LedgerV1 store. The child is
    /// projected from the already-bound replay seal under a one-shot permit,
    /// then the existing adjacent-body reducer stages the typed parent
    /// `Advanced` tombstone and exact Prepare/Commit Sign child.
    #[cfg_attr(not(test), allow(dead_code))]
    #[allow(clippy::result_large_err)]
    pub(super) fn prepare_sealed_validate_sign_transition<'coordinator, 'registry, 'adapter>(
        &'coordinator mut self,
        lease: &TurnLease,
        verified: &VerifiedHeightContext,
        publication: PreparedReadyDurableValidatePersistedSignPreAdmission<'registry, 'adapter>,
    ) -> Result<
        PreparedSealedValidateSignTransition<'coordinator, 'registry, 'adapter>,
        SealedValidateSignTransitionError<'registry, 'adapter>,
    > {
        if self.ledger_store.is_none() {
            return Err(SealedValidateSignTransitionError {
                _publication: publication,
                _failure: SealedValidateSignTransitionFailure::MissingLedgerStore,
            });
        }
        let projection = match publication.project_for_body_transition(
            SealedValidateSignProjectionPermit::new(),
            lease,
            verified,
        ) {
            Ok(projection) => projection,
            Err(error) => {
                return Err(SealedValidateSignTransitionError {
                    _publication: publication,
                    _failure: SealedValidateSignTransitionFailure::Projection(error),
                });
            }
        };
        let SealedValidateSignProjection {
            lease: projected_lease,
            candidate,
            parent_payload,
        } = projection;
        let edge = match candidate.key.phase() {
            LifecyclePhase::Prepare => DurableContinuationEdge::ValidateToSignPrepare,
            LifecyclePhase::Commit => DurableContinuationEdge::ValidateToSignCommit,
            _ => {
                return Err(SealedValidateSignTransitionError {
                    _publication: publication,
                    _failure: SealedValidateSignTransitionFailure::Projection(
                        SealedValidateTerminalProjectionError::InvalidCarrier,
                    ),
                });
            }
        };
        let transition = match stage_body_stage_transition(
            self,
            &projected_lease,
            candidate,
            parent_payload,
            edge,
        ) {
            Ok(transition) => transition,
            Err(error) => {
                return Err(SealedValidateSignTransitionError {
                    _publication: publication,
                    _failure: SealedValidateSignTransitionFailure::Stage(error),
                });
            }
        };
        Ok(PreparedSealedValidateSignTransition {
            coordinator: self,
            publication,
            staged: transition.staged,
            lease: projected_lease,
            edge,
            parent_ordinal: transition.parent_ordinal,
            child_ordinal: transition.child_ordinal,
            child_slot: transition.child_slot,
            child_digest: transition.child_digest,
        })
    }

    /// Consume one sealed inactive or no-effect Validate preview into an inert
    /// no-successor coordinator cut.
    ///
    /// The registry derives the exact BodyFrame from its still-installed
    /// completion. Busy, Apply, Persist, and Report branches cannot construct
    /// the opaque projection, and no receipt crosses this production seam.
    #[cfg_attr(not(test), allow(dead_code))]
    #[allow(clippy::result_large_err)]
    pub(super) fn prepare_sealed_validate_no_successor_transition<
        'coordinator,
        'registry,
        'adapter,
    >(
        &'coordinator mut self,
        lease: &TurnLease,
        preview: PreparedReadyDurableValidateAdapterPreview<'registry, 'adapter>,
    ) -> Result<
        PreparedSealedValidateNoSuccessorTransition<'coordinator, 'registry, 'adapter>,
        SealedValidateNoSuccessorTransitionError<'registry, 'adapter>,
    > {
        let projection = match preview.project_no_successor_for_body_transition(
            SealedValidateNoSuccessorProjectionPermit::new(),
            lease,
        ) {
            Ok(projection) => projection,
            Err(error) => {
                return Err(SealedValidateNoSuccessorTransitionError {
                    _preview: preview,
                    _failure: SealedValidateNoSuccessorTransitionFailure::Projection(error),
                });
            }
        };
        let transition = match stage_validate_no_successor_transition(
            self,
            &projection.lease,
            projection.parent_payload,
            projection.release_consensus_reservation,
        ) {
            Ok(transition) => transition,
            Err(error) => {
                return Err(SealedValidateNoSuccessorTransitionError {
                    _preview: preview,
                    _failure: SealedValidateNoSuccessorTransitionFailure::Stage(error),
                });
            }
        };
        Ok(PreparedSealedValidateNoSuccessorTransition {
            _coordinator: self,
            _preview: preview,
            staged: transition.staged,
            parent_ordinal: transition.parent_ordinal,
            released_consensus_reservation: transition.released_consensus_reservation,
        })
    }

    /// Stage one sealed certified-Fetch retirement and exact Store admission.
    ///
    /// The move-only successor owns the installed Fetch address, exact durable
    /// frame, child effect and pending binding, projected digest, and mandatory
    /// replay authority. No raw successor input crosses this production seam.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn prepare_sealed_fetch_store_transition<'coordinator, 'registry>(
        &'coordinator mut self,
        lease: &TurnLease,
        verified: &VerifiedHeightContext,
        successor: PreparedCertifiedFetchStoreSuccessor<'registry>,
    ) -> Result<PreparedSealedBodyStageTransition<'coordinator, 'registry>, BodyStageTransitionError>
    {
        let candidate = successor
            .project_for_body_transition(lease, verified)
            .map_err(map_sealed_successor_projection_error)?;
        let parent_payload = candidate.payload;
        let transition = stage_body_stage_transition(
            self,
            lease,
            candidate,
            parent_payload,
            DurableContinuationEdge::FetchToStore,
        )?;
        Ok(PreparedSealedBodyStageTransition {
            _coordinator: self,
            _successor: SealedBodyStageSuccessor::CertifiedFetchStore(successor),
            staged: transition.staged,
            edge: DurableContinuationEdge::FetchToStore,
            parent_ordinal: transition.parent_ordinal,
            child_ordinal: transition.child_ordinal,
            owner: transition.owner,
            child_slot: transition.child_slot,
            child_digest: transition.child_digest,
        })
    }

    /// Stage one sealed Store retirement and exact Validate admission.
    ///
    /// The Store token retains its registry borrow while it projects the child
    /// from the exact certified family and BodyFrame. The candidate's sealed
    /// payload is also the required parent payload and is never overwritten.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn prepare_sealed_store_validate_transition<'coordinator, 'registry>(
        &'coordinator mut self,
        lease: &TurnLease,
        verified: &VerifiedHeightContext,
        successor: PreparedDurableStoreValidateSuccessor<'registry>,
    ) -> Result<PreparedSealedBodyStageTransition<'coordinator, 'registry>, BodyStageTransitionError>
    {
        let candidate = successor
            .project_for_body_transition(lease, verified)
            .map_err(map_sealed_successor_projection_error)?;
        let parent_payload = candidate.payload;
        let transition = stage_body_stage_transition(
            self,
            lease,
            candidate,
            parent_payload,
            DurableContinuationEdge::StoreToValidate,
        )?;
        Ok(PreparedSealedBodyStageTransition {
            _coordinator: self,
            _successor: SealedBodyStageSuccessor::DurableStoreValidate(successor),
            staged: transition.staged,
            edge: DurableContinuationEdge::StoreToValidate,
            parent_ordinal: transition.parent_ordinal,
            child_ordinal: transition.child_ordinal,
            owner: transition.owner,
            child_slot: transition.child_slot,
            child_digest: transition.child_digest,
        })
    }

    /// Consume one exact invalid-body replay seal into an inert report cut.
    ///
    /// Report effect, pending ownership, rejection evidence, and BodyFrame all
    /// remain nested in the adapter/registry token while it projects the child.
    /// The claimed lease must retain the Consensus reservation authenticated by
    /// that same rejected completion.
    #[cfg_attr(not(test), allow(dead_code))]
    #[allow(clippy::result_large_err)]
    pub(super) fn prepare_sealed_validate_report_transition<'coordinator, 'registry, 'adapter>(
        &'coordinator mut self,
        lease: &TurnLease,
        verified: &VerifiedHeightContext,
        report: PreparedInvalidBodyReportReplayPreAdmission<'registry, 'adapter>,
    ) -> Result<
        PreparedSealedValidateReportTransition<'coordinator, 'registry, 'adapter>,
        SealedValidateReportTransitionError<'registry, 'adapter>,
    > {
        let projection = match report.project_for_body_transition(
            SealedInvalidBodyReportProjectionPermit::new(),
            lease,
            verified,
        ) {
            Ok(projection) => projection,
            Err(error) => {
                return Err(SealedValidateReportTransitionError {
                    _report: report,
                    _failure: SealedValidateReportTransitionFailure::Projection(error),
                });
            }
        };
        let transition = match stage_body_stage_transition(
            self,
            &projection.lease,
            projection.candidate,
            projection.parent_payload,
            DurableContinuationEdge::ValidateToInvalidBodyReport,
        ) {
            Ok(transition) => transition,
            Err(error) => {
                return Err(SealedValidateReportTransitionError {
                    _report: report,
                    _failure: SealedValidateReportTransitionFailure::Stage(error),
                });
            }
        };
        Ok(PreparedSealedValidateReportTransition {
            _coordinator: self,
            _report: report,
            staged: transition.staged,
            edge: DurableContinuationEdge::ValidateToInvalidBodyReport,
            parent_ordinal: transition.parent_ordinal,
            child_ordinal: transition.child_ordinal,
            owner: transition.owner,
            child_slot: transition.child_slot,
            child_digest: transition.child_digest,
        })
    }
}

impl<'coordinator, 'registry, 'adapter>
    PreparedSealedValidateSignTransition<'coordinator, 'registry, 'adapter>
{
    /// Reserve the exact concrete replacement, fsync the matching LedgerV1
    /// successor, then publish only infallible in-memory swaps.
    ///
    /// Registry preparation converts the existing recovered-WAL restorable cut
    /// into its non-restoring live form before storage I/O. Thus every error
    /// returned here owns restart-only authority and can never put the volatile
    /// Validate parent back beside an advanced durable ledger.
    #[cfg_attr(not(test), allow(dead_code))]
    #[allow(clippy::result_large_err)]
    pub(super) fn persist_and_publish(
        self,
    ) -> Result<(), LiveValidateSignPublicationError<'coordinator, 'registry, 'adapter>> {
        let Self {
            coordinator,
            publication,
            staged,
            lease,
            edge: _,
            parent_ordinal,
            child_ordinal,
            child_slot,
            child_digest,
        } = self;
        let registry = match publication.prepare_registry_publication(
            &lease,
            child_ordinal,
            child_slot,
            child_digest,
        ) {
            Ok(registry) => registry,
            Err(error) => {
                return Err(LiveValidateSignPublicationError {
                    _coordinator: coordinator,
                    _staged: staged,
                    _failure: LiveValidateSignPublicationFailure::Registry(error),
                });
            }
        };
        debug_assert_eq!(lease.ordinal(), parent_ordinal);
        if let Err(error) = coordinator.persist_exact_staged_successor(&staged) {
            return Err(LiveValidateSignPublicationError {
                _coordinator: coordinator,
                _staged: staged,
                _failure: LiveValidateSignPublicationFailure::Ledger {
                    _error: error,
                    _publication: registry,
                },
            });
        }

        *coordinator = staged;
        registry.publish_after_ledger_fsync();
        Ok(())
    }
}

#[cfg(test)]
fn digest_from_hash(hash: &iroha_crypto::Hash) -> super::LifecycleDigest {
    let mut bytes = [0_u8; 32];
    bytes.copy_from_slice(hash.as_ref());
    super::LifecycleDigest::new(bytes)
}

#[cfg(test)]
mod static_tests {
    use super::super::{LifecycleDigest, LifecycleKey, LifecycleRound, LifecycleStage};
    use super::*;

    fn key(
        phase: LifecyclePhase,
        proposal_round: bool,
        subject: bool,
        commitment: Option<LifecycleDigest>,
    ) -> LifecycleKey {
        LifecycleKey::new(
            LifecycleDigest::new([1; 32]),
            LifecycleRound::new(7, 3),
            proposal_round.then_some(LifecycleRound::new(7, 3)),
            subject.then_some(LifecycleDigest::new([2; 32])),
            phase,
            commitment,
        )
    }

    fn stage(kind: LifecycleStageKind, scope: PredecessorScope) -> LifecycleStage {
        LifecycleStage::new(kind, scope)
    }

    #[test]
    fn durable_successor_relation_covers_all_ten_exact_continuation_edges() {
        let commitment = LifecycleDigest::new([3; 32]);
        let exact = [
            (
                DurableContinuationEdge::FetchToStore,
                LifecycleWorkClass::Fetch,
                key(LifecyclePhase::Fetch, true, true, Some(commitment)),
                LifecycleStageKind::FetchBody,
                LifecycleWorkClass::Store,
                key(LifecyclePhase::Store, true, true, Some(commitment)),
                LifecycleStageKind::StoreBody,
            ),
            (
                DurableContinuationEdge::StoreToValidate,
                LifecycleWorkClass::Store,
                key(LifecyclePhase::Store, true, true, Some(commitment)),
                LifecycleStageKind::StoreBody,
                LifecycleWorkClass::Validate,
                key(LifecyclePhase::Validate, true, true, Some(commitment)),
                LifecycleStageKind::ValidateBody,
            ),
            (
                DurableContinuationEdge::ValidateToApply,
                LifecycleWorkClass::Validate,
                key(LifecyclePhase::Validate, true, true, None),
                LifecycleStageKind::ValidateBody,
                LifecycleWorkClass::Apply,
                key(LifecyclePhase::Apply, true, true, Some(commitment)),
                LifecycleStageKind::ApplyDecision,
            ),
            (
                DurableContinuationEdge::ValidateToInvalidBodyReport,
                LifecycleWorkClass::Validate,
                key(LifecyclePhase::Validate, true, true, None),
                LifecycleStageKind::ValidateBody,
                LifecycleWorkClass::InvalidBodyReport,
                key(
                    LifecyclePhase::DiagnosticInvalidBody,
                    true,
                    true,
                    Some(commitment),
                ),
                LifecycleStageKind::ReportInvalidBody,
            ),
            (
                DurableContinuationEdge::ValidateToSignPrepare,
                LifecycleWorkClass::Validate,
                key(LifecyclePhase::Validate, true, true, None),
                LifecycleStageKind::ValidateBody,
                LifecycleWorkClass::SignVote,
                key(LifecyclePhase::Prepare, true, true, Some(commitment)),
                LifecycleStageKind::SignPrepareVote,
            ),
            (
                DurableContinuationEdge::ValidateToSignCommit,
                LifecycleWorkClass::Validate,
                key(LifecyclePhase::Validate, true, true, None),
                LifecycleStageKind::ValidateBody,
                LifecycleWorkClass::SignVote,
                key(LifecyclePhase::Commit, true, true, Some(commitment)),
                LifecycleStageKind::SignCommitVote,
            ),
            (
                DurableContinuationEdge::SignProposalToBroadcast,
                LifecycleWorkClass::SignProposal,
                key(LifecyclePhase::Proposal, true, true, None),
                LifecycleStageKind::SignProposal,
                LifecycleWorkClass::Broadcast,
                key(LifecyclePhase::BroadcastProposal, true, true, None),
                LifecycleStageKind::BroadcastProposal,
            ),
            (
                DurableContinuationEdge::SignPrepareToBroadcast,
                LifecycleWorkClass::SignVote,
                key(LifecyclePhase::Prepare, true, true, Some(commitment)),
                LifecycleStageKind::SignPrepareVote,
                LifecycleWorkClass::Broadcast,
                key(
                    LifecyclePhase::BroadcastPrepareVote,
                    true,
                    true,
                    Some(commitment),
                ),
                LifecycleStageKind::BroadcastPrepareVote,
            ),
            (
                DurableContinuationEdge::SignCommitToBroadcast,
                LifecycleWorkClass::SignVote,
                key(LifecyclePhase::Commit, true, true, Some(commitment)),
                LifecycleStageKind::SignCommitVote,
                LifecycleWorkClass::Broadcast,
                key(
                    LifecyclePhase::BroadcastCommitVote,
                    true,
                    true,
                    Some(commitment),
                ),
                LifecycleStageKind::BroadcastCommitVote,
            ),
            (
                DurableContinuationEdge::SignTimeoutToBroadcast,
                LifecycleWorkClass::SignTimeout,
                key(LifecyclePhase::Timeout, false, false, None),
                LifecycleStageKind::SignTimeoutVote,
                LifecycleWorkClass::Broadcast,
                key(LifecyclePhase::BroadcastTimeoutVote, false, false, None),
                LifecycleStageKind::BroadcastTimeoutVote,
            ),
        ];
        for (edge, parent_class, parent_key, parent_kind, child_class, child_key, child_kind) in
            exact
        {
            assert!(durable_continuation_successor_is_exact(
                edge,
                parent_class,
                parent_key,
                stage(parent_kind, PredecessorScope::Independent),
                child_class,
                child_key,
                stage(child_kind, PredecessorScope::Independent),
            ));
        }

        assert!(!durable_continuation_successor_is_exact(
            DurableContinuationEdge::ValidateToApply,
            LifecycleWorkClass::Validate,
            key(LifecyclePhase::Validate, false, true, None),
            stage(
                LifecycleStageKind::ValidateBody,
                PredecessorScope::Independent,
            ),
            LifecycleWorkClass::Apply,
            key(LifecyclePhase::Apply, false, true, Some(commitment)),
            stage(
                LifecycleStageKind::ApplyDecision,
                PredecessorScope::Independent,
            ),
        ));
        assert!(!durable_continuation_successor_is_exact(
            DurableContinuationEdge::ValidateToApply,
            LifecycleWorkClass::Validate,
            key(LifecyclePhase::Validate, true, false, None),
            stage(
                LifecycleStageKind::ValidateBody,
                PredecessorScope::Independent,
            ),
            LifecycleWorkClass::Apply,
            key(LifecyclePhase::Apply, true, false, Some(commitment)),
            stage(
                LifecycleStageKind::ApplyDecision,
                PredecessorScope::Independent,
            ),
        ));
        assert!(!durable_continuation_successor_is_exact(
            DurableContinuationEdge::StoreToValidate,
            LifecycleWorkClass::Store,
            key(LifecyclePhase::Store, true, true, Some(commitment)),
            stage(LifecycleStageKind::StoreBody, PredecessorScope::Independent,),
            LifecycleWorkClass::Validate,
            key(LifecyclePhase::Validate, true, true, Some(commitment)),
            stage(
                LifecycleStageKind::ValidateBody,
                PredecessorScope::ReadyOrdinalPrefix,
            ),
        ));
        assert!(!durable_continuation_successor_is_exact(
            DurableContinuationEdge::FetchToStore,
            LifecycleWorkClass::Fetch,
            key(LifecyclePhase::Fetch, true, true, Some(commitment)),
            stage(LifecycleStageKind::FetchBody, PredecessorScope::Independent,),
            LifecycleWorkClass::Store,
            key(
                LifecyclePhase::Store,
                true,
                true,
                Some(LifecycleDigest::new([4; 32])),
            ),
            stage(LifecycleStageKind::StoreBody, PredecessorScope::Independent,),
        ));
        assert!(!durable_continuation_successor_is_exact(
            DurableContinuationEdge::SignTimeoutToBroadcast,
            LifecycleWorkClass::SignTimeout,
            key(LifecyclePhase::Timeout, false, false, None),
            stage(
                LifecycleStageKind::SignTimeoutVote,
                PredecessorScope::Independent,
            ),
            LifecycleWorkClass::Broadcast,
            key(
                LifecyclePhase::BroadcastTimeoutVote,
                true,
                true,
                Some(commitment),
            ),
            stage(
                LifecycleStageKind::BroadcastTimeoutVote,
                PredecessorScope::Independent,
            ),
        ));
    }

    #[test]
    fn durable_successor_payload_relation_rejects_body_frame_substitution() {
        let round = LifecycleRound::new(7, 3);
        let frame = DurablePayloadReference::BodyFrame(
            super::super::schema::DurableBodyFrameReference::new(
                LifecycleDigest::new([1; 32]),
                round,
                LifecycleDigest::new([2; 32]),
                LifecycleDigest::new([3; 32]),
                LifecycleDigest::new([4; 32]),
            ),
        );
        let foreign = DurablePayloadReference::BodyFrame(
            super::super::schema::DurableBodyFrameReference::new(
                LifecycleDigest::new([1; 32]),
                round,
                LifecycleDigest::new([2; 32]),
                LifecycleDigest::new([3; 32]),
                LifecycleDigest::new([5; 32]),
            ),
        );
        assert!(durable_continuation_payload_is_exact(
            DurableContinuationEdge::FetchToStore,
            frame,
            frame,
        ));
        assert!(!durable_continuation_payload_is_exact(
            DurableContinuationEdge::FetchToStore,
            DurablePayloadReference::None,
            frame,
        ));
        for edge in [
            DurableContinuationEdge::StoreToValidate,
            DurableContinuationEdge::ValidateToApply,
        ] {
            assert!(durable_continuation_payload_is_exact(edge, frame, frame));
            assert!(!durable_continuation_payload_is_exact(
                edge,
                DurablePayloadReference::None,
                DurablePayloadReference::None,
            ));
            assert!(!durable_continuation_payload_is_exact(edge, frame, foreign,));
            assert!(!durable_continuation_payload_is_exact(
                edge,
                frame,
                DurablePayloadReference::None,
            ));
            assert!(!durable_continuation_payload_is_exact(
                edge,
                DurablePayloadReference::None,
                frame,
            ));
        }
        assert!(durable_continuation_payload_is_exact(
            DurableContinuationEdge::ValidateToSignPrepare,
            frame,
            DurablePayloadReference::None,
        ));
        assert!(!durable_continuation_payload_is_exact(
            DurableContinuationEdge::ValidateToSignPrepare,
            frame,
            frame,
        ));
        assert!(!durable_continuation_payload_is_exact(
            DurableContinuationEdge::FetchToStore,
            DurablePayloadReference::None,
            DurablePayloadReference::None,
        ));
        assert!(!durable_continuation_payload_is_exact(
            DurableContinuationEdge::ValidateToSignPrepare,
            DurablePayloadReference::None,
            DurablePayloadReference::None,
        ));
        for edge in [
            DurableContinuationEdge::SignProposalToBroadcast,
            DurableContinuationEdge::SignPrepareToBroadcast,
            DurableContinuationEdge::SignCommitToBroadcast,
            DurableContinuationEdge::SignTimeoutToBroadcast,
        ] {
            assert!(durable_continuation_payload_is_exact(
                edge,
                DurablePayloadReference::None,
                DurablePayloadReference::None,
            ));
            assert!(!durable_continuation_payload_is_exact(
                edge,
                frame,
                DurablePayloadReference::None,
            ));
        }
    }

    #[test]
    fn transition_surface_is_ordered_borrow_bound_and_inert() {
        let source = include_str!("v2_lifecycle_body_pipeline_transition.rs");
        let production = source
            .split_once("\n#[cfg(test)]\nmod static_tests {")
            .map(|(production, _)| production)
            .expect("transition source has one production prefix");
        let authorized_core = production
            .split("fn stage_body_stage_transition")
            .nth(1)
            .and_then(|suffix| suffix.split("/// Fully reduced coordinator copy").next())
            .expect("body-stage reducer has one bounded production body");
        let staging = authorized_core
            .find("stage_durable_transaction")
            .expect("staged transition clones coordinator state");
        let settlement = authorized_core
            .find("reduce_settle_body_parent_for_continuation")
            .expect("staged transition settles its parent");
        let admission = authorized_core
            .find("reduce_admit")
            .expect("staged transition admits its child");
        assert!(
            settlement < admission,
            "the same-class Effect branch must release capacity before child admission"
        );
        for required in [
            "candidate.replay_authority_is_exact(coordinator.active_context)",
            ".physical_geometry",
            ".normalized()",
            "let Some(&child_digest) = projected_slots.get(&child_slot)",
            "durable_continuation_payload_is_exact",
        ] {
            assert!(
                authorized_core.contains(required),
                "authorized transition core omitted {required}"
            );
        }
        for forbidden in [
            "projection::admission_request",
            "projection::durable_body_frame_reference",
            "candidate.payload =",
            "PendingRuntimeEffectBinding",
            "AdapterEffect",
        ] {
            assert!(
                !authorized_core.contains(forbidden),
                "authorized transition core reopened raw authority through {forbidden}"
            );
        }
        let sealed_fetch = production
            .split("pub(super) fn prepare_sealed_fetch_store_transition")
            .nth(1)
            .and_then(|suffix| {
                suffix
                    .split("/// Stage one sealed Store retirement and exact Validate admission")
                    .next()
            })
            .expect("sealed Fetch-to-Store entrypoint has one bounded body");
        let sealed_store = production
            .split("pub(super) fn prepare_sealed_store_validate_transition")
            .nth(1)
            .and_then(|suffix| {
                suffix
                    .split("/// Consume one exact invalid-body replay seal")
                    .next()
            })
            .expect("sealed Store-to-Validate entrypoint has one bounded body");
        let sealed_no_successor = production
            .split("pub(super) fn prepare_sealed_validate_no_successor_transition")
            .nth(1)
            .and_then(|suffix| {
                suffix
                    .split("/// Stage one sealed certified-Fetch retirement")
                    .next()
            })
            .expect("sealed Validate no-successor entrypoint has one bounded body");
        let sealed_report = production
            .split("pub(super) fn prepare_sealed_validate_report_transition")
            .nth(1)
            .and_then(|suffix| suffix.split("#[cfg(test)]\nfn digest_from_hash").next())
            .expect("sealed Validate report entrypoint has one bounded body");
        let sealed_sign = production
            .split("pub(super) fn prepare_sealed_validate_sign_transition")
            .nth(1)
            .and_then(|suffix| {
                suffix
                    .split("/// Consume one sealed inactive or no-effect Validate preview")
                    .next()
            })
            .expect("sealed Validate-to-Sign entrypoint has one bounded body");
        assert!(sealed_fetch.contains("PreparedCertifiedFetchStoreSuccessor<'registry>"));
        assert!(sealed_store.contains("PreparedDurableStoreValidateSuccessor<'registry>"));
        for sealed in [sealed_fetch, sealed_store] {
            assert!(sealed.contains("project_for_body_transition(lease, verified)"));
            assert!(sealed.contains("PreparedSealedBodyStageTransition"));
            for forbidden in [
                "&AdapterEffect",
                "&PendingRuntimeEffectBinding",
                "&DurableBodyReceipt",
                "CandidateAdmission",
                "candidate.payload =",
                "projection::admission_request",
            ] {
                assert!(
                    !sealed.contains(forbidden),
                    "sealed transition entrypoint accepts or forges {forbidden}"
                );
            }
        }
        assert!(
            sealed_no_successor.contains(
                "preview: PreparedReadyDurableValidateAdapterPreview<'registry, 'adapter>"
            )
        );
        assert!(sealed_no_successor.contains("project_no_successor_for_body_transition("));
        assert!(sealed_no_successor.contains("SealedValidateNoSuccessorProjectionPermit::new()"));
        assert!(sealed_no_successor.contains("_preview: preview"));
        assert!(
            sealed_report.contains(
                "report: PreparedInvalidBodyReportReplayPreAdmission<'registry, 'adapter>"
            )
        );
        assert!(sealed_report.contains("SealedInvalidBodyReportProjectionPermit::new()"));
        assert!(sealed_report.contains("_report: report"));
        for required in [
            "self.ledger_store.is_none()",
            "publication.project_for_body_transition(",
            "SealedValidateSignProjectionPermit::new()",
            "DurableContinuationEdge::ValidateToSignPrepare",
            "DurableContinuationEdge::ValidateToSignCommit",
            "stage_body_stage_transition(",
            "PreparedSealedValidateSignTransition",
        ] {
            assert!(
                sealed_sign.contains(required),
                "sealed Validate-to-Sign entrypoint omitted {required}"
            );
        }
        for forbidden in [
            "&AdapterEffect",
            "&PendingRuntimeEffectBinding",
            "&DurableBodyReceipt",
            "projection::admission_request",
            "persist_durable_projection",
        ] {
            assert!(
                !sealed_sign.contains(forbidden),
                "sealed Validate-to-Sign entrypoint exposes {forbidden}"
            );
        }
        for sealed in [sealed_no_successor, sealed_report] {
            for forbidden in [
                "&DurableBodyReceipt",
                "&AdapterEffect",
                "&PendingRuntimeEffectBinding",
                "projection::admission_request",
                "candidate.payload =",
                "fn commit(",
                "fn staged(",
            ] {
                assert!(
                    !sealed.contains(forbidden),
                    "sealed terminal Validate entrypoint exposes {forbidden}"
                );
            }
        }
        assert!(!production.contains("prepare_fetch_store_transition"));
        assert!(!production.contains("prepare_store_validate_transition"));
        assert!(!production.contains("prepare_validate_report_transition"));
        assert!(!production.contains("prepare_validate_no_successor_transition"));
        assert!(!production.contains("stage_raw_body_stage_transition"));
        assert!(!production.contains("prepare_ready_validate_apply_transition"));
        assert!(!production.contains("prepare_validate_sign_transition"));
        assert!(!production.contains("fn prepare_body_stage_transition"));
        assert!(!production.contains("projection::admission_request"));
        let bls_tests = source
            .split_once("\n#[cfg(all(test, feature = \"bls\"))]\nmod tests {")
            .map(|(_, tests)| tests)
            .expect("BLS transition tests have one bounded suffix");
        for forbidden in [
            "projection::admission_request",
            "prepare_fetch_store_transition",
            "prepare_store_validate_transition",
            "prepare_validate_apply_transition(",
            ".prepare_body_stage_transition(",
        ] {
            assert!(
                !bls_tests.contains(forbidden),
                "BLS transition fixture reopened raw authority through {forbidden}"
            );
        }
        assert!(bls_tests.contains("prepare_authorized_body_transition("));
        assert!(bls_tests.contains("exact_live_wal_body_successor_candidate_for_test("));
        assert!(production.contains("PreparedSealedBodyStageTransition<'coordinator, 'registry>"));
        assert!(production.contains("_successor: SealedBodyStageSuccessor<'registry>"));
        assert!(production.contains("&'a mut LifecycleCoordinator"));
        assert!(production.contains("DurableContinuationEdge::FetchToStore"));
        assert!(production.contains("DurableContinuationEdge::StoreToValidate"));
        assert!(production.contains("DurableContinuationEdge::ValidateToApply"));
        assert!(production.contains("DurableContinuationEdge::ValidateToSignPrepare"));
        assert!(production.contains("DurableContinuationEdge::ValidateToSignCommit"));
        assert!(production.contains("DurableContinuationEdge::ValidateToInvalidBodyReport"));
        assert!(production.contains("DurableContinuation::AdvancedNoSuccessor"));
        assert!(production.contains("PreparedReadyDurableValidateAdapterPreview"));
        assert!(production.contains("PreparedInvalidBodyReportReplayPreAdmission"));
        assert!(production.contains("PreparedSealedValidateNoSuccessorTransition"));
        assert!(production.contains("PreparedSealedValidateReportTransition"));
        for (permit, linearity, mint) in [
            (
                "pub(super) struct SealedValidateNoSuccessorProjectionPermit",
                "impl Drop for SealedValidateNoSuccessorProjectionLinearity",
                "SealedValidateNoSuccessorProjectionPermit::new()",
            ),
            (
                "pub(in crate::sumeragi) struct SealedInvalidBodyReportProjectionPermit",
                "impl Drop for SealedInvalidBodyReportProjectionLinearity",
                "SealedInvalidBodyReportProjectionPermit::new()",
            ),
            (
                "pub(in crate::sumeragi) struct SealedValidateSignProjectionPermit",
                "impl Drop for SealedValidateSignProjectionLinearity",
                "SealedValidateSignProjectionPermit::new()",
            ),
        ] {
            assert!(production.contains(permit));
            assert!(production.contains(linearity));
            assert_eq!(production.matches(mint).count(), 1);
            assert!(!production.contains(&format!("#[derive(Clone)]\n{permit}")));
            assert!(!production.contains(&format!("#[derive(Copy)]\n{permit}")));
            assert!(!production.contains(&format!("#[derive(Clone, Copy)]\n{permit}")));
        }
        assert!(!production.contains("enum BodyStageTransitionEdge"));
        assert!(!production.contains("pub(super) fn stage_body_stage_transition"));
        for forbidden in [
            "persist_durable_projection",
            "fn commit(",
            "fn staged(",
            "ConcreteLifecycleWorkRegistry",
            "RuntimeEffectOwnership",
            "legacy_ordinal",
        ] {
            assert!(
                !production.contains(forbidden),
                "inert transition acquired forbidden authority: {forbidden}"
            );
        }
        for caller_source in [
            include_str!("v2.rs"),
            include_str!("v2_effects.rs"),
            include_str!("v2_runner.rs"),
            include_str!("v2_worker.rs"),
            include_str!("v2_runtime.rs"),
            include_str!("v2_lifecycle_coordinator.rs"),
        ] {
            for unwired in [
                "prepare_sealed_fetch_store_transition",
                "prepare_sealed_store_validate_transition",
                "prepare_sealed_validate_no_successor_transition",
                "prepare_sealed_validate_report_transition",
            ] {
                assert!(
                    !caller_source.contains(unwired),
                    "body-frame transition became production-wired through {unwired}"
                );
            }
        }

        let publication = production
            .split("pub(super) fn persist_and_publish(")
            .nth(1)
            .and_then(|suffix| suffix.split("\n}\n\n#[cfg(test)]").next())
            .expect("live Validate-to-Sign publication has one bounded body");
        let registry_preflight = publication
            .find("prepare_registry_publication(")
            .expect("registry reservation precedes fsync");
        let ledger_fsync = publication
            .find("persist_exact_staged_successor(&staged)")
            .expect("exact LedgerV1 fsync is mandatory");
        let coordinator_swap = publication
            .find("*coordinator = staged")
            .expect("coordinator swap follows fsync");
        let adapter_swap = publication
            .find("registry.publish_after_ledger_fsync()")
            .expect("registry and adapter publication follows coordinator swap");
        assert!(registry_preflight < ledger_fsync);
        assert!(ledger_fsync < coordinator_swap && coordinator_swap < adapter_swap);
        let post_fsync = &publication[coordinator_swap..];
        for forbidden in [
            "?",
            "return Err",
            "publish_status",
            "persist_durable_projection",
            "persist_exact_staged_successor",
        ] {
            assert!(
                !post_fsync.contains(forbidden),
                "post-fsync publication acquired fallible work through {forbidden}"
            );
        }

        let exact_fsync_callers = production
            .matches(".persist_exact_staged_successor(")
            .count()
            + [
                include_str!("v2.rs"),
                include_str!("v2_effects.rs"),
                include_str!("v2_runner.rs"),
                include_str!("v2_worker.rs"),
                include_str!("v2_runtime.rs"),
                include_str!("v2_lifecycle_concrete_admission.rs"),
                include_str!("v2_lifecycle_work_registry.rs"),
            ]
            .iter()
            .map(|source| {
                source
                    .split("\n#[cfg(test)]\nmod tests {")
                    .next()
                    .expect("caller source has a production prefix")
                    .matches(".persist_exact_staged_successor(")
                    .count()
            })
            .sum::<usize>();
        assert_eq!(
            exact_fsync_callers, 1,
            "the sealed live Validate-to-Sign transaction must be the sole exact-fsync caller"
        );
        let ledger_production = include_str!("v2_lifecycle_ledger.rs")
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("ledger source has one production prefix");
        assert_eq!(
            ledger_production
                .matches(".persist_exact_successor(")
                .count(),
            1,
            "the same staged transaction helper must be the sole exact-store successor caller"
        );
    }
}

#[cfg(all(test, feature = "bls"))]
mod tests {
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::{block::consensus_v2 as wire, peer::PeerId};

    use super::*;
    use crate::sumeragi::{
        v2_core::{EventTag, Generation},
        v2_lifecycle_coordinator::replay_authority::{
            CertifiedFetchReplayEvidenceV1, CertifiedStoreReplayEvidenceV1,
            DurableValidateReplayEvidenceV1,
        },
        v2_runtime::{RuntimeEffectOwnership, bind_adapter_effect_batch_ownership},
    };

    struct FetchStoreFixture {
        coordinator: LifecycleCoordinator,
        lease: TurnLease,
        verified: VerifiedHeightContext,
        durable_receipt: DurableBodyReceipt,
        store_effect: AdapterEffect,
        store_pending: PendingRuntimeEffectBinding,
        store_candidate: CandidateAdmission,
        store_replay: CertifiedStoreReplayEvidenceV1,
    }

    struct StoreValidateFixture {
        coordinator: LifecycleCoordinator,
        lease: TurnLease,
        verified: VerifiedHeightContext,
        durable_receipt: DurableBodyReceipt,
        store_effect: AdapterEffect,
        store_pending: PendingRuntimeEffectBinding,
        validate_effect: AdapterEffect,
        validate_pending: PendingRuntimeEffectBinding,
        validate_candidate: CandidateAdmission,
        validate_replay: DurableValidateReplayEvidenceV1,
    }

    struct ValidateApplyFixture {
        coordinator: LifecycleCoordinator,
        lease: TurnLease,
        verified: VerifiedHeightContext,
        validate_effect: AdapterEffect,
        validate_pending: PendingRuntimeEffectBinding,
        validate_candidate: CandidateAdmission,
        validate_replay: DurableValidateReplayEvidenceV1,
        validated_receipt: ValidatedBodyReceipt,
        apply_effect: AdapterEffect,
        apply_candidate: CandidateAdmission,
    }

    fn capacity_geometry(effect_limit: usize) -> super::super::schema::CapacityGeometry {
        super::super::schema::CapacityGeometry::new(CapacityClass::ALL.into_iter().map(|class| {
            (
                class,
                if class == CapacityClass::Effect {
                    effect_limit
                } else {
                    1
                },
            )
        }))
    }

    fn lifecycle_context(context: &wire::HeightContext) -> super::super::LifecycleContext {
        let mut id = [0_u8; 32];
        id.copy_from_slice(context.id().0.as_ref());
        super::super::LifecycleContext::new(super::super::LifecycleDigest::new(id), context.height)
    }

    fn verified_context() -> (VerifiedHeightContext, wire::HeightContext) {
        let mut keys = (1_u8..=4)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic Fetch-to-Store BLS key")
            })
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        let proofs = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("Fetch-to-Store proof of possession")
            })
            .collect::<Vec<_>>();
        let roster = keys
            .iter()
            .map(|key| wire::ValidatorPower {
                validator: PeerId::new(key.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        let context = wire::HeightContext {
            network_id: crate::sumeragi::synthetic_network_id("fetch-store-transition-test"),
            protocol_version: wire::PROTOCOL_VERSION,
            height: 1,
            epoch: 1,
            epoch_end_height: 100,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: wire::DualQuorum::from_roster(&roster).expect("fixture quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"fetch-store nexus context"),
            execution_policy_hash: Hash::new(b"fetch-store execution policy"),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 1024,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: 512 * 1024,
                max_chunk_count: 1024,
            },
            leader_seed: [0x51; 32],
        };
        let verified = VerifiedHeightContext::genesis(context.clone(), proofs)
            .expect("verified Fetch-to-Store height context");
        (verified, context)
    }

    fn fixture_execution_commitment() -> wire::ExecutionCommitment {
        wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"fetch-store state root"),
            Hash::new(b"fetch-store events root"),
            Hash::new(b"fetch-store trace root"),
            1,
            Hash::new(b"fetch-store fee summary"),
        )
    }

    fn body_manifest(
        verified: &VerifiedHeightContext,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> wire::PayloadManifest {
        wire::PayloadManifest {
            round,
            subject,
            payload_size_bytes: 1,
            layout: verified.context().da_layout,
            chunk_hashes: vec![Hash::new(b"body-pipeline chunk")],
            chunk_root: Hash::new(b"body-pipeline chunk root"),
        }
    }

    fn durable_body_receipt(
        verified: &VerifiedHeightContext,
        manifest: &wire::PayloadManifest,
    ) -> DurableBodyReceipt {
        DurableBodyReceipt::for_test(
            verified.context().id(),
            manifest.round,
            manifest.subject,
            HashOf::new(manifest),
        )
    }

    fn prepare_authorized_body_transition<'a>(
        coordinator: &'a mut LifecycleCoordinator,
        lease: &TurnLease,
        candidate: CandidateAdmission,
        parent_payload: DurablePayloadReference,
        edge: DurableContinuationEdge,
    ) -> Result<PreparedBodyStageTransition<'a>, BodyStageTransitionError> {
        let transition =
            stage_body_stage_transition(coordinator, lease, candidate, parent_payload, edge)?;
        Ok(PreparedBodyStageTransition {
            _coordinator: coordinator,
            staged: transition.staged,
            edge,
            parent_ordinal: transition.parent_ordinal,
            child_ordinal: transition.child_ordinal,
            owner: transition.owner,
            child_slot: transition.child_slot,
            child_digest: transition.child_digest,
        })
    }

    fn prepare_authorized_validate_apply_transition<'a>(
        coordinator: &'a mut LifecycleCoordinator,
        lease: &TurnLease,
        validated_receipt: &ValidatedBodyReceipt,
        apply_effect: &AdapterEffect,
        apply_candidate: CandidateAdmission,
    ) -> Result<PreparedBodyStageTransition<'a>, BodyStageTransitionError> {
        let AdapterEffect::Apply {
            subject,
            certificate,
            ..
        } = apply_effect
        else {
            return Err(BodyStageTransitionError::InvalidValidationReceipt);
        };
        let durable = validated_receipt.durable();
        if durable.context_id() != certificate.round.context_id
            || durable.round() != certificate.proposal_round
            || durable.subject() != *subject
            || validated_receipt.execution_commitment() != certificate.execution_commitment
        {
            return Err(BodyStageTransitionError::InvalidValidationReceipt);
        }
        let parent_payload = DurablePayloadReference::BodyFrame(
            projection::durable_body_frame_reference(coordinator.active_context, durable)
                .ok_or(BodyStageTransitionError::InvalidBodyFrameReference)?,
        );
        prepare_authorized_body_transition(
            coordinator,
            lease,
            apply_candidate,
            parent_payload,
            DurableContinuationEdge::ValidateToApply,
        )
    }

    fn fetch_store_fixture(effect_limit: usize) -> FetchStoreFixture {
        fetch_store_fixture_with_authority(effect_limit, wire::GlobalPhase::Prepare)
    }

    #[allow(clippy::too_many_lines)]
    fn fetch_store_fixture_with_authority(
        effect_limit: usize,
        authority_phase: wire::GlobalPhase,
    ) -> FetchStoreFixture {
        let (verified, context) = verified_context();
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let tag = EventTag::new(context.height, round.view, Generation::new(1));
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"fetch-store block")),
            payload_hash: Hash::new(b"fetch-store payload"),
        };
        let execution_commitment = fixture_execution_commitment();
        let certificate = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: authority_phase,
            subject,
            execution_commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0x51],
        };
        let manifest = body_manifest(&verified, round, subject);
        let certified_sources = context
            .roster
            .iter()
            .map(|validator| validator.validator.clone())
            .collect::<Vec<_>>();
        let fetch_effect = AdapterEffect::FetchBody {
            tag,
            round,
            subject,
            manifest: Some(manifest.clone()),
            certified_sources: certified_sources.clone(),
            certificate: Some(certificate.clone()),
        };
        let store_effect = AdapterEffect::StoreBody {
            tag,
            round,
            subject,
        };
        let durable_receipt = durable_body_receipt(&verified, &manifest);
        let response = wire::CertifiedBodyResponse {
            request_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"body-transition certified request",
            )),
            manifest: manifest.clone(),
            body: vec![0x51],
            responder: 0,
            signature: vec![0x52],
        };
        let fetch_owner = bind_adapter_effect_batch_ownership(
            core::slice::from_ref(&fetch_effect),
            vec![RuntimeEffectOwnership::fresh_for_test(tag, 1)],
        )
        .expect("bind certified Fetch fixture")
        .pop()
        .expect("one certified Fetch owner");
        let fetch_pending = fetch_owner
            .pending_adapter_effect_binding(&fetch_effect)
            .expect("mint sealed certified Fetch binding");
        let fetch_digest = digest_from_hash(fetch_pending.exact_effect_identity());
        let store_pending = fetch_pending
            .project_certified_fetch_store_successor(&fetch_effect, &store_effect)
            .expect("project exact ordinal-free Store successor");
        let fetch_replay = CertifiedFetchReplayEvidenceV1::from_signed_response_for_test(
            &fetch_effect,
            &response,
            &durable_receipt,
        )
        .expect("certified response projects exact Fetch replay evidence");
        let store_replay = fetch_replay
            .project_store_for_test(&store_effect, &durable_receipt)
            .expect("certified Fetch evidence projects exact Store replay evidence");
        let store_candidate = store_replay
            .project_candidate_for_test(&verified, &store_effect, &durable_receipt, &store_pending)
            .expect("canonical V1 evidence projects the Store candidate fixture");
        let replay = super::super::replay_authority::exact_durable_certified_fetch_record_fixture(
            lifecycle_context(&context),
            tag,
            certificate,
            manifest,
            certified_sources,
            &durable_receipt,
        );
        let fetch_slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
        let parent = super::super::CandidateAdmission::new(
            replay.key,
            store_candidate.causal_root,
            replay.work_class,
            replay.stage,
            InitialLifecycleState::Ready,
            store_candidate.reconstruction_source,
            replay.payload,
            replay.authority,
            super::super::PhysicalGeometry::new(
                [super::super::PhysicalSlot::new(fetch_slot, fetch_digest)],
                [fetch_slot],
            ),
            None,
        );
        let mut coordinator = LifecycleCoordinator::new(
            lifecycle_context(&context),
            0,
            capacity_geometry(effect_limit),
        );
        let AdmissionDecision::Admitted {
            ordinal,
            producer_turn_ordinal: None,
            ..
        } = coordinator.admit(AdmissionRequest::Candidate(parent))
        else {
            panic!("admit Fetch parent fixture")
        };
        let record = &coordinator.records[&ordinal];
        let ready = super::super::SchedulerReadyInputs::new(record, None, [0; 6]);
        let inputs = super::super::SchedulerInputs::new([], [(ordinal, ready)])
            .expect("unique Fetch scheduler census");
        let super::super::TurnPlan::Execute(lease) = coordinator.plan_turn(inputs) else {
            panic!("claim Fetch fixture")
        };
        FetchStoreFixture {
            coordinator,
            lease,
            verified,
            durable_receipt,
            store_effect,
            store_pending,
            store_candidate,
            store_replay,
        }
    }

    fn store_validate_fixture(
        effect_limit: usize,
        inherited_commitment: bool,
    ) -> StoreValidateFixture {
        store_validate_fixture_with_authority(
            effect_limit,
            inherited_commitment.then_some(wire::GlobalPhase::Prepare),
        )
    }

    #[allow(clippy::too_many_lines)]
    fn store_validate_fixture_with_authority(
        effect_limit: usize,
        inherited_authority: Option<wire::GlobalPhase>,
    ) -> StoreValidateFixture {
        let FetchStoreFixture {
            verified,
            durable_receipt,
            store_effect,
            store_pending: certified_store_pending,
            store_candidate: certified_store_candidate,
            store_replay: certified_store_replay,
            ..
        } = fetch_store_fixture_with_authority(
            effect_limit,
            inherited_authority.unwrap_or(wire::GlobalPhase::Prepare),
        );
        let AdapterEffect::StoreBody {
            tag,
            round,
            subject,
        } = store_effect
        else {
            unreachable!("Fetch successor fixture is one Store effect")
        };
        let store_effect = AdapterEffect::StoreBody {
            tag,
            round,
            subject,
        };
        let validate_effect = AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        };
        let (store_pending, store_candidate, validate_pending, validate_replay) =
            if inherited_authority.is_some() {
                let validate_pending = certified_store_pending
                    .project_store_validate_successor(&store_effect, &validate_effect)
                    .expect("project exact certified Validate successor");
                let validate_replay = DurableValidateReplayEvidenceV1::certified(
                    certified_store_replay
                        .project_validate(
                            &store_effect,
                            &durable_receipt,
                            &validate_effect,
                            &validate_pending,
                        )
                        .expect("certified Store evidence projects exact Validate evidence"),
                );
                (
                    certified_store_pending,
                    certified_store_candidate,
                    validate_pending,
                    validate_replay,
                )
            } else {
                let manifest = body_manifest(&verified, round, subject);
                let proposal = wire::Proposal {
                    round,
                    proposer: 0,
                    subject,
                    manifest: manifest.clone(),
                    justification: wire::ProposalJustification::ParentCommit(
                        wire::ParentCommitJustification { certificate: None },
                    ),
                    signature: vec![0x61],
                };
                let fetch_effect = AdapterEffect::FetchBody {
                    tag,
                    round,
                    subject,
                    manifest: Some(manifest),
                    certified_sources: Vec::new(),
                    certificate: None,
                };
                let mut fetch_owner = bind_adapter_effect_batch_ownership(
                    core::slice::from_ref(&fetch_effect),
                    vec![RuntimeEffectOwnership::fresh_for_test(tag, 2)],
                )
                .expect("bind remote-Proposal Fetch fixture")
                .pop()
                .expect("one remote-Proposal Fetch owner");
                assert!(
                    fetch_owner.bind_authenticated_remote_proposal_replay_for_test(
                        proposal,
                        &fetch_effect
                    )
                );
                let fetch_pending = fetch_owner
                    .pending_adapter_effect_binding(&fetch_effect)
                    .expect("mint sealed remote-Proposal Fetch binding");
                let fetch_replay = fetch_owner
                    .exact_remote_proposal_fetch_replay(&fetch_effect)
                    .expect("retain authenticated remote-Proposal replay evidence");
                let store_pending = fetch_pending
                    .project_proposal_fetch_store_successor(&fetch_effect, &store_effect)
                    .expect("project exact remote-Proposal Store successor");
                let stored_replay = fetch_replay
                    .project_exact_store(&store_effect, &store_pending)
                    .expect("project remote-Proposal Store replay evidence")
                    .bind_durable_body(&store_effect, &durable_receipt)
                    .expect("bind remote-Proposal Store to its body frame");
                let store_candidate = stored_replay
                    .project_candidate_for_test(
                        &verified,
                        &store_effect,
                        &durable_receipt,
                        &store_pending,
                    )
                    .expect("canonical Proposal evidence projects Store candidate");
                let validate_pending = store_pending
                    .project_store_validate_successor(&store_effect, &validate_effect)
                    .expect("project exact remote-Proposal Validate successor");
                let validate_replay = DurableValidateReplayEvidenceV1::remote_proposal(
                    stored_replay
                        .project_exact_validate(
                            &store_effect,
                            &durable_receipt,
                            &validate_effect,
                            &validate_pending,
                        )
                        .expect("project remote-Proposal Validate replay evidence"),
                );
                (
                    store_pending,
                    store_candidate,
                    validate_pending,
                    validate_replay,
                )
            };
        let validate_candidate = validate_replay
            .project_candidate_for_test(
                &verified,
                &validate_effect,
                &durable_receipt,
                &validate_pending,
            )
            .expect("canonical V1 evidence projects the Validate candidate fixture");
        let mut coordinator = LifecycleCoordinator::new(
            lifecycle_context(verified.context()),
            0,
            capacity_geometry(effect_limit),
        );
        let AdmissionDecision::Admitted {
            ordinal,
            producer_turn_ordinal: None,
            ..
        } = coordinator.admit(AdmissionRequest::Candidate(store_candidate))
        else {
            panic!("admit Store parent fixture")
        };
        let record = &coordinator.records[&ordinal];
        let ready = super::super::SchedulerReadyInputs::new(record, None, [0; 6]);
        let inputs = super::super::SchedulerInputs::new([], [(ordinal, ready)])
            .expect("unique Store scheduler census");
        let super::super::TurnPlan::Execute(lease) = coordinator.plan_turn(inputs) else {
            panic!("claim Store fixture")
        };
        StoreValidateFixture {
            coordinator,
            lease,
            verified,
            durable_receipt,
            store_effect,
            store_pending,
            validate_effect,
            validate_pending,
            validate_candidate,
            validate_replay,
        }
    }

    fn validate_apply_fixture(
        effect_limit: usize,
        inherited_commitment: bool,
    ) -> ValidateApplyFixture {
        validate_apply_fixture_with_authority(
            effect_limit,
            inherited_commitment.then_some(wire::GlobalPhase::Prepare),
        )
    }

    fn validate_apply_fixture_with_authority(
        effect_limit: usize,
        inherited_authority: Option<wire::GlobalPhase>,
    ) -> ValidateApplyFixture {
        let StoreValidateFixture {
            verified,
            durable_receipt,
            validate_effect,
            validate_pending,
            validate_candidate,
            validate_replay,
            ..
        } = store_validate_fixture_with_authority(effect_limit, inherited_authority);
        let AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        } = validate_effect
        else {
            unreachable!("Store successor fixture is one Validate effect")
        };
        let validate_effect = AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        };
        let apply_effect = AdapterEffect::Apply {
            tag,
            subject,
            certificate: wire::QuorumCertificate {
                round,
                proposal_round: round,
                phase: wire::GlobalPhase::Commit,
                subject,
                execution_commitment: fixture_execution_commitment(),
                signers: vec![0, 1, 2],
                aggregate_signature: vec![0xA5],
            },
        };
        let apply_pending = validate_pending
            .project_validate_apply_successor(&validate_effect, &apply_effect)
            .expect("project exact ordinal-free Apply successor");
        let validated_receipt = ValidatedBodyReceipt::for_test_with_commitment(
            durable_receipt.clone(),
            fixture_execution_commitment(),
        );
        let apply_candidate =
            super::super::replay_authority::exact_live_wal_body_successor_candidate_for_test(
                &verified,
                &validate_effect,
                &validate_pending,
                &apply_effect,
                &apply_pending,
                Some(&durable_receipt),
            )
            .expect("canonical live-WAL evidence projects the Apply candidate fixture");
        let mut coordinator = LifecycleCoordinator::new(
            lifecycle_context(verified.context()),
            0,
            capacity_geometry(effect_limit),
        );
        let AdmissionDecision::Admitted {
            ordinal,
            producer_turn_ordinal: None,
            ..
        } = coordinator.admit(AdmissionRequest::Candidate(validate_candidate))
        else {
            panic!("admit Validate parent fixture")
        };
        let record = &coordinator.records[&ordinal];
        let ready = super::super::SchedulerReadyInputs::new(record, Some(false), [0; 6]);
        let inputs = super::super::SchedulerInputs::new([], [(ordinal, ready)])
            .expect("unique Validate scheduler census");
        let super::super::TurnPlan::Execute(lease) = coordinator.plan_turn(inputs) else {
            panic!("claim Validate fixture")
        };
        ValidateApplyFixture {
            coordinator,
            lease,
            verified,
            validate_effect,
            validate_pending,
            validate_candidate,
            validate_replay,
            validated_receipt,
            apply_effect,
            apply_candidate,
        }
    }

    #[test]
    fn full_effect_capacity_stages_net_zero_success_and_drop_is_inert() {
        let FetchStoreFixture {
            mut coordinator,
            lease,
            verified,
            durable_receipt,
            store_candidate,
            ..
        } = fetch_store_fixture(1);
        assert_eq!(coordinator.capacity_used[&CapacityClass::Effect], 1);
        let before = format!("{coordinator:#?}");
        let expected_frame = DurablePayloadReference::BodyFrame(
            projection::durable_body_frame_reference(
                lifecycle_context(verified.context()),
                &durable_receipt,
            )
            .expect("durable Fetch completion projects one body frame"),
        );
        let prepared = prepare_authorized_body_transition(
            &mut coordinator,
            &lease,
            store_candidate,
            expected_frame,
            DurableContinuationEdge::FetchToStore,
        )
        .expect("Fetch release makes room for exact Store at full capacity");
        assert!(matches!(
            prepared.edge,
            DurableContinuationEdge::FetchToStore
        ));
        assert_eq!(prepared.parent_ordinal, lease.ordinal());
        assert_eq!(prepared.child_ordinal, lease.ordinal() + 1);
        assert_eq!(prepared.owner, lease.owner());
        assert_eq!(
            prepared.child_slot.capacity_class(),
            Some(CapacityClass::Effect)
        );
        assert_eq!(
            prepared.staged.capacity_used[&CapacityClass::Effect],
            1,
            "Fetch retirement and Store admission are net-zero"
        );
        assert_eq!(
            prepared.staged.records[&lease.ordinal()].state,
            LifecycleState::Terminal(TerminalOutcome::Advanced)
        );
        assert_eq!(
            prepared.staged.records[&prepared.child_ordinal].state,
            LifecycleState::Ready
        );
        assert_eq!(
            prepared.staged.records[&prepared.child_ordinal]
                .physical_slots
                .get(&prepared.child_slot),
            Some(&prepared.child_digest)
        );
        assert_eq!(
            prepared.staged.records[&prepared.child_ordinal]
                .episode
                .slot_universe,
            std::collections::BTreeSet::from([prepared.child_slot])
        );
        assert_eq!(
            prepared.staged.records[&prepared.child_ordinal]
                .episode
                .consumed_slots,
            std::collections::BTreeSet::from([prepared.child_slot])
        );
        assert_eq!(
            prepared.staged.durable_records[&prepared.parent_ordinal].payload,
            expected_frame
        );
        assert_eq!(
            prepared.staged.durable_records[&prepared.child_ordinal].payload,
            expected_frame
        );
        drop(prepared);
        assert_eq!(format!("{coordinator:#?}"), before);
    }

    #[test]
    fn wrong_and_stale_fetch_leases_reject_without_coordinator_mutation() {
        let FetchStoreFixture {
            mut coordinator,
            lease,
            store_candidate,
            ..
        } = fetch_store_fixture(1);
        let before = format!("{coordinator:#?}");
        let mut wrong = lease.clone();
        wrong.work_class = LifecycleWorkClass::Store;
        assert!(matches!(
            prepare_authorized_body_transition(
                &mut coordinator,
                &wrong,
                store_candidate.clone(),
                store_candidate.payload,
                DurableContinuationEdge::FetchToStore,
            ),
            Err(BodyStageTransitionError::WrongParentShape)
        ));
        assert_eq!(format!("{coordinator:#?}"), before);

        let mut stale = lease.clone();
        stale.id = super::super::LeaseId(lease.id().0 + 1);
        let parent_payload = store_candidate.payload;
        assert!(matches!(
            prepare_authorized_body_transition(
                &mut coordinator,
                &stale,
                store_candidate,
                parent_payload,
                DurableContinuationEdge::FetchToStore,
            ),
            Err(BodyStageTransitionError::StaleLease)
        ));
        assert_eq!(format!("{coordinator:#?}"), before);
    }

    #[test]
    fn foreign_store_projection_rejects_without_coordinator_mutation() {
        let FetchStoreFixture {
            coordinator,
            verified,
            durable_receipt,
            store_effect,
            store_pending,
            store_replay,
            ..
        } = fetch_store_fixture(1);
        let before = format!("{coordinator:#?}");
        let AdapterEffect::StoreBody {
            tag,
            round,
            mut subject,
        } = store_effect
        else {
            unreachable!("fixture Store effect")
        };
        subject.payload_hash = Hash::new(b"foreign Store body");
        let foreign = AdapterEffect::StoreBody {
            tag,
            round,
            subject,
        };
        assert!(matches!(
            store_replay.project_candidate_for_test(
                &verified,
                &foreign,
                &durable_receipt,
                &store_pending,
            ),
            Err(AdapterEffectAdmissionError::InvalidCarrier)
        ));
        assert_eq!(format!("{coordinator:#?}"), before);
    }

    #[test]
    fn fetch_store_rejects_a_foreign_body_receipt_without_mutation() {
        let FetchStoreFixture {
            coordinator,
            verified,
            durable_receipt,
            store_effect,
            store_pending,
            store_replay,
            ..
        } = fetch_store_fixture(1);
        let AdapterEffect::StoreBody { round, subject, .. } = &store_effect else {
            unreachable!("fixture retains one Store effect")
        };
        let foreign_round = wire::ConsensusRound {
            view: round.view + 1,
            ..*round
        };
        let foreign_receipt = DurableBodyReceipt::for_test(
            verified.context().id(),
            foreign_round,
            *subject,
            durable_receipt.manifest_hash(),
        );
        let before = format!("{coordinator:#?}");
        assert!(matches!(
            store_replay.project_candidate_for_test(
                &verified,
                &store_effect,
                &foreign_receipt,
                &store_pending,
            ),
            Err(AdapterEffectAdmissionError::InvalidCarrier)
        ));
        assert_eq!(format!("{coordinator:#?}"), before);
    }

    #[test]
    fn fetch_store_rejects_a_payload_free_parent_after_body_completion() {
        let FetchStoreFixture {
            mut coordinator,
            lease,
            store_candidate,
            ..
        } = fetch_store_fixture(1);
        coordinator
            .durable_records
            .get_mut(&lease.ordinal())
            .expect("claimed Fetch retains durable metadata")
            .payload = DurablePayloadReference::None;
        let before = format!("{coordinator:#?}");
        let parent_payload = store_candidate.payload;
        assert!(matches!(
            prepare_authorized_body_transition(
                &mut coordinator,
                &lease,
                store_candidate,
                parent_payload,
                DurableContinuationEdge::FetchToStore,
            ),
            Err(BodyStageTransitionError::InvalidBodyFrameReference)
        ));
        assert_eq!(format!("{coordinator:#?}"), before);
    }

    #[test]
    fn staged_capacity_wait_leaves_fetch_parent_claimed() {
        let FetchStoreFixture {
            mut coordinator,
            lease,
            store_candidate,
            ..
        } = fetch_store_fixture(1);
        coordinator
            .capacity_geometry
            .limits
            .insert(CapacityClass::Effect, 0);
        let before_capacity = format!("{coordinator:#?}");
        let parent_payload = store_candidate.payload;
        assert!(matches!(
            prepare_authorized_body_transition(
                &mut coordinator,
                &lease,
                store_candidate,
                parent_payload,
                DurableContinuationEdge::FetchToStore,
            ),
            Err(BodyStageTransitionError::ChildAdmission(decision))
                if matches!(*decision, AdmissionDecision::WaitForCapacity(_))
        ));
        assert_eq!(format!("{coordinator:#?}"), before_capacity);
        assert_eq!(coordinator.active_lease, Some(lease));
    }

    #[test]
    fn fetch_store_rejects_max_high_water_without_mutation() {
        let FetchStoreFixture {
            mut coordinator,
            lease,
            store_candidate,
            ..
        } = fetch_store_fixture(1);
        coordinator.high_water = u128::MAX;
        let before_ordinal = format!("{coordinator:#?}");
        let parent_payload = store_candidate.payload;
        assert!(matches!(
            prepare_authorized_body_transition(
                &mut coordinator,
                &lease,
                store_candidate,
                parent_payload,
                DurableContinuationEdge::FetchToStore,
            ),
            Err(BodyStageTransitionError::OrdinalExhausted)
        ));
        assert_eq!(format!("{coordinator:#?}"), before_ordinal);
        assert_eq!(coordinator.active_lease, Some(lease));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn full_effect_capacity_stages_exact_store_validate_cut_and_drop_is_inert() {
        let StoreValidateFixture {
            mut coordinator,
            lease,
            validate_pending,
            validate_candidate,
            ..
        } = store_validate_fixture(1, true);
        let expected_frame = validate_candidate.payload;
        let capacity_used_before = coordinator.capacity_used.clone();
        let capacity_generation_before = coordinator.capacity_generation.clone();
        let before = format!("{coordinator:#?}");
        let prepared = prepare_authorized_body_transition(
            &mut coordinator,
            &lease,
            validate_candidate.clone(),
            expected_frame,
            DurableContinuationEdge::StoreToValidate,
        )
        .expect("Store release makes room for exact Validate at full capacity");

        assert!(matches!(
            prepared.edge,
            DurableContinuationEdge::StoreToValidate
        ));
        assert_eq!(prepared.parent_ordinal, lease.ordinal());
        assert_eq!(prepared.child_ordinal, lease.ordinal() + 1);
        assert_eq!(prepared.owner, lease.owner());
        assert_eq!(prepared.staged.high_water, prepared.child_ordinal);
        assert!(prepared.staged.active_lease.is_none());
        assert_eq!(
            prepared.child_slot,
            PhysicalSlotId::for_capacity(CapacityClass::Effect, 0)
        );
        assert_eq!(
            prepared.child_digest,
            digest_from_hash(validate_pending.exact_effect_identity())
        );

        let parent = &prepared.staged.records[&prepared.parent_ordinal];
        assert_eq!(parent.ordinal, prepared.parent_ordinal);
        assert_eq!(parent.owner, lease.owner());
        assert_eq!(parent.key, lease.key());
        assert_eq!(parent.work_class, LifecycleWorkClass::Store);
        assert_eq!(parent.stage.kind(), LifecycleStageKind::StoreBody);
        assert_eq!(
            parent.state,
            LifecycleState::Terminal(TerminalOutcome::Advanced)
        );
        assert_eq!(parent.physical_slots, *lease.physical_slots());
        let parent_slots = lease
            .physical_slots()
            .keys()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(parent.episode.slot_universe, parent_slots);
        assert_eq!(parent.episode.consumed_slots, parent_slots);
        assert!(!prepared.staged.ready_index.contains(&parent.ordinal));
        assert_eq!(
            prepared.staged.key_index.get(&parent.key),
            Some(&parent.ordinal)
        );
        let parent_metadata = &prepared.staged.durable_records[&parent.ordinal];
        assert_eq!(
            parent_metadata.reconstruction_source,
            parent.owner.causal_root().digest()
        );
        assert_eq!(parent_metadata.payload, expected_frame);

        let child = &prepared.staged.records[&prepared.child_ordinal];
        assert_eq!(child.ordinal, prepared.child_ordinal);
        assert_eq!(child.owner, lease.owner());
        assert_eq!(child.key, validate_candidate.key);
        assert_eq!(child.work_class, LifecycleWorkClass::Validate);
        assert_eq!(child.stage.kind(), LifecycleStageKind::ValidateBody);
        assert_eq!(child.state, LifecycleState::Ready);
        assert_eq!(
            child.physical_slots,
            std::collections::BTreeMap::from([(prepared.child_slot, prepared.child_digest)])
        );
        assert_eq!(
            child.episode.slot_universe,
            std::collections::BTreeSet::from([prepared.child_slot])
        );
        assert_eq!(
            child.episode.consumed_slots,
            std::collections::BTreeSet::from([prepared.child_slot])
        );
        assert!(prepared.staged.ready_index.contains(&child.ordinal));
        assert_eq!(
            prepared.staged.key_index.get(&child.key),
            Some(&child.ordinal)
        );
        assert_eq!(
            prepared.staged.owner_index.get(&child.owner.causal_root()),
            Some(&child.owner)
        );
        assert!(
            prepared.staged.durable_records[&child.ordinal].matches_admission(&validate_candidate)
        );
        assert_eq!(
            prepared.staged.durable_records[&child.ordinal].payload, parent_metadata.payload,
            "Store and Validate retain one byte-identical body frame"
        );
        assert_eq!(child.key.context(), parent.key.context());
        assert_eq!(child.key.round(), parent.key.round());
        assert_eq!(child.key.proposal_round(), parent.key.proposal_round());
        assert_eq!(child.key.subject(), parent.key.subject());
        assert_eq!(
            child.key.execution_commitment(),
            parent.key.execution_commitment()
        );
        assert_eq!(parent.key.phase(), LifecyclePhase::Store);
        assert_eq!(child.key.phase(), LifecyclePhase::Validate);

        assert_eq!(
            prepared.staged.capacity_used[&CapacityClass::Effect],
            capacity_used_before[&CapacityClass::Effect]
        );
        assert_eq!(
            prepared.staged.capacity_generation[&CapacityClass::Effect],
            capacity_generation_before[&CapacityClass::Effect] + 1
        );
        for class in CapacityClass::ALL
            .into_iter()
            .filter(|class| *class != CapacityClass::Effect)
        {
            assert_eq!(
                prepared.staged.capacity_used[&class],
                capacity_used_before[&class]
            );
            assert_eq!(
                prepared.staged.capacity_generation[&class],
                capacity_generation_before[&class]
            );
        }

        drop(prepared);
        assert_eq!(format!("{coordinator:#?}"), before);
    }

    #[test]
    fn store_validate_accepts_exact_no_commitment_lineage() {
        let StoreValidateFixture {
            mut coordinator,
            lease,
            validate_candidate,
            ..
        } = store_validate_fixture(1, false);
        assert_eq!(lease.key().execution_commitment(), None);
        let before = format!("{coordinator:#?}");
        let parent_payload = validate_candidate.payload;
        let prepared = prepare_authorized_body_transition(
            &mut coordinator,
            &lease,
            validate_candidate,
            parent_payload,
            DurableContinuationEdge::StoreToValidate,
        )
        .expect("ordinary body statement retains its exact absent commitment");
        assert_eq!(
            prepared.staged.records[&prepared.child_ordinal]
                .key
                .execution_commitment(),
            None
        );
        drop(prepared);
        assert_eq!(format!("{coordinator:#?}"), before);
    }

    #[test]
    fn store_validate_rejects_a_substituted_frame_without_mutation() {
        let StoreValidateFixture {
            mut coordinator,
            lease,
            durable_receipt,
            validate_candidate,
            ..
        } = store_validate_fixture(1, true);
        let substituted = DurableBodyReceipt::for_test(
            durable_receipt.context_id(),
            durable_receipt.round(),
            durable_receipt.subject(),
            HashOf::from_untyped_unchecked(Hash::new(b"substituted body manifest")),
        );
        let substituted_payload = DurablePayloadReference::BodyFrame(
            projection::durable_body_frame_reference(coordinator.active_context, &substituted)
                .expect("substituted receipt still projects a structurally valid frame"),
        );
        let before = format!("{coordinator:#?}");
        assert!(matches!(
            prepare_authorized_body_transition(
                &mut coordinator,
                &lease,
                validate_candidate,
                substituted_payload,
                DurableContinuationEdge::StoreToValidate,
            ),
            Err(BodyStageTransitionError::InvalidBodyFrameReference)
        ));
        assert_eq!(format!("{coordinator:#?}"), before);
    }

    #[test]
    fn wrong_and_stale_store_leases_reject_without_coordinator_mutation() {
        let StoreValidateFixture {
            mut coordinator,
            lease,
            validate_candidate,
            ..
        } = store_validate_fixture(1, true);
        let before = format!("{coordinator:#?}");
        let mut wrong = lease.clone();
        wrong.work_class = LifecycleWorkClass::Validate;
        assert!(matches!(
            prepare_authorized_body_transition(
                &mut coordinator,
                &wrong,
                validate_candidate.clone(),
                validate_candidate.payload,
                DurableContinuationEdge::StoreToValidate,
            ),
            Err(BodyStageTransitionError::WrongParentShape)
        ));
        assert_eq!(format!("{coordinator:#?}"), before);

        let mut stale = lease.clone();
        stale.id = super::super::LeaseId(lease.id().0 + 1);
        assert!(matches!(
            prepare_authorized_body_transition(
                &mut coordinator,
                &stale,
                validate_candidate.clone(),
                validate_candidate.payload,
                DurableContinuationEdge::StoreToValidate,
            ),
            Err(BodyStageTransitionError::StaleLease)
        ));
        assert_eq!(format!("{coordinator:#?}"), before);
    }

    #[test]
    fn wrong_validate_effect_binding_and_owner_reject_without_mutation() {
        let StoreValidateFixture {
            mut coordinator,
            lease,
            verified,
            durable_receipt,
            store_effect,
            store_pending,
            validate_effect,
            validate_pending,
            validate_candidate,
            validate_replay,
        } = store_validate_fixture(1, true);
        let before = format!("{coordinator:#?}");
        let (tag, round, mut wrong_subject) = match &validate_effect {
            AdapterEffect::ValidateBody {
                tag,
                round,
                subject,
            } => (*tag, *round, *subject),
            _ => unreachable!("fixture Validate effect"),
        };
        wrong_subject.payload_hash = Hash::new(b"foreign Validate body");
        let wrong_effect = AdapterEffect::ValidateBody {
            tag,
            round,
            subject: wrong_subject,
        };
        assert!(matches!(
            validate_replay.project_candidate_for_test(
                &verified,
                &wrong_effect,
                &durable_receipt,
                &validate_pending,
            ),
            Err(AdapterEffectAdmissionError::InvalidCarrier)
        ));
        assert_eq!(format!("{coordinator:#?}"), before);
        assert!(matches!(
            validate_replay.project_candidate_for_test(
                &verified,
                &validate_effect,
                &durable_receipt,
                &store_pending,
            ),
            Err(AdapterEffectAdmissionError::InvalidCarrier)
        ));
        assert_eq!(format!("{coordinator:#?}"), before);

        let foreign_owner_tag = EventTag::new(
            tag.height(),
            tag.view(),
            Generation::new(
                tag.generation()
                    .get()
                    .checked_add(1)
                    .expect("fixture generation remains bounded"),
            ),
        );
        let foreign_store_owner = bind_adapter_effect_batch_ownership(
            core::slice::from_ref(&store_effect),
            vec![RuntimeEffectOwnership::fresh_for_test(
                foreign_owner_tag,
                99,
            )],
        )
        .expect("bind foreign Store owner")
        .pop()
        .expect("one foreign Store owner");
        let foreign_store_pending = foreign_store_owner
            .pending_adapter_effect_binding(&store_effect)
            .expect("mint foreign Store pending binding");
        let foreign_validate_pending = foreign_store_pending
            .project_store_validate_successor(&store_effect, &validate_effect)
            .expect("project foreign Validate pending binding");
        assert_ne!(
            foreign_validate_pending.causal_lifecycle_key(),
            validate_pending.causal_lifecycle_key()
        );
        let parent_payload = validate_candidate.payload;
        let mut foreign_candidate = validate_candidate;
        foreign_candidate.causal_root = super::super::CausalRoot::new(digest_from_hash(
            foreign_validate_pending.causal_lifecycle_key(),
        ));
        assert!(matches!(
            prepare_authorized_body_transition(
                &mut coordinator,
                &lease,
                foreign_candidate,
                parent_payload,
                DurableContinuationEdge::StoreToValidate,
            ),
            Err(BodyStageTransitionError::ForeignSuccessorOwner)
        ));
        assert_eq!(format!("{coordinator:#?}"), before);
    }

    #[test]
    fn foreign_store_lineage_rejects_without_mutation() {
        let StoreValidateFixture {
            mut coordinator,
            mut lease,
            validate_candidate,
            ..
        } = store_validate_fixture(1, true);
        let incumbent_key = lease.key();
        let foreign_key = super::super::LifecycleKey::new(
            incumbent_key.context(),
            incumbent_key.round(),
            incumbent_key.proposal_round(),
            Some(super::super::LifecycleDigest::new([0xF1; 32])),
            LifecyclePhase::Store,
            incumbent_key.execution_commitment(),
        );
        lease.key = foreign_key;
        coordinator.active_lease = Some(lease.clone());
        assert_eq!(
            coordinator.key_index.remove(&incumbent_key),
            Some(lease.ordinal())
        );
        assert_eq!(
            coordinator.key_index.insert(foreign_key, lease.ordinal()),
            None
        );
        coordinator
            .records
            .get_mut(&lease.ordinal())
            .expect("claimed Store record")
            .key = foreign_key;
        let before = format!("{coordinator:#?}");
        assert!(matches!(
            prepare_authorized_body_transition(
                &mut coordinator,
                &lease,
                validate_candidate.clone(),
                validate_candidate.payload,
                DurableContinuationEdge::StoreToValidate,
            ),
            Err(BodyStageTransitionError::ForeignSuccessorLineage)
        ));
        assert_eq!(format!("{coordinator:#?}"), before);
    }

    #[test]
    fn store_validate_rejects_max_high_water_without_mutation() {
        let StoreValidateFixture {
            mut coordinator,
            lease,
            validate_candidate,
            ..
        } = store_validate_fixture(1, true);
        coordinator.high_water = u128::MAX;
        let before = format!("{coordinator:#?}");
        assert!(matches!(
            prepare_authorized_body_transition(
                &mut coordinator,
                &lease,
                validate_candidate.clone(),
                validate_candidate.payload,
                DurableContinuationEdge::StoreToValidate,
            ),
            Err(BodyStageTransitionError::OrdinalExhausted)
        ));
        assert_eq!(format!("{coordinator:#?}"), before);
        assert_eq!(coordinator.active_lease, Some(lease));
    }

    #[test]
    fn store_validate_rejects_capacity_generation_overflow_without_mutation() {
        let StoreValidateFixture {
            mut coordinator,
            lease,
            validate_candidate,
            ..
        } = store_validate_fixture(1, true);
        coordinator
            .capacity_generation
            .insert(CapacityClass::Effect, u64::MAX);
        let before = format!("{coordinator:#?}");
        assert!(matches!(
            prepare_authorized_body_transition(
                &mut coordinator,
                &lease,
                validate_candidate.clone(),
                validate_candidate.payload,
                DurableContinuationEdge::StoreToValidate,
            ),
            Err(BodyStageTransitionError::InvalidCapacityTransition)
        ));
        assert_eq!(format!("{coordinator:#?}"), before);
        assert_eq!(coordinator.active_lease, Some(lease));
    }

    #[test]
    fn corrupt_store_reconstruction_source_rejects_without_mutation() {
        let StoreValidateFixture {
            mut coordinator,
            lease,
            validate_candidate,
            ..
        } = store_validate_fixture(1, true);
        let corrupt = super::super::LifecycleDigest::new([0xD3; 32]);
        assert_ne!(corrupt, lease.owner().causal_root().digest());
        coordinator
            .durable_records
            .get_mut(&lease.ordinal())
            .expect("Store durable metadata")
            .reconstruction_source = corrupt;
        let before = format!("{coordinator:#?}");
        assert!(matches!(
            prepare_authorized_body_transition(
                &mut coordinator,
                &lease,
                validate_candidate.clone(),
                validate_candidate.payload,
                DurableContinuationEdge::StoreToValidate,
            ),
            Err(BodyStageTransitionError::StaleLease)
        ));
        assert_eq!(format!("{coordinator:#?}"), before);
        assert_eq!(coordinator.active_lease, Some(lease));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn full_effect_capacity_stages_exact_validate_apply_cut_and_drop_is_inert() {
        let ValidateApplyFixture {
            mut coordinator,
            lease,
            validated_receipt,
            apply_effect,
            apply_candidate,
            ..
        } = validate_apply_fixture(1, true);
        let expected_frame = apply_candidate.payload;
        let capacity_used_before = coordinator.capacity_used.clone();
        let capacity_generation_before = coordinator.capacity_generation.clone();
        let before = format!("{coordinator:#?}");
        let prepared = prepare_authorized_validate_apply_transition(
            &mut coordinator,
            &lease,
            &validated_receipt,
            &apply_effect,
            apply_candidate.clone(),
        )
        .expect("Validate release makes room for exact Apply at full capacity");

        assert!(matches!(
            prepared.edge,
            DurableContinuationEdge::ValidateToApply
        ));
        assert_eq!(prepared.parent_ordinal, lease.ordinal());
        assert_eq!(prepared.child_ordinal, lease.ordinal() + 1);
        assert_eq!(prepared.owner, lease.owner());
        let parent = &prepared.staged.records[&prepared.parent_ordinal];
        let child = &prepared.staged.records[&prepared.child_ordinal];
        assert_eq!(parent.work_class, LifecycleWorkClass::Validate);
        assert_eq!(parent.stage.kind(), LifecycleStageKind::ValidateBody);
        assert_eq!(
            parent.state,
            LifecycleState::Terminal(TerminalOutcome::Advanced)
        );
        assert_eq!(child.owner, lease.owner());
        assert_eq!(child.key, apply_candidate.key);
        assert_eq!(child.work_class, LifecycleWorkClass::Apply);
        assert_eq!(child.stage.kind(), LifecycleStageKind::ApplyDecision);
        assert_eq!(child.state, LifecycleState::Ready);
        assert_eq!(child.key.context(), parent.key.context());
        assert_eq!(child.key.round(), parent.key.round());
        assert_eq!(child.key.proposal_round(), parent.key.proposal_round());
        assert_eq!(child.key.subject(), parent.key.subject());
        assert_eq!(
            child.key.execution_commitment(),
            parent.key.execution_commitment()
        );
        assert!(child.key.execution_commitment().is_some());
        assert_eq!(
            child.physical_slots,
            std::collections::BTreeMap::from([(prepared.child_slot, prepared.child_digest)])
        );
        assert!(prepared.staged.ready_index.contains(&child.ordinal));
        assert!(
            prepared.staged.durable_records[&child.ordinal].matches_admission(&apply_candidate)
        );
        assert_eq!(
            prepared.staged.durable_records[&parent.ordinal].payload,
            expected_frame
        );
        assert_eq!(
            prepared.staged.durable_records[&child.ordinal].payload, expected_frame,
            "Validate and Apply retain one byte-identical body frame"
        );
        assert_eq!(
            prepared.staged.durable_records[&parent.ordinal].continuation,
            DurableContinuation::successor(DurableContinuationEdge::ValidateToApply, child.ordinal,)
        );
        assert_eq!(
            prepared.staged.capacity_used[&CapacityClass::Effect],
            capacity_used_before[&CapacityClass::Effect]
        );
        assert_eq!(
            prepared.staged.capacity_generation[&CapacityClass::Effect],
            capacity_generation_before[&CapacityClass::Effect] + 1
        );
        drop(prepared);
        assert_eq!(format!("{coordinator:#?}"), before);
    }

    #[allow(clippy::too_many_lines)]
    fn assert_validate_sign_transition_is_exact(phase: wire::GlobalPhase) {
        let inherited = match phase {
            wire::GlobalPhase::Prepare => None,
            wire::GlobalPhase::Commit => Some(wire::GlobalPhase::Prepare),
        };
        let ValidateApplyFixture {
            mut coordinator,
            lease,
            verified,
            validate_effect,
            validate_pending,
            validated_receipt,
            ..
        } = validate_apply_fixture_with_authority(1, inherited);
        let AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        } = &validate_effect
        else {
            unreachable!("fixture retains one Validate effect")
        };
        let (tag, round, subject) = (*tag, *round, *subject);
        let sign_effect = AdapterEffect::Sign {
            tag,
            request: SignRequest::Vote(wire::Vote {
                round,
                proposal_round: round,
                phase,
                subject,
                execution_commitment: validated_receipt.execution_commitment(),
                signer: 0,
                signature: Vec::new(),
            }),
        };
        let sign_pending = match phase {
            wire::GlobalPhase::Prepare => validate_pending
                .project_validate_sign_prepare_successor(&validate_effect, &sign_effect),
            wire::GlobalPhase::Commit => validate_pending
                .project_validate_sign_commit_successor(&validate_effect, &sign_effect),
        }
        .expect("sealed Validate authority projects its exact Sign successor");
        let expected_edge = match phase {
            wire::GlobalPhase::Prepare => DurableContinuationEdge::ValidateToSignPrepare,
            wire::GlobalPhase::Commit => DurableContinuationEdge::ValidateToSignCommit,
        };
        let expected_stage = match phase {
            wire::GlobalPhase::Prepare => LifecycleStageKind::SignPrepareVote,
            wire::GlobalPhase::Commit => LifecycleStageKind::SignCommitVote,
        };
        let before = format!("{coordinator:#?}");
        let effect_used_before = coordinator.capacity_used[&CapacityClass::Effect];
        let effect_generation_before = coordinator.capacity_generation[&CapacityClass::Effect];
        let sign_candidate =
            super::super::replay_authority::exact_live_wal_body_successor_candidate_for_test(
                &verified,
                &validate_effect,
                &validate_pending,
                &sign_effect,
                &sign_pending,
                None,
            )
            .expect("canonical live-WAL evidence projects the Sign candidate");
        let parent_payload = DurablePayloadReference::BodyFrame(
            projection::durable_body_frame_reference(
                coordinator.active_context,
                validated_receipt.durable(),
            )
            .expect("Validate fixture retains one exact body frame"),
        );
        let prepared = prepare_authorized_body_transition(
            &mut coordinator,
            &lease,
            sign_candidate,
            parent_payload,
            expected_edge,
        )
        .expect("stage exact Validate-to-Sign durable cut");
        assert_eq!(prepared.edge, expected_edge);
        assert_eq!(prepared.parent_ordinal, lease.ordinal());
        assert_eq!(prepared.child_ordinal, lease.ordinal() + 1);
        assert_eq!(prepared.owner, lease.owner());
        assert_eq!(
            prepared.staged.records[&lease.ordinal()].state,
            LifecycleState::Terminal(TerminalOutcome::Advanced)
        );
        let child = &prepared.staged.records[&prepared.child_ordinal];
        assert_eq!(child.owner, lease.owner());
        assert_eq!(child.work_class, LifecycleWorkClass::SignVote);
        assert_eq!(child.stage.kind(), expected_stage);
        assert_eq!(child.state, LifecycleState::Ready);
        assert_eq!(
            prepared.staged.durable_records[&lease.ordinal()].continuation,
            DurableContinuation::successor(expected_edge, child.ordinal)
        );
        assert!(matches!(
            prepared.staged.durable_records[&lease.ordinal()].payload,
            DurablePayloadReference::BodyFrame(_)
        ));
        assert_eq!(
            prepared.staged.durable_records[&child.ordinal].payload,
            DurablePayloadReference::None
        );
        assert_eq!(
            prepared.staged.capacity_used[&CapacityClass::Effect],
            effect_used_before
        );
        assert_eq!(
            prepared.staged.capacity_generation[&CapacityClass::Effect],
            effect_generation_before + 1
        );
        super::super::ledger::LifecycleLedgerV1::from_coordinator(&prepared.staged)
            .expect("typed Validate-to-Sign edge projects into LedgerV1");
        drop(prepared);
        assert_eq!(format!("{coordinator:#?}"), before);
    }

    #[test]
    fn validate_sign_prepare_and_commit_stage_exact_net_zero_cuts() {
        assert_validate_sign_transition_is_exact(wire::GlobalPhase::Prepare);
        assert_validate_sign_transition_is_exact(wire::GlobalPhase::Commit);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn rejected_validate_reservation_converts_into_exact_report_capacity() {
        // The complete sealed report wrapper spans private adapter and registry
        // fixture state owned by their respective modules. Adding a test-only
        // constructor here would reopen the boundary this tranche closes.
        // Adapter tests cover exact registered-Prepare report preview/drop,
        // registry tests cover exact Ready carrier and foreign-height rejection,
        // and this test joins canonical report evidence to the shared staging
        // core while proving raw report admission is rejected and drop is inert.
        let ValidateApplyFixture {
            mut coordinator,
            mut lease,
            verified,
            validate_effect,
            validate_pending,
            validate_replay,
            validated_receipt,
            ..
        } = validate_apply_fixture_with_authority(1, Some(wire::GlobalPhase::Prepare));
        let AdapterEffect::ValidateBody {
            tag: _,
            round,
            subject,
        } = &validate_effect
        else {
            unreachable!("fixture retains one Validate effect")
        };
        let report_effect = AdapterEffect::ReportInvalidCertifiedBody {
            subject: *subject,
            certificate: wire::QuorumCertificate {
                round: *round,
                proposal_round: *round,
                phase: wire::GlobalPhase::Prepare,
                subject: *subject,
                execution_commitment: validated_receipt.execution_commitment(),
                signers: vec![0, 1, 2],
                aggregate_signature: vec![0x51],
            },
        };
        let report_pending = validate_pending
            .project_validate_report_invalid_certified_body_successor(
                &validate_effect,
                &report_effect,
            )
            .expect("Prepare-authorized Validate projects its exact report");
        lease.output_reservation = Some(super::super::schema::LeaseCapacityReservation::new(
            CapacityClass::Consensus,
            coordinator.capacity_generation[&CapacityClass::Consensus],
        ));
        coordinator.active_lease = Some(lease.clone());
        let effect_used_before = coordinator.capacity_used[&CapacityClass::Effect];
        let effect_generation_before = coordinator.capacity_generation[&CapacityClass::Effect];
        let consensus_used_before = coordinator.capacity_used[&CapacityClass::Consensus];
        let consensus_generation_before =
            coordinator.capacity_generation[&CapacityClass::Consensus];
        let before = format!("{coordinator:#?}");
        let report_candidate =
            super::super::replay_authority::exact_invalid_body_report_candidate_for_test(
                &verified,
                &validate_replay,
                &validate_effect,
                &validate_pending,
                validated_receipt.durable(),
                &report_effect,
                &report_pending,
            )
            .expect("canonical rejection evidence projects the report candidate");
        let parent_payload = DurablePayloadReference::BodyFrame(
            projection::durable_body_frame_reference(
                coordinator.active_context,
                validated_receipt.durable(),
            )
            .expect("rejected Validate fixture retains one exact body frame"),
        );
        let prepared = prepare_authorized_body_transition(
            &mut coordinator,
            &lease,
            report_candidate,
            parent_payload,
            DurableContinuationEdge::ValidateToInvalidBodyReport,
        )
        .expect("convert the reserved rejected Validate into one report child");
        let child = &prepared.staged.records[&prepared.child_ordinal];
        assert_eq!(child.work_class, LifecycleWorkClass::InvalidBodyReport);
        assert_eq!(child.stage.kind(), LifecycleStageKind::ReportInvalidBody);
        assert_eq!(child.state, LifecycleState::Ready);
        assert_eq!(child.owner, lease.owner());
        assert_eq!(
            prepared.child_slot.capacity_class(),
            Some(CapacityClass::Consensus)
        );
        assert_eq!(
            prepared.staged.durable_records[&lease.ordinal()].continuation,
            DurableContinuation::successor(
                DurableContinuationEdge::ValidateToInvalidBodyReport,
                child.ordinal,
            )
        );
        assert!(matches!(
            prepared.staged.durable_records[&lease.ordinal()].payload,
            DurablePayloadReference::BodyFrame(_)
        ));
        assert_eq!(
            prepared.staged.durable_records[&child.ordinal].payload,
            DurablePayloadReference::None
        );
        assert_eq!(
            prepared.staged.capacity_used[&CapacityClass::Effect],
            effect_used_before - 1
        );
        assert_eq!(
            prepared.staged.capacity_generation[&CapacityClass::Effect],
            effect_generation_before + 1
        );
        assert_eq!(
            prepared.staged.capacity_used[&CapacityClass::Consensus],
            consensus_used_before + 1
        );
        assert_eq!(
            prepared.staged.capacity_generation[&CapacityClass::Consensus],
            consensus_generation_before
        );
        super::super::ledger::LifecycleLedgerV1::from_coordinator(&prepared.staged)
            .expect("typed Validate-to-report edge projects into LedgerV1");
        drop(prepared);
        assert_eq!(format!("{coordinator:#?}"), before);
    }

    fn assert_validate_no_successor_cut_is_exact(rejected: bool) {
        let ValidateApplyFixture {
            mut coordinator,
            mut lease,
            validated_receipt,
            ..
        } = validate_apply_fixture(1, false);
        if rejected {
            lease.output_reservation = Some(super::super::schema::LeaseCapacityReservation::new(
                CapacityClass::Consensus,
                coordinator.capacity_generation[&CapacityClass::Consensus],
            ));
            coordinator.active_lease = Some(lease.clone());
        }
        let effect_used_before = coordinator.capacity_used[&CapacityClass::Effect];
        let effect_generation_before = coordinator.capacity_generation[&CapacityClass::Effect];
        let consensus_used_before = coordinator.capacity_used[&CapacityClass::Consensus];
        let consensus_generation_before =
            coordinator.capacity_generation[&CapacityClass::Consensus];
        let high_water_before = coordinator.high_water;
        let before = format!("{coordinator:#?}");
        let parent_payload = DurablePayloadReference::BodyFrame(
            projection::durable_body_frame_reference(
                coordinator.active_context,
                validated_receipt.durable(),
            )
            .expect("terminal Validate fixture retains its exact body frame"),
        );
        let transition =
            stage_validate_no_successor_transition(&coordinator, &lease, parent_payload, rejected)
                .expect("stage exact terminal Validate with no successor");
        assert_eq!(transition.parent_ordinal, lease.ordinal());
        assert_eq!(transition.released_consensus_reservation, rejected);
        assert_eq!(transition.staged.high_water, high_water_before);
        assert_eq!(
            transition.staged.records[&lease.ordinal()].state,
            LifecycleState::Terminal(TerminalOutcome::Advanced)
        );
        assert_eq!(
            transition.staged.durable_records[&lease.ordinal()].continuation,
            DurableContinuation::AdvancedNoSuccessor
        );
        assert_eq!(
            transition.staged.durable_records[&lease.ordinal()].payload,
            DurablePayloadReference::BodyFrame(
                projection::durable_body_frame_reference(
                    coordinator.active_context,
                    validated_receipt.durable(),
                )
                .expect("terminal Validate retains its exact body frame"),
            )
        );
        assert_eq!(
            transition.staged.capacity_used[&CapacityClass::Effect],
            effect_used_before - 1
        );
        assert_eq!(
            transition.staged.capacity_generation[&CapacityClass::Effect],
            effect_generation_before + 1
        );
        assert_eq!(
            transition.staged.capacity_used[&CapacityClass::Consensus],
            consensus_used_before
        );
        assert_eq!(
            transition.staged.capacity_generation[&CapacityClass::Consensus],
            consensus_generation_before + u64::from(rejected)
        );
        super::super::ledger::LifecycleLedgerV1::from_coordinator(&transition.staged)
            .expect("typed Validate no-successor tombstone projects into LedgerV1");
        drop(transition);
        assert_eq!(format!("{coordinator:#?}"), before);
    }

    #[test]
    fn validated_and_rejected_no_effect_cuts_release_exact_capacity() {
        // The registry test pins all four accepted preview discriminators and
        // rejects every Busy/Apply/Persist/Report branch. This lower-level cut
        // then proves the permit-bound projection's only two capacity outcomes
        // without adding a test constructor for the private dual-borrow preview.
        assert_validate_no_successor_cut_is_exact(false);
        assert_validate_no_successor_cut_is_exact(true);

        let ValidateApplyFixture {
            coordinator,
            lease,
            validated_receipt,
            ..
        } = validate_apply_fixture(1, false);
        let parent_payload = DurablePayloadReference::BodyFrame(
            projection::durable_body_frame_reference(
                coordinator.active_context,
                validated_receipt.durable(),
            )
            .expect("terminal Validate fixture retains its exact body frame"),
        );
        assert!(matches!(
            stage_validate_no_successor_transition(&coordinator, &lease, parent_payload, true,),
            Err(BodyStageTransitionError::InvalidOutputReservation)
        ));
    }

    #[test]
    fn validate_apply_acquires_commit_authority_for_ordinary_validation() {
        let ValidateApplyFixture {
            mut coordinator,
            lease,
            validated_receipt,
            apply_effect,
            apply_candidate,
            ..
        } = validate_apply_fixture(1, false);
        assert_eq!(lease.key().execution_commitment(), None);
        let before = format!("{coordinator:#?}");
        let prepared = prepare_authorized_validate_apply_transition(
            &mut coordinator,
            &lease,
            &validated_receipt,
            &apply_effect,
            apply_candidate,
        )
        .expect("ordinary Validate may acquire exact Commit authority");
        assert!(
            prepared.staged.records[&prepared.child_ordinal]
                .key
                .execution_commitment()
                .is_some()
        );
        drop(prepared);
        assert_eq!(format!("{coordinator:#?}"), before);
    }

    #[test]
    fn advanced_validate_link_stutters_and_recovers_its_exact_apply() {
        let ValidateApplyFixture {
            mut coordinator,
            lease,
            validate_candidate,
            validated_receipt,
            apply_effect,
            apply_candidate,
            ..
        } = validate_apply_fixture(1, true);
        let retry = AdmissionRequest::Candidate(validate_candidate);
        let mut prepared = prepare_authorized_validate_apply_transition(
            &mut coordinator,
            &lease,
            &validated_receipt,
            &apply_effect,
            apply_candidate,
        )
        .expect("stage exact durable Validate-to-Apply link");
        assert!(matches!(
            prepared.staged.admit(retry),
            AdmissionDecision::StutterTerminal { owner } if owner == lease.owner()
        ));

        let ledger = super::super::ledger::LifecycleLedgerV1::from_coordinator(&prepared.staged)
            .expect("project linked body rows into LedgerV1");
        let physical_universes = prepared
            .staged
            .records
            .iter()
            .map(|(ordinal, record)| (*ordinal, record.episode.slot_universe.clone()))
            .collect();
        let snapshot = ledger
            .recovery_snapshot(physical_universes)
            .expect("decode an authenticated linked recovery snapshot");
        let authority = prepared.staged.episode_authority.clone();
        let mut recovered =
            LifecycleCoordinator::new_with_authority(authority.clone(), snapshot.high_water);
        recovered.reconcile_restart(snapshot.clone());
        assert_eq!(recovered.fault(), None);
        assert_eq!(
            recovered.durable_records[&lease.ordinal()].continuation,
            DurableContinuation::successor(
                DurableContinuationEdge::ValidateToApply,
                lease.ordinal() + 1,
            )
        );

        let mut missing_link = snapshot;
        missing_link
            .records
            .iter_mut()
            .find(|record| record.ordinal == lease.ordinal())
            .expect("recovery contains terminal Validate parent")
            .continuation = DurableContinuation::None;
        let mut rejected =
            LifecycleCoordinator::new_with_authority(authority, missing_link.high_water);
        rejected.reconcile_restart(missing_link);
        assert_eq!(rejected.fault(), Some(CoordinatorFault::RecoveryRejected));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn durable_open_joins_terminal_validate_to_authenticated_apply() {
        let temporary = tempfile::tempdir().expect("temporary lifecycle roots");
        let ledger_root = temporary.path().join("ledger");
        let body_root = temporary.path().join("bodies");
        let missing_payload_root = temporary.path().join("missing-payloads");
        let exact_payload_root = temporary.path().join("exact-payloads");
        let ValidateApplyFixture {
            mut coordinator,
            lease,
            verified,
            validated_receipt,
            apply_effect,
            apply_candidate,
            ..
        } = validate_apply_fixture(1, true);
        let prepared = prepare_authorized_validate_apply_transition(
            &mut coordinator,
            &lease,
            &validated_receipt,
            &apply_effect,
            apply_candidate.clone(),
        )
        .expect("stage exact durable Validate-to-Apply link");
        let authority = prepared.staged.episode_authority.clone();
        let ledger = super::super::ledger::LifecycleLedgerV1::from_coordinator(&prepared.staged)
            .expect("project exact linked ledger");
        let (ledger_store, empty) = super::super::ledger::LifecycleLedgerStoreV1::open(
            &ledger_root,
            lifecycle_context(verified.context()),
        )
        .expect("open durable lifecycle ledger");
        assert!(empty.records().is_empty());
        ledger_store
            .persist(&ledger)
            .expect("persist linked Validate-to-Apply ledger");
        drop(ledger_store);

        let body_store = crate::sumeragi::v2_body_store::V2BodyStore::open(
            &body_root,
            verified.context().clone(),
        )
        .expect("open exact-context body store");
        let signer = KeyPair::try_from_seed(vec![250; 32], Algorithm::BlsNormal)
            .expect("deterministic empty-cut signer");
        let (mut missing_payload_store, missing_payloads) =
            crate::sumeragi::v2_certified_serve_payload_store::CertifiedServePayloadStoreV1::open(
                &missing_payload_root,
                verified.context(),
            )
            .expect("open empty missing-candidate payload store");
        let missing_payloads = missing_payloads
            .authenticate(&verified, &signer, &body_store)
            .expect("authenticate empty Serve payload cut");
        let missing_cut =
            super::super::AuthenticatedLifecycleRecoveryCut::from_authenticated_parts(
                ledger.clone(),
                [],
                [],
                missing_payloads,
            )
            .expect("assemble missing Apply recovery cut");
        assert!(
            LifecycleCoordinator::open_with_authority(
                authority.clone(),
                &ledger_root,
                &mut missing_payload_store,
                missing_cut,
            )
            .is_err(),
            "a live Apply successor requires exact authenticated recovery coverage"
        );

        let (mut exact_payload_store, exact_payloads) =
            crate::sumeragi::v2_certified_serve_payload_store::CertifiedServePayloadStoreV1::open(
                &exact_payload_root,
                verified.context(),
            )
            .expect("open empty exact-candidate payload store");
        let exact_payloads = exact_payloads
            .authenticate(&verified, &signer, &body_store)
            .expect("authenticate second empty Serve payload cut");
        let exact_cut = super::super::AuthenticatedLifecycleRecoveryCut::from_authenticated_parts(
            ledger,
            [apply_candidate],
            [],
            exact_payloads,
        )
        .expect("assemble exact Apply recovery cut");
        let restarted = LifecycleCoordinator::open_with_authority(
            authority,
            &ledger_root,
            &mut exact_payload_store,
            exact_cut,
        )
        .expect("linked terminal Validate and authenticated live Apply reopen exactly");
        assert_eq!(restarted.fault(), None);
        assert_eq!(
            restarted.records[&lease.ordinal()].state,
            LifecycleState::Terminal(TerminalOutcome::Advanced)
        );
        assert_eq!(
            restarted.durable_records[&lease.ordinal()].continuation,
            DurableContinuation::successor(
                DurableContinuationEdge::ValidateToApply,
                lease.ordinal() + 1,
            )
        );
        assert_eq!(
            restarted.records[&(lease.ordinal() + 1)].state,
            LifecycleState::Ready
        );
    }

    #[test]
    fn validate_apply_rejects_wrong_binding_and_foreign_commitment_without_mutation() {
        let ValidateApplyFixture {
            mut coordinator,
            lease,
            verified,
            validate_effect,
            validate_pending,
            validated_receipt,
            apply_effect,
            apply_candidate,
            ..
        } = validate_apply_fixture(1, true);
        let before = format!("{coordinator:#?}");
        assert!(
            super::super::replay_authority::exact_live_wal_body_successor_candidate_for_test(
                &verified,
                &validate_effect,
                &validate_pending,
                &apply_effect,
                &validate_pending,
                Some(validated_receipt.durable()),
            )
            .is_none()
        );
        assert_eq!(format!("{coordinator:#?}"), before);

        let AdapterEffect::Apply {
            tag,
            subject,
            mut certificate,
        } = apply_effect
        else {
            unreachable!("fixture Apply effect")
        };
        certificate.execution_commitment =
            wire::ExecutionCommitment::without_topups_or_merge_carrier(
                Hash::new(b"foreign Validate-Apply state root"),
                Hash::new(b"foreign Validate-Apply events root"),
                Hash::new(b"foreign Validate-Apply trace root"),
                2,
                Hash::new(b"foreign Validate-Apply fee summary"),
            );
        let foreign_apply = AdapterEffect::Apply {
            tag,
            subject,
            certificate,
        };
        assert!(
            validate_pending
                .project_validate_apply_successor(&validate_effect, &foreign_apply)
                .is_none(),
            "Prepare-authorized Validate must reject a different Commit result"
        );
        assert!(matches!(
            prepare_authorized_validate_apply_transition(
                &mut coordinator,
                &lease,
                &validated_receipt,
                &foreign_apply,
                apply_candidate,
            ),
            Err(BodyStageTransitionError::InvalidValidationReceipt)
        ));
        assert_eq!(format!("{coordinator:#?}"), before);
    }

    #[test]
    fn ordinary_validate_receipt_rejects_self_consistent_foreign_apply_binding() {
        let ValidateApplyFixture {
            mut coordinator,
            lease,
            verified,
            validate_effect,
            validate_pending,
            validated_receipt,
            apply_effect,
            ..
        } = validate_apply_fixture(1, false);
        let AdapterEffect::Apply {
            tag,
            subject,
            mut certificate,
        } = apply_effect
        else {
            unreachable!("fixture Apply effect")
        };
        certificate.execution_commitment =
            wire::ExecutionCommitment::without_topups_or_merge_carrier(
                Hash::new(b"ordinary forged state root"),
                Hash::new(b"ordinary forged events root"),
                Hash::new(b"ordinary forged trace root"),
                3,
                Hash::new(b"ordinary forged fee summary"),
            );
        let foreign_apply = AdapterEffect::Apply {
            tag,
            subject,
            certificate,
        };
        let foreign_pending = validate_pending
            .project_validate_apply_successor(&validate_effect, &foreign_apply)
            .expect("ordinary lineage alone permits one internally exact Commit binding");
        let foreign_candidate =
            super::super::replay_authority::exact_live_wal_body_successor_candidate_for_test(
                &verified,
                &validate_effect,
                &validate_pending,
                &foreign_apply,
                &foreign_pending,
                Some(validated_receipt.durable()),
            )
            .expect("self-consistent foreign Apply has exact test WAL evidence");
        let before = format!("{coordinator:#?}");
        assert!(matches!(
            prepare_authorized_validate_apply_transition(
                &mut coordinator,
                &lease,
                &validated_receipt,
                &foreign_apply,
                foreign_candidate,
            ),
            Err(BodyStageTransitionError::InvalidValidationReceipt)
        ));
        assert_eq!(format!("{coordinator:#?}"), before);
    }

    #[test]
    fn commit_authorized_validate_retains_only_the_exact_commit_result() {
        let ValidateApplyFixture {
            mut coordinator,
            lease,
            validate_effect,
            validate_pending,
            validated_receipt,
            apply_effect,
            apply_candidate,
            ..
        } = validate_apply_fixture_with_authority(1, Some(wire::GlobalPhase::Commit));
        assert!(lease.key().execution_commitment().is_some());
        let before = format!("{coordinator:#?}");
        let prepared = prepare_authorized_validate_apply_transition(
            &mut coordinator,
            &lease,
            &validated_receipt,
            &apply_effect,
            apply_candidate,
        )
        .expect("exact Commit-authorized Validate retains its Apply authority");
        assert_eq!(
            prepared.staged.records[&prepared.child_ordinal]
                .key
                .execution_commitment(),
            lease.key().execution_commitment()
        );
        drop(prepared);
        assert_eq!(format!("{coordinator:#?}"), before);

        let AdapterEffect::Apply {
            tag,
            subject,
            mut certificate,
        } = apply_effect
        else {
            unreachable!("fixture Apply effect")
        };
        certificate.execution_commitment =
            wire::ExecutionCommitment::without_topups_or_merge_carrier(
                Hash::new(b"changed retained Commit state root"),
                Hash::new(b"changed retained Commit events root"),
                Hash::new(b"changed retained Commit trace root"),
                4,
                Hash::new(b"changed retained Commit fee summary"),
            );
        let changed_apply = AdapterEffect::Apply {
            tag,
            subject,
            certificate,
        };
        assert!(
            validate_pending
                .project_validate_apply_successor(&validate_effect, &changed_apply)
                .is_none(),
            "Commit authority may retain only its exact statement"
        );
        assert_eq!(format!("{coordinator:#?}"), before);
    }

    #[test]
    fn validate_apply_rejects_corrupt_parent_commitment_lineage_without_mutation() {
        let ValidateApplyFixture {
            mut coordinator,
            mut lease,
            validated_receipt,
            apply_effect,
            apply_candidate,
            ..
        } = validate_apply_fixture(1, true);
        let incumbent_key = lease.key();
        let foreign_key = super::super::LifecycleKey::new(
            incumbent_key.context(),
            incumbent_key.round(),
            incumbent_key.proposal_round(),
            incumbent_key.subject(),
            LifecyclePhase::Validate,
            Some(super::super::LifecycleDigest::new([0xE1; 32])),
        );
        assert_ne!(
            foreign_key.execution_commitment(),
            incumbent_key.execution_commitment()
        );
        lease.key = foreign_key;
        coordinator.active_lease = Some(lease.clone());
        assert_eq!(
            coordinator.key_index.remove(&incumbent_key),
            Some(lease.ordinal())
        );
        assert_eq!(
            coordinator.key_index.insert(foreign_key, lease.ordinal()),
            None
        );
        coordinator
            .records
            .get_mut(&lease.ordinal())
            .expect("claimed Validate record")
            .key = foreign_key;
        let before = format!("{coordinator:#?}");
        assert!(matches!(
            prepare_authorized_validate_apply_transition(
                &mut coordinator,
                &lease,
                &validated_receipt,
                &apply_effect,
                apply_candidate,
            ),
            Err(BodyStageTransitionError::ForeignSuccessorLineage)
        ));
        assert_eq!(format!("{coordinator:#?}"), before);
    }
}
