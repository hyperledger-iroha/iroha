//! Sealed coordinator staging and publication for adjacent body-pipeline successors.

use super::{
    AdapterEffectAdmissionError, AdmissionDecision, AdmissionRequest, CandidateAdmission,
    CapacityClass, CoordinatorFault, InitialLifecycleState, LifecycleCoordinator, LifecyclePhase,
    LifecycleStageKind, LifecycleState, LifecycleWorkClass, OwnerId, PhysicalSlotId,
    PredecessorScope, TerminalOutcome, TurnLease, WaitSource, WaitToken,
    schema::{DurableContinuation, DurableContinuationEdge, DurablePayloadReference},
    work_registry::{
        BoundRecoveredLifecycleSignBroadcastAndSignSuccessor,
        LiveValidateSignRegistryPublicationError, PreparedCertifiedFetchStoreSuccessor,
        PreparedDurableStoreValidateSuccessor, PreparedInvalidBodyReportReplayPreAdmission,
        PreparedLiveValidateSignRegistryPublication, PreparedReadyDurableValidateAdapterPreview,
        PreparedReadyDurableValidatePersistedSignPreAdmission,
        PreparedRecoveredDecisionFetchStoreSuccessor,
        PreparedRecoveredLifecycleSignBroadcastAndSignSuccessor,
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

/// Fully checked staged state for one recovered Sign with two reducer children.
///
/// The Broadcast remains the typed continuation of the claimed Sign. The
/// follow-on Vote Sign is a separate WAL-owned Ready row, admitted in the same
/// invisible coordinator copy at the immediately following ordinal.
#[cfg_attr(not(test), allow(dead_code))]
struct StagedRecoveredLifecycleSignBroadcastAndSignTransition {
    staged: LifecycleCoordinator,
    parent_ordinal: u128,
    broadcast_ordinal: u128,
    next_sign_ordinal: u128,
    broadcast_owner: OwnerId,
    next_sign_owner: OwnerId,
    broadcast_slot: PhysicalSlotId,
    broadcast_digest: super::LifecycleDigest,
    next_sign_slot: PhysicalSlotId,
    next_sign_digest: super::LifecycleDigest,
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

/// One-shot authority for reading the two admissions of a combined Sign result.
///
/// WAL recovery retains both executable children in one opaque projection.
/// Only this transition module can mint the permit which clones their inert
/// admissions into an unpublished coordinator copy; registry ownership stays
/// inseparable until the later post-fsync commit.
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct RecoveredLifecycleBroadcastAndSignTransitionProjectionPermitV1 {
    _linearity: RecoveredLifecycleBroadcastAndSignTransitionProjectionLinearityV1,
}

#[cfg_attr(not(test), allow(dead_code))]
struct RecoveredLifecycleBroadcastAndSignTransitionProjectionLinearityV1;

impl Drop for RecoveredLifecycleBroadcastAndSignTransitionProjectionLinearityV1 {
    fn drop(&mut self) {}
}

#[cfg_attr(not(test), allow(dead_code))]
impl RecoveredLifecycleBroadcastAndSignTransitionProjectionPermitV1 {
    fn new() -> Self {
        Self {
            _linearity: RecoveredLifecycleBroadcastAndSignTransitionProjectionLinearityV1,
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

#[cfg_attr(not(test), allow(dead_code))]
fn recovered_broadcast_and_next_sign_are_exact(
    broadcast: &CandidateAdmission,
    next_sign: &CandidateAdmission,
) -> bool {
    let exact_stages = matches!(
        (
            broadcast.key.phase(),
            broadcast.stage.kind(),
            next_sign.key.phase(),
            next_sign.stage.kind(),
        ),
        (
            LifecyclePhase::BroadcastProposal,
            LifecycleStageKind::BroadcastProposal,
            LifecyclePhase::Prepare,
            LifecycleStageKind::SignPrepareVote,
        ) | (
            LifecyclePhase::BroadcastPrepareVote,
            LifecycleStageKind::BroadcastPrepareVote,
            LifecyclePhase::Commit,
            LifecycleStageKind::SignCommitVote,
        )
    );
    let commitment_is_exact = match broadcast.key.phase() {
        LifecyclePhase::BroadcastProposal => {
            broadcast.key.execution_commitment().is_none()
                && next_sign.key.execution_commitment().is_some()
        }
        LifecyclePhase::BroadcastPrepareVote => {
            broadcast.key.execution_commitment() == next_sign.key.execution_commitment()
        }
        _ => false,
    };
    exact_stages
        && commitment_is_exact
        && broadcast.key.context() == next_sign.key.context()
        && broadcast.key.round() == next_sign.key.round()
        && broadcast.key.proposal_round() == next_sign.key.proposal_round()
        && broadcast.key.subject() == next_sign.key.subject()
}

/// Stage one recovered `Signed` reducer result containing Broadcast plus Sign.
///
/// The first child uses the ordinary typed Sign-to-Broadcast continuation and
/// consumes the claimed parent's Consensus overlay. The independently
/// WAL-owned follow-on Sign is then admitted at the next ordinal in the same
/// unpublished coordinator copy. No live coordinator state changes here.
#[cfg_attr(not(test), allow(dead_code))]
fn stage_recovered_lifecycle_sign_broadcast_and_sign_transition(
    coordinator: &LifecycleCoordinator,
    lease: &TurnLease,
    broadcast: CandidateAdmission,
    next_sign: CandidateAdmission,
) -> Result<StagedRecoveredLifecycleSignBroadcastAndSignTransition, BodyStageTransitionError> {
    let next_sign_candidate = next_sign.clone();
    let next_sign_slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
    let Ok((next_slots, next_universe, next_consumed)) = next_sign.physical_geometry.normalized()
    else {
        return Err(BodyStageTransitionError::InvalidChildProjection);
    };
    let Some(&next_sign_digest) = next_slots.get(&next_sign_slot) else {
        return Err(BodyStageTransitionError::InvalidChildProjection);
    };
    if !recovered_broadcast_and_next_sign_are_exact(&broadcast, &next_sign)
        || next_sign.work_class != LifecycleWorkClass::SignVote
        || next_sign.stage.predecessor_scope() != PredecessorScope::Independent
        || next_sign.initial_state != InitialLifecycleState::Ready
        || next_sign.payload != DurablePayloadReference::None
        || next_sign.producer_turn.is_some()
        || next_sign.causal_root == lease.owner().causal_root()
        || coordinator.owner_index.contains_key(&next_sign.causal_root)
        || next_sign.reconstruction_source != next_sign.causal_root.digest()
        || !next_sign.replay_authority_is_exact(coordinator.active_context)
        || next_slots.len() != 1
        || next_universe.len() != 1
        || next_consumed != next_universe
        || !next_universe.contains(&next_sign_slot)
    {
        return Err(BodyStageTransitionError::InvalidChildProjection);
    }

    let capacity_used_before = coordinator.capacity_used.clone();
    let capacity_generation_before = coordinator.capacity_generation.clone();
    let records_before = coordinator.records.len();
    let durable_records_before = coordinator.durable_records.len();
    let first = stage_recovered_lifecycle_sign_broadcast_transition(coordinator, lease, broadcast)?;
    let expected_next_sign_ordinal = first
        .child_ordinal
        .checked_add(1)
        .ok_or(BodyStageTransitionError::OrdinalExhausted)?;
    let StagedBodyStageTransition {
        mut staged,
        parent_ordinal,
        child_ordinal: broadcast_ordinal,
        owner: broadcast_owner,
        child_slot: broadcast_slot,
        child_digest: broadcast_digest,
    } = first;
    let decision = staged.reduce_admit(AdmissionRequest::Candidate(next_sign));
    let AdmissionDecision::Admitted {
        owner: next_sign_owner,
        ordinal: next_sign_ordinal,
        producer_turn_ordinal: None,
    } = decision
    else {
        return Err(BodyStageTransitionError::ChildAdmission(Box::new(decision)));
    };
    if next_sign_ordinal != expected_next_sign_ordinal
        || next_sign_owner.causal_root() != next_sign_candidate.causal_root
        || next_sign_owner == broadcast_owner
    {
        return Err(BodyStageTransitionError::InvalidChildOwner);
    }

    let next_record_is_exact = staged
        .records
        .get(&next_sign_ordinal)
        .is_some_and(|record| {
            record.owner == next_sign_owner
                && record.ordinal == next_sign_ordinal
                && record.key == next_sign_candidate.key
                && record.work_class == LifecycleWorkClass::SignVote
                && record.stage == next_sign_candidate.stage
                && record.state == LifecycleState::Ready
                && record.physical_slots == next_slots
                && record.episode.slot_universe == next_universe
                && record.episode.consumed_slots == next_consumed
        });
    if !next_record_is_exact
        || staged.active_lease.is_some()
        || staged.high_water != next_sign_ordinal
        || staged.records.len() != records_before.saturating_add(2)
        || staged.durable_records.len() != durable_records_before.saturating_add(2)
        || staged.key_index.get(&next_sign_candidate.key) != Some(&next_sign_ordinal)
        || staged.owner_index.get(&next_sign_candidate.causal_root) != Some(&next_sign_owner)
        || !staged.ready_index.contains(&broadcast_ordinal)
        || !staged.ready_index.contains(&next_sign_ordinal)
        || staged
            .durable_records
            .get(&next_sign_ordinal)
            .is_none_or(|metadata| {
                !metadata.matches_admission(&next_sign_candidate)
                    || metadata.continuation != DurableContinuation::None
            })
        || staged.capacity_used[&CapacityClass::Effect]
            != capacity_used_before[&CapacityClass::Effect]
        || staged.capacity_generation[&CapacityClass::Effect]
            != capacity_generation_before[&CapacityClass::Effect].saturating_add(1)
        || staged.capacity_used[&CapacityClass::Consensus]
            != capacity_used_before[&CapacityClass::Consensus].saturating_add(1)
        || staged.capacity_generation[&CapacityClass::Consensus]
            != capacity_generation_before[&CapacityClass::Consensus]
        || CapacityClass::ALL
            .into_iter()
            .filter(|class| !matches!(class, CapacityClass::Effect | CapacityClass::Consensus))
            .any(|class| {
                staged.capacity_used[&class] != capacity_used_before[&class]
                    || staged.capacity_generation[&class] != capacity_generation_before[&class]
            })
    {
        return Err(BodyStageTransitionError::InvalidStagedRecords);
    }

    Ok(StagedRecoveredLifecycleSignBroadcastAndSignTransition {
        staged,
        parent_ordinal,
        broadcast_ordinal,
        next_sign_ordinal,
        broadcast_owner,
        next_sign_owner,
        broadcast_slot,
        broadcast_digest,
        next_sign_slot,
        next_sign_digest,
    })
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

/// Fully staged recovered Proposal Broadcast-and-next-Sign publication.
///
/// Both concrete child addresses have rejoined the opaque registry successor,
/// and, for Proposal, the exact output reservation remains held outside this
/// token until the LedgerV1 successor is fsynced. Proposal publication parks
/// its Broadcast behind that owner; Vote publication leaves Broadcast Ready
/// for typed refanout. The independently WAL-owned next Sign remains Ready and
/// a crash reconstructs the Broadcast output debt from LedgerV1.
#[must_use = "combined recovered Sign transition has not been published"]
pub(super) struct PreparedRecoveredLifecycleSignBroadcastAndSignTransition<
    'coordinator,
    'registry,
    'adapter,
> {
    coordinator: &'coordinator mut LifecycleCoordinator,
    successor: BoundRecoveredLifecycleSignBroadcastAndSignSuccessor<'registry, 'adapter>,
    staged: LifecycleCoordinator,
    parent_ordinal: u128,
    broadcast_ordinal: u128,
    next_sign_ordinal: u128,
    broadcast_owner: OwnerId,
    next_sign_owner: OwnerId,
    broadcast_slot: PhysicalSlotId,
    broadcast_digest: super::LifecycleDigest,
    next_sign_slot: PhysicalSlotId,
    next_sign_digest: super::LifecycleDigest,
    broadcast_wait: WaitToken,
    publication_is_vote: bool,
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

impl PreparedRecoveredLifecycleSignBroadcastAndSignTransition<'_, '_, '_> {
    /// Fsync the exact two-child LedgerV1 successor while all owners stay borrowed.
    pub(super) fn persist_exact_successor(
        &self,
    ) -> Result<(), super::ledger::LifecycleLedgerError> {
        self.coordinator
            .persist_exact_staged_successor(&self.staged)
    }

    /// Publish both children under the preauthenticated output mode.
    pub(super) fn commit_after_publication(self) {
        let Self {
            coordinator,
            successor,
            staged,
            parent_ordinal,
            broadcast_ordinal,
            next_sign_ordinal,
            broadcast_owner,
            next_sign_owner,
            broadcast_slot,
            broadcast_digest,
            next_sign_slot,
            next_sign_digest,
            broadcast_wait,
            publication_is_vote,
        } = self;
        assert!(staged.records.get(&parent_ordinal).is_some_and(|record| {
            record.state == LifecycleState::Terminal(TerminalOutcome::Advanced)
        }));
        assert!(
            staged
                .records
                .get(&broadcast_ordinal)
                .is_some_and(|record| {
                    record.owner == broadcast_owner
                        && record.state == LifecycleState::Ready
                        && record.physical_slots.get(&broadcast_slot) == Some(&broadcast_digest)
                })
        );
        assert!(
            staged
                .records
                .get(&next_sign_ordinal)
                .is_some_and(|record| {
                    record.owner == next_sign_owner
                        && record.state == LifecycleState::Ready
                        && record.physical_slots.get(&next_sign_slot) == Some(&next_sign_digest)
                })
        );
        assert!(staged.active_lease.is_none());

        let adapter = successor.commit_after_publication();
        *coordinator = staged;
        if publication_is_vote {
            assert!(coordinator.ready_index.contains(&broadcast_ordinal));
            assert!(
                coordinator
                    .records
                    .get(&broadcast_ordinal)
                    .is_some_and(|record| { record.state == LifecycleState::Ready })
            );
        } else {
            assert!(coordinator.ready_index.remove(&broadcast_ordinal));
            coordinator
                .records
                .get_mut(&broadcast_ordinal)
                .expect("published combined Broadcast retains its staged row")
                .state = LifecycleState::Waiting(broadcast_wait);
            assert!(
                coordinator
                    .observed_generation
                    .insert(
                        broadcast_wait.source(),
                        broadcast_wait.observed_generation()
                    )
                    .is_none_or(|known| known == broadcast_wait.observed_generation())
            );
        }
        assert!(coordinator.ready_index.contains(&next_sign_ordinal));
        if publication_is_vote {
            adapter.commit_after_durable_vote_broadcast_and_sign();
        } else {
            adapter.commit_after_durable_broadcast_and_sign();
        }
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

    /// Stage the exact two-child result of one recovered `Signed` event.
    ///
    /// Both opaque admissions fit one coordinator snapshot, and the registry
    /// binds the same fresh addresses without splitting executable authority.
    /// The returned value retains every owner through the one allowed fsync.
    pub(super) fn prepare_recovered_lifecycle_sign_broadcast_and_sign_transition<
        'coordinator,
        'registry,
        'adapter,
    >(
        &'coordinator mut self,
        lease: &TurnLease,
        successor: PreparedRecoveredLifecycleSignBroadcastAndSignSuccessor<'registry, 'adapter>,
    ) -> Result<
        PreparedRecoveredLifecycleSignBroadcastAndSignTransition<'coordinator, 'registry, 'adapter>,
        BodyStageTransitionError,
    > {
        if self.ledger_store.is_none() {
            return Err(BodyStageTransitionError::InvalidStagedRecords);
        }
        let publication_is_vote = successor
            .publication_is_vote()
            .ok_or(BodyStageTransitionError::InvalidChildProjection)?;
        let (broadcast, next_sign) = successor.project_transition_candidates(
            RecoveredLifecycleBroadcastAndSignTransitionProjectionPermitV1::new(),
        );
        let transition = stage_recovered_lifecycle_sign_broadcast_and_sign_transition(
            self, lease, broadcast, next_sign,
        )?;
        let successor = successor
            .bind_staged_children(
                &transition.staged,
                transition.broadcast_ordinal,
                transition.next_sign_ordinal,
            )
            .map_err(|_| BodyStageTransitionError::InvalidChildProjection)?;
        let broadcast_wait_source = WaitSource::Recovery(transition.broadcast_digest);
        let broadcast_wait_generation = transition
            .staged
            .observed_generation
            .get(&broadcast_wait_source)
            .copied()
            .unwrap_or(0);
        if broadcast_wait_generation == u64::MAX {
            return Err(BodyStageTransitionError::InvalidStagedRecords);
        }
        Ok(PreparedRecoveredLifecycleSignBroadcastAndSignTransition {
            coordinator: self,
            successor,
            staged: transition.staged,
            parent_ordinal: transition.parent_ordinal,
            broadcast_ordinal: transition.broadcast_ordinal,
            next_sign_ordinal: transition.next_sign_ordinal,
            broadcast_owner: transition.broadcast_owner,
            next_sign_owner: transition.next_sign_owner,
            broadcast_slot: transition.broadcast_slot,
            broadcast_digest: transition.broadcast_digest,
            next_sign_slot: transition.next_sign_slot,
            next_sign_digest: transition.next_sign_digest,
            broadcast_wait: WaitToken::new(broadcast_wait_source, broadcast_wait_generation),
            publication_is_vote,
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

include!("v2_lifecycle_body_pipeline_transition_static_tests.rs");

include!("v2_lifecycle_body_pipeline_transition_tests.rs");
