//! Sealed restart join for WAL-ahead Validate vote continuations.

use iroha_data_model::block::consensus_v2 as wire;

use super::{
    CandidateAdmission, CapacityClass, DurablePayloadReference, DurableValidateReplayEvidenceV1,
    InitialLifecycleState, LifecycleStageKind, LifecycleWorkClass, PredecessorScope,
    RecoveredDecisionApplyCandidateLineageV1, RecoveredDecisionApplyReplayLineageV1,
    RecoveredWalControlReplayEvidenceV1, RecoveredWalDecisionFetchReplayEvidenceV1,
    body_pipeline_transition::{
        durable_continuation_successor_is_exact, durable_validate_payload_is_exact,
    },
    ledger::{AuthenticatedRecoveredWalValidateLedgerParent, DurableWalVoteLedgerRepairReceipt},
    projection,
    replay_authority::{
        RecoveredWalControlCandidateProjectionV1, RecoveredWalDecisionFetchCandidateProjectionV1,
    },
    schema::DurableContinuationEdge,
};
use crate::sumeragi::{
    v2::{
        AdapterEffect, RecoveredDecisionApplyCandidateProjectionPermit, RecoveredWalFrameIdentity,
        VerifiedHeightContext,
    },
    v2_body_store::{
        DurableBodyReceipt, RecoveredDecisionApplyAdapterPreviewPermit,
        RecoveredDecisionApplyReplayPermit, RecoveredDecisionFetchStoreBodyAuthorityV1,
        ValidatedBodyReceipt,
    },
    v2_runtime::{
        PendingRuntimeEffectBinding, RecoveredWalCandidateProjectionPermit,
        RecoveredWalVoteProjectionFailure, RecoveredWalVoteSuccessor,
        project_recovered_lifecycle_next_wal_vote_candidate,
    },
};

/// Why one recovered WAL vote could not join its exact Validate predecessor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RecoveredWalVoteLifecycleRepairErrorKind {
    ParentProjection,
    ChildProjection,
    InvalidWalIdentity,
    InvalidReplayEvidence,
    InvalidParent,
    InvalidChild,
    ForeignOwner,
    ForeignLineage,
}

/// Drop-safe failure which returns every move-only recovery input.
///
/// The caller may retry after rebuilding the surrounding startup cut. No
/// ledger, coordinator, registry, adapter, or WAL state is changed while this
/// value is produced.
#[must_use = "failed WAL lifecycle recovery retains all move-only inputs"]
pub(super) struct RecoveredWalVoteLifecycleRepairError {
    kind: RecoveredWalVoteLifecycleRepairErrorKind,
    _retained: RecoveredWalVoteLifecycleRepairRetained,
}

enum RecoveredWalVoteLifecycleRepairRetained {
    Successor {
        _successor: RecoveredWalVoteSuccessor,
    },
    Projection {
        _projection: AuthenticatedRecoveredWalVoteProjection,
    },
}

/// Consuming projection retaining the recovered successor beside both candidates.
///
/// Construction requires a runtime-private permit and the wrapper has no
/// parts API outside this lifecycle-repair module.
#[must_use = "a recovered WAL candidate projection must enter lifecycle repair"]
pub(in crate::sumeragi) struct AuthenticatedRecoveredWalVoteProjection {
    successor: RecoveredWalVoteSuccessor,
    parent: CandidateAdmission,
    child: CandidateAdmission,
}

/// Closed runtime projection of one recovered Proposal/Timeout control Sign.
///
/// The complete WAL identity, canonical replay evidence, effect, pending
/// owner, and logical candidate remain private to this module. Ledger and
/// registry code receive only fixed comparison, staging, and splice oracles;
/// there is no parts API.
#[must_use = "a recovered control projection must enter exact storage recovery"]
pub(in crate::sumeragi) struct AuthenticatedRecoveredWalControlProjection {
    wal_identity: RecoveredWalFrameIdentity,
    replay_evidence: RecoveredWalControlReplayEvidenceV1,
    effect: AdapterEffect,
    pending: PendingRuntimeEffectBinding,
    candidate: CandidateAdmission,
}

/// Opaque signed Broadcast successor of one recovered lifecycle Sign.
///
/// The complete signed envelope, pending causal binding, and canonical replay
/// admission remain inseparable. A concrete registry replacement must retain
/// the original recovered Sign carrier beside this value so the predecessor
/// projection can be rechecked after restart or before publication.
#[must_use = "recovered signed Broadcast must enter its parent replacement transaction"]
pub(in crate::sumeragi) struct RecoveredLifecycleSignedBroadcastProjectionV1 {
    effect: AdapterEffect,
    pending: PendingRuntimeEffectBinding,
    candidate: CandidateAdmission,
    cold_proposal_output: Option<crate::sumeragi::v2::RecoveredLifecycleColdProposalOutputV1>,
}

/// Opaque recovered signature successor retaining both reducer children.
///
/// The signed Broadcast has been projected through its exact recovered parent,
/// while the follow-on Vote Sign remains sealed to its latest authenticated
/// WAL frame and validated body receipt. No parts accessor exists; live
/// publication and cold recovery must consume the pair as one transaction input.
#[must_use = "combined recovered Broadcast and Sign projection must remain inseparable"]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::sumeragi) struct RecoveredLifecycleSignedBroadcastAndSignProjectionV1 {
    broadcast: RecoveredLifecycleSignedBroadcastProjectionV1,
    next_sign: super::replay_authority::RecoveredLifecycleNextWalVoteCandidateProjectionV1,
    cold_adapter_authority_minted: bool,
}

/// Opaque durable source for refanout of one recovered signed Broadcast.
///
/// The live Broadcast row remains the crash-recovery owner. Only the exact
/// service permit can unpack the message, so refanout cannot be redirected to
/// a substitute height or envelope.
#[must_use = "recovered signed Broadcast output authority must be consumed by its service"]
pub(in crate::sumeragi) struct RecoveredLifecycleSignedBroadcastOutputAuthorityV1 {
    context_id: wire::HeightContextId,
    height: u64,
    message: wire::ConsensusMessageV2,
    cold_proposal_output: Option<crate::sumeragi::v2::RecoveredLifecycleColdProposalOutputV1>,
}

impl RecoveredLifecycleSignedBroadcastOutputAuthorityV1 {
    /// Build one context-bound output authority for focused service tests.
    #[cfg(test)]
    pub(in crate::sumeragi) fn for_test(
        context: &wire::HeightContext,
        message: wire::ConsensusMessageV2,
    ) -> Self {
        Self {
            context_id: context.id(),
            height: context.height,
            message,
            cold_proposal_output: None,
        }
    }

    /// Build one cold Proposal output authority for focused service tests.
    #[cfg(test)]
    pub(in crate::sumeragi) fn for_cold_proposal_test(
        context: &wire::HeightContext,
        message: wire::ConsensusMessageV2,
        output: crate::sumeragi::v2::RecoveredLifecycleColdProposalOutputV1,
    ) -> Self {
        Self {
            context_id: context.id(),
            height: context.height,
            message,
            cold_proposal_output: Some(output),
        }
    }

    /// Release the fixed output projection only to the service-private permit.
    pub(in crate::sumeragi) fn consume_for_service(
        self,
        _permit: crate::sumeragi::v2_worker::RecoveredLifecycleSignBroadcastOutputPermitV1,
    ) -> (
        wire::HeightContextId,
        u64,
        wire::ConsensusMessageV2,
        Option<crate::sumeragi::v2::RecoveredLifecycleColdProposalOutputV1>,
    ) {
        (
            self.context_id,
            self.height,
            self.message,
            self.cold_proposal_output,
        )
    }
}

/// WAL-module permit for unpacking an adapter-authenticated Broadcast.
///
/// Construction is private here; the adapter authority accepts it only by
/// move, so no sibling can route an unchecked raw Broadcast effect into a
/// recovered carrier projection.
pub(in crate::sumeragi) struct RecoveredLifecycleSignBroadcastProjectionPermitV1 {
    _linearity: RecoveredLifecycleSignBroadcastProjectionPermitLinearityV1,
}

struct RecoveredLifecycleSignBroadcastProjectionPermitLinearityV1;

impl Drop for RecoveredLifecycleSignBroadcastProjectionPermitLinearityV1 {
    fn drop(&mut self) {}
}

impl RecoveredLifecycleSignBroadcastProjectionPermitV1 {
    fn new() -> Self {
        Self {
            _linearity: RecoveredLifecycleSignBroadcastProjectionPermitLinearityV1,
        }
    }

    /// Mint the same move-only permit for a directly coupled adapter fixture.
    #[cfg(test)]
    pub(in crate::sumeragi) fn for_test() -> Self {
        Self::new()
    }
}

impl RecoveredLifecycleSignedBroadcastProjectionV1 {
    /// Compare two complete sealed child projections without exposing parts.
    pub(super) fn exactly_matches(&self, other: &Self) -> bool {
        self.effect == other.effect
            && self.pending == other.pending
            && self.candidate == other.candidate
            && match (&self.cold_proposal_output, &other.cold_proposal_output) {
                (Some(left), Some(right)) => left.exactly_matches(right),
                (None, None) => true,
                (Some(_), None) | (None, Some(_)) => false,
            }
    }

    /// Return the exact installed digest while retaining all replay authority.
    pub(super) fn digest(&self) -> super::LifecycleDigest {
        super::LifecycleDigest::new(*self.pending.exact_effect_identity().as_ref())
    }

    /// Borrow the closed admission for one staged parent-to-child transition.
    pub(super) const fn candidate(&self) -> &CandidateAdmission {
        &self.candidate
    }

    /// Rejoin one signed Vote to its opaque WAL parent for scheduler tests.
    ///
    /// This keeps the effect and pending binding inside the ordinary closed
    /// Broadcast projection; callers receive no constituent or signing permit.
    #[cfg(test)]
    pub(super) fn from_next_wal_vote_for_scheduler_fixture(
        parent: &super::replay_authority::RecoveredLifecycleNextWalVoteCandidateProjectionV1,
        verified: &VerifiedHeightContext,
        broadcast: AdapterEffect,
    ) -> Option<Self> {
        let closed = parent.project_authenticated_signed_broadcast(verified, broadcast)?;
        let (effect, pending, candidate) = closed
            .consume_for_recovered_wal(RecoveredLifecycleSignBroadcastProjectionPermitV1::new());
        let projection = Self {
            effect,
            pending,
            candidate,
            cold_proposal_output: None,
        };
        projection
            .validates_from_next_wal_vote(verified, parent)
            .then_some(projection)
    }

    /// Project one fixed refanout authority from the still-live durable child.
    pub(super) fn project_output_authority(
        &self,
        verified: &VerifiedHeightContext,
    ) -> Option<RecoveredLifecycleSignedBroadcastOutputAuthorityV1> {
        let AdapterEffect::Broadcast(message) = &self.effect else {
            return None;
        };
        let context = projection::lifecycle_context(verified.context());
        (self.pending.exactly_binds_adapter_effect(&self.effect)
            && self.candidate.replay_authority_is_exact(context)
            && self.candidate.key.context() == context.id()
            && self.candidate.key.round().height() == context.height())
        .then(|| RecoveredLifecycleSignedBroadcastOutputAuthorityV1 {
            context_id: verified.context().id(),
            height: verified.context().height,
            message: message.clone(),
            cold_proposal_output: self.cold_proposal_output.clone(),
        })
    }

    /// Compare the complete Ready child against one exact LedgerV1 row.
    pub(super) fn exactly_matches_record(
        &self,
        record: &super::ledger::LifecycleLedgerRecordV1,
        owner: super::OwnerId,
    ) -> bool {
        record.key() == Some(self.candidate.key)
            && record.owner() == owner
            && record.work_class() == Some(LifecycleWorkClass::Broadcast)
            && record.stage() == Some(self.candidate.stage)
            && record.terminal() == Some(None)
            && record.reconstruction_source() == self.candidate.reconstruction_source
            && record.durable_payload() == Some(DurablePayloadReference::None)
            && record.continuation() == Some(super::schema::DurableContinuation::None)
            && record.replay_matches_candidate(&self.candidate)
    }

    /// Insert this exact live child during typed cold recovery.
    pub(super) fn splice_candidate_from_record(
        &self,
        record: &super::ledger::LifecycleLedgerRecordV1,
        owner: super::OwnerId,
        candidates: &mut std::collections::BTreeMap<super::LifecycleKey, CandidateAdmission>,
    ) -> bool {
        self.exactly_matches_record(record, owner)
            && !candidates.contains_key(&self.candidate.key)
            && candidates
                .insert(self.candidate.key, self.candidate.clone())
                .is_none()
    }

    /// Recheck that the cold recovery census retained only this child key.
    pub(super) fn owns_spliced_candidate(
        &self,
        candidates: &std::collections::BTreeMap<super::LifecycleKey, CandidateAdmission>,
    ) -> bool {
        candidates.get(&self.candidate.key) == Some(&self.candidate)
    }

    /// Compare the exact Ready Broadcast row, indexes, and physical geometry.
    pub(super) fn matches_current_ready_record(
        &self,
        context: super::LifecycleContext,
        address: super::work_registry::ConcreteWorkAddress,
        digest: super::LifecycleDigest,
        coordinator: &super::LifecycleCoordinator,
    ) -> bool {
        let Ok((physical, universe, consumed)) = self.candidate.physical_geometry.normalized()
        else {
            return false;
        };
        let (Some(record), Some(metadata)) = (
            coordinator.records.get(&address.ordinal),
            coordinator.durable_records.get(&address.ordinal),
        ) else {
            return false;
        };
        self.validates_at_raw_context(context, address, digest)
            && coordinator.fault.is_none()
            && coordinator.active_context == context
            && coordinator.high_water >= address.ordinal
            && record.key == self.candidate.key
            && record.owner == address.owner
            && record.ordinal == address.ordinal
            && record.work_class == LifecycleWorkClass::Broadcast
            && record.stage == self.candidate.stage
            && record.state == super::LifecycleState::Ready
            && record.physical_slots == physical
            && record.episode.slot_universe == universe
            && record.episode.consumed_slots == consumed
            && physical.get(&address.slot) == Some(&digest)
            && metadata.matches_admission(&self.candidate)
            && coordinator.key_index.get(&self.candidate.key) == Some(&address.ordinal)
            && coordinator.owner_index.get(&self.candidate.causal_root) == Some(&address.owner)
            && coordinator.ready_index.contains(&address.ordinal)
    }

