//! Inert coordinator staging for adjacent direct body-pipeline successors.

use super::{
    AdapterEffectAdmissionError, AdmissionDecision, AdmissionRequest, CapacityClass,
    CoordinatorFault, InitialLifecycleState, LifecycleCoordinator, LifecyclePhase,
    LifecycleStageKind, LifecycleState, LifecycleWorkClass, OwnerId, PhysicalSlotId,
    PredecessorScope, TerminalOutcome, TurnLease, TurnOutcome, projection,
    schema::DurablePayloadReference,
};
use crate::sumeragi::{
    v2::{AdapterEffect, VerifiedHeightContext},
    v2_body_store::ValidatedBodyReceipt,
    v2_runtime::PendingRuntimeEffectBinding,
};

/// Closed adjacent body-pipeline edges accepted by the shared staged reducer.
#[derive(Clone, Copy)]
enum BodyStageTransitionEdge {
    FetchToStore,
    StoreToValidate,
    ValidateToApply,
}

impl BodyStageTransitionEdge {
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
            Self::ValidateToApply => {
                child.execution_commitment().is_some()
                    && parent
                        .execution_commitment()
                        .is_none_or(|commitment| child.execution_commitment() == Some(commitment))
            }
        }
    }
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
    InvalidValidationReceipt,
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

/// Stage one exact adjacent body lifecycle transition on a coordinator copy.
///
/// Projection and every exactness check precede cloning. On the staged copy,
/// the parent is terminalized before admission so a full Effect capacity can
/// admit its one-slot successor without a transient extra charge.
#[allow(clippy::too_many_lines)]
fn stage_body_stage_transition(
    coordinator: &LifecycleCoordinator,
    lease: &TurnLease,
    verified: &VerifiedHeightContext,
    child_effect: &AdapterEffect,
    child_pending: &PendingRuntimeEffectBinding,
    edge: BodyStageTransitionEdge,
) -> Result<StagedBodyStageTransition, BodyStageTransitionError> {
    let (parent_work_class, parent_phase, parent_stage) = edge.parent();
    let (child_work_class, child_phase, child_stage) = edge.child();
    if parent_work_class.capacity_class() != CapacityClass::Effect
        || child_work_class.capacity_class() != CapacityClass::Effect
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
    let parent = coordinator
        .records
        .get(&lease.ordinal())
        .ok_or(BodyStageTransitionError::StaleLease)?;
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
        || coordinator
            .durable_records
            .get(&parent.ordinal)
            .is_none_or(|metadata| {
                metadata.reconstruction_source != parent.owner.causal_root().digest()
                    || metadata.payload != DurablePayloadReference::None
            })
    {
        return Err(BodyStageTransitionError::StaleLease);
    }

    let request = projection::admission_request(
        coordinator.active_context,
        verified,
        child_effect,
        child_pending,
    )
    .map_err(BodyStageTransitionError::Projection)?;
    let AdmissionRequest::Candidate(candidate) = &request else {
        return Err(BodyStageTransitionError::InvalidChildProjection);
    };
    let child_slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
    let child_digest = digest_from_hash(child_pending.exact_effect_identity());
    let (projected_slots, projected_universe, projected_consumed) = candidate
        .physical_geometry
        .normalized()
        .map_err(|_| BodyStageTransitionError::InvalidChildProjection)?;
    if candidate.work_class != child_work_class
        || candidate.key.phase() != child_phase
        || candidate.stage.kind() != child_stage
        || candidate.stage.predecessor_scope() != PredecessorScope::Independent
        || candidate.initial_state != InitialLifecycleState::Ready
        || candidate.payload != DurablePayloadReference::None
        || candidate.producer_turn.is_some()
        || projected_slots.len() != 1
        || projected_slots.get(&child_slot) != Some(&child_digest)
        || projected_universe.len() != 1
        || !projected_universe.contains(&child_slot)
        || projected_consumed.len() != 1
        || !projected_consumed.contains(&child_slot)
    {
        return Err(BodyStageTransitionError::InvalidChildProjection);
    }
    if candidate.causal_root != lease.owner().causal_root() {
        return Err(BodyStageTransitionError::ForeignSuccessorOwner);
    }
    if !edge.preserves_lineage(lease.key(), candidate.key) {
        return Err(BodyStageTransitionError::ForeignSuccessorLineage);
    }

    let expected_child_ordinal = coordinator
        .high_water
        .checked_add(1)
        .ok_or(BodyStageTransitionError::OrdinalExhausted)?;
    let effect_capacity_before = coordinator.capacity_used[&CapacityClass::Effect];
    let expected_effect_generation = coordinator.capacity_generation[&CapacityClass::Effect]
        .checked_add(1)
        .ok_or(BodyStageTransitionError::InvalidCapacityTransition)?;
    let projected_candidate = candidate.clone();
    let records_before = coordinator.records.len();
    let durable_records_before = coordinator.durable_records.len();
    let mut staged = coordinator.stage_durable_transaction();
    staged.reduce_settle_turn(lease.clone(), TurnOutcome::Advanced, None);
    if let Some(fault) = staged.fault {
        return Err(BodyStageTransitionError::ParentSettlement(fault));
    }
    let decision = staged.reduce_admit(request);
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
                    || metadata.payload != DurablePayloadReference::None
            })
        || !staged
            .durable_records
            .get(&child_ordinal)
            .is_some_and(|metadata| metadata.matches_admission(&projected_candidate))
    {
        return Err(BodyStageTransitionError::InvalidStagedRecords);
    }
    if staged.capacity_used[&CapacityClass::Effect] != effect_capacity_before
        || staged.capacity_generation[&CapacityClass::Effect] != expected_effect_generation
        || CapacityClass::ALL
            .into_iter()
            .filter(|class| *class != CapacityClass::Effect)
            .any(|class| {
                staged.capacity_used[&class] != coordinator.capacity_used[&class]
                    || staged.capacity_generation[&class] != coordinator.capacity_generation[&class]
            })
    {
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
    edge: BodyStageTransitionEdge,
    parent_ordinal: u128,
    child_ordinal: u128,
    owner: OwnerId,
    child_slot: PhysicalSlotId,
    child_digest: super::LifecycleDigest,
}

