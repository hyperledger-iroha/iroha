//! Sealed durable-open and authenticated restart reconciliation.

use std::collections::{BTreeMap, BTreeSet};

#[cfg(test)]
use std::path::Path;

use thiserror::Error;

use super::{
    AdmissionDecision, AdmissionRequest, CandidateAdmission, CoordinatorFault,
    DurablePayloadReference, LifecycleContext, LifecycleCoordinator, LifecycleDigest, LifecycleKey,
    LifecycleStage, LifecycleStageKind, LifecycleState, LifecycleWorkClass, TerminalOutcome,
    authority::AuthenticatedEpisodeAuthority,
    body_pipeline_transition::{
        durable_continuation_payload_is_exact, durable_continuation_successor_is_exact,
        durable_validate_payload_is_exact,
    },
    ledger::{
        LifecycleLedgerError, LifecycleLedgerRecordV1, LifecycleLedgerStoreV1, LifecycleLedgerV1,
        RecoveredLifecycleSignedBroadcastAndSignLedgerProjectionV1,
    },
    replay_authority::{
        CertifiedServeTerminalReplayAuthorityPairV1, LifecycleReplayAuthorityV1,
        PreparedDurableCertifiedBodyPipelineStartupV1,
        RecoveredLifecycleNextWalVoteCandidateProjectionV1,
        recovered_decision_body_continuation_is_exact, signed_broadcast_continuation_is_exact,
    },
    schema::{
        CausalRoot, DurableContinuation, DurableContinuationEdge, DurableRecordMetadata,
        LifecycleRecord, MAX_PHYSICAL_SLOTS_PER_RECORD, OwnerId, PredecessorScope,
        RecoverySnapshot, SchedulerEpisode, WaitSource, WaitToken, has_lifecycle_record_capacity,
        serve_and_producer_keys_match,
    },
    wal_recovery::{
        AuthenticatedRecoveredWalControlProjection,
        AuthenticatedRecoveredWalDecisionFetchProjection, RecoveredDecisionFetchStoreProjectionV1,
        RecoveredDecisionValidateInstalledSealV1, RecoveredDecisionValidateProjectionV1,
        RecoveredLifecycleSignedBroadcastAndSignProjectionV1,
    },
    work_registry::{
        AuthenticatedRecoveredWalSignProjection, CertifiedServeRegistryBatchPublicationError,
        ConcreteLifecycleWorkRegistry, PreparedCertifiedServeRegistryBatchV1,
    },
};
/// Exclusive WAL-owned startup projection admitted by storage recovery.
#[derive(Clone, Copy)]
enum RecoveredWalStartupProjectionV1<'authority> {
    None,
    PhaseVote(&'authority AuthenticatedRecoveredWalSignProjection),
    PhaseBroadcast(
        &'authority AuthenticatedRecoveredWalSignProjection,
        &'authority super::wal_recovery::RecoveredLifecycleSignedBroadcastProjectionV1,
    ),
    PhaseBroadcastAndNextSign(
        &'authority AuthenticatedRecoveredWalSignProjection,
        &'authority RecoveredLifecycleSignedBroadcastAndSignLedgerProjectionV1,
        &'authority super::wal_recovery::RecoveredLifecycleSignedBroadcastProjectionV1,
        &'authority RecoveredLifecycleNextWalVoteCandidateProjectionV1,
    ),
    ControlSign(&'authority AuthenticatedRecoveredWalControlProjection),
    ControlBroadcast(
        &'authority AuthenticatedRecoveredWalControlProjection,
        &'authority super::wal_recovery::RecoveredLifecycleSignedBroadcastProjectionV1,
    ),
    ControlBroadcastAndSign(
        &'authority AuthenticatedRecoveredWalControlProjection,
        &'authority RecoveredLifecycleSignedBroadcastAndSignLedgerProjectionV1,
        &'authority RecoveredLifecycleSignedBroadcastAndSignProjectionV1,
    ),
    DecisionFetch(&'authority AuthenticatedRecoveredWalDecisionFetchProjection),
    DecisionStore(
        &'authority AuthenticatedRecoveredWalDecisionFetchProjection,
        &'authority RecoveredDecisionFetchStoreProjectionV1,
    ),
    DecisionValidate(
        &'authority AuthenticatedRecoveredWalDecisionFetchProjection,
        &'authority RecoveredDecisionFetchStoreProjectionV1,
        &'authority RecoveredDecisionValidateProjectionV1,
    ),
    DecisionApply(&'authority crate::sumeragi::v2::RecoveredDecisionApplyStagedStorageV1),
    DecisionReleasedApply(&'authority crate::sumeragi::v2::RecoveredDecisionApplyStagedStorageV1),
}
#[cfg(test)]
use super::RolloverSnapshot;
#[cfg(test)]
use crate::sumeragi::v2_certified_serve_payload_store::{
    CertifiedServePayloadNegativeOutcome, DurableCertifiedServeAdmissionReceipt,
};
use crate::sumeragi::{
    v2::VerifiedHeightContext,
    v2_body_store::{
        DurableBodyValidationOutcome, RecoveredTerminalValidateOutcomeCatalogError, V2BodyStore,
        ValidatedBodyReceipt,
    },
    v2_certified_serve_payload_store::{
        AuthenticatedCertifiedServePayloadRecoveryCut,
        AuthenticatedRecoveredCertifiedServePayloadState, CertifiedServePayloadId,
        CertifiedServePayloadStoreError, CertifiedServePayloadStoreV1,
    },
};
/// Storage-authenticated identity of one terminal Validate with no successor.
///
/// The body outcome is consumed while this seal is minted and cannot be
/// replayed or rebound afterward. The historical no-child reducer branch is
/// represented by the checksummed typed ledger tombstone itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AuthenticatedValidateNoSuccessorRecovery {
    key: LifecycleKey,
    causal_root: CausalRoot,
    reconstruction_source: LifecycleDigest,
    stage: LifecycleStage,
    payload: DurablePayloadReference,
}
/// Ledger-authenticated claim for one terminal Validate body outcome.
///
/// The claim is not storage authority: its private fields are decoded from one
/// exact checksummed ledger row and become authoritative only when the body
/// store's move-only recovery catalog consumes a matching semantic outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::sumeragi) struct TerminalValidateNoSuccessorClaim {
    context: LifecycleContext,
    ordinal: u128,
    key: LifecycleKey,
    owner: OwnerId,
    reconstruction_source: LifecycleDigest,
    stage: LifecycleStage,
    payload: DurablePayloadReference,
    row_identity: LifecycleDigest,
}
impl TerminalValidateNoSuccessorClaim {
    /// Compare one sealed body-store outcome with the complete ledger identity.
    pub(in crate::sumeragi) fn matches_outcome(
        &self,
        outcome: &DurableBodyValidationOutcome,
    ) -> bool {
        super::projection::recovered_validate_no_successor_ledger_identity_is_authenticated(
            self.context,
            self.key,
            self.owner.causal_root(),
            self.reconstruction_source,
            self.stage,
            self.payload,
            outcome,
        )
    }
    /// Compare one successful body-store marker with the complete ledger identity.
    pub(in crate::sumeragi) fn matches_validated_receipt(
        &self,
        receipt: &ValidatedBodyReceipt,
    ) -> bool {
        super::projection::recovered_validate_no_successor_validated_receipt_is_authenticated(
            self.context,
            self.key,
            self.owner.causal_root(),
            self.reconstruction_source,
            self.stage,
            self.payload,
            receipt,
        )
    }
    fn exactly_matches_ledger_record(&self, record: &LifecycleLedgerRecordV1) -> bool {
        record.ordinal() == self.ordinal
            && record.owner() == self.owner
            && record.key() == Some(self.key)
            && record.work_class() == Some(LifecycleWorkClass::Validate)
            && record.stage() == Some(self.stage)
            && record.terminal() == Some(Some(TerminalOutcome::Advanced))
            && record.reconstruction_source() == self.reconstruction_source
            && record.durable_payload() == Some(self.payload)
            && record.continuation() == Some(DurableContinuation::AdvancedNoSuccessor)
            && record.exact_row_identity() == self.row_identity
    }
    fn exactly_matches_coordinator_tombstone(
        &self,
        context: LifecycleContext,
        coordinator: &LifecycleCoordinator,
    ) -> bool {
        if context != self.context
            || coordinator.active_context != context
            || coordinator.fault.is_some()
            || coordinator.high_water < self.ordinal
        {
            return false;
        }
        let Some(record) = coordinator.records.get(&self.ordinal) else {
            return false;
        };
        let Some(metadata) = coordinator.durable_records.get(&self.ordinal) else {
            return false;
        };
        record.ordinal == self.ordinal
            && record.owner == self.owner
            && record.key == self.key
            && record.work_class == LifecycleWorkClass::Validate
            && record.stage == self.stage
            && record.state == LifecycleState::Terminal(TerminalOutcome::Advanced)
            && record.physical_slots.is_empty()
            && metadata.reconstruction_source == self.reconstruction_source
            && metadata.payload == self.payload
            && metadata.continuation == DurableContinuation::AdvancedNoSuccessor
            && coordinator.key_index.get(&self.key) == Some(&self.ordinal)
            && coordinator.owner_index.get(&self.owner.causal_root()) == Some(&self.owner)
            && !coordinator.ready_index.contains(&self.ordinal)
    }
    fn into_authenticated(self) -> AuthenticatedValidateNoSuccessorRecovery {
        AuthenticatedValidateNoSuccessorRecovery {
            key: self.key,
            causal_root: self.owner.causal_root(),
            reconstruction_source: self.reconstruction_source,
            stage: self.stage,
            payload: self.payload,
        }
    }
}
/// Move-only cold authority for one released terminal Validate success.
///
/// Construction requires the body store to consume the sole successful marker
/// selected by this checksummed ledger claim. The token intentionally retains
/// no process-local effect fingerprint: cold recovery can rejoin it only to the
/// exact ledger and coordinator tombstone from which the claim was decoded.
#[derive(Debug)]
#[must_use = "released terminal Validate authority must remain with recovered Apply"]
pub(in crate::sumeragi) struct AuthenticatedRecoveredReleasedValidateNoSuccessorV1 {
    claim: TerminalValidateNoSuccessorClaim,
    validated: ValidatedBodyReceipt,
}
impl AuthenticatedRecoveredReleasedValidateNoSuccessorV1 {
    /// Mint from the exact successful marker consumed by the body-store catalog.
    pub(in crate::sumeragi) fn from_consumed_body_store_success(
        claim: TerminalValidateNoSuccessorClaim,
        validated: ValidatedBodyReceipt,
    ) -> Option<Self> {
        claim
            .matches_validated_receipt(&validated)
            .then_some(Self { claim, validated })
    }
    /// Return whether this authority belongs to the immutable height context.
    pub(in crate::sumeragi) fn belongs_to_context(&self, context: LifecycleContext) -> bool {
        self.claim.context == context
    }
    /// Return the immutable terminal Validate ordinal.
    pub(in crate::sumeragi) const fn ordinal(&self) -> u128 {
        self.claim.ordinal
    }
    /// Return the complete historical lifecycle owner.
    pub(in crate::sumeragi) const fn owner(&self) -> OwnerId {
        self.claim.owner
    }
    /// Compare another receipt without exposing the catalog-consumed marker.
    pub(in crate::sumeragi) fn exactly_matches_validated_receipt(
        &self,
        context: LifecycleContext,
        receipt: &ValidatedBodyReceipt,
    ) -> bool {
        self.belongs_to_context(context)
            && &self.validated == receipt
            && self.claim.matches_validated_receipt(receipt)
    }
    /// Join this authority to the exact retained ledger tombstone.
    pub(super) fn exactly_matches_ledger_record(&self, record: &LifecycleLedgerRecordV1) -> bool {
        self.claim.matches_validated_receipt(&self.validated)
            && self.claim.exactly_matches_ledger_record(record)
    }
    /// Join this authority to the exact reconstructed logical tombstone.
    pub(in crate::sumeragi) fn matches_current_terminal_record(
        &self,
        context: LifecycleContext,
        coordinator: &LifecycleCoordinator,
    ) -> bool {
        self.claim.matches_validated_receipt(&self.validated)
            && self
                .claim
                .exactly_matches_coordinator_tombstone(context, coordinator)
    }
}
/// Move-only, post-authentication join between durable logical rows and their
/// exact storage-reconstructed work.
///
/// Constructors stay inside the lifecycle authority. Production storage code
/// receives this value only after the exhaustive effect classifier, body/WAL
/// reconciliation, and Certified-Serve payload resolver have authenticated all
/// of its parts. Terminal no-successor Validate rows additionally require an
/// exact body-store outcome bound to their immutable parent identity. The
/// move-only payload cut may retain authenticated store-only crash tails;
/// durable open removes those orphans only after every ledger Serve resolves
/// exactly and before the reconciled ledger is published.
#[derive(Debug)]
#[must_use]
pub(crate) struct AuthenticatedLifecycleRecoveryCut {
    context: LifecycleContext,
    /// Exact already-opened frame classified by the storage-only assembler.
    /// Durable open rejects a different reread for every production and focused
    /// test cut; there is no unauthenticated frame bypass.
    authenticated_ledger: LifecycleLedgerV1,
    candidates: BTreeMap<LifecycleKey, CandidateAdmission>,
    validate_no_successor: BTreeMap<LifecycleKey, AuthenticatedValidateNoSuccessorRecovery>,
    released_validate: Option<AuthenticatedRecoveredReleasedValidateNoSuccessorV1>,
    lifecycle_outputs: Option<PreparedLifecycleOutputRecoveryV1>,
    serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
}
impl AuthenticatedLifecycleRecoveryCut {
    /// Detach the sole storage-authenticated released Validate authority.
    pub(in crate::sumeragi) fn take_released_validate_authority(
        &mut self,
    ) -> Option<AuthenticatedRecoveredReleasedValidateNoSuccessorV1> {
        self.released_validate.take()
    }
    /// Detach the complete authenticated cold-output census exactly once.
    pub(in crate::sumeragi) fn take_lifecycle_output_recovery(
        &mut self,
    ) -> Option<PreparedLifecycleOutputRecoveryV1> {
        self.lifecycle_outputs.take()
    }

    /// Consume the exact post-prune Serve payload census into its owner.
    pub(super) fn into_serve_payloads(self) -> AuthenticatedCertifiedServePayloadRecoveryCut {
        debug_assert!(
            self.lifecycle_outputs.is_none(),
            "cold lifecycle outputs must enter their registry before recovery retirement"
        );
        debug_assert!(
            self.released_validate.is_none(),
            "released Validate authority must enter its recovered Apply carrier"
        );
        self.serve_payloads
    }
    /// Assemble an exact test fixture from already authenticated projections.
    ///
    /// Production recovery uses the sealed body-pipeline factory matching its
    /// authenticated startup authority, including
    /// [`Self::assemble_storage_only_with_body_pipeline_startup`] and
    /// [`Self::assemble_storage_only_with_recovered_wal_sign_and_body_pipeline_startup`].
    /// This raw candidate surface deliberately does not exist outside test builds.
    #[cfg(test)]
    pub(super) fn from_authenticated_parts(
        authenticated_ledger: LifecycleLedgerV1,
        candidates: impl IntoIterator<Item = CandidateAdmission>,
        validate_no_successor: impl IntoIterator<
            Item = (CandidateAdmission, DurableBodyValidationOutcome),
        >,
        serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
    ) -> Option<Self> {
        let context = authenticated_ledger.context();
        if digest_bytes(serve_payloads.context_id().0.as_ref()) != context.id()
            || serve_payloads.height() != context.height()
        {
            return None;
        }
        let mut candidate_map = BTreeMap::new();
        for candidate in candidates {
            if matches!(
                candidate.work_class,
                LifecycleWorkClass::CertifiedServe | LifecycleWorkClass::ProducerTurn
            ) || candidate_map.insert(candidate.key, candidate).is_some()
            {
                return None;
            }
        }
        let mut validate_no_successor_map = BTreeMap::new();
        for (candidate, outcome) in validate_no_successor {
            if candidate_map.contains_key(&candidate.key)
                || !super::projection::recovered_validate_no_successor_is_authenticated(
                    context, &candidate, &outcome,
                )
            {
                return None;
            }
            let authenticated = AuthenticatedValidateNoSuccessorRecovery {
                key: candidate.key,
                causal_root: candidate.causal_root,
                reconstruction_source: candidate.reconstruction_source,
                stage: candidate.stage,
                payload: candidate.payload,
            };
            if validate_no_successor_map
                .insert(candidate.key, authenticated)
                .is_some()
            {
                return None;
            }
        }
        Some(Self {
            context,
            authenticated_ledger,
            candidates: candidate_map,
            validate_no_successor: validate_no_successor_map,
            released_validate: None,
            lifecycle_outputs: None,
            serve_payloads,
        })
    }
    // STORAGE_ONLY_LIFECYCLE_RECOVERY_ASSEMBLER_BEGIN
    /// Assemble the sole ordinary body-pipeline production startup recovery cut.
    ///
    /// The opaque Fetch phase moves every logical candidate directly into this
    /// recovery value while retaining every concrete completion for the
    /// subsequent empty-registry install. Terminal Validate outcomes are
    /// consumed from the same owned body-store instance. Any other live
    /// ordinary class remains unsupported and fails closed.
    #[allow(clippy::result_large_err)]
    pub(super) fn assemble_storage_only_with_body_pipeline_startup(
        ledger: LifecycleLedgerV1,
        serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
        body_store: &mut V2BodyStore,
        mut body_pipeline: PreparedDurableCertifiedBodyPipelineStartupV1,
    ) -> Result<(Self, PreparedDurableCertifiedBodyPipelineStartupV1), LifecycleRecoveryAssemblyError>
    {
        let recovery = Self::assemble_storage_only_with_terminal_validate_outcomes(
            ledger,
            serve_payloads,
            body_store,
            RecoveredWalStartupProjectionV1::None,
            Some(&mut body_pipeline),
        )?;
        Ok((recovery, body_pipeline))
    }
    /// Assemble the final repaired-WAL Sign, every ordinary durable body row, and
    /// all terminal Validate outcomes from one exact post-repair frame.
    ///
    /// The installed Sign projection remains borrowed, while the Fetch phase
    /// is consumed only after its complete frame-bound census is spliced. This
    /// is the sole storage assembler used by the unified recovered-vote
    /// production startup.
    #[allow(clippy::result_large_err)]
    pub(super) fn assemble_storage_only_with_recovered_wal_sign_and_body_pipeline_startup(
        ledger: LifecycleLedgerV1,
        serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
        body_store: &mut V2BodyStore,
        projection: &AuthenticatedRecoveredWalSignProjection,
        mut body_pipeline: PreparedDurableCertifiedBodyPipelineStartupV1,
    ) -> Result<(Self, PreparedDurableCertifiedBodyPipelineStartupV1), LifecycleRecoveryAssemblyError>
    {
        let recovery = Self::assemble_storage_only_with_terminal_validate_outcomes(
            ledger,
            serve_payloads,
            body_store,
            RecoveredWalStartupProjectionV1::PhaseVote(projection),
            Some(&mut body_pipeline),
        )?;
        Ok((recovery, body_pipeline))
    }
    /// Assemble an exact recovered Validate→Sign→Broadcast phase-vote cut.
    #[allow(clippy::result_large_err)]
    pub(super) fn assemble_storage_only_with_recovered_phase_broadcast_and_body_pipeline_startup(
        ledger: LifecycleLedgerV1,
        serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
        body_store: &mut V2BodyStore,
        projection: &AuthenticatedRecoveredWalSignProjection,
        broadcast: &super::wal_recovery::RecoveredLifecycleSignedBroadcastProjectionV1,
        mut body_pipeline: PreparedDurableCertifiedBodyPipelineStartupV1,
    ) -> Result<(Self, PreparedDurableCertifiedBodyPipelineStartupV1), LifecycleRecoveryAssemblyError>
    {
        let recovery = Self::assemble_storage_only_with_terminal_validate_outcomes(
            ledger,
            serve_payloads,
            body_store,
            RecoveredWalStartupProjectionV1::PhaseBroadcast(projection, broadcast),
            Some(&mut body_pipeline),
        )?;
        Ok((recovery, body_pipeline))
    }
    /// Assemble the same phase pair after executable ownership has split into
    /// its two cold registry carriers.
    #[allow(clippy::result_large_err)]
    pub(super) fn assemble_storage_only_with_recovered_phase_broadcast_and_next_sign_and_body_pipeline_startup(
        ledger: LifecycleLedgerV1,
        serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
        body_store: &mut V2BodyStore,
        projection: &AuthenticatedRecoveredWalSignProjection,
        pair: &RecoveredLifecycleSignedBroadcastAndSignLedgerProjectionV1,
        broadcast: &super::wal_recovery::RecoveredLifecycleSignedBroadcastProjectionV1,
        next_sign: &RecoveredLifecycleNextWalVoteCandidateProjectionV1,
        mut body_pipeline: PreparedDurableCertifiedBodyPipelineStartupV1,
    ) -> Result<(Self, PreparedDurableCertifiedBodyPipelineStartupV1), LifecycleRecoveryAssemblyError>
    {
        let recovery = Self::assemble_storage_only_with_terminal_validate_outcomes(
            ledger,
            serve_payloads,
            body_store,
            RecoveredWalStartupProjectionV1::PhaseBroadcastAndNextSign(
                projection, pair, broadcast, next_sign,
            ),
            Some(&mut body_pipeline),
        )?;
        Ok((recovery, body_pipeline))
    }
    /// Assemble the exact standalone control Sign with every durable Fetch.
    ///
    /// The exclusive startup-projection enum makes a phase-vote/control pair
    /// unrepresentable. Only the control projection's exact live row may cross
    /// the ordinary-row fail-closed classifier.
    #[allow(clippy::result_large_err)]
    pub(super) fn assemble_storage_only_with_recovered_wal_control_sign_and_body_pipeline_startup(
        ledger: LifecycleLedgerV1,
        serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
        body_store: &mut V2BodyStore,
        projection: &AuthenticatedRecoveredWalControlProjection,
        mut body_pipeline: PreparedDurableCertifiedBodyPipelineStartupV1,
    ) -> Result<(Self, PreparedDurableCertifiedBodyPipelineStartupV1), LifecycleRecoveryAssemblyError>
    {
        let recovery = Self::assemble_storage_only_with_terminal_validate_outcomes(
            ledger,
            serve_payloads,
            body_store,
            RecoveredWalStartupProjectionV1::ControlSign(projection),
            Some(&mut body_pipeline),
        )?;
        Ok((recovery, body_pipeline))
    }
    /// Assemble an exact Advanced control Sign with its sole live Broadcast.
    #[allow(clippy::result_large_err)]
    pub(super) fn assemble_storage_only_with_recovered_control_broadcast_and_body_pipeline_startup(
        ledger: LifecycleLedgerV1,
        serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
        body_store: &mut V2BodyStore,
        control: &AuthenticatedRecoveredWalControlProjection,
        broadcast: &super::wal_recovery::RecoveredLifecycleSignedBroadcastProjectionV1,
        mut body_pipeline: PreparedDurableCertifiedBodyPipelineStartupV1,
    ) -> Result<(Self, PreparedDurableCertifiedBodyPipelineStartupV1), LifecycleRecoveryAssemblyError>
    {
        let recovery = Self::assemble_storage_only_with_terminal_validate_outcomes(
            ledger,
            serve_payloads,
            body_store,
            RecoveredWalStartupProjectionV1::ControlBroadcast(control, broadcast),
            Some(&mut body_pipeline),
        )?;
        Ok((recovery, body_pipeline))
    }
    /// Assemble an exact control-Proposal Broadcast plus follow-on WAL Vote Sign.
    ///
    /// The frame-bound pair and executable combined projection are borrowed as
    /// one exclusive WAL startup class. Both live child candidates are spliced
    /// before the all-row census runs, so neither can survive a partial
    /// admission and unrelated durable Fetch owners remain independently
    /// authenticated.
    #[cfg_attr(not(test), allow(dead_code))]
    #[allow(clippy::result_large_err)]
    pub(super) fn assemble_storage_only_with_recovered_control_broadcast_and_sign_and_body_pipeline_startup(
        ledger: LifecycleLedgerV1,
        serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
        body_store: &mut V2BodyStore,
        control: &AuthenticatedRecoveredWalControlProjection,
        pair: &RecoveredLifecycleSignedBroadcastAndSignLedgerProjectionV1,
        combined: &RecoveredLifecycleSignedBroadcastAndSignProjectionV1,
        mut body_pipeline: PreparedDurableCertifiedBodyPipelineStartupV1,
    ) -> Result<(Self, PreparedDurableCertifiedBodyPipelineStartupV1), LifecycleRecoveryAssemblyError>
    {
        let recovery = Self::assemble_storage_only_with_terminal_validate_outcomes(
            ledger,
            serve_payloads,
            body_store,
            RecoveredWalStartupProjectionV1::ControlBroadcastAndSign(control, pair, combined),
            Some(&mut body_pipeline),
        )?;
        Ok((recovery, body_pipeline))
    }
    /// Assemble the standalone Decision Fetch with every durable body-backed Fetch.
    ///
    /// The exclusive startup enum prevents coexistence with a phase vote or
    /// control Sign. Only this exact WAL-owned, payload-free Fetch row may
    /// cross the ordinary-row fail-closed classifier.
    #[allow(clippy::result_large_err)]
    pub(super) fn assemble_storage_only_with_recovered_wal_decision_fetch_and_body_pipeline_startup(
        ledger: LifecycleLedgerV1,
        serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
        body_store: &mut V2BodyStore,
        projection: &AuthenticatedRecoveredWalDecisionFetchProjection,
        mut body_pipeline: PreparedDurableCertifiedBodyPipelineStartupV1,
    ) -> Result<(Self, PreparedDurableCertifiedBodyPipelineStartupV1), LifecycleRecoveryAssemblyError>
    {
        let recovery = Self::assemble_storage_only_with_terminal_validate_outcomes(
            ledger,
            serve_payloads,
            body_store,
            RecoveredWalStartupProjectionV1::DecisionFetch(projection),
            Some(&mut body_pipeline),
        )?;
        Ok((recovery, body_pipeline))
    }
    /// Assemble an advanced recovered WAL Fetch with its sole live Store child.
    #[allow(clippy::result_large_err)]
    pub(super) fn assemble_storage_only_with_recovered_decision_store_and_body_pipeline_startup(
        ledger: LifecycleLedgerV1,
        serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
        body_store: &mut V2BodyStore,
        fetch: &AuthenticatedRecoveredWalDecisionFetchProjection,
        store: &RecoveredDecisionFetchStoreProjectionV1,
        mut body_pipeline: PreparedDurableCertifiedBodyPipelineStartupV1,
    ) -> Result<(Self, PreparedDurableCertifiedBodyPipelineStartupV1), LifecycleRecoveryAssemblyError>
    {
        let recovery = Self::assemble_storage_only_with_terminal_validate_outcomes(
            ledger,
            serve_payloads,
            body_store,
            RecoveredWalStartupProjectionV1::DecisionStore(fetch, store),
            Some(&mut body_pipeline),
        )?;
        Ok((recovery, body_pipeline))
    }
    /// Assemble an advanced recovered Decision Store with its sole live Validate child.
    #[allow(clippy::result_large_err)]
    pub(super) fn assemble_storage_only_with_recovered_decision_validate_and_body_pipeline_startup(
        ledger: LifecycleLedgerV1,
        serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
        body_store: &mut V2BodyStore,
        fetch: &AuthenticatedRecoveredWalDecisionFetchProjection,
        store: &RecoveredDecisionFetchStoreProjectionV1,
        validate: &RecoveredDecisionValidateProjectionV1,
        mut body_pipeline: PreparedDurableCertifiedBodyPipelineStartupV1,
    ) -> Result<(Self, PreparedDurableCertifiedBodyPipelineStartupV1), LifecycleRecoveryAssemblyError>
    {
        let recovery = Self::assemble_storage_only_with_terminal_validate_outcomes(
            ledger,
            serve_payloads,
            body_store,
            RecoveredWalStartupProjectionV1::DecisionValidate(fetch, store, validate),
            Some(&mut body_pipeline),
        )?;
        Ok((recovery, body_pipeline))
    }
    /// Assemble one exact recovered Decision body chain with every unrelated
    /// ordinary durable body-pipeline row.
    ///
    /// The projection must name an already-terminal Fetch/Store/Validate
    /// prefix and the sole live Apply successor. It is borrowed while the
    /// candidate is spliced, so the dedicated registry carrier remains owned
    /// by the caller for the later atomic install.
    #[allow(clippy::result_large_err)]
    pub(super) fn assemble_storage_only_with_recovered_decision_apply_and_body_pipeline_startup(
        ledger: LifecycleLedgerV1,
        serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
        body_store: &mut V2BodyStore,
        projection: &crate::sumeragi::v2::RecoveredDecisionApplyStagedStorageV1,
        mut body_pipeline: PreparedDurableCertifiedBodyPipelineStartupV1,
    ) -> Result<(Self, PreparedDurableCertifiedBodyPipelineStartupV1), LifecycleRecoveryAssemblyError>
    {
        let recovery = Self::assemble_storage_only_with_terminal_validate_outcomes(
            ledger,
            serve_payloads,
            body_store,
            RecoveredWalStartupProjectionV1::DecisionApply(projection),
            Some(&mut body_pipeline),
        )?;
        Ok((recovery, body_pipeline))
    }
    /// Assemble one exact standalone recovered Apply beside an older
    /// storage-authenticated successful Validate tombstone.
    #[allow(clippy::result_large_err)]
    pub(super) fn assemble_storage_only_with_recovered_released_decision_apply_and_body_pipeline_startup(
        ledger: LifecycleLedgerV1,
        serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
        body_store: &mut V2BodyStore,
        projection: &crate::sumeragi::v2::RecoveredDecisionApplyStagedStorageV1,
        mut body_pipeline: PreparedDurableCertifiedBodyPipelineStartupV1,
    ) -> Result<(Self, PreparedDurableCertifiedBodyPipelineStartupV1), LifecycleRecoveryAssemblyError>
    {
        let recovery = Self::assemble_storage_only_with_terminal_validate_outcomes(
            ledger,
            serve_payloads,
            body_store,
            RecoveredWalStartupProjectionV1::DecisionReleasedApply(projection),
            Some(&mut body_pipeline),
        )?;
        Ok((recovery, body_pipeline))
    }
    #[allow(clippy::result_large_err)]
    fn assemble_storage_only_with_terminal_validate_outcomes(
        ledger: LifecycleLedgerV1,
        serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
        body_store: &mut V2BodyStore,
        recovered_wal: RecoveredWalStartupProjectionV1<'_>,
        body_pipeline: Option<&mut PreparedDurableCertifiedBodyPipelineStartupV1>,
    ) -> Result<Self, LifecycleRecoveryAssemblyError> {
        let (candidates, claims, lifecycle_outputs) =
            match assemble_storage_only_candidates_and_terminal_validate_claims(
                &ledger,
                &serve_payloads,
                recovered_wal,
                body_pipeline,
            ) {
                Ok(assembled) => assembled,
                Err(kind) => {
                    return Err(LifecycleRecoveryAssemblyError {
                        kind,
                        _authenticated_ledger: ledger,
                        _serve_payloads: serve_payloads,
                    });
                }
            };
        let released_claim = match recovered_wal {
            RecoveredWalStartupProjectionV1::DecisionReleasedApply(projection) => {
                let mut matching = claims.values().copied().filter(|claim| {
                    claim.matches_validated_receipt(projection.validated_receipt())
                });
                let Some(claim) = matching.next() else {
                    return Err(LifecycleRecoveryAssemblyError {
                        kind: LifecycleRecoveryAssemblyErrorKind::RecoveredWalSign(
                            "released recovered Apply has no exact successful Validate claim",
                        ),
                        _authenticated_ledger: ledger,
                        _serve_payloads: serve_payloads,
                    });
                };
                if matching.next().is_some() {
                    return Err(LifecycleRecoveryAssemblyError {
                        kind: LifecycleRecoveryAssemblyErrorKind::RecoveredWalSign(
                            "released recovered Apply names multiple successful Validate claims",
                        ),
                        _authenticated_ledger: ledger,
                        _serve_payloads: serve_payloads,
                    });
                }
                Some(claim)
            }
            _ => None,
        };
        let needs_invalid_body_markers = lifecycle_outputs.invalid_body_reports().next().is_some();
        if claims.is_empty() && !needs_invalid_body_markers {
            return Ok(Self {
                context: ledger.context(),
                authenticated_ledger: ledger,
                candidates,
                validate_no_successor: BTreeMap::new(),
                released_validate: None,
                lifecycle_outputs: (!lifecycle_outputs.is_empty()).then_some(lifecycle_outputs),
                serve_payloads,
            });
        }
        let mut catalog = match body_store.detach_terminal_validate_outcome_catalog() {
            Ok(catalog) => catalog,
            Err(error) => {
                let detail = match error {
                    RecoveredTerminalValidateOutcomeCatalogError::UnrevalidatedMarkers => {
                        "durable markers have not completed semantic replay"
                    }
                    RecoveredTerminalValidateOutcomeCatalogError::AmbiguousOutcome => {
                        "one proposal is present in both closed outcome maps"
                    }
                };
                return Err(LifecycleRecoveryAssemblyError {
                    kind: LifecycleRecoveryAssemblyErrorKind::TerminalValidateOutcomeCatalog(
                        detail,
                    ),
                    _authenticated_ledger: ledger,
                    _serve_payloads: serve_payloads,
                });
            }
        };
        for claim in claims.values() {
            let selected = if released_claim == Some(*claim) {
                catalog.select_exact_successful_terminal_validate(claim)
            } else {
                catalog.select_exact_terminal_validate(claim)
            };
            if !selected {
                return Err(LifecycleRecoveryAssemblyError {
                    kind: LifecycleRecoveryAssemblyErrorKind::MissingTerminalValidateOutcome {
                        ordinal: claim.ordinal,
                        stage: claim.stage,
                    },
                    _authenticated_ledger: ledger,
                    _serve_payloads: serve_payloads,
                });
            }
        }
        for report in lifecycle_outputs.invalid_body_reports() {
            if !catalog.select_exact_invalid_body_report(report) {
                return Err(LifecycleRecoveryAssemblyError {
                    kind: LifecycleRecoveryAssemblyErrorKind::InvalidLifecycleOutputRecovery {
                        ordinal: report.ordinal(),
                        work_class: report.candidate().work_class,
                        stage: report.candidate().stage,
                    },
                    _authenticated_ledger: ledger,
                    _serve_payloads: serve_payloads,
                });
            }
        }
        let validate_no_successor = claims
            .values()
            .copied()
            .map(|claim| (claim.key, claim.into_authenticated()))
            .collect();
        let released_validate = if let Some(claim) = released_claim {
            let Some(released) = catalog.commit_selected_with_released_validate(claim) else {
                return Err(LifecycleRecoveryAssemblyError {
                    kind: LifecycleRecoveryAssemblyErrorKind::RecoveredWalSign(
                        "released recovered Apply lost its selected successful Validate",
                    ),
                    _authenticated_ledger: ledger,
                    _serve_payloads: serve_payloads,
                });
            };
            Some(released)
        } else {
            catalog.commit_selected();
            None
        };
        let recovery = Self {
            context: ledger.context(),
            authenticated_ledger: ledger,
            candidates,
            validate_no_successor,
            released_validate,
            lifecycle_outputs: (!lifecycle_outputs.is_empty()).then_some(lifecycle_outputs),
            serve_payloads,
        };
        Ok(recovery)
    }
    // STORAGE_ONLY_LIFECYCLE_RECOVERY_ASSEMBLER_END
    fn authenticates_opened_ledger(&self, opened: &LifecycleLedgerV1) -> bool {
        &self.authenticated_ledger == opened
    }
    /// Assemble the empty logical side of a focused recovered-WAL fixture from
    /// a real authenticated payload-store cut.
    #[cfg(test)]
    pub(in crate::sumeragi) fn empty_for_recovered_wal_test(
        verified: &VerifiedHeightContext,
        authenticated_ledger: LifecycleLedgerV1,
        serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
    ) -> Option<Self> {
        if authenticated_ledger.context()
            != super::projection::lifecycle_context(verified.context())
        {
            return None;
        }
        Self::from_authenticated_parts(authenticated_ledger, [], [], serve_payloads)
    }
    /// Open the exact ledger frame and assemble an empty recovered-WAL test cut.
    #[cfg(test)]
    pub(crate) fn open_empty_for_recovered_wal_test(
        verified: &VerifiedHeightContext,
        ledger_root: &Path,
        serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
    ) -> Result<Self, LifecycleOpenError> {
        let context = super::projection::lifecycle_context(verified.context());
        let (_ledger_store, ledger) = LifecycleLedgerStoreV1::open(ledger_root, context)?;
        Self::empty_for_recovered_wal_test(verified, ledger, serve_payloads).ok_or_else(|| {
            LifecycleOpenErrorKind::InvalidRecovery("empty recovered-WAL test cut is inconsistent")
                .into()
        })
    }
    // RECOVERED_WAL_SIGN_RECOVERY_SPLICE_BEGIN
    /// Replace one exact recovered Validate parent by its authenticated WAL
    /// Sign successor, or accept an already-repaired exact child.
    ///
    /// The comparison is complete before mutation. A recovery cut with neither
    /// exact side, both sides, a foreign context, or a terminal no-successor
    /// claim therefore stays byte-for-byte unchanged. The caller retains the
    /// closed concrete registry row which authenticated both candidates; this
    /// method never exposes either candidate outside the sealed startup path.
    pub(super) fn splice_recovered_wal_sign(
        &mut self,
        projection: &AuthenticatedRecoveredWalSignProjection,
    ) -> bool {
        if !projection.belongs_to_context(self.context)
            || self
                .validate_no_successor
                .contains_key(&projection.parent_key())
            || self
                .validate_no_successor
                .contains_key(&projection.child_key())
        {
            return false;
        }
        projection.splice_candidates(&mut self.candidates)
    }
    /// Revalidate the post-splice recovery ownership without exposing either
    /// retained candidate.
    pub(super) fn owns_recovered_wal_sign(
        &self,
        projection: &AuthenticatedRecoveredWalSignProjection,
    ) -> bool {
        projection.belongs_to_context(self.context)
            && !self
                .validate_no_successor
                .contains_key(&projection.parent_key())
            && !self
                .validate_no_successor
                .contains_key(&projection.child_key())
            && projection.owns_spliced_candidates(&self.candidates)
    }
    /// Revalidate the exact live Broadcast beneath one recovered phase vote.
    pub(super) fn owns_recovered_phase_broadcast(
        &self,
        projection: &AuthenticatedRecoveredWalSignProjection,
        broadcast: &super::wal_recovery::RecoveredLifecycleSignedBroadcastProjectionV1,
    ) -> bool {
        projection.belongs_to_context(self.context)
            && broadcast.owns_spliced_candidate(&self.candidates)
            && projection.signed_broadcast_chain_is_exact(
                self.context,
                self.authenticated_ledger.records(),
                broadcast,
            )
    }
    /// Revalidate the split phase Broadcast and next-Sign carriers.
    pub(super) fn owns_recovered_phase_broadcast_and_next_sign(
        &self,
        pair: &RecoveredLifecycleSignedBroadcastAndSignLedgerProjectionV1,
        broadcast: &super::wal_recovery::RecoveredLifecycleSignedBroadcastProjectionV1,
        next_sign: &RecoveredLifecycleNextWalVoteCandidateProjectionV1,
    ) -> bool {
        matches!(
            pair.parent(),
            super::ledger::RecoveredLifecycleSignedBroadcastAndSignParentV1::PhasePrepare { .. }
        ) && pair.exactly_matches_ledger(&self.authenticated_ledger)
            && broadcast.owns_spliced_candidate(&self.candidates)
            && next_sign.owns_spliced_candidate(&self.candidates)
    }
    /// Revalidate the exact standalone control Sign retained by recovery.
    pub(super) fn owns_recovered_wal_control_sign(
        &self,
        projection: &AuthenticatedRecoveredWalControlProjection,
    ) -> bool {
        projection.belongs_to_context(self.context)
            && projection.owns_spliced_candidate(&self.candidates)
    }
    /// Revalidate the exact live Broadcast retained beneath its control WAL parent.
    pub(super) fn owns_recovered_control_broadcast(
        &self,
        control: &AuthenticatedRecoveredWalControlProjection,
        broadcast: &super::wal_recovery::RecoveredLifecycleSignedBroadcastProjectionV1,
    ) -> bool {
        control.belongs_to_context(self.context)
            && broadcast.owns_spliced_candidate(&self.candidates)
    }
    /// Revalidate both already-split carriers of one control-owned durable pair.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn owns_recovered_control_broadcast_and_next_sign(
        &self,
        pair: &RecoveredLifecycleSignedBroadcastAndSignLedgerProjectionV1,
        broadcast: &super::wal_recovery::RecoveredLifecycleSignedBroadcastProjectionV1,
        next_sign: &RecoveredLifecycleNextWalVoteCandidateProjectionV1,
    ) -> bool {
        pair.exactly_matches_ledger(&self.authenticated_ledger)
            && broadcast.owns_spliced_candidate(&self.candidates)
            && next_sign.owns_spliced_candidate(&self.candidates)
    }
    /// Revalidate the exact standalone Decision Fetch retained by recovery.
    pub(super) fn owns_recovered_wal_decision_fetch(
        &self,
        projection: &AuthenticatedRecoveredWalDecisionFetchProjection,
    ) -> bool {
        projection.belongs_to_context(self.context)
            && projection.owns_spliced_candidate(&self.candidates)
    }
    /// Revalidate the exact recovered Store child retained by cold recovery.
    pub(super) fn owns_recovered_decision_store(
        &self,
        fetch: &AuthenticatedRecoveredWalDecisionFetchProjection,
        store: &RecoveredDecisionFetchStoreProjectionV1,
    ) -> bool {
        fetch.belongs_to_context(self.context)
            && store.context() == self.context
            && store.owns_spliced_candidate(&self.candidates)
    }
    /// Revalidate the exact recovered Validate child retained by cold recovery.
    pub(super) fn owns_recovered_decision_validate(
        &self,
        seal: &RecoveredDecisionValidateInstalledSealV1,
    ) -> bool {
        seal.context() == self.context
            && seal
                .live_validate_record(&self.authenticated_ledger)
                .is_some()
            && seal.owns_spliced_candidate(&self.candidates)
    }
    /// Seed the opaque installed projection's exact Validate parent.
    #[cfg(test)]
    pub(super) fn seed_recovered_wal_parent_for_test(
        &mut self,
        projection: &AuthenticatedRecoveredWalSignProjection,
    ) -> bool {
        projection.belongs_to_context(self.context)
            && self.candidates.is_empty()
            && self.validate_no_successor.is_empty()
            && projection.seed_parent_candidate_for_test(&mut self.candidates)
    }
    /// Seed the opaque installed projection's exact Sign child.
    #[cfg(test)]
    pub(super) fn seed_recovered_wal_child_for_test(
        &mut self,
        projection: &AuthenticatedRecoveredWalSignProjection,
    ) -> bool {
        projection.belongs_to_context(self.context)
            && self.candidates.is_empty()
            && self.validate_no_successor.is_empty()
            && projection.seed_child_candidate_for_test(&mut self.candidates)
    }
    /// Seed both opaque projection sides for an ambiguity-preservation test.
    #[cfg(test)]
    pub(super) fn seed_both_recovered_wal_candidates_for_test(
        &mut self,
        projection: &AuthenticatedRecoveredWalSignProjection,
    ) -> bool {
        projection.belongs_to_context(self.context)
            && self.candidates.is_empty()
            && self.validate_no_successor.is_empty()
            && projection.seed_both_candidates_for_test(&mut self.candidates)
    }
    // RECOVERED_WAL_SIGN_RECOVERY_SPLICE_END
}
/// Owned failure from the storage-only recovery-cut assembler.
///
/// The exact opened LedgerV1 frame and the move-only authenticated Serve cut
/// remain sealed here on every failure. A caller may therefore fail-stop
/// without discarding either durable authority or accidentally retrying from a
/// different frame.
#[derive(Debug, Error)]
#[error("{kind}")]
#[must_use = "failed lifecycle recovery assembly still owns durable authority"]
pub(crate) struct LifecycleRecoveryAssemblyError {
    #[source]
    kind: LifecycleRecoveryAssemblyErrorKind,
    _authenticated_ledger: LifecycleLedgerV1,
    _serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
}
impl LifecycleRecoveryAssemblyError {
    /// Borrow the typed, non-authorizing diagnostic.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const fn kind(&self) -> &LifecycleRecoveryAssemblyErrorKind {
        &self.kind
    }
}
/// Exhaustive reason why durable storage could not form one recovery cut.
#[derive(Debug, Error)]
pub(crate) enum LifecycleRecoveryAssemblyErrorKind {
    /// One live ordinary row has no exact durable carrier reconstruction.
    #[error(
        "live lifecycle ordinal {ordinal} ({work_class:?}, {stage:?}) has no durable recovery authority"
    )]
    MissingDurableRecoveryAuthority {
        /// Immutable ledger ordinal of the unsupported live row.
        ordinal: u128,
        /// Exhaustive logical work class retained by LedgerV1.
        work_class: LifecycleWorkClass,
        /// Exact immutable execution stage retained by LedgerV1.
        stage: LifecycleStage,
    },
    /// One persisted output source, signature, predecessor, or marker did not authenticate.
    #[error(
        "live lifecycle output ordinal {ordinal} ({work_class:?}, {stage:?}) failed exact cold authentication"
    )]
    InvalidLifecycleOutputRecovery {
        /// Immutable ledger ordinal of the rejected output row.
        ordinal: u128,
        /// Closed logical output class retained by LedgerV1.
        work_class: LifecycleWorkClass,
        /// Exact immutable execution stage retained by LedgerV1.
        stage: LifecycleStage,
    },
    /// A terminal Validate/no-child tombstone lost its consumed body outcome.
    #[error("terminal Validate ordinal {ordinal} ({stage:?}) has no authenticated body outcome")]
    MissingTerminalValidateOutcome {
        /// Immutable ledger ordinal of the no-successor tombstone.
        ordinal: u128,
        /// Exact immutable Validate stage retained by LedgerV1.
        stage: LifecycleStage,
    },
    /// The body-store recovery catalog is not ready for exact terminal coverage.
    #[error("terminal Validate body-outcome catalog is unavailable: {0}")]
    TerminalValidateOutcomeCatalog(&'static str),
    /// A checksummed record could not be decoded into the closed schema.
    #[error("lifecycle ordinal {ordinal} has invalid durable {field}")]
    InvalidDurableRecord {
        /// Immutable ledger ordinal of the malformed row.
        ordinal: u128,
        /// Closed field name which failed typed decoding.
        field: &'static str,
    },
    /// Work class and execution stage no longer form one closed schema pair.
    #[error(
        "lifecycle ordinal {ordinal} has inconsistent durable class/stage ({work_class:?}, {stage:?})"
    )]
    InvalidDurableRecordShape {
        /// Immutable ledger ordinal of the malformed row.
        ordinal: u128,
        /// Decoded logical work class.
        work_class: LifecycleWorkClass,
        /// Decoded immutable execution stage.
        stage: LifecycleStage,
    },
    /// The opaque installed recovered-WAL projection and repaired frame differ.
    #[error("recovered-WAL Sign storage recovery is incomplete: {0}")]
    RecoveredWalSign(&'static str),
    /// The complete recovered body-pipeline census differs from the ledger.
    #[error("durable body-pipeline startup census is inconsistent: {0}")]
    DurableCertifiedBodyPipeline(&'static str),
    /// The authenticated Serve cut did not resolve the ledger exactly.
    #[error("Certified-Serve storage recovery is incomplete: {0}")]
    CertifiedServe(#[source] LifecycleOpenError),
}
/// Failure to open the sole durable lifecycle authority for one height.
#[derive(Debug, Error)]
#[error("{0}")]
pub(crate) struct LifecycleOpenError(LifecycleOpenErrorKind);
#[derive(Debug, Error)]
enum LifecycleOpenErrorKind {
    #[error("authenticated lifecycle recovery cut is inconsistent: {0}")]
    InvalidRecovery(&'static str),
    #[error(transparent)]
    Ledger(#[from] LifecycleLedgerError),
    #[error(transparent)]
    PayloadStore(#[from] CertifiedServePayloadStoreError),
}
impl From<LifecycleOpenErrorKind> for LifecycleOpenError {
    fn from(error: LifecycleOpenErrorKind) -> Self {
        Self(error)
    }
}
impl From<LifecycleLedgerError> for LifecycleOpenError {
    fn from(error: LifecycleLedgerError) -> Self {
        Self(LifecycleOpenErrorKind::Ledger(error))
    }
}
impl From<CertifiedServePayloadStoreError> for LifecycleOpenError {
    fn from(error: CertifiedServePayloadStoreError) -> Self {
        Self(LifecycleOpenErrorKind::PayloadStore(error))
    }
}
/// In-memory durable-open result before either local store is published.
///
/// The coordinator has completed exhaustive recovery and rebinding, but its
/// LedgerV1 projection and payload-orphan pruning remain uncommitted. This
/// closed stage lets the recovered-WAL registry transaction compare its exact
/// installed Sign row before any durable open publication occurs.
#[must_use = "prepared lifecycle open has not published its durable stores"]
pub(super) struct PreparedLifecycleCoordinatorOpen {
    coordinator: LifecycleCoordinator,
    store: LifecycleLedgerStoreV1,
    persisted_predecessor: LifecycleLedgerV1,
    authenticated_successor: LifecycleLedgerV1,
    retained_serve_payloads: BTreeSet<CertifiedServePayloadId>,
    certified_serve_registry: Option<PreparedCertifiedServeRegistryBatchV1>,
}
/// Fail-stop durable-open commit error retaining the complete prepared state.
#[must_use = "failed lifecycle open still owns its prepared coordinator authority"]
pub(super) struct LifecycleOpenCommitError {
    error: LifecycleOpenError,
    _prepared: PreparedLifecycleCoordinatorOpen,
}
impl LifecycleOpenCommitError {
    pub(super) fn into_error(self) -> LifecycleOpenError {
        self.error
    }
}
impl PreparedLifecycleCoordinatorOpen {
    /// Borrow the completely rebound coordinator before store publication.
    pub(super) const fn coordinator(&self) -> &LifecycleCoordinator {
        &self.coordinator
    }
    /// Borrow the exact opened LedgerV1 store before publication.
    pub(super) const fn store(&self) -> &LifecycleLedgerStoreV1 {
        &self.store
    }
    /// Prune authenticated payload orphans, then publish the exact coordinator
    /// projection, retaining this whole stage on either failure.
    #[allow(clippy::result_large_err)]
    #[cfg(test)]
    pub(super) fn commit(
        mut self,
        payload_store: &mut CertifiedServePayloadStoreV1,
        recovery: &mut AuthenticatedLifecycleRecoveryCut,
    ) -> Result<LifecycleCoordinator, LifecycleOpenCommitError> {
        if let Err(error) = self.publish_durable_open(payload_store, recovery) {
            return Err(LifecycleOpenCommitError {
                error,
                _prepared: self,
            });
        }
        self.coordinator.ledger_store = Some(self.store);
        Ok(self.coordinator)
    }
    /// Atomically install the complete Serve/Producer concrete batch around
    /// LedgerV1 publication. Registry preflight failure changes neither owner;
    /// store publication failure removes the staged carriers before returning
    /// the still-complete prepared open.
    #[allow(clippy::result_large_err)]
    pub(super) fn commit_with_registry(
        mut self,
        registry: &mut ConcreteLifecycleWorkRegistry,
        payload_store: &mut CertifiedServePayloadStoreV1,
        recovery: &mut AuthenticatedLifecycleRecoveryCut,
    ) -> Result<LifecycleCoordinator, LifecycleOpenCommitError> {
        let Some(batch) = self.certified_serve_registry.take() else {
            return Err(LifecycleOpenCommitError {
                error: LifecycleOpenErrorKind::InvalidRecovery(
                    "Certified-Serve concrete batch was already consumed",
                )
                .into(),
                _prepared: self,
            });
        };
        let Some(owner_held_outputs) =
            recovery.exact_lifecycle_output_ordinals_for_registry_census(&self.coordinator)
        else {
            self.certified_serve_registry = Some(batch);
            return Err(LifecycleOpenCommitError {
                error: LifecycleOpenErrorKind::InvalidRecovery(
                    "cold lifecycle output registry census is inconsistent",
                )
                .into(),
                _prepared: self,
            });
        };
        let publication = registry.install_certified_serve_startup_batch_before_publication(
            batch,
            &self.coordinator,
            &owner_held_outputs,
            || self.publish_durable_open(payload_store, recovery),
        );
        match publication {
            Ok(()) => {}
            Err(CertifiedServeRegistryBatchPublicationError::Preflight(batch)) => {
                self.certified_serve_registry = Some(batch);
                return Err(LifecycleOpenCommitError {
                    error: LifecycleOpenErrorKind::InvalidRecovery(
                        "Certified-Serve concrete registry preflight failed",
                    )
                    .into(),
                    _prepared: self,
                });
            }
            Err(CertifiedServeRegistryBatchPublicationError::Publication(error, batch)) => {
                self.certified_serve_registry = Some(batch);
                return Err(LifecycleOpenCommitError {
                    error,
                    _prepared: self,
                });
            }
        }
        self.coordinator.ledger_store = Some(self.store);
        Ok(self.coordinator)
    }
    fn publish_durable_open(
        &self,
        payload_store: &mut CertifiedServePayloadStoreV1,
        recovery: &mut AuthenticatedLifecycleRecoveryCut,
    ) -> Result<(), LifecycleOpenError> {
        // Exact recovery stutters validate the attached frame without replacing it;
        // payload-orphan pruning still runs because it authenticates a separate store.
        let projection = match LifecycleLedgerV1::from_coordinator(&self.coordinator) {
            Ok(projection) => projection,
            Err(error) => return Err(error.into()),
        };
        if projection != self.authenticated_successor {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "prepared coordinator changed its authenticated LedgerV1 successor",
            )
            .into());
        }
        // Orphans are authenticated as absent from both the retained recovery
        // frame and this exact successor. Remove them before advancing the
        // ledger so every fallible filesystem operation precedes the sole
        // logical publication fsync. A partial prune can only remove unowned
        // Pending files and is safely repeated after restart.
        payload_store.prune_authenticated_orphans(
            &mut recovery.serve_payloads,
            &self.retained_serve_payloads,
        )?;
        if let Err(error) = self
            .store
            .persist_exact_successor(&self.persisted_predecessor, &projection)
        {
            return Err(error.into());
        }
        Ok(())
    }
}
impl LifecycleCoordinator {
    /// Rebuild records after seeding the ordinal high-water mark.
    pub(super) fn reconcile_restart_inner(&mut self, snapshot: RecoverySnapshot) {
        let pristine = self.fault.is_none()
            && self.ledger_store.is_none()
            && self.active_lease.is_none()
            && self.records.is_empty()
            && self.key_index.is_empty()
            && self.owner_index.is_empty()
            && self.ready_index.is_empty()
            && self.admission_waits.is_empty()
            && self.durable_records.is_empty()
            && self.producer_debts.is_empty()
            && self.observed_generation.is_empty()
            && self.capacity_used.values().all(|used| *used == 0)
            && self.high_water == snapshot.high_water;
        if !pristine {
            if self.fault.is_none() {
                self.fault = Some(CoordinatorFault::RecoveryRejected);
            }
            return;
        }
        let mut rebuilt = Self::new_with_authority(self.episode_authority.clone(), self.high_water);
        let mut rejected = snapshot.context != self.active_context
            || !has_lifecycle_record_capacity(0, snapshot.records.len());
        for recovered in snapshot.records {
            rejected |= recovered.key.context != snapshot.context.id
                || recovered.key.round.height != snapshot.context.height
                || recovered
                    .key
                    .proposal_round
                    .is_some_and(|round| round.height != snapshot.context.height)
                || recovered.ordinal == 0
                || recovered.ordinal > snapshot.high_water
                || recovered.owner.first_admission_ordinal == 0
                || recovered.owner.first_admission_ordinal > recovered.ordinal
                || !recovered
                    .work_class
                    .accepts_stage(recovered.key.phase, recovered.stage)
                || !recovered
                    .payload
                    .matches_terminal(recovered.work_class, recovered.terminal)
                || !recovered.replay_authority_is_exact(snapshot.context)
                || (recovered.work_class == LifecycleWorkClass::CertifiedServe
                    && recovered.key.subject.is_none())
                || rebuilt.key_index.contains_key(&recovered.key)
                || rebuilt.records.contains_key(&recovered.ordinal)
                || recovered.physical_slot_universe.len() > MAX_PHYSICAL_SLOTS_PER_RECORD
                || !rebuilt.episode_authority.admits_slots(
                    recovered.work_class.capacity_class(),
                    &recovered.physical_slot_universe,
                );
            let episode_universe = rebuilt.episode_authority.universe_for(recovered.key);
            rejected |= episode_universe.is_none();
            if rejected {
                break;
            }
            if let Some(known) = rebuilt.owner_index.get(&recovered.owner.causal_root) {
                rejected |= *known != recovered.owner;
            } else {
                rebuilt
                    .owner_index
                    .insert(recovered.owner.causal_root, recovered.owner);
            }
            let state = if let Some(outcome) = recovered.terminal {
                LifecycleState::Terminal(outcome)
            } else if recovered.work_class == LifecycleWorkClass::ProducerTurn {
                let serve_ordinal = snapshot
                    .producer_debts
                    .iter()
                    .find_map(|(serve, producer)| {
                        (*producer == recovered.ordinal).then_some(*serve)
                    });
                let Some(serve_ordinal) = serve_ordinal else {
                    rejected = true;
                    break;
                };
                LifecycleState::Waiting(WaitToken::new(WaitSource::ProducerTurn(serve_ordinal), 0))
            } else {
                LifecycleState::Waiting(WaitToken::new(
                    WaitSource::Recovery(recovered.reconstruction_source),
                    0,
                ))
            };
            if !matches!(state, LifecycleState::Terminal(_)) {
                let class = recovered.work_class.capacity_class();
                let delta = BTreeMap::from([(class, 1)]);
                if rebuilt.first_capacity_wait(&delta).is_some() {
                    rejected = true;
                    break;
                }
                rebuilt.apply_capacity_delta(&delta);
            }
            rebuilt.durable_records.insert(
                recovered.ordinal,
                DurableRecordMetadata::from_recovered(&recovered),
            );
            rebuilt.insert_record(LifecycleRecord {
                key: recovered.key,
                owner: recovered.owner,
                ordinal: recovered.ordinal,
                work_class: recovered.work_class,
                stage: recovered.stage,
                state,
                physical_slots: BTreeMap::new(),
                episode: SchedulerEpisode {
                    universe: episode_universe.expect("validated recovery universe exists"),
                    slot_universe: recovered.physical_slot_universe,
                    consumed_slots: BTreeSet::new(),
                    frozen_predecessors: BTreeSet::new(),
                },
            });
        }
        let recovered_nonterminal: BTreeSet<_> = rebuilt
            .records
            .iter()
            .filter_map(|(ordinal, record)| {
                (!matches!(record.state, LifecycleState::Terminal(_))).then_some(*ordinal)
            })
            .collect();
        for record in rebuilt.records.values_mut() {
            if !matches!(
                record.stage.predecessor_scope,
                PredecessorScope::Independent
            ) {
                record.episode.frozen_predecessors = recovered_nonterminal
                    .range(..record.ordinal)
                    .copied()
                    .collect();
            }
        }
        rebuilt.producer_debts = snapshot.producer_debts;
        let mut continuation_successors = BTreeSet::new();
        rejected |= rebuilt.records.values().any(|record| {
            let metadata = &rebuilt.durable_records[&record.ordinal];
            let terminal = match record.state {
                LifecycleState::Terminal(outcome) => Some(outcome),
                LifecycleState::Waiting(_) | LifecycleState::Ready | LifecycleState::Claimed(_) => {
                    None
                }
            };
            if record.work_class == LifecycleWorkClass::Validate
                && !durable_validate_payload_is_exact(record.key, metadata.payload)
            {
                return true;
            }
            if !metadata.continuation.matches_record(
                record.work_class,
                terminal,
                record.ordinal,
                rebuilt.high_water,
            ) {
                return true;
            }
            let Some((edge, successor)) = metadata.continuation.successor_parts() else {
                return metadata.continuation == DurableContinuation::AdvancedNoSuccessor
                    && (metadata.reconstruction_source != record.owner.causal_root().digest()
                        || !durable_validate_payload_is_exact(record.key, metadata.payload));
            };
            metadata.reconstruction_source != record.owner.causal_root().digest()
                || !continuation_successors.insert(successor)
                || rebuilt.records.get(&successor).is_none_or(|child| {
                    let child_metadata = &rebuilt.durable_records[&successor];
                    let payload_and_replay_are_exact =
                        recovered_decision_body_continuation_is_exact(
                            edge,
                            &metadata.replay_authority,
                            metadata.payload,
                            &child_metadata.replay_authority,
                            child_metadata.payload,
                        )
                        .or_else(|| {
                            signed_broadcast_continuation_is_exact(
                                edge,
                                &metadata.replay_authority,
                                metadata.payload,
                                &child_metadata.replay_authority,
                                child_metadata.payload,
                            )
                        })
                        .unwrap_or_else(|| {
                            durable_continuation_payload_is_exact(
                                edge,
                                metadata.payload,
                                child_metadata.payload,
                            )
                        });
                    child.owner != record.owner
                        || child_metadata.reconstruction_source != metadata.reconstruction_source
                        || !payload_and_replay_are_exact
                        || !durable_continuation_successor_is_exact(
                            edge,
                            record.work_class,
                            record.key,
                            record.stage,
                            child.work_class,
                            child.key,
                            child.stage,
                        )
                })
        });
        rejected |= rebuilt.owner_index.values().any(|owner| {
            rebuilt
                .records
                .get(&owner.first_admission_ordinal)
                .is_none_or(|record| record.owner != *owner)
        });
        let unique_producers: BTreeSet<_> = rebuilt.producer_debts.values().copied().collect();
        rejected |= unique_producers.len() != rebuilt.producer_debts.len();
        rejected |= rebuilt.producer_debts.iter().any(|(serve, producer)| {
            let (Some(serve_record), Some(producer_record)) =
                (rebuilt.records.get(serve), rebuilt.records.get(producer))
            else {
                return true;
            };
            serve.checked_add(1) != Some(*producer)
                || serve_record.work_class != LifecycleWorkClass::CertifiedServe
                || serve_record.stage.kind != LifecycleStageKind::CertifiedServe
                || producer_record.work_class != LifecycleWorkClass::ProducerTurn
                || producer_record.stage.kind != LifecycleStageKind::ProducerTurn
                || !serve_and_producer_keys_match(serve_record.key, producer_record.key)
                || serve_record.owner != producer_record.owner
                || rebuilt
                    .durable_records
                    .get(serve)
                    .is_none_or(|serve_metadata| {
                        rebuilt
                            .durable_records
                            .get(producer)
                            .is_none_or(|producer_metadata| {
                                producer_metadata.reconstruction_source
                                    != serve_metadata.reconstruction_source
                                    || !producer_metadata
                                        .replay_authority
                                        .same_persisted_family(&serve_metadata.replay_authority)
                            })
                    })
                || matches!(producer_record.state, LifecycleState::Terminal(_))
        });
        rejected |= rebuilt
            .records
            .values()
            .any(|record| match record.work_class {
                LifecycleWorkClass::CertifiedServe => record
                    .ordinal
                    .checked_add(1)
                    .and_then(|producer| rebuilt.records.get(&producer))
                    .is_none_or(|producer| {
                        producer.work_class != LifecycleWorkClass::ProducerTurn
                            || producer.stage.kind != LifecycleStageKind::ProducerTurn
                            || !serve_and_producer_keys_match(record.key, producer.key)
                            || producer.owner != record.owner
                            || rebuilt.durable_records[&producer.ordinal].reconstruction_source
                                != rebuilt.durable_records[&record.ordinal].reconstruction_source
                            || !rebuilt.durable_records[&producer.ordinal]
                                .replay_authority
                                .same_persisted_family(
                                    &rebuilt.durable_records[&record.ordinal].replay_authority,
                                )
                            || (record.state
                                == LifecycleState::Terminal(TerminalOutcome::Cancelled)
                                && producer.state
                                    != LifecycleState::Terminal(TerminalOutcome::Cancelled))
                    }),
                LifecycleWorkClass::ProducerTurn => record
                    .ordinal
                    .checked_sub(1)
                    .and_then(|serve| rebuilt.records.get(&serve))
                    .is_none_or(|serve| {
                        serve.work_class != LifecycleWorkClass::CertifiedServe
                            || serve.stage.kind != LifecycleStageKind::CertifiedServe
                            || !serve_and_producer_keys_match(serve.key, record.key)
                            || serve.owner != record.owner
                            || rebuilt.durable_records[&serve.ordinal].reconstruction_source
                                != rebuilt.durable_records[&record.ordinal].reconstruction_source
                            || !rebuilt.durable_records[&serve.ordinal]
                                .replay_authority
                                .same_persisted_family(
                                    &rebuilt.durable_records[&record.ordinal].replay_authority,
                                )
                    }),
                _ => false,
            });
        rejected |= rebuilt.records.values().any(|record| {
            let live = !matches!(record.state, LifecycleState::Terminal(_));
            match record.work_class {
                LifecycleWorkClass::CertifiedServe => {
                    live && !rebuilt.producer_debts.contains_key(&record.ordinal)
                }
                LifecycleWorkClass::ProducerTurn => {
                    let has_debt = rebuilt
                        .producer_debts
                        .values()
                        .any(|producer| *producer == record.ordinal);
                    has_debt != live
                }
                _ => false,
            }
        });
        let debts: Vec<_> = rebuilt
            .producer_debts
            .iter()
            .map(|(serve, producer)| (*serve, *producer))
            .collect();
        for (serve, producer) in debts {
            if rejected
                || rebuilt
                    .records
                    .get(&producer)
                    .is_some_and(|record| matches!(record.state, LifecycleState::Terminal(_)))
            {
                rejected = true;
                break;
            }
            match rebuilt.records[&serve].state {
                LifecycleState::Terminal(TerminalOutcome::Cancelled) => {
                    // Ledger snapshots persist Serve cancellation and producer
                    // cancellation atomically, without an outstanding debt.
                    rejected = true;
                    break;
                }
                LifecycleState::Terminal(_) => rebuilt.make_ready(producer),
                LifecycleState::Waiting(_) | LifecycleState::Ready | LifecycleState::Claimed(_) => {
                }
            }
        }
        if rejected {
            rebuilt.records.clear();
            rebuilt.key_index.clear();
            rebuilt.owner_index.clear();
            rebuilt.ready_index.clear();
            rebuilt.durable_records.clear();
            rebuilt.producer_debts.clear();
            rebuilt
                .capacity_used
                .values_mut()
                .for_each(|used| *used = 0);
            rebuilt.fault = Some(CoordinatorFault::RecoveryRejected);
        }
        *self = rebuilt;
    }

    fn reconcile_store_ahead_terminal_serve(
        &mut self,
        serve_ordinal: u128,
        mut candidate: CandidateAdmission,
        update: TerminalUpdate,
    ) -> Result<(), LifecycleOpenError> {
        if update.ordinal != serve_ordinal
            || !update
                .replay
                .exactly_matches_recovered_candidate(self.active_context, &candidate)
        {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "payload-store-ahead replay family changed its recovered candidate",
            )
            .into());
        }
        let producer_ordinal = self.producer_debts.get(&serve_ordinal).copied().ok_or(
            LifecycleOpenErrorKind::InvalidRecovery(
                "payload-store-ahead Serve has no adjacent producer debt",
            ),
        )?;
        if !update.replay.exactly_advances_pending_records(
            self.active_context,
            self.records
                .get(&serve_ordinal)
                .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                    "payload-store-ahead Serve record disappeared",
                ))?,
            self.durable_records.get(&serve_ordinal).ok_or(
                LifecycleOpenErrorKind::InvalidRecovery(
                    "payload-store-ahead Serve metadata disappeared",
                ),
            )?,
            self.records
                .get(&producer_ordinal)
                .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                    "payload-store-ahead producer record disappeared",
                ))?,
            self.durable_records.get(&producer_ordinal).ok_or(
                LifecycleOpenErrorKind::InvalidRecovery(
                    "payload-store-ahead producer metadata disappeared",
                ),
            )?,
        ) {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "payload-store-ahead frame is not the exact Pending pair successor",
            )
            .into());
        }
        let producer =
            candidate
                .producer_turn
                .as_ref()
                .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                    "payload-store-ahead Serve lost its producer companion",
                ))?;
        self.durable_records
            .get_mut(&serve_ordinal)
            .expect("preflight retained Serve metadata")
            .replay_authority = candidate.replay_authority.clone();
        self.durable_records
            .get_mut(&producer_ordinal)
            .expect("preflight retained ProducerTurn metadata")
            .replay_authority = producer.replay_authority.clone();
        if !matches!(
            self.records[&serve_ordinal].state,
            LifecycleState::Waiting(wait)
                if matches!(wait.source, super::WaitSource::Recovery(_))
        ) || self
            .rebind_recovered_candidate(serve_ordinal, &mut candidate)
            .is_err()
        {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "payload-store-ahead candidate did not rebind exactly",
            )
            .into());
        }
        if !update.replay.exactly_matches_rebound_records(
            self.active_context,
            &self.records[&serve_ordinal],
            &self.durable_records[&serve_ordinal],
            &self.records[&producer_ordinal],
            &self.durable_records[&producer_ordinal],
        ) {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "rebound payload-store-ahead pair changed before settlement",
            )
            .into());
        }
        let expected_payload = update.payload;
        let expected_outcome = update.outcome;
        let (payload, outcome, serve_replay, producer_replay) =
            update.replay.consume_terminal_rebind();
        if payload != expected_payload || outcome != expected_outcome {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "payload-store-ahead terminal projection changed during settlement",
            )
            .into());
        }
        let serve_metadata = self
            .durable_records
            .get_mut(&serve_ordinal)
            .expect("rebound Serve metadata remains present");
        serve_metadata.payload = payload;
        serve_metadata.replay_authority = serve_replay;
        self.durable_records
            .get_mut(&producer_ordinal)
            .expect("rebound ProducerTurn metadata remains present")
            .replay_authority = producer_replay;
        self.finish_terminal(serve_ordinal, outcome).map_err(|_| {
            LifecycleOpenErrorKind::InvalidRecovery(
                "payload-store terminal cut could not settle its Serve",
            )
        })?;
        if self.durable_records[&serve_ordinal].payload != expected_payload {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "terminal payload projection changed during settlement",
            )
            .into());
        }
        Ok(())
    }
    /// Open with an already authenticated bounded episode authority.
    #[cfg(test)]
    pub(super) fn open_with_authority(
        authority: AuthenticatedEpisodeAuthority,
        ledger_root: &Path,
        payload_store: &mut CertifiedServePayloadStoreV1,
        mut recovery: AuthenticatedLifecycleRecoveryCut,
    ) -> Result<Self, LifecycleOpenError> {
        Self::open_with_authority_borrowed(authority, ledger_root, payload_store, &mut recovery)
    }
    // RECOVERED_WAL_SIGN_BORROWED_OPEN_BEGIN
    /// Open while retaining the move-only authenticated recovery cut outside
    /// the coordinator transaction.
    ///
    /// Recovered-WAL startup needs this form so every failure can keep the
    /// exact payload authentication and installed registry borrow sealed for a
    /// fail-stop restart. Candidate values are cloned only into the new
    /// coordinator; no storage or concrete-work authority is duplicated.
    #[cfg(test)]
    pub(super) fn open_with_authority_borrowed(
        authority: AuthenticatedEpisodeAuthority,
        ledger_root: &Path,
        payload_store: &mut CertifiedServePayloadStoreV1,
        recovery: &mut AuthenticatedLifecycleRecoveryCut,
    ) -> Result<Self, LifecycleOpenError> {
        let prepared =
            Self::prepare_with_authority_borrowed(authority, ledger_root, payload_store, recovery)?;
        prepared
            .commit(payload_store, recovery)
            .map_err(LifecycleOpenCommitError::into_error)
    }
    /// Complete recovery and rebinding without publishing either local store.
    #[cfg(test)]
    pub(super) fn prepare_with_authority_borrowed(
        authority: AuthenticatedEpisodeAuthority,
        ledger_root: &Path,
        payload_store: &CertifiedServePayloadStoreV1,
        recovery: &AuthenticatedLifecycleRecoveryCut,
    ) -> Result<PreparedLifecycleCoordinatorOpen, LifecycleOpenError> {
        let context = authority.context();
        let (store, ledger) = LifecycleLedgerStoreV1::open(ledger_root, context)?;
        Self::prepare_with_exact_store_borrowed(authority, store, ledger, payload_store, recovery)
    }
    /// Prepare from the exact ledger-store instance retained by the consuming
    /// ordinary body-pipeline storage cut. No caller-selected path can be substituted.
    pub(super) fn prepare_with_authenticated_store_borrowed(
        authority: AuthenticatedEpisodeAuthority,
        store: LifecycleLedgerStoreV1,
        payload_store: &CertifiedServePayloadStoreV1,
        recovery: &AuthenticatedLifecycleRecoveryCut,
    ) -> Result<PreparedLifecycleCoordinatorOpen, LifecycleOpenError> {
        let ledger = store.load()?;
        Self::prepare_with_exact_store_borrowed(authority, store, ledger, payload_store, recovery)
    }
    /// Prepare a fully authenticated prospective successor while the exact
    /// retained store still contains its predecessor frame.
    ///
    /// This is the sole pre-fsync open used by recovered Decision Apply. All
    /// logical reconstruction, Serve payload validation, and registry-batch
    /// preparation target `successor`; publication later compares and replaces
    /// `predecessor` through the same store instance.
    pub(super) fn prepare_with_authenticated_successor_store_borrowed(
        authority: AuthenticatedEpisodeAuthority,
        store: LifecycleLedgerStoreV1,
        predecessor: LifecycleLedgerV1,
        successor: LifecycleLedgerV1,
        payload_store: &CertifiedServePayloadStoreV1,
        recovery: &AuthenticatedLifecycleRecoveryCut,
    ) -> Result<PreparedLifecycleCoordinatorOpen, LifecycleOpenError> {
        if !store.load().is_ok_and(|opened| opened == predecessor) {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "lifecycle ledger predecessor changed before prospective open",
            )
            .into());
        }
        Self::prepare_with_exact_store_successor_borrowed(
            authority,
            store,
            predecessor,
            successor,
            payload_store,
            recovery,
        )
    }
    fn prepare_with_exact_store_borrowed(
        authority: AuthenticatedEpisodeAuthority,
        store: LifecycleLedgerStoreV1,
        ledger: LifecycleLedgerV1,
        payload_store: &CertifiedServePayloadStoreV1,
        recovery: &AuthenticatedLifecycleRecoveryCut,
    ) -> Result<PreparedLifecycleCoordinatorOpen, LifecycleOpenError> {
        Self::prepare_with_exact_store_successor_borrowed(
            authority,
            store,
            ledger.clone(),
            ledger,
            payload_store,
            recovery,
        )
    }
    fn prepare_with_exact_store_successor_borrowed(
        authority: AuthenticatedEpisodeAuthority,
        store: LifecycleLedgerStoreV1,
        persisted_predecessor: LifecycleLedgerV1,
        ledger: LifecycleLedgerV1,
        payload_store: &CertifiedServePayloadStoreV1,
        recovery: &AuthenticatedLifecycleRecoveryCut,
    ) -> Result<PreparedLifecycleCoordinatorOpen, LifecycleOpenError> {
        let context = authority.context();
        if recovery.context != context {
            return Err(LifecycleOpenErrorKind::InvalidRecovery("foreign recovery context").into());
        }
        payload_store.validate_authenticated_cut(&recovery.serve_payloads)?;
        if !recovery.authenticates_opened_ledger(&ledger) {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "lifecycle ledger changed after recovery-cut authentication",
            )
            .into());
        }
        validate_terminal_validate_no_successor_recovery(&ledger, &recovery.validate_no_successor)?;
        let records_by_key = decoded_records_by_key(&ledger)?;
        let (serve_candidates, terminal_updates, retained_serve_payloads, serve_replay_pairs) =
            resolve_serve_payloads(context, &ledger, &records_by_key, &recovery.serve_payloads)?;
        let has_store_ahead_terminal_updates = !terminal_updates.is_empty();
        let mut recovered_candidates = recovery.candidates.clone();
        for candidate in serve_candidates {
            if recovered_candidates
                .insert(candidate.key, candidate)
                .is_some()
            {
                return Err(LifecycleOpenErrorKind::InvalidRecovery(
                    "Serve projection collided with non-Serve recovery work",
                )
                .into());
            }
        }
        let mut physical_universes = ledger
            .records()
            .iter()
            .map(|record| (record.ordinal(), BTreeSet::new()))
            .collect::<BTreeMap<_, _>>();
        let mut candidates_by_ordinal = BTreeMap::new();
        let mut producer_coverage = BTreeSet::new();
        let terminal_updates_by_ordinal = terminal_updates
            .iter()
            .map(|update| (update.ordinal, update))
            .collect::<BTreeMap<_, _>>();
        for (_, mut candidate) in recovered_candidates {
            candidate.canonicalize_geometry().map_err(|_| {
                LifecycleOpenErrorKind::InvalidRecovery("invalid physical geometry")
            })?;
            let record = records_by_key.get(&candidate.key).copied().ok_or(
                LifecycleOpenErrorKind::InvalidRecovery(
                    "recovered candidate has no durable semantic row",
                ),
            )?;
            let ordinal = record.ordinal();
            let terminal_update = terminal_updates_by_ordinal.get(&ordinal).copied();
            validate_candidate_record(ledger.context(), record, &candidate, terminal_update)?;
            if candidates_by_ordinal
                .insert(ordinal, candidate.clone())
                .is_some()
            {
                return Err(LifecycleOpenErrorKind::InvalidRecovery(
                    "multiple candidates cover one durable row",
                )
                .into());
            }
            if record.terminal().flatten().is_none() {
                let (_, universe, _) = candidate.physical_geometry.normalized().map_err(|_| {
                    LifecycleOpenErrorKind::InvalidRecovery("invalid primary geometry")
                })?;
                if !authority.admits_slots(candidate.work_class.capacity_class(), &universe) {
                    return Err(LifecycleOpenErrorKind::InvalidRecovery(
                        "primary geometry exceeds authenticated capacity",
                    )
                    .into());
                }
                physical_universes.insert(ordinal, universe);
            }
            match (candidate.work_class, candidate.producer_turn.as_ref()) {
                (LifecycleWorkClass::CertifiedServe, Some(producer)) => {
                    let producer_ordinal =
                        ordinal
                            .checked_add(1)
                            .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                                "producer ordinal overflowed",
                            ))?;
                    let producer_record = ledger_record_at(&ledger, producer_ordinal).ok_or(
                        LifecycleOpenErrorKind::InvalidRecovery(
                            "Serve has no adjacent durable producer",
                        ),
                    )?;
                    let replay_matches = terminal_update.map_or_else(
                        || producer_record.replay_matches_producer(producer),
                        |update| {
                            update
                                .replay
                                .exactly_matches_recovered_candidate(ledger.context(), &candidate)
                                && record.replay_is_exact_pending_predecessor(
                                    ledger.context(),
                                    producer_record,
                                    &update.replay,
                                )
                        },
                    );
                    if producer_record.key() != Some(producer.key)
                        || producer_record.owner() != record.owner()
                        || producer_record.work_class() != Some(LifecycleWorkClass::ProducerTurn)
                        || producer_record.stage() != Some(producer.stage)
                        || producer_record.reconstruction_source() != producer.reconstruction_source
                        || !replay_matches
                    {
                        return Err(LifecycleOpenErrorKind::InvalidRecovery(
                            "producer companion changed durable semantics",
                        )
                        .into());
                    }
                    if producer_record.terminal().flatten().is_none() {
                        let (_, universe, _) =
                            producer.physical_geometry.normalized().map_err(|_| {
                                LifecycleOpenErrorKind::InvalidRecovery("invalid producer geometry")
                            })?;
                        if !authority.admits_slots(
                            LifecycleWorkClass::ProducerTurn.capacity_class(),
                            &universe,
                        ) || !producer_coverage.insert(producer_ordinal)
                        {
                            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                                "producer geometry or coverage is invalid",
                            )
                            .into());
                        }
                        physical_universes.insert(producer_ordinal, universe);
                    }
                }
                (LifecycleWorkClass::CertifiedServe, None) => {
                    return Err(LifecycleOpenErrorKind::InvalidRecovery(
                        "recovered Serve lacks its producer companion",
                    )
                    .into());
                }
                (_, Some(_)) => {
                    return Err(LifecycleOpenErrorKind::InvalidRecovery(
                        "non-Serve candidate carries a producer companion",
                    )
                    .into());
                }
                (_, None) => {}
            }
        }
        drop(terminal_updates_by_ordinal);
        let mut required_candidates = BTreeSet::new();
        let mut required_producers = BTreeSet::new();
        for record in ledger.records() {
            let terminal = record
                .terminal()
                .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                    "durable terminal cannot be decoded",
                ))?;
            match (record.work_class(), terminal) {
                (Some(LifecycleWorkClass::ProducerTurn), None) => {
                    required_producers.insert(record.ordinal());
                }
                (Some(_), None) => {
                    required_candidates.insert(record.ordinal());
                }
                (Some(LifecycleWorkClass::CertifiedServe), Some(_)) => {
                    if record
                        .ordinal()
                        .checked_add(1)
                        .and_then(|ordinal| ledger_record_at(&ledger, ordinal))
                        .is_some_and(|producer| producer.terminal().flatten().is_none())
                    {
                        required_candidates.insert(record.ordinal());
                    }
                }
                (Some(_), Some(_)) => {}
                (None, _) => {
                    return Err(LifecycleOpenErrorKind::InvalidRecovery(
                        "durable work class cannot be decoded",
                    )
                    .into());
                }
            }
        }
        if required_candidates != candidates_by_ordinal.keys().copied().collect()
            || required_producers != producer_coverage
        {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "live durable record coverage is not exact",
            )
            .into());
        }
        let snapshot = ledger.recovery_snapshot(physical_universes)?;
        let mut coordinator =
            LifecycleCoordinator::new_with_authority(authority, ledger.high_water());
        coordinator.reconcile_restart(snapshot);
        if coordinator.fault == Some(CoordinatorFault::RecoveryRejected) {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "coordinator rejected the reconstructed durable state",
            )
            .into());
        }
        let mut terminal_updates = terminal_updates
            .into_iter()
            .map(|update| (update.ordinal, update))
            .collect::<BTreeMap<_, _>>();
        for (ordinal, candidate) in candidates_by_ordinal {
            if let Some(update) = terminal_updates.remove(&ordinal) {
                coordinator.reconcile_store_ahead_terminal_serve(ordinal, candidate, update)?;
                continue;
            }
            if matches!(
                coordinator.records[&ordinal].state,
                LifecycleState::Terminal(_)
            ) {
                coordinator.rebind_terminal_serve_producer(ordinal, candidate)?;
                continue;
            }
            match coordinator.reduce_admit(AdmissionRequest::Candidate(candidate)) {
                AdmissionDecision::Retry {
                    ordinal: rebound, ..
                } if rebound == ordinal => {}
                _ => {
                    return Err(LifecycleOpenErrorKind::InvalidRecovery(
                        "recovered candidate did not rebind exactly",
                    )
                    .into());
                }
            }
        }
        if !terminal_updates.is_empty() {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "payload-store-ahead Serve transition has no exact candidate owner",
            )
            .into());
        }
        if coordinator.records.values().any(|record| {
            matches!(
                record.state,
                LifecycleState::Waiting(wait)
                    if matches!(wait.source, super::WaitSource::Recovery(_))
            )
        }) {
            return Err(
                LifecycleOpenErrorKind::InvalidRecovery("recovery work remains unbound").into(),
            );
        }
        let certified_serve_registry = PreparedCertifiedServeRegistryBatchV1::from_recovered_pairs(
            &coordinator,
            serve_replay_pairs,
        )
        .map_err(|_| {
            LifecycleOpenErrorKind::InvalidRecovery(
                "Certified-Serve concrete recovery coverage is not exact",
            )
        })?;
        let authenticated_successor = LifecycleLedgerV1::from_coordinator(&coordinator)?;
        if !has_store_ahead_terminal_updates && authenticated_successor != ledger {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "recovered coordinator does not reproduce its authenticated LedgerV1 frame",
            )
            .into());
        }
        Ok(PreparedLifecycleCoordinatorOpen {
            coordinator,
            store,
            persisted_predecessor,
            authenticated_successor,
            retained_serve_payloads,
            certified_serve_registry: Some(certified_serve_registry),
        })
    }
    // RECOVERED_WAL_SIGN_BORROWED_OPEN_END
    /// Exercise the test-only rollover state transition in focused reducer tests.
    #[cfg(test)]
    pub(crate) fn rollover(&mut self, snapshot: RolloverSnapshot) {
        self.rollover_inner(snapshot, None);
    }
    /// Exercise test-only rollover with a retained Serve payload store.
    #[cfg(test)]
    pub(crate) fn rollover_with_payload_store(
        &mut self,
        snapshot: RolloverSnapshot,
        payload_store: &mut CertifiedServePayloadStoreV1,
    ) {
        self.rollover_inner(snapshot, Some(payload_store));
    }
    #[cfg(test)]
    fn rollover_inner(
        &mut self,
        snapshot: RolloverSnapshot,
        payload_store: Option<&mut CertifiedServePayloadStoreV1>,
    ) {
        if self.fault.is_some() {
            return;
        }
        if !self.rollover_snapshot_is_exact(&snapshot) {
            self.fault = Some(CoordinatorFault::InvalidRollover);
            return;
        }
        if self.ledger_store.is_none() {
            let mut next = self.stage_durable_transaction();
            if snapshot.successor_ledger_root.is_some()
                || !snapshot.serve_cancellations.is_empty()
                || next.retire_for_rollover(&snapshot).is_err()
            {
                self.fault = Some(CoordinatorFault::InvalidRollover);
                return;
            }
            next.activate_successor(snapshot);
            *self = next;
            return;
        }
        let Some(successor_root) = snapshot.successor_ledger_root.as_deref() else {
            self.fault = Some(CoordinatorFault::InvalidRollover);
            return;
        };
        if !self.serve_cancellation_receipts_are_exact(&snapshot) {
            self.fault = Some(CoordinatorFault::InvalidRollover);
            return;
        }
        let Some(serve_wait_rollbacks) = self.serve_wait_rollback_receipts() else {
            self.fault = Some(CoordinatorFault::InvalidRollover);
            return;
        };
        let mut retired = self.stage_durable_transaction();
        if retired.retire_for_rollover(&snapshot).is_err() {
            self.fault = Some(CoordinatorFault::InvalidRollover);
            return;
        }
        if !serve_wait_rollbacks.is_empty()
            && payload_store
                .ok_or(())
                .and_then(|store| {
                    store
                        .rollback_pending_batch(&serve_wait_rollbacks)
                        .map_err(|_| ())
                })
                .is_err()
        {
            self.fault = Some(CoordinatorFault::DurabilityFailure);
            return;
        }
        let retired_projection = match LifecycleLedgerV1::from_coordinator(&retired) {
            Ok(ledger) => ledger,
            Err(_) => {
                self.fault = Some(CoordinatorFault::DurabilityFailure);
                return;
            }
        };
        if retired
            .ledger_store
            .as_ref()
            .expect("durable rollover retains its predecessor store")
            .persist(&retired_projection)
            .is_err()
        {
            self.fault = Some(CoordinatorFault::DurabilityFailure);
            return;
        }
        let successor_store =
            match LifecycleLedgerStoreV1::open(successor_root, snapshot.successor_context) {
                Ok((store, existing))
                    if existing.records().is_empty()
                        && existing.producer_debts().is_empty()
                        && (existing.high_water() == 0
                            || existing.high_water() == snapshot.retained_high_water) =>
                {
                    store
                }
                Ok(_) | Err(_) => {
                    retired.fault = Some(CoordinatorFault::DurabilityFailure);
                    *self = retired;
                    return;
                }
            };
        let mut successor = LifecycleCoordinator::new_with_authority(
            snapshot.successor_authority.clone(),
            snapshot.retained_high_water,
        );
        let successor_projection = match LifecycleLedgerV1::from_coordinator(&successor) {
            Ok(ledger) => ledger,
            Err(_) => {
                retired.fault = Some(CoordinatorFault::DurabilityFailure);
                *self = retired;
                return;
            }
        };
        if successor_store.persist(&successor_projection).is_err() {
            retired.fault = Some(CoordinatorFault::DurabilityFailure);
            *self = retired;
            return;
        }
        successor.ledger_store = Some(successor_store);
        *self = successor;
    }
    #[cfg(test)]
    fn rollover_snapshot_is_exact(&self, snapshot: &RolloverSnapshot) -> bool {
        let live_ordinals = self
            .records
            .iter()
            .filter_map(|(ordinal, record)| {
                (!matches!(record.state, LifecycleState::Terminal(_))).then_some(*ordinal)
            })
            .collect::<BTreeSet<_>>();
        let pending_keys = self
            .admission_waits
            .keys()
            .copied()
            .collect::<BTreeSet<_>>();
        self.active_lease.is_none()
            && self.active_context == snapshot.retired_context
            && snapshot.successor_context.id != snapshot.retired_context.id
            && snapshot.successor_predecessor == snapshot.retired_context.id
            && snapshot.successor_authority.context() == snapshot.successor_context
            && snapshot.retired_context.height.checked_add(1)
                == Some(snapshot.successor_context.height)
            && snapshot.retained_high_water == self.high_water
            && snapshot.retire_ordinals == live_ordinals
            && snapshot.retire_admission_keys == pending_keys
    }
    #[cfg(test)]
    fn serve_cancellation_receipts_are_exact(&self, snapshot: &RolloverSnapshot) -> bool {
        let mut cancellations = BTreeMap::new();
        for receipt in &snapshot.serve_cancellations {
            if receipt.outcome() != CertifiedServePayloadNegativeOutcome::Cancelled {
                return false;
            }
            let request = digest_bytes(receipt.id().request_hash().as_ref());
            let certificate = digest_bytes(receipt.certificate_hash().as_ref());
            if cancellations.insert(request, certificate).is_some() {
                return false;
            }
        }
        let mut expected = BTreeMap::new();
        for record in self.records.values().filter(|record| {
            record.work_class == LifecycleWorkClass::CertifiedServe
                && !matches!(record.state, LifecycleState::Terminal(_))
        }) {
            let DurablePayloadReference::CertifiedServePending {
                request,
                certificate,
            } = self.durable_records[&record.ordinal].payload
            else {
                return false;
            };
            if expected.insert(request, certificate).is_some() {
                return false;
            }
        }
        expected == cancellations
    }
    #[cfg(test)]
    fn serve_wait_rollback_receipts(&self) -> Option<Vec<DurableCertifiedServeAdmissionReceipt>> {
        let mut receipts = Vec::new();
        for waiting in self.admission_waits.values() {
            match (waiting.candidate.work_class, waiting.serve_payload_receipt) {
                (LifecycleWorkClass::CertifiedServe, Some(receipt)) => receipts.push(receipt),
                (LifecycleWorkClass::CertifiedServe, None) | (_, Some(_)) => return None,
                (_, None) => {}
            }
        }
        Some(receipts)
    }
    #[cfg(test)]
    fn retire_for_rollover(&mut self, snapshot: &RolloverSnapshot) -> Result<(), CoordinatorFault> {
        let cancellations = snapshot
            .serve_cancellations
            .iter()
            .map(|receipt| (digest_bytes(receipt.id().request_hash().as_ref()), *receipt))
            .collect::<BTreeMap<_, _>>();
        for ordinal in &snapshot.retire_ordinals {
            let Some(record) = self.records.get(ordinal) else {
                return Err(CoordinatorFault::InvalidRollover);
            };
            if !matches!(record.state, LifecycleState::Terminal(_))
                && record.work_class == LifecycleWorkClass::CertifiedServe
                && self.ledger_store.is_some()
            {
                let DurablePayloadReference::CertifiedServePending { request, .. } = self
                    .durable_records
                    .get(ordinal)
                    .ok_or(CoordinatorFault::InvalidRollover)?
                    .payload
                else {
                    return Err(CoordinatorFault::InvalidRollover);
                };
                let receipt = cancellations
                    .get(&request)
                    .copied()
                    .ok_or(CoordinatorFault::InvalidRollover)?;
                let producer_ordinal = self
                    .producer_debts
                    .get(ordinal)
                    .copied()
                    .ok_or(CoordinatorFault::InvalidRollover)?;
                let terminal = CertifiedServeTerminalReplayAuthorityPairV1::from_negative_receipt(
                    self.active_context,
                    &self.records[ordinal],
                    &self.durable_records[ordinal],
                    self.records
                        .get(&producer_ordinal)
                        .ok_or(CoordinatorFault::InvalidRollover)?,
                    self.durable_records
                        .get(&producer_ordinal)
                        .ok_or(CoordinatorFault::InvalidRollover)?,
                    receipt,
                )
                .ok_or(CoordinatorFault::InvalidRollover)?;
                let (payload, outcome, serve_replay, producer_replay) =
                    terminal.consume_terminal_rebind();
                if outcome != TerminalOutcome::Cancelled {
                    return Err(CoordinatorFault::InvalidRollover);
                }
                let metadata = self
                    .durable_records
                    .get_mut(ordinal)
                    .expect("rollover preflight retained Serve metadata");
                metadata.payload = payload;
                metadata.replay_authority = serve_replay;
                self.durable_records
                    .get_mut(&producer_ordinal)
                    .expect("rollover preflight retained ProducerTurn metadata")
                    .replay_authority = producer_replay;
            }
            if !self
                .records
                .get(ordinal)
                .is_some_and(|record| matches!(record.state, LifecycleState::Terminal(_)))
            {
                self.finish_terminal(*ordinal, TerminalOutcome::Cancelled)?;
            }
        }
        for key in &snapshot.retire_admission_keys {
            self.admission_waits.remove(key);
        }
        if !self.producer_debts.is_empty()
            || self.capacity_used.values().any(|used| *used != 0)
            || self
                .records
                .values()
                .any(|record| !matches!(record.state, LifecycleState::Terminal(_)))
        {
            return Err(CoordinatorFault::InvalidRollover);
        }
        Ok(())
    }
    #[cfg(test)]
    fn activate_successor(&mut self, snapshot: RolloverSnapshot) {
        self.records.clear();
        self.key_index.clear();
        self.ready_index.clear();
        self.owner_index.clear();
        self.durable_records.clear();
        self.producer_debts.clear();
        self.observed_generation.clear();
        self.capacity_generation
            .values_mut()
            .for_each(|generation| *generation = 0);
        self.next_lease = Some(1);
        self.capacity_geometry = snapshot.successor_authority.capacity_geometry().clone();
        self.episode_authority = snapshot.successor_authority;
        self.active_context = snapshot.successor_context;
    }
    fn rebind_terminal_serve_producer(
        &mut self,
        serve_ordinal: u128,
        mut candidate: CandidateAdmission,
    ) -> Result<(), LifecycleOpenError> {
        candidate.canonicalize_geometry().map_err(|_| {
            LifecycleOpenErrorKind::InvalidRecovery("invalid terminal Serve geometry")
        })?;
        let serve =
            self.records
                .get(&serve_ordinal)
                .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                    "terminal Serve record disappeared",
                ))?;
        if serve.work_class != LifecycleWorkClass::CertifiedServe
            || !matches!(serve.state, LifecycleState::Terminal(_))
            || serve.key != candidate.key
            || serve.owner.causal_root() != candidate.causal_root
            || serve.stage != candidate.stage
            || !self.durable_records[&serve_ordinal].matches_admission(&candidate)
            || !self.retry_companion_matches(serve, &candidate)
        {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "terminal Serve recovery companion changed semantics",
            )
            .into());
        }
        let producer_ordinal = self.producer_debts.get(&serve_ordinal).copied().ok_or(
            LifecycleOpenErrorKind::InvalidRecovery("terminal Serve has no live producer debt"),
        )?;
        let producer =
            candidate
                .producer_turn
                .as_ref()
                .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                    "terminal Serve lacks producer geometry",
                ))?;
        let (physical, universe, consumed) = producer
            .physical_geometry
            .normalized()
            .map_err(|_| LifecycleOpenErrorKind::InvalidRecovery("invalid producer geometry"))?;
        let record = self.records.get_mut(&producer_ordinal).ok_or(
            LifecycleOpenErrorKind::InvalidRecovery("terminal Serve producer disappeared"),
        )?;
        if record.episode.slot_universe != universe
            || !record.physical_slots.is_empty()
            || !record.episode.consumed_slots.is_empty()
            || !matches!(record.state, LifecycleState::Ready)
        {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "terminal Serve producer cannot be rebound",
            )
            .into());
        }
        record.physical_slots = physical;
        record.episode.consumed_slots = consumed;
        Ok(())
    }
}
fn validate_terminal_validate_no_successor_recovery(
    ledger: &LifecycleLedgerV1,
    recovered: &BTreeMap<LifecycleKey, AuthenticatedValidateNoSuccessorRecovery>,
) -> Result<(), LifecycleOpenError> {
    let mut expected = BTreeMap::new();
    for record in ledger.records() {
        if record.work_class() != Some(LifecycleWorkClass::Validate)
            || record.terminal() != Some(Some(TerminalOutcome::Advanced))
            || record.continuation() != Some(DurableContinuation::AdvancedNoSuccessor)
        {
            continue;
        }
        let key = record.key().ok_or(LifecycleOpenErrorKind::InvalidRecovery(
            "terminal Validate key cannot be decoded",
        ))?;
        let proof = AuthenticatedValidateNoSuccessorRecovery {
            key,
            causal_root: record.owner().causal_root(),
            reconstruction_source: record.reconstruction_source(),
            stage: record
                .stage()
                .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                    "terminal Validate stage cannot be decoded",
                ))?,
            payload: record
                .durable_payload()
                .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                    "terminal Validate body-frame payload cannot be decoded",
                ))?,
        };
        if expected.insert(key, proof).is_some() {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "terminal Validate recovery identity is duplicated",
            )
            .into());
        }
    }
    if &expected != recovered {
        return Err(LifecycleOpenErrorKind::InvalidRecovery(
            "terminal Validate no-successor recovery coverage is not exact",
        )
        .into());
    }
    Ok(())
}
fn decoded_records_by_key(
    ledger: &super::ledger::LifecycleLedgerV1,
) -> Result<BTreeMap<LifecycleKey, &LifecycleLedgerRecordV1>, LifecycleOpenError> {
    let mut records = BTreeMap::new();
    for record in ledger.records() {
        let key = record.key().ok_or(LifecycleOpenErrorKind::InvalidRecovery(
            "durable key cannot be decoded",
        ))?;
        if records.insert(key, record).is_some() {
            return Err(
                LifecycleOpenErrorKind::InvalidRecovery("duplicate durable semantic key").into(),
            );
        }
    }
    Ok(records)
}
fn ledger_record_at(
    ledger: &super::ledger::LifecycleLedgerV1,
    ordinal: u128,
) -> Option<&LifecycleLedgerRecordV1> {
    ledger
        .records()
        .binary_search_by_key(&ordinal, LifecycleLedgerRecordV1::ordinal)
        .ok()
        .and_then(|index| ledger.records().get(index))
}
fn digest_bytes(bytes: &[u8]) -> LifecycleDigest {
    let mut digest = [0_u8; 32];
    digest.copy_from_slice(bytes);
    LifecycleDigest::new(digest)
}
fn validate_candidate_record(
    context: LifecycleContext,
    record: &LifecycleLedgerRecordV1,
    candidate: &CandidateAdmission,
    terminal_update: Option<&TerminalUpdate>,
) -> Result<(), LifecycleOpenError> {
    let replay_matches = terminal_update.map_or_else(
        || record.replay_matches_candidate(candidate),
        |update| {
            update.ordinal == record.ordinal()
                && update
                    .payload
                    .matches_terminal(LifecycleWorkClass::CertifiedServe, Some(update.outcome))
                && update
                    .replay
                    .exactly_matches_recovered_candidate(context, candidate)
        },
    );
    if !candidate.replay_authority_is_exact(context)
        || record.owner().causal_root() != candidate.causal_root
        || record.work_class() != Some(candidate.work_class)
        || record.stage() != Some(candidate.stage)
        || record.reconstruction_source() != candidate.reconstruction_source
        || record
            .durable_payload()
            .is_none_or(|payload| !payload.same_admission_material(candidate.payload))
        || !replay_matches
        || candidate.initial_state != super::InitialLifecycleState::Ready
    {
        return Err(LifecycleOpenErrorKind::InvalidRecovery(
            "recovered candidate changed durable semantics",
        )
        .into());
    }
    Ok(())
}
struct TerminalUpdate {
    ordinal: u128,
    outcome: TerminalOutcome,
    payload: DurablePayloadReference,
    replay: CertifiedServeTerminalReplayAuthorityPairV1,
}
/// One payload-store-ahead terminal transition bound to its exact ledger pair.
///
/// The only consuming surface rechecks the immutable source rows before
/// releasing the values which the ledger module must install. There is no raw
/// constructor or parts accessor.
#[must_use = "the authenticated Serve terminal update must be applied or dropped"]
pub(in crate::sumeragi::v2_lifecycle_coordinator) struct CompleteTipServeTerminalUpdateV1 {
    context: LifecycleContext,
    source_serve: LifecycleLedgerRecordV1,
    source_producer: LifecycleLedgerRecordV1,
    terminal: TerminalUpdate,
}
impl CompleteTipServeTerminalUpdateV1 {
    fn exactly_matches_pair(
        &self,
        serve: &LifecycleLedgerRecordV1,
        producer: &LifecycleLedgerRecordV1,
    ) -> bool {
        self.source_serve == *serve
            && self.source_producer == *producer
            && self.terminal.ordinal == serve.ordinal()
            && self.terminal.payload.matches_terminal(
                LifecycleWorkClass::CertifiedServe,
                Some(self.terminal.outcome),
            )
            && serve.replay_is_exact_pending_predecessor(
                self.context,
                producer,
                &self.terminal.replay,
            )
    }
    /// Consume this update only for the exact Pending Serve/Producer source pair.
    ///
    /// The returned tuple is the fixed ledger mutation payload: terminal Serve
    /// payload, terminal outcome, Serve replay authority, and Producer replay
    /// authority, in that order. It is unavailable for substituted rows.
    pub(in crate::sumeragi::v2_lifecycle_coordinator) fn consume_for_exact_ledger_pair(
        self,
        serve: &LifecycleLedgerRecordV1,
        producer: &LifecycleLedgerRecordV1,
    ) -> Option<(
        DurablePayloadReference,
        TerminalOutcome,
        LifecycleReplayAuthorityV1,
        LifecycleReplayAuthorityV1,
    )> {
        if !self.exactly_matches_pair(serve, producer) {
            return None;
        }
        let expected_payload = self.terminal.payload;
        let expected_outcome = self.terminal.outcome;
        let parts = self.terminal.replay.consume_terminal_rebind();
        (parts.0 == expected_payload && parts.1 == expected_outcome).then_some(parts)
    }
}
/// Move-only CompleteTip reconciliation of one final payload cut and ledger frame.
///
/// The authenticated payload cut remains owned by this seal. Every final-cut
/// ID has exactly one ledger Serve owner, every live Serve has one terminal
/// update, and a terminal Serve whose adjacent Producer remains live has one
/// explicit no-update coverage entry. Callers can neither reconstruct updates
/// nor detach the underlying payload authentication.
#[must_use = "CompleteTip Serve reconciliation must be consumed by ledger retirement"]
pub(in crate::sumeragi::v2_lifecycle_coordinator) struct CompleteTipServeRetirementReconciliationV1
{
    source_context: LifecycleContext,
    source_frame_identity: LifecycleDigest,
    terminal_updates: BTreeMap<u128, CompleteTipServeTerminalUpdateV1>,
    terminal_serve_live_producers:
        BTreeMap<u128, (LifecycleLedgerRecordV1, LifecycleLedgerRecordV1)>,
    _authenticated_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
}
impl CompleteTipServeRetirementReconciliationV1 {
    /// Check that a ledger is byte-identical to the frame authenticated here.
    pub(in crate::sumeragi::v2_lifecycle_coordinator) fn authenticates_source(
        &self,
        ledger: &LifecycleLedgerV1,
    ) -> bool {
        ledger.context() == self.source_context
            && ledger.frame_identity() == self.source_frame_identity
    }
    /// Remove the sole terminal transition for an exact live Serve pair.
    ///
    /// A mismatched or already-consumed pair leaves the reconciliation intact.
    pub(in crate::sumeragi::v2_lifecycle_coordinator) fn take_terminal_update_for_exact_pair(
        &mut self,
        serve: &LifecycleLedgerRecordV1,
        producer: &LifecycleLedgerRecordV1,
    ) -> Option<CompleteTipServeTerminalUpdateV1> {
        let ordinal = serve.ordinal();
        self.terminal_updates
            .get(&ordinal)
            .is_some_and(|update| update.exactly_matches_pair(serve, producer))
            .then(|| {
                self.terminal_updates
                    .remove(&ordinal)
                    .expect("the exact update remained present")
            })
    }
    /// Consume no-update coverage for a terminal Serve with a live Producer.
    ///
    /// Retirement still has to terminalize the Producer and discharge its
    /// debt, but must not rewrite the already-terminal Serve payload.
    pub(in crate::sumeragi::v2_lifecycle_coordinator) fn take_terminal_serve_live_producer_coverage(
        &mut self,
        serve: &LifecycleLedgerRecordV1,
        producer: &LifecycleLedgerRecordV1,
    ) -> bool {
        let ordinal = serve.ordinal();
        let exact = self
            .terminal_serve_live_producers
            .get(&ordinal)
            .is_some_and(|(expected_serve, expected_producer)| {
                expected_serve == serve && expected_producer == producer
            });
        if exact {
            self.terminal_serve_live_producers.remove(&ordinal);
        }
        exact
    }
    /// Return true after every required Serve action or coverage proof was consumed.
    pub(in crate::sumeragi::v2_lifecycle_coordinator) fn is_drained(&self) -> bool {
        self.terminal_updates.is_empty() && self.terminal_serve_live_producers.is_empty()
    }
}
#[allow(clippy::too_many_lines)]
fn resolve_serve_payloads(
    context: LifecycleContext,
    ledger: &LifecycleLedgerV1,
    records: &BTreeMap<LifecycleKey, &LifecycleLedgerRecordV1>,
    recovered: &AuthenticatedCertifiedServePayloadRecoveryCut,
) -> Result<
    (
        Vec<CandidateAdmission>,
        Vec<TerminalUpdate>,
        BTreeSet<CertifiedServePayloadId>,
        BTreeMap<LifecycleKey, super::replay_authority::CertifiedServeReplayEvidencePairV1>,
    ),
    LifecycleOpenError,