    /// Compare the exact live Broadcast state accepted at height finalization.
    ///
    /// Ordinary and cold-start scheduling remain Ready-only. The sole extra
    /// state admitted here is the volatile wait installed after an exact
    /// durable-Broadcast refanout: it must name this carrier's digest through
    /// `Recovery`, retain the coordinator's exact observed generation, and be
    /// absent from the Ready index. A crash discards that wait and reconstructs
    /// the durable Ledger row as Ready, so this oracle must never be reused by
    /// startup recovery.
    pub(super) fn matches_current_finalization_record(
        &self,
        context: super::LifecycleContext,
        address: super::work_registry::ConcreteWorkAddress,
        digest: super::LifecycleDigest,
        coordinator: &super::LifecycleCoordinator,
    ) -> bool {
        if self.matches_current_ready_record(context, address, digest, coordinator) {
            return true;
        }
        let Ok((physical, universe, consumed)) = self.candidate.physical_geometry.normalized()
        else {
            return false;
        };
        let (Some(record), Some(metadata)) = (
            coordinator.records.get(&address.ordinal),
            coordinator.durable_records.get(&address.ordinal),
        ) else {
            return false;
        };
        let super::LifecycleState::Waiting(wait) = record.state else {
            return false;
        };
        let expected_source = super::WaitSource::Recovery(digest);
        self.validates_at_raw_context(context, address, digest)
            && coordinator.fault.is_none()
            && coordinator.active_lease.is_none()
            && coordinator.active_context == context
            && coordinator.high_water >= address.ordinal
            && record.key == self.candidate.key
            && record.owner == address.owner
            && record.ordinal == address.ordinal
            && record.work_class == LifecycleWorkClass::Broadcast
            && record.stage == self.candidate.stage
            && wait.source() == expected_source
            && wait.observed_generation() != u64::MAX
            && coordinator.observed_generation.get(&expected_source)
                == Some(&wait.observed_generation())
            && record.physical_slots == physical
            && record.episode.slot_universe == universe
            && record.episode.consumed_slots == consumed
            && physical.get(&address.slot) == Some(&digest)
            && metadata.matches_admission(&self.candidate)
            && coordinator.key_index.get(&self.candidate.key) == Some(&address.ordinal)
            && coordinator.owner_index.get(&self.candidate.causal_root) == Some(&address.owner)
            && !coordinator.ready_index.contains(&address.ordinal)
    }

    /// Compare the exact claimed Broadcast row and its sole active lease.
    pub(super) fn matches_current_claimed_record(
        &self,
        context: super::LifecycleContext,
        address: super::work_registry::ConcreteWorkAddress,
        digest: super::LifecycleDigest,
        coordinator: &super::LifecycleCoordinator,
        lease: &super::TurnLease,
    ) -> bool {
        let Ok((physical, universe, consumed)) = self.candidate.physical_geometry.normalized()
        else {
            return false;
        };
        let (Some(record), Some(metadata)) = (
            coordinator.records.get(&address.ordinal),
            coordinator.durable_records.get(&address.ordinal),
        ) else {
            return false;
        };
        self.validates_at_raw_context(context, address, digest)
            && coordinator.fault.is_none()
            && coordinator.active_context == context
            && coordinator.active_lease.as_ref() == Some(lease)
            && lease.ordinal() == address.ordinal
            && lease.owner() == address.owner
            && lease.work_class() == LifecycleWorkClass::Broadcast
            && lease.physical_slots() == &physical
            && record.key == self.candidate.key
            && record.owner == address.owner
            && record.ordinal == address.ordinal
            && record.work_class == LifecycleWorkClass::Broadcast
            && record.stage == self.candidate.stage
            && record.state == super::LifecycleState::Claimed(lease.id())
            && record.physical_slots == physical
            && record.episode.slot_universe == universe
            && record.episode.consumed_slots == consumed
            && metadata.matches_admission(&self.candidate)
            && coordinator.key_index.get(&self.candidate.key) == Some(&address.ordinal)
            && coordinator.owner_index.get(&self.candidate.causal_root) == Some(&address.owner)
            && !coordinator.ready_index.contains(&address.ordinal)
    }

    fn validates_at_raw_context(
        &self,
        context: super::LifecycleContext,
        address: super::work_registry::ConcreteWorkAddress,
        installed_digest: super::LifecycleDigest,
    ) -> bool {
        let Ok((physical, universe, consumed)) = self.candidate.physical_geometry.normalized()
        else {
            return false;
        };
        let slot = super::PhysicalSlotId::for_capacity(super::CapacityClass::Consensus, 0);
        self.digest() == installed_digest
            && self.candidate.replay_authority_is_exact(context)
            && self.candidate.causal_root == address.owner.causal_root()
            && self.candidate.work_class == LifecycleWorkClass::Broadcast
            && self.candidate.initial_state == super::InitialLifecycleState::Ready
            && self.candidate.producer_turn.is_none()
            && address.slot == slot
            && physical == std::collections::BTreeMap::from([(slot, installed_digest)])
            && universe == std::collections::BTreeSet::from([slot])
            && consumed == universe
    }

    /// Recheck the sealed child at one deterministic registry address.
    pub(super) fn validates_at(
        &self,
        verified: &VerifiedHeightContext,
        address: super::work_registry::ConcreteWorkAddress,
        installed_digest: super::LifecycleDigest,
    ) -> bool {
        let context = projection::lifecycle_context(verified.context());
        let Ok((physical, universe, consumed)) = self.candidate.physical_geometry.normalized()
        else {
            return false;
        };
        let slot = super::PhysicalSlotId::for_capacity(super::CapacityClass::Consensus, 0);
        self.pending.exactly_binds_adapter_effect(&self.effect)
            && self.digest() == installed_digest
            && self.candidate.replay_authority_is_exact(context)
            && self.candidate.causal_root == address.owner.causal_root()
            && self.candidate.work_class == super::LifecycleWorkClass::Broadcast
            && self.candidate.initial_state == super::InitialLifecycleState::Ready
            && self.candidate.producer_turn.is_none()
            && address.slot == slot
            && physical == std::collections::BTreeMap::from([(slot, installed_digest)])
            && universe == std::collections::BTreeSet::from([slot])
            && consumed == universe
    }

    /// Revalidate the complete broadcast binding and canonical admission.
    fn validates_from_sign(
        &self,
        verified: &VerifiedHeightContext,
        sign_effect: &AdapterEffect,
        sign_pending: &PendingRuntimeEffectBinding,
    ) -> bool {
        sign_pending
            .project_signed_broadcast_successor(sign_effect, &self.effect)
            .as_ref()
            == Some(&self.pending)
            && super::replay_authority::exact_signed_broadcast_successor_candidate(
                verified,
                &self.effect,
                &self.pending,
            )
            .as_ref()
                == Some(&self.candidate)
    }

    /// Revalidate this child against one exact standalone recovered WAL Vote.
    pub(super) fn validates_from_next_wal_vote(
        &self,
        verified: &VerifiedHeightContext,
        parent: &super::replay_authority::RecoveredLifecycleNextWalVoteCandidateProjectionV1,
    ) -> bool {
        self.cold_proposal_output.is_none()
            && parent.signed_broadcast_successor_is_exact(
                verified,
                &self.effect,
                &self.pending,
                &self.candidate,
            )
    }
}

impl RecoveredLifecycleSignedBroadcastAndSignProjectionV1 {
    fn children_are_exact(&self, verified: &VerifiedHeightContext) -> bool {
        let context = projection::lifecycle_context(verified.context());
        self.broadcast
            .pending
            .exactly_binds_adapter_effect(&self.broadcast.effect)
            && self.broadcast.candidate.replay_authority_is_exact(context)
            && self.broadcast.candidate.work_class == LifecycleWorkClass::Broadcast
            && self.broadcast.candidate.initial_state == InitialLifecycleState::Ready
            && self.broadcast.candidate.payload == DurablePayloadReference::None
            && self.broadcast.candidate.producer_turn.is_none()
            && self
                .broadcast
                .cold_proposal_output
                .as_ref()
                .is_none_or(|output| output.matches_broadcast(&self.broadcast.effect))
            && self.next_sign.is_exact(verified)
            && self
                .next_sign
                .is_distinct_from_broadcast_candidate(&self.broadcast.candidate)
    }

    /// Compare the retained Broadcast with one independently frame-authenticated child.
    pub(super) fn broadcast_exactly_matches(
        &self,
        expected: &RecoveredLifecycleSignedBroadcastProjectionV1,
    ) -> bool {
        self.broadcast.exactly_matches(expected)
    }

    /// Mint one comparison-only authority for replaying the historical Sign.
    ///
    /// The executable pair stays retained here for Ledger/registry recovery.
    /// Affinity prevents two adapter startups from advancing the same durable
    /// pair to its destination next-Sign fence during one owner assembly.
    pub(super) fn project_cold_adapter_replay_authority(
        &mut self,
        verified: &VerifiedHeightContext,
    ) -> Option<crate::sumeragi::v2::RecoveredLifecycleSignBroadcastAndSignColdAdapterAuthorityV1>
    {
        if self.cold_adapter_authority_minted || !self.children_are_exact(verified) {
            return None;
        }
        let next_sign = self.next_sign.project_cold_adapter_next_sign(
            verified,
            RecoveredLifecycleSignBroadcastProjectionPermitV1::new(),
        )?;
        let authority = crate::sumeragi::v2::RecoveredLifecycleSignBroadcastAndSignColdAdapterAuthorityV1::from_recovered_wal(
            RecoveredLifecycleSignBroadcastProjectionPermitV1::new(),
            self.broadcast.effect.clone(),
            next_sign,
        )?;
        self.cold_adapter_authority_minted = true;
        Some(authority)
    }

    /// Clone both inert admissions only under the transition module's affine permit.
    ///
    /// The executable Broadcast and next-Sign carriers remain owned here; the
    /// tuple can exist only inside the staged, pre-fsync two-child transition.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn project_transition_candidates(
        &self,
        permit: super::body_pipeline_transition::RecoveredLifecycleBroadcastAndSignTransitionProjectionPermitV1,
    ) -> (CandidateAdmission, CandidateAdmission) {
        let next_sign = self
            .next_sign
            .project_candidate_for_combined_transition(permit);
        (self.broadcast.candidate.clone(), next_sign)
    }

    /// Rejoin both opaque children to one staged coordinator successor.
    ///
    /// This is the sole live bridge between inert admission staging and the
    /// still-unsplit executable projection. It returns no candidate, effect,
    /// pending, WAL, or body constituent.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn matches_staged_ready_children(
        &self,
        verified: &VerifiedHeightContext,
        coordinator: &super::LifecycleCoordinator,
        broadcast_ordinal: u128,
        next_sign_ordinal: u128,
    ) -> bool {
        let Some(expected_next) = broadcast_ordinal.checked_add(1) else {
            return false;
        };
        if expected_next != next_sign_ordinal
            || coordinator.high_water != next_sign_ordinal
            || !self.children_are_exact(verified)
        {
            return false;
        }
        let (Some(broadcast_record), Some(next_sign_record)) = (
            coordinator.records.get(&broadcast_ordinal),
            coordinator.records.get(&next_sign_ordinal),
        ) else {
            return false;
        };
        let (Some((&broadcast_slot, &broadcast_digest)), Some((&next_slot, &next_digest))) = (
            broadcast_record.physical_slots.first_key_value(),
            next_sign_record.physical_slots.first_key_value(),
        ) else {
            return false;
        };
        let (Some(broadcast_address), Some(next_sign_address)) = (
            super::work_registry::ConcreteWorkAddress::new(
                broadcast_record.owner,
                broadcast_ordinal,
                broadcast_slot,
            ),
            super::work_registry::ConcreteWorkAddress::new(
                next_sign_record.owner,
                next_sign_ordinal,
                next_slot,
            ),
        ) else {
            return false;
        };
        broadcast_record.physical_slots.len() == 1
            && next_sign_record.physical_slots.len() == 1
            && broadcast_record.owner.causal_root() == self.broadcast.candidate.causal_root
            && next_sign_record.owner.first_admission_ordinal() == next_sign_ordinal
            && broadcast_record.owner != next_sign_record.owner
            && self.broadcast.matches_current_ready_record(
                coordinator.active_context,
                broadcast_address,
                broadcast_digest,
                coordinator,
            )
            && self.next_sign.matches_current_ready_record(
                verified,
                next_sign_address,
                next_digest,
                coordinator,
            )
    }

    /// Split executable ownership only in the assertion-only post-fsync registry tail.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn into_registry_children(
        self,
        _permit: super::work_registry::RecoveredLifecycleBroadcastAndSignRegistryCommitPermitV1,
    ) -> (
        RecoveredLifecycleSignedBroadcastProjectionV1,
        super::replay_authority::RecoveredLifecycleNextWalVoteCandidateProjectionV1,
    ) {
        (self.broadcast, self.next_sign)
    }

    /// Compare both opaque children with two exact fresh standalone rows.
    ///
    /// The Broadcast retains its inherited Sign causal root. The follow-on
    /// Sign instead owns the independent causal root reconstructed from its
    /// own WAL replay source. Neither candidate is released by this oracle.
    pub(super) fn exactly_matches_fresh_records(
        &self,
        context: super::LifecycleContext,
        broadcast_record: &super::ledger::LifecycleLedgerRecordV1,
        next_sign_record: &super::ledger::LifecycleLedgerRecordV1,
    ) -> bool {
        let broadcast_owner = broadcast_record.owner();
        broadcast_record.ordinal() != next_sign_record.ordinal()
            && self
                .next_sign
                .is_distinct_from_broadcast_candidate(&self.broadcast.candidate)
            && broadcast_owner.causal_root() == self.broadcast.candidate.causal_root
            && self.broadcast.candidate.key.context() == context.id()
            && self.broadcast.candidate.key.round().height() == context.height()
            && self
                .broadcast
                .exactly_matches_record(broadcast_record, broadcast_owner)
            && self
                .next_sign
                .exactly_matches_fresh_record(context, next_sign_record)
    }

    /// Splice both candidates only after the complete fresh-row pair matches.
    ///
    /// The preflight checks both keys before either insertion, so a rejected
    /// cold cut cannot leave a partial candidate census.
    pub(super) fn splice_candidates_from_records(
        &self,
        context: super::LifecycleContext,
        broadcast_record: &super::ledger::LifecycleLedgerRecordV1,
        next_sign_record: &super::ledger::LifecycleLedgerRecordV1,
        candidates: &mut std::collections::BTreeMap<super::LifecycleKey, CandidateAdmission>,
    ) -> bool {
        if !self.exactly_matches_fresh_records(context, broadcast_record, next_sign_record)
            || candidates.contains_key(&self.broadcast.candidate.key)
            || !self.next_sign.is_absent_from_candidates(candidates)
        {
            return false;
        }
        let broadcast_inserted = candidates
            .insert(
                self.broadcast.candidate.key,
                self.broadcast.candidate.clone(),
            )
            .is_none();
        let next_inserted = self.next_sign.splice_candidate_from_fresh_record(
            context,
            next_sign_record,
            candidates,
        );
        if !broadcast_inserted || !next_inserted {
            candidates.remove(&self.broadcast.candidate.key);
            return false;
        }
        true
    }

    /// Require the complete cold census to retain both exact opaque children.
    ///
    /// Unrelated authenticated carriers are deliberately preserved: the pair
    /// need not end at the ledger high-water mark and cannot claim the whole
    /// height census as its own.
    pub(super) fn owns_spliced_candidates(
        &self,
        candidates: &std::collections::BTreeMap<super::LifecycleKey, CandidateAdmission>,
    ) -> bool {
        candidates.get(&self.broadcast.candidate.key) == Some(&self.broadcast.candidate)
            && self.next_sign.owns_spliced_candidate(candidates)
    }
}

fn project_recovered_signed_broadcast(
    verified: &VerifiedHeightContext,
    sign_effect: &AdapterEffect,
    sign_pending: &PendingRuntimeEffectBinding,
    broadcast_effect: &AdapterEffect,
) -> Option<RecoveredLifecycleSignedBroadcastProjectionV1> {
    let broadcast_pending =
        sign_pending.project_signed_broadcast_successor(sign_effect, broadcast_effect)?;
    let candidate = super::replay_authority::exact_signed_broadcast_successor_candidate(
        verified,
        broadcast_effect,
        &broadcast_pending,
    )?;
    let projection = RecoveredLifecycleSignedBroadcastProjectionV1 {
        effect: broadcast_effect.clone(),
        pending: broadcast_pending,
        candidate,
        cold_proposal_output: None,
    };
    projection
        .validates_from_sign(verified, sign_effect, sign_pending)
        .then_some(projection)
}