impl LifecycleCoordinator {
    /// Stage one exact claimed Fetch retirement and Store admission without
    /// mutating or persisting the live coordinator.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn prepare_fetch_store_transition<'a>(
        &'a mut self,
        lease: &TurnLease,
        verified: &VerifiedHeightContext,
        store_effect: &AdapterEffect,
        store_pending: &PendingRuntimeEffectBinding,
    ) -> Result<PreparedBodyStageTransition<'a>, BodyStageTransitionError> {
        self.prepare_body_stage_transition(
            lease,
            verified,
            store_effect,
            store_pending,
            BodyStageTransitionEdge::FetchToStore,
        )
    }

    /// Stage one exact claimed Store retirement and Validate admission without
    /// mutating or persisting the live coordinator.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn prepare_store_validate_transition<'a>(
        &'a mut self,
        lease: &TurnLease,
        verified: &VerifiedHeightContext,
        validate_effect: &AdapterEffect,
        validate_pending: &PendingRuntimeEffectBinding,
    ) -> Result<PreparedBodyStageTransition<'a>, BodyStageTransitionError> {
        self.prepare_body_stage_transition(
            lease,
            verified,
            validate_effect,
            validate_pending,
            BodyStageTransitionEdge::StoreToValidate,
        )
    }

    /// Stage one exact claimed Validate retirement and Apply admission without
    /// mutating or persisting the live coordinator.
    ///
    /// Unlike the earlier body edges, an ordinary Validate key may acquire the
    /// exact Commit execution commitment carried by its Apply successor. A
    /// commitment already present on the parent must remain byte-identical.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn prepare_validate_apply_transition<'a>(
        &'a mut self,
        lease: &TurnLease,
        verified: &VerifiedHeightContext,
        validated_receipt: &ValidatedBodyReceipt,
        apply_effect: &AdapterEffect,
        apply_pending: &PendingRuntimeEffectBinding,
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
        if durable.context_id() != verified.context().id()
            || durable.round() != certificate.proposal_round
            || durable.subject() != *subject
            || validated_receipt.execution_commitment() != certificate.execution_commitment
        {
            return Err(BodyStageTransitionError::InvalidValidationReceipt);
        }
        self.prepare_body_stage_transition(
            lease,
            verified,
            apply_effect,
            apply_pending,
            BodyStageTransitionEdge::ValidateToApply,
        )
    }

    fn prepare_body_stage_transition<'a>(
        &'a mut self,
        lease: &TurnLease,
        verified: &VerifiedHeightContext,
        child_effect: &AdapterEffect,
        child_pending: &PendingRuntimeEffectBinding,
        edge: BodyStageTransitionEdge,
    ) -> Result<PreparedBodyStageTransition<'a>, BodyStageTransitionError> {
        let transition =
            stage_body_stage_transition(self, lease, verified, child_effect, child_pending, edge)?;
        Ok(PreparedBodyStageTransition {
            _coordinator: self,
            staged: transition.staged,
            edge,
            parent_ordinal: transition.parent_ordinal,
            child_ordinal: transition.child_ordinal,
            owner: transition.owner,
            child_slot: transition.child_slot,
            child_digest: transition.child_digest,
        })
    }
}

