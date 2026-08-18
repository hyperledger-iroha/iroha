//! Deterministic lifecycle scheduling core for Sumeragi v2.
//!
//! This pure reducer is the staged replacement for the distributed
//! Serve/witness/latch scheduling state.
use std::collections::{BTreeMap, BTreeSet};

#[cfg(test)]
use super::{v2, v2_runner};

#[path = "v2_lifecycle_authority.rs"]
mod authority;
#[path = "v2_lifecycle_coordinator_support.rs"]
mod coordinator_support;
#[cfg(test)]
pub(crate) use coordinator_support::{
    reviewed_lifecycle_ledger_source_for_test, reviewed_lifecycle_work_registry_source_for_test,
    reviewed_v2_adapter_source_for_test, reviewed_v2_runtime_source_for_test, run_source_contract,
    source_contract_test,
};
/// Sealed coordinator cuts for adjacent direct body-pipeline transitions.
#[path = "v2_lifecycle_body_pipeline_transition.rs"]
#[cfg_attr(not(test), allow(dead_code))]
mod body_pipeline_transition;
/// Atomic seam between digest-only admission and process-local concrete work.
#[path = "v2_lifecycle_concrete_admission.rs"]
#[cfg_attr(not(test), allow(dead_code))]
mod concrete_admission;
/// Sealed fair-ingress queue cut for future composite rank capture.
#[path = "v2_lifecycle_ingress_position.rs"]
#[cfg_attr(not(test), allow(dead_code))]
mod ingress_position;
/// Consuming production launch from the recovered owner into runtime and I/O.
#[path = "v2_lifecycle_launch.rs"]
#[cfg_attr(not(test), allow(dead_code))]
mod launch;
/// Durable lifecycle ledger, sealed behind the coordinator authority.
#[path = "v2_lifecycle_ledger.rs"]
mod ledger;
#[path = "v2_lifecycle_open.rs"]
mod open;
#[path = "v2_lifecycle_projection.rs"]
mod projection;
/// Closed codec prerequisite for restart-authenticated lifecycle replay.
#[path = "v2_lifecycle_replay_authority.rs"]
#[cfg_attr(not(test), allow(dead_code))]
mod replay_authority;
/// Sealed production planner-input authentication.
#[path = "v2_lifecycle_scheduler_inputs.rs"]
#[cfg_attr(not(test), allow(dead_code))]
mod scheduler_inputs;
/// Pure lifecycle schema and value definitions.
#[path = "v2_lifecycle_schema.rs"]
mod schema;
/// Sealed executor join for exact fair-ingress selector debt.
#[path = "v2_lifecycle_selector.rs"]
#[cfg_attr(not(test), allow(dead_code))]
mod selector;
#[path = "v2_lifecycle_settlement.rs"]
mod settlement;
/// Restart-only join from authenticated WAL replay into typed lifecycle work.
#[path = "v2_lifecycle_wal_recovery.rs"]
#[cfg_attr(not(test), allow(dead_code))]
mod wal_recovery;
/// Process-local concrete work remains outside the logical scheduler state.
// The lifecycle-owned non-Pending runner retains this registry beside the
// serialized runtime and services for the complete height.
#[path = "v2_lifecycle_work_registry.rs"]
#[cfg_attr(not(test), allow(dead_code))]
mod work_registry;
use authority::AuthenticatedEpisodeAuthority;
#[cfg(test)]
pub(crate) use authority::RolloverSnapshot;
use body_pipeline_transition::durable_validate_payload_is_exact;
pub(in crate::sumeragi) use body_pipeline_transition::{
    SealedInvalidBodyReportProjectionPermit, SealedValidateSignProjectionPermit,
};
pub(crate) use concrete_admission::LifecycleWorkRegistryHolder;
#[cfg(test)]
pub(in crate::sumeragi) use launch::ProductionPreparedCertifiedServeTestSettlementV1;
#[allow(unused_imports)]
pub(in crate::sumeragi) use launch::{
    ActivatedProductionLifecycleV1, FinalizedProductionLifecycleRolloverV1,
    LaunchedProductionLifecycleV1, PendingKuraActivatedProductionLifecycleV1,
    PendingKuraProductionLifecycleV1, PreparedPendingKuraLaneRecoveryV1,
    ProductionLifecycleActivationErrorV1, ProductionLifecycleCleanupReadyV1,
    ProductionLifecycleCompletionSelectionV1, ProductionLifecycleCompletionTurnV1,
    ProductionLifecycleFinalizationErrorV1, ProductionLifecycleFinalizationOutcomeV1,
    ProductionLifecycleIngressSelectionV1, ProductionLifecycleIngressTurnV1,
    ProductionLifecycleLaunchErrorV1, ProductionLifecycleLaunchInputsV1,
    ProductionLifecycleLiveClockActivationPermitV1, ProductionLifecycleOutputRolloverPermitV1,
    ProductionLifecyclePostOutputHandoffV1, ProductionLifecyclePreActivationErrorV1,
    ProductionLifecyclePreparedLocalProposalStateV1,
    ProductionLifecycleServeRetirementAuthenticationPermitV1, ProductionLifecycleShutdownErrorV1,
    ProductionPendingKuraApplyInstallErrorV1, ProductionPendingKuraApplyRecoveryErrorV1,
    ProductionPendingKuraApplyRecoveryProgressV1, ProductionPreparedOrdinaryIngressTurnV1,
    ProductionRecoveredDecisionApplyCompletionErrorV1,
    ProductionRecoveredDecisionApplyCompletionV1, ProductionRecoveredDecisionApplyRetryV1,
    ProductionRecoveredDecisionFetchStoreSettlementFailureV1,
    ProductionRecoveredDecisionFetchStoreSettlementV1,
    ProductionRecoveredLifecycleProposalBroadcastAndSignSettlementV1,
    ProductionRecoveredLifecycleSignBroadcastPreparationV1,
    ProductionRecoveredLifecycleSignBroadcastSettlementV1,
    ProductionRecoveredLifecycleSignCompletionSelectionV1,
    ProductionRecoveredLifecycleVoteBroadcastAndSignSettlementV1,
    ProductionV2CompletionObserverActivationPermitV1, RetainedRecoveredDecisionApplyDeferredV1,
};
pub(crate) use ledger::AuthenticatedRecoveredWalValidateLedgerParent;
pub(crate) use ledger::ProductionLifecycleStartupErrorV1;
#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use ledger::WalVoteLedgerRepairTestSummary;
pub(in crate::sumeragi) use ledger::{
    AuthenticatedCompleteTipPredecessorStorageV1, CompleteTipPredecessorStorageErrorV1,
    LaunchedRecoveredCompleteTipSuccessorLifecycleV1, LifecycleLedgerV1,
    RetiredRecoveredCompleteTipActivationAuthorityV1, open_complete_tip_predecessor_storage,
};
#[cfg(all(test, feature = "bls"))]
/// Run the two release-bound CompleteTip disk-retirement regressions.
pub(crate) fn run_complete_tip_retirement_release_regressions() {
    ledger::tests::durable_ready_fetch_recovery::complete_tip_retirement_survives_completed_serve_body_cleanup_with_live_work();
    ledger::tests::durable_ready_fetch_recovery::complete_tip_retirement_binds_only_the_exact_unlaunched_successor_owner();
}
#[cfg(all(test, feature = "bls"))]
/// Build one exact retired CompleteTip/H+1 pair for runner restart tests.
#[allow(
    private_interfaces,
    reason = "the crate-visible fixture intentionally returns a Sumeragi-sealed authority"
)]
pub(crate) fn complete_tip_restart_activation_fixture() -> (
    std::sync::Arc<crate::kura::Kura>,
    std::path::PathBuf,
    iroha_data_model::block::consensus_v2::HeightContext,
    RetiredRecoveredCompleteTipActivationAuthorityV1,
) {
    ledger::tests::durable_ready_fetch_recovery::complete_tip_restart_activation_fixture()
}
#[cfg(all(test, feature = "bls"))]
/// Build the exact retired H/H+1 inputs for lifecycle clean-shutdown tests.
pub(in crate::sumeragi) fn complete_tip_lifecycle_shutdown_fixture() -> (
    std::sync::Arc<crate::kura::Kura>,
    super::v2::VerifiedHeightContext,
    iroha_crypto::KeyPair,
    RetiredRecoveredCompleteTipActivationAuthorityV1,
) {
    ledger::tests::durable_ready_fetch_recovery::complete_tip_lifecycle_shutdown_fixture()
}
#[cfg(test)]
pub(in crate::sumeragi) use ledger::LifecycleLedgerStoreV1;
#[cfg(test)]
pub(crate) use ledger::{
    append_same_owner_foreign_terminal_for_test,
    substitute_recovered_control_replay_authority_for_test,
    substitute_recovered_decision_fetch_owner_for_test,
    substitute_recovered_decision_fetch_replay_authority_for_test,
};
pub(super) use open::TerminalValidateNoSuccessorClaim;
#[allow(unused_imports, reason = "release-bound lifecycle error seam")]
pub(crate) use open::{AuthenticatedLifecycleRecoveryCut, LifecycleOpenError};
#[cfg(test)]
#[allow(
    unused_imports,
    reason = "reviewed certified-serve admission test seam"
)]
pub(crate) use projection::CertifiedServeAdmissionBoundaryError;
pub(in crate::sumeragi) use projection::CertifiedServeTerminalReplayAuthorizationV1;
#[allow(
    unused_imports,
    reason = "typed terminal replay failure is part of the owner boundary"
)]
pub(in crate::sumeragi) use projection::CertifiedServeTerminalReplayFailureV1;
pub(in crate::sumeragi) use projection::lifecycle_context;
#[allow(unused_imports, reason = "reviewed certified-serve test seam")]
pub(crate) use projection::{
    AdapterEffectAdmissionError, CertifiedServeAdmissionError,
    CertifiedServeTerminalSettlementErrorV1, CertifiedServeTerminalSettlementFailureV1,
    ProducerTurnTerminalSettlementErrorV1, ProducerTurnTerminalSettlementFailureV1,
};
pub(in crate::sumeragi) use replay_authority::LifecycleReplayAuthorityV1;
pub(in crate::sumeragi) use replay_authority::RecoveredDecisionApplyCandidateLineageV1;
pub(super) use replay_authority::SealedLiveWalPersistedEffectV1;
#[allow(unused_imports, reason = "reviewed replay-evidence namespace")]
pub(in crate::sumeragi) use replay_authority::{
    DurableCertifiedFetchPendingMintPermit, DurableValidateReplayEvidenceV1,
    InvalidBodyReportReplayEvidenceV1, LocalBodyPreIntentReplaySealV1,
    LocalProposalIntentReplayEvidenceV1, LocalProposalReadyReplayEvidenceV1,
    LocalValidateReplayEvidenceV1, RecoveredDecisionApplyReplayLineageV1,
    RecoveredLifecycleNextWalVoteCandidateProjectionV1, RecoveredLifecycleNextWalVoteSealV1,
    RemoteProposalFetchReplayEvidenceV1, RemoteProposalStoreReplayEvidenceV1,
    RemoteProposalStoredReplayEvidenceV1, RemoteProposalValidateReplayEvidenceV1,
};
pub(crate) use replay_authority::{
    RecoveredWalControlReplayEvidenceV1, RecoveredWalDecisionFetchReplayEvidenceV1,
    RecoveredWalVoteReplayEvidenceV1,
};
pub(in crate::sumeragi) use scheduler_inputs::ProducerTurnSchedulerClaimErrorV1;
#[allow(
    unused_imports,
    reason = "reviewed scheduler-input namespace retained for production wiring"
)]
pub(crate) use scheduler_inputs::{
    AuthenticatedSchedulerInputsFactory, PreparedProductionIngressCapacityWait,
    ProductionCompletionReadyWorkV1, ProductionIngressCapacityRetry,
    ProductionIngressCapacityStatus, ProductionIngressSchedulerInputsError,
    ProductionIngressTurnPreparation, ProductionSchedulerInputsError, QueuedProductionIngressFetch,
};
pub(in crate::sumeragi) use scheduler_inputs::{
    CertifiedServeSchedulerObservationV1, claim_certified_serve_turn_v1,
};
pub(in crate::sumeragi) use scheduler_inputs::{
    ProductionRecoveredCompletionDispatchErrorV1, ProductionRecoveredCompletionDispatchV1,
    ProductionRecoveredDecisionFetchPersistenceErrorV1,
    ProductionRecoveredDecisionFetchPersistenceV1, ProductionRecoveredLifecycleSignDispatchErrorV1,
    ProductionRecoveredLifecycleSignDispatchV1,
    ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1,
    ProductionRecoveredLifecycleSignedBroadcastRefanoutV1,
};
#[cfg(test)]
use schema::MAX_LIFECYCLE_RECORDS_PER_HEIGHT;
#[cfg_attr(
    not(test),
    allow(unused_imports, reason = "reviewed scheduler schema namespace")
)]
pub(crate) use schema::{
    AdmissionDecision, AdmissionRejection, AdmissionRequest, CandidateAdmission, CapacityClass,
    CausalRoot, CoordinatorFault, InitialLifecycleState, LeaseId, LifecycleContext,
    LifecycleDigest, LifecycleKey, LifecyclePhase, LifecycleRecord, LifecycleRound, LifecycleStage,
    LifecycleStageKind, LifecycleState, LifecycleWorkClass, OwnerId, PhysicalGeometry,
    PhysicalReplacement, PhysicalSlot, PhysicalSlotId, PredecessorScope, ProducerTurnAdmission,
    ReadyEvent, SchedulerEpisodeUniverse, SchedulerInputs, SchedulerRank, TerminalOutcome,
    TurnLease, TurnOutcome, TurnPlan, WaitSource, WaitToken,
};
use schema::{
    CapacityAdmissionWait, CapacityGeometry, DurableContinuation, DurablePayloadReference,
    DurableRecordMetadata, DurableServeNegativeOutcome, LeaseCapacityReservation,
    RecoveredLifecycleRecord, RecoverySnapshot, SchedulerEpisode, SchedulerReadyInputs,
    first_capacity_wait, frozen_predecessors, has_lifecycle_record_capacity,
    lower_enter_view_ordinals, serve_and_producer_keys_match,
};
#[cfg(test)]
pub(crate) use schema::{NonCandidateEffect, RetryAction};
#[cfg(test)]
pub(crate) use selector::CertifiedFetchReadyPublicationError;
#[allow(unused_imports, reason = "reviewed persistence selector namespace")]
pub(crate) use selector::{
    CertifiedFetchBodyPersistenceCompletion, CertifiedFetchBodyPersistenceCompletionError,
    CertifiedFetchBodyPersistencePreparationError, CertifiedFetchBodyPersistencePreparationFailure,
    CertifiedFetchBodyPersistenceRestartError, CertifiedFetchBodyPersistenceRetryError,
    LifecycleIngressIoTargetKind, LifecycleIngressIoTargetSeal, LifecycleIngressSelectorError,
    PreparedLifecycleIngressSelector,
};
#[allow(unused_imports, reason = "reviewed recovered-fetch selector namespace")]
pub(in crate::sumeragi) use selector::{
    CertifiedFetchBodyPersistenceId, CertifiedFetchBodyPersistenceTask,
    RecoveredDecisionFetchBodyPersistenceCompletionV1, RecoveredDecisionFetchBodyPersistenceIdV1,
    RecoveredDecisionFetchBodyPersistencePreparationErrorV1,
    RecoveredDecisionFetchBodyPersistenceTaskV1, RecoveredDecisionFetchExactDequeueErrorV1,
};
#[cfg_attr(
    not(test),
    allow(unused_imports, reason = "reviewed recovered-WAL projection namespace")
)]
pub(in crate::sumeragi) use wal_recovery::{
    AuthenticatedRecoveredWalControlProjection, AuthenticatedRecoveredWalDecisionFetchProjection,
    AuthenticatedRecoveredWalVoteProjection, RecoveredDecisionApplyPendingLineageV1,
    RecoveredDecisionFetchStoreAdapterAuthorityV1, RecoveredDecisionFetchStoreProjectionV1,
};
#[allow(unused_imports, reason = "reviewed recovered-WAL successor namespace")]
pub(in crate::sumeragi) use wal_recovery::{
    RecoveredLifecycleSignBroadcastProjectionPermitV1,
    RecoveredLifecycleSignedBroadcastAndSignProjectionV1,
    RecoveredLifecycleSignedBroadcastOutputAuthorityV1,
};
pub(in crate::sumeragi) use work_registry::ClaimedCertifiedServeDispatchV1;
pub(in crate::sumeragi) use work_registry::RecoveredDecisionApplyRegistryProjectionPermit;
#[cfg(test)]
pub(in crate::sumeragi) use work_registry::RecoveredLifecycleSignClassV1;
pub(in crate::sumeragi) use work_registry::{
    AttemptedProducerTurnV1, ClaimedProducerTurnV1, PreparedRecoveredDecisionApplyDispatch,
    PreparedRecoveredDecisionFetchDispatchV1, PreparedRecoveredLifecycleSignDispatch,
    ReadyValidateSignPredecessorAuthority, RecoveredDecisionApplyCompletionProjectionPermit,
    RecoveredDecisionApplyDispatchIdentityV1, RecoveredDecisionApplyDispatchKeyV1,
    RecoveredDecisionFetchDispatchIdentityV1, RecoveredDecisionFetchDispatchKeyV1,
    RecoveredLifecycleSignDispatchIdentityV1, RecoveredLifecycleSignDispatchKeyV1,
};
#[allow(unused_imports, reason = "reviewed recovered-WAL registry namespace")]
pub(crate) use work_registry::{
    AuthenticatedRecoveredWalValidateLifecycleRepair,
    DurableAuthenticatedRecoveredWalValidateLifecycleRepair, ExactStoreRecoveredWalPersistError,
    ExactStoreRecoveredWalSignInstallError, InstalledRecoveredWalSignRegistryCut,
    InstalledRecoveredWalSignStorage, OpenedRecoveredWalSignLifecycleCut,
    OpenedRecoveredWalValidateLedger, PersistedRecoveredWalValidateLedger,
    PreparedReadyDurableValidateExecution, ProductionOpenedRecoveredWalSignLifecycleCut,
    ProductionRecoveredWalStorageError, ReadyDurableValidateOutcomeKind,
    ReadyRejectedAdapterAuthority, ReadyValidatedAdapterAuthority, RecoveredWalParentFactoryError,
    RecoveredWalProductionOwnerOpenV1, RecoveredWalSignInstallError,
    RecoveredWalSignLifecycleOpenError, RecoveredWalValidateLedgerPersistError,
    RecoveredWalValidateRegistryCut, RecoveredWalValidateRegistryJoinError,
};
pub(in crate::sumeragi) use work_registry::{
    LiveValidateSignRegistryReservation, LiveValidateSignWorkProjectionPermit,
    PreparedLiveValidateSignRegistryWork,
};
const MAX_PENDING_ADMISSION_WAITS: usize = 64;
/// Sole allocator and writer of logical Sumeragi lifecycle state.
#[derive(Debug)]
#[cfg_attr(test, derive(Clone))]
pub(crate) struct LifecycleCoordinator {
    episode_authority: AuthenticatedEpisodeAuthority,
    active_context: LifecycleContext,
    records: BTreeMap<u128, LifecycleRecord>,
    key_index: BTreeMap<LifecycleKey, u128>,
    owner_index: BTreeMap<CausalRoot, OwnerId>,
    ready_index: BTreeSet<u128>,
    admission_waits: BTreeMap<LifecycleKey, CapacityAdmissionWait>,
    active_lease: Option<TurnLease>,
    high_water: u128,
    next_lease: Option<u128>,
    durable_records: BTreeMap<u128, DurableRecordMetadata>,
    capacity_geometry: CapacityGeometry,
    capacity_used: BTreeMap<CapacityClass, usize>,
    capacity_generation: BTreeMap<CapacityClass, u64>,
    observed_generation: BTreeMap<WaitSource, u64>,
    producer_debts: BTreeMap<u128, u128>,
    ledger_store: Option<ledger::LifecycleLedgerStoreV1>,
    fault: Option<CoordinatorFault>,
}
// PRODUCTION_LIFECYCLE_OWNER_DECLARATION_BEGIN
/// Sole process owner of recovered V1 lifecycle execution state.
///
/// Construction consumes one authenticated recovered-adapter startup and
/// privately selects its storage-only or recovered-WAL repair branch. Before
/// live planning, a second consuming launch transition must move the exact
/// body store into the I/O worker and leave its instance seal in this owner.
/// None of the adapter, coordinator, registry, or payload store can be
/// detached, cloned, or separately installed through this API.
#[must_use = "the production lifecycle owner must remain alive for its height"]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct ProductionLifecycleOwnerV1 {
    verified: crate::sumeragi::v2::VerifiedHeightContext,
    coordinator: LifecycleCoordinator,
    registry: LifecycleWorkRegistryHolder,
    payload_store: crate::sumeragi::v2_certified_serve_payload_store::CertifiedServePayloadStoreV1,
    serve_payloads: crate::sumeragi::v2_certified_serve_payload_store::AuthenticatedCertifiedServePayloadRecoveryCut,
    body_store: Option<crate::sumeragi::v2_body_store::V2BodyStore>,
    body_store_identity: Option<crate::sumeragi::v2_body_store::V2BodyStoreInstanceIdentity>,
    kura_binding: Option<crate::sumeragi::v2::RecoveredLifecycleOwnerKuraBindingV1>,
    apply_service: Option<crate::sumeragi::v2_apply::V2ApplyService>,
    adapter_startup: Option<crate::sumeragi::v2::ProductionLifecycleAdapterStartupV1>,
}
// PRODUCTION_LIFECYCLE_OWNER_DECLARATION_END
/// Move-only permit for transferring the recovery-replay Apply service.
///
/// Only the lifecycle launch child can name the private seal field. Sumeragi
/// siblings may consume the permit through the worker seam but cannot mint a
/// parallel path around the unified owner.
#[must_use = "the Apply-service launch permit must be consumed by the I/O worker"]
pub(in crate::sumeragi) struct ProductionLifecycleApplyServiceLaunchPermitV1 {
    _seal: ProductionLifecycleApplyServiceLaunchPermitSealV1,
}
struct ProductionLifecycleApplyServiceLaunchPermitSealV1;
impl Drop for ProductionLifecycleApplyServiceLaunchPermitSealV1 {
    fn drop(&mut self) {}
}
impl ProductionLifecycleOwnerV1 {
    /// Bind the Kura and exact Apply service authenticated by the production factory.
    ///
    /// Test-only raw-root fixtures never call this method and remain
    /// deliberately unlaunchable. A second binding is an invariant violation
    /// because it would detach already-opened storage from its live Kura owner.
    pub(in crate::sumeragi) fn with_recovered_kura_binding_and_apply_service(
        mut self,
        binding: crate::sumeragi::v2::RecoveredLifecycleOwnerKuraBindingV1,
        apply_service: crate::sumeragi::v2_apply::V2ApplyService,
    ) -> Self {
        assert!(self.kura_binding.is_none());
        assert!(self.apply_service.is_none());
        self.kura_binding = Some(binding);
        self.apply_service = Some(apply_service);
        self
    }
}
impl LifecycleCoordinator {
    #[cfg(test)]
    fn new(
        active_context: LifecycleContext,
        high_water: u128,
        capacity_geometry: CapacityGeometry,
    ) -> Self {
        let authority = authority::test_authority(
            active_context,
            (2_u8..=5).map(|byte| LifecycleDigest::new([byte; 32])),
            0,
            capacity_geometry,
        )
        .expect("the fixed test authority is valid");
        Self::new_with_authority(authority, high_water)
    }
    /// Construct an empty coordinator from a sealed height authority.
    fn new_with_authority(
        episode_authority: AuthenticatedEpisodeAuthority,
        high_water: u128,
    ) -> Self {
        let active_context = episode_authority.context();
        let capacity_geometry = episode_authority.capacity_geometry().clone();
        Self {
            episode_authority,
            active_context,
            records: BTreeMap::new(),
            key_index: BTreeMap::new(),
            owner_index: BTreeMap::new(),
            ready_index: BTreeSet::new(),
            admission_waits: BTreeMap::new(),
            active_lease: None,
            high_water,
            next_lease: Some(1),
            durable_records: BTreeMap::new(),
            capacity_used: CapacityClass::ALL
                .into_iter()
                .map(|class| (class, 0))
                .collect(),
            capacity_generation: CapacityClass::ALL
                .into_iter()
                .map(|class| (class, 0))
                .collect(),
            capacity_geometry,
            observed_generation: BTreeMap::new(),
            producer_debts: BTreeMap::new(),
            ledger_store: None,
            fault: None,
        }
    }
    /// Return the currently open typed context.
    pub(crate) const fn active_context(&self) -> LifecycleContext {
        self.active_context
    }
    /// Return the durable ordinal high-water mark.
    #[cfg(test)]
    pub(crate) const fn high_water(&self) -> u128 {
        self.high_water
    }
    /// Return the latched fail-closed condition, when present.
    pub(crate) const fn fault(&self) -> Option<CoordinatorFault> {
        self.fault
    }
    /// Project and admit one exact runtime-bound production adapter effect.
    ///
    /// The live lifecycle runner uses its specialized typed owner paths; this
    /// generic projection remains a closed compatibility seam for exact
    /// runtime-effect admission checks.
    #[cfg_attr(not(test), allow(dead_code))]
    fn admit_bound_adapter_effect(
        &mut self,
        verified: &crate::sumeragi::v2::VerifiedHeightContext,
        effect: &crate::sumeragi::v2::AdapterEffect,
        ownership: &crate::sumeragi::v2_runtime::RuntimeEffectOwnership,
    ) -> Result<AdmissionDecision, AdapterEffectAdmissionError> {
        let pending = ownership
            .pending_adapter_effect_binding(effect)
            .ok_or(AdapterEffectAdmissionError::UnboundEffect)?;
        self.admit_pending_adapter_effect(verified, effect, &pending)
    }
    /// Project and admit one sealed ordinal-free adapter-effect binding.
    ///
    /// The lifecycle stack already owns the matching concrete-work registry;
    /// this ordinal-free form remains the internal projection used by the
    /// generic compatibility seam above.
    #[cfg_attr(not(test), allow(dead_code))]
    fn admit_pending_adapter_effect(
        &mut self,
        verified: &crate::sumeragi::v2::VerifiedHeightContext,
        effect: &crate::sumeragi::v2::AdapterEffect,
        pending: &crate::sumeragi::v2_runtime::PendingRuntimeEffectBinding,
    ) -> Result<AdmissionDecision, AdapterEffectAdmissionError> {
        let request =
            projection::admission_request(self.active_context, verified, effect, pending)?;
        Ok(self.admit(request))
    }
    /// Project and atomically admit one post-fsync authenticated Certified-Serve request.
    ///
    /// Tests use this direct seam; production Serve admission reaches the same
    /// adjacent Serve/ProducerTurn constructor through the sealed owner path.
    #[cfg(test)]
    pub(super) fn admit_certified_serve(
        &mut self,
        verified: &crate::sumeragi::v2::VerifiedHeightContext,
        request: &crate::sumeragi::v2_transport::AuthenticatedCertifiedBodyRequest,
        receipt: crate::sumeragi::v2_certified_serve_payload_store::DurableCertifiedServeAdmissionReceipt,
    ) -> Result<AdmissionDecision, CertifiedServeAdmissionError> {
        let request = projection::certified_serve_admission_request(
            self.active_context,
            verified,
            request,
            receipt,
        )?;
        Ok(self.admit(request))
    }
    /// Atomically validate and reserve a logical lifecycle admission.
    pub(crate) fn admit(&mut self, request: AdmissionRequest) -> AdmissionDecision {
        if self.ledger_store.is_none() {
            return self.reduce_admit(request);
        }
        let mut next = self.stage_durable_transaction();
        let decision = next.reduce_admit(request);
        if matches!(decision, AdmissionDecision::Admitted { .. }) {
            if next.persist_durable_projection().is_err() {
                self.fault = Some(CoordinatorFault::DurabilityFailure);
                return AdmissionDecision::FailClosed(CoordinatorFault::DurabilityFailure);
            }
        }
        *self = next;
        decision
    }
    fn reduce_admit(&mut self, request: AdmissionRequest) -> AdmissionDecision {
        if let Some(fault) = self.fault {
            return AdmissionDecision::FailClosed(fault);
        }
        let AdmissionRequest::Candidate(mut candidate) = request else {
            return AdmissionDecision::NonCandidate;
        };
        if candidate.key.context != self.active_context.id
            || candidate.key.round.height != self.active_context.height
            || candidate
                .key
                .proposal_round
                .is_some_and(|round| round.height != self.active_context.height)
        {
            return AdmissionDecision::Rejected(AdmissionRejection::ForeignContext);
        }
        let pending_wait = self.admission_waits.get(&candidate.key).cloned();
        let mut pending_unlocked = false;
        if let Some(waiting) = pending_wait.as_ref() {
            if waiting.candidate.causal_root != candidate.causal_root {
                return AdmissionDecision::Rejected(AdmissionRejection::ForeignOwner);
            }
            let WaitSource::Capacity(class) = waiting.wait_token.source else {
                unreachable!("admission waits are capacity-fenced")
            };
            pending_unlocked =
                self.capacity_generation[&class] > waiting.wait_token.observed_generation;
            if pending_unlocked {
                // Once the named generation advances, the old fence cannot
                // survive a changed, superseded, or otherwise invalid retry.
                self.admission_waits.remove(&candidate.key);
            }
        }
        if !candidate.replay_authority_is_exact(self.active_context) {
            return AdmissionDecision::Rejected(AdmissionRejection::InvalidDurableMetadata);
        }
        if candidate.work_class == LifecycleWorkClass::CertifiedServe
            && candidate.reconstruction_source != candidate.causal_root.digest()
        {
            return AdmissionDecision::Rejected(AdmissionRejection::InvalidDurableMetadata);
        }
        if let Some(ordinal) = self.key_index.get(&candidate.key).copied() {
            let (owner, work_class, stage, state) = {
                let record = self
                    .records
                    .get(&ordinal)
                    .expect("key index is kept bijective with lifecycle records");
                (record.owner, record.work_class, record.stage, record.state)
            };
            if owner.causal_root != candidate.causal_root {
                return AdmissionDecision::Rejected(AdmissionRejection::ForeignOwner);
            }
            if work_class != candidate.work_class
                || stage != candidate.stage
                || !candidate
                    .payload
                    .matches_terminal(candidate.work_class, None)
                || (candidate.work_class == LifecycleWorkClass::Validate
                    && !durable_validate_payload_is_exact(candidate.key, candidate.payload))
                || !self
                    .durable_records
                    .get(&ordinal)
                    .is_some_and(|metadata| metadata.matches_admission(&candidate))
                || !self.retry_companion_matches(&self.records[&ordinal], &candidate)
            {
                return AdmissionDecision::Rejected(AdmissionRejection::SemanticDrift);
            }
            if matches!(
                state,
                LifecycleState::Waiting(WaitToken {
                    source: WaitSource::Recovery(_),
                    ..
                })
            ) && let Err(rejection) = self.rebind_recovered_candidate(ordinal, &mut candidate)
            {
                return AdmissionDecision::Rejected(rejection);
            }
            return match self.records[&ordinal].state {
                LifecycleState::Terminal(outcome @ TerminalOutcome::Completed(Some(_))) => {
                    AdmissionDecision::ReplayTerminal { owner, outcome }
                }
                LifecycleState::Terminal(_) => AdmissionDecision::StutterTerminal { owner },
                LifecycleState::Waiting(_) | LifecycleState::Ready | LifecycleState::Claimed(_) => {
                    AdmissionDecision::Retry {
                        owner,
                        ordinal,
                        action: work_class.retry_action(),
                    }
                }
            };
        }
        if let Some(conflict) = self.admission_waits.values().find(|waiting| {
            waiting
                .candidate
                .producer_turn
                .as_ref()
                .is_some_and(|producer| producer.key == candidate.key)
        }) {
            return AdmissionDecision::Rejected(
                if conflict.candidate.causal_root == candidate.causal_root {
                    AdmissionRejection::InvalidProducerTurn
                } else {
                    AdmissionRejection::ForeignOwner
                },
            );
        }
        if candidate.work_class == LifecycleWorkClass::ProducerTurn {
            return AdmissionDecision::Rejected(AdmissionRejection::InvalidProducerTurn);
        }
        if !candidate
            .work_class
            .accepts_stage(candidate.key.phase, candidate.stage)
        {
            return AdmissionDecision::Rejected(AdmissionRejection::InvalidWorkShape);
        }
        if !candidate
            .payload
            .matches_terminal(candidate.work_class, None)
            || (candidate.work_class == LifecycleWorkClass::Validate
                && !durable_validate_payload_is_exact(candidate.key, candidate.payload))
        {
            return AdmissionDecision::Rejected(AdmissionRejection::InvalidDurableMetadata);
        }
        if matches!(
            candidate.initial_state,
            InitialLifecycleState::Waiting(WaitToken {
                source: WaitSource::Capacity(_)
                    | WaitSource::Recovery(_)
                    | WaitSource::ProducerTurn(_),
                ..
            })
        ) {
            self.admission_waits.remove(&candidate.key);
            return AdmissionDecision::Rejected(AdmissionRejection::InvalidInitialState);
        }
        if matches!(
            candidate.initial_state,
            InitialLifecycleState::Waiting(WaitToken {
                observed_generation: u64::MAX,
                ..
            })
        ) {
            self.admission_waits.remove(&candidate.key);
            return AdmissionDecision::Rejected(AdmissionRejection::InvalidInitialState);
        }
        let producer = match (candidate.work_class, candidate.producer_turn.as_ref()) {
            (LifecycleWorkClass::CertifiedServe, Some(producer)) => Some(producer),
            (LifecycleWorkClass::CertifiedServe, None) => {
                return AdmissionDecision::Rejected(AdmissionRejection::MissingProducerTurn);
            }
            (_, Some(_)) => {
                return AdmissionDecision::Rejected(AdmissionRejection::UnexpectedProducerTurn);
            }
            (_, None) => None,
        };
        if producer.is_some_and(|producer| {
            !serve_and_producer_keys_match(candidate.key, producer.key)
                || producer.stage.kind != LifecycleStageKind::ProducerTurn
                || producer.reconstruction_source != candidate.reconstruction_source
                || self.key_index.contains_key(&producer.key)
        }) {
            return AdmissionDecision::Rejected(AdmissionRejection::InvalidProducerTurn);
        }
        let (ordinal_count, ordinal_span) = if producer.is_some() {
            (2_usize, 1_u128)
        } else {
            (1_usize, 0_u128)
        };
        if !has_lifecycle_record_capacity(self.records.len(), ordinal_count) {
            self.admission_waits.remove(&candidate.key);
            return AdmissionDecision::Rejected(AdmissionRejection::AdmissionQueueFull);
        }
        if let Some(producer) = producer
            && let Some(conflict) = self.admission_waits.iter().find_map(|(key, waiting)| {
                (*key != candidate.key
                    && (waiting.candidate.key == producer.key
                        || waiting
                            .candidate
                            .producer_turn
                            .as_ref()
                            .is_some_and(|pending| pending.key == producer.key)))
                .then_some(waiting)
            })
        {
            return AdmissionDecision::Rejected(
                if conflict.candidate.causal_root == candidate.causal_root {
                    AdmissionRejection::InvalidProducerTurn
                } else {
                    AdmissionRejection::ForeignOwner
                },
            );
        }
        if let Err(rejection) = candidate.canonicalize_geometry() {
            return AdmissionDecision::Rejected(rejection);
        }
        let producer = candidate.producer_turn.as_ref();
        if let Some(waiting) = pending_wait {
            if waiting.candidate != candidate {
                return AdmissionDecision::Rejected(AdmissionRejection::SemanticDrift);
            }
            let WaitSource::Capacity(class) = waiting.wait_token.source else {
                unreachable!("admission waits are capacity-fenced")
            };
            if !pending_unlocked {
                return AdmissionDecision::WaitForCapacity(waiting.wait_token);
            }
            // The exact fence has advanced. Revalidation either admits the
            // frozen request, installs a new fence, or conclusively rejects it.
            debug_assert!(
                self.capacity_generation[&class] > waiting.wait_token.observed_generation
            );
        }
        if candidate.work_class == LifecycleWorkClass::EnterView
            && (self.records.values().any(|record| {
                record.work_class == LifecycleWorkClass::EnterView
                    && record.key.context == candidate.key.context
                    && record.key.round.height == candidate.key.round.height
                    && record.key.round.view >= candidate.key.round.view
            }) || self.admission_waits.values().any(|waiting| {
                waiting.candidate.work_class == LifecycleWorkClass::EnterView
                    && waiting.candidate.key.context == candidate.key.context
                    && waiting.candidate.key.round.height == candidate.key.round.height
                    && waiting.candidate.key.round.view >= candidate.key.round.view
            }))
        {
            self.admission_waits.remove(&candidate.key);
            return AdmissionDecision::Rejected(AdmissionRejection::EnterViewConflict);
        }
        let candidate_geometry = match candidate.physical_geometry.normalized() {
            Ok(geometry) => geometry,
            Err(rejection) => return AdmissionDecision::Rejected(rejection),
        };
        let producer_geometry = match producer
            .map(|producer| producer.physical_geometry.normalized())
            .transpose()
        {
            Ok(geometry) => geometry,
            Err(rejection) => return AdmissionDecision::Rejected(rejection),
        };
        let Some(candidate_universe) = self.episode_authority.universe_for(candidate.key) else {
            return AdmissionDecision::Rejected(AdmissionRejection::InvalidEpisodeUniverse);
        };
        let producer_universe = if let Some(producer) = producer {
            let Some(universe) = self.episode_authority.universe_for(producer.key) else {
                return AdmissionDecision::Rejected(AdmissionRejection::InvalidEpisodeUniverse);
            };
            Some(universe)
        } else {
            None
        };
        if !self
            .episode_authority
            .admits_slots(candidate.work_class.capacity_class(), &candidate_geometry.1)
            || producer_geometry.as_ref().is_some_and(|geometry| {
                !self
                    .episode_authority
                    .admits_slots(CapacityClass::Producer, &geometry.1)
            })
        {
            return AdmissionDecision::Rejected(AdmissionRejection::InvalidEpisodeUniverse);
        }
        let mut capacity_delta = BTreeMap::<CapacityClass, usize>::new();
        *capacity_delta
            .entry(candidate.work_class.capacity_class())
            .or_default() += 1;
        if producer.is_some() {
            *capacity_delta.entry(CapacityClass::Producer).or_default() += 1;
        }
        if let Some(wait) = self.first_capacity_wait(&capacity_delta) {
            if candidate.work_class == LifecycleWorkClass::EnterView {
                self.retire_lower_enter_view_admission_waits(candidate.key);
            }
            if !self.admission_waits.contains_key(&candidate.key)
                && self.admission_waits.len() >= MAX_PENDING_ADMISSION_WAITS
            {
                return AdmissionDecision::Rejected(AdmissionRejection::AdmissionQueueFull);
            }
            self.admission_waits.insert(
                candidate.key,
                CapacityAdmissionWait {
                    candidate: candidate.clone(),
                    wait_token: wait,
                    serve_payload_receipt: None,
                },
            );
            return AdmissionDecision::WaitForCapacity(wait);
        }
        self.admission_waits.remove(&candidate.key);
        let Some(first_ordinal) = self.high_water.checked_add(1) else {
            return AdmissionDecision::Rejected(AdmissionRejection::OrdinalExhausted);
        };
        let Some(last_ordinal) = first_ordinal.checked_add(ordinal_span) else {
            return AdmissionDecision::Rejected(AdmissionRejection::OrdinalExhausted);
        };
        let owner = self
            .owner_index
            .get(&candidate.causal_root)
            .copied()
            .unwrap_or(OwnerId {
                causal_root: candidate.causal_root,
                first_admission_ordinal: first_ordinal,
            });
        let state = match candidate.initial_state {
            InitialLifecycleState::Ready => LifecycleState::Ready,
            InitialLifecycleState::Waiting(wait) => {
                let known = self
                    .observed_generation
                    .get(&wait.source)
                    .copied()
                    .unwrap_or(0);
                if known > wait.observed_generation {
                    LifecycleState::Ready
                } else {
                    self.advance_observed_generation(wait.source, wait.observed_generation);
                    LifecycleState::Waiting(wait)
                }
            }
        };
        let frozen_predecessors =
            self.frozen_predecessors(candidate.stage.predecessor_scope, first_ordinal);
        let record = LifecycleRecord {
            key: candidate.key,
            owner,
            ordinal: first_ordinal,
            work_class: candidate.work_class,
            stage: candidate.stage,
            state,
            physical_slots: candidate_geometry.0,
            episode: SchedulerEpisode {
                universe: candidate_universe,
                slot_universe: candidate_geometry.1,
                consumed_slots: candidate_geometry.2,
                frozen_predecessors,
            },
        };
        self.high_water = last_ordinal;
        self.owner_index
            .entry(candidate.causal_root)
            .or_insert(owner);
        self.apply_capacity_delta(&capacity_delta);
        self.durable_records.insert(
            first_ordinal,
            DurableRecordMetadata::from_candidate(&candidate),
        );
        self.insert_record(record);
        let producer_turn_ordinal = producer.map(|producer| {
            let ordinal = first_ordinal + 1;
            let producer_geometry =
                producer_geometry.expect("producer geometry accompanies producer admission");
            let frozen_predecessors =
                self.frozen_predecessors(producer.stage.predecessor_scope, ordinal);
            let record = LifecycleRecord {
                key: producer.key,
                owner,
                ordinal,
                work_class: LifecycleWorkClass::ProducerTurn,
                stage: producer.stage,
                state: LifecycleState::Waiting(WaitToken::new(
                    WaitSource::ProducerTurn(first_ordinal),
                    0,
                )),
                physical_slots: producer_geometry.0,
                episode: SchedulerEpisode {
                    universe: producer_universe
                        .expect("producer universe accompanies producer admission"),
                    slot_universe: producer_geometry.1,
                    consumed_slots: producer_geometry.2,
                    frozen_predecessors,
                },
            };
            self.durable_records
                .insert(ordinal, DurableRecordMetadata::from_producer(&producer));
            self.insert_record(record);
            self.producer_debts.insert(first_ordinal, ordinal);
            ordinal
        });
        AdmissionDecision::Admitted {
            owner,
            ordinal: first_ordinal,
            producer_turn_ordinal,
        }
    }
    /// Publish a late completion without minting a witness or new ordinal.
    pub(crate) fn publish_ready(&mut self, event: ReadyEvent) {
        if self.fault.is_some() {
            return;
        }
        if matches!(
            event.wait_token.source,
            WaitSource::Capacity(_) | WaitSource::ProducerTurn(_)
        ) {
            self.fault = Some(CoordinatorFault::InvalidReadyEvent);
            return;
        }
        let Some(record) = self.records.get(&event.ordinal) else {
            self.fault = Some(CoordinatorFault::InvalidReadyEvent);
            return;
        };
        if record.owner != event.owner {
            self.fault = Some(CoordinatorFault::InvalidReadyEvent);
            return;
        }
        match record.state {
            LifecycleState::Terminal(_) => return,
            LifecycleState::Ready => return,
            LifecycleState::Claimed(_) => {
                self.fault = Some(CoordinatorFault::InvalidReadyEvent);
                return;
            }
            LifecycleState::Waiting(wait) if wait != event.wait_token => {
                self.fault = Some(CoordinatorFault::InvalidReadyEvent);
                return;
            }
            LifecycleState::Waiting(_) => {}
        }
        let Some(generation) = event.wait_token.observed_generation.checked_add(1) else {
            self.fault = Some(CoordinatorFault::InvalidReadyEvent);
            return;
        };
        if let Some(replacement) = event.replacement
            && self.replace_physical(event.ordinal, replacement).is_err()
        {
            self.fault = Some(CoordinatorFault::InvalidPhysicalTransition);
            return;
        }
        self.advance_observed_generation(event.wait_token.source, generation);
    }
    /// Select one ready unit for execution outside the coordinator lock.
    fn plan_turn(&mut self, inputs: SchedulerInputs) -> TurnPlan {
        if let Some(fault) = self.fault {
            return TurnPlan::FailClosed(fault);
        }
        if let Some(lease) = self.active_lease.as_ref() {
            let fault = CoordinatorFault::UnsettledLease(lease.id);
            self.fault = Some(fault);
            return TurnPlan::FailClosed(fault);
        }
        let (generations, ready_rows) = inputs.into_parts();
        let mut prospective_generations = self.observed_generation.clone();
        let mut invalid_inputs = false;
        for (source, generation) in &generations {
            if !matches!(source, WaitSource::External(_) | WaitSource::Recovery(_)) {
                invalid_inputs = true;
                continue;
            }
            let known = prospective_generations.entry(*source).or_default();
            *known = (*known).max(*generation);
        }
        let prospective_ready: BTreeSet<_> = self
            .records
            .iter()
            .filter_map(|(ordinal, record)| match record.state {
                LifecycleState::Ready => Some(*ordinal),
                LifecycleState::Waiting(wait) => match wait.source {
                    WaitSource::Capacity(_) => None,
                    WaitSource::ProducerTurn(_) => None,
                    WaitSource::External(_) | WaitSource::Recovery(_) => (prospective_generations
                        .get(&wait.source)
                        .copied()
                        .unwrap_or(0)
                        > wait.observed_generation)
                        .then_some(*ordinal),
                },
                LifecycleState::Claimed(_) | LifecycleState::Terminal(_) => None,
            })
            .collect();
        invalid_inputs |= ready_rows.len() != prospective_ready.len();
        let mut output_classes = BTreeMap::new();
        for ordinal in &prospective_ready {
            let record = &self.records[ordinal];
            let Some(row) = ready_rows.get(ordinal) else {
                invalid_inputs = true;
                continue;
            };
            if !row.identity_matches(*ordinal, record) {
                invalid_inputs = true;
                continue;
            }
            output_classes.insert(*ordinal, row.output_capacity_class());
        }
        let capacity_waits: BTreeMap<_, _> = output_classes
            .iter()
            .filter_map(|(ordinal, class)| {
                ready_rows
                    .get(ordinal)
                    .is_some_and(SchedulerReadyInputs::physical_capacity_available)
                    .then_some(*class)
                    .flatten()
                    .and_then(|class| {
                        self.first_capacity_wait(&BTreeMap::from([(class, 1)]))
                            .map(|wait| (*ordinal, wait))
                    })
            })
            .collect();
        let selectable_ready: BTreeSet<_> = prospective_ready
            .iter()
            .filter(|ordinal| {
                !capacity_waits.contains_key(*ordinal)
                    && ready_rows
                        .get(*ordinal)
                        .is_some_and(SchedulerReadyInputs::physical_capacity_available)
            })
            .copied()
            .collect();
        let mut ranks = BTreeMap::new();
        for ordinal in &selectable_ready {
            let record = &self.records[ordinal];
            let Some(row) = ready_rows.get(ordinal) else {
                invalid_inputs = true;
                continue;
            };
            let Ok(frozen_predecessors) = u64::try_from(
                record
                    .episode
                    .frozen_predecessors
                    .iter()
                    .filter(|predecessor| selectable_ready.contains(*predecessor))
                    .count(),
            ) else {
                invalid_inputs = true;
                continue;
            };
            let [mode, capacity, selector, lane, source, runner] = row.live_debts();
            let rank = SchedulerRank::new(
                record.stage.kind.remaining_stages(),
                frozen_predecessors,
                mode,
                capacity,
                selector,
                lane,
                source,
                runner,
            );
            ranks.insert(*ordinal, rank);
        }
        if invalid_inputs {
            self.fault = Some(CoordinatorFault::InvalidSchedulerInputs);
            return TurnPlan::FailClosed(CoordinatorFault::InvalidSchedulerInputs);
        }
        for (source, generation) in generations {
            self.advance_observed_generation(source, generation);
        }
        debug_assert_eq!(self.ready_index, prospective_ready);
        let selected = self
            .ready_index
            .iter()
            .copied()
            .filter(|ordinal| selectable_ready.contains(ordinal))
            .filter(|ordinal| self.ready_entry_is_eligible(*ordinal, &selectable_ready))
            .map(|ordinal| (ranks[&ordinal], ordinal))
            .min();
        if let Some((rank, selected_ordinal)) = selected {
            let Some(id_value) = self.next_lease else {
                self.fault = Some(CoordinatorFault::LeaseExhausted);
                return TurnPlan::FailClosed(CoordinatorFault::LeaseExhausted);
            };
            let Some(next_lease) = id_value.checked_add(1) else {
                self.next_lease = None;
                self.fault = Some(CoordinatorFault::LeaseExhausted);
                return TurnPlan::FailClosed(CoordinatorFault::LeaseExhausted);
            };
            let id = LeaseId(id_value);
            self.next_lease = Some(next_lease);
            self.ready_index.remove(&selected_ordinal);
            let output_reservation = output_classes[&selected_ordinal].map(|class| {
                LeaseCapacityReservation::new(
                    class,
                    self.capacity_generation.get(&class).copied().unwrap_or(0),
                )
            });
            let record = self
                .records
                .get_mut(&selected_ordinal)
                .expect("ready index is bijective with lifecycle records");
            record.state = LifecycleState::Claimed(id);
            let lease = TurnLease {
                id,
                ordinal: record.ordinal,
                owner: record.owner,
                key: record.key,
                work_class: record.work_class,
                stage: record.stage,
                rank,
                physical_slots: record.physical_slots.clone(),
                output_reservation,
            };
            self.active_lease = Some(lease.clone());
            return TurnPlan::Execute(lease);
        }
        let mut waits: BTreeSet<_> = self
            .records
            .values()
            .filter_map(|record| match record.state {
                LifecycleState::Waiting(wait) => Some(wait),
                LifecycleState::Ready
                | LifecycleState::Claimed(_)
                | LifecycleState::Terminal(_) => None,
            })
            .collect();
        waits.extend(
            self.admission_waits
                .values()
                .map(|waiting| waiting.wait_token),
        );
        waits.extend(capacity_waits.into_values());
        if waits.is_empty() {
            TurnPlan::Idle
        } else {
            TurnPlan::Waiting(waits)
        }
    }
    /// Restore one volatile claim which never crossed an executor boundary.
    ///
    /// This narrow rollback is used only while a service reservation still
    /// owns capacity and before registry/worker commit. It does not rewind the
    /// monotone lease source. Durable or externally visible work must use a
    /// typed settlement transaction instead.
    fn rollback_unpublished_turn(&mut self, lease: &TurnLease) -> bool {
        if self.fault.is_some() || self.active_lease.as_ref() != Some(lease) {
            return false;
        }
        let Some(record) = self.records.get_mut(&lease.ordinal) else {
            return false;
        };
        if record.state != LifecycleState::Claimed(lease.id)
            || record.owner != lease.owner
            || record.key != lease.key
            || record.work_class != lease.work_class
            || record.stage != lease.stage
            || record.physical_slots != lease.physical_slots
            || lease.output_reservation.is_some()
            || self.ready_index.contains(&lease.ordinal)
        {
            return false;
        }
        record.state = LifecycleState::Ready;
        let inserted = self.ready_index.insert(lease.ordinal);
        assert!(
            inserted,
            "an unpublished lifecycle rollback must restore one absent Ready index"
        );
        self.active_lease = None;
        true
    }
    /// Restore one unpublished claim carrying an exact output overlay.
    ///
    /// The overlay is not durable occupancy: `plan_turn` only projects it over
    /// `capacity_used`. Removing it therefore leaves usage unchanged, but
    /// advances that class's generation so an already-observed capacity wait
    /// cannot sleep through the newly available slot.
    fn rollback_unpublished_reserved_turn(
        &mut self,
        lease: &TurnLease,
        expected_class: CapacityClass,
    ) -> bool {
        if self.fault.is_some() || self.active_lease.as_ref() != Some(lease) {
            return false;
        }
        let Some(reservation) = lease.output_reservation() else {
            return false;
        };
        if reservation.class() != expected_class
            || reservation.wait_token().observed_generation()
                != self.capacity_generation[&expected_class]
            || self.capacity_used[&expected_class]
                .checked_add(1)
                .is_none_or(|reserved| reserved > self.capacity_geometry.limit(expected_class))
        {
            return false;
        }
        let Some(record) = self.records.get_mut(&lease.ordinal) else {
            return false;
        };
        if record.state != LifecycleState::Claimed(lease.id)
            || record.owner != lease.owner
            || record.key != lease.key
            || record.work_class != lease.work_class
            || record.stage != lease.stage
            || record.physical_slots != lease.physical_slots
            || self.ready_index.contains(&lease.ordinal)
        {
            return false;
        }
        let Some(next_generation) = self.capacity_generation[&expected_class].checked_add(1) else {
            self.fault = Some(CoordinatorFault::CapacityAccounting);
            return false;
        };
        record.state = LifecycleState::Ready;
        let inserted = self.ready_index.insert(lease.ordinal);
        assert!(
            inserted,
            "an unpublished reserved rollback must restore one absent Ready index"
        );
        self.active_lease = None;
        self.capacity_generation
            .insert(expected_class, next_generation);
        true
    }
    /// Rebuild records after seeding the ordinal high-water mark.
    fn reconcile_restart(&mut self, snapshot: RecoverySnapshot) {
        self.reconcile_restart_inner(snapshot);
    }
    fn retry_companion_matches(
        &self,
        record: &LifecycleRecord,
        candidate: &CandidateAdmission,
    ) -> bool {
        if record.work_class != LifecycleWorkClass::CertifiedServe {
            return candidate.producer_turn.is_none();
        }
        let Some(candidate_producer) = candidate.producer_turn.as_ref() else {
            return false;
        };
        let Some(producer_ordinal) = record.ordinal.checked_add(1) else {
            return false;
        };
        self.records.get(&producer_ordinal).is_some_and(|producer| {
            let debt_matches = if matches!(producer.state, LifecycleState::Terminal(_)) {
                !self.producer_debts.contains_key(&record.ordinal)
            } else {
                self.producer_debts.get(&record.ordinal) == Some(&producer_ordinal)
            };
            debt_matches
                && producer.work_class == LifecycleWorkClass::ProducerTurn
                && producer.key == candidate_producer.key
                && producer.stage == candidate_producer.stage
                && self
                    .durable_records
                    .get(&producer.ordinal)
                    .is_some_and(|metadata| {
                        metadata.reconstruction_source == candidate_producer.reconstruction_source
                            && metadata.payload == DurablePayloadReference::None
                            && metadata.replay_authority == candidate_producer.replay_authority
                    })
        })
    }
    fn rebind_recovered_candidate(
        &mut self,
        ordinal: u128,
        candidate: &mut CandidateAdmission,
    ) -> Result<(), AdmissionRejection> {
        if candidate.initial_state != InitialLifecycleState::Ready {
            return Err(AdmissionRejection::InvalidInitialState);
        }
        candidate.canonicalize_geometry()?;
        let candidate_geometry = candidate.physical_geometry.normalized()?;
        let record = self
            .records
            .get(&ordinal)
            .expect("recovery retry retains its indexed record");
        if !record.physical_slots.is_empty()
            || !record.episode.consumed_slots.is_empty()
            || record.episode.slot_universe != candidate_geometry.1
            || !self
                .episode_authority
                .admits_slots(record.work_class.capacity_class(), &candidate_geometry.1)
        {
            return Err(AdmissionRejection::SemanticDrift);
        }
        let producer_geometry = if let Some(candidate_producer) = candidate.producer_turn.as_ref() {
            let producer_ordinal = self
                .producer_debts
                .get(&ordinal)
                .copied()
                .ok_or(AdmissionRejection::InvalidProducerTurn)?;
            let geometry = candidate_producer.physical_geometry.normalized()?;
            let producer = self
                .records
                .get(&producer_ordinal)
                .ok_or(AdmissionRejection::InvalidProducerTurn)?;
            if !producer.physical_slots.is_empty()
                || !producer.episode.consumed_slots.is_empty()
                || producer.episode.slot_universe != geometry.1
                || !self
                    .episode_authority
                    .admits_slots(CapacityClass::Producer, &geometry.1)
            {
                return Err(AdmissionRejection::SemanticDrift);
            }
            Some((producer_ordinal, geometry))
        } else {
            None
        };
        {
            let record = self
                .records
                .get_mut(&ordinal)
                .expect("recovery retry retains its indexed record");
            record.physical_slots = candidate_geometry.0;
            record.episode.consumed_slots = candidate_geometry.2;
        }
        if let Some((producer_ordinal, geometry)) = producer_geometry {
            let producer = self
                .records
                .get_mut(&producer_ordinal)
                .expect("validated recovery producer remains present");
            producer.physical_slots = geometry.0;
            producer.episode.consumed_slots = geometry.2;
        }
        self.make_ready(ordinal);
        Ok(())
    }
    fn advance_observed_generation(&mut self, source: WaitSource, generation: u64) {
        debug_assert!(matches!(
            source,
            WaitSource::External(_) | WaitSource::Recovery(_)
        ));
        let known = self.observed_generation.entry(source).or_default();
        *known = (*known).max(generation);
        let known = *known;
        let stale: Vec<_> = self
            .records
            .iter()
            .filter_map(|(ordinal, record)| match record.state {
                LifecycleState::Waiting(wait)
                    if wait.source == source && wait.observed_generation < known =>
                {
                    Some(*ordinal)
                }
                LifecycleState::Waiting(_)
                | LifecycleState::Ready
                | LifecycleState::Claimed(_)
                | LifecycleState::Terminal(_) => None,
            })
            .collect();
        for ordinal in stale {
            self.make_ready(ordinal);
        }
    }
    fn first_capacity_wait(&self, delta: &BTreeMap<CapacityClass, usize>) -> Option<WaitToken> {
        let mut effective_used = self.capacity_used.clone();
        if let Some(reservation) = self
            .active_lease
            .as_ref()
            .and_then(TurnLease::output_reservation)
        {
            let used = effective_used.entry(reservation.class()).or_default();
            *used = used.checked_add(1).unwrap_or(usize::MAX);
        }
        first_capacity_wait(
            &effective_used,
            &self.capacity_geometry,
            &self.capacity_generation,
            delta,
        )
    }
    fn apply_capacity_delta(&mut self, delta: &BTreeMap<CapacityClass, usize>) {
        for (class, added) in delta {
            *self.capacity_used.entry(*class).or_default() += added;
        }
    }
    fn release_capacity(&mut self, class: CapacityClass) -> Result<(), CoordinatorFault> {
        let used = self.capacity_used.entry(class).or_default();
        *used = used
            .checked_sub(1)
            .ok_or(CoordinatorFault::CapacityAccounting)?;
        let generation = self.capacity_generation.entry(class).or_default();
        *generation = generation
            .checked_add(1)
            .ok_or(CoordinatorFault::CapacityAccounting)?;
        Ok(())
    }
    fn insert_record(&mut self, record: LifecycleRecord) {
        let ordinal = record.ordinal;
        let key = record.key;
        if record.state == LifecycleState::Ready {
            self.ready_index.insert(ordinal);
        }
        self.key_index.insert(key, ordinal);
        self.records.insert(ordinal, record);
    }
    fn make_ready(&mut self, ordinal: u128) {
        let record = self
            .records
            .get_mut(&ordinal)
            .expect("readiness publication names an existing record");
        if !matches!(record.state, LifecycleState::Terminal(_)) {
            record.state = LifecycleState::Ready;
            self.ready_index.insert(ordinal);
        }
    }
    fn replace_physical(
        &mut self,
        ordinal: u128,
        replacement: PhysicalReplacement,
    ) -> Result<(), CoordinatorFault> {
        if replacement.existing_slot != replacement.replacement.id {
            return Err(CoordinatorFault::InvalidPhysicalTransition);
        }
        let record = self
            .records
            .get_mut(&ordinal)
            .ok_or(CoordinatorFault::InvalidPhysicalTransition)?;
        if !record
            .physical_slots
            .contains_key(&replacement.existing_slot)
        {
            return Err(CoordinatorFault::InvalidPhysicalTransition);
        }
        if record.physical_slots.iter().any(|(slot, digest)| {
            *slot != replacement.existing_slot && *digest == replacement.replacement.digest
        }) {
            record.physical_slots.remove(&replacement.existing_slot);
        } else {
            record
                .physical_slots
                .insert(replacement.existing_slot, replacement.replacement.digest);
        }
        Ok(())
    }
    fn frozen_predecessors(&self, scope: PredecessorScope, ordinal: u128) -> BTreeSet<u128> {
        frozen_predecessors(&self.records, scope, ordinal)
    }
    fn ready_entry_is_eligible(&self, ordinal: u128, selectable_ready: &BTreeSet<u128>) -> bool {
        let record = self
            .records
            .get(&ordinal)
            .expect("ready index is bijective with lifecycle records");
        if record
            .episode
            .frozen_predecessors
            .iter()
            .any(|predecessor| selectable_ready.contains(predecessor))
        {
            return false;
        }
        !selectable_ready.iter().any(|candidate| {
            *candidate < ordinal
                && self.records.get(candidate).is_some_and(|record| {
                    record.stage.predecessor_scope == PredecessorScope::ProducerHandoffBarrier
                })
        })
    }
    fn finish_replenishment(
        &mut self,
        ordinal: u128,
        slot: PhysicalSlot,
    ) -> Result<(), CoordinatorFault> {
        let record = self
            .records
            .get_mut(&ordinal)
            .ok_or(CoordinatorFault::InvalidPhysicalTransition)?;
        if !record.episode.slot_universe.contains(&slot.id)
            || !record.episode.consumed_slots.insert(slot.id)
        {
            return Err(CoordinatorFault::InvalidPhysicalTransition);
        }
        if !record
            .physical_slots
            .values()
            .any(|digest| *digest == slot.digest)
        {
            record.physical_slots.insert(slot.id, slot.digest);
        }
        record.state = LifecycleState::Ready;
        self.ready_index.insert(ordinal);
        Ok(())
    }
    fn supersede_lower_enter_views(
        &mut self,
        installed: LifecycleKey,
    ) -> Result<(), CoordinatorFault> {
        for ordinal in lower_enter_view_ordinals(&self.records, installed) {
            self.finish_terminal(ordinal, TerminalOutcome::Cancelled)?;
        }
        self.retire_lower_enter_view_admission_waits(installed);
        Ok(())
    }
    fn retire_lower_enter_view_admission_waits(&mut self, installed: LifecycleKey) {
        self.admission_waits.retain(|_, waiting| {
            let candidate = &waiting.candidate;
            candidate.work_class != LifecycleWorkClass::EnterView
                || candidate.key.context != installed.context
                || candidate.key.round.height != installed.round.height
                || candidate.key.round.view >= installed.round.view
        });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn production_coordinator_stays_below_the_architecture_line_budget() {
        let source = include_str!("v2_lifecycle_coordinator.rs");
        let production = source
            .split_once("\n#[cfg(test)]\nmod tests {")
            .expect("coordinator keeps one exact production/test boundary")
            .0;
        let code_lines = production
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with("//"))
            .count();
        assert!(
            code_lines < 1_500,
            "LifecycleCoordinator production surface grew to {code_lines} code lines"
        );
    }
    fn digest(byte: u8) -> LifecycleDigest {
        LifecycleDigest::new([byte; 32])
    }
    fn context() -> LifecycleContext {
        LifecycleContext::new(digest(1), 7)
    }
    fn stage_kind_for_phase(phase: LifecyclePhase) -> LifecycleStageKind {
        match phase {
            LifecyclePhase::Proposal => LifecycleStageKind::SignProposal,
            LifecyclePhase::Prepare => LifecycleStageKind::SignPrepareVote,
            LifecyclePhase::Commit => LifecycleStageKind::SignCommitVote,
            LifecyclePhase::Timeout => LifecycleStageKind::SignTimeoutVote,
            LifecyclePhase::Fetch => LifecycleStageKind::FetchBody,
            LifecyclePhase::Store => LifecycleStageKind::StoreBody,
            LifecyclePhase::Validate => LifecycleStageKind::ValidateBody,
            LifecyclePhase::Apply => LifecycleStageKind::ApplyDecision,
            LifecyclePhase::BroadcastProposal => LifecycleStageKind::BroadcastProposal,
            LifecyclePhase::BroadcastPrepareVote => LifecycleStageKind::BroadcastPrepareVote,
            LifecyclePhase::BroadcastCommitVote => LifecycleStageKind::BroadcastCommitVote,
            LifecyclePhase::BroadcastPrepareQc => LifecycleStageKind::BroadcastPrepareQc,
            LifecyclePhase::BroadcastCommitQc => LifecycleStageKind::BroadcastCommitQc,
            LifecyclePhase::BroadcastTimeoutVote => LifecycleStageKind::BroadcastTimeoutVote,
            LifecyclePhase::BroadcastTc => LifecycleStageKind::BroadcastTc,
            LifecyclePhase::EnterView => LifecycleStageKind::EnterView,
            LifecyclePhase::DiagnosticProposalEquivocation => {
                LifecycleStageKind::ReportProposalEquivocation
            }
            LifecyclePhase::DiagnosticVoteEquivocation => {
                LifecycleStageKind::ReportVoteEquivocation
            }
            LifecyclePhase::DiagnosticTimeoutEquivocation => {
                LifecycleStageKind::ReportTimeoutEquivocation
            }
            LifecyclePhase::DiagnosticInvalidBody => LifecycleStageKind::ReportInvalidBody,
            LifecyclePhase::Serve => LifecycleStageKind::CertifiedServe,
            LifecyclePhase::ProducerTurn => LifecycleStageKind::ProducerTurn,
        }
    }
    fn key(seed: u8, phase: LifecyclePhase) -> LifecycleKey {
        super::replay_authority::exact_record_fixture(context(), stage_kind_for_phase(phase), seed)
            .key
    }
    fn stage(
        kind: LifecycleStageKind,
        _seed: u16,
        predecessor_scope: PredecessorScope,
    ) -> LifecycleStage {
        LifecycleStage::new(kind, predecessor_scope)
    }
    fn geometry(seed: u8, class: CapacityClass) -> PhysicalGeometry {
        PhysicalGeometry::new(
            [PhysicalSlot::new(
                PhysicalSlotId::for_capacity(class, 0),
                digest(seed),
            )],
            [
                PhysicalSlotId::for_capacity(class, 0),
                PhysicalSlotId::for_capacity(class, 1),
            ],
        )
    }
    fn capacities(limit: usize) -> CapacityGeometry {
        CapacityGeometry::new(CapacityClass::ALL.into_iter().map(|class| (class, limit)))
    }
    fn authority(
        context: LifecycleContext,
        capacity_geometry: CapacityGeometry,
    ) -> AuthenticatedEpisodeAuthority {
        super::authority::test_authority(context, (2_u8..=5).map(digest), 0, capacity_geometry)
            .expect("test authority is valid")
    }
    #[test]
    fn scheduler_rank_is_named_wide_and_lexicographic() {
        let wide = u64::from(u16::MAX) + 1;
        let earlier = SchedulerRank::new(1, wide, 3, 4, 5, 6, 7, 8);
        let later = SchedulerRank::new(2, 0, 0, 0, 0, 0, 0, 0);
        assert_eq!(earlier.components(), [1, wide, 3, 4, 5, 6, 7, 8]);
        assert!(earlier < later);
    }
    #[test]
    fn closed_operation_topologies_strictly_descend_to_their_successors() {
        assert!(
            LifecycleStageKind::FetchBody.remaining_stages()
                > LifecycleStageKind::StoreBody.remaining_stages()
        );
        assert!(
            LifecycleStageKind::StoreBody.remaining_stages()
                > LifecycleStageKind::ValidateBody.remaining_stages()
        );
        assert!(
            LifecycleStageKind::ValidateBody.remaining_stages()
                > LifecycleStageKind::ApplyDecision.remaining_stages()
        );
        assert!(
            LifecycleStageKind::ValidateBody.remaining_stages()
                > LifecycleStageKind::SignPrepareVote.remaining_stages()
        );
        assert!(
            LifecycleStageKind::ValidateBody.remaining_stages()
                > LifecycleStageKind::SignCommitVote.remaining_stages()
        );
        assert!(
            LifecycleStageKind::ValidateBody.remaining_stages()
                > LifecycleStageKind::ReportInvalidBody.remaining_stages()
        );
        assert!(
            LifecycleStageKind::SignProposal.remaining_stages()
                > LifecycleStageKind::BroadcastProposal.remaining_stages()
        );
        assert!(
            LifecycleStageKind::CertifiedServe.remaining_stages()
                > LifecycleStageKind::ProducerTurn.remaining_stages()
        );
        assert!(
            LifecycleStageKind::ALL
                .into_iter()
                .all(|kind| kind.remaining_stages() > 0)
        );
    }
    #[test]
    fn direct_registry_factory_authenticates_empty_census_as_idle() {
        let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
        let registry = LifecycleWorkRegistryHolder::empty();
        let inputs = coordinator
            .direct_registry_scheduler_inputs_for_test(&registry)
            .expect("an empty directly-owned Ready census is complete");
        assert_eq!(coordinator.plan_turn(inputs), TurnPlan::Idle);
    }
    #[test]
    fn direct_registry_factory_rejects_unsealed_fetch_ready_work_without_mutation() {
        let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
        let registry = LifecycleWorkRegistryHolder::empty();
        let (_, ordinal, _) = admitted(coordinator.admit(AdmissionRequest::Candidate(candidate(
            1,
            LifecycleWorkClass::Fetch,
            LifecyclePhase::Fetch,
            InitialLifecycleState::Ready,
            PredecessorScope::Independent,
        ))));
        let before = format!("{coordinator:?}");
        assert_eq!(
            coordinator.direct_registry_scheduler_inputs_for_test(&registry),
            Err(ProductionSchedulerInputsError::InvalidRecoveredDecisionFetchCarrier { ordinal })
        );
        assert_eq!(format!("{coordinator:?}"), before);
    }
    #[test]
    fn direct_registry_factory_rejects_corrupt_ready_index_without_mutation() {
        let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
        let registry = LifecycleWorkRegistryHolder::empty();
        let (_, ordinal, _) = admitted(coordinator.admit(AdmissionRequest::Candidate(candidate(
            2,
            LifecycleWorkClass::Fetch,
            LifecyclePhase::Fetch,
            InitialLifecycleState::Ready,
            PredecessorScope::Independent,
        ))));
        assert!(coordinator.ready_index.remove(&ordinal));
        let before = format!("{coordinator:?}");
        assert_eq!(
            coordinator.direct_registry_scheduler_inputs_for_test(&registry),
            Err(ProductionSchedulerInputsError::InvalidReadyCensus)
        );
        assert_eq!(format!("{coordinator:?}"), before);
    }
    crate::sumeragi::v2_lifecycle_coordinator::source_contract_test!(
        production_scheduler_factory_has_no_raw_rank_mint
    );
    #[test]
    fn planning_requires_a_fresh_rank_for_every_ready_ordinal() {
        let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
        admitted(coordinator.admit(AdmissionRequest::Candidate(candidate(
            1,
            LifecycleWorkClass::Fetch,
            LifecyclePhase::Fetch,
            InitialLifecycleState::Ready,
            PredecessorScope::Independent,
        ))));
        assert_eq!(
            coordinator.plan_turn(SchedulerInputs::new([], []).expect("unique empty snapshot")),
            TurnPlan::FailClosed(CoordinatorFault::InvalidSchedulerInputs)
        );
    }
    #[test]
    fn test_scheduler_input_mint_rejects_duplicates_and_local_generations() {
        let source = WaitSource::External(digest(12));
        assert_eq!(
            SchedulerInputs::new([(source, 1), (source, 2)], []),
            Err(super::schema::SchedulerInputError::DuplicateGenerationSource)
        );
        assert_eq!(
            SchedulerInputs::new([(WaitSource::Capacity(CapacityClass::Effect), 1)], []),
            Err(super::schema::SchedulerInputError::UnsupportedGenerationSource)
        );
        let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
        admitted(coordinator.admit(AdmissionRequest::Candidate(candidate(
            13,
            LifecycleWorkClass::Fetch,
            LifecyclePhase::Fetch,
            InitialLifecycleState::Ready,
            PredecessorScope::Independent,
        ))));
        let row = SchedulerReadyInputs::new(&coordinator.records[&1], None, [0; 6]);
        assert_eq!(
            SchedulerInputs::new([], [(1, row), (1, row)]),
            Err(super::schema::SchedulerInputError::DuplicateReadyOrdinal)
        );
    }
    #[test]
    fn malformed_ready_census_cannot_publish_a_generation_before_failure() {
        let source = WaitSource::External(digest(14));
        let wait = WaitToken::new(source, 0);
        let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
        admit_waiting_fetch(&mut coordinator, 14, wait, PredecessorScope::Independent);
        assert_eq!(coordinator.records[&1].state, LifecycleState::Waiting(wait));
        assert!(coordinator.ready_index.is_empty());
        let record = &coordinator.records[&1];
        let forbidden_attestation = SchedulerReadyInputs::new(record, Some(false), [0; 6]);
        let foreign = SchedulerReadyInputs::with_identity_for_test(
            record,
            OwnerId::new(CausalRoot::new(digest(0xEE)), 1),
            record.key,
            Some(false),
            [0; 6],
        );
        for malformed_rows in [
            vec![],
            vec![(1, forbidden_attestation)],
            vec![(1, forbidden_attestation), (2, forbidden_attestation)],
            vec![(1, foreign)],
        ] {
            let mut trial = coordinator.clone();
            let malformed = SchedulerInputs::new([(source, 1)], malformed_rows)
                .expect("the malformed census itself has unique input identities");
            assert_eq!(
                trial.plan_turn(malformed),
                TurnPlan::FailClosed(CoordinatorFault::InvalidSchedulerInputs)
            );
            assert_eq!(trial.records[&1].state, LifecycleState::Waiting(wait));
            assert!(trial.ready_index.is_empty());
            assert_eq!(trial.observed_generation.get(&source), Some(&0));
        }
    }
    #[test]
    fn fresh_rank_snapshot_selects_and_is_bound_to_the_lease() {
        let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
        for seed in [1, 2] {
            admitted(coordinator.admit(AdmissionRequest::Candidate(candidate(
                seed,
                LifecycleWorkClass::Fetch,
                LifecyclePhase::Fetch,
                InitialLifecycleState::Ready,
                PredecessorScope::Independent,
            ))));
        }
        let selected_rank = SchedulerRank::new(5, 0, 1, 0, 0, 0, 0, 0);
        let lease = execute(plan_turn_with_modes(&mut coordinator, [(1, 100), (2, 1)]));
        assert_eq!(lease.ordinal(), 2);
        assert_eq!(lease.rank(), selected_rank);
    }
    #[test]
    fn physical_capacity_filters_selection_without_removing_ready_rows() {
        let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
        for seed in [0x31, 0x32] {
            admitted(coordinator.admit(AdmissionRequest::Candidate(candidate(
                seed,
                LifecycleWorkClass::Fetch,
                LifecyclePhase::Fetch,
                InitialLifecycleState::Ready,
                PredecessorScope::Independent,
            ))));
        }
        let inputs = SchedulerInputs::new(
            [],
            [
                (
                    1,
                    SchedulerReadyInputs::new(&coordinator.records[&1], None, [0; 6])
                        .with_physical_capacity_for_test(false),
                ),
                (
                    2,
                    SchedulerReadyInputs::new(&coordinator.records[&2], None, [100, 0, 0, 0, 0, 0]),
                ),
            ],
        )
        .expect("two exact physical-capacity rows");
        let lease = execute(coordinator.plan_turn(inputs));
        assert_eq!(lease.ordinal(), 2);
        assert_eq!(coordinator.records[&1].state, LifecycleState::Ready);
        assert!(coordinator.ready_index.contains(&1));

        assert!(coordinator.rollback_unpublished_turn(&lease));
        let unavailable = SchedulerInputs::new(
            [],
            coordinator.ready_index.iter().map(|ordinal| {
                (
                    *ordinal,
                    SchedulerReadyInputs::new(&coordinator.records[ordinal], None, [0; 6])
                        .with_physical_capacity_for_test(false),
                )
            }),
        )
        .expect("the unchanged Ready census remains authenticated");
        assert_eq!(coordinator.plan_turn(unavailable), TurnPlan::Idle);
        assert!(
            coordinator
                .records
                .values()
                .all(|record| record.state == LifecycleState::Ready)
        );
        assert_eq!(coordinator.ready_index, BTreeSet::from([1, 2]));
    }