/// Rejoin one adapter-authenticated signed Broadcast to its exact standalone
/// recovered WAL Vote without exposing either projection's constituent parts.
pub(super) fn project_recovered_next_wal_vote_signed_broadcast(
    parent: &super::replay_authority::RecoveredLifecycleNextWalVoteCandidateProjectionV1,
    verified: &VerifiedHeightContext,
    authority: crate::sumeragi::v2::RecoveredLifecycleSignBroadcastProjectionAuthorityV1,
) -> Option<(
    super::work_registry::RecoveredLifecycleSignDispatchKeyV1,
    RecoveredLifecycleSignedBroadcastProjectionV1,
)> {
    let (key, broadcast) = authority
        .consume_for_recovered_wal(RecoveredLifecycleSignBroadcastProjectionPermitV1::new());
    let closed = parent.project_authenticated_signed_broadcast(verified, broadcast)?;
    let (effect, pending, candidate) =
        closed.consume_for_recovered_wal(RecoveredLifecycleSignBroadcastProjectionPermitV1::new());
    let projection = RecoveredLifecycleSignedBroadcastProjectionV1 {
        effect,
        pending,
        candidate,
        cold_proposal_output: None,
    };
    projection
        .validates_from_next_wal_vote(verified, parent)
        .then_some((key, projection))
}

/// Rejoin an adapter-authenticated Broadcast-and-next-Sign pair to its exact
/// standalone recovered WAL Vote parent.
///
/// The adapter authority is unpacked only here. Its next Vote remains sealed
/// through body/WAL authentication and is converted to executable admission
/// before the pair is returned as the existing opaque combined projection.
pub(super) fn project_recovered_next_wal_vote_signed_broadcast_and_sign(
    parent: &super::replay_authority::RecoveredLifecycleNextWalVoteCandidateProjectionV1,
    verified: &VerifiedHeightContext,
    authority: crate::sumeragi::v2::RecoveredLifecycleSignBroadcastAndSignAuthorityV1,
) -> Option<(
    super::work_registry::RecoveredLifecycleSignDispatchKeyV1,
    RecoveredLifecycleSignedBroadcastAndSignProjectionV1,
)> {
    let (key, broadcast, next_sign) = authority
        .consume_for_recovered_wal(RecoveredLifecycleSignBroadcastProjectionPermitV1::new());
    if !next_sign.matches_verified_height(verified) {
        return None;
    }
    let next_sign =
        project_recovered_lifecycle_next_wal_vote_candidate(verified, next_sign).ok()?;
    let closed = parent.project_authenticated_signed_broadcast(verified, broadcast)?;
    let (effect, pending, candidate) =
        closed.consume_for_recovered_wal(RecoveredLifecycleSignBroadcastProjectionPermitV1::new());
    let broadcast = RecoveredLifecycleSignedBroadcastProjectionV1 {
        effect,
        pending,
        candidate,
        cold_proposal_output: None,
    };
    let combined = RecoveredLifecycleSignedBroadcastAndSignProjectionV1 {
        broadcast,
        next_sign,
        cold_adapter_authority_minted: false,
    };
    (combined
        .broadcast
        .validates_from_next_wal_vote(verified, parent)
        && combined.children_are_exact(verified))
    .then_some((key, combined))
}

/// Dedicated durable/registry handoff for one recovered control Sign.
///
/// This carrier permanently retains the complete projection beside its exact
/// installed geometry. Only comparison oracles and the paired digest are
/// available; there is no effect, pending, replay, locator, candidate, byte,
/// or ordinal extraction surface.
#[must_use = "the recovered control carrier must remain installed in the concrete registry"]
pub(super) struct DurableRecoveredWalControlSignCarrierV1 {
    projection: AuthenticatedRecoveredWalControlProjection,
    owner: super::OwnerId,
    ordinal: u128,
    slot: super::PhysicalSlotId,
    digest: super::LifecycleDigest,
}

/// Closed runtime projection of one exact recovered Decision Fetch.
///
/// The authenticated WAL identity, complete Fetch effect, replay evidence,
/// pending owner, and logical candidate remain inseparable and have no parts
/// API outside this module.
#[must_use = "a recovered Decision Fetch projection must enter exact storage recovery"]
pub(in crate::sumeragi) struct AuthenticatedRecoveredWalDecisionFetchProjection {
    wal_identity: RecoveredWalFrameIdentity,
    replay_evidence: RecoveredWalDecisionFetchReplayEvidenceV1,
    effect: AdapterEffect,
    pending: PendingRuntimeEffectBinding,
    candidate: CandidateAdmission,
}

/// Opaque first body-backed successor of one recovered WAL Decision Fetch.
///
/// The original Fetch projection remains installed while this value is
/// prepared. This successor owns only the reducer-derived Store effect and its
/// predecessor-derived binding, exact BodyFrame, and recovered-WAL replay
/// authority. It is never represented as ordinary pending effect work.
#[must_use = "recovered Decision Store projection must enter one closed publication"]
pub(in crate::sumeragi) struct RecoveredDecisionFetchStoreProjectionV1 {
    effect: AdapterEffect,
    pending: PendingRuntimeEffectBinding,
    body: RecoveredDecisionFetchStoreBodyAuthorityV1,
    candidate: CandidateAdmission,
}

/// Carrier-authenticated input for the direct recovered Fetch body preview.
#[must_use = "recovered Decision Store adapter authority must be consumed exactly once"]
pub(in crate::sumeragi) struct RecoveredDecisionFetchStoreAdapterAuthorityV1 {
    tag: crate::sumeragi::v2_core::EventTag,
    body: RecoveredDecisionFetchStoreBodyAuthorityV1,
}

impl RecoveredDecisionFetchStoreAdapterAuthorityV1 {
    /// Borrow the exact reducer tag retained by the WAL Fetch.
    pub(in crate::sumeragi) const fn tag(&self) -> crate::sumeragi::v2_core::EventTag {
        self.tag
    }

    /// Borrow the body manifest only for the fixed direct adapter preview.
    pub(in crate::sumeragi) const fn manifest(&self) -> &wire::PayloadManifest {
        self.body.manifest()
    }

    /// Consume the authority into its still-opaque body frame after preview.
    pub(in crate::sumeragi) fn into_body(self) -> RecoveredDecisionFetchStoreBodyAuthorityV1 {
        self.body
    }
}

/// Closed pending-binding lineage for the fixed recovered-Decision body preview.
///
/// The original Fetch binding remains inside its authenticated projection.
/// These three successors are derived in order and expose no causal key,
/// effect identity, statement, or constituent binding.
#[must_use = "recovered Decision pending lineage must remain inside its staged composite"]
pub(in crate::sumeragi) struct RecoveredDecisionApplyPendingLineageV1 {
    store: PendingRuntimeEffectBinding,
    validate: PendingRuntimeEffectBinding,
    apply: PendingRuntimeEffectBinding,
}

impl RecoveredDecisionApplyPendingLineageV1 {
    /// Recheck each predecessor-derived binding against its exact stage effect.
    pub(in crate::sumeragi) fn exactly_matches(
        &self,
        store: &AdapterEffect,
        validate: &AdapterEffect,
        apply: &AdapterEffect,
    ) -> bool {
        self.store.exactly_binds_adapter_effect(store)
            && self.validate.exactly_binds_adapter_effect(validate)
            && self.apply.exactly_binds_adapter_effect(apply)
    }

    /// Consume the three fixed bindings into one candidate lineage and retain
    /// only the final Apply binding needed by the live carrier.
    ///
    /// Failure returns the intact pending lineage. This keeps projection
    /// one-shot without cloning any runtime ownership token.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::sumeragi) fn project_candidate_lineage(
        self,
        permit: RecoveredDecisionApplyCandidateProjectionPermit,
        replay: &RecoveredDecisionApplyReplayLineageV1,
        verified: &VerifiedHeightContext,
        durable: &DurableBodyReceipt,
        store: &AdapterEffect,
        validate: &AdapterEffect,
        apply: &AdapterEffect,
        fetch: &AuthenticatedRecoveredWalDecisionFetchProjection,
    ) -> Result<
        (
            RecoveredDecisionApplyCandidateLineageV1,
            PendingRuntimeEffectBinding,
        ),
        Self,
    > {
        let Self {
            store: store_pending,
            validate: validate_pending,
            apply: apply_pending,
        } = self;
        let lineage = replay.project_candidate_lineage(
            permit,
            verified,
            durable,
            store,
            &store_pending,
            validate,
            &validate_pending,
            apply,
            &apply_pending,
        );
        let Some(lineage) = lineage else {
            return Err(Self {
                store: store_pending,
                validate: validate_pending,
                apply: apply_pending,
            });
        };
        if !fetch.owns_apply_lineage(verified, &lineage) {
            return Err(Self {
                store: store_pending,
                validate: validate_pending,
                apply: apply_pending,
            });
        }
        drop(store_pending);
        drop(validate_pending);
        Ok((lineage, apply_pending))
    }
}

/// Dedicated durable/registry carrier for one recovered Decision Fetch.
#[must_use = "the recovered Decision Fetch carrier must remain installed"]
pub(super) struct DurableRecoveredWalDecisionFetchCarrierV1 {
    projection: AuthenticatedRecoveredWalDecisionFetchProjection,
    owner: super::OwnerId,
    ordinal: u128,
    slot: super::PhysicalSlotId,
    digest: super::LifecycleDigest,
}

impl AuthenticatedRecoveredWalControlProjection {
    /// Seal the runtime-private recovered-frame projection.
    pub(in crate::sumeragi) fn from_runtime_projection(
        wal_identity: RecoveredWalFrameIdentity,
        replay_evidence: RecoveredWalControlReplayEvidenceV1,
        effect: AdapterEffect,
        pending: PendingRuntimeEffectBinding,
        candidate: RecoveredWalControlCandidateProjectionV1,
    ) -> Self {
        Self {
            wal_identity,
            replay_evidence,
            effect,
            pending,
            candidate: candidate.into_candidate(),
        }
    }

    /// Revalidate every nested authority without releasing any component.
    pub(super) fn is_exact(&self, verified: &VerifiedHeightContext) -> bool {
        self.wal_identity.is_exact()
            && self
                .replay_evidence
                .exactly_matches_recovered_control(self.wal_identity, &self.effect)
            && self.pending.exactly_binds_adapter_effect(&self.effect)
            && self
                .replay_evidence
                .project_recovered_control_candidate_for_comparison(
                    verified,
                    self.wal_identity,
                    &self.effect,
                    &self.pending,
                    &self.candidate,
                )
            && control_candidate_shape_is_exact(&self.candidate)
    }

    /// Compare the sealed candidate with one exact lifecycle context.
    pub(super) fn belongs_to_context(&self, context: super::LifecycleContext) -> bool {
        self.candidate.key.context() == context.id()
            && self.candidate.key.round().height() == context.height()
    }

    /// Whether a durable row has this projection's exact semantic key.
    pub(super) fn names_record(&self, record: &super::ledger::LifecycleLedgerRecordV1) -> bool {
        record.key() == Some(self.candidate.key)
    }

    /// Compare every persisted admission field, including standalone owner identity.
    pub(super) fn exactly_matches_record(
        &self,
        record: &super::ledger::LifecycleLedgerRecordV1,
    ) -> bool {
        self.names_record(record)
            && record.owner().causal_root() == self.candidate.causal_root
            && record.owner().first_admission_ordinal() == record.ordinal()
            && record.work_class() == Some(self.candidate.work_class)
            && record.stage() == Some(self.candidate.stage)
            && record.terminal() == Some(None)
            && record.reconstruction_source() == self.candidate.reconstruction_source
            && record.durable_payload() == Some(DurablePayloadReference::None)
            && record.continuation() == Some(super::schema::DurableContinuation::None)
            && record.replay_matches_candidate(&self.candidate)
    }

    fn signed_broadcast_edge(&self) -> Option<super::schema::DurableContinuationEdge> {
        match self.candidate.stage.kind() {
            super::LifecycleStageKind::SignProposal => {
                Some(super::schema::DurableContinuationEdge::SignProposalToBroadcast)
            }
            super::LifecycleStageKind::SignTimeoutVote => {
                Some(super::schema::DurableContinuationEdge::SignTimeoutToBroadcast)
            }
            _ => None,
        }
    }

    /// Compare this exact Sign as the Advanced parent of one durable Broadcast.
    pub(super) fn exactly_matches_advanced_record(
        &self,
        record: &super::ledger::LifecycleLedgerRecordV1,
        child_ordinal: u128,
    ) -> bool {
        self.names_record(record)
            && record.owner().causal_root() == self.candidate.causal_root
            && record.owner().first_admission_ordinal() == record.ordinal()
            && record.work_class() == Some(self.candidate.work_class)
            && record.stage() == Some(self.candidate.stage)
            && record.terminal() == Some(Some(super::TerminalOutcome::Advanced))
            && record.reconstruction_source() == self.candidate.reconstruction_source
            && record.durable_payload() == Some(DurablePayloadReference::None)
            && self.signed_broadcast_edge().is_some_and(|edge| {
                record.continuation()
                    == Some(super::schema::DurableContinuation::successor(
                        edge,
                        child_ordinal,
                    ))
            })
            && record.replay_matches_candidate(&self.candidate)
    }

    /// Reconstruct and roster-authenticate the selected durable Broadcast child.
    pub(super) fn recover_durable_signed_broadcast(
        &self,
        verified: &VerifiedHeightContext,
        child: super::replay_authority::DurableRecoveredSignedBroadcastChildV1,
    ) -> Option<RecoveredLifecycleSignedBroadcastProjectionV1> {
        let broadcast = child
            .consume_for_recovered_wal(RecoveredLifecycleSignBroadcastProjectionPermitV1::new());
        let AdapterEffect::Broadcast(message) = &broadcast else {
            return None;
        };
        verified.verify_consensus_message(message).ok()?;
        project_recovered_signed_broadcast(verified, &self.effect, &self.pending, &broadcast)
    }

    /// Seal the already-authenticated durable child for cold adapter replay.
    pub(super) fn project_cold_adapter_authority(
        &self,
        verified: &VerifiedHeightContext,
        broadcast: &RecoveredLifecycleSignedBroadcastProjectionV1,
    ) -> Option<crate::sumeragi::v2::RecoveredLifecycleSignColdAdapterAuthorityV1> {
        if !broadcast.validates_from_sign(verified, &self.effect, &self.pending) {
            return None;
        }
        let AdapterEffect::Sign { tag, request } = &self.effect else {
            return None;
        };
        crate::sumeragi::v2::RecoveredLifecycleSignColdAdapterAuthorityV1::from_recovered_wal(
            RecoveredLifecycleSignBroadcastProjectionPermitV1::new(),
            *tag,
            request.clone(),
            broadcast.effect.clone(),
        )
    }