fn digest_from_hash(hash: &iroha_crypto::Hash) -> super::LifecycleDigest {
    let mut bytes = [0_u8; 32];
    bytes.copy_from_slice(hash.as_ref());
    super::LifecycleDigest::new(bytes)
}

#[cfg(test)]
mod static_tests {
    #[test]
    fn transition_surface_is_ordered_borrow_bound_and_inert() {
        let source = include_str!("v2_lifecycle_body_pipeline_transition.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("transition source has one production prefix");
        let projection = production
            .find("projection::admission_request")
            .expect("staged transition projects its child");
        let staging = production
            .find("stage_durable_transaction")
            .expect("staged transition clones coordinator state");
        let settlement = production
            .find("reduce_settle_turn")
            .expect("staged transition settles its parent");
        let admission = production
            .find("reduce_admit")
            .expect("staged transition admits its child");
        assert!(
            settlement < admission,
            "the parent must release capacity before child admission"
        );
        assert!(
            projection < staging,
            "projection must precede state staging"
        );
        assert!(production.contains("&'a mut LifecycleCoordinator"));
        assert!(production.contains("BodyStageTransitionEdge::FetchToStore"));
        assert!(production.contains("BodyStageTransitionEdge::StoreToValidate"));
        assert!(production.contains("BodyStageTransitionEdge::ValidateToApply"));
        assert!(!production.contains("pub(super) enum BodyStageTransitionEdge"));
        assert!(!production.contains("pub(super) fn stage_body_stage_transition"));
        for forbidden in [
            "persist_durable_projection",
            "fn commit(",
            "fn staged(",
            "work_registry",
            "RuntimeEffectOwnership",
            "legacy_ordinal",
        ] {
            assert!(
                !production.contains(forbidden),
                "inert transition acquired forbidden authority: {forbidden}"
            );
        }
    }
}

#[cfg(all(test, feature = "bls"))]
mod tests {
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::{block::consensus_v2 as wire, peer::PeerId};

    use super::*;
    use crate::sumeragi::{
        v2_core::{EventTag, Generation},
        v2_runtime::{RuntimeEffectOwnership, bind_adapter_effect_batch_ownership},
    };

    struct FetchStoreFixture {
        coordinator: LifecycleCoordinator,
        lease: TurnLease,
        verified: VerifiedHeightContext,
        store_effect: AdapterEffect,
        store_pending: PendingRuntimeEffectBinding,
    }

    struct StoreValidateFixture {
        coordinator: LifecycleCoordinator,
        lease: TurnLease,
        verified: VerifiedHeightContext,
        store_effect: AdapterEffect,
        store_pending: PendingRuntimeEffectBinding,
        validate_effect: AdapterEffect,
        validate_pending: PendingRuntimeEffectBinding,
    }