    #[test]
    fn validate_ready_census_requires_an_exact_carrier_attestation() {
        let source = WaitSource::External(digest(0xD1));
        let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
        admitted(coordinator.admit(AdmissionRequest::Candidate(candidate(
            0xD1,
            LifecycleWorkClass::Validate,
            LifecyclePhase::Validate,
            InitialLifecycleState::Ready,
            PredecessorScope::Independent,
        ))));
        let missing = SchedulerInputs::new(
            [(source, 1)],
            [(
                1,
                SchedulerReadyInputs::new(&coordinator.records[&1], None, [0; 6]),
            )],
        )
        .expect("one unique but unattested Validate row");
        assert_eq!(
            coordinator.plan_turn(missing),
            TurnPlan::FailClosed(CoordinatorFault::InvalidSchedulerInputs)
        );
        assert_eq!(coordinator.observed_generation.get(&source), None);
        let mut stale = LifecycleCoordinator::new(context(), 0, capacities(8));
        admitted(stale.admit(AdmissionRequest::Candidate(candidate(
            0xD2,
            LifecycleWorkClass::Validate,
            LifecyclePhase::Validate,
            InitialLifecycleState::Ready,
            PredecessorScope::Independent,
        ))));
        let inputs = scheduler_inputs(&stale, []);
        let slot = *stale.records[&1]
            .physical_slots
            .first_key_value()
            .expect("one Validate slot")
            .0;
        stale
            .records
            .get_mut(&1)
            .expect("Validate record")
            .physical_slots
            .insert(slot, digest(0xEE));
        assert_eq!(
            stale.plan_turn(inputs),
            TurnPlan::FailClosed(CoordinatorFault::InvalidSchedulerInputs)
        );
        assert!(stale.active_lease.is_none());
    }
    #[test]
    fn validate_admission_requires_a_key_bound_body_frame() {
        let mut foreign = candidate(
            0xD8,
            LifecycleWorkClass::Validate,
            LifecyclePhase::Validate,
            InitialLifecycleState::Ready,
            PredecessorScope::Independent,
        );
        let DurablePayloadReference::BodyFrame(mut frame) = foreign.payload else {
            panic!("Validate fixture carries one body frame")
        };
        frame.context = digest(0xF8);
        foreign.payload = DurablePayloadReference::BodyFrame(frame);
        let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
        assert_eq!(
            coordinator.admit(AdmissionRequest::Candidate(foreign)),
            AdmissionDecision::Rejected(AdmissionRejection::InvalidDurableMetadata)
        );
        assert!(coordinator.records.is_empty());
        assert!(coordinator.durable_records.is_empty());
    }
    #[test]
    fn serve_admission_rejects_individually_valid_foreign_producer_family() {
        let mut candidate = serve_candidate(0xD9, InitialLifecycleState::Ready);
        candidate
            .producer_turn
            .as_mut()
            .expect("Serve fixture has one reserved producer")
            .replay_authority =
            super::replay_authority::foreign_certified_serve_family_authority_fixture(
                context(),
                LifecycleStageKind::ProducerTurn,
                0xD9,
            );
        let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
        assert_eq!(
            coordinator.admit(AdmissionRequest::Candidate(candidate)),
            AdmissionDecision::Rejected(AdmissionRejection::InvalidDurableMetadata)
        );
        assert!(coordinator.records.is_empty());
        assert!(coordinator.durable_records.is_empty());
    }
    #[test]
    fn rejected_validate_is_capacity_gated_before_claim_and_cannot_lasso() {
        let geometry = capacities(1);
        let external = WaitToken::new(WaitSource::External(digest(0xD3)), 0);
        let mut blocked = LifecycleCoordinator::new(context(), 0, geometry.clone());
        admitted(blocked.admit(AdmissionRequest::Candidate(capacity_matched(
            candidate(
                0xD3,
                LifecycleWorkClass::Broadcast,
                LifecyclePhase::BroadcastProposal,
                InitialLifecycleState::Waiting(external),
                PredecessorScope::Independent,
            ),
            &geometry,
        ))));
        let (_, validate_ordinal, _) =
            admitted(blocked.admit(AdmissionRequest::Candidate(capacity_matched(
                candidate(
                    0xD4,
                    LifecycleWorkClass::Validate,
                    LifecyclePhase::Validate,
                    InitialLifecycleState::Ready,
                    PredecessorScope::Independent,
                ),
                &geometry,
            ))));
        let capacity_wait = WaitToken::new(WaitSource::Capacity(CapacityClass::Consensus), 0);
        let mut validated = blocked.clone();
        let validated_inputs =
            scheduler_inputs_with_validate_kinds(&validated, [], [(validate_ordinal, false)]);
        let validated_lease = execute(validated.plan_turn(validated_inputs));
        assert_eq!(validated_lease.ordinal(), validate_ordinal);
        assert_eq!(validated_lease.output_reservation(), None);
        let inputs = scheduler_inputs_with_validate_kinds(&blocked, [], [(validate_ordinal, true)]);
        assert_eq!(
            blocked.plan_turn(inputs),
            TurnPlan::Waiting(BTreeSet::from([external, capacity_wait]))
        );
        assert_eq!(
            blocked.records[&validate_ordinal].state,
            LifecycleState::Ready
        );
        assert!(blocked.active_lease.is_none());
        let mut coordinator = LifecycleCoordinator::new(context(), 0, geometry.clone());
        let (_, validate_ordinal, _) = admitted(coordinator.admit(AdmissionRequest::Candidate(
            capacity_matched(
                candidate(
                    0xD5,
                    LifecycleWorkClass::Validate,
                    LifecyclePhase::Validate,
                    InitialLifecycleState::Ready,
                    PredecessorScope::Independent,
                ),
                &geometry,
            ),
        )));
        let (_, release_ordinal, _) = admitted(coordinator.admit(AdmissionRequest::Candidate(
            capacity_matched(
                candidate(
                    0xD6,
                    LifecycleWorkClass::Broadcast,
                    LifecyclePhase::BroadcastProposal,
                    InitialLifecycleState::Ready,
                    PredecessorScope::Independent,
                ),
                &geometry,
            ),
        )));
        let release_inputs =
            scheduler_inputs_with_validate_kinds(&coordinator, [], [(validate_ordinal, true)]);
        let release = execute(coordinator.plan_turn(release_inputs));
        assert_eq!(release.ordinal(), release_ordinal);
        assert_eq!(release.output_reservation(), None);
        coordinator.settle_turn(release, TurnOutcome::Advanced);
        assert_eq!(
            coordinator.capacity_generation[&CapacityClass::Consensus],
            1
        );
        let reserved_inputs =
            scheduler_inputs_with_validate_kinds(&coordinator, [], [(validate_ordinal, true)]);
        let reserved = execute(coordinator.plan_turn(reserved_inputs));
        assert_eq!(reserved.ordinal(), validate_ordinal);
        assert_eq!(
            reserved
                .output_reservation()
                .map(super::schema::LeaseCapacityReservation::wait_token),
            Some(WaitToken::new(
                WaitSource::Capacity(CapacityClass::Consensus),
                1,
            ))
        );
        assert_eq!(coordinator.capacity_used[&CapacityClass::Consensus], 0);
        assert_eq!(
            coordinator.admit(AdmissionRequest::Candidate(capacity_matched(
                candidate(
                    0xD7,
                    LifecycleWorkClass::Broadcast,
                    LifecyclePhase::BroadcastProposal,
                    InitialLifecycleState::Ready,
                    PredecessorScope::Independent,
                ),
                &geometry,
            ))),
            AdmissionDecision::WaitForCapacity(WaitToken::new(
                WaitSource::Capacity(CapacityClass::Consensus),
                1,
            ))
        );
        coordinator.settle_turn(reserved.clone(), TurnOutcome::Advanced);
        assert_eq!(coordinator.active_lease, Some(reserved));
        assert_eq!(
            coordinator.fault,
            Some(CoordinatorFault::InvalidTerminalOutcome)
        );
    }
    fn capacity_matched(
        mut candidate: CandidateAdmission,
        geometry: &CapacityGeometry,
    ) -> CandidateAdmission {
        retain_slots_within_capacity(&mut candidate.physical_geometry, geometry);
        if let Some(producer) = candidate.producer_turn.as_mut() {
            retain_slots_within_capacity(&mut producer.physical_geometry, geometry);
        }
        candidate
    }
    fn retain_slots_within_capacity(physical: &mut PhysicalGeometry, geometry: &CapacityGeometry) {
        let admitted = |slot: PhysicalSlotId| {
            slot.capacity_class().is_some_and(|class| {
                usize::from(slot.1) < geometry.limits.get(&class).copied().unwrap_or(0)
            })
        };
        physical.initial.retain(|slot| admitted(slot.id));
        physical.replenishment_slots.retain(|slot| admitted(*slot));
    }
    fn recovery_capacity_matched(
        mut record: RecoveredLifecycleRecord,
        geometry: &CapacityGeometry,
    ) -> RecoveredLifecycleRecord {
        record.physical_slot_universe.retain(|slot| {
            slot.capacity_class().is_some_and(|class| {
                usize::from(slot.index()) < geometry.limits.get(&class).copied().unwrap_or(0)
            })
        });
        record
    }
    fn candidate(
        seed: u8,
        work_class: LifecycleWorkClass,
        phase: LifecyclePhase,
        initial_state: InitialLifecycleState,
        predecessor_scope: PredecessorScope,
    ) -> CandidateAdmission {
        let kind = stage_kind_for_phase(phase);
        let replay = super::replay_authority::exact_record_fixture(context(), kind, seed);
        assert_eq!((replay.work_class, replay.key.phase()), (work_class, phase));
        let mut candidate = CandidateAdmission::new(
            replay.key,
            CausalRoot::new(digest(seed.wrapping_add(128))),
            work_class,
            stage(kind, u16::from(seed), predecessor_scope),
            initial_state,
            digest(seed.wrapping_add(96)),
            replay.payload,
            replay.authority,
            geometry(seed, work_class.capacity_class()),
            None,
        );
        if work_class == LifecycleWorkClass::CertifiedServe {
            candidate.causal_root = CausalRoot::new(candidate.reconstruction_source);
        }
        candidate
    }
    fn serve_candidate(seed: u8, initial_state: InitialLifecycleState) -> CandidateAdmission {
        let mut serve = candidate(
            seed,
            LifecycleWorkClass::CertifiedServe,
            LifecyclePhase::Serve,
            initial_state,
            PredecessorScope::ReadyOrdinalPrefix,
        );
        serve.causal_root = CausalRoot::new(serve.reconstruction_source);
        let replay = super::replay_authority::exact_record_fixture(
            context(),
            LifecycleStageKind::ProducerTurn,
            seed,
        );
        let producer = ProducerTurnAdmission::new(
            replay.key,
            stage(
                LifecycleStageKind::ProducerTurn,
                u16::from(seed),
                PredecessorScope::ProducerHandoffBarrier,
            ),
            digest(seed.wrapping_add(96)),
            replay.authority,
            geometry(seed.wrapping_add(1), CapacityClass::Producer),
        );
        serve.producer_turn = Some(producer);
        serve
    }
    fn admitted(decision: AdmissionDecision) -> (OwnerId, u128, Option<u128>) {
        let AdmissionDecision::Admitted {
            owner,
            ordinal,
            producer_turn_ordinal,
        } = decision
        else {
            panic!("expected admission, found {decision:?}");
        };
        (owner, ordinal, producer_turn_ordinal)
    }
    fn admit_waiting_fetch(
        coordinator: &mut LifecycleCoordinator,
        seed: u8,
        wait: WaitToken,
        predecessor_scope: PredecessorScope,
    ) -> (OwnerId, u128, Option<u128>) {
        let admitted = admitted(coordinator.admit(AdmissionRequest::Candidate(candidate(
            seed,
            LifecycleWorkClass::Fetch,
            LifecyclePhase::Fetch,
            InitialLifecycleState::Ready,
            predecessor_scope,
        ))));
        let ordinal = admitted.1;
        assert!(coordinator.ready_index.remove(&ordinal));
        coordinator
            .records
            .get_mut(&ordinal)
            .expect("Fetch record")
            .state = LifecycleState::Waiting(wait);
        coordinator.advance_observed_generation(wait.source, wait.observed_generation);
        admitted
    }
    fn execute(plan: TurnPlan) -> TurnLease {
        let TurnPlan::Execute(lease) = plan else {
            panic!("expected executable turn, found {plan:?}");
        };
        lease
    }
    #[test]
    fn unpublished_turn_rollback_restores_ready_and_clears_the_active_lease() {
        let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
        let (_, ordinal, _) = admitted(coordinator.admit(AdmissionRequest::Candidate(candidate(
            91,
            LifecycleWorkClass::SignTimeout,
            LifecyclePhase::Timeout,
            InitialLifecycleState::Ready,
            PredecessorScope::Independent,
        ))));
        let lease = execute(plan_turn(&mut coordinator, []));
        assert_eq!(lease.ordinal(), ordinal);
        assert!(matches!(
            coordinator.records[&ordinal].state,
            LifecycleState::Claimed(id) if id == lease.id()
        ));
        assert!(!coordinator.ready_index.contains(&ordinal));
        assert!(coordinator.rollback_unpublished_turn(&lease));
        assert_eq!(coordinator.records[&ordinal].state, LifecycleState::Ready);
        assert!(coordinator.ready_index.contains(&ordinal));
        assert!(coordinator.active_lease.is_none());
        assert!(
            !coordinator.rollback_unpublished_turn(&lease),
            "one unpublished claim can roll back at most once"
        );
    }
    #[test]
    fn unpublished_reserved_turn_rollback_releases_overlay_and_wakes_capacity() {
        let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
        let (_, ordinal, _) = admitted(coordinator.admit(AdmissionRequest::Candidate(candidate(
            92,
            LifecycleWorkClass::SignTimeout,
            LifecyclePhase::Timeout,
            InitialLifecycleState::Ready,
            PredecessorScope::Independent,
        ))));
        let mut lease = execute(plan_turn(&mut coordinator, []));
        let generation = coordinator.capacity_generation[&CapacityClass::Consensus];
        lease.output_reservation = Some(LeaseCapacityReservation::new(
            CapacityClass::Consensus,
            generation,
        ));
        coordinator.active_lease = Some(lease.clone());
        assert!(coordinator.rollback_unpublished_reserved_turn(&lease, CapacityClass::Consensus));
        assert_eq!(coordinator.records[&ordinal].state, LifecycleState::Ready);
        assert!(coordinator.ready_index.contains(&ordinal));
        assert!(coordinator.active_lease.is_none());
        assert_eq!(
            coordinator.capacity_generation[&CapacityClass::Consensus],
            generation + 1
        );
        assert_eq!(coordinator.capacity_used[&CapacityClass::Consensus], 0);
        assert!(!coordinator.rollback_unpublished_reserved_turn(&lease, CapacityClass::Consensus));
    }
    fn completed_serve(response_seed: u8) -> TurnOutcome {
        TurnOutcome::Terminal(TerminalOutcome::Completed(Some(digest(response_seed))))
    }
    fn settle_with_test_serve_receipt(
        coordinator: &mut LifecycleCoordinator,
        lease: TurnLease,
        outcome: TurnOutcome,
    ) {
        if lease.work_class == LifecycleWorkClass::CertifiedServe
            && let TurnOutcome::Terminal(terminal) = outcome
            && let Some(replay) = test_serve_terminal_replay(coordinator, &lease, terminal)
        {
            coordinator.settle_turn_with_durable_serve_terminal(lease, replay);
        } else {
            coordinator.settle_turn(lease, outcome);
        }
    }
    fn test_serve_terminal_replay(
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
        terminal: TerminalOutcome,
    ) -> Option<super::replay_authority::CertifiedServeTerminalReplayAuthorityPairV1> {
        let producer_ordinal = coordinator.producer_debts.get(&lease.ordinal).copied()?;
        super::replay_authority::CertifiedServeTerminalReplayAuthorityPairV1::from_test_terminal_outcome(
            coordinator.active_context,
            &coordinator.records[&lease.ordinal],
            &coordinator.durable_records[&lease.ordinal],
            &coordinator.records[&producer_ordinal],
            &coordinator.durable_records[&producer_ordinal],
            terminal,
        )
    }
    fn scheduler_inputs(
        coordinator: &LifecycleCoordinator,
        generations: impl IntoIterator<Item = (WaitSource, u64)>,
    ) -> SchedulerInputs {
        scheduler_inputs_with_validate_kinds(coordinator, generations, [])
    }
    fn scheduler_inputs_with_validate_kinds(
        coordinator: &LifecycleCoordinator,
        generations: impl IntoIterator<Item = (WaitSource, u64)>,
        rejected_validate: impl IntoIterator<Item = (u128, bool)>,
    ) -> SchedulerInputs {
        let generations: BTreeMap<_, _> = generations.into_iter().collect();
        let rejected_validate: BTreeMap<_, _> = rejected_validate.into_iter().collect();
        SchedulerInputs::new(
            generations
                .iter()
                .map(|(source, generation)| (*source, *generation)),
            coordinator.records.iter().filter_map(|(ordinal, record)| {
                let ready = match record.state {
                    LifecycleState::Ready => true,
                    LifecycleState::Waiting(wait)
                        if matches!(
                            wait.source,
                            WaitSource::External(_) | WaitSource::Recovery(_)
                        ) =>
                    {
                        coordinator
                            .observed_generation
                            .get(&wait.source)
                            .copied()
                            .unwrap_or(0)
                            .max(generations.get(&wait.source).copied().unwrap_or(0))
                            > wait.observed_generation
                    }
                    LifecycleState::Waiting(_)
                    | LifecycleState::Claimed(_)
                    | LifecycleState::Terminal(_) => false,
                };
                ready.then_some((
                    *ordinal,
                    SchedulerReadyInputs::new(
                        record,
                        (record.work_class == LifecycleWorkClass::Validate)
                            .then(|| rejected_validate.get(ordinal).copied().unwrap_or(false)),
                        [0, 0, record.key.round.view, 0, 0, 0],
                    ),
                ))
            }),
        )
        .expect("test scheduler inputs are unique and externally generated")
    }
    fn plan_turn(
        coordinator: &mut LifecycleCoordinator,
        generations: impl IntoIterator<Item = (WaitSource, u64)>,
    ) -> TurnPlan {
        let inputs = scheduler_inputs(coordinator, generations);
        coordinator.plan_turn(inputs)
    }
    fn plan_turn_with_modes(
        coordinator: &mut LifecycleCoordinator,
        modes: impl IntoIterator<Item = (u128, u64)>,
    ) -> TurnPlan {
        let modes: BTreeMap<_, _> = modes.into_iter().collect();
        let rows: Vec<_> = coordinator
            .ready_index
            .iter()
            .map(|ordinal| {
                let record = &coordinator.records[ordinal];
                (
                    *ordinal,
                    SchedulerReadyInputs::new(
                        record,
                        (record.work_class == LifecycleWorkClass::Validate).then_some(false),
                        [modes[ordinal], 0, 0, 0, 0, 0],
                    ),
                )
            })
            .collect();
        coordinator.plan_turn(
            SchedulerInputs::new([], rows).expect("mode rows have unique ready ordinals"),
        )
    }
    #[test]
    fn admission_is_exact_and_foreign_owner_is_rejected_before_physical_validation() {
        let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
        assert_eq!(
            coordinator.admit(AdmissionRequest::NonCandidate(NonCandidateEffect(()))),
            AdmissionDecision::NonCandidate
        );
        assert_eq!(coordinator.high_water, 0);
        let first = candidate(
            3,
            LifecycleWorkClass::Fetch,
            LifecyclePhase::Fetch,
            InitialLifecycleState::Ready,
            PredecessorScope::Independent,
        );
        let (owner, ordinal, producer) =
            admitted(coordinator.admit(AdmissionRequest::Candidate(first.clone())));
        assert_eq!((ordinal, producer, coordinator.high_water), (1, None, 1));
        assert_eq!(coordinator.records.len(), 1);
        assert_eq!(
            coordinator.admit(AdmissionRequest::Candidate(first.clone())),
            AdmissionDecision::Retry {
                owner,
                ordinal,
                action: RetryAction::ReenqueueIncumbent,
            }
        );
        assert_eq!((coordinator.records.len(), coordinator.high_water), (1, 1));
        let mut foreign = first;
        foreign.causal_root = CausalRoot::new(digest(250));
        foreign.physical_geometry = PhysicalGeometry::new(
            [
                PhysicalSlot::new(PhysicalSlotId::new(0, 0), digest(8)),
                PhysicalSlot::new(PhysicalSlotId::new(0, 0), digest(9)),
            ],
            [],
        );
        assert_eq!(
            coordinator.admit(AdmissionRequest::Candidate(foreign)),
            AdmissionDecision::Rejected(AdmissionRejection::ForeignOwner)
        );
        assert_eq!((coordinator.records.len(), coordinator.high_water), (1, 1));
    }
    #[test]
    fn retry_policy_is_exhaustive_for_every_work_class() {
        let cases = [
            (
                LifecycleWorkClass::SignProposal,
                RetryAction::StutterLiveSigner,
            ),
            (LifecycleWorkClass::SignVote, RetryAction::StutterLiveSigner),
            (
                LifecycleWorkClass::SignTimeout,
                RetryAction::StutterLiveSigner,
            ),
            (LifecycleWorkClass::Fetch, RetryAction::ReenqueueIncumbent),
            (LifecycleWorkClass::Store, RetryAction::ReenqueueIncumbent),
            (
                LifecycleWorkClass::Validate,
                RetryAction::ReenqueueIncumbent,
            ),
            (LifecycleWorkClass::Apply, RetryAction::RefanoutIncumbent),
            (LifecycleWorkClass::Broadcast, RetryAction::RefanoutEnvelope),
            (
                LifecycleWorkClass::EnterView,
                RetryAction::StutterInstalledView,
            ),
            (
                LifecycleWorkClass::EquivocationReport,
                RetryAction::StutterDiagnostic,
            ),
            (
                LifecycleWorkClass::InvalidBodyReport,
                RetryAction::StutterDiagnostic,
            ),
        ];
        for (index, (work_class, action)) in cases.into_iter().enumerate() {
            let seed = u8::try_from(index + 40).expect("small retry fixture index");
            let phase = match work_class {
                LifecycleWorkClass::SignProposal => LifecyclePhase::Proposal,
                LifecycleWorkClass::SignVote => LifecyclePhase::Prepare,
                LifecycleWorkClass::SignTimeout => LifecyclePhase::Timeout,
                LifecycleWorkClass::Fetch => LifecyclePhase::Fetch,
                LifecycleWorkClass::Store => LifecyclePhase::Store,
                LifecycleWorkClass::Validate => LifecyclePhase::Validate,
                LifecycleWorkClass::Apply => LifecyclePhase::Apply,
                LifecycleWorkClass::Broadcast => LifecyclePhase::BroadcastProposal,
                LifecycleWorkClass::EnterView => LifecyclePhase::EnterView,
                LifecycleWorkClass::EquivocationReport => {
                    LifecyclePhase::DiagnosticProposalEquivocation
                }
                LifecycleWorkClass::InvalidBodyReport => LifecyclePhase::DiagnosticInvalidBody,
                LifecycleWorkClass::CertifiedServe | LifecycleWorkClass::ProducerTurn => {
                    unreachable!("handled by dedicated retry fixtures")
                }
            };
            let request = candidate(
                seed,
                work_class,
                phase,
                InitialLifecycleState::Ready,
                PredecessorScope::Independent,
            );
            let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
            let (owner, ordinal, _) =
                admitted(coordinator.admit(AdmissionRequest::Candidate(request.clone())));
            assert_eq!(
                coordinator.admit(AdmissionRequest::Candidate(request)),
                AdmissionDecision::Retry {
                    owner,
                    ordinal,
                    action,
                }
            );
        }
        let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
        let serve = serve_candidate(90, InitialLifecycleState::Ready);
        let (owner, ordinal, producer) =
            admitted(coordinator.admit(AdmissionRequest::Candidate(serve.clone())));
        assert_eq!(
            coordinator.admit(AdmissionRequest::Candidate(serve.clone())),
            AdmissionDecision::Retry {
                owner,
                ordinal,
                action: RetryAction::ReenqueueIncumbent,
            }
        );
        let mut drifted_request = serve.clone();
        let DurablePayloadReference::CertifiedServePending { request, .. } =
            &mut drifted_request.payload
        else {
            panic!("Serve retry carries a pending payload receipt")
        };
        *request = digest(255);
        assert_eq!(
            coordinator.admit(AdmissionRequest::Candidate(drifted_request)),
            AdmissionDecision::Rejected(AdmissionRejection::InvalidDurableMetadata)
        );
        let mut drifted_producer = serve.clone();
        drifted_producer
            .producer_turn
            .as_mut()
            .expect("Serve retry carries its producer companion")
            .key
            .subject = Some(digest(1));
        assert_eq!(
            coordinator.admit(AdmissionRequest::Candidate(drifted_producer)),
            AdmissionDecision::Rejected(AdmissionRejection::InvalidDurableMetadata)
        );
        let mut missing_producer = serve;
        missing_producer.producer_turn = None;
        assert_eq!(
            coordinator.admit(AdmissionRequest::Candidate(missing_producer)),
            AdmissionDecision::Rejected(AdmissionRejection::SemanticDrift)
        );
        let producer_ordinal = producer.expect("Serve reserves one producer turn");
        let producer_record = coordinator.records[&producer_ordinal].clone();
        let producer_retry = CandidateAdmission::new(
            producer_record.key,
            producer_record.owner.causal_root,
            LifecycleWorkClass::ProducerTurn,
            producer_record.stage,
            InitialLifecycleState::Ready,
            coordinator.durable_records[&producer_ordinal].reconstruction_source,
            DurablePayloadReference::None,
            coordinator.durable_records[&producer_ordinal]
                .replay_authority
                .clone(),
            PhysicalGeometry::new([], []),
            None,
        );
        assert_eq!(
            coordinator.admit(AdmissionRequest::Candidate(producer_retry)),
            AdmissionDecision::Retry {
                owner,
                ordinal: producer_ordinal,
                action: RetryAction::StutterProducerTurn,
            }
        );
        let mut fresh = LifecycleCoordinator::new(context(), 0, capacities(8));
        assert_eq!(
            fresh.admit(AdmissionRequest::Candidate(candidate(
                91,
                LifecycleWorkClass::ProducerTurn,
                LifecyclePhase::ProducerTurn,
                InitialLifecycleState::Ready,
                PredecessorScope::ProducerHandoffBarrier,
            ))),
            AdmissionDecision::Rejected(AdmissionRejection::InvalidProducerTurn)
        );
    }
    #[test]
    fn capacity_classes_match_the_existing_runtime_resources() {
        assert!(has_lifecycle_record_capacity(
            MAX_LIFECYCLE_RECORDS_PER_HEIGHT - 2,
            2
        ));
        assert!(!has_lifecycle_record_capacity(
            MAX_LIFECYCLE_RECORDS_PER_HEIGHT - 1,
            2
        ));
        assert!(!has_lifecycle_record_capacity(usize::MAX, 1));
        for work_class in [
            LifecycleWorkClass::SignProposal,
            LifecycleWorkClass::SignVote,
            LifecycleWorkClass::SignTimeout,
            LifecycleWorkClass::Fetch,
            LifecycleWorkClass::Store,
            LifecycleWorkClass::Validate,
            LifecycleWorkClass::Apply,
        ] {
            assert_eq!(work_class.capacity_class(), CapacityClass::Effect);
        }
        for work_class in [
            LifecycleWorkClass::Broadcast,
            LifecycleWorkClass::EnterView,
            LifecycleWorkClass::EquivocationReport,
            LifecycleWorkClass::InvalidBodyReport,
        ] {
            assert_eq!(work_class.capacity_class(), CapacityClass::Consensus);
        }
        assert_eq!(
            LifecycleWorkClass::CertifiedServe.capacity_class(),
            CapacityClass::Serve
        );
        assert_eq!(
            LifecycleWorkClass::ProducerTurn.capacity_class(),
            CapacityClass::Producer
        );
    }
    #[test]
    fn broadcast_and_diagnostic_statement_kinds_are_closed_and_distinct() {
        let broadcast_phases = [
            LifecyclePhase::BroadcastProposal,
            LifecyclePhase::BroadcastPrepareVote,
            LifecyclePhase::BroadcastCommitVote,
            LifecyclePhase::BroadcastPrepareQc,
            LifecyclePhase::BroadcastCommitQc,
            LifecyclePhase::BroadcastTimeoutVote,
            LifecyclePhase::BroadcastTc,
        ];
        for (index, phase) in broadcast_phases.into_iter().enumerate() {
            let seed = u8::try_from(index + 110).expect("bounded Broadcast phase index");
            let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
            admitted(coordinator.admit(AdmissionRequest::Candidate(candidate(
                seed,
                LifecycleWorkClass::Broadcast,
                phase,
                InitialLifecycleState::Ready,
                PredecessorScope::Independent,
            ))));
        }
        for (index, phase) in [
            LifecyclePhase::DiagnosticProposalEquivocation,
            LifecyclePhase::DiagnosticVoteEquivocation,
            LifecyclePhase::DiagnosticTimeoutEquivocation,
        ]
        .into_iter()
        .enumerate()
        {
            let seed = u8::try_from(index + 120).expect("bounded diagnostic phase index");
            let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
            admitted(coordinator.admit(AdmissionRequest::Candidate(candidate(
                seed,
                LifecycleWorkClass::EquivocationReport,
                phase,
                InitialLifecycleState::Ready,
                PredecessorScope::Independent,
            ))));
        }
        let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
        admitted(coordinator.admit(AdmissionRequest::Candidate(candidate(
            124,
            LifecycleWorkClass::InvalidBodyReport,
            LifecyclePhase::DiagnosticInvalidBody,
            InitialLifecycleState::Ready,
            PredecessorScope::Independent,
        ))));
        let mut wrong_broadcast_class = candidate(
            125,
            LifecycleWorkClass::SignVote,
            LifecyclePhase::Commit,
            InitialLifecycleState::Ready,
            PredecessorScope::Independent,
        );
        wrong_broadcast_class.work_class = LifecycleWorkClass::Broadcast;
        assert_eq!(
            coordinator.admit(AdmissionRequest::Candidate(wrong_broadcast_class)),
            AdmissionDecision::Rejected(AdmissionRejection::InvalidDurableMetadata)
        );
        let mut wrong_execution_stage = candidate(
            126,
            LifecycleWorkClass::Fetch,
            LifecyclePhase::Fetch,
            InitialLifecycleState::Ready,
            PredecessorScope::Independent,
        );
        wrong_execution_stage.stage.kind = LifecycleStageKind::StoreBody;
        assert_eq!(
            coordinator.admit(AdmissionRequest::Candidate(wrong_execution_stage)),
            AdmissionDecision::Rejected(AdmissionRejection::InvalidDurableMetadata)
        );
    }
    #[test]
    fn full_capacity_waits_without_allocating_and_retries_after_release() {
        let geometry = capacities(1);
        let mut coordinator = LifecycleCoordinator::new(context(), 0, geometry.clone());
        admitted(
            coordinator.admit(AdmissionRequest::Candidate(capacity_matched(
                candidate(
                    92,
                    LifecycleWorkClass::Apply,
                    LifecyclePhase::Apply,
                    InitialLifecycleState::Ready,
                    PredecessorScope::Independent,
                ),
                &geometry,
            ))),
        );
        let second = capacity_matched(
            candidate(
                93,
                LifecycleWorkClass::Store,
                LifecyclePhase::Store,
                InitialLifecycleState::Ready,
                PredecessorScope::Independent,
            ),
            &geometry,
        );
        assert_eq!(
            coordinator.admit(AdmissionRequest::Candidate(second.clone())),
            AdmissionDecision::WaitForCapacity(WaitToken::new(
                WaitSource::Capacity(CapacityClass::Effect),
                0,
            ))
        );
        assert_eq!(coordinator.admission_waits.len(), 1);
        assert_eq!(
            coordinator.admit(AdmissionRequest::Candidate(second.clone())),
            AdmissionDecision::WaitForCapacity(WaitToken::new(
                WaitSource::Capacity(CapacityClass::Effect),
                0,
            ))
        );
        let mut foreign = second.clone();
        foreign.causal_root = CausalRoot::new(digest(203));
        foreign.physical_geometry = PhysicalGeometry::new(
            [
                PhysicalSlot::new(PhysicalSlotId::new(0, 0), digest(1)),
                PhysicalSlot::new(PhysicalSlotId::new(0, 0), digest(2)),
            ],
            [],
        );
        assert_eq!(
            coordinator.admit(AdmissionRequest::Candidate(foreign)),
            AdmissionDecision::Rejected(AdmissionRejection::ForeignOwner)
        );
        assert_eq!(coordinator.high_water, 1);
        let lease = execute(plan_turn(&mut coordinator, []));
        coordinator.settle_turn(lease, TurnOutcome::Advanced);
        assert_eq!(
            admitted(coordinator.admit(AdmissionRequest::Candidate(second))).1,
            2
        );
        assert!(coordinator.admission_waits.is_empty());
    }
    #[test]
    fn capacity_fence_canonicalizes_carrier_order_and_retires_after_invalid_retry() {
        let geometry = capacities(2);
        let mut coordinator = LifecycleCoordinator::new(context(), 0, geometry.clone());
        admitted(
            coordinator.admit(AdmissionRequest::Candidate(capacity_matched(
                candidate(
                    92,
                    LifecycleWorkClass::Apply,
                    LifecyclePhase::Apply,
                    InitialLifecycleState::Ready,
                    PredecessorScope::Independent,
                ),
                &geometry,
            ))),
        );
        admitted(
            coordinator.admit(AdmissionRequest::Candidate(capacity_matched(
                candidate(
                    94,
                    LifecycleWorkClass::Apply,
                    LifecyclePhase::Apply,
                    InitialLifecycleState::Ready,
                    PredecessorScope::Independent,
                ),
                &geometry,
            ))),
        );
        let mut pending = capacity_matched(
            candidate(
                93,
                LifecycleWorkClass::Store,
                LifecyclePhase::Store,
                InitialLifecycleState::Ready,
                PredecessorScope::Independent,
            ),
            &geometry,
        );
        pending.physical_geometry = PhysicalGeometry::new(
            [
                PhysicalSlot::new(
                    PhysicalSlotId::for_capacity(CapacityClass::Effect, 1),
                    digest(201),
                ),
                PhysicalSlot::new(
                    PhysicalSlotId::for_capacity(CapacityClass::Effect, 0),
                    digest(200),
                ),
            ],
            [],
        );
        let wait = WaitToken::new(WaitSource::Capacity(CapacityClass::Effect), 0);
        assert_eq!(
            coordinator.admit(AdmissionRequest::Candidate(pending.clone())),
            AdmissionDecision::WaitForCapacity(wait)
        );
        pending.physical_geometry.initial.reverse();
        assert_eq!(
            coordinator.admit(AdmissionRequest::Candidate(pending.clone())),
            AdmissionDecision::WaitForCapacity(wait)
        );
        let lease = execute(plan_turn(&mut coordinator, []));
        coordinator.settle_turn(lease, TurnOutcome::Advanced);
        pending.stage.kind = LifecycleStageKind::CertifiedServe;
        assert_eq!(
            coordinator.admit(AdmissionRequest::Candidate(pending)),
            AdmissionDecision::Rejected(AdmissionRejection::InvalidDurableMetadata)
        );
        assert!(coordinator.admission_waits.is_empty());
    }
    #[test]
    fn capacity_fence_freezes_the_complete_serve_companion() {
        let geometry = CapacityGeometry::new([
            (CapacityClass::Consensus, 1),
            (CapacityClass::Effect, 1),
            (CapacityClass::Serve, 1),
            (CapacityClass::Producer, 0),
        ]);
        let mut coordinator = LifecycleCoordinator::new(context(), 0, geometry.clone());
        let serve = capacity_matched(
            serve_candidate(100, InitialLifecycleState::Ready),
            &geometry,
        );
        let wait = WaitToken::new(WaitSource::Capacity(CapacityClass::Producer), 0);
        assert_eq!(
            coordinator.admit(AdmissionRequest::Candidate(serve.clone())),
            AdmissionDecision::WaitForCapacity(wait)
        );
        assert_eq!(
            plan_turn(&mut coordinator, []),
            TurnPlan::Waiting(BTreeSet::from([wait]))
        );
        let mut drifted = serve.clone();
        drifted
            .producer_turn
            .as_mut()
            .expect("Serve companion")
            .key
            .subject = Some(digest(205));
        assert_eq!(
            coordinator.admit(AdmissionRequest::Candidate(drifted)),
            AdmissionDecision::Rejected(AdmissionRejection::InvalidDurableMetadata)
        );
        assert_eq!(coordinator.admission_waits[&serve.key].candidate, serve);
    }
    #[test]
    fn pending_serve_companion_key_is_exclusive_before_physical_refinement() {
        let geometry = CapacityGeometry::new([
            (CapacityClass::Consensus, 1),
            (CapacityClass::Effect, 1),
            (CapacityClass::Serve, 2),
            (CapacityClass::Producer, 0),
        ]);
        let mut coordinator = LifecycleCoordinator::new(context(), 0, geometry.clone());
        let first = capacity_matched(
            serve_candidate(100, InitialLifecycleState::Ready),
            &geometry,
        );
        let reserved_key = first.producer_turn.as_ref().expect("Serve companion").key;
        assert!(matches!(
            coordinator.admit(AdmissionRequest::Candidate(first.clone())),
            AdmissionDecision::WaitForCapacity(_)
        ));
        let mut foreign = capacity_matched(
            serve_candidate(102, InitialLifecycleState::Ready),
            &geometry,
        );
        foreign.producer_turn.as_mut().expect("Serve companion").key = reserved_key;
        assert_eq!(
            coordinator.admit(AdmissionRequest::Candidate(foreign.clone())),
            AdmissionDecision::Rejected(AdmissionRejection::InvalidDurableMetadata)
        );
        foreign.causal_root = first.causal_root;
        assert_eq!(
            coordinator.admit(AdmissionRequest::Candidate(foreign)),
            AdmissionDecision::Rejected(AdmissionRejection::InvalidDurableMetadata)
        );
        let mut colliding = candidate(
            104,
            LifecycleWorkClass::Fetch,
            LifecyclePhase::Fetch,
            InitialLifecycleState::Ready,
            PredecessorScope::Independent,
        );
        colliding.key = reserved_key;
        assert_eq!(
            coordinator.admit(AdmissionRequest::Candidate(colliding)),
            AdmissionDecision::Rejected(AdmissionRejection::InvalidDurableMetadata)
        );
        assert_eq!(coordinator.admission_waits.len(), 1);
    }
    #[test]
    fn record_level_capacity_wait_fails_closed_instead_of_self_lassoing() {
        let geometry = capacities(1);
        let mut coordinator = LifecycleCoordinator::new(context(), 0, geometry.clone());
        admitted(
            coordinator.admit(AdmissionRequest::Candidate(capacity_matched(
                candidate(
                    94,
                    LifecycleWorkClass::Fetch,
                    LifecyclePhase::Fetch,
                    InitialLifecycleState::Ready,
                    PredecessorScope::Independent,
                ),
                &geometry,
            ))),
        );
        let first = execute(plan_turn(&mut coordinator, []));
        coordinator.settle_turn(
            first.clone(),
            TurnOutcome::Blocked(WaitToken::new(
                WaitSource::Capacity(CapacityClass::Effect),
                0,
            )),
        );
        assert_eq!(coordinator.active_lease, Some(first));
        assert_eq!(coordinator.fault, Some(CoordinatorFault::InvalidReadyEvent));
    }
    #[test]
    fn capacity_arithmetic_overflow_waits_instead_of_wrapping() {
        let geometry = capacities(8);
        let mut coordinator = LifecycleCoordinator::new(context(), 0, geometry.clone());
        coordinator
            .capacity_used
            .insert(CapacityClass::Effect, usize::MAX);
        assert_eq!(
            coordinator.admit(AdmissionRequest::Candidate(capacity_matched(
                candidate(
                    103,
                    LifecycleWorkClass::Fetch,
                    LifecyclePhase::Fetch,
                    InitialLifecycleState::Ready,
                    PredecessorScope::Independent,
                ),
                &geometry,
            ))),
            AdmissionDecision::WaitForCapacity(WaitToken::new(
                WaitSource::Capacity(CapacityClass::Effect),
                0,
            ))
        );
    }
    #[test]
    fn pending_capacity_fences_have_a_deterministic_bound() {
        let geometry = capacities(0);
        let mut coordinator = LifecycleCoordinator::new(context(), 0, geometry.clone());
        for seed in 1..=u8::try_from(MAX_PENDING_ADMISSION_WAITS).expect("bound fits u8") {
            assert!(matches!(
                coordinator.admit(AdmissionRequest::Candidate(capacity_matched(
                    candidate(
                        seed,
                        LifecycleWorkClass::Fetch,
                        LifecyclePhase::Fetch,
                        InitialLifecycleState::Ready,
                        PredecessorScope::Independent,
                    ),
                    &geometry,
                ))),
                AdmissionDecision::WaitForCapacity(_)
            ));
        }
        assert_eq!(
            coordinator.admission_waits.len(),
            MAX_PENDING_ADMISSION_WAITS
        );
        assert_eq!(
            coordinator.admit(AdmissionRequest::Candidate(capacity_matched(
                candidate(
                    65,
                    LifecycleWorkClass::Fetch,
                    LifecyclePhase::Fetch,
                    InitialLifecycleState::Ready,
                    PredecessorScope::Independent,
                ),
                &geometry,
            ))),
            AdmissionDecision::Rejected(AdmissionRejection::AdmissionQueueFull)
        );
    }
    #[test]
    fn enter_view_accepts_only_monotonic_exact_views() {
        let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
        let wait = WaitToken::new(WaitSource::External(digest(204)), 0);
        let view_one = candidate(
            1,
            LifecycleWorkClass::EnterView,
            LifecyclePhase::EnterView,
            InitialLifecycleState::Waiting(wait),
            PredecessorScope::Independent,
        );
        let (view_one_owner, _, _) =
            admitted(coordinator.admit(AdmissionRequest::Candidate(view_one.clone())));
        let mut conflicting = view_one.clone();
        conflicting.key.subject = Some(digest(201));
        assert_eq!(
            coordinator.admit(AdmissionRequest::Candidate(conflicting)),
            AdmissionDecision::Rejected(AdmissionRejection::InvalidDurableMetadata)
        );
        let higher = candidate(
            2,
            LifecycleWorkClass::EnterView,
            LifecyclePhase::EnterView,
            InitialLifecycleState::Ready,
            PredecessorScope::Independent,
        );
        let (higher_owner, higher_ordinal, _) =
            admitted(coordinator.admit(AdmissionRequest::Candidate(higher.clone())));
        assert_eq!(
            coordinator.admit(AdmissionRequest::Candidate(higher)),
            AdmissionDecision::Retry {
                owner: higher_owner,
                ordinal: higher_ordinal,
                action: RetryAction::StutterInstalledView,
            }
        );
        let lease = execute(plan_turn(&mut coordinator, []));
        assert_eq!(lease.ordinal, higher_ordinal);
        coordinator.settle_turn(lease, TurnOutcome::Advanced);
        assert_eq!(
            coordinator.records[&1].state,
            LifecycleState::Terminal(TerminalOutcome::Cancelled)
        );
        coordinator.publish_ready(ReadyEvent::new(1, view_one_owner, wait, None));
        assert_eq!(
            plan_turn(&mut coordinator, [(WaitSource::External(digest(204)), 1,)]),
            TurnPlan::Idle
        );
        let stale = candidate(
            0,
            LifecycleWorkClass::EnterView,
            LifecyclePhase::EnterView,
            InitialLifecycleState::Ready,
            PredecessorScope::Independent,
        );
        assert_eq!(
            coordinator.admit(AdmissionRequest::Candidate(stale)),
            AdmissionDecision::Rejected(AdmissionRejection::EnterViewConflict)
        );
    }
    #[test]
    fn installed_view_retires_a_lower_capacity_fence() {
        let geometry = capacities(1);
        let mut coordinator = LifecycleCoordinator::new(context(), 0, geometry.clone());
        admitted(
            coordinator.admit(AdmissionRequest::Candidate(capacity_matched(
                candidate(
                    50,
                    LifecycleWorkClass::Broadcast,
                    LifecyclePhase::BroadcastProposal,
                    InitialLifecycleState::Ready,
                    PredecessorScope::Independent,
                ),
                &geometry,
            ))),
        );
        let lower = capacity_matched(
            candidate(
                1,
                LifecycleWorkClass::EnterView,
                LifecyclePhase::EnterView,
                InitialLifecycleState::Ready,
                PredecessorScope::Independent,
            ),
            &geometry,
        );
        let higher = capacity_matched(
            candidate(
                2,
                LifecycleWorkClass::EnterView,
                LifecyclePhase::EnterView,
                InitialLifecycleState::Ready,
                PredecessorScope::Independent,
            ),
            &geometry,
        );
        assert!(matches!(
            coordinator.admit(AdmissionRequest::Candidate(lower.clone())),
            AdmissionDecision::WaitForCapacity(_)
        ));
        assert!(matches!(
            coordinator.admit(AdmissionRequest::Candidate(higher.clone())),
            AdmissionDecision::WaitForCapacity(_)
        ));
        assert_eq!(
            coordinator
                .admission_waits
                .keys()
                .copied()
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([higher.key])
        );
        assert_eq!(
            coordinator.admit(AdmissionRequest::Candidate(lower.clone())),
            AdmissionDecision::Rejected(AdmissionRejection::EnterViewConflict)
        );
        let lease = execute(plan_turn(&mut coordinator, []));
        coordinator.settle_turn(lease, TurnOutcome::Advanced);
        admitted(coordinator.admit(AdmissionRequest::Candidate(higher)));
        let lease = execute(plan_turn(&mut coordinator, []));
        coordinator.settle_turn(lease, TurnOutcome::Advanced);
        assert!(coordinator.admission_waits.is_empty());
        assert_eq!(
            coordinator.admit(AdmissionRequest::Candidate(lower)),
            AdmissionDecision::Rejected(AdmissionRejection::EnterViewConflict)
        );
    }
    #[test]
    fn waiting_records_do_not_block_ready_work_but_ready_prefixes_precede_serve() {
        let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
        let wait = WaitToken::new(WaitSource::External(digest(99)), 0);
        admit_waiting_fetch(&mut coordinator, 3, wait, PredecessorScope::Independent);
        let (_, serve, producer) = admitted(coordinator.admit(AdmissionRequest::Candidate(
            serve_candidate(5, InitialLifecycleState::Ready),
        )));
        assert_eq!((serve, producer), (2, Some(3)));
        assert_eq!(
            coordinator.records[&serve].episode.frozen_predecessors,
            BTreeSet::from([1])
        );
        assert_eq!(
            coordinator.records[&3].episode.frozen_predecessors,
            BTreeSet::from([1, 2])
        );
        assert_eq!(execute(plan_turn(&mut coordinator, [])).ordinal, 2);
        let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
        admitted(coordinator.admit(AdmissionRequest::Candidate(candidate(
            3,
            LifecycleWorkClass::Fetch,
            LifecyclePhase::Fetch,
            InitialLifecycleState::Ready,
            PredecessorScope::Independent,
        ))));
        let (_, serve, producer) = admitted(coordinator.admit(AdmissionRequest::Candidate(
            serve_candidate(5, InitialLifecycleState::Ready),
        )));
        assert_eq!(
            coordinator.records[&serve].episode.frozen_predecessors,
            BTreeSet::from([1])
        );
        assert_eq!(
            coordinator.records[&producer.expect("producer")]
                .episode
                .frozen_predecessors,
            BTreeSet::from([1, 2])
        );
        assert_eq!(execute(plan_turn(&mut coordinator, [])).ordinal, 1);
    }
    #[test]
    fn a_frozen_waiting_predecessor_blocks_only_after_it_becomes_ready() {
        let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
        let source = WaitSource::External(digest(98));
        admit_waiting_fetch(
            &mut coordinator,
            3,
            WaitToken::new(source, 0),
            PredecessorScope::Independent,
        );
        admitted(
            coordinator.admit(AdmissionRequest::Candidate(serve_candidate(
                5,
                InitialLifecycleState::Ready,
            ))),
        );
        assert_eq!(
            execute(plan_turn(&mut coordinator, [(source, 1)])).ordinal,
            1
        );
    }
    #[test]
    fn dropped_or_stale_lease_fails_closed_without_reselection() {
        let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
        admitted(coordinator.admit(AdmissionRequest::Candidate(candidate(
            7,
            LifecycleWorkClass::Fetch,
            LifecyclePhase::Fetch,
            InitialLifecycleState::Ready,
            PredecessorScope::Independent,
        ))));
        let lease = execute(plan_turn(&mut coordinator, []));
        assert_eq!(
            plan_turn(&mut coordinator, []),
            TurnPlan::FailClosed(CoordinatorFault::UnsettledLease(lease.id))
        );
        let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
        admitted(coordinator.admit(AdmissionRequest::Candidate(candidate(
            8,
            LifecycleWorkClass::Fetch,
            LifecyclePhase::Fetch,
            InitialLifecycleState::Ready,
            PredecessorScope::Independent,
        ))));
        let lease = execute(plan_turn(&mut coordinator, []));
        let mut stale = lease.clone();
        stale.id = LeaseId(99);
        coordinator.settle_turn(stale, TurnOutcome::Advanced);
        assert_eq!(coordinator.active_lease, Some(lease));
        assert_eq!(
            plan_turn(&mut coordinator, []),
            TurnPlan::FailClosed(CoordinatorFault::StaleLease)
        );
    }
    #[test]
    fn blocked_generation_and_late_completion_publish_exactly_once() {
        let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
        let (owner, ordinal, _) =
            admitted(coordinator.admit(AdmissionRequest::Candidate(candidate(
                9,
                LifecycleWorkClass::Validate,
                LifecyclePhase::Validate,
                InitialLifecycleState::Ready,
                PredecessorScope::Independent,
            ))));
        let lease = execute(plan_turn(&mut coordinator, []));
        let wait = WaitToken::new(WaitSource::External(digest(99)), 0);
        coordinator.settle_turn(lease, TurnOutcome::Blocked(wait));
        assert_eq!(
            plan_turn(&mut coordinator, [(WaitSource::External(digest(99)), 0,)]),
            TurnPlan::Waiting(BTreeSet::from([wait]))
        );
        let replacement = PhysicalReplacement::new(
            PhysicalSlotId::for_capacity(CapacityClass::Effect, 0),
            PhysicalSlot::new(
                PhysicalSlotId::for_capacity(CapacityClass::Effect, 0),
                digest(100),
            ),
        );
        coordinator.publish_ready(ReadyEvent::new(ordinal, owner, wait, Some(replacement)));
        assert_eq!(
            coordinator.records[&ordinal].physical_slots
                [&PhysicalSlotId::for_capacity(CapacityClass::Effect, 0)],
            digest(100)
        );
        coordinator.publish_ready(ReadyEvent::new(
            ordinal,
            owner,
            wait,
            Some(PhysicalReplacement::new(
                PhysicalSlotId::for_capacity(CapacityClass::Effect, 0),
                PhysicalSlot::new(
                    PhysicalSlotId::for_capacity(CapacityClass::Effect, 0),
                    digest(101),
                ),
            )),
        ));
        assert_eq!(
            coordinator.records[&ordinal].physical_slots
                [&PhysicalSlotId::for_capacity(CapacityClass::Effect, 0)],
            digest(100)
        );
        assert_eq!(execute(plan_turn(&mut coordinator, [])).ordinal, ordinal);
    }
    #[test]
    fn reserved_fetch_uses_one_generation_to_reblock_and_the_next_to_publish_ready() {
        let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
        let source = WaitSource::External(digest(0xD1));
        let initial_wait = WaitToken::new(source, 4);
        let (owner, ordinal, _) = admit_waiting_fetch(
            &mut coordinator,
            0xD2,
            initial_wait,
            PredecessorScope::Independent,
        );
        let inputs = scheduler_inputs(&coordinator, [(source, 5)]);
        let lease = execute(coordinator.plan_turn(inputs));
        assert_eq!(lease.ordinal(), ordinal);
        let submitted_wait = WaitToken::new(source, 5);
        coordinator.settle_turn(lease, TurnOutcome::Blocked(submitted_wait));
        assert_eq!(coordinator.fault, None);
        assert_eq!(
            coordinator.records[&ordinal].state,
            LifecycleState::Waiting(submitted_wait)
        );
        assert_eq!(coordinator.observed_generation.get(&source), Some(&5));
        coordinator.publish_ready(ReadyEvent::new(ordinal, owner, submitted_wait, None));
        assert_eq!(coordinator.fault, None);
        assert_eq!(coordinator.records[&ordinal].state, LifecycleState::Ready);
        assert_eq!(coordinator.observed_generation.get(&source), Some(&6));
    }
    #[test]
    fn source_generation_advance_atomically_wakes_every_stale_waiter() {
        let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
        let source = WaitSource::External(digest(201));
        for seed in [90, 91] {
            admit_waiting_fetch(
                &mut coordinator,
                seed,
                WaitToken::new(source, 0),
                PredecessorScope::Independent,
            );
        }
        assert!(coordinator.ready_index.is_empty());
        let selected = execute(plan_turn(&mut coordinator, [(source, 1)]));
        assert_eq!(coordinator.ready_index.len(), 1);
        assert!(
            coordinator
                .records
                .values()
                .all(|record| !matches!(record.state, LifecycleState::Waiting(_)))
        );
        assert_eq!(coordinator.active_lease, Some(selected));
    }
    #[test]
    fn blocking_at_a_new_generation_atomically_wakes_older_waiters() {
        let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
        let source = WaitSource::External(digest(202));
        admit_waiting_fetch(
            &mut coordinator,
            92,
            WaitToken::new(source, 0),
            PredecessorScope::Independent,
        );
        admitted(coordinator.admit(AdmissionRequest::Candidate(candidate(
            93,
            LifecycleWorkClass::Store,
            LifecyclePhase::Store,
            InitialLifecycleState::Ready,
            PredecessorScope::Independent,
        ))));
        let lease = execute(plan_turn(&mut coordinator, []));
        assert_eq!(lease.ordinal, 2);
        coordinator.settle_turn(lease, TurnOutcome::Blocked(WaitToken::new(source, 1)));
        assert_eq!(coordinator.records[&1].state, LifecycleState::Ready);
        assert_eq!(
            coordinator.records[&2].state,
            LifecycleState::Waiting(WaitToken::new(source, 1))
        );
    }
    #[test]
    fn unchanged_external_generation_cannot_reawaken_a_blocked_turn() {
        let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
        admitted(coordinator.admit(AdmissionRequest::Candidate(candidate(
            96,
            LifecycleWorkClass::Fetch,
            LifecyclePhase::Fetch,
            InitialLifecycleState::Ready,
            PredecessorScope::Independent,
        ))));
        let source = WaitSource::External(digest(202));
        let lease = execute(plan_turn(&mut coordinator, []));
        coordinator.settle_turn(lease, TurnOutcome::Blocked(WaitToken::new(source, 0)));
        let lease = execute(plan_turn(&mut coordinator, [(source, 1)]));
        coordinator.settle_turn(
            lease.clone(),
            TurnOutcome::Blocked(WaitToken::new(source, 0)),
        );
        assert_eq!(coordinator.active_lease, Some(lease));
        assert_eq!(
            plan_turn(&mut coordinator, [(source, 1)]),
            TurnPlan::FailClosed(CoordinatorFault::InvalidReadyEvent)
        );
    }
    #[test]
    fn non_progressing_max_generation_waits_fail_closed() {
        let source = WaitSource::External(digest(212));
        let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
        assert_eq!(
            coordinator.admit(AdmissionRequest::Candidate(candidate(
                106,
                LifecycleWorkClass::Store,
                LifecyclePhase::Store,
                InitialLifecycleState::Waiting(WaitToken::new(source, u64::MAX)),
                PredecessorScope::Independent,
            ))),
            AdmissionDecision::Rejected(AdmissionRejection::InvalidInitialState)
        );
        admitted(coordinator.admit(AdmissionRequest::Candidate(candidate(
            107,
            LifecycleWorkClass::Fetch,
            LifecyclePhase::Fetch,
            InitialLifecycleState::Ready,
            PredecessorScope::Independent,
        ))));
        let lease = execute(plan_turn(&mut coordinator, []));
        coordinator.settle_turn(
            lease.clone(),
            TurnOutcome::Blocked(WaitToken::new(source, u64::MAX)),
        );
        assert_eq!(coordinator.active_lease, Some(lease));
        assert_eq!(coordinator.fault, Some(CoordinatorFault::InvalidReadyEvent));
        let mut publication = LifecycleCoordinator::new(context(), 0, capacities(8));
        let (owner, ordinal, _) = admit_waiting_fetch(
            &mut publication,
            108,
            WaitToken::new(source, u64::MAX - 1),
            PredecessorScope::Independent,
        );
        publication.records.get_mut(&ordinal).expect("record").state =
            LifecycleState::Waiting(WaitToken::new(source, u64::MAX));
        publication.publish_ready(ReadyEvent::new(
            ordinal,
            owner,
            WaitToken::new(source, u64::MAX),
            None,
        ));
        assert_eq!(publication.fault, Some(CoordinatorFault::InvalidReadyEvent));
    }
    #[test]
    fn finite_replenishment_coalesces_duplicates_and_cannot_repeat() {
        let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
        admitted(coordinator.admit(AdmissionRequest::Candidate(candidate(
            10,
            LifecycleWorkClass::Fetch,
            LifecyclePhase::Fetch,
            InitialLifecycleState::Ready,
            PredecessorScope::Independent,
        ))));
        let lease = execute(plan_turn(&mut coordinator, []));
        coordinator.settle_turn(
            lease,
            TurnOutcome::Replenished(PhysicalSlot::new(
                PhysicalSlotId::for_capacity(CapacityClass::Effect, 1),
                digest(10),
            )),
        );
        assert_eq!(coordinator.records[&1].physical_slots.len(), 1);
        let lease = execute(plan_turn(&mut coordinator, []));
        coordinator.settle_turn(
            lease.clone(),
            TurnOutcome::Replenished(PhysicalSlot::new(
                PhysicalSlotId::for_capacity(CapacityClass::Effect, 1),
                digest(11),
            )),
        );
        assert_eq!(coordinator.active_lease, Some(lease));
        assert_eq!(
            plan_turn(&mut coordinator, []),
            TurnPlan::FailClosed(CoordinatorFault::InvalidPhysicalTransition)
        );
    }
    #[test]
    fn duplicate_initial_carriers_share_a_digest_but_consume_each_finite_slot() {
        let mut request = candidate(
            97,
            LifecycleWorkClass::Fetch,
            LifecyclePhase::Fetch,
            InitialLifecycleState::Ready,
            PredecessorScope::Independent,
        );
        request.physical_geometry = PhysicalGeometry::new(
            [
                PhysicalSlot::new(
                    PhysicalSlotId::for_capacity(CapacityClass::Effect, 0),
                    digest(97),
                ),
                PhysicalSlot::new(
                    PhysicalSlotId::for_capacity(CapacityClass::Effect, 1),
                    digest(97),
                ),
            ],
            [],
        );
        let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
        admitted(coordinator.admit(AdmissionRequest::Candidate(request)));
        assert_eq!(coordinator.records[&1].physical_slots.len(), 1);
        assert_eq!(
            coordinator.records[&1].episode.consumed_slots,
            BTreeSet::from([
                PhysicalSlotId::for_capacity(CapacityClass::Effect, 0),
                PhysicalSlotId::for_capacity(CapacityClass::Effect, 1),
            ])
        );
        assert_coordinator_invariants(&coordinator);
    }
    #[test]
    fn duplicate_carrier_coalescing_is_independent_of_input_order() {
        let mut forward = candidate(
            98,
            LifecycleWorkClass::Fetch,
            LifecyclePhase::Fetch,
            InitialLifecycleState::Ready,
            PredecessorScope::Independent,
        );
        forward.physical_geometry = PhysicalGeometry::new(
            [
                PhysicalSlot::new(
                    PhysicalSlotId::for_capacity(CapacityClass::Effect, 1),
                    digest(98),
                ),
                PhysicalSlot::new(
                    PhysicalSlotId::for_capacity(CapacityClass::Effect, 0),
                    digest(98),
                ),
            ],
            [],
        );
        let mut reverse = forward.clone();
        reverse.physical_geometry.initial.reverse();
        let mut left = LifecycleCoordinator::new(context(), 0, capacities(8));
        let mut right = LifecycleCoordinator::new(context(), 0, capacities(8));
        admitted(left.admit(AdmissionRequest::Candidate(forward)));
        admitted(right.admit(AdmissionRequest::Candidate(reverse)));
        assert_eq!(left.records[&1], right.records[&1]);
        assert_eq!(
            left.records[&1].physical_slots,
            BTreeMap::from([(
                PhysicalSlotId::for_capacity(CapacityClass::Effect, 0),
                digest(98),
            )])
        );
    }
    #[test]
    fn episode_universe_is_minted_only_from_the_sealed_height_authority() {
        let admitted_candidate = candidate(
            101,
            LifecycleWorkClass::Fetch,
            LifecyclePhase::Fetch,
            InitialLifecycleState::Ready,
            PredecessorScope::Independent,
        );
        let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
        let expected_universe = coordinator
            .episode_authority
            .universe_for(admitted_candidate.key)
            .expect("sealed height authority covers the admitted key");
        admitted(coordinator.admit(AdmissionRequest::Candidate(admitted_candidate)));
        assert_eq!(coordinator.records[&1].episode.universe, expected_universe);
        let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
        let mut invalid_slot = candidate(
            102,
            LifecycleWorkClass::Fetch,
            LifecyclePhase::Fetch,
            InitialLifecycleState::Ready,
            PredecessorScope::Independent,
        );
        invalid_slot.physical_geometry = PhysicalGeometry::new(
            [PhysicalSlot::new(PhysicalSlotId::new(9, 0), digest(102))],
            [],
        );
        assert_eq!(
            coordinator.admit(AdmissionRequest::Candidate(invalid_slot)),
            AdmissionDecision::Rejected(AdmissionRejection::InvalidEpisodeUniverse)
        );
        let mut wrong_slot_class = candidate(
            104,
            LifecycleWorkClass::Fetch,
            LifecyclePhase::Fetch,
            InitialLifecycleState::Ready,
            PredecessorScope::Independent,
        );
        wrong_slot_class.physical_geometry = PhysicalGeometry::new(
            [PhysicalSlot::new(
                PhysicalSlotId::for_capacity(CapacityClass::Consensus, 0),
                digest(104),
            )],
            [],
        );
        assert_eq!(
            coordinator.admit(AdmissionRequest::Candidate(wrong_slot_class)),
            AdmissionDecision::Rejected(AdmissionRejection::InvalidEpisodeUniverse)
        );
        assert!(coordinator.records.is_empty());
        let first = candidate(
            105,
            LifecycleWorkClass::Fetch,
            LifecyclePhase::Fetch,
            InitialLifecycleState::Ready,
            PredecessorScope::Independent,
        );
        admitted(coordinator.admit(AdmissionRequest::Candidate(first)));
        let minted = &coordinator.records[&1].episode.universe;
        assert_eq!(minted.leader, digest(3));
        assert_eq!(
            minted.authenticated_roster_slots,
            BTreeSet::from([0, 1, 2, 3])
        );
        assert_eq!(minted.capacity_geometry, capacities(8).limits);
        assert!(super::authority::test_authority(context(), [], 0, capacities(8)).is_none());
        assert!(
            super::authority::test_authority(context(), [digest(2), digest(2)], 0, capacities(8))
                .is_none()
        );
        assert!(
            super::authority::test_authority(context(), [digest(2)], 1, capacities(8)).is_none()
        );
        assert!(
            super::authority::test_authority(
                context(),
                [digest(2)],
                0,
                capacities(usize::from(u16::MAX) + 2)
            )
            .is_none()
        );
        let successor_key = key(106, LifecyclePhase::Store);
        let expected = coordinator
            .episode_authority
            .universe_for(successor_key)
            .expect("verified key has one authority-derived universe");
        assert_eq!(expected.leader, digest(4));
        assert_eq!(expected.context, context().id);
        assert_eq!(expected.phase, LifecyclePhase::Store);
        assert_eq!(expected.subject, successor_key.subject());
        assert_eq!(expected.target, successor_key.scheduler_target());
        assert_eq!(
            expected.capacity_geometry,
            coordinator.capacity_geometry.limits
        );
        let subjectless = LifecycleKey::new(
            context().id,
            LifecycleRound::new(context().height, 107),
            None,
            None,
            LifecyclePhase::EnterView,
            None,
        );
        let subjectless_universe = coordinator
            .episode_authority
            .universe_for(subjectless)
            .expect("subjectless work has a context-derived target");
        assert_eq!(subjectless_universe.target, context().id);
    }
    #[test]
    fn serve_retirement_activates_reserved_producer_before_later_admission() {
        let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
        let (_, serve, producer) = admitted(coordinator.admit(AdmissionRequest::Candidate(
            serve_candidate(11, InitialLifecycleState::Ready),
        )));
        assert_eq!((serve, producer), (1, Some(2)));
        let lease = execute(plan_turn(&mut coordinator, []));
        assert_eq!(lease.ordinal, 1);
        settle_with_test_serve_receipt(&mut coordinator, lease, completed_serve(231));
        admitted(coordinator.admit(AdmissionRequest::Candidate(candidate(
            20,
            LifecycleWorkClass::Fetch,
            LifecyclePhase::Fetch,
            InitialLifecycleState::Ready,
            PredecessorScope::Independent,
        ))));
        let producer_lease = execute(plan_turn(&mut coordinator, []));
        assert_eq!(producer_lease.ordinal, 2);
        coordinator.settle_turn(
            producer_lease,
            TurnOutcome::Terminal(TerminalOutcome::Completed(None)),
        );
        assert_eq!(execute(plan_turn(&mut coordinator, [])).ordinal, 3);
    }
    #[test]
    fn generic_serve_terminal_settlement_requires_a_post_fsync_receipt() {
        let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
        admitted(
            coordinator.admit(AdmissionRequest::Candidate(serve_candidate(
                10,
                InitialLifecycleState::Ready,
            ))),
        );
        let lease = execute(plan_turn(&mut coordinator, []));
        coordinator.settle_turn(lease.clone(), completed_serve(230));
        assert_eq!(
            coordinator.fault,
            Some(CoordinatorFault::InvalidTerminalOutcome)
        );
        assert_eq!(coordinator.active_lease, Some(lease));
        assert!(matches!(
            coordinator.durable_records[&1].payload,
            DurablePayloadReference::CertifiedServePending { .. }
        ));
    }
    #[test]
    fn serve_and_producer_share_one_reconstruction_source() {
        let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
        let mut serve = serve_candidate(15, InitialLifecycleState::Ready);
        serve
            .producer_turn
            .as_mut()
            .expect("Serve owns its producer companion")
            .reconstruction_source = digest(250);
        assert_eq!(
            coordinator.admit(AdmissionRequest::Candidate(serve)),
            AdmissionDecision::Rejected(AdmissionRejection::InvalidDurableMetadata)
        );
        assert!(coordinator.records.is_empty());
        assert!(coordinator.durable_records.is_empty());
        let mut drifted_key = serve_candidate(16, InitialLifecycleState::Ready);
        drifted_key
            .producer_turn
            .as_mut()
            .expect("Serve owns its producer companion")
            .key
            .subject = Some(digest(250));
        assert_eq!(
            coordinator.admit(AdmissionRequest::Candidate(drifted_key)),
            AdmissionDecision::Rejected(AdmissionRejection::InvalidDurableMetadata)
        );
        assert!(coordinator.records.is_empty());
        let mut foreign_root = serve_candidate(17, InitialLifecycleState::Ready);
        foreign_root.causal_root = CausalRoot::new(digest(251));
        assert_eq!(
            coordinator.admit(AdmissionRequest::Candidate(foreign_root)),
            AdmissionDecision::Rejected(AdmissionRejection::InvalidDurableMetadata)
        );
    }
    #[test]
    fn durable_ledger_is_projected_from_the_coordinator_record_bijection() {
        let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
        admitted(
            coordinator.admit(AdmissionRequest::Candidate(serve_candidate(
                14,
                InitialLifecycleState::Ready,
            ))),
        );
        let live = ledger::LifecycleLedgerV1::from_coordinator(&coordinator)
            .expect("admission has one valid durable projection");
        assert_eq!(live.high_water(), 2);
        assert_eq!(live.records().len(), 2);
        assert_eq!(live.producer_debts().len(), 1);
        let lease = execute(plan_turn(&mut coordinator, []));
        settle_with_test_serve_receipt(&mut coordinator, lease, completed_serve(235));
        let terminal = ledger::LifecycleLedgerV1::from_coordinator(&coordinator)
            .expect("terminal Serve and live producer remain one valid ledger pair");
        assert!(matches!(
            terminal.records()[0].durable_payload(),
            Some(DurablePayloadReference::CertifiedServeCompleted {
                response,
                ..
            }) if response == digest(235)
        ));
        assert_eq!(terminal.producer_debts().len(), 1);
    }
    #[test]
    fn durable_admission_and_terminal_settlement_publish_one_atomic_ledger() {
        let root = tempfile::tempdir().expect("temporary ledger directory");
        let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
        coordinator
            .attach_empty_test_ledger(root.path())
            .expect("attach empty durable ledger");
        admitted(
            coordinator.admit(AdmissionRequest::Candidate(serve_candidate(
                16,
                InitialLifecycleState::Ready,
            ))),
        );
        let (_, admitted_ledger) = ledger::LifecycleLedgerStoreV1::open(root.path(), context())
            .expect("reload admitted ledger");
        assert_eq!(admitted_ledger.high_water(), 2);
        assert_eq!(admitted_ledger.records().len(), 2);
        assert_eq!(admitted_ledger.producer_debts().len(), 1);
        let universes = coordinator
            .records
            .iter()
            .map(|(ordinal, record)| (*ordinal, record.episode.slot_universe.clone()))
            .collect();
        let snapshot = admitted_ledger
            .recovery_snapshot(universes)
            .expect("authenticated storage covers every durable record");
        let mut restarted = LifecycleCoordinator::new(context(), 2, capacities(8));
        restarted.reconcile_restart(snapshot);
        assert_eq!(restarted.fault, None);
        assert_eq!(restarted.high_water, 2);
        assert_eq!(restarted.records.len(), 2);
        let lease = execute(plan_turn(&mut coordinator, []));
        settle_with_test_serve_receipt(&mut coordinator, lease, completed_serve(236));
        let (_, settled_ledger) = ledger::LifecycleLedgerStoreV1::open(root.path(), context())
            .expect("reload settled ledger");
        assert_eq!(
            settled_ledger.records()[0].terminal(),
            Some(Some(TerminalOutcome::Completed(Some(digest(236)))))
        );
        assert_eq!(settled_ledger.producer_debts().len(), 1);
    }
    #[test]
    fn failed_durable_admission_exposes_no_owner_or_ordinal() {
        let root = tempfile::tempdir().expect("temporary ledger directory");
        let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
        coordinator
            .attach_empty_test_ledger(root.path())
            .expect("attach empty durable ledger");
        coordinator.redirect_test_ledger_to_missing_parent(root.path());
        assert_eq!(
            coordinator.admit(AdmissionRequest::Candidate(candidate(
                17,
                LifecycleWorkClass::Fetch,
                LifecyclePhase::Fetch,
                InitialLifecycleState::Ready,
                PredecessorScope::Independent,
            ))),
            AdmissionDecision::FailClosed(CoordinatorFault::DurabilityFailure)
        );
        assert_eq!(coordinator.high_water, 0);
        assert!(coordinator.records.is_empty());
        assert!(coordinator.durable_records.is_empty());
        assert_eq!(coordinator.fault, Some(CoordinatorFault::DurabilityFailure));
        let (_, persisted) = ledger::LifecycleLedgerStoreV1::open(root.path(), context())
            .expect("original ledger remains readable");
        assert_eq!(persisted.high_water(), 0);
        assert!(persisted.records().is_empty());
    }
    #[test]
    fn failed_terminal_persistence_keeps_the_active_lease_visible() {
        let root = tempfile::tempdir().expect("temporary ledger directory");
        let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
        coordinator
            .attach_empty_test_ledger(root.path())
            .expect("attach empty durable ledger");
        admitted(coordinator.admit(AdmissionRequest::Candidate(candidate(
            18,
            LifecycleWorkClass::Broadcast,
            LifecyclePhase::BroadcastProposal,
            InitialLifecycleState::Ready,
            PredecessorScope::Independent,
        ))));
        let lease = execute(plan_turn(&mut coordinator, []));
        coordinator.redirect_test_ledger_to_missing_parent(root.path());
        coordinator.settle_turn(lease.clone(), TurnOutcome::Advanced);
        assert_eq!(coordinator.fault, Some(CoordinatorFault::DurabilityFailure));
        assert_eq!(coordinator.active_lease, Some(lease.clone()));
        assert_eq!(
            coordinator.records[&lease.ordinal].state,
            LifecycleState::Claimed(lease.id)
        );
        let (_, persisted) = ledger::LifecycleLedgerStoreV1::open(root.path(), context())
            .expect("pre-terminal ledger remains readable");
        assert_eq!(persisted.records()[0].terminal(), Some(None));
    }
    #[test]
    fn durable_body_advanced_without_its_successor_fails_closed() {
        let root = tempfile::tempdir().expect("temporary ledger directory");
        let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
        coordinator
            .attach_empty_test_ledger(root.path())
            .expect("attach empty durable ledger");
        admitted(coordinator.admit(AdmissionRequest::Candidate(candidate(
            19,
            LifecycleWorkClass::Fetch,
            LifecyclePhase::Fetch,
            InitialLifecycleState::Ready,
            PredecessorScope::Independent,
        ))));
        let lease = execute(plan_turn(&mut coordinator, []));
        coordinator.settle_turn(lease.clone(), TurnOutcome::Advanced);
        assert_eq!(
            coordinator.fault,
            Some(CoordinatorFault::InvalidTerminalOutcome)
        );
        assert_eq!(coordinator.active_lease, Some(lease.clone()));
        assert_eq!(
            coordinator.records[&lease.ordinal].state,
            LifecycleState::Claimed(lease.id)
        );
        assert_eq!(
            coordinator.durable_records[&lease.ordinal].continuation,
            DurableContinuation::None
        );
        let (_, persisted) = ledger::LifecycleLedgerStoreV1::open(root.path(), context())
            .expect("pre-transition body ledger remains readable");
        assert_eq!(persisted.records()[0].terminal(), Some(None));
        assert_eq!(
            persisted.records()[0].continuation(),
            Some(DurableContinuation::None)
        );
    }
    #[test]
    fn body_advanced_requires_a_typed_composite_even_without_a_ledger_store() {
        for (seed, work_class, phase) in [
            (20, LifecycleWorkClass::Fetch, LifecyclePhase::Fetch),
            (21, LifecycleWorkClass::Store, LifecyclePhase::Store),
            (22, LifecycleWorkClass::Validate, LifecyclePhase::Validate),
            (
                23,
                LifecycleWorkClass::SignProposal,
                LifecyclePhase::Proposal,
            ),
            (24, LifecycleWorkClass::SignVote, LifecyclePhase::Prepare),
            (25, LifecycleWorkClass::SignTimeout, LifecyclePhase::Timeout),
        ] {
            for outcome in [
                TurnOutcome::Advanced,
                TurnOutcome::Terminal(TerminalOutcome::Advanced),
            ] {
                let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
                admitted(coordinator.admit(AdmissionRequest::Candidate(candidate(
                    seed,
                    work_class,
                    phase,
                    InitialLifecycleState::Ready,
                    PredecessorScope::Independent,
                ))));
                let lease = execute(plan_turn(&mut coordinator, []));
                coordinator.settle_turn(lease.clone(), outcome);
                assert_eq!(
                    coordinator.fault,
                    Some(CoordinatorFault::InvalidTerminalOutcome)
                );
                assert_eq!(coordinator.active_lease, Some(lease.clone()));
                assert_eq!(
                    coordinator.records[&lease.ordinal].state,
                    LifecycleState::Claimed(lease.id)
                );
                assert_eq!(
                    coordinator.durable_records[&lease.ordinal].continuation,
                    DurableContinuation::None
                );
            }
        }
    }
    #[test]
    fn serve_and_producer_terminalization_fail_closed_without_the_atomic_debt() {
        let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
        admitted(
            coordinator.admit(AdmissionRequest::Candidate(serve_candidate(
                12,
                InitialLifecycleState::Ready,
            ))),
        );
        let lease = execute(plan_turn(&mut coordinator, []));
        let replay = test_serve_terminal_replay(
            &coordinator,
            &lease,
            TerminalOutcome::Completed(Some(digest(232))),
        )
        .expect("exact pending Serve pair mints one terminal replay receipt");
        coordinator.producer_debts.clear();
        coordinator.settle_turn_with_durable_serve_terminal(lease.clone(), replay);
        assert_eq!(coordinator.active_lease, Some(lease));
        assert_eq!(
            coordinator.fault,
            Some(CoordinatorFault::CapacityAccounting)
        );
        let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
        admitted(
            coordinator.admit(AdmissionRequest::Candidate(serve_candidate(
                13,
                InitialLifecycleState::Ready,
            ))),
        );
        let serve = execute(plan_turn(&mut coordinator, []));
        settle_with_test_serve_receipt(&mut coordinator, serve, completed_serve(233));
        let producer = execute(plan_turn(&mut coordinator, []));
        coordinator.producer_debts.clear();
        coordinator.settle_turn(producer.clone(), TurnOutcome::Advanced);
        assert_eq!(coordinator.active_lease, Some(producer));
        assert_eq!(
            coordinator.fault,
            Some(CoordinatorFault::CapacityAccounting)
        );
    }
    #[test]
    fn producer_handoff_blocks_later_work_without_making_serve_a_global_barrier() {
        let mut coordinator = LifecycleCoordinator::new(context(), 0, capacities(8));
        admitted(
            coordinator.admit(AdmissionRequest::Candidate(serve_candidate(
                98,
                InitialLifecycleState::Ready,
            ))),
        );
        let later = candidate(
            99,
            LifecycleWorkClass::Broadcast,
            LifecyclePhase::BroadcastProposal,
            InitialLifecycleState::Ready,
            PredecessorScope::Independent,
        );
        admitted(coordinator.admit(AdmissionRequest::Candidate(later)));
        let unrelated = execute(plan_turn_with_modes(&mut coordinator, [(1, 100), (3, 0)]));
        assert_eq!(unrelated.ordinal, 3);
        coordinator.settle_turn(unrelated, TurnOutcome::Advanced);
        let serve = execute(plan_turn(&mut coordinator, []));
        assert_eq!(serve.ordinal, 1);
        settle_with_test_serve_receipt(&mut coordinator, serve, completed_serve(234));
        admitted(coordinator.admit(AdmissionRequest::Candidate(candidate(
            100,
            LifecycleWorkClass::Broadcast,
            LifecyclePhase::BroadcastProposal,
            InitialLifecycleState::Ready,
            PredecessorScope::Independent,
        ))));
        assert_eq!(
            execute(plan_turn_with_modes(&mut coordinator, [(2, 100), (4, 0)])).ordinal,
            2
        );
    }
    #[test]
    fn terminal_tombstone_replays_without_resurrection_or_capacity_charge() {
        let geometry = capacities(1);
        let mut coordinator = LifecycleCoordinator::new(context(), 0, geometry.clone());
        let request = capacity_matched(
            candidate(
                21,
                LifecycleWorkClass::Apply,
                LifecyclePhase::Apply,
                InitialLifecycleState::Ready,
                PredecessorScope::Independent,
            ),
            &geometry,
        );
        let (owner, _, _) =
            admitted(coordinator.admit(AdmissionRequest::Candidate(request.clone())));
        let lease = execute(plan_turn(&mut coordinator, []));
        let result = digest(200);
        coordinator.settle_turn(
            lease,
            TurnOutcome::Terminal(TerminalOutcome::Completed(Some(result))),
        );
        assert_eq!(
            coordinator.admit(AdmissionRequest::Candidate(request)),
            AdmissionDecision::ReplayTerminal {
                owner,
                outcome: TerminalOutcome::Completed(Some(result)),
            }
        );
        assert_eq!(coordinator.records.len(), 1);
    }
    const EXPLORER_TEMPLATES: [(LifecycleWorkClass, LifecyclePhase, LifecycleStageKind); 21] = [
        (
            LifecycleWorkClass::SignProposal,
            LifecyclePhase::Proposal,
            LifecycleStageKind::SignProposal,
        ),
        (
            LifecycleWorkClass::SignVote,
            LifecyclePhase::Prepare,
            LifecycleStageKind::SignPrepareVote,
        ),
        (
            LifecycleWorkClass::SignVote,
            LifecyclePhase::Commit,
            LifecycleStageKind::SignCommitVote,
        ),
        (
            LifecycleWorkClass::SignTimeout,
            LifecyclePhase::Timeout,
            LifecycleStageKind::SignTimeoutVote,
        ),
        (
            LifecycleWorkClass::Fetch,
            LifecyclePhase::Fetch,
            LifecycleStageKind::FetchBody,
        ),
        (
            LifecycleWorkClass::Store,
            LifecyclePhase::Store,
            LifecycleStageKind::StoreBody,
        ),
        (
            LifecycleWorkClass::Validate,
            LifecyclePhase::Validate,
            LifecycleStageKind::ValidateBody,
        ),
        (
            LifecycleWorkClass::Apply,
            LifecyclePhase::Apply,
            LifecycleStageKind::ApplyDecision,
        ),
        (
            LifecycleWorkClass::Broadcast,
            LifecyclePhase::BroadcastProposal,
            LifecycleStageKind::BroadcastProposal,
        ),
        (
            LifecycleWorkClass::Broadcast,
            LifecyclePhase::BroadcastPrepareVote,
            LifecycleStageKind::BroadcastPrepareVote,
        ),
        (
            LifecycleWorkClass::Broadcast,
            LifecyclePhase::BroadcastCommitVote,
            LifecycleStageKind::BroadcastCommitVote,
        ),
        (
            LifecycleWorkClass::Broadcast,
            LifecyclePhase::BroadcastPrepareQc,
            LifecycleStageKind::BroadcastPrepareQc,
        ),
        (
            LifecycleWorkClass::Broadcast,
            LifecyclePhase::BroadcastCommitQc,
            LifecycleStageKind::BroadcastCommitQc,
        ),
        (
            LifecycleWorkClass::Broadcast,
            LifecyclePhase::BroadcastTimeoutVote,
            LifecycleStageKind::BroadcastTimeoutVote,
        ),
        (
            LifecycleWorkClass::Broadcast,
            LifecyclePhase::BroadcastTc,
            LifecycleStageKind::BroadcastTc,
        ),
        (
            LifecycleWorkClass::EnterView,
            LifecyclePhase::EnterView,
            LifecycleStageKind::EnterView,
        ),
        (
            LifecycleWorkClass::EquivocationReport,
            LifecyclePhase::DiagnosticProposalEquivocation,
            LifecycleStageKind::ReportProposalEquivocation,
        ),
        (
            LifecycleWorkClass::EquivocationReport,
            LifecyclePhase::DiagnosticVoteEquivocation,
            LifecycleStageKind::ReportVoteEquivocation,
        ),
        (
            LifecycleWorkClass::EquivocationReport,
            LifecyclePhase::DiagnosticTimeoutEquivocation,
            LifecycleStageKind::ReportTimeoutEquivocation,
        ),
        (
            LifecycleWorkClass::InvalidBodyReport,
            LifecyclePhase::DiagnosticInvalidBody,
            LifecycleStageKind::ReportInvalidBody,
        ),
        (
            LifecycleWorkClass::CertifiedServe,
            LifecyclePhase::Serve,
            LifecycleStageKind::CertifiedServe,
        ),
    ];
    fn explorer_candidate(
        seed: u8,
        (work_class, phase, kind): (LifecycleWorkClass, LifecyclePhase, LifecycleStageKind),
    ) -> CandidateAdmission {
        if work_class == LifecycleWorkClass::CertifiedServe {
            return serve_candidate(seed, InitialLifecycleState::Ready);
        }
        let mut candidate = candidate(
            seed,
            work_class,
            phase,
            InitialLifecycleState::Ready,
            PredecessorScope::Independent,
        );
        candidate.stage.kind = kind;
        candidate
    }
    fn assert_coordinator_invariants(coordinator: &LifecycleCoordinator) {
        assert_eq!(
            coordinator.episode_authority.context(),
            coordinator.active_context
        );
        assert_eq!(
            coordinator.episode_authority.capacity_geometry(),
            &coordinator.capacity_geometry
        );
        assert_eq!(coordinator.records.len(), coordinator.key_index.len());
        assert_eq!(
            coordinator.records.keys().copied().collect::<BTreeSet<_>>(),
            coordinator
                .durable_records
                .keys()
                .copied()
                .collect::<BTreeSet<_>>()
        );
        for owner in coordinator.owner_index.values() {
            assert_eq!(
                coordinator
                    .records
                    .get(&owner.first_admission_ordinal)
                    .map(|record| record.owner),
                Some(*owner)
            );
        }
        for (ordinal, record) in &coordinator.records {
            let durable = coordinator
                .durable_records
                .get(ordinal)
                .expect("every logical record has one durable projection");
            let terminal = match record.state {
                LifecycleState::Terminal(outcome) => Some(outcome),
                LifecycleState::Waiting(_) | LifecycleState::Ready | LifecycleState::Claimed(_) => {
                    None
                }
            };
            assert!(
                durable
                    .payload
                    .matches_terminal(record.work_class, terminal)
            );
            assert!(
                record.work_class != LifecycleWorkClass::CertifiedServe
                    || record.key.subject.is_some()
            );
            assert_eq!(*ordinal, record.ordinal);
            assert_eq!(coordinator.key_index.get(&record.key), Some(ordinal));
            assert_eq!(
                coordinator
                    .episode_authority
                    .universe_for(record.key)
                    .as_ref(),
                Some(&record.episode.universe)
            );
            assert!(coordinator.episode_authority.admits_slots(
                record.work_class.capacity_class(),
                &record.episode.slot_universe,
            ));
            assert!(record.owner.first_admission_ordinal <= *ordinal);
            assert_eq!(
                coordinator.owner_index.get(&record.owner.causal_root),
                Some(&record.owner)
            );
            assert!(
                record
                    .physical_slots
                    .keys()
                    .all(|slot| record.episode.slot_universe.contains(slot))
            );
            assert!(
                record
                    .episode
                    .consumed_slots
                    .is_subset(&record.episode.slot_universe)
            );
            assert!(
                record
                    .episode
                    .frozen_predecessors
                    .iter()
                    .all(|predecessor| {
                        *predecessor < record.ordinal
                            && coordinator.records.contains_key(predecessor)
                    })
            );
            if matches!(
                record.stage.predecessor_scope,
                PredecessorScope::Independent
            ) {
                assert!(record.episode.frozen_predecessors.is_empty());
            }
            let unique_digests: BTreeSet<_> = record.physical_slots.values().collect();
            assert_eq!(unique_digests.len(), record.physical_slots.len());
            if let LifecycleState::Waiting(wait) = record.state {
                match wait.source {
                    WaitSource::Capacity(_) => {
                        panic!("admitted records never wait on reserved capacity")
                    }
                    WaitSource::External(_) | WaitSource::Recovery(_) => {
                        assert!(
                            coordinator
                                .observed_generation
                                .get(&wait.source)
                                .is_none_or(|known| *known <= wait.observed_generation)
                        );
                    }
                    WaitSource::ProducerTurn(serve) => {
                        assert_eq!(coordinator.producer_debts.get(&serve), Some(ordinal));
                    }
                }
            }
        }
        let expected_ready: BTreeSet<_> = coordinator
            .records
            .iter()
            .filter_map(|(ordinal, record)| {
                (record.state == LifecycleState::Ready).then_some(*ordinal)
            })
            .collect();
        assert_eq!(coordinator.ready_index, expected_ready);
        for (key, waiting) in &coordinator.admission_waits {
            assert!(!coordinator.key_index.contains_key(key));
            assert!(
                waiting.serve_payload_receipt.is_none()
                    || waiting.candidate.work_class == LifecycleWorkClass::CertifiedServe
            );
            assert!(
                coordinator
                    .episode_authority
                    .universe_for(waiting.candidate.key)
                    .is_some()
            );
            let WaitSource::Capacity(class) = waiting.wait_token.source else {
                panic!("admission wait must name capacity")
            };
            assert!(
                waiting.wait_token.observed_generation <= coordinator.capacity_generation[&class]
            );
        }
        let claimed: Vec<_> = coordinator
            .records
            .values()
            .filter_map(|record| match record.state {
                LifecycleState::Claimed(lease) => Some((record.ordinal, lease)),
                LifecycleState::Waiting(_)
                | LifecycleState::Ready
                | LifecycleState::Terminal(_) => None,
            })
            .collect();
        match coordinator.active_lease.as_ref() {
            Some(lease) => assert_eq!(claimed, vec![(lease.ordinal, lease.id)]),
            None => assert!(claimed.is_empty()),
        }
        for class in CapacityClass::ALL {
            let expected = coordinator
                .records
                .values()
                .filter(|record| {
                    record.work_class.capacity_class() == class
                        && !matches!(record.state, LifecycleState::Terminal(_))
                })
                .count();
            assert_eq!(coordinator.capacity_used[&class], expected);
            assert!(expected <= coordinator.capacity_geometry.limits[&class]);
        }
        assert!(
            coordinator
                .records
                .keys()
                .next_back()
                .is_none_or(|ordinal| *ordinal <= coordinator.high_water)
        );
        for record in coordinator.records.values() {
            match record.work_class {
                LifecycleWorkClass::CertifiedServe => {
                    let producer = record.ordinal.checked_add(1).expect("Serve pair ordinal");
                    let pair = coordinator.records.get(&producer).expect("Serve pair");
                    assert_eq!(pair.owner, record.owner);
                    assert_eq!(pair.work_class, LifecycleWorkClass::ProducerTurn);
                    assert!(serve_and_producer_keys_match(record.key, pair.key));
                    assert_eq!(
                        coordinator.durable_records[&record.ordinal].reconstruction_source,
                        coordinator.durable_records[&pair.ordinal].reconstruction_source
                    );
                    assert_eq!(pair.stage.kind, LifecycleStageKind::ProducerTurn);
                    let pair_is_live = !matches!(pair.state, LifecycleState::Terminal(_));
                    let has_debt =
                        coordinator.producer_debts.get(&record.ordinal) == Some(&producer);
                    assert_eq!(has_debt, pair_is_live);
                    if !matches!(record.state, LifecycleState::Terminal(_)) {
                        assert!(has_debt);
                    }
                    if record.state == LifecycleState::Terminal(TerminalOutcome::Cancelled) {
                        assert_eq!(
                            pair.state,
                            LifecycleState::Terminal(TerminalOutcome::Cancelled)
                        );
                    }
                }
                LifecycleWorkClass::ProducerTurn => {
                    let serve = record
                        .ordinal
                        .checked_sub(1)
                        .expect("producer pair ordinal");
                    let pair = coordinator.records.get(&serve).expect("producer pair");
                    assert_eq!(pair.owner, record.owner);
                    assert_eq!(pair.work_class, LifecycleWorkClass::CertifiedServe);
                    assert!(serve_and_producer_keys_match(pair.key, record.key));
                    assert_eq!(
                        coordinator.durable_records[&record.ordinal].reconstruction_source,
                        coordinator.durable_records[&pair.ordinal].reconstruction_source
                    );
                    assert_eq!(pair.stage.kind, LifecycleStageKind::CertifiedServe);
                    let has_debt = coordinator.producer_debts.get(&serve) == Some(&record.ordinal);
                    assert_eq!(
                        has_debt,
                        !matches!(record.state, LifecycleState::Terminal(_))
                    );
                }
                _ => {}
            }
        }
        for (serve, producer) in &coordinator.producer_debts {
            let serve_record = &coordinator.records[serve];
            let producer_record = &coordinator.records[producer];
            assert_eq!(serve.checked_add(1), Some(*producer));
            assert_eq!(serve_record.work_class, LifecycleWorkClass::CertifiedServe);
            assert_eq!(serve_record.stage.kind, LifecycleStageKind::CertifiedServe);
            assert_eq!(producer_record.work_class, LifecycleWorkClass::ProducerTurn);
            assert!(serve_and_producer_keys_match(
                serve_record.key,
                producer_record.key
            ));
            assert_eq!(producer_record.stage.kind, LifecycleStageKind::ProducerTurn);
            assert_eq!(serve_record.owner, producer_record.owner);
            assert!(!matches!(
                producer_record.state,
                LifecycleState::Terminal(_)
            ));
        }
        for producer in coordinator.records.values().filter(|record| {
            record.work_class == LifecycleWorkClass::ProducerTurn
                && record.state == LifecycleState::Ready
        }) {
            assert!(coordinator.ready_index.iter().all(|entry| {
                *entry <= producer.ordinal
                    || !coordinator.ready_entry_is_eligible(*entry, &coordinator.ready_index)
            }));
        }
    }
    fn assert_terminal_irreversibility(
        before: &LifecycleCoordinator,
        after: &LifecycleCoordinator,
    ) {
        if before.active_context != after.active_context {
            assert!(after.high_water >= before.high_water);
            return;
        }
        for (ordinal, record) in &before.records {
            if let LifecycleState::Terminal(outcome) = record.state {
                assert_eq!(
                    after.records.get(ordinal).map(|record| record.state),
                    Some(LifecycleState::Terminal(outcome)),
                    "terminal state disappeared across explorer transition:\nBEFORE={before:#?}\nAFTER={after:#?}"
                );
            }
        }
    }
    fn recovery_snapshot(coordinator: &LifecycleCoordinator) -> RecoverySnapshot {
        RecoverySnapshot {
            context: coordinator.active_context,
            high_water: coordinator.high_water,
            records: coordinator
                .records
                .values()
                .map(|record| RecoveredLifecycleRecord {
                    key: record.key,
                    owner: record.owner,
                    ordinal: record.ordinal,
                    work_class: record.work_class,
                    stage: record.stage,
                    terminal: match record.state {
                        LifecycleState::Terminal(outcome) => Some(outcome),
                        LifecycleState::Waiting(_)
                        | LifecycleState::Ready
                        | LifecycleState::Claimed(_) => None,
                    },
                    reconstruction_source: coordinator.durable_records[&record.ordinal]
                        .reconstruction_source,
                    payload: coordinator.durable_records[&record.ordinal].payload,
                    replay_authority: coordinator.durable_records[&record.ordinal]
                        .replay_authority
                        .clone(),
                    continuation: coordinator.durable_records[&record.ordinal].continuation,
                    physical_slot_universe: record.episode.slot_universe.clone(),
                })
                .collect(),
            producer_debts: coordinator.producer_debts.clone(),
        }
    }
    fn recovered_pair_record(
        seed: u8,
        owner: OwnerId,
        ordinal: u128,
        work_class: LifecycleWorkClass,
        terminal: Option<TerminalOutcome>,
    ) -> RecoveredLifecycleRecord {
        let kind = match work_class {
            LifecycleWorkClass::CertifiedServe => LifecycleStageKind::CertifiedServe,
            LifecycleWorkClass::ProducerTurn => LifecycleStageKind::ProducerTurn,
            _ => panic!("pair fixture requires Serve or ProducerTurn"),
        };
        let replay = super::replay_authority::exact_record_fixture(context(), kind, seed);
        let candidate_stage = stage(
            kind,
            1,
            if work_class == LifecycleWorkClass::ProducerTurn {
                PredecessorScope::ProducerHandoffBarrier
            } else {
                PredecessorScope::ReadyOrdinalPrefix
            },
        );
        let payload = match (work_class, terminal, replay.payload) {
            (LifecycleWorkClass::CertifiedServe, None, pending) => pending,
            (
                LifecycleWorkClass::CertifiedServe,
                Some(TerminalOutcome::Completed(Some(response))),
                DurablePayloadReference::CertifiedServePending {
                    request,
                    certificate,
                },
            ) => DurablePayloadReference::CertifiedServeCompleted {
                request,
                certificate,
                response,
            },
            (
                LifecycleWorkClass::CertifiedServe,
                Some(outcome),
                DurablePayloadReference::CertifiedServePending {
                    request,
                    certificate,
                },
            ) => DurablePayloadReference::CertifiedServeNegative {
                request,
                certificate,
                outcome: DurableServeNegativeOutcome::from_terminal(outcome)
                    .unwrap_or(DurableServeNegativeOutcome::Cancelled),
            },
            _ => DurablePayloadReference::None,
        };
        let replay_authority = if work_class == LifecycleWorkClass::CertifiedServe {
            replay
                .authority
                .terminalized_certified_serve(context(), replay.key, candidate_stage, payload)
                .expect("canonical Certified-Serve fixture terminalizes exactly")
        } else {
            replay.authority
        };
        RecoveredLifecycleRecord {
            key: replay.key,
            owner,
            ordinal,
            work_class,
            stage: candidate_stage,
            terminal,
            reconstruction_source: digest(200),
            payload,
            replay_authority,
            continuation: DurableContinuation::None,
            physical_slot_universe: BTreeSet::new(),
        }
    }
    include!("tests/v2_lifecycle_coordinator_explorer_cases.rs");
}