    /// Preview one exact durable Proposal Broadcast and its follow-on Vote Sign.
    ///
    /// Unlike live completion, this cold path has no worker dispatch key. The
    /// recovered control WAL projection supplies the historical unsigned
    /// Proposal, while the frame-authenticated Broadcast supplies only its
    /// verified signature. The adapter retains both children unpublished until
    /// the next Vote has rejoined this height's exact body store and WAL frame.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn prepare_cold_signed_broadcast_and_sign(
        &self,
        verified: &VerifiedHeightContext,
        startup: crate::sumeragi::v2::ProductionLifecycleAdapterStartupV1,
        broadcast: &RecoveredLifecycleSignedBroadcastProjectionV1,
    ) -> Result<
        crate::sumeragi::v2::PreparedRecoveredLifecycleSignedBroadcastAndSignColdPreviewV1,
        &'static str,
    > {
        if !self.is_exact(verified)
            || !broadcast.validates_from_sign(verified, &self.effect, &self.pending)
        {
            return Err("recovered control Broadcast changed before cold pair preview");
        }
        let AdapterEffect::Sign { tag, request } = &self.effect else {
            return Err("recovered control parent is not a Sign effect");
        };
        let authority = crate::sumeragi::v2::RecoveredLifecycleSignedBroadcastColdPreviewAuthorityV1::from_recovered_wal(
            RecoveredLifecycleSignBroadcastProjectionPermitV1::new(),
            *tag,
            request.clone(),
            broadcast.effect.clone(),
        )
        .ok_or("recovered control Broadcast is not an exact Proposal child")?;
        startup.prepare_recovered_lifecycle_signed_broadcast_and_sign(verified, authority)
    }

    /// Rejoin the cold adapter/body seal to this exact control WAL parent.
    ///
    /// The returned startup is still at the historical Sign fence. The caller
    /// must consume the comparison-only cold replay authority from the combined
    /// projection and advance that same startup before opening live ownership.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn project_authenticated_cold_signed_broadcast_and_sign(
        &self,
        verified: &VerifiedHeightContext,
        seal: crate::sumeragi::v2::RecoveredLifecycleSignedBroadcastAndSignColdSealV1,
    ) -> Option<(
        crate::sumeragi::v2::ProductionLifecycleAdapterStartupV1,
        RecoveredLifecycleSignedBroadcastAndSignProjectionV1,
    )> {
        if !self.is_exact(verified) {
            return None;
        }
        let (startup, broadcast, next_sign, cold_proposal_output) = seal
            .consume_for_recovered_wal(RecoveredLifecycleSignBroadcastProjectionPermitV1::new());
        if !next_sign.matches_verified_height(verified) {
            return None;
        }
        let next_sign =
            project_recovered_lifecycle_next_wal_vote_candidate(verified, next_sign).ok()?;
        let mut broadcast =
            project_recovered_signed_broadcast(verified, &self.effect, &self.pending, &broadcast)?;
        if cold_proposal_output
            .as_ref()
            .is_some_and(|output| !output.matches_broadcast(&broadcast.effect))
        {
            return None;
        }
        broadcast.cold_proposal_output = cold_proposal_output;
        let combined = RecoveredLifecycleSignedBroadcastAndSignProjectionV1 {
            broadcast,
            next_sign,
            cold_adapter_authority_minted: false,
        };
        combined
            .children_are_exact(verified)
            .then_some((startup, combined))
    }

    /// Prove one opened ledger contains this exact standalone row once.
    pub(super) fn exactly_matches_ledger_at(
        &self,
        ledger: &super::ledger::LifecycleLedgerV1,
        ordinal: u128,
    ) -> bool {
        if !self.belongs_to_context(ledger.context()) {
            return false;
        }
        let mut records = ledger
            .records()
            .iter()
            .filter(|record| self.names_record(record));
        let Some(record) = records.next() else {
            return false;
        };
        let exact_owner = super::OwnerId::new(self.candidate.causal_root, ordinal);
        records.next().is_none()
            && ledger
                .records()
                .iter()
                .filter(|candidate| candidate.owner() == exact_owner)
                .count()
                == 1
            && record.ordinal() == ordinal
            && self.exactly_matches_record(record)
    }

    /// Build the sole fresh standalone Ready row at the ledger-selected ordinal.
    pub(super) fn fresh_record(
        &self,
        ordinal: u128,
    ) -> Result<super::ledger::LifecycleLedgerRecordV1, super::ledger::LifecycleLedgerError> {
        super::ledger::LifecycleLedgerRecordV1::new(
            self.candidate.key,
            super::OwnerId::new(self.candidate.causal_root, ordinal),
            ordinal,
            self.candidate.work_class,
            self.candidate.stage,
            None,
            self.candidate.reconstruction_source,
            self.candidate.payload,
            self.candidate.replay_authority.clone(),
            super::schema::DurableContinuation::None,
        )
    }

    /// Insert the exact candidate only after the installed ledger row matches it.
    pub(super) fn splice_candidate_from_record(
        &self,
        record: &super::ledger::LifecycleLedgerRecordV1,
        candidates: &mut std::collections::BTreeMap<super::LifecycleKey, CandidateAdmission>,
    ) -> bool {
        self.exactly_matches_record(record)
            && !candidates.contains_key(&self.candidate.key)
            && candidates
                .insert(self.candidate.key, self.candidate.clone())
                .is_none()
    }

    /// Return whether recovery retained this one exact candidate and no substitute.
    pub(super) fn owns_spliced_candidate(
        &self,
        candidates: &std::collections::BTreeMap<super::LifecycleKey, CandidateAdmission>,
    ) -> bool {
        candidates.get(&self.candidate.key) == Some(&self.candidate)
    }

    /// Match a concrete registry address and digest without exposing effect or pending parts.
    pub(super) fn validates_installation(
        &self,
        owner: super::OwnerId,
        ordinal: u128,
        slot: super::PhysicalSlotId,
        digest: super::LifecycleDigest,
    ) -> bool {
        let Ok((physical, universe, consumed)) = self.candidate.physical_geometry.normalized()
        else {
            return false;
        };
        owner == super::OwnerId::new(self.candidate.causal_root, ordinal)
            && slot == super::PhysicalSlotId::for_capacity(super::CapacityClass::Effect, 0)
            && physical.len() == 1
            && universe.len() == 1
            && consumed == universe
            && physical.get(&slot) == Some(&digest)
            && digest == super::LifecycleDigest::new(*self.pending.exact_effect_identity().as_ref())
    }

    /// Match the exact Ready coordinator row, metadata, indexes, geometry, and carrier.
    pub(super) fn matches_current_ready_record(
        &self,
        owner: super::OwnerId,
        ordinal: u128,
        slot: super::PhysicalSlotId,
        digest: super::LifecycleDigest,
        coordinator: &super::LifecycleCoordinator,
    ) -> bool {
        let Ok((physical, universe, consumed)) = self.candidate.physical_geometry.normalized()
        else {
            return false;
        };
        let Some(record) = coordinator.records.get(&ordinal) else {
            return false;
        };
        let Some(metadata) = coordinator.durable_records.get(&ordinal) else {
            return false;
        };
        self.validates_installation(owner, ordinal, slot, digest)
            && coordinator.fault.is_none()
            && coordinator.active_context.id() == self.candidate.key.context()
            && coordinator.active_context.height() == self.candidate.key.round().height()
            && coordinator.high_water >= ordinal
            && record.key == self.candidate.key
            && record.owner == owner
            && record.ordinal == ordinal
            && record.work_class == self.candidate.work_class
            && record.stage == self.candidate.stage
            && record.state == super::LifecycleState::Ready
            && record.physical_slots == physical
            && record.episode.slot_universe == universe
            && record.episode.consumed_slots == consumed
            && physical.get(&slot) == Some(&digest)
            && metadata.matches_admission(&self.candidate)
            && coordinator.key_index.get(&self.candidate.key) == Some(&ordinal)
            && coordinator.owner_index.get(&self.candidate.causal_root) == Some(&owner)
            && coordinator.ready_index.contains(&ordinal)
    }

    fn matches_claimed_record(
        &self,
        owner: super::OwnerId,
        ordinal: u128,
        slot: super::PhysicalSlotId,
        digest: super::LifecycleDigest,
        coordinator: &super::LifecycleCoordinator,
        lease: &super::TurnLease,
    ) -> bool {
        let Ok((physical, universe, consumed)) = self.candidate.physical_geometry.normalized()
        else {
            return false;
        };
        let (Some(record), Some(metadata)) = (
            coordinator.records.get(&ordinal),
            coordinator.durable_records.get(&ordinal),
        ) else {
            return false;
        };
        self.validates_installation(owner, ordinal, slot, digest)
            && coordinator.fault.is_none()
            && coordinator.active_context.id() == self.candidate.key.context()
            && coordinator.active_context.height() == self.candidate.key.round().height()
            && coordinator.active_lease.as_ref() == Some(lease)
            && record.key == self.candidate.key
            && record.owner == owner
            && record.ordinal == ordinal
            && record.work_class == self.candidate.work_class
            && record.stage == self.candidate.stage
            && record.state == super::LifecycleState::Claimed(lease.id())
            && lease.key() == record.key
            && lease.owner() == record.owner
            && lease.ordinal() == record.ordinal
            && lease.work_class() == record.work_class
            && lease.stage() == record.stage
            && lease.physical_slots() == &record.physical_slots
            && record.physical_slots == physical
            && record.episode.slot_universe == universe
            && record.episode.consumed_slots == consumed
            && physical.get(&slot) == Some(&digest)
            && metadata.matches_admission(&self.candidate)
            && coordinator.key_index.get(&self.candidate.key) == Some(&ordinal)
            && coordinator.owner_index.get(&self.candidate.causal_root) == Some(&owner)
            && !coordinator.ready_index.contains(&ordinal)
    }

    /// Consume the projection into its one exact dedicated registry carrier.
    pub(super) fn into_durable_carrier(
        self,
        owner: super::OwnerId,
        ordinal: u128,
        slot: super::PhysicalSlotId,
    ) -> Result<DurableRecoveredWalControlSignCarrierV1, Self> {
        let Ok((physical, universe, consumed)) = self.candidate.physical_geometry.normalized()
        else {
            return Err(self);
        };
        let Some(&digest) = physical.get(&slot) else {
            return Err(self);
        };
        if physical.len() != 1
            || universe.len() != 1
            || consumed != universe
            || !self.validates_installation(owner, ordinal, slot, digest)
        {
            return Err(self);
        }
        Ok(DurableRecoveredWalControlSignCarrierV1 {
            projection: self,
            owner,
            ordinal,
            slot,
            digest,
        })
    }
}

impl DurableRecoveredWalControlSignCarrierV1 {
    /// Return the digest only while it remains paired with the sealed carrier.
    pub(super) const fn installed_digest(&self) -> super::LifecycleDigest {
        self.digest
    }

    /// Compare the complete installed address and physical identity.
    pub(super) fn validates_at(
        &self,
        owner: super::OwnerId,
        ordinal: u128,
        slot: super::PhysicalSlotId,
        digest: super::LifecycleDigest,
    ) -> bool {
        self.owner == owner
            && self.ordinal == ordinal
            && self.slot == slot
            && self.digest == digest
            && self
                .projection
                .validates_installation(owner, ordinal, slot, digest)
    }

    /// Reopen and match the exact durable standalone row.
    pub(super) fn validates_in_store(&self, store: &super::ledger::LifecycleLedgerStoreV1) -> bool {
        store.revalidates_authenticated_wal_control_sign(&self.projection, self.ordinal)
            && self.validates_at(self.owner, self.ordinal, self.slot, self.digest)
    }

    /// Rejoin this standalone carrier to one exact in-memory ledger frame.
    pub(super) fn validates_in_ledger(
        &self,
        verified: &VerifiedHeightContext,
        ledger: &super::ledger::LifecycleLedgerV1,
    ) -> bool {
        self.projection.is_exact(verified)
            && self
                .projection
                .exactly_matches_ledger_at(ledger, self.ordinal)
            && self.validates_at(self.owner, self.ordinal, self.slot, self.digest)
    }

    /// Rejoin an advanced control Sign and its exact live Broadcast child.
    pub(super) fn validates_signed_broadcast_in_ledger(
        &self,
        verified: &VerifiedHeightContext,
        broadcast: &RecoveredLifecycleSignedBroadcastProjectionV1,
        ledger: &super::ledger::LifecycleLedgerV1,
        child_ordinal: u128,
    ) -> bool {
        ledger
            .authenticate_recovered_control_signed_broadcast(verified, &self.projection)
            .is_ok_and(|(observed, parent_ordinal, observed_child)| {
                parent_ordinal == self.ordinal
                    && observed_child == child_ordinal
                    && observed.exactly_matches(broadcast)
            })
    }

    /// Compare the current Ready record, metadata, indexes, geometry, and carrier.
    pub(super) fn matches_current_ready_record(
        &self,
        coordinator: &super::LifecycleCoordinator,
    ) -> bool {
        self.projection.matches_current_ready_record(
            self.owner,
            self.ordinal,
            self.slot,
            self.digest,
            coordinator,
        )
    }

    /// Compare the sole current claimed lease with this complete carrier.
    pub(super) fn matches_claimed_record(
        &self,
        coordinator: &super::LifecycleCoordinator,
        lease: &super::TurnLease,
    ) -> bool {
        self.projection.matches_claimed_record(
            self.owner,
            self.ordinal,
            self.slot,
            self.digest,
            coordinator,
            lease,
        )
    }

    /// Project the complete exact Sign effect into its dedicated worker task.
    ///
    /// The registry-minted identity rehashes the retained tag/request pair.
    /// No effect, request, pending owner, or WAL locator is returned separately.
    pub(super) fn project_recovered_lifecycle_sign_task(
        &self,
        identity: super::work_registry::RecoveredLifecycleSignDispatchIdentityV1,
    ) -> Option<crate::sumeragi::v2_worker::RecoveredLifecycleSignTaskV1> {
        let AdapterEffect::Sign { tag, request } = &self.projection.effect else {
            return None;
        };
        crate::sumeragi::v2_worker::RecoveredLifecycleSignTaskV1::from_registry_projection(
            identity,
            *tag,
            request.clone(),
        )
    }

    /// Project the mandatory signed Broadcast while retaining this exact WAL carrier.
    pub(super) fn project_authenticated_signed_broadcast(
        &self,
        verified: &VerifiedHeightContext,
        authority: crate::sumeragi::v2::RecoveredLifecycleSignBroadcastProjectionAuthorityV1,
    ) -> Option<(
        super::work_registry::RecoveredLifecycleSignDispatchKeyV1,
        RecoveredLifecycleSignedBroadcastProjectionV1,
    )> {
        let (key, broadcast) = authority
            .consume_for_recovered_wal(RecoveredLifecycleSignBroadcastProjectionPermitV1::new());
        let projection = project_recovered_signed_broadcast(
            verified,
            &self.projection.effect,
            &self.projection.pending,
            &broadcast,
        )?;
        Some((key, projection))
    }

