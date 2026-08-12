//! Sealed restart join for WAL-ahead Validate vote continuations.

use iroha_data_model::block::consensus_v2 as wire;

use super::{
    CandidateAdmission, CapacityClass, DurablePayloadReference, DurableValidateReplayEvidenceV1,
    InitialLifecycleState, LifecycleStageKind, LifecycleWorkClass, PredecessorScope,
    RecoveredDecisionApplyCandidateLineageV1, RecoveredDecisionApplyReplayLineageV1,
    RecoveredWalControlCandidateProjectionV1, RecoveredWalControlReplayEvidenceV1,
    RecoveredWalDecisionFetchCandidateProjectionV1, RecoveredWalDecisionFetchReplayEvidenceV1,
    body_pipeline_transition::{
        durable_continuation_successor_is_exact, durable_validate_payload_is_exact,
    },
    ledger::{AuthenticatedRecoveredWalValidateLedgerParent, DurableWalVoteLedgerRepairReceipt},
    projection,
    schema::DurableContinuationEdge,
};
use crate::sumeragi::{
    v2::{
        AdapterEffect, RecoveredDecisionApplyCandidateProjectionPermit, RecoveredWalFrameIdentity,
        VerifiedHeightContext,
    },
    v2_body_store::{
        DurableBodyReceipt, RecoveredDecisionApplyAdapterPreviewPermit,
        RecoveredDecisionApplyReplayPermit, ValidatedBodyReceipt,
    },
    v2_runtime::{
        PendingRuntimeEffectBinding, RecoveredWalCandidateProjectionPermit,
        RecoveredWalVoteProjectionFailure, RecoveredWalVoteSuccessor,
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

    /// Prove the authenticated recovery cut retains this exact logical Sign.
    pub(super) fn owns_recovery(
        &self,
        recovery: &super::open::AuthenticatedLifecycleRecoveryCut,
    ) -> bool {
        recovery.owns_recovered_wal_control_sign(&self.projection)
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

impl DurableRecoveredWalDecisionFetchCarrierV1 {
    /// Return the digest only while paired with the complete carrier.
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

    /// Reopen and match the exact durable standalone Fetch row.
    pub(super) fn validates_in_store(&self, store: &super::ledger::LifecycleLedgerStoreV1) -> bool {
        store.revalidates_authenticated_wal_decision_fetch(&self.projection, self.ordinal)
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

    /// Prove the authenticated recovery cut retains this exact Fetch.
    pub(super) fn owns_recovery(
        &self,
        recovery: &super::open::AuthenticatedLifecycleRecoveryCut,
    ) -> bool {
        recovery.owns_recovered_wal_decision_fetch(&self.projection)
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
    pub(in crate::sumeragi) const fn from_runtime_projection(
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

    /// Borrow only the recovered Sign effect retained by this durable repair.
    ///
    /// This narrow view exists solely so the closed concrete-registry carrier
    /// can satisfy the registry's non-consuming effect-borrow contract. It
    /// exposes neither pending binding nor a consuming effect/authority pair.
    pub(super) const fn installed_child_effect(&self) -> &AdapterEffect {
        self.repair.projection.installed_child_effect()
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