> {
    if digest_bytes(recovered.context_id().0.as_ref()) != context.id()
        || recovered.height() != context.height()
    {
        return Err(
            LifecycleOpenErrorKind::InvalidRecovery("foreign Certified-Serve payload cut").into(),
        );
    }
    let mut recovered_by_request = BTreeMap::new();
    for payload in recovered.iter() {
        let request = digest_bytes(payload.id().request_hash().as_ref());
        if recovered_by_request.insert(request, payload).is_some() {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "duplicate authenticated Serve request identity",
            )
            .into());
        }
    }
    let mut candidates = Vec::new();
    let mut updates = Vec::new();
    let mut retained = BTreeSet::new();
    let mut replay_pairs = BTreeMap::new();
    for (key, record) in records {
        if record.work_class() != Some(LifecycleWorkClass::CertifiedServe) {
            continue;
        }
        let durable = record
            .durable_payload()
            .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                "Serve payload cannot be decoded",
            ))?;
        let request = durable
            .request()
            .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                "Serve ledger row lost its signed-request identity",
            ))?;
        let payload = recovered_by_request.get(&request).copied().ok_or(
            LifecycleOpenErrorKind::InvalidRecovery("Serve payload is missing from storage"),
        )?;
        retained.insert(payload.id());
        let (candidate, resolved, projected_terminal, projected_replay, replay_pair) =
            super::projection::recovered_certified_serve_projection(context, payload)
                .map_err(|_| {
                    LifecycleOpenErrorKind::InvalidRecovery(
                        "authenticated Serve payload could not be projected",
                    )
                })?
                .into_registry_parts();
        if candidate.key != *key {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "Serve six-field key changed its body/request identity",
            )
            .into());
        }
        if !durable.same_admission_material(resolved) {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "Serve payload changed request or certificate identity",
            )
            .into());
        }
        let ledger_terminal = record
            .terminal()
            .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                "Serve terminal cannot be decoded",
            ))?;
        if durable == resolved {
            if ledger_terminal != projected_terminal {
                return Err(LifecycleOpenErrorKind::InvalidRecovery(
                    "Serve payload state disagrees with its ledger terminal",
                )
                .into());
            }
            let producer =
                candidate
                    .producer_turn
                    .as_ref()
                    .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                        "steady Serve recovery lost its producer replay authority",
                    ))?;
            let producer_record = record
                .ordinal()
                .checked_add(1)
                .and_then(|ordinal| ledger_record_at(ledger, ordinal))
                .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                    "steady Serve recovery lost its adjacent producer row",
                ))?;
            if !record.replay_matches_candidate(&candidate)
                || !producer_record.replay_matches_producer(producer)
            {
                return Err(LifecycleOpenErrorKind::InvalidRecovery(
                    "steady Serve recovery frame changed its exact persisted family",
                )
                .into());
            }
            if projected_terminal.is_some() != projected_replay.is_some() {
                return Err(LifecycleOpenErrorKind::InvalidRecovery(
                    "terminal Serve recovery lost its exact replay family",
                )
                .into());
            }
        } else {
            let outcome = match (durable, resolved, projected_terminal) {
                (
                    DurablePayloadReference::CertifiedServePending { .. },
                    DurablePayloadReference::CertifiedServeCompleted { response, .. },
                    Some(TerminalOutcome::Completed(Some(projected_response))),
                ) if response == projected_response
                    && matches!(
                        payload.state(),
                        AuthenticatedRecoveredCertifiedServePayloadState::Completed(completed)
                            if completed.permits_payload_store_ahead_terminal_rebind()
                    ) =>
                {
                    TerminalOutcome::Completed(Some(response))
                }
                (
                    DurablePayloadReference::CertifiedServePending { .. },
                    DurablePayloadReference::CertifiedServeNegative { outcome, .. },
                    Some(projected),
                ) if outcome.terminal() == projected => projected,
                _ => {
                    return Err(LifecycleOpenErrorKind::InvalidRecovery(
                        "Serve payload storage regressed or conflicts with the ledger",
                    )
                    .into());
                }
            };
            if ledger_terminal.is_some() {
                return Err(LifecycleOpenErrorKind::InvalidRecovery(
                    "terminal Serve payload disagrees with its ledger tombstone",
                )
                .into());
            }
            let replay = projected_replay.ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                "payload-store-ahead Serve lost its terminal replay family",
            ))?;
            updates.push(TerminalUpdate {
                ordinal: record.ordinal(),
                outcome,
                payload: resolved,
                replay,
            });
        }
        let producer_is_live = record
            .ordinal()
            .checked_add(1)
            .and_then(|ordinal| ledger_record_at(ledger, ordinal))
            .is_some_and(|producer| {
                producer.work_class() == Some(LifecycleWorkClass::ProducerTurn)
                    && producer.terminal() == Some(None)
            });
        if ledger_terminal.is_none() || producer_is_live {
            if replay_pairs.insert(candidate.key, replay_pair).is_some() {
                return Err(LifecycleOpenErrorKind::InvalidRecovery(
                    "duplicate Certified-Serve concrete replay family",
                )
                .into());
            }
            candidates.push(candidate);
        }
    }
    if recovered.iter().any(|payload| {
        !retained.contains(&payload.id())
            && !matches!(
                payload.state(),
                AuthenticatedRecoveredCertifiedServePayloadState::Pending
            )
    }) {
        return Err(LifecycleOpenErrorKind::InvalidRecovery(
            "terminal Serve payload has no durable ledger owner",
        )
        .into());
    }
    Ok((candidates, updates, retained, replay_pairs))
}
/// Authenticate the complete predecessor Serve/payload census for CompleteTip retirement.
///
/// This comparison performs no payload or ledger mutation. It accepts
/// payload-store-ahead terminal frames so the consuming retirement transaction
/// can reconcile them, while rejecting a terminal orphan or any missing,
/// duplicate, foreign, or semantically drifted Serve owner.
pub(super) fn authenticate_complete_tip_serve_census(
    ledger: &LifecycleLedgerV1,
    recovered: &AuthenticatedCertifiedServePayloadRecoveryCut,
) -> Result<BTreeSet<CertifiedServePayloadId>, LifecycleOpenError> {
    let mut records = BTreeMap::new();
    for record in ledger.records() {
        let key = record.key().ok_or(LifecycleOpenErrorKind::InvalidRecovery(
            "CompleteTip predecessor has an undecodable lifecycle key",
        ))?;
        if records.insert(key, record).is_some() {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "CompleteTip predecessor has duplicate lifecycle keys",
            )
            .into());
        }
    }
    let (_, _, retained, _) =
        resolve_serve_payloads(ledger.context(), ledger, &records, recovered)?;
    Ok(retained)
}
/// Authenticate the exact live Serve census before ordinary height retirement.
///
/// The durable ledger owns admitted Serve rows, while capacity-fenced Serve
/// requests remain solely in the coordinator's bounded admission-wait map.
/// Their payload frames are nevertheless durable and must be present as exact
/// Pending entries in the freshly authenticated cut. No other payload orphan
/// is accepted. This read-only join runs only after ingress closure and the
/// exact-output handoff; the consuming retirement subsequently prunes those
/// wait-owned Pending frames and reconciles the retained ledger-owned rows.
pub(super) fn authenticate_live_finalization_serve_census(
    verified: &VerifiedHeightContext,
    ledger: &LifecycleLedgerV1,
    coordinator: &LifecycleCoordinator,
    recovered: &AuthenticatedCertifiedServePayloadRecoveryCut,
) -> Result<BTreeSet<CertifiedServePayloadId>, LifecycleOpenError> {
    let context = coordinator.active_context;
    if coordinator.fault.is_some()
        || coordinator.active_lease.is_some()
        || context != super::projection::lifecycle_context(verified.context())
        || ledger.context() != context
        || LifecycleLedgerV1::from_coordinator(coordinator)
            .ok()
            .as_ref()
            != Some(ledger)
        || coordinator.admission_waits.len() > super::MAX_PENDING_ADMISSION_WAITS
    {
        return Err(LifecycleOpenErrorKind::InvalidRecovery(
            "live finalization coordinator is not an exact quiescent ledger owner",
        )
        .into());
    }

    let retained = authenticate_complete_tip_serve_census(ledger, recovered)?;
    let mut owned = retained.clone();
    for (key, waiting) in &coordinator.admission_waits {
        let super::WaitSource::Capacity(class) = waiting.wait_token.source() else {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "live finalization admission wait lost its capacity fence",
            )
            .into());
        };
        let mut canonical = waiting.candidate.clone();
        if *key != waiting.candidate.key
            || coordinator.key_index.contains_key(key)
            || class != waiting.candidate.work_class.capacity_class()
            || waiting.wait_token.observed_generation() > coordinator.capacity_generation[&class]
            || canonical.canonicalize_geometry().is_err()
            || canonical != waiting.candidate
            || !waiting.candidate.replay_authority_is_exact(context)
            || !waiting
                .candidate
                .work_class
                .accepts_stage(waiting.candidate.key.phase(), waiting.candidate.stage)
            || coordinator
                .episode_authority
                .universe_for(waiting.candidate.key)
                .is_none()
        {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "live finalization admission wait changed its sealed candidate",
            )
            .into());
        }

        match (waiting.candidate.work_class, waiting.serve_payload_receipt) {
            (LifecycleWorkClass::CertifiedServe, Some(receipt)) => {
                let Some(payload) = recovered.get(receipt.id()) else {
                    return Err(LifecycleOpenErrorKind::InvalidRecovery(
                        "live finalization lost a wait-owned Serve payload",
                    )
                    .into());
                };
                if !matches!(
                    payload.state(),
                    AuthenticatedRecoveredCertifiedServePayloadState::Pending
                ) || !receipt.exactly_matches_pending(payload.request())
                {
                    return Err(LifecycleOpenErrorKind::InvalidRecovery(
                        "live finalization wait-owned Serve payload changed",
                    )
                    .into());
                }
                let prepared = super::projection::prepare_certified_serve_admission(
                    context,
                    verified,
                    payload.request(),
                    receipt,
                )
                .map_err(|_| {
                    LifecycleOpenError::from(LifecycleOpenErrorKind::InvalidRecovery(
                        "live finalization Serve wait no longer projects exactly",
                    ))
                })?;
                let (candidate, _replay) = prepared.into_candidate_and_replay();
                if candidate != waiting.candidate || !owned.insert(receipt.id()) {
                    return Err(LifecycleOpenErrorKind::InvalidRecovery(
                        "live finalization has duplicate or drifted Serve wait ownership",
                    )
                    .into());
                }
            }
            (LifecycleWorkClass::CertifiedServe, None) | (_, Some(_)) => {
                return Err(LifecycleOpenErrorKind::InvalidRecovery(
                    "live finalization admission wait lost typed Serve ownership",
                )
                .into());
            }
            (_, None) => {}
        }
    }

    let recovered_ids = recovered.iter().map(|payload| payload.id()).collect();
    if owned != recovered_ids {
        return Err(LifecycleOpenErrorKind::InvalidRecovery(
            "live finalization payload cut contains an unexplained orphan",
        )
        .into());
    }
    Ok(retained)
}
/// Seal the final post-mutation Serve cut for CompleteTip ledger retirement.
///
/// Unlike the pre-mutation census, this boundary permits no Pending orphan:
/// every payload ID in the final authenticated cut must be retained by one
/// exact ledger Serve. Every live Serve must resolve through a payload-store-
/// ahead terminal update, while an already-terminal Serve may contribute only
/// explicit coverage for its still-live adjacent Producer.
///
/// # Errors
///
/// Returns an error when the final cut is foreign, incomplete, still contains
/// any unowned payload, or cannot cover the exact Serve/Producer inventory.
pub(in crate::sumeragi::v2_lifecycle_coordinator) fn reconcile_complete_tip_serve_retirement(
    ledger: &LifecycleLedgerV1,
    recovered: AuthenticatedCertifiedServePayloadRecoveryCut,
) -> Result<CompleteTipServeRetirementReconciliationV1, LifecycleOpenError> {
    let records = decoded_records_by_key(ledger)?;
    let (serve_candidates, terminal_updates, retained, _replay_pairs) =
        resolve_serve_payloads(ledger.context(), ledger, &records, &recovered)?;
    validate_storage_only_serve_coverage(ledger, &records, &serve_candidates, &terminal_updates)?;
    let final_cut_ids = recovered
        .iter()
        .map(|payload| payload.id())
        .collect::<BTreeSet<_>>();
    if retained != final_cut_ids {
        return Err(LifecycleOpenErrorKind::InvalidRecovery(
            "CompleteTip final Serve cut contains an unowned payload",
        )
        .into());
    }
    let mut expected_terminal_updates = BTreeSet::new();
    let mut expected_terminal_serve_live_producers = BTreeSet::new();
    for serve in ledger
        .records()
        .iter()
        .filter(|record| record.work_class() == Some(LifecycleWorkClass::CertifiedServe))
    {
        let producer_ordinal =
            serve
                .ordinal()
                .checked_add(1)
                .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                    "CompleteTip Serve producer ordinal overflowed",
                ))?;
        let producer = ledger_record_at(ledger, producer_ordinal).ok_or(
            LifecycleOpenErrorKind::InvalidRecovery("CompleteTip Serve lost its adjacent Producer"),
        )?;
        let serve_terminal = serve
            .terminal()
            .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                "CompleteTip Serve terminal cannot be decoded",
            ))?;
        let producer_terminal =
            producer
                .terminal()
                .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                    "CompleteTip Producer terminal cannot be decoded",
                ))?;
        if serve_terminal.is_none() {
            expected_terminal_updates.insert(serve.ordinal());
        } else if producer_terminal.is_none() {
            expected_terminal_serve_live_producers.insert(serve.ordinal());
        }
    }
    let mut updates = BTreeMap::new();
    for terminal in terminal_updates {
        let serve = ledger_record_at(ledger, terminal.ordinal).ok_or(
            LifecycleOpenErrorKind::InvalidRecovery(
                "CompleteTip terminal update lost its Serve row",
            ),
        )?;
        let producer_ordinal =
            terminal
                .ordinal
                .checked_add(1)
                .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                    "CompleteTip terminal update producer ordinal overflowed",
                ))?;
        let producer = ledger_record_at(ledger, producer_ordinal).ok_or(
            LifecycleOpenErrorKind::InvalidRecovery(
                "CompleteTip terminal update lost its Producer row",
            ),
        )?;
        let update = CompleteTipServeTerminalUpdateV1 {
            context: ledger.context(),
            source_serve: serve.clone(),
            source_producer: producer.clone(),
            terminal,
        };
        if !update.exactly_matches_pair(serve, producer)
            || updates.insert(serve.ordinal(), update).is_some()
        {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "CompleteTip terminal Serve updates are not an exact ledger pair census",
            )
            .into());
        }
    }
    if updates.keys().copied().collect::<BTreeSet<_>>() != expected_terminal_updates {
        return Err(LifecycleOpenErrorKind::InvalidRecovery(
            "CompleteTip terminal updates do not cover every live Serve exactly",
        )
        .into());
    }
    let mut terminal_serve_live_producers = BTreeMap::new();
    for serve_ordinal in expected_terminal_serve_live_producers {
        let serve =
            ledger_record_at(ledger, serve_ordinal).expect("Serve ordinal came from ledger");
        let producer = ledger_record_at(
            ledger,
            serve_ordinal
                .checked_add(1)
                .expect("validated Serve producer ordinal"),
        )
        .expect("validated Serve retained its adjacent Producer");
        terminal_serve_live_producers.insert(serve_ordinal, (serve.clone(), producer.clone()));
    }
    Ok(CompleteTipServeRetirementReconciliationV1 {
        source_context: ledger.context(),
        source_frame_identity: ledger.frame_identity(),
        terminal_updates: updates,
        terminal_serve_live_producers,
        _authenticated_payloads: recovered,
    })
}
fn terminal_validate_no_successor_claim(
    context: LifecycleContext,
    record: &LifecycleLedgerRecordV1,
) -> Result<Option<TerminalValidateNoSuccessorClaim>, LifecycleRecoveryAssemblyErrorKind> {
    let ordinal = record.ordinal();
    let work_class =
        record
            .work_class()
            .ok_or(LifecycleRecoveryAssemblyErrorKind::InvalidDurableRecord {
                ordinal,
                field: "work class",
            })?;
    let stage = record
        .stage()
        .ok_or(LifecycleRecoveryAssemblyErrorKind::InvalidDurableRecord {
            ordinal,
            field: "stage",
        })?;
    let terminal =
        record
            .terminal()
            .ok_or(LifecycleRecoveryAssemblyErrorKind::InvalidDurableRecord {
                ordinal,
                field: "terminal",
            })?;
    let continuation =
        record
            .continuation()
            .ok_or(LifecycleRecoveryAssemblyErrorKind::InvalidDurableRecord {
                ordinal,
                field: "continuation",
            })?;
    if work_class != LifecycleWorkClass::Validate
        || terminal != Some(TerminalOutcome::Advanced)
        || continuation != DurableContinuation::AdvancedNoSuccessor
    {
        return Ok(None);
    }
    let key = record
        .key()
        .ok_or(LifecycleRecoveryAssemblyErrorKind::InvalidDurableRecord {
            ordinal,
            field: "key",
        })?;
    let payload = record.durable_payload().ok_or(
        LifecycleRecoveryAssemblyErrorKind::InvalidDurableRecord {
            ordinal,
            field: "payload",
        },
    )?;
    Ok(Some(TerminalValidateNoSuccessorClaim {
        context,
        ordinal,
        key,
        owner: record.owner(),
        reconstruction_source: record.reconstruction_source(),
        stage,
        payload,
        row_identity: record.exact_row_identity(),
    }))
}
fn recovered_control_broadcast_and_sign_records<'ledger>(
    ledger: &'ledger LifecycleLedgerV1,
    control: &AuthenticatedRecoveredWalControlProjection,
    pair: &RecoveredLifecycleSignedBroadcastAndSignLedgerProjectionV1,
    combined: &RecoveredLifecycleSignedBroadcastAndSignProjectionV1,
) -> Option<[&'ledger LifecycleLedgerRecordV1; 3]> {
    if pair.parent()
        != super::ledger::RecoveredLifecycleSignedBroadcastAndSignParentV1::ControlProposal
        || !pair.exactly_matches_ledger(ledger)
    {
        return None;
    }
    let record_at = |ordinal| {
        ledger
            .records()
            .binary_search_by_key(&ordinal, LifecycleLedgerRecordV1::ordinal)
            .ok()
            .and_then(|index| ledger.records().get(index))
    };
    let parent = record_at(pair.parent_ordinal())?;
    let broadcast = record_at(pair.broadcast_ordinal())?;
    let next_sign = record_at(pair.next_sign_ordinal())?;
    (control.exactly_matches_advanced_record(parent, pair.broadcast_ordinal())
        && combined.exactly_matches_fresh_records(ledger.context(), broadcast, next_sign))
    .then_some([parent, broadcast, next_sign])
}
fn recovered_phase_broadcast_and_next_sign_records<'ledger>(
    ledger: &'ledger LifecycleLedgerV1,
    projection: &AuthenticatedRecoveredWalSignProjection,
    pair: &RecoveredLifecycleSignedBroadcastAndSignLedgerProjectionV1,
    broadcast: &super::wal_recovery::RecoveredLifecycleSignedBroadcastProjectionV1,
    next_sign: &RecoveredLifecycleNextWalVoteCandidateProjectionV1,
) -> Option<[&'ledger LifecycleLedgerRecordV1; 4]> {
    let super::ledger::RecoveredLifecycleSignedBroadcastAndSignParentV1::PhasePrepare {
        validate_ordinal,
    } = pair.parent()
    else {
        return None;
    };
    if !projection.belongs_to_context(ledger.context()) || !pair.exactly_matches_ledger(ledger) {
        return None;
    }
    let record_at = |ordinal| {
        ledger
            .records()
            .binary_search_by_key(&ordinal, LifecycleLedgerRecordV1::ordinal)
            .ok()
            .and_then(|index| ledger.records().get(index))
    };
    let validate = record_at(validate_ordinal)?;
    let parent = record_at(pair.parent_ordinal())?;
    let broadcast_record = record_at(pair.broadcast_ordinal())?;
    let next_sign_record = record_at(pair.next_sign_ordinal())?;
    (validate.key() == Some(projection.parent_key())
        && parent.key() == Some(projection.child_key())
        && broadcast.exactly_matches_record(broadcast_record, parent.owner())
        && next_sign.exactly_matches_fresh_record(ledger.context(), next_sign_record))
    .then_some([validate, parent, broadcast_record, next_sign_record])
}
fn recovered_wal_exactly_owns_signed_broadcast(
    ledger: &LifecycleLedgerV1,
    recovered_wal: RecoveredWalStartupProjectionV1<'_>,
    record: &LifecycleLedgerRecordV1,
) -> bool {
    match recovered_wal {
        RecoveredWalStartupProjectionV1::PhaseBroadcast(projection, broadcast) => {
            projection.signed_broadcast_chain_is_exact(
                ledger.context(),
                ledger.records(),
                broadcast,
            ) && broadcast.exactly_matches_record(record, record.owner())
        }
        RecoveredWalStartupProjectionV1::PhaseBroadcastAndNextSign(
            projection,
            pair,
            broadcast,
            next_sign,
        ) => recovered_phase_broadcast_and_next_sign_records(
            ledger, projection, pair, broadcast, next_sign,
        )
        .is_some_and(|[_validate, _parent, broadcast_record, _next_sign]| {
            broadcast_record.ordinal() == record.ordinal()
        }),
        RecoveredWalStartupProjectionV1::ControlBroadcast(control, broadcast) => {
            ledger.records().iter().any(|parent| {
                let Some((_, child_ordinal)) = parent
                    .continuation()
                    .and_then(DurableContinuation::successor_parts)
                else {
                    return false;
                };
                child_ordinal == record.ordinal()
                    && control.exactly_matches_advanced_record(parent, child_ordinal)
                    && record.owner() == parent.owner()
                    && broadcast.exactly_matches_record(record, parent.owner())
            })
        }
        RecoveredWalStartupProjectionV1::ControlBroadcastAndSign(control, pair, combined) => {
            recovered_control_broadcast_and_sign_records(ledger, control, pair, combined)
                .is_some_and(|[_parent, broadcast_record, _next_sign]| {
                    broadcast_record.ordinal() == record.ordinal()
                })
        }
        RecoveredWalStartupProjectionV1::None
        | RecoveredWalStartupProjectionV1::PhaseVote(_)
        | RecoveredWalStartupProjectionV1::ControlSign(_)
        | RecoveredWalStartupProjectionV1::DecisionFetch(_)
        | RecoveredWalStartupProjectionV1::DecisionStore(_, _)
        | RecoveredWalStartupProjectionV1::DecisionValidate(_, _, _)
        | RecoveredWalStartupProjectionV1::DecisionApply(_)
        | RecoveredWalStartupProjectionV1::DecisionReleasedApply(_) => false,
    }
}
fn assemble_storage_only_candidates_and_terminal_validate_claims(
    ledger: &LifecycleLedgerV1,
    serve_payloads: &AuthenticatedCertifiedServePayloadRecoveryCut,
    recovered_wal: RecoveredWalStartupProjectionV1<'_>,
    mut body_pipeline: Option<&mut PreparedDurableCertifiedBodyPipelineStartupV1>,
) -> Result<
    (
        BTreeMap<LifecycleKey, CandidateAdmission>,
        BTreeMap<LifecycleKey, TerminalValidateNoSuccessorClaim>,
        PreparedLifecycleOutputRecoveryV1,
    ),
    LifecycleRecoveryAssemblyErrorKind,