    /// Project the exact signed Broadcast while retaining its WAL/body-bound Sign.
    ///
    /// Live publication consumes this pair atomically. Cold owner assembly must
    /// still join it to the frame-bound Ledger pair and complete startup census.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn project_authenticated_signed_broadcast_and_sign(
        &self,
        verified: &VerifiedHeightContext,
        authority: crate::sumeragi::v2::RecoveredLifecycleSignBroadcastAndSignAuthorityV1,
    ) -> Option<(
        super::work_registry::RecoveredLifecycleSignDispatchKeyV1,
        RecoveredLifecycleSignedBroadcastAndSignProjectionV1,
    )> {
        let (key, broadcast, next_sign) = authority
            .consume_for_recovered_wal(RecoveredLifecycleSignBroadcastProjectionPermitV1::new());
        if !next_sign.matches_verified_height(verified) {
            return None;
        }
        let next_sign =
            project_recovered_lifecycle_next_wal_vote_candidate(verified, next_sign).ok()?;
        let projection = project_recovered_signed_broadcast(
            verified,
            &self.projection.effect,
            &self.projection.pending,
            &broadcast,
        )?;
        let combined = RecoveredLifecycleSignedBroadcastAndSignProjectionV1 {
            broadcast: projection,
            next_sign,
            cold_adapter_authority_minted: false,
        };
        combined
            .children_are_exact(verified)
            .then_some((key, combined))
    }

    /// Reconstruct a durable signed child only through this exact control WAL owner.
    pub(super) fn recover_durable_signed_broadcast(
        &self,
        verified: &VerifiedHeightContext,
        child: super::replay_authority::DurableRecoveredSignedBroadcastChildV1,
    ) -> Option<RecoveredLifecycleSignedBroadcastProjectionV1> {
        self.projection
            .recover_durable_signed_broadcast(verified, child)
    }

    /// Bind the durable child back to this exact Sign for cold adapter replay.
    pub(super) fn project_cold_adapter_authority(
        &self,
        verified: &VerifiedHeightContext,
        broadcast: &RecoveredLifecycleSignedBroadcastProjectionV1,
    ) -> Option<crate::sumeragi::v2::RecoveredLifecycleSignColdAdapterAuthorityV1> {
        self.projection
            .project_cold_adapter_authority(verified, broadcast)
    }

    /// Recheck a retained Broadcast successor against this exact control Sign.
    pub(super) fn matches_signed_broadcast(
        &self,
        verified: &VerifiedHeightContext,
        broadcast: &RecoveredLifecycleSignedBroadcastProjectionV1,
    ) -> bool {
        broadcast.validates_from_sign(verified, &self.projection.effect, &self.projection.pending)
    }

    /// Prove the authenticated recovery cut retains this exact logical Sign.
    pub(super) fn owns_recovery(
        &self,
        recovery: &super::open::AuthenticatedLifecycleRecoveryCut,
    ) -> bool {
        recovery.owns_recovered_wal_control_sign(&self.projection)
    }

    /// Reopen the exact Advanced Sign/live Broadcast pair retained by this parent.
    pub(super) fn validates_signed_broadcast_in_store(
        &self,
        verified: &VerifiedHeightContext,
        broadcast: &RecoveredLifecycleSignedBroadcastProjectionV1,
        store: &super::ledger::LifecycleLedgerStoreV1,
        child_ordinal: u128,
    ) -> bool {
        store.revalidates_recovered_control_signed_broadcast(
            verified,
            &self.projection,
            broadcast,
            self.ordinal,
            child_ordinal,
        )
    }

    /// Recheck the storage-only census under this exact control WAL parent.
    pub(super) fn owns_signed_broadcast_recovery(
        &self,
        recovery: &super::open::AuthenticatedLifecycleRecoveryCut,
        broadcast: &RecoveredLifecycleSignedBroadcastProjectionV1,
    ) -> bool {
        recovery.owns_recovered_control_broadcast(&self.projection, broadcast)
    }
}

impl AuthenticatedRecoveredWalDecisionFetchProjection {
    /// Seal the runtime-private recovered Decision Fetch projection.
    pub(in crate::sumeragi) fn from_runtime_projection(
        wal_identity: RecoveredWalFrameIdentity,
        replay_evidence: RecoveredWalDecisionFetchReplayEvidenceV1,
        effect: AdapterEffect,
        pending: PendingRuntimeEffectBinding,
        candidate: RecoveredWalDecisionFetchCandidateProjectionV1,
    ) -> Self {
        Self {
            wal_identity,
            replay_evidence,
            effect,
            pending,
            candidate: candidate.into_candidate(),
        }
    }

    /// Revalidate the complete nested authority against one verified height.
    pub(super) fn is_exact(&self, verified: &VerifiedHeightContext) -> bool {
        self.wal_identity.is_exact()
            && self
                .replay_evidence
                .exactly_matches_recovered_decision_fetch(verified, self.wal_identity, &self.effect)
            && self.pending.exactly_binds_adapter_effect(&self.effect)
            && self
                .replay_evidence
                .project_recovered_decision_fetch_candidate_for_comparison(
                    verified,
                    self.wal_identity,
                    &self.effect,
                    &self.pending,
                    &self.candidate,
                )
            && decision_fetch_candidate_shape_is_exact(&self.candidate)
    }

    /// Recheck that one closed Store/Validate/Apply lineage is the sole
    /// continuation of this exact payload-free Decision Fetch.
    pub(in crate::sumeragi) fn owns_apply_lineage(
        &self,
        verified: &VerifiedHeightContext,
        lineage: &RecoveredDecisionApplyCandidateLineageV1,
    ) -> bool {
        self.is_exact(verified)
            && lineage.is_exact(projection::lifecycle_context(verified.context()))
            && lineage.exactly_follows_fetch_candidate(&self.candidate)
    }

    /// Compare this projection with one lifecycle context.
    pub(in crate::sumeragi) fn belongs_to_context(&self, context: super::LifecycleContext) -> bool {
        self.candidate.key.context() == context.id()
            && self.candidate.key.round().height() == context.height()
    }

    /// Compare the cold adapter's reconstructed Fetch under the body-cut permit.
    pub(in crate::sumeragi) fn matches_fast_forward_fetch(
        &self,
        _permit: &RecoveredDecisionApplyAdapterPreviewPermit,
        verified: &VerifiedHeightContext,
        effect: &AdapterEffect,
    ) -> bool {
        self.is_exact(verified) && &self.effect == effect
    }

    /// Derive the only pending-binding chain accepted by the fixed body preview.
    ///
    /// The body-cut permit prevents this otherwise pure projection from being
    /// invoked with independently supplied effects. No binding or causal
    /// constituent leaves the returned opaque lineage.
    pub(in crate::sumeragi) fn project_decision_apply_pending_lineage(
        &self,
        permit: &RecoveredDecisionApplyAdapterPreviewPermit,
        verified: &VerifiedHeightContext,
        fetch: &AdapterEffect,
        store: &AdapterEffect,
        validate: &AdapterEffect,
        apply: &AdapterEffect,
    ) -> Option<RecoveredDecisionApplyPendingLineageV1> {
        if !self.matches_fast_forward_fetch(permit, verified, fetch) {
            return None;
        }
        let store_pending = self
            .pending
            .project_certified_fetch_store_successor(&self.effect, store)?;
        let validate_pending = store_pending.project_store_validate_successor(store, validate)?;
        let apply_pending = validate_pending.project_validate_apply_successor(validate, apply)?;
        let lineage = RecoveredDecisionApplyPendingLineageV1 {
            store: store_pending,
            validate: validate_pending,
            apply: apply_pending,
        };
        lineage
            .exactly_matches(store, validate, apply)
            .then_some(lineage)
    }

    /// Derive the exact first Store successor from one fsynced body and reducer preview.
    pub(in crate::sumeragi) fn project_decision_fetch_store(
        &self,
        verified: &VerifiedHeightContext,
        body: RecoveredDecisionFetchStoreBodyAuthorityV1,
        store_effect: &AdapterEffect,
    ) -> Option<RecoveredDecisionFetchStoreProjectionV1> {
        if !self.is_exact(verified) || !self.matches_durable_body(body.durable()) {
            return None;
        }
        let pending = self
            .pending
            .project_certified_fetch_store_successor(&self.effect, store_effect)?;
        if pending.causal_lifecycle_key() != self.pending.causal_lifecycle_key()
            || pending.exact_effect_identity() == self.pending.exact_effect_identity()
            || !pending.exactly_binds_adapter_effect(store_effect)
        {
            return None;
        }
        let replay = RecoveredDecisionApplyReplayLineageV1::from_sealed_recovered_decision(
            &self.replay_evidence,
            verified,
            self.wal_identity,
            &self.effect,
            body.manifest(),
            body.durable(),
        )?;
        let candidate = replay.project_recovered_fetch_store_candidate(
            verified,
            body.durable(),
            store_effect,
            &pending,
        )?;
        if candidate.causal_root != self.candidate.causal_root
            || candidate.reconstruction_source != self.candidate.reconstruction_source
            || !durable_continuation_successor_is_exact(
                DurableContinuationEdge::FetchToStore,
                self.candidate.work_class,
                self.candidate.key,
                self.candidate.stage,
                candidate.work_class,
                candidate.key,
                candidate.stage,
            )
        {
            return None;
        }
        Some(RecoveredDecisionFetchStoreProjectionV1 {
            effect: store_effect.clone(),
            pending,
            body,
            candidate,
        })
    }

    /// Bind one exact durable body to the current recovered Fetch adapter event.
    pub(in crate::sumeragi) fn project_store_adapter_authority(
        &self,
        body: RecoveredDecisionFetchStoreBodyAuthorityV1,
    ) -> Option<RecoveredDecisionFetchStoreAdapterAuthorityV1> {
        let AdapterEffect::FetchBody {
            tag,
            round,
            subject,
            manifest: None,
            certificate: Some(certificate),
            ..
        } = &self.effect
        else {
            return None;
        };
        (body.durable().round() == *round
            && body.durable().subject() == *subject
            && body.manifest().round == *round
            && body.manifest().subject == *subject
            && certificate.phase == wire::GlobalPhase::Commit
            && certificate.proposal_round == *round
            && certificate.subject == *subject)
            .then_some(RecoveredDecisionFetchStoreAdapterAuthorityV1 { tag: *tag, body })
    }

    /// Return whether one durable row names this exact Fetch key.
    pub(super) fn names_record(&self, record: &super::ledger::LifecycleLedgerRecordV1) -> bool {
        record.key() == Some(self.candidate.key)
    }

    /// Compare every persisted standalone admission field.
    pub(super) fn exactly_matches_record(
        &self,
        record: &super::ledger::LifecycleLedgerRecordV1,
    ) -> bool {
        self.names_record(record)
            && record.owner().causal_root() == self.candidate.causal_root
            && record.owner().first_admission_ordinal() == record.ordinal()
            && record.work_class() == Some(LifecycleWorkClass::Fetch)
            && record.stage() == Some(self.candidate.stage)
            && record.terminal() == Some(None)
            && record.reconstruction_source() == self.candidate.reconstruction_source
            && record.durable_payload() == Some(DurablePayloadReference::None)
            && record.continuation() == Some(super::schema::DurableContinuation::None)
            && record.replay_matches_candidate(&self.candidate)
    }

    /// Compare the exact terminal Fetch parent of the recovered body chain.
    pub(super) fn exactly_matches_advanced_apply_parent(
        &self,
        record: &super::ledger::LifecycleLedgerRecordV1,
        store_ordinal: u128,
    ) -> bool {
        self.names_record(record)
            && record.owner().causal_root() == self.candidate.causal_root
            && record.owner().first_admission_ordinal() == record.ordinal()
            && record.work_class() == Some(LifecycleWorkClass::Fetch)
            && record.stage() == Some(self.candidate.stage)
            && record.terminal() == Some(Some(super::TerminalOutcome::Advanced))
            && record.reconstruction_source() == self.candidate.reconstruction_source
            && record.durable_payload() == Some(DurablePayloadReference::None)
            && record.continuation()
                == Some(super::schema::DurableContinuation::successor(
                    DurableContinuationEdge::FetchToStore,
                    store_ordinal,
                ))
            && record.replay_matches_candidate(&self.candidate)
    }

    /// Prove the opened ledger contains this standalone row exactly once.
    pub(super) fn exactly_matches_ledger_at(
        &self,
        ledger: &super::ledger::LifecycleLedgerV1,
        ordinal: u128,
    ) -> bool {
        if !self.belongs_to_context(ledger.context()) {
            return false;
        }
        let mut records = ledger
            .records()
            .iter()
            .filter(|record| self.names_record(record));
        let Some(record) = records.next() else {
            return false;
        };
        let exact_owner = super::OwnerId::new(self.candidate.causal_root, ordinal);
        records.next().is_none()
            && ledger
                .records()
                .iter()
                .filter(|candidate| candidate.owner() == exact_owner)
                .count()
                == 1
            && record.ordinal() == ordinal
            && self.exactly_matches_record(record)
    }

    /// Construct the deterministic fresh Ready Fetch row.
    pub(super) fn fresh_record(
        &self,
        ordinal: u128,
    ) -> Result<super::ledger::LifecycleLedgerRecordV1, super::ledger::LifecycleLedgerError> {
        super::ledger::LifecycleLedgerRecordV1::new(
            self.candidate.key,
            super::OwnerId::new(self.candidate.causal_root, ordinal),
            ordinal,
            self.candidate.work_class,
            self.candidate.stage,
            None,
            self.candidate.reconstruction_source,
            self.candidate.payload,
            self.candidate.replay_authority.clone(),
            super::schema::DurableContinuation::None,
        )
    }

    /// Splice the exact candidate after its durable row matches.
    pub(super) fn splice_candidate_from_record(
        &self,
        record: &super::ledger::LifecycleLedgerRecordV1,
        candidates: &mut std::collections::BTreeMap<super::LifecycleKey, CandidateAdmission>,
    ) -> bool {
        self.exactly_matches_record(record)
            && !candidates.contains_key(&self.candidate.key)
            && candidates
                .insert(self.candidate.key, self.candidate.clone())
                .is_none()
    }

    /// Check that recovery retained this one exact Fetch candidate.
    pub(super) fn owns_spliced_candidate(
        &self,
        candidates: &std::collections::BTreeMap<super::LifecycleKey, CandidateAdmission>,
    ) -> bool {
        candidates.get(&self.candidate.key) == Some(&self.candidate)
    }

    /// Compare an exact semantically revalidated body marker without exposing coordinates.
    pub(in crate::sumeragi) fn matches_validated_body(
        &self,
        validated: &ValidatedBodyReceipt,
    ) -> bool {
        self.matches_durable_body_and_commitment(
            validated.durable(),
            validated.execution_commitment(),
        )
    }

    /// Derive the private recovered-Decision body replay family.
    ///
    /// Only the same-store body cut can mint the permit, so arbitrary manifest
    /// or receipt parts cannot cross this otherwise structural projection.
    pub(in crate::sumeragi) fn project_decision_apply_replay_lineage(
        &self,
        _permit: RecoveredDecisionApplyReplayPermit,
        verified: &VerifiedHeightContext,
        manifest: &wire::PayloadManifest,
        durable: &DurableBodyReceipt,
    ) -> Option<RecoveredDecisionApplyReplayLineageV1> {
        if !self.is_exact(verified) || !self.matches_durable_body(durable) {
            return None;
        }
        RecoveredDecisionApplyReplayLineageV1::from_sealed_recovered_decision(
            &self.replay_evidence,
            verified,
            self.wal_identity,
            &self.effect,
            manifest,
            durable,
        )
    }

    /// Compare a quarantined success marker without treating it as revalidated authority.
    ///
    /// This equality is only a fail-closed duplicate-prevention check. It does
    /// not promote or detach the marker and cannot authorize Apply.
    pub(in crate::sumeragi) fn matches_durable_body_and_commitment(
        &self,
        durable: &DurableBodyReceipt,
        execution_commitment: wire::ExecutionCommitment,
    ) -> bool {
        let Some(expected_commitment) = self.durable_body_execution_commitment(durable) else {
            return false;
        };
        execution_commitment == expected_commitment
    }