    struct ValidateApplyFixture {
        coordinator: LifecycleCoordinator,
        lease: TurnLease,
        verified: VerifiedHeightContext,
        validate_effect: AdapterEffect,
        validate_pending: PendingRuntimeEffectBinding,
        validated_receipt: ValidatedBodyReceipt,
        apply_effect: AdapterEffect,
        apply_pending: PendingRuntimeEffectBinding,
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
            chain_id: "fetch-store-transition-test".into(),
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
        let fetch_effect = AdapterEffect::FetchBody {
            tag,
            round,
            subject,
            manifest: None,
            certified_sources: context
                .roster
                .iter()
                .map(|validator| validator.validator.clone())
                .collect(),
            certificate: Some(certificate),
        };
        let store_effect = AdapterEffect::StoreBody {
            tag,
            round,
            subject,
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
        let store_request = projection::admission_request(
            lifecycle_context(&context),
            &verified,
            &store_effect,
            &store_pending,
        )
        .expect("project Store candidate fixture");
        let AdmissionRequest::Candidate(store_candidate) = store_request else {
            panic!("Store fixture projects one candidate")
        };
        let fetch_key = super::super::LifecycleKey::new(
            store_candidate.key.context(),
            store_candidate.key.round(),
            store_candidate.key.proposal_round(),
            store_candidate.key.subject(),
            LifecyclePhase::Fetch,
            store_candidate.key.execution_commitment(),
        );
        let fetch_slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
        let parent = super::super::CandidateAdmission::new(
            fetch_key,
            store_candidate.causal_root,
            LifecycleWorkClass::Fetch,
            super::super::LifecycleStage::new(
                LifecycleStageKind::FetchBody,
                PredecessorScope::Independent,
            ),
            InitialLifecycleState::Ready,
            store_candidate.reconstruction_source,
            DurablePayloadReference::None,
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
        let ready =
            super::super::SchedulerReadyInputs::new(record.owner, record.key, 0, 0, 0, 0, 0, 0);
        let inputs = super::super::SchedulerInputs::new([], [(ordinal, ready)])
            .expect("unique Fetch scheduler census");
        let super::super::TurnPlan::Execute(lease) = coordinator.plan_turn(inputs) else {
            panic!("claim Fetch fixture")
        };
        FetchStoreFixture {
            coordinator,
            lease,
            verified,
            store_effect,
            store_pending,
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

    fn store_validate_fixture_with_authority(
        effect_limit: usize,
        inherited_authority: Option<wire::GlobalPhase>,
    ) -> StoreValidateFixture {
        let FetchStoreFixture {
            verified,
            store_effect,
            store_pending: certified_store_pending,
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
        let store_pending = if inherited_authority.is_some() {
            certified_store_pending
        } else {
            let store_owner = bind_adapter_effect_batch_ownership(
                core::slice::from_ref(&store_effect),
                vec![RuntimeEffectOwnership::fresh_for_test(tag, 2)],
            )
            .expect("bind ordinary Store fixture")
            .pop()
            .expect("one ordinary Store owner");
            store_owner
                .pending_adapter_effect_binding(&store_effect)
                .expect("mint sealed ordinary Store binding")
        };
        let validate_effect = AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        };
        let validate_pending = store_pending
            .project_store_validate_successor(&store_effect, &validate_effect)
            .expect("project exact ordinal-free Validate successor");
        let store_request = projection::admission_request(
            lifecycle_context(verified.context()),
            &verified,
            &store_effect,
            &store_pending,
        )
        .expect("project Store parent fixture");
        let AdmissionRequest::Candidate(store_candidate) = store_request else {
            panic!("Store fixture projects one candidate")
        };
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
        let ready =
            super::super::SchedulerReadyInputs::new(record.owner, record.key, 0, 0, 0, 0, 0, 0);
        let inputs = super::super::SchedulerInputs::new([], [(ordinal, ready)])
            .expect("unique Store scheduler census");
        let super::super::TurnPlan::Execute(lease) = coordinator.plan_turn(inputs) else {
            panic!("claim Store fixture")
        };
        StoreValidateFixture {
            coordinator,
            lease,
            verified,
            store_effect,
            store_pending,
            validate_effect,
            validate_pending,
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
            validate_effect,
            validate_pending,
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
        let durable_receipt = crate::sumeragi::v2_body_store::DurableBodyReceipt::for_test(
            verified.context().id(),
            round,
            subject,
            HashOf::from_untyped_unchecked(Hash::new(b"validate-apply manifest")),
        );
        let validated_receipt = ValidatedBodyReceipt::for_test_with_commitment(
            durable_receipt,
            fixture_execution_commitment(),
        );
        let validate_request = projection::admission_request(
            lifecycle_context(verified.context()),
            &verified,
            &validate_effect,
            &validate_pending,
        )
        .expect("project Validate parent fixture");
        let AdmissionRequest::Candidate(validate_candidate) = validate_request else {
            panic!("Validate fixture projects one candidate")
        };
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
        let ready =
            super::super::SchedulerReadyInputs::new(record.owner, record.key, 0, 0, 0, 0, 0, 0);
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
            validated_receipt,
            apply_effect,
            apply_pending,
        }
    }

    #[test]
    fn full_effect_capacity_stages_net_zero_success_and_drop_is_inert() {
        let FetchStoreFixture {
            mut coordinator,
            lease,
            verified,
            store_effect,
            store_pending,
        } = fetch_store_fixture(1);
        assert_eq!(coordinator.capacity_used[&CapacityClass::Effect], 1);
        let before = format!("{coordinator:#?}");
        let prepared = coordinator
            .prepare_fetch_store_transition(&lease, &verified, &store_effect, &store_pending)
            .expect("Fetch release makes room for exact Store at full capacity");
        assert!(matches!(
            prepared.edge,
            BodyStageTransitionEdge::FetchToStore
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
        drop(prepared);
        assert_eq!(format!("{coordinator:#?}"), before);
    }

    #[test]
    fn wrong_and_stale_fetch_leases_reject_without_coordinator_mutation() {
        let FetchStoreFixture {
            mut coordinator,
            lease,
            verified,
            store_effect,
            store_pending,
        } = fetch_store_fixture(1);
        let before = format!("{coordinator:#?}");
        let mut wrong = lease.clone();
        wrong.work_class = LifecycleWorkClass::Store;
        assert!(matches!(
            coordinator.prepare_fetch_store_transition(
                &wrong,
                &verified,
                &store_effect,
                &store_pending
            ),
            Err(BodyStageTransitionError::WrongParentShape)
        ));
        assert_eq!(format!("{coordinator:#?}"), before);

        let mut stale = lease.clone();
        stale.id = super::super::LeaseId(lease.id().0 + 1);
        assert!(matches!(
            coordinator.prepare_fetch_store_transition(
                &stale,
                &verified,
                &store_effect,
                &store_pending
            ),
            Err(BodyStageTransitionError::StaleLease)
        ));
        assert_eq!(format!("{coordinator:#?}"), before);
    }

    #[test]
    fn foreign_store_projection_rejects_without_coordinator_mutation() {
        let FetchStoreFixture {
            mut coordinator,
            lease,
            verified,
            store_effect,
            store_pending,
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
            coordinator.prepare_fetch_store_transition(&lease, &verified, &foreign, &store_pending),
            Err(BodyStageTransitionError::Projection(
                AdapterEffectAdmissionError::UnboundEffect
            ))
        ));
        assert_eq!(format!("{coordinator:#?}"), before);
    }

    #[test]
    fn staged_capacity_wait_leaves_fetch_parent_claimed() {
        let FetchStoreFixture {
            mut coordinator,
            lease,
            verified,
            store_effect,
            store_pending,
        } = fetch_store_fixture(1);
        coordinator
            .capacity_geometry
            .limits
            .insert(CapacityClass::Effect, 0);
        let before_capacity = format!("{coordinator:#?}");
        assert!(matches!(
            coordinator.prepare_fetch_store_transition(
                &lease,
                &verified,
                &store_effect,
                &store_pending
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
            verified,
            store_effect,
            store_pending,
        } = fetch_store_fixture(1);
        coordinator.high_water = u128::MAX;
        let before_ordinal = format!("{coordinator:#?}");
        assert!(matches!(
            coordinator.prepare_fetch_store_transition(
                &lease,
                &verified,
                &store_effect,
                &store_pending
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
            verified,
            validate_effect,
            validate_pending,
            ..
        } = store_validate_fixture(1, true);
        let AdmissionRequest::Candidate(validate_candidate) = projection::admission_request(
            lifecycle_context(verified.context()),
            &verified,
            &validate_effect,
            &validate_pending,
        )
        .expect("project exact Validate fixture") else {
            panic!("Validate fixture projects one candidate")
        };
        let capacity_used_before = coordinator.capacity_used.clone();
        let capacity_generation_before = coordinator.capacity_generation.clone();
        let before = format!("{coordinator:#?}");
        let prepared = coordinator
            .prepare_store_validate_transition(
                &lease,
                &verified,
                &validate_effect,
                &validate_pending,
            )
            .expect("Store release makes room for exact Validate at full capacity");

        assert!(matches!(
            prepared.edge,
            BodyStageTransitionEdge::StoreToValidate
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
        assert_eq!(parent_metadata.payload, DurablePayloadReference::None);

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
            verified,
            validate_effect,
            validate_pending,
            ..
        } = store_validate_fixture(1, false);
        assert_eq!(lease.key().execution_commitment(), None);
        let before = format!("{coordinator:#?}");
        let prepared = coordinator
            .prepare_store_validate_transition(
                &lease,
                &verified,
                &validate_effect,
                &validate_pending,
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
    fn wrong_and_stale_store_leases_reject_without_coordinator_mutation() {
        let StoreValidateFixture {
            mut coordinator,
            lease,
            verified,
            validate_effect,
            validate_pending,
            ..
        } = store_validate_fixture(1, true);
        let before = format!("{coordinator:#?}");
        let mut wrong = lease.clone();
        wrong.work_class = LifecycleWorkClass::Validate;
        assert!(matches!(
            coordinator.prepare_store_validate_transition(
                &wrong,
                &verified,
                &validate_effect,
                &validate_pending,
            ),
            Err(BodyStageTransitionError::WrongParentShape)
        ));
        assert_eq!(format!("{coordinator:#?}"), before);

        let mut stale = lease.clone();
        stale.id = super::super::LeaseId(lease.id().0 + 1);
        assert!(matches!(
            coordinator.prepare_store_validate_transition(
                &stale,
                &verified,
                &validate_effect,
                &validate_pending,
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
            store_effect,
            store_pending,
            validate_effect,
            validate_pending,
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
            coordinator.prepare_store_validate_transition(
                &lease,
                &verified,
                &wrong_effect,
                &validate_pending,
            ),
            Err(BodyStageTransitionError::Projection(
                AdapterEffectAdmissionError::UnboundEffect
            ))
        ));
        assert_eq!(format!("{coordinator:#?}"), before);
        assert!(matches!(
            coordinator.prepare_store_validate_transition(
                &lease,
                &verified,
                &validate_effect,
                &store_pending,
            ),
            Err(BodyStageTransitionError::Projection(
                AdapterEffectAdmissionError::UnboundEffect
            ))
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
        assert!(matches!(
            coordinator.prepare_store_validate_transition(
                &lease,
                &verified,
                &validate_effect,
                &foreign_validate_pending,
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
            verified,
            validate_effect,
            validate_pending,
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
            coordinator.prepare_store_validate_transition(
                &lease,
                &verified,
                &validate_effect,
                &validate_pending,
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
            verified,
            validate_effect,
            validate_pending,
            ..
        } = store_validate_fixture(1, true);
        coordinator.high_water = u128::MAX;
        let before = format!("{coordinator:#?}");
        assert!(matches!(
            coordinator.prepare_store_validate_transition(
                &lease,
                &verified,
                &validate_effect,
                &validate_pending,
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
            verified,
            validate_effect,
            validate_pending,
            ..
        } = store_validate_fixture(1, true);
        coordinator
            .capacity_generation
            .insert(CapacityClass::Effect, u64::MAX);
        let before = format!("{coordinator:#?}");
        assert!(matches!(
            coordinator.prepare_store_validate_transition(
                &lease,
                &verified,
                &validate_effect,
                &validate_pending,
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
            verified,
            validate_effect,
            validate_pending,
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
            coordinator.prepare_store_validate_transition(
                &lease,
                &verified,
                &validate_effect,
                &validate_pending,
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
            verified,
            validated_receipt,
            apply_effect,
            apply_pending,
            ..
        } = validate_apply_fixture(1, true);
        let AdmissionRequest::Candidate(apply_candidate) = projection::admission_request(
            lifecycle_context(verified.context()),
            &verified,
            &apply_effect,
            &apply_pending,
        )
        .expect("project exact Apply fixture") else {
            panic!("Apply fixture projects one candidate")
        };
        let capacity_used_before = coordinator.capacity_used.clone();
        let capacity_generation_before = coordinator.capacity_generation.clone();
        let before = format!("{coordinator:#?}");
        let prepared = coordinator
            .prepare_validate_apply_transition(
                &lease,
                &verified,
                &validated_receipt,
                &apply_effect,
                &apply_pending,
            )
            .expect("Validate release makes room for exact Apply at full capacity");

        assert!(matches!(
            prepared.edge,
            BodyStageTransitionEdge::ValidateToApply
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

    #[test]
    fn validate_apply_acquires_commit_authority_for_ordinary_validation() {
        let ValidateApplyFixture {
            mut coordinator,
            lease,
            verified,
            validated_receipt,
            apply_effect,
            apply_pending,
            ..
        } = validate_apply_fixture(1, false);
        assert_eq!(lease.key().execution_commitment(), None);
        let before = format!("{coordinator:#?}");
        let prepared = coordinator
            .prepare_validate_apply_transition(
                &lease,
                &verified,
                &validated_receipt,
                &apply_effect,
                &apply_pending,
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
    fn validate_apply_rejects_wrong_binding_and_foreign_commitment_without_mutation() {
        let ValidateApplyFixture {
            mut coordinator,
            lease,
            verified,
            validate_effect,
            validate_pending,
            validated_receipt,
            apply_effect,
            apply_pending,
        } = validate_apply_fixture(1, true);
        let before = format!("{coordinator:#?}");
        assert!(matches!(
            coordinator.prepare_validate_apply_transition(
                &lease,
                &verified,
                &validated_receipt,
                &apply_effect,
                &validate_pending,
            ),
            Err(BodyStageTransitionError::Projection(
                AdapterEffectAdmissionError::UnboundEffect
            ))
        ));
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
            coordinator.prepare_validate_apply_transition(
                &lease,
                &verified,
                &validated_receipt,
                &foreign_apply,
                &apply_pending,
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
        let before = format!("{coordinator:#?}");
        assert!(matches!(
            coordinator.prepare_validate_apply_transition(
                &lease,
                &verified,
                &validated_receipt,
                &foreign_apply,
                &foreign_pending,
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
            verified,
            validate_effect,
            validate_pending,
            validated_receipt,
            apply_effect,
            apply_pending,
        } = validate_apply_fixture_with_authority(1, Some(wire::GlobalPhase::Commit));
        assert!(lease.key().execution_commitment().is_some());
        let before = format!("{coordinator:#?}");
        let prepared = coordinator
            .prepare_validate_apply_transition(
                &lease,
                &verified,
                &validated_receipt,
                &apply_effect,
                &apply_pending,
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
            verified,
            validated_receipt,
            apply_effect,
            apply_pending,
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
            coordinator.prepare_validate_apply_transition(
                &lease,
                &verified,
                &validated_receipt,
                &apply_effect,
                &apply_pending,
            ),
            Err(BodyStageTransitionError::ForeignSuccessorLineage)
        ));
        assert_eq!(format!("{coordinator:#?}"), before);
    }
}