> {
    let body_pipeline_startup = body_pipeline.is_some();
    let belongs_to_context = match recovered_wal {
        RecoveredWalStartupProjectionV1::None => true,
        RecoveredWalStartupProjectionV1::PhaseVote(projection) => {
            projection.belongs_to_context(ledger.context())
        }
        RecoveredWalStartupProjectionV1::PhaseBroadcast(projection, broadcast) => {
            projection.belongs_to_context(ledger.context())
                && broadcast
                    .candidate()
                    .replay_authority_is_exact(ledger.context())
        }
        RecoveredWalStartupProjectionV1::PhaseBroadcastAndNextSign(
            projection,
            pair,
            broadcast,
            next_sign,
        ) => recovered_phase_broadcast_and_next_sign_records(
            ledger, projection, pair, broadcast, next_sign,
        )
        .is_some(),
        RecoveredWalStartupProjectionV1::ControlSign(projection) => {
            projection.belongs_to_context(ledger.context())
        }
        RecoveredWalStartupProjectionV1::ControlBroadcast(control, broadcast) => {
            control.belongs_to_context(ledger.context())
                && broadcast
                    .candidate()
                    .replay_authority_is_exact(ledger.context())
        }
        RecoveredWalStartupProjectionV1::ControlBroadcastAndSign(control, pair, combined) => {
            control.belongs_to_context(ledger.context())
                && recovered_control_broadcast_and_sign_records(ledger, control, pair, combined)
                    .is_some()
        }
        RecoveredWalStartupProjectionV1::DecisionFetch(projection) => {
            projection.belongs_to_context(ledger.context())
        }
        RecoveredWalStartupProjectionV1::DecisionStore(fetch, store) => {
            fetch.belongs_to_context(ledger.context()) && store.context() == ledger.context()
        }
        RecoveredWalStartupProjectionV1::DecisionValidate(fetch, store, _validate) => {
            fetch.belongs_to_context(ledger.context()) && store.context() == ledger.context()
        }
        RecoveredWalStartupProjectionV1::DecisionApply(projection) => {
            projection.fetch().belongs_to_context(ledger.context())
        }
        RecoveredWalStartupProjectionV1::DecisionReleasedApply(projection) => {
            projection.fetch().belongs_to_context(ledger.context())
        }
    };
    if !belongs_to_context {
        return Err(LifecycleRecoveryAssemblyErrorKind::RecoveredWalSign(
            "installed projection belongs to another lifecycle context",
        ));
    }
    let mut candidates = BTreeMap::new();
    match recovered_wal {
        RecoveredWalStartupProjectionV1::ControlBroadcastAndSign(control, pair, combined) => {
            let Some([_parent, broadcast, next_sign]) =
                recovered_control_broadcast_and_sign_records(ledger, control, pair, combined)
            else {
                return Err(LifecycleRecoveryAssemblyErrorKind::RecoveredWalSign(
                    "recovered control Broadcast-and-Sign lost its exact durable pair",
                ));
            };
            if !combined.splice_candidates_from_records(
                ledger.context(),
                broadcast,
                next_sign,
                &mut candidates,
            ) {
                return Err(LifecycleRecoveryAssemblyErrorKind::RecoveredWalSign(
                    "recovered control Broadcast-and-Sign candidates could not splice atomically",
                ));
            }
        }
        RecoveredWalStartupProjectionV1::PhaseBroadcastAndNextSign(
            projection,
            pair,
            broadcast,
            next_sign,
        ) => {
            let Some([_validate, _parent, broadcast_record, next_sign_record]) =
                recovered_phase_broadcast_and_next_sign_records(
                    ledger, projection, pair, broadcast, next_sign,
                )
            else {
                return Err(LifecycleRecoveryAssemblyErrorKind::RecoveredWalSign(
                    "recovered split phase Broadcast-and-Sign lost its exact durable pair",
                ));
            };
            let broadcast_key = broadcast_record.key().ok_or(
                LifecycleRecoveryAssemblyErrorKind::RecoveredWalSign(
                    "recovered split phase Broadcast lost its semantic key",
                ),
            )?;
            if candidates.contains_key(&broadcast_key)
                || !next_sign.is_absent_from_candidates(&candidates)
                || !broadcast.splice_candidate_from_record(
                    broadcast_record,
                    broadcast_record.owner(),
                    &mut candidates,
                )
            {
                return Err(LifecycleRecoveryAssemblyErrorKind::RecoveredWalSign(
                    "recovered split phase Broadcast-and-Sign candidates could not splice atomically",
                ));
            }
            if !next_sign.splice_candidate_from_fresh_record(
                ledger.context(),
                next_sign_record,
                &mut candidates,
            ) {
                candidates.remove(&broadcast_key);
                return Err(LifecycleRecoveryAssemblyErrorKind::RecoveredWalSign(
                    "recovered split phase Broadcast-and-Sign candidates could not splice atomically",
                ));
            }
        }
        _ => {}
    }
    let lifecycle_outputs = match body_pipeline.as_ref() {
        Some(pipeline) => {
            PreparedLifecycleOutputRecoveryV1::assemble(ledger, pipeline.verified(), recovered_wal)?
        }
        None => PreparedLifecycleOutputRecoveryV1 {
            entries: BTreeMap::new(),
        },
    };
    let mut claims = BTreeMap::new();
    for record in ledger.records() {
        match classify_storage_only_record(record) {
            Ok(()) => {}
            Err(LifecycleRecoveryAssemblyErrorKind::MissingTerminalValidateOutcome { .. }) => {
                let Some(claim) = terminal_validate_no_successor_claim(ledger.context(), record)?
                else {
                    return Err(LifecycleRecoveryAssemblyErrorKind::InvalidDurableRecord {
                        ordinal: record.ordinal(),
                        field: "terminal Validate recovery claim",
                    });
                };
                if claims.insert(claim.key, claim).is_some() {
                    return Err(LifecycleRecoveryAssemblyErrorKind::InvalidDurableRecord {
                        ordinal: record.ordinal(),
                        field: "duplicate terminal Validate recovery key",
                    });
                }
            }
            Err(
                kind @ LifecycleRecoveryAssemblyErrorKind::MissingDurableRecoveryAuthority {
                    work_class,
                    ..
                },
            ) => {
                let admitted_recovered_wal = match recovered_wal {
                    RecoveredWalStartupProjectionV1::None => false,
                    RecoveredWalStartupProjectionV1::PhaseVote(projection)
                        if record.key() == Some(projection.child_key()) =>
                    {
                        projection.insert_repaired_child_from_record(
                            ledger.context(),
                            record,
                            &mut candidates,
                        )
                    }
                    RecoveredWalStartupProjectionV1::PhaseBroadcast(_projection, broadcast)
                        if work_class == LifecycleWorkClass::Broadcast =>
                    {
                        broadcast.splice_candidate_from_record(
                            record,
                            record.owner(),
                            &mut candidates,
                        )
                    }
                    RecoveredWalStartupProjectionV1::PhaseBroadcastAndNextSign(
                        _projection,
                        pair,
                        broadcast,
                        next_sign,
                    ) if record.ordinal() == pair.broadcast_ordinal()
                        || record.ordinal() == pair.next_sign_ordinal() =>
                    {
                        broadcast.owns_spliced_candidate(&candidates)
                            && next_sign.owns_spliced_candidate(&candidates)
                    }
                    RecoveredWalStartupProjectionV1::ControlSign(projection)
                        if projection.names_record(record) =>
                    {
                        projection.splice_candidate_from_record(record, &mut candidates)
                    }
                    RecoveredWalStartupProjectionV1::ControlBroadcast(_control, broadcast)
                        if work_class == LifecycleWorkClass::Broadcast =>
                    {
                        broadcast.splice_candidate_from_record(
                            record,
                            record.owner(),
                            &mut candidates,
                        )
                    }
                    RecoveredWalStartupProjectionV1::ControlBroadcastAndSign(
                        _control,
                        pair,
                        combined,
                    ) if record.ordinal() == pair.broadcast_ordinal()
                        || record.ordinal() == pair.next_sign_ordinal() =>
                    {
                        combined.owns_spliced_candidates(&candidates)
                    }
                    RecoveredWalStartupProjectionV1::DecisionFetch(projection)
                        if projection.names_record(record) =>
                    {
                        projection.splice_candidate_from_record(record, &mut candidates)
                    }
                    RecoveredWalStartupProjectionV1::DecisionStore(fetch, store)
                        if work_class == LifecycleWorkClass::Store =>
                    {
                        recovered_decision_store_chain_records(ledger, fetch, store).is_some_and(
                            |[parent, child]| {
                                child.ordinal() == record.ordinal()
                                    && store.splice_candidate_from_record(
                                        record,
                                        parent.owner(),
                                        &mut candidates,
                                    )
                            },
                        )
                    }
                    RecoveredWalStartupProjectionV1::DecisionValidate(fetch, store, validate)
                        if work_class == LifecycleWorkClass::Validate =>
                    {
                        recovered_decision_validate_chain_records(ledger, fetch, store, validate)
                            .is_some_and(|[_fetch, store_record, validate_record]| {
                                validate_record.ordinal() == record.ordinal()
                                    && validate.splice_candidate_from_records(
                                        store_record.owner(),
                                        store_record,
                                        validate_record,
                                        &mut candidates,
                                    )
                            })
                    }
                    RecoveredWalStartupProjectionV1::DecisionApply(projection)
                        if work_class == LifecycleWorkClass::Apply =>
                    {
                        splice_recovered_decision_apply_candidate(
                            ledger,
                            projection,
                            record,
                            &mut candidates,
                        )
                    }
                    RecoveredWalStartupProjectionV1::DecisionReleasedApply(projection)
                        if work_class == LifecycleWorkClass::Apply =>
                    {
                        projection
                            .lineage()
                            .splice_standalone_apply_candidate_from_record(
                                ledger.context(),
                                record,
                                &mut candidates,
                            )
                    }
                    RecoveredWalStartupProjectionV1::PhaseVote(_)
                    | RecoveredWalStartupProjectionV1::PhaseBroadcast(_, _)
                    | RecoveredWalStartupProjectionV1::PhaseBroadcastAndNextSign(_, _, _, _)
                    | RecoveredWalStartupProjectionV1::ControlSign(_)
                    | RecoveredWalStartupProjectionV1::ControlBroadcast(_, _)
                    | RecoveredWalStartupProjectionV1::ControlBroadcastAndSign(_, _, _)
                    | RecoveredWalStartupProjectionV1::DecisionFetch(_)
                    | RecoveredWalStartupProjectionV1::DecisionStore(_, _)
                    | RecoveredWalStartupProjectionV1::DecisionValidate(_, _, _)
                    | RecoveredWalStartupProjectionV1::DecisionApply(_)
                    | RecoveredWalStartupProjectionV1::DecisionReleasedApply(_) => false,
                };
                if admitted_recovered_wal {
                    continue;
                }
                if lifecycle_outputs.owns_record(record) {
                    continue;
                }
                if matches!(
                    work_class,
                    LifecycleWorkClass::Fetch
                        | LifecycleWorkClass::Store
                        | LifecycleWorkClass::Validate
                ) && body_pipeline
                    .as_ref()
                    .is_some_and(|pipeline| pipeline.contains_live_ordinal(record.ordinal()))
                {
                    continue;
                }
                return Err(kind);
            }
            Err(kind) => return Err(kind),
        }
    }
    if let Some(body_pipeline) = body_pipeline.as_mut()
        && !body_pipeline.splice_candidates(ledger, &mut candidates)
    {
        return Err(
            LifecycleRecoveryAssemblyErrorKind::DurableCertifiedBodyPipeline(
                "the frame-bound all-row census did not splice exactly once",
            ),
        );
    }
    if !lifecycle_outputs.splice_candidates(&mut candidates) {
        let (ordinal, output) = lifecycle_outputs
            .entries
            .first_key_value()
            .expect("failed lifecycle output splice retains at least one entry");
        return Err(
            LifecycleRecoveryAssemblyErrorKind::InvalidLifecycleOutputRecovery {
                ordinal: *ordinal,
                work_class: output.candidate().work_class,
                stage: output.candidate().stage,
            },
        );
    }
    match recovered_wal {
        RecoveredWalStartupProjectionV1::PhaseVote(projection) => {
            if !projection.owns_spliced_candidates(&candidates) {
                return Err(LifecycleRecoveryAssemblyErrorKind::RecoveredWalSign(
                    "repaired frame has no exact live installed phase-vote Sign",
                ));
            }
            if !projection.repaired_pair_is_exact(ledger.context(), ledger.records()) {
                return Err(LifecycleRecoveryAssemblyErrorKind::RecoveredWalSign(
                    "repaired frame lost the exact terminal Validate parent or typed Sign edge",
                ));
            }
        }
        RecoveredWalStartupProjectionV1::PhaseBroadcast(projection, broadcast) => {
            if !broadcast.owns_spliced_candidate(&candidates)
                || !projection.signed_broadcast_chain_is_exact(
                    ledger.context(),
                    ledger.records(),
                    broadcast,
                )
            {
                return Err(LifecycleRecoveryAssemblyErrorKind::RecoveredWalSign(
                    "repaired frame lost its exact phase-vote Broadcast chain",
                ));
            }
        }
        RecoveredWalStartupProjectionV1::PhaseBroadcastAndNextSign(
            projection,
            pair,
            broadcast,
            next_sign,
        ) => {
            if recovered_phase_broadcast_and_next_sign_records(
                ledger, projection, pair, broadcast, next_sign,
            )
            .is_none()
                || !broadcast.owns_spliced_candidate(&candidates)
                || !next_sign.owns_spliced_candidate(&candidates)
            {
                return Err(LifecycleRecoveryAssemblyErrorKind::RecoveredWalSign(
                    "repaired frame has no exact split recovered phase Broadcast-and-Sign pair",
                ));
            }
        }
        RecoveredWalStartupProjectionV1::ControlSign(projection) => {
            if !projection.owns_spliced_candidate(&candidates) {
                return Err(LifecycleRecoveryAssemblyErrorKind::RecoveredWalSign(
                    "repaired frame has no exact live installed control Sign",
                ));
            }
        }
        RecoveredWalStartupProjectionV1::ControlBroadcast(control, broadcast) => {
            let exact_chain = ledger.records().iter().any(|parent| {
                let Some((_, child_ordinal)) = parent
                    .continuation()
                    .and_then(DurableContinuation::successor_parts)
                else {
                    return false;
                };
                control.exactly_matches_advanced_record(parent, child_ordinal)
                    && ledger
                        .records()
                        .binary_search_by_key(&child_ordinal, |record| record.ordinal())
                        .ok()
                        .and_then(|index| ledger.records().get(index))
                        .is_some_and(|child| {
                            child.owner() == parent.owner()
                                && broadcast.exactly_matches_record(child, parent.owner())
                        })
            });
            if !exact_chain || !broadcast.owns_spliced_candidate(&candidates) {
                return Err(LifecycleRecoveryAssemblyErrorKind::RecoveredWalSign(
                    "repaired frame has no exact live recovered control Broadcast",
                ));
            }
        }
        RecoveredWalStartupProjectionV1::ControlBroadcastAndSign(control, pair, combined) => {
            if recovered_control_broadcast_and_sign_records(ledger, control, pair, combined)
                .is_none()
                || !combined.owns_spliced_candidates(&candidates)
            {
                return Err(LifecycleRecoveryAssemblyErrorKind::RecoveredWalSign(
                    "repaired frame has no exact recovered control Broadcast-and-Sign pair",
                ));
            }
        }
        RecoveredWalStartupProjectionV1::DecisionFetch(projection) => {
            if !projection.owns_spliced_candidate(&candidates) {
                return Err(LifecycleRecoveryAssemblyErrorKind::RecoveredWalSign(
                    "repaired frame has no exact live installed Decision Fetch",
                ));
            }
        }
        RecoveredWalStartupProjectionV1::DecisionStore(fetch, store) => {
            if !store.owns_spliced_candidate(&candidates)
                || recovered_decision_store_chain_records(ledger, fetch, store).is_none()
            {
                return Err(LifecycleRecoveryAssemblyErrorKind::RecoveredWalSign(
                    "repaired frame has no exact live recovered Decision Store",
                ));
            }
        }
        RecoveredWalStartupProjectionV1::DecisionValidate(fetch, store, validate) => {
            if !validate.owns_spliced_candidate(&candidates)
                || recovered_decision_validate_chain_records(ledger, fetch, store, validate)
                    .is_none()
            {
                return Err(LifecycleRecoveryAssemblyErrorKind::RecoveredWalSign(
                    "repaired frame has no exact live recovered Decision Validate",
                ));
            }
        }
        RecoveredWalStartupProjectionV1::DecisionApply(projection) => {
            if !projection
                .lineage()
                .owns_spliced_apply_candidate(&candidates)
                || !recovered_decision_apply_chain_is_exact(ledger, projection)
            {
                return Err(LifecycleRecoveryAssemblyErrorKind::RecoveredWalSign(
                    "repaired frame has no exact live recovered Decision Apply",
                ));
            }
        }
        RecoveredWalStartupProjectionV1::DecisionReleasedApply(projection) => {
            if !projection
                .lineage()
                .owns_spliced_apply_candidate(&candidates)
                || !recovered_released_decision_apply_chain_is_exact(ledger, projection)
            {
                return Err(LifecycleRecoveryAssemblyErrorKind::RecoveredWalSign(
                    "repaired frame has no exact standalone recovered Decision Apply",
                ));
            }
        }
        RecoveredWalStartupProjectionV1::None => {
            if candidates.values().any(|candidate| {
                !matches!(candidate.work_class, LifecycleWorkClass::Fetch)
                    && !(body_pipeline_startup
                        && matches!(
                            candidate.work_class,
                            LifecycleWorkClass::Store | LifecycleWorkClass::Validate
                        ))
                    && !matches!(
                        candidate.work_class,
                        LifecycleWorkClass::Broadcast
                            | LifecycleWorkClass::EquivocationReport
                            | LifecycleWorkClass::InvalidBodyReport
                    )
            }) {
                return Err(LifecycleRecoveryAssemblyErrorKind::RecoveredWalSign(
                    "storage-only assembly created non-body work without installed authority",
                ));
            }
        }
    }
    validate_storage_only_serve_recovery(ledger, serve_payloads)?;
    Ok((candidates, claims, lifecycle_outputs))
}
fn recovered_decision_store_chain_records<'ledger>(
    ledger: &'ledger LifecycleLedgerV1,
    fetch: &AuthenticatedRecoveredWalDecisionFetchProjection,
    store: &RecoveredDecisionFetchStoreProjectionV1,
) -> Option<[&'ledger LifecycleLedgerRecordV1; 2]> {
    let (fetch_ordinal, store_ordinal) = ledger
        .authenticate_recovered_decision_fetch_store(fetch, store)
        .ok()?;
    let record_at = |ordinal| {
        ledger
            .records()
            .binary_search_by_key(&ordinal, LifecycleLedgerRecordV1::ordinal)
            .ok()
            .and_then(|index| ledger.records().get(index))
    };
    Some([record_at(fetch_ordinal)?, record_at(store_ordinal)?])
}
fn recovered_decision_validate_chain_records<'ledger>(
    ledger: &'ledger LifecycleLedgerV1,
    fetch: &AuthenticatedRecoveredWalDecisionFetchProjection,
    store: &RecoveredDecisionFetchStoreProjectionV1,
    validate: &RecoveredDecisionValidateProjectionV1,
) -> Option<[&'ledger LifecycleLedgerRecordV1; 3]> {
    let (fetch_ordinal, store_ordinal, validate_ordinal) = ledger
        .authenticate_recovered_decision_store_validate(fetch, store, validate)
        .ok()?;
    let record_at = |ordinal| {
        ledger
            .records()
            .binary_search_by_key(&ordinal, LifecycleLedgerRecordV1::ordinal)
            .ok()
            .and_then(|index| ledger.records().get(index))
    };
    Some([
        record_at(fetch_ordinal)?,
        record_at(store_ordinal)?,
        record_at(validate_ordinal)?,
    ])
}
fn recovered_decision_apply_chain_records<'ledger>(
    ledger: &'ledger LifecycleLedgerV1,
    projection: &crate::sumeragi::v2::RecoveredDecisionApplyStagedStorageV1,
) -> Option<[&'ledger LifecycleLedgerRecordV1; 4]> {
    let mut fetches = ledger
        .records()
        .iter()
        .filter(|record| projection.fetch().names_record(record));
    let fetch = fetches.next()?;
    if fetches.next().is_some() {
        return None;
    }
    let (DurableContinuationEdge::FetchToStore, store_ordinal) = fetch
        .continuation()
        .and_then(DurableContinuation::successor_parts)?
    else {
        return None;
    };
    let record_at = |ordinal| {
        ledger
            .records()
            .binary_search_by_key(&ordinal, LifecycleLedgerRecordV1::ordinal)
            .ok()
            .and_then(|index| ledger.records().get(index))
    };
    let store = record_at(store_ordinal)?;
    let (DurableContinuationEdge::StoreToValidate, validate_ordinal) = store
        .continuation()
        .and_then(DurableContinuation::successor_parts)?
    else {
        return None;
    };
    let validate = record_at(validate_ordinal)?;
    let (DurableContinuationEdge::ValidateToApply, apply_ordinal) = validate
        .continuation()
        .and_then(DurableContinuation::successor_parts)?
    else {
        return None;
    };
    let apply = record_at(apply_ordinal)?;
    let owner = fetch.owner();
    (ledger
        .records()
        .iter()
        .filter(|record| record.owner() == owner)
        .count()
        == 4
        && projection
            .fetch()
            .exactly_matches_advanced_apply_parent(fetch, store_ordinal)
        && projection
            .lineage()
            .exactly_matches_successor_records(owner, store, validate, apply))
    .then_some([fetch, store, validate, apply])
}
fn recovered_decision_apply_chain_is_exact(
    ledger: &LifecycleLedgerV1,
    projection: &crate::sumeragi::v2::RecoveredDecisionApplyStagedStorageV1,
) -> bool {
    recovered_decision_apply_chain_records(ledger, projection).is_some()
}
fn recovered_released_decision_apply_chain_is_exact(
    ledger: &LifecycleLedgerV1,
    projection: &crate::sumeragi::v2::RecoveredDecisionApplyStagedStorageV1,
) -> bool {
    ledger
        .stage_recovered_released_decision_apply(projection)
        .is_ok_and(|(staged, _apply_ordinal, changed)| !changed && staged == *ledger)
}
fn splice_recovered_decision_apply_candidate(
    ledger: &LifecycleLedgerV1,
    projection: &crate::sumeragi::v2::RecoveredDecisionApplyStagedStorageV1,
    current: &LifecycleLedgerRecordV1,
    candidates: &mut BTreeMap<LifecycleKey, CandidateAdmission>,
) -> bool {
    let Some([_fetch, store, validate, apply]) =
        recovered_decision_apply_chain_records(ledger, projection)
    else {
        return false;
    };
    apply.ordinal() == current.ordinal()
        && projection.lineage().splice_apply_candidate_from_records(
            apply.owner(),
            store,
            validate,
            apply,
            candidates,
        )
}
fn validate_storage_only_serve_recovery(
    ledger: &LifecycleLedgerV1,
    serve_payloads: &AuthenticatedCertifiedServePayloadRecoveryCut,
) -> Result<(), LifecycleRecoveryAssemblyErrorKind> {
    let records = decoded_records_by_key(ledger)
        .map_err(LifecycleRecoveryAssemblyErrorKind::CertifiedServe)?;
    let (serve_candidates, terminal_updates, _retained, _replay_pairs) =
        resolve_serve_payloads(ledger.context(), ledger, &records, serve_payloads)
            .map_err(LifecycleRecoveryAssemblyErrorKind::CertifiedServe)?;
    validate_storage_only_serve_coverage(ledger, &records, &serve_candidates, &terminal_updates)
        .map_err(LifecycleRecoveryAssemblyErrorKind::CertifiedServe)
}
/// Recheck the retained post-prune Serve cut against one exact owner ledger.
pub(super) fn authenticated_serve_payloads_match_ledger(
    ledger: &LifecycleLedgerV1,
    serve_payloads: &AuthenticatedCertifiedServePayloadRecoveryCut,
) -> bool {
    validate_storage_only_serve_recovery(ledger, serve_payloads).is_ok()
}
// STORAGE_ONLY_LIFECYCLE_RECOVERY_CLASSIFIER_BEGIN
fn classify_storage_only_record(
    record: &LifecycleLedgerRecordV1,
) -> Result<(), LifecycleRecoveryAssemblyErrorKind> {
    let ordinal = record.ordinal();
    let work_class =
        record
            .work_class()
            .ok_or(LifecycleRecoveryAssemblyErrorKind::InvalidDurableRecord {
                ordinal,
                field: "work class",
            })?;
    let stage = record
        .stage()
        .ok_or(LifecycleRecoveryAssemblyErrorKind::InvalidDurableRecord {
            ordinal,
            field: "stage",
        })?;
    let terminal =
        record
            .terminal()
            .ok_or(LifecycleRecoveryAssemblyErrorKind::InvalidDurableRecord {
                ordinal,
                field: "terminal",
            })?;
    let continuation =
        record
            .continuation()
            .ok_or(LifecycleRecoveryAssemblyErrorKind::InvalidDurableRecord {
                ordinal,
                field: "continuation",
            })?;
    let stage_work_class = match stage.kind() {
        LifecycleStageKind::SignProposal => LifecycleWorkClass::SignProposal,
        LifecycleStageKind::SignPrepareVote | LifecycleStageKind::SignCommitVote => {
            LifecycleWorkClass::SignVote
        }
        LifecycleStageKind::SignTimeoutVote => LifecycleWorkClass::SignTimeout,
        LifecycleStageKind::FetchBody => LifecycleWorkClass::Fetch,
        LifecycleStageKind::StoreBody => LifecycleWorkClass::Store,
        LifecycleStageKind::ValidateBody => LifecycleWorkClass::Validate,
        LifecycleStageKind::ApplyDecision => LifecycleWorkClass::Apply,
        LifecycleStageKind::BroadcastProposal
        | LifecycleStageKind::BroadcastPrepareVote
        | LifecycleStageKind::BroadcastCommitVote
        | LifecycleStageKind::BroadcastPrepareQc
        | LifecycleStageKind::BroadcastCommitQc
        | LifecycleStageKind::BroadcastTimeoutVote
        | LifecycleStageKind::BroadcastTc => LifecycleWorkClass::Broadcast,
        LifecycleStageKind::EnterView => LifecycleWorkClass::EnterView,
        LifecycleStageKind::ReportProposalEquivocation
        | LifecycleStageKind::ReportVoteEquivocation
        | LifecycleStageKind::ReportTimeoutEquivocation => LifecycleWorkClass::EquivocationReport,
        LifecycleStageKind::ReportInvalidBody => LifecycleWorkClass::InvalidBodyReport,
        LifecycleStageKind::CertifiedServe => LifecycleWorkClass::CertifiedServe,
        LifecycleStageKind::ProducerTurn => LifecycleWorkClass::ProducerTurn,
    };
    if stage_work_class != work_class {
        return Err(
            LifecycleRecoveryAssemblyErrorKind::InvalidDurableRecordShape {
                ordinal,
                work_class,
                stage,
            },
        );
    }
    if work_class == LifecycleWorkClass::Validate
        && terminal == Some(TerminalOutcome::Advanced)
        && continuation == DurableContinuation::AdvancedNoSuccessor
    {
        return Err(
            LifecycleRecoveryAssemblyErrorKind::MissingTerminalValidateOutcome { ordinal, stage },
        );
    }
    match work_class {
        LifecycleWorkClass::CertifiedServe | LifecycleWorkClass::ProducerTurn => Ok(()),
        LifecycleWorkClass::SignProposal
        | LifecycleWorkClass::SignVote
        | LifecycleWorkClass::SignTimeout
        | LifecycleWorkClass::Fetch
        | LifecycleWorkClass::Store
        | LifecycleWorkClass::Validate
        | LifecycleWorkClass::Apply
        | LifecycleWorkClass::Broadcast
        | LifecycleWorkClass::EnterView
        | LifecycleWorkClass::EquivocationReport
        | LifecycleWorkClass::InvalidBodyReport => terminal.map_or_else(
            || {
                Err(
                    LifecycleRecoveryAssemblyErrorKind::MissingDurableRecoveryAuthority {
                        ordinal,
                        work_class,
                        stage,
                    },
                )
            },
            |_| Ok(()),
        ),
    }
}
// STORAGE_ONLY_LIFECYCLE_RECOVERY_CLASSIFIER_END
fn validate_storage_only_serve_coverage(
    ledger: &LifecycleLedgerV1,
    records: &BTreeMap<LifecycleKey, &LifecycleLedgerRecordV1>,
    candidates: &[CandidateAdmission],
    terminal_updates: &[TerminalUpdate],
) -> Result<(), LifecycleOpenError> {
    let mut covered_serves = BTreeSet::new();
    let mut covered_producers = BTreeSet::new();
    let terminal_updates = terminal_updates
        .iter()
        .map(|update| (update.ordinal, update))
        .collect::<BTreeMap<_, _>>();
    for candidate in candidates {
        if candidate.work_class != LifecycleWorkClass::CertifiedServe {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "storage-only payload projection produced non-Serve work",
            )
            .into());
        }
        let record =
            records
                .get(&candidate.key)
                .copied()
                .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                    "storage-only Serve projection has no durable owner",
                ))?;
        let terminal_update = terminal_updates.get(&record.ordinal()).copied();
        validate_candidate_record(ledger.context(), record, candidate, terminal_update)?;
        if !covered_serves.insert(record.ordinal()) {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "storage-only Serve projection duplicated one ledger row",
            )
            .into());
        }
        let producer_ordinal =
            record
                .ordinal()
                .checked_add(1)
                .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                    "storage-only Serve producer ordinal overflowed",
                ))?;
        let producer_record = ledger_record_at(ledger, producer_ordinal).ok_or(
            LifecycleOpenErrorKind::InvalidRecovery(
                "storage-only Serve projection lost its adjacent producer",
            ),
        )?;
        let producer =
            candidate
                .producer_turn
                .as_ref()
                .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                    "storage-only Serve projection lacks its producer companion",
                ))?;
        let replay_matches = terminal_update.map_or_else(
            || producer_record.replay_matches_producer(producer),
            |update| {
                update
                    .replay
                    .exactly_matches_recovered_candidate(ledger.context(), candidate)
                    && record.replay_is_exact_pending_predecessor(
                        ledger.context(),
                        producer_record,
                        &update.replay,
                    )
            },
        );
        if producer_record.key() != Some(producer.key)
            || producer_record.owner() != record.owner()
            || producer_record.work_class() != Some(LifecycleWorkClass::ProducerTurn)
            || producer_record.stage() != Some(producer.stage)
            || producer_record.reconstruction_source() != producer.reconstruction_source
            || !replay_matches
        {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "storage-only producer projection changed durable semantics",
            )
            .into());
        }
        if producer_record.terminal().flatten().is_none()
            && !covered_producers.insert(producer_ordinal)
        {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "storage-only producer projection duplicated one ledger row",
            )
            .into());
        }
    }
    let mut expected_serves = BTreeSet::new();
    let mut expected_producers = BTreeSet::new();
    for record in ledger.records() {
        let terminal = record
            .terminal()
            .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                "storage-only coverage cannot decode durable terminal",
            ))?;
        match record.work_class() {
            Some(LifecycleWorkClass::CertifiedServe) => {
                let producer_is_live = record
                    .ordinal()
                    .checked_add(1)
                    .and_then(|ordinal| ledger_record_at(ledger, ordinal))
                    .is_some_and(|producer| producer.terminal().flatten().is_none());
                if terminal.is_none() || producer_is_live {
                    expected_serves.insert(record.ordinal());
                }
            }
            Some(LifecycleWorkClass::ProducerTurn) if terminal.is_none() => {
                expected_producers.insert(record.ordinal());
            }
            Some(
                LifecycleWorkClass::SignProposal
                | LifecycleWorkClass::SignVote
                | LifecycleWorkClass::SignTimeout
                | LifecycleWorkClass::Fetch
                | LifecycleWorkClass::Store
                | LifecycleWorkClass::Validate
                | LifecycleWorkClass::Apply
                | LifecycleWorkClass::Broadcast
                | LifecycleWorkClass::EnterView
                | LifecycleWorkClass::EquivocationReport
                | LifecycleWorkClass::InvalidBodyReport
                | LifecycleWorkClass::ProducerTurn,
            ) => {}
            None => {
                return Err(LifecycleOpenErrorKind::InvalidRecovery(
                    "storage-only coverage cannot decode durable work class",
                )
                .into());
            }
        }
    }
    if covered_serves != expected_serves || covered_producers != expected_producers {
        return Err(LifecycleOpenErrorKind::InvalidRecovery(
            "storage-only Serve/producer recovery coverage is not exact",
        )
        .into());
    }
    Ok(())
}
include!("v2_lifecycle_open_output_recovery.rs");
include!("v2_lifecycle_open_recovery_tests.rs");