    /// Compare only the exact durable body coordinates.
    ///
    /// This rejection-only check lets startup fail closed on a deterministic
    /// rejection for the body named by a durable Commit Decision. It does not
    /// authorize Fetch or Apply.
    pub(in crate::sumeragi) fn matches_durable_body(&self, durable: &DurableBodyReceipt) -> bool {
        self.durable_body_execution_commitment(durable).is_some()
    }

    fn durable_body_execution_commitment(
        &self,
        durable: &DurableBodyReceipt,
    ) -> Option<wire::ExecutionCommitment> {
        let AdapterEffect::FetchBody {
            round,
            subject,
            certificate: Some(certificate),
            ..
        } = &self.effect
        else {
            return None;
        };
        (durable.context_id() == round.context_id
            && durable.round() == *round
            && durable.subject() == *subject)
            .then_some(certificate.execution_commitment)
    }

    /// Compare a concrete registry address and digest.
    pub(super) fn validates_installation(
        &self,
        owner: super::OwnerId,
        ordinal: u128,
        slot: super::PhysicalSlotId,
        digest: super::LifecycleDigest,
    ) -> bool {
        let Ok((physical, universe, consumed)) = self.candidate.physical_geometry.normalized()
        else {
            return false;
        };
        owner == super::OwnerId::new(self.candidate.causal_root, ordinal)
            && slot == super::PhysicalSlotId::for_capacity(super::CapacityClass::Effect, 0)
            && physical.len() == 1
            && universe.len() == 1
            && consumed == universe
            && physical.get(&slot) == Some(&digest)
            && digest == super::LifecycleDigest::new(*self.pending.exact_effect_identity().as_ref())
    }

    /// Compare the exact Ready coordinator record and its complete indexes.
    pub(super) fn matches_current_ready_record(
        &self,
        owner: super::OwnerId,
        ordinal: u128,
        slot: super::PhysicalSlotId,
        digest: super::LifecycleDigest,
        coordinator: &super::LifecycleCoordinator,
    ) -> bool {
        let Ok((physical, universe, consumed)) = self.candidate.physical_geometry.normalized()
        else {
            return false;
        };
        let Some(record) = coordinator.records.get(&ordinal) else {
            return false;
        };
        let Some(metadata) = coordinator.durable_records.get(&ordinal) else {
            return false;
        };
        self.validates_installation(owner, ordinal, slot, digest)
            && coordinator.fault.is_none()
            && coordinator.active_context.id() == self.candidate.key.context()
            && coordinator.active_context.height() == self.candidate.key.round().height()
            && coordinator.high_water >= ordinal
            && record.key == self.candidate.key
            && record.owner == owner
            && record.ordinal == ordinal
            && record.work_class == LifecycleWorkClass::Fetch
            && record.stage == self.candidate.stage
            && record.state == super::LifecycleState::Ready
            && record.physical_slots == physical
            && record.episode.slot_universe == universe
            && record.episode.consumed_slots == consumed
            && physical.get(&slot) == Some(&digest)
            && metadata.matches_admission(&self.candidate)
            && coordinator.key_index.get(&self.candidate.key) == Some(&ordinal)
            && coordinator.owner_index.get(&self.candidate.causal_root) == Some(&owner)
            && coordinator.ready_index.contains(&ordinal)
    }

    fn matches_claimed_record(
        &self,
        owner: super::OwnerId,
        ordinal: u128,
        slot: super::PhysicalSlotId,
        digest: super::LifecycleDigest,
        coordinator: &super::LifecycleCoordinator,
        lease: &super::TurnLease,
    ) -> bool {
        let Ok((physical, universe, consumed)) = self.candidate.physical_geometry.normalized()
        else {
            return false;
        };
        let (Some(record), Some(metadata)) = (
            coordinator.records.get(&ordinal),
            coordinator.durable_records.get(&ordinal),
        ) else {
            return false;
        };
        self.validates_installation(owner, ordinal, slot, digest)
            && coordinator.fault.is_none()
            && coordinator.active_context.id() == self.candidate.key.context()
            && coordinator.active_context.height() == self.candidate.key.round().height()
            && coordinator.active_lease.as_ref() == Some(lease)
            && record.key == self.candidate.key
            && record.owner == owner
            && record.ordinal == ordinal
            && record.work_class == LifecycleWorkClass::Fetch
            && record.stage == self.candidate.stage
            && record.state == super::LifecycleState::Claimed(lease.id())
            && lease.key() == record.key
            && lease.owner() == record.owner
            && lease.ordinal() == record.ordinal
            && lease.work_class() == record.work_class
            && lease.stage() == record.stage
            && lease.physical_slots() == &record.physical_slots
            && record.physical_slots == physical
            && record.episode.slot_universe == universe
            && record.episode.consumed_slots == consumed
            && physical.get(&slot) == Some(&digest)
            && metadata.matches_admission(&self.candidate)
            && coordinator.key_index.get(&self.candidate.key) == Some(&ordinal)
            && coordinator.owner_index.get(&self.candidate.causal_root) == Some(&owner)
            && !coordinator.ready_index.contains(&ordinal)
    }

    /// Consume the projection into its dedicated installed carrier.
    pub(super) fn into_durable_carrier(
        self,
        owner: super::OwnerId,
        ordinal: u128,
        slot: super::PhysicalSlotId,
    ) -> Result<DurableRecoveredWalDecisionFetchCarrierV1, Self> {
        let Ok((physical, universe, consumed)) = self.candidate.physical_geometry.normalized()
        else {
            return Err(self);
        };
        let Some(&digest) = physical.get(&slot) else {
            return Err(self);
        };
        if physical.len() != 1
            || universe.len() != 1
            || consumed != universe
            || !self.validates_installation(owner, ordinal, slot, digest)
        {
            return Err(self);
        }
        Ok(DurableRecoveredWalDecisionFetchCarrierV1 {
            projection: self,
            owner,
            ordinal,
            slot,
            digest,
        })
    }
}

impl RecoveredDecisionFetchStoreProjectionV1 {
    /// Revalidate the closed Store candidate against its exact body and height.
    pub(super) fn is_exact(&self, verified: &VerifiedHeightContext) -> bool {
        let context = projection::lifecycle_context(verified.context());
        let expected_slot = super::PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
        self.pending.exactly_binds_adapter_effect(&self.effect)
            && self.body.durable().context_id() == verified.context().id()
            && self.body.durable().round() == self.body.manifest().round
            && self.body.durable().subject() == self.body.manifest().subject
            && self.body.durable().manifest_hash()
                == iroha_crypto::HashOf::new(self.body.manifest())
            && self.candidate.work_class == LifecycleWorkClass::Store
            && self.candidate.stage.kind() == LifecycleStageKind::StoreBody
            && self.candidate.initial_state == InitialLifecycleState::Ready
            && self.candidate.replay_authority_is_exact(context)
            && self.candidate.physical_geometry.normalized().is_ok_and(
                |(physical, universe, consumed)| {
                    physical.len() == 1
                        && physical.get(&expected_slot) == Some(&self.digest())
                        && universe == std::collections::BTreeSet::from([expected_slot])
                        && consumed == universe
                },
            )
    }

    /// Project the logical child only inside recovered-specific coordinator staging.
    pub(super) fn candidate_for_transition(
        &self,
        verified: &VerifiedHeightContext,
    ) -> Option<CandidateAdmission> {
        self.is_exact(verified).then(|| self.candidate.clone())
    }

    /// Return the exact derived child digest without exposing the effect binding.
    pub(super) fn digest(&self) -> super::LifecycleDigest {
        let mut bytes = [0_u8; 32];
        bytes.copy_from_slice(self.pending.exact_effect_identity().as_ref());
        super::LifecycleDigest::new(bytes)
    }

    /// Return the immutable lifecycle context retained by the child candidate.
    pub(super) fn context(&self) -> super::LifecycleContext {
        super::LifecycleContext::new(
            self.candidate.key.context(),
            self.candidate.key.round().height(),
        )
    }

    /// Recheck one installed child address without releasing successor parts.
    pub(super) fn validates_at(
        &self,
        context: super::LifecycleContext,
        address: super::work_registry::ConcreteWorkAddress,
        digest: super::LifecycleDigest,
    ) -> bool {
        let expected_slot = super::PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
        address.owner.causal_root() == self.candidate.causal_root
            && address.ordinal > address.owner.first_admission_ordinal()
            && address.slot == expected_slot
            && digest == self.digest()
            && self.pending.exactly_binds_adapter_effect(&self.effect)
            && self.candidate.key.context() == context.id()
            && self.candidate.key.round().height() == context.height()
            && self.body.durable().context_id().0.as_ref() == context.id().as_bytes()
            && self.body.durable().round().height == context.height()
            && self.candidate.replay_authority_is_exact(context)
            && self.candidate.physical_geometry.normalized().is_ok_and(
                |(physical, universe, consumed)| {
                    physical == std::collections::BTreeMap::from([(expected_slot, digest)])
                        && universe == std::collections::BTreeSet::from([expected_slot])
                        && consumed == universe
                },
            )
    }

    /// Compare one Store ledger row against the complete closed projection.
    pub(super) fn exactly_matches_record(
        &self,
        record: &super::ledger::LifecycleLedgerRecordV1,
        owner: super::OwnerId,
    ) -> bool {
        record.key() == Some(self.candidate.key)
            && record.owner() == owner
            && record.work_class() == Some(LifecycleWorkClass::Store)
            && record.stage() == Some(self.candidate.stage)
            && record.terminal() == Some(None)
            && record.reconstruction_source() == self.candidate.reconstruction_source
            && record.durable_payload() == Some(self.candidate.payload)
            && record.continuation() == Some(super::schema::DurableContinuation::None)
            && record.replay_matches_candidate(&self.candidate)
    }

    /// Construct the deterministic live Store row for LedgerV1 publication.
    pub(super) fn fresh_record(
        &self,
        owner: super::OwnerId,
        ordinal: u128,
    ) -> Result<super::ledger::LifecycleLedgerRecordV1, super::ledger::LifecycleLedgerError> {
        super::ledger::LifecycleLedgerRecordV1::new(
            self.candidate.key,
            owner,
            ordinal,
            self.candidate.work_class,
            self.candidate.stage,
            None,
            self.candidate.reconstruction_source,
            self.candidate.payload,
            self.candidate.replay_authority.clone(),
            super::schema::DurableContinuation::None,
        )
    }

    /// Insert the exact live Store candidate during typed cold recovery.
    pub(super) fn splice_candidate_from_record(
        &self,
        record: &super::ledger::LifecycleLedgerRecordV1,
        owner: super::OwnerId,
        candidates: &mut std::collections::BTreeMap<super::LifecycleKey, CandidateAdmission>,
    ) -> bool {
        self.exactly_matches_record(record, owner)
            && !candidates.contains_key(&self.candidate.key)
            && candidates
                .insert(self.candidate.key, self.candidate.clone())
                .is_none()
    }

    /// Check that cold recovery retained this exact Store candidate.
    pub(super) fn owns_spliced_candidate(
        &self,
        candidates: &std::collections::BTreeMap<super::LifecycleKey, CandidateAdmission>,
    ) -> bool {
        candidates.get(&self.candidate.key) == Some(&self.candidate)
    }

    /// Compare the exact Ready Store coordinator record and its complete indexes.
    pub(super) fn matches_current_ready_record(
        &self,
        context: super::LifecycleContext,
        address: super::work_registry::ConcreteWorkAddress,
        digest: super::LifecycleDigest,
        coordinator: &super::LifecycleCoordinator,
    ) -> bool {
        let Ok((physical, universe, consumed)) = self.candidate.physical_geometry.normalized()
        else {
            return false;
        };
        let (Some(record), Some(metadata)) = (
            coordinator.records.get(&address.ordinal),
            coordinator.durable_records.get(&address.ordinal),
        ) else {
            return false;
        };
        self.validates_at(context, address, digest)
            && coordinator.fault.is_none()
            && coordinator.active_context == context
            && coordinator.high_water >= address.ordinal
            && record.key == self.candidate.key
            && record.owner == address.owner
            && record.ordinal == address.ordinal
            && record.work_class == LifecycleWorkClass::Store
            && record.stage == self.candidate.stage
            && record.state == super::LifecycleState::Ready
            && record.physical_slots == physical
            && record.episode.slot_universe == universe
            && record.episode.consumed_slots == consumed
            && physical.get(&address.slot) == Some(&digest)
            && metadata.matches_admission(&self.candidate)
            && coordinator.key_index.get(&self.candidate.key) == Some(&address.ordinal)
            && coordinator.owner_index.get(&self.candidate.causal_root) == Some(&address.owner)
            && coordinator.ready_index.contains(&address.ordinal)
    }
}

impl DurableRecoveredWalDecisionFetchCarrierV1 {
    /// Return the digest only while paired with the complete carrier.
    pub(super) const fn installed_digest(&self) -> super::LifecycleDigest {
        self.digest
    }

    /// Revalidate the carrier's sealed original Fetch coordinates.
    pub(super) fn is_exact(&self) -> bool {
        self.validates_at(self.owner, self.ordinal, self.slot, self.digest)
    }

    /// Rejoin the installed carrier to its verified WAL replay projection.
    pub(super) fn validates(&self, verified: &VerifiedHeightContext) -> bool {
        self.projection.is_exact(verified) && self.is_exact()
    }

    /// Rejoin this standalone Fetch carrier to one exact in-memory ledger frame.
    pub(super) fn validates_in_ledger(
        &self,
        verified: &VerifiedHeightContext,
        ledger: &super::ledger::LifecycleLedgerV1,
    ) -> bool {
        self.validates(verified)
            && self
                .projection
                .exactly_matches_ledger_at(ledger, self.ordinal)
    }

    /// Return the immutable causal owner while retaining the complete carrier.
    pub(super) const fn causal_root(&self) -> super::CausalRoot {
        self.owner.causal_root()
    }

    /// Compare the complete installed address and physical identity.
    pub(super) fn validates_at(
        &self,
        owner: super::OwnerId,
        ordinal: u128,
        slot: super::PhysicalSlotId,
        digest: super::LifecycleDigest,
    ) -> bool {
        self.owner == owner
            && self.ordinal == ordinal
            && self.slot == slot
            && self.digest == digest
            && self
                .projection
                .validates_installation(owner, ordinal, slot, digest)
    }

    /// Reopen and match the exact durable standalone Fetch row.
    pub(super) fn validates_in_store(&self, store: &super::ledger::LifecycleLedgerStoreV1) -> bool {
        store.revalidates_authenticated_wal_decision_fetch(&self.projection, self.ordinal)
            && self.validates_at(self.owner, self.ordinal, self.slot, self.digest)
    }

    /// Reopen and match the exact advanced Fetch plus live Store crash cut.
    pub(super) fn validates_recovered_store_in_store(
        &self,
        store_projection: &RecoveredDecisionFetchStoreProjectionV1,
        store: &super::ledger::LifecycleLedgerStoreV1,
    ) -> bool {
        store.revalidates_recovered_decision_fetch_store(
            &self.projection,
            self.ordinal,
            store_projection,
        ) && self.validates_at(self.owner, self.ordinal, self.slot, self.digest)
    }

    /// Rejoin the retained Fetch and Store against one exact in-memory ledger prefix.
    pub(super) fn validates_recovered_store_in_ledger(
        &self,
        store_projection: &RecoveredDecisionFetchStoreProjectionV1,
        ledger: &super::ledger::LifecycleLedgerV1,
    ) -> bool {
        ledger
            .authenticate_recovered_decision_fetch_store(&self.projection, store_projection)
            .is_ok_and(|(fetch_ordinal, _)| fetch_ordinal == self.ordinal)
            && self.validates_at(self.owner, self.ordinal, self.slot, self.digest)
    }

    /// Compare the current Ready coordinator record and carrier.
    pub(super) fn matches_current_ready_record(
        &self,
        coordinator: &super::LifecycleCoordinator,
    ) -> bool {
        self.projection.matches_current_ready_record(
            self.owner,
            self.ordinal,
            self.slot,
            self.digest,
            coordinator,
        )
    }

    /// Compare the sole active claimed lease with this exact recovered Fetch.
    pub(super) fn matches_claimed_record(
        &self,
        coordinator: &super::LifecycleCoordinator,
        lease: &super::TurnLease,
    ) -> bool {
        self.projection.matches_claimed_record(
            self.owner,
            self.ordinal,
            self.slot,
            self.digest,
            coordinator,
            lease,
        )
    }

    /// Project the complete payload-free Fetch into a sealed request authority.
    pub(super) fn project_recovered_decision_fetch_request(
        &self,
        identity: super::work_registry::RecoveredDecisionFetchDispatchIdentityV1,
    ) -> Option<crate::sumeragi::v2_worker::RecoveredDecisionFetchRequestAuthorityV1> {
        let AdapterEffect::FetchBody {
            tag,
            round,
            subject,
            manifest: None,
            certified_sources,
            certificate: Some(certificate),
        } = &self.projection.effect
        else {
            return None;
        };
        crate::sumeragi::v2_worker::RecoveredDecisionFetchRequestAuthorityV1::from_registry_projection(
            identity,
            *tag,
            *round,
            *subject,
            certified_sources.clone(),
            certificate.clone(),
        )
    }

    /// Project only the exact body-preview authority for the claimed Fetch carrier.
    pub(super) fn project_store_adapter_authority(
        &self,
        body: RecoveredDecisionFetchStoreBodyAuthorityV1,
    ) -> Option<RecoveredDecisionFetchStoreAdapterAuthorityV1> {
        self.projection.project_store_adapter_authority(body)
    }

    /// Derive the closed Store successor from the reducer preview.
    pub(super) fn project_store_successor(
        &self,
        verified: &VerifiedHeightContext,
        body: RecoveredDecisionFetchStoreBodyAuthorityV1,
        store_effect: &AdapterEffect,
    ) -> Option<RecoveredDecisionFetchStoreProjectionV1> {
        self.projection
            .project_decision_fetch_store(verified, body, store_effect)
    }

    /// Prove the authenticated recovery cut retains this exact Fetch.
    pub(super) fn owns_recovery(
        &self,
        recovery: &super::open::AuthenticatedLifecycleRecoveryCut,
    ) -> bool {
        recovery.owns_recovered_wal_decision_fetch(&self.projection)
    }

    /// Prove cold recovery retains the Store child of this exact WAL Fetch.
    pub(super) fn owns_store_recovery(
        &self,
        store: &RecoveredDecisionFetchStoreProjectionV1,
        recovery: &super::open::AuthenticatedLifecycleRecoveryCut,
    ) -> bool {
        recovery.owns_recovered_decision_store(&self.projection, store)
    }
}

fn decision_fetch_candidate_shape_is_exact(candidate: &CandidateAdmission) -> bool {
    let Ok((physical, universe, consumed)) = candidate.physical_geometry.normalized() else {
        return false;
    };
    candidate.work_class == LifecycleWorkClass::Fetch
        && candidate.key.phase() == super::LifecyclePhase::Fetch
        && candidate.stage.kind() == LifecycleStageKind::FetchBody
        && candidate.initial_state == InitialLifecycleState::Ready
        && candidate.stage.predecessor_scope() == PredecessorScope::Independent
        && candidate.payload == DurablePayloadReference::None
        && candidate.causal_root.digest() == candidate.reconstruction_source
        && candidate.producer_turn.is_none()
        && candidate
            .physical_geometry
            .canonicalized()
            .is_ok_and(|canonical| canonical == candidate.physical_geometry)
        && physical.len() == 1
        && universe.len() == 1
        && consumed == universe
        && physical
            .keys()
            .all(|slot| slot.capacity_class() == Some(CapacityClass::Effect))
}

fn control_candidate_shape_is_exact(candidate: &CandidateAdmission) -> bool {
    let expected = match (
        candidate.work_class,
        candidate.key.phase(),
        candidate.stage.kind(),
    ) {
        (
            LifecycleWorkClass::SignProposal,
            super::LifecyclePhase::Proposal,
            LifecycleStageKind::SignProposal,
        )
        | (
            LifecycleWorkClass::SignTimeout,
            super::LifecyclePhase::Timeout,
            LifecycleStageKind::SignTimeoutVote,
        ) => true,
        _ => false,
    };
    let Ok((physical, universe, consumed)) = candidate.physical_geometry.normalized() else {
        return false;
    };
    expected
        && candidate.initial_state == InitialLifecycleState::Ready
        && candidate.stage.predecessor_scope() == PredecessorScope::Independent
        && candidate.payload == DurablePayloadReference::None
        && candidate.causal_root.digest() == candidate.reconstruction_source
        && candidate.producer_turn.is_none()
        && candidate
            .physical_geometry
            .canonicalized()
            .is_ok_and(|canonical| canonical == candidate.physical_geometry)
        && physical.len() == 1
        && universe.len() == 1
        && consumed == universe
        && physical
            .keys()
            .all(|slot| slot.capacity_class() == Some(CapacityClass::Effect))
}

impl AuthenticatedRecoveredWalVoteProjection {
    /// Assemble the one successful result of the consuming runtime projection.
    pub(in crate::sumeragi) fn from_runtime_projection(
        _permit: RecoveredWalCandidateProjectionPermit,
        successor: RecoveredWalVoteSuccessor,
        parent: CandidateAdmission,
        child: CandidateAdmission,
    ) -> Self {
        Self {
            successor,
            parent,
            child,
        }
    }

    const fn parent(&self) -> &CandidateAdmission {
        &self.parent
    }

    const fn child(&self) -> &CandidateAdmission {
        &self.child
    }

    fn concrete_pair_is_exact(&self) -> bool {
        self.successor.replay_evidence_is_exact() && self.successor.concrete_pair_is_exact()
    }

    fn concrete_pair_matches_validation(&self, validated: &ValidatedBodyReceipt) -> bool {
        self.successor.concrete_pair_matches_validation(validated)
    }

    const fn installed_child_effect(&self) -> &AdapterEffect {
        self.successor.installed_child_effect()
    }
}

#[cfg_attr(not(test), allow(dead_code))]
impl RecoveredWalVoteLifecycleRepairError {
    /// Return a stable diagnostic classification without exposing authority.
    pub(super) const fn reason(&self) -> &'static str {
        match self.kind {
            RecoveredWalVoteLifecycleRepairErrorKind::ParentProjection => {
                "recovered Validate projection failed"
            }
            RecoveredWalVoteLifecycleRepairErrorKind::ChildProjection => {
                "recovered Sign projection failed"
            }
            RecoveredWalVoteLifecycleRepairErrorKind::InvalidWalIdentity => {
                "recovered WAL identity is inconsistent"
            }
            RecoveredWalVoteLifecycleRepairErrorKind::InvalidReplayEvidence => {
                "recovered WAL replay evidence is inconsistent"
            }
            RecoveredWalVoteLifecycleRepairErrorKind::InvalidParent => {
                "recovered Validate parent is invalid"
            }
            RecoveredWalVoteLifecycleRepairErrorKind::InvalidChild => {
                "recovered Sign child is invalid"
            }
            RecoveredWalVoteLifecycleRepairErrorKind::ForeignOwner => {
                "recovered WAL continuation changed causal owner"
            }
            RecoveredWalVoteLifecycleRepairErrorKind::ForeignLineage => {
                "recovered WAL continuation changed body lineage"
            }
        }
    }
}

/// Authenticated, move-only WAL-ahead parent/child lifecycle repair.
///
/// Both logical candidates are projected from sealed runtime bindings. The
/// child binding was itself minted only by consuming the latest exact matching
/// adapter-authenticated WAL vote seal, including the full PrepareQC for a
/// recovered Commit. Terminal WAL continuity is authenticated independently.
/// This value is inert: it exposes no ledger persistence, coordinator
/// mutation, registry installation, or adapter commit surface.
#[must_use = "an authenticated WAL lifecycle repair has not been staged or published"]
pub(super) struct AuthenticatedWalVoteLifecycleRepair {
    projection: AuthenticatedRecoveredWalVoteProjection,
    edge: DurableContinuationEdge,
}

/// Post-fsync WAL recovery authority bound to one exact LedgerV1 replacement.
///
/// The token still retains the concrete Validate parent and Sign successor.
/// It exposes no effect/binding extraction or registry mutation; the future
/// startup transaction must consume it directly into the exact child address.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "a durable WAL repair still owns its concrete lifecycle handoff"]
pub(super) struct DurableAuthenticatedWalVoteLifecycleRepair {
    repair: AuthenticatedWalVoteLifecycleRepair,
    receipt: DurableWalVoteLedgerRepairReceipt,
}

#[cfg_attr(not(test), allow(dead_code))]
impl AuthenticatedWalVoteLifecycleRepair {
    /// Borrow the exact recovered Validate admission projection.
    pub(super) const fn parent(&self) -> &CandidateAdmission {
        self.projection.parent()
    }

    /// Borrow the exact recovered Sign admission projection.
    pub(super) const fn child(&self) -> &CandidateAdmission {
        self.projection.child()
    }

    /// Return the typed durable Validate-to-Sign continuation edge.
    pub(super) const fn edge(&self) -> DurableContinuationEdge {
        self.edge
    }

    /// Revalidate both retained concrete effects against their sealed bindings.
    pub(super) fn concrete_pair_is_exact(&self) -> bool {
        self.projection.concrete_pair_is_exact()
    }

    /// Return whether one durable validation is the exact outcome carried by
    /// this concrete Validate-to-Sign recovery pair.
    ///
    /// This equality oracle deliberately exposes neither concrete effect nor
    /// pending binding. The registry recovery token uses it to keep the body
    /// receipt tied to the authenticated WAL vote after detaching the parent
    /// row.
    pub(super) fn concrete_pair_matches_validation(
        &self,
        validated: &ValidatedBodyReceipt,
    ) -> bool {
        let active_context = super::LifecycleContext::new(
            self.parent().key.context(),
            self.parent().key.round().height(),
        );
        let expected_payload =
            projection::durable_body_frame_reference(active_context, validated.durable())
                .map(DurablePayloadReference::BodyFrame);
        self.concrete_pair_is_exact()
            && Some(self.parent().payload) == expected_payload
            && self.projection.concrete_pair_matches_validation(validated)
    }

    /// Reconstruct a roster-authenticated durable Broadcast child.
    ///
    /// The caller must still bind this repair to the exact opened LedgerV1
    /// frame before installing the returned projection. Keeping this pure
    /// projection here lets cold recovery perform every fallible semantic and
    /// cryptographic check before consuming the repair into its frame receipt.
    pub(super) fn recover_durable_signed_broadcast(
        &self,
        verified: &VerifiedHeightContext,
        child: super::replay_authority::DurableRecoveredSignedBroadcastChildV1,
    ) -> Option<RecoveredLifecycleSignedBroadcastProjectionV1> {
        let broadcast = child
            .consume_for_recovered_wal(RecoveredLifecycleSignBroadcastProjectionPermitV1::new());
        let AdapterEffect::Broadcast(message) = &broadcast else {
            return None;
        };
        verified.verify_consensus_message(message).ok()?;
        let pending = self
            .projection
            .successor
            .project_signed_broadcast_successor(&broadcast)?;
        let candidate = super::replay_authority::exact_signed_broadcast_successor_candidate(
            verified, &broadcast, &pending,
        )?;
        let projection = RecoveredLifecycleSignedBroadcastProjectionV1 {
            effect: broadcast,
            pending,
            candidate,
            cold_proposal_output: None,
        };
        self.matches_signed_broadcast(verified, &projection)
            .then_some(projection)
    }

    /// Recheck one signed child against this recovered Validate→Sign repair.
    pub(super) fn matches_signed_broadcast(
        &self,
        verified: &VerifiedHeightContext,
        broadcast: &RecoveredLifecycleSignedBroadcastProjectionV1,
    ) -> bool {
        self.projection
            .successor
            .signed_broadcast_successor_is_exact(&broadcast.effect, &broadcast.pending)
            && super::replay_authority::exact_signed_broadcast_successor_candidate(
                verified,
                &broadcast.effect,
                &broadcast.pending,
            )
            .as_ref()
                == Some(&broadcast.candidate)
    }

    /// Seal this exact vote signature for cold reducer replay.
    pub(super) fn project_cold_adapter_authority(
        &self,
        verified: &VerifiedHeightContext,
        broadcast: &RecoveredLifecycleSignedBroadcastProjectionV1,
    ) -> Option<crate::sumeragi::v2::RecoveredLifecycleSignColdAdapterAuthorityV1> {
        if !self.matches_signed_broadcast(verified, broadcast) {
            return None;
        }
        let AdapterEffect::Sign { tag, request } = self.projection.installed_child_effect() else {
            return None;
        };
        crate::sumeragi::v2::RecoveredLifecycleSignColdAdapterAuthorityV1::from_recovered_wal(
            RecoveredLifecycleSignBroadcastProjectionPermitV1::new(),
            *tag,
            request.clone(),
            broadcast.effect.clone(),
        )
    }

    /// Bind this move-only repair to the exact post-fsync ledger receipt.
    #[allow(clippy::result_large_err)]
    pub(super) fn bind_durable_ledger_receipt(
        self,
        receipt: DurableWalVoteLedgerRepairReceipt,
    ) -> Result<DurableAuthenticatedWalVoteLifecycleRepair, (Self, DurableWalVoteLedgerRepairReceipt)>
    {
        if !receipt.matches(&self) {
            return Err((self, receipt));
        }
        Ok(DurableAuthenticatedWalVoteLifecycleRepair {
            repair: self,
            receipt,
        })
    }
}

#[cfg_attr(not(test), allow(dead_code))]
impl DurableAuthenticatedWalVoteLifecycleRepair {
    /// Return the durable Sign child ordinal.
    pub(super) const fn child_ordinal(&self) -> u128 {
        self.receipt.child_ordinal()
    }

    /// Return the hash of the complete fsynced LedgerV1 frame.
    pub(super) const fn ledger_frame_hash(&self) -> super::LifecycleDigest {
        self.receipt.ledger_frame_hash()
    }

    /// Borrow the authenticated repair for idempotent post-fsync verification.
    pub(super) const fn repair(&self) -> &AuthenticatedWalVoteLifecycleRepair {
        &self.repair
    }

    /// Rejoin this repaired pair to an unchanged in-memory ledger lineage.
    pub(super) fn validates_in_ledger(&self, ledger: &super::ledger::LifecycleLedgerV1) -> bool {
        ledger
            .stage_authenticated_wal_vote_repair(&self.repair)
            .is_ok_and(|(staged, child_ordinal, changed)| {
                !changed && staged == *ledger && child_ordinal == self.child_ordinal()
            })
    }

    /// Match one validation to the retained concrete Validate-to-Sign pair.
    pub(super) fn concrete_pair_matches_validation(
        &self,
        validated: &ValidatedBodyReceipt,
    ) -> bool {
        self.repair.concrete_pair_matches_validation(validated)
    }

    /// Borrow only the recovered Sign effect retained by this durable repair.
    ///
    /// This narrow view exists solely so the closed concrete-registry carrier
    /// can satisfy the registry's non-consuming effect-borrow contract. It
    /// exposes neither pending binding nor a consuming effect/authority pair.
    pub(super) const fn installed_child_effect(&self) -> &AdapterEffect {
        self.repair.projection.installed_child_effect()
    }

    /// Project the mandatory signed Broadcast from the exact recovered vote owner.
    pub(super) fn project_authenticated_signed_broadcast(
        &self,
        verified: &VerifiedHeightContext,
        authority: crate::sumeragi::v2::RecoveredLifecycleSignBroadcastProjectionAuthorityV1,
    ) -> Option<(
        super::work_registry::RecoveredLifecycleSignDispatchKeyV1,
        RecoveredLifecycleSignedBroadcastProjectionV1,
    )> {
        let (key, broadcast) = authority
            .consume_for_recovered_wal(RecoveredLifecycleSignBroadcastProjectionPermitV1::new());
        let pending = self
            .repair
            .projection
            .successor
            .project_signed_broadcast_successor(&broadcast)?;
        let candidate = super::replay_authority::exact_signed_broadcast_successor_candidate(
            verified, &broadcast, &pending,
        )?;
        let projection = RecoveredLifecycleSignedBroadcastProjectionV1 {
            effect: broadcast,
            pending,
            candidate,
            cold_proposal_output: None,
        };
        self.matches_signed_broadcast(verified, &projection)
            .then_some((key, projection))
    }

    /// Project the exact signed Broadcast while retaining its WAL/body-bound Sign.
    ///
    /// Live publication consumes this pair atomically. Cold owner assembly must
    /// still join it to the frame-bound Ledger pair and complete startup census.
    pub(super) fn project_authenticated_signed_broadcast_and_sign(
        &self,
        verified: &VerifiedHeightContext,
        authority: crate::sumeragi::v2::RecoveredLifecycleSignBroadcastAndSignAuthorityV1,
    ) -> Option<(
        super::work_registry::RecoveredLifecycleSignDispatchKeyV1,
        RecoveredLifecycleSignedBroadcastAndSignProjectionV1,
    )> {
        let (key, broadcast, next_sign) = authority
            .consume_for_recovered_wal(RecoveredLifecycleSignBroadcastProjectionPermitV1::new());
        if !next_sign.matches_verified_height(verified)
            || !next_sign.matches_phase_vote_repair(self)
        {
            return None;
        }
        let next_sign =
            project_recovered_lifecycle_next_wal_vote_candidate(verified, next_sign).ok()?;
        let pending = self
            .repair
            .projection
            .successor
            .project_signed_broadcast_successor(&broadcast)?;
        let candidate = super::replay_authority::exact_signed_broadcast_successor_candidate(
            verified, &broadcast, &pending,
        )?;
        let broadcast = RecoveredLifecycleSignedBroadcastProjectionV1 {
            effect: broadcast,
            pending,
            candidate,
            cold_proposal_output: None,
        };
        let combined = RecoveredLifecycleSignedBroadcastAndSignProjectionV1 {
            broadcast,
            next_sign,
            cold_adapter_authority_minted: false,
        };
        (self.matches_signed_broadcast(verified, &combined.broadcast)
            && combined.children_are_exact(verified))
        .then_some((key, combined))
    }

    /// Reconstruct a durable signed child only through this exact phase-vote WAL owner.
    pub(super) fn recover_durable_signed_broadcast(
        &self,
        verified: &VerifiedHeightContext,
        child: super::replay_authority::DurableRecoveredSignedBroadcastChildV1,
    ) -> Option<RecoveredLifecycleSignedBroadcastProjectionV1> {
        self.repair
            .recover_durable_signed_broadcast(verified, child)
    }

    /// Bind the durable vote child back to this exact WAL Sign for cold replay.
    pub(super) fn project_cold_adapter_authority(
        &self,
        verified: &VerifiedHeightContext,
        broadcast: &RecoveredLifecycleSignedBroadcastProjectionV1,
    ) -> Option<crate::sumeragi::v2::RecoveredLifecycleSignColdAdapterAuthorityV1> {
        self.repair
            .project_cold_adapter_authority(verified, broadcast)
    }

    /// Preview an exact durable Prepare Broadcast and its follow-on Commit Sign.
    ///
    /// The historical Prepare Sign remains owned by this repair. The adapter
    /// startup is replayed only on clones and the next Vote cannot become
    /// executable until the exact revalidated body marker and latest WAL owner
    /// rejoin through the returned preview.
    pub(super) fn prepare_cold_signed_broadcast_and_sign(
        &self,
        verified: &VerifiedHeightContext,
        startup: crate::sumeragi::v2::ProductionLifecycleAdapterStartupV1,
        broadcast: &RecoveredLifecycleSignedBroadcastProjectionV1,
    ) -> Result<
        crate::sumeragi::v2::PreparedRecoveredLifecycleSignedBroadcastAndSignColdPreviewV1,
        &'static str,
    > {
        if !self.matches_signed_broadcast(verified, broadcast) {
            return Err("recovered phase Broadcast changed before cold pair preview");
        }
        let AdapterEffect::Sign { tag, request } = self.installed_child_effect() else {
            return Err("recovered phase parent is not a Sign effect");
        };
        let authority = crate::sumeragi::v2::RecoveredLifecycleSignedBroadcastColdPreviewAuthorityV1::from_recovered_wal(
            RecoveredLifecycleSignBroadcastProjectionPermitV1::new(),
            *tag,
            request.clone(),
            broadcast.effect.clone(),
        )
        .ok_or("recovered phase Broadcast is not an exact Prepare-vote child")?;
        startup.prepare_recovered_lifecycle_signed_broadcast_and_sign(verified, authority)
    }

    /// Rejoin the cold adapter/body seal to this exact phase-vote repair.
    ///
    /// The returned startup is still at the historical Prepare-Sign fence.
    /// The caller must advance it with the comparison-only authority retained
    /// by the combined projection before opening live ownership.
    pub(super) fn project_authenticated_cold_signed_broadcast_and_sign(
        &self,
        verified: &VerifiedHeightContext,
        seal: crate::sumeragi::v2::RecoveredLifecycleSignedBroadcastAndSignColdSealV1,
    ) -> Option<(
        crate::sumeragi::v2::ProductionLifecycleAdapterStartupV1,
        RecoveredLifecycleSignedBroadcastAndSignProjectionV1,
    )> {
        let (startup, broadcast, next_sign, cold_proposal_output) = seal
            .consume_for_recovered_wal(RecoveredLifecycleSignBroadcastProjectionPermitV1::new());
        if cold_proposal_output.is_some()
            || !next_sign.matches_verified_height(verified)
            || !next_sign.matches_phase_vote_repair(self)
        {
            return None;
        }
        let next_sign =
            project_recovered_lifecycle_next_wal_vote_candidate(verified, next_sign).ok()?;
        let pending = self
            .repair
            .projection
            .successor
            .project_signed_broadcast_successor(&broadcast)?;
        let candidate = super::replay_authority::exact_signed_broadcast_successor_candidate(
            verified, &broadcast, &pending,
        )?;
        let broadcast = RecoveredLifecycleSignedBroadcastProjectionV1 {
            effect: broadcast,
            pending,
            candidate,
            cold_proposal_output: None,
        };
        let combined = RecoveredLifecycleSignedBroadcastAndSignProjectionV1 {
            broadcast,
            next_sign,
            cold_adapter_authority_minted: false,
        };
        (self.matches_signed_broadcast(verified, &combined.broadcast)
            && combined.children_are_exact(verified))
        .then_some((startup, combined))
    }

    /// Recheck a retained Broadcast successor against the sealed recovered vote.
    pub(super) fn matches_signed_broadcast(
        &self,
        verified: &VerifiedHeightContext,
        broadcast: &RecoveredLifecycleSignedBroadcastProjectionV1,
    ) -> bool {
        self.repair.matches_signed_broadcast(verified, broadcast)
    }

    /// Bind this authority to one frame already loaded from the exact store.
    pub(super) fn belongs_to_loaded(
        &self,
        store: &super::ledger::LifecycleLedgerStoreV1,
        ledger: &super::ledger::LifecycleLedgerV1,
    ) -> bool {
        self.receipt.belongs_to_loaded(store, ledger)
    }
}

/// Join one recovered Validate binding to the exact current vote continuation
/// authenticated from its latest matching WAL frame.
///
/// Every check is read-only. Success consumes all move-only inputs into one
/// opaque recovery value; failure returns those inputs unchanged.
#[allow(clippy::result_large_err)]
pub(super) fn authenticate_recovered_wal_vote_lifecycle_from_ledger_parent(
    verified: &VerifiedHeightContext,
    parent: &AuthenticatedRecoveredWalValidateLedgerParent,
    successor: RecoveredWalVoteSuccessor,
) -> Result<AuthenticatedWalVoteLifecycleRepair, RecoveredWalVoteLifecycleRepairError> {
    authenticate_recovered_wal_vote_lifecycle(
        verified,
        RecoveredValidatePayloadAuthority::Ledger(parent),
        successor,
    )
}

/// Join one recovered Validate binding to the exact durable body retained by
/// its installed completion carrier.
#[allow(clippy::result_large_err)]
pub(super) fn authenticate_recovered_wal_vote_lifecycle_from_durable_body(
    verified: &VerifiedHeightContext,
    durable: &DurableBodyReceipt,
    replay_evidence: &DurableValidateReplayEvidenceV1,
    successor: RecoveredWalVoteSuccessor,
) -> Result<AuthenticatedWalVoteLifecycleRepair, RecoveredWalVoteLifecycleRepairError> {
    authenticate_recovered_wal_vote_lifecycle(
        verified,
        RecoveredValidatePayloadAuthority::Durable {
            receipt: durable,
            replay_evidence,
        },
        successor,
    )
}

#[allow(variant_size_differences)]
enum RecoveredValidatePayloadAuthority<'a> {
    Ledger(&'a AuthenticatedRecoveredWalValidateLedgerParent),
    Durable {
        receipt: &'a DurableBodyReceipt,
        replay_evidence: &'a DurableValidateReplayEvidenceV1,
    },
}

#[allow(clippy::result_large_err)]
fn authenticate_recovered_wal_vote_lifecycle(
    verified: &VerifiedHeightContext,
    parent_payload: RecoveredValidatePayloadAuthority<'_>,
    successor: RecoveredWalVoteSuccessor,
) -> Result<AuthenticatedWalVoteLifecycleRepair, RecoveredWalVoteLifecycleRepairError> {
    let projected = match parent_payload {
        RecoveredValidatePayloadAuthority::Ledger(parent) => {
            successor.into_ledger_lifecycle_projection(verified, parent)
        }
        RecoveredValidatePayloadAuthority::Durable {
            receipt,
            replay_evidence,
        } => successor.into_durable_lifecycle_projection(verified, receipt, replay_evidence),
    };
    let projection = match projected {
        Ok(projection) => projection,
        Err(failure) => {
            let (kind, successor) = match failure {
                RecoveredWalVoteProjectionFailure::InvalidWalIdentity(successor) => (
                    RecoveredWalVoteLifecycleRepairErrorKind::InvalidWalIdentity,
                    successor,
                ),
                RecoveredWalVoteProjectionFailure::InvalidReplayEvidence(successor) => (
                    RecoveredWalVoteLifecycleRepairErrorKind::InvalidReplayEvidence,
                    successor,
                ),
                RecoveredWalVoteProjectionFailure::Parent(successor) => (
                    RecoveredWalVoteLifecycleRepairErrorKind::ParentProjection,
                    successor,
                ),
                RecoveredWalVoteProjectionFailure::Child(successor) => (
                    RecoveredWalVoteLifecycleRepairErrorKind::ChildProjection,
                    successor,
                ),
            };
            return Err(RecoveredWalVoteLifecycleRepairError {
                kind,
                _retained: RecoveredWalVoteLifecycleRepairRetained::Successor {
                    _successor: successor,
                },
            });
        }
    };

    let structural = (|| {
        let parent = projection.parent();
        let child = projection.child();
        if !candidate_shape_is_exact(parent, LifecycleWorkClass::Validate)
            || parent.key.phase() != super::LifecyclePhase::Validate
            || parent.stage.kind() != LifecycleStageKind::ValidateBody
        {
            Err(RecoveredWalVoteLifecycleRepairErrorKind::InvalidParent)
        } else {
            let edge = match (child.key.phase(), child.stage.kind()) {
                (super::LifecyclePhase::Prepare, LifecycleStageKind::SignPrepareVote) => {
                    Some(DurableContinuationEdge::ValidateToSignPrepare)
                }
                (super::LifecyclePhase::Commit, LifecycleStageKind::SignCommitVote) => {
                    Some(DurableContinuationEdge::ValidateToSignCommit)
                }
                _ => None,
            }
            .ok_or(RecoveredWalVoteLifecycleRepairErrorKind::InvalidChild)?;
            if !candidate_shape_is_exact(child, LifecycleWorkClass::SignVote) {
                Err(RecoveredWalVoteLifecycleRepairErrorKind::InvalidChild)
            } else if parent.causal_root != child.causal_root
                || parent.reconstruction_source != child.reconstruction_source
            {
                Err(RecoveredWalVoteLifecycleRepairErrorKind::ForeignOwner)
            } else if !durable_continuation_successor_is_exact(
                edge,
                parent.work_class,
                parent.key,
                parent.stage,
                child.work_class,
                child.key,
                child.stage,
            ) {
                Err(RecoveredWalVoteLifecycleRepairErrorKind::ForeignLineage)
            } else {
                Ok(edge)
            }
        }
    })();
    match structural {
        Ok(edge) => Ok(AuthenticatedWalVoteLifecycleRepair { projection, edge }),
        Err(kind) => Err(RecoveredWalVoteLifecycleRepairError {
            kind,
            _retained: RecoveredWalVoteLifecycleRepairRetained::Projection {
                _projection: projection,
            },
        }),
    }
}

fn candidate_shape_is_exact(
    candidate: &CandidateAdmission,
    expected_work_class: LifecycleWorkClass,
) -> bool {
    let canonical = candidate.physical_geometry.canonicalized();
    let normalized = candidate.physical_geometry.normalized();
    let payload_is_exact = match expected_work_class {
        LifecycleWorkClass::Validate => {
            durable_validate_payload_is_exact(candidate.key, candidate.payload)
        }
        LifecycleWorkClass::SignVote => candidate.payload == DurablePayloadReference::None,
        _ => false,
    };
    candidate.work_class == expected_work_class
        && candidate.stage.predecessor_scope() == PredecessorScope::Independent
        && candidate.initial_state == InitialLifecycleState::Ready
        && candidate.causal_root.digest() == candidate.reconstruction_source
        && payload_is_exact
        && candidate.producer_turn.is_none()
        && matches!(
            (canonical, normalized),
            (Ok(canonical), Ok((physical, universe, consumed)))
                if canonical == candidate.physical_geometry
                    && physical.len() == 1
                    && universe.len() == 1
                    && consumed == universe
                    && physical.keys().all(|slot| {
                        slot.capacity_class() == Some(CapacityClass::Effect)
                    })
        )
}
