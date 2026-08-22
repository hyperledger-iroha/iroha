//! Atomic admission boundary between digest-only lifecycle state and concrete work.
#[cfg(test)]
use super::work_registry::{ConcreteLifecycleWork, RegistryPublicationError};
use super::{
    AdmissionDecision, AdmissionRequest, CandidateAdmission, CoordinatorFault,
    LifecycleCoordinator, LifecycleDigest, LifecyclePhase, LifecycleStageKind, LifecycleState,
    LifecycleWorkClass, PredecessorScope, ProductionLifecycleOwnerV1, TurnLease, TurnOutcome,
    WaitSource, WaitToken,
    body_pipeline_transition::durable_validate_payload_is_exact,
    projection::AdapterEffectAdmissionError,
    schema::AttestedReadyValidateDemand,
    work_registry::{
        AuthenticatedRecoveredWalValidateLifecycleRepair, BoundAdapterRegistryPublicationErrorV1,
        ConcreteLifecycleWorkRegistry, ConcreteWorkAddress, DurableValidateCompletionAuthority,
        DurableValidateCompletionPublication, DurableValidateCompletionPublicationError,
        DurableValidateDispatch, DurableValidateExecutionError,
        DurableValidateRegistryPublicationErrorV1, ExecutedDurableValidateDispatch,
        LifecycleOutputRegistryJoinV1, LiveWalRegistryPublicationErrorV1,
        OpenedRecoveredWalValidateLedger, PendingDurableValidateAdmissionV1,
        PendingLifecycleOutputAdmissionV1, PendingLiveWalSignAdmissionV1,
        PreparedLifecycleAdmissionErrorV1, PreparedLifecycleAdmissionOwnerV1,
        PreparedLifecycleAdmissionV1, PreparedLifecycleOutputExecutionV1,
        PreparedLifecycleOutputRegistryRetirementV1, PreparedRecoveredDecisionApplyDispatch,
        PublishedDurableValidateCompletion, ReadyRecoveredDecisionApplyAttestation,
        ReadyRecoveredDecisionApplyAttestationError, ReadyValidateCarrierError,
        RecoveredDecisionApplyDispatchProjectionError, RecoveredWalParentFactoryError,
        RegistryError, reconstruct_recovered_wal_validate_parent,
    },
};
use crate::sumeragi::{
    v2::{AdapterEffect, RecoveredWalVoteSign, VerifiedHeightContext},
    v2_body_store::V2BodyStore,
    v2_runtime::PendingRuntimeEffectBinding,
};
/// Opaque process-local holder for concrete lifecycle work.
///
/// It deliberately exposes only empty construction. Logical scheduling and
/// every registry mutation remain sealed behind [`LifecycleCoordinator`].
pub(crate) struct LifecycleWorkRegistryHolder {
    registry: ConcreteLifecycleWorkRegistry,
}
impl LifecycleWorkRegistryHolder {
    /// Construct an empty holder for the production lifecycle service.
    pub(crate) fn empty() -> Self {
        Self {
            registry: ConcreteLifecycleWorkRegistry::default(),
        }
    }
    /// Borrow the exact concrete census for a coordinator-owned scheduler cut.
    pub(super) const fn registry(&self) -> &ConcreteLifecycleWorkRegistry {
        &self.registry
    }
    /// Borrow the concrete map only for one coordinator-owned composite transaction.
    pub(super) fn registry_mut(&mut self) -> &mut ConcreteLifecycleWorkRegistry {
        &mut self.registry
    }
    /// Join one runtime output to its exact next lifecycle row.
    fn join_lifecycle_output(
        &self,
        coordinator: &LifecycleCoordinator,
        execution: &PreparedLifecycleOutputExecutionV1,
    ) -> Result<LifecycleOutputRegistryJoinV1, RegistryError> {
        self.registry.join_lifecycle_output(coordinator, execution)
    }
    /// Recheck one staged output terminal before its LedgerV1 fsync.
    fn lifecycle_output_terminal_is_exact(
        &self,
        current: &LifecycleCoordinator,
        staged: &LifecycleCoordinator,
        prepared: PreparedLifecycleOutputRegistryRetirementV1,
        execution: &PreparedLifecycleOutputExecutionV1,
    ) -> bool {
        self.registry
            .lifecycle_output_terminal_is_exact(current, staged, prepared, execution)
    }
    /// Recheck a volatile carrier installed at its exact terminal ledger address.
    fn lifecycle_output_terminal_installed_is_exact(
        &self,
        coordinator: &LifecycleCoordinator,
        prepared: PreparedLifecycleOutputRegistryRetirementV1,
        execution: &PreparedLifecycleOutputExecutionV1,
    ) -> bool {
        self.registry
            .lifecycle_output_terminal_installed_is_exact(coordinator, prepared, execution)
    }
    /// Remove one preflighted output carrier after its terminal row is durable.
    fn publish_lifecycle_output_terminal_after_fsync(
        &mut self,
        prepared: PreparedLifecycleOutputRegistryRetirementV1,
    ) {
        self.registry
            .publish_lifecycle_output_terminal_after_fsync(prepared);
    }
    /// Bind the exact Ready Apply row to its sealed recovered-Decision carrier.
    ///
    /// The holder exposes neither its registry nor the carrier. The returned
    /// attestation contains only the registry-authenticated service demand.
    pub(super) fn attest_ready_recovered_decision_apply(
        &self,
        coordinator: &LifecycleCoordinator,
        ordinal: u128,
    ) -> Result<ReadyRecoveredDecisionApplyAttestation, ReadyRecoveredDecisionApplyAttestationError>
    {
        self.registry
            .attest_ready_recovered_decision_apply(coordinator, ordinal)
    }
    /// Project the exact claimed recovered Decision Apply into its dedicated worker task.
    ///
    /// The holder keeps the concrete registry private and returns only the
    /// move-only task emitted by the registry's fixed carrier projection.
    pub(super) fn prepare_recovered_decision_apply_dispatch(
        &mut self,
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
    ) -> Result<
        PreparedRecoveredDecisionApplyDispatch<'_>,
        RecoveredDecisionApplyDispatchProjectionError,
    > {
        self.registry
            .prepare_recovered_decision_apply_dispatch(coordinator, lease)
    }
    /// Return whether one recovered Broadcast declares a paired next Vote.
    pub(super) fn recovered_lifecycle_signed_broadcast_declares_next_vote(
        &self,
        coordinator: &LifecycleCoordinator,
        broadcast_ordinal: u128,
    ) -> bool {
        self.registry
            .recovered_lifecycle_signed_broadcast_declares_next_vote(coordinator, broadcast_ordinal)
    }
    /// Return the paired next-Vote ordinal retained by one Ready Broadcast.
    pub(super) fn recovered_lifecycle_signed_broadcast_paired_next_vote_ordinal(
        &self,
        coordinator: &LifecycleCoordinator,
        broadcast_ordinal: u128,
    ) -> Option<u128> {
        self.registry
            .recovered_lifecycle_signed_broadcast_paired_next_vote_ordinal(
                coordinator,
                broadcast_ordinal,
            )
    }
    /// Attest one recovered signed Broadcast as a durable Ready refanout source.
    pub(super) fn attest_ready_recovered_lifecycle_signed_broadcast(
        &self,
        coordinator: &LifecycleCoordinator,
        ordinal: u128,
    ) -> Result<(), &'static str> {
        self.registry
            .attest_ready_recovered_lifecycle_signed_broadcast(coordinator, ordinal)
    }
    /// Attest one adjacent signed-Broadcast and next-WAL-Vote pair.
    pub(super) fn attest_ready_recovered_lifecycle_signed_broadcast_and_next_vote(
        &self,
        coordinator: &LifecycleCoordinator,
        broadcast_ordinal: u128,
        next_sign_ordinal: u128,
    ) -> Result<
        super::work_registry::ReadyRecoveredLifecycleSignAttestationV1,
        super::work_registry::ReadyRecoveredLifecycleSignAttestationErrorV1,
    > {
        self.registry
            .attest_ready_recovered_lifecycle_signed_broadcast_and_next_vote(
                coordinator,
                broadcast_ordinal,
                next_sign_ordinal,
            )
    }
    /// Project one claimed recovered Broadcast into its refanout authority.
    pub(super) fn project_claimed_recovered_lifecycle_signed_broadcast_output(
        &self,
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
    ) -> Option<super::wal_recovery::RecoveredLifecycleSignedBroadcastOutputAuthorityV1> {
        self.registry
            .project_claimed_recovered_lifecycle_signed_broadcast_output(coordinator, lease)
    }
    /// Attest one Ready recovered Sign without exposing its concrete carrier.
    pub(super) fn attest_ready_recovered_lifecycle_sign(
        &self,
        coordinator: &LifecycleCoordinator,
        ordinal: u128,
    ) -> Result<
        super::work_registry::ReadyRecoveredLifecycleSignAttestationV1,
        super::work_registry::ReadyRecoveredLifecycleSignAttestationErrorV1,
    > {
        self.registry
            .attest_ready_recovered_lifecycle_sign(coordinator, ordinal)
    }
    /// Project one claimed recovered Sign into its dedicated worker task.
    pub(super) fn prepare_recovered_lifecycle_sign_dispatch(
        &mut self,
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
    ) -> Result<
        super::work_registry::PreparedRecoveredLifecycleSignDispatch<'_>,
        super::work_registry::RecoveredLifecycleSignDispatchProjectionErrorV1,
    > {
        self.registry
            .prepare_recovered_lifecycle_sign_dispatch(coordinator, lease)
    }
    /// Attest one exact Ready recovered Decision Fetch.
    pub(super) fn attest_ready_recovered_decision_fetch(
        &self,
        coordinator: &LifecycleCoordinator,
        ordinal: u128,
    ) -> Result<
        super::work_registry::ReadyRecoveredDecisionFetchAttestationV1,
        super::work_registry::ReadyRecoveredDecisionFetchAttestationErrorV1,
    > {
        self.registry
            .attest_ready_recovered_decision_fetch(coordinator, ordinal)
    }
    /// Bind one guarded worker completion to the exact claimed Apply carrier.
    pub(super) fn prepare_recovered_decision_apply_terminal_transition(
        &self,
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
        completion: &crate::sumeragi::v2_apply::RecoveredDecisionApplyCompletionV1,
    ) -> Option<(
        super::work_registry::PreparedRecoveredDecisionApplyTerminalTransitionV1,
        crate::sumeragi::v2::RecoveredDecisionApplyAdapterCompletionAuthorityV1,
    )> {
        self.registry
            .prepare_recovered_decision_apply_terminal_transition(coordinator, lease, completion)
    }
    /// Publish one recovered Apply terminal and remove its carrier after fsync.
    pub(super) fn publish_recovered_decision_apply_terminal_transition<T, E>(
        &mut self,
        prepared: super::work_registry::PreparedRecoveredDecisionApplyTerminalTransitionV1,
        current: &LifecycleCoordinator,
        staged: &LifecycleCoordinator,
        lease: &TurnLease,
        publish: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, super::work_registry::RecoveredDecisionApplyTerminalPublicationError<E>> {
        self.registry
            .publish_recovered_decision_apply_terminal_transition(
                prepared, current, staged, lease, publish,
            )
    }
    /// Reconstruct one storage-authenticated recovered Validate parent without a scheduler lease.
    ///
    /// The concrete registry never leaves this holder. Success returns only
    /// the opaque opened ledger and authenticated repair, while every failure
    /// retains or restores all WAL and body-marker authority internally.
    #[cfg_attr(not(test), allow(dead_code))]
    #[allow(clippy::result_large_err)]
    pub(crate) fn reconstruct_recovered_wal_validate_parent<'registry, 'body>(
        &'registry mut self,
        verified: &VerifiedHeightContext,
        body_store: &'body mut V2BodyStore,
        ledger_root: &std::path::Path,
        recovered: RecoveredWalVoteSign,
    ) -> Result<
        (
            OpenedRecoveredWalValidateLedger,
            AuthenticatedRecoveredWalValidateLifecycleRepair<'registry>,
        ),
        RecoveredWalParentFactoryError<'body>,
    > {
        reconstruct_recovered_wal_validate_parent(
            &mut self.registry,
            verified,
            body_store,
            ledger_root,
            recovered,
        )
    }
    /// Wrap a concrete registry for focused atomic-boundary tests.
    #[cfg(test)]
    pub(super) fn from_registry_for_test(registry: ConcreteLifecycleWorkRegistry) -> Self {
        Self { registry }
    }
    /// Borrow the concrete registry for drop- and failure-invariance checks.
    #[cfg(test)]
    pub(super) const fn registry_for_test(&self) -> &ConcreteLifecycleWorkRegistry {
        &self.registry
    }
    /// Mutably borrow the registry for focused corruption and unwind tests.
    #[cfg(test)]
    pub(super) fn registry_for_test_mut(&mut self) -> &mut ConcreteLifecycleWorkRegistry {
        &mut self.registry
    }
}
/// Closed precommit failure from the exact claimed-Validate dispatch cut.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DurableValidateDispatchError {
    /// The coordinator had already latched a fail-closed condition.
    CoordinatorFaulted,
    /// The supplied lease is not the coordinator's exact claimed Validate row.
    StaleLease,
    /// The closed concrete Validate carrier failed exact execution preflight.
    Registry(DurableValidateExecutionError),
    /// The sealed registry source did not produce one external wait.
    InvalidWaitSource,
    /// No later wake generation can be represented for this exact source.
    WaitGenerationExhausted,
    /// Another waiting lifecycle row already uses the supposedly unique source.
    AliasedWaitSource,
    /// Pure blocked settlement changed more than the exact lease and source row.
    InvalidStagedTransition,
}
/// Closed failure while binding a Ready Validate carrier into scheduler input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ReadyValidateDemandAttestationError {
    /// The coordinator row or one of its reverse indexes is not exact and Ready.
    InvalidCoordinatorIndex,
    /// The process-local carrier no longer matches the coordinator address/digest.
    Registry(ReadyValidateCarrierError),
}
/// Closed reason an effect/pending pair could not cross the atomic boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AdapterEffectAdmissionFailure {
    /// Exact admitted-address registration rejected the pair.
    Registry(RegistryError),
    /// The lifecycle ledger could not publish after registry installation.
    Durability,
}
/// Ownership-preserving result of one concrete adapter-effect admission.
#[derive(Debug)]
#[must_use = "the caller must retain or execute the returned concrete work"]
pub(super) enum AdapterEffectAdmissionTransaction {
    /// The first logical admission, concrete installation, and optional ledger
    /// publication all committed.
    Admitted(AdmissionDecision),
    /// An exact restart carrier was installed at its existing logical address
    /// and the recovered record's Ready transition was durably published.
    Rebound(AdmissionDecision),
    /// No new concrete entry was installed. The caller retains the exact pair.
    Returned {
        /// Logical decision governing retry, terminal replay, wait, or drop.
        decision: AdmissionDecision,
        /// Complete replay-authorized owner supplied by the caller.
        prepared: PreparedLifecycleAdmissionV1,
    },
    /// Projection, registration, or publication failed before a new logical
    /// record and concrete entry could commit together.
    Failed {
        /// Closed failure classification.
        failure: AdapterEffectAdmissionFailure,
        /// Complete replay-authorized owner returned to the caller.
        prepared: PreparedLifecycleAdmissionV1,
    },
}
/// Closed retry/fail-stop class for owner-facing durable Validate admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::sumeragi) enum ProductionDurableValidateAdmissionFailureV1 {
    /// Origin-specific projection did not match the owner's active verified context.
    Projection(AdapterEffectAdmissionError),
    /// Concrete registration rejected the exact carrier before publication.
    Registry,
    /// LedgerV1 publication was attempted and the owner is now fail-closed.
    Durability,
}
/// Owner-preserving result of one live durable Validate admission settlement.
///
/// Success consumes the origin-specific pending owner. Every non-committing
/// outcome returns that same move-only owner; neither a generic effect,
/// pending binding, candidate, nor caller-selected ordinal crosses this seam.
#[allow(variant_size_differences)]
#[derive(Debug)]
#[must_use = "durable Validate admission settlement must be observed"]
pub(in crate::sumeragi) enum ProductionDurableValidateAdmissionSettlementV1 {
    /// A new logical ordinal and its exact concrete carrier committed atomically.
    Admitted(AdmissionDecision),
    /// The exact recovered logical ordinal regained its concrete carrier.
    Rebound(AdmissionDecision),
    /// Logical reduction did not install work and returned the exact owner.
    Returned {
        /// Deterministic logical decision governing retry, wait, replay, or rejection.
        decision: AdmissionDecision,
        /// Complete origin-specific durable Validate owner retained for settlement.
        pending: PendingDurableValidateAdmissionV1,
    },
    /// Preparation, registration, or publication failed with ownership intact.
    Failed {
        /// Closed retry/fail-stop classification.
        failure: ProductionDurableValidateAdmissionFailureV1,
        /// Complete origin-specific durable Validate owner retained by the caller.
        pending: PendingDurableValidateAdmissionV1,
    },
}
/// Closed retry/fail-stop class for live-WAL Sign admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::sumeragi) enum ProductionLiveWalSignAdmissionFailureV1 {
    /// The exact WAL/local companion did not match the active verified context.
    Projection(AdapterEffectAdmissionError),
    /// Concrete registration rejected the exact carrier before publication.
    Registry,
    /// LedgerV1 publication was attempted and the owner is now fail-closed.
    Durability,
}

/// Owner-preserving result of one live-WAL Sign admission settlement.
#[derive(Debug)]
#[must_use = "live WAL Sign admission settlement must be observed"]
pub(in crate::sumeragi) enum ProductionLiveWalSignAdmissionSettlementV1 {
    /// A new logical ordinal and its exact Sign carrier committed atomically.
    Admitted(AdmissionDecision),
    /// The exact durable logical ordinal regained its live concrete carrier.
    Rebound(AdmissionDecision),
    /// Logical reduction returned the exact pending owner without publication.
    Returned {
        /// Deterministic logical decision governing retry or capacity.
        decision: AdmissionDecision,
        /// Complete live-WAL owner retained for settlement.
        pending: PendingLiveWalSignAdmissionV1,
    },
    /// Preparation, registration, or publication failed with ownership intact.
    Failed {
        /// Closed failure classification.
        failure: ProductionLiveWalSignAdmissionFailureV1,
        /// Complete live-WAL owner retained by the caller.
        pending: PendingLiveWalSignAdmissionV1,
    },
}
/// Closed failure from lifecycle-owned output admission, service I/O, or terminal fsync.
#[derive(Debug)]
pub(in crate::sumeragi) enum ProductionLifecycleOutputAdmissionFailureV1<E> {
    /// A genuinely direct output did not project in the active verified context.
    Projection(AdapterEffectAdmissionError),
    /// The exact concrete row or its staged terminal successor was invalid.
    Registry(RegistryError),
    /// The output service rejected or failed the exact effect.
    Service(E),
    /// LedgerV1 publication was attempted and the lifecycle owner is fail-closed.
    Durability,
}
/// Closed result of one lifecycle-owned output service attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::sumeragi) enum LifecycleOutputServiceDispositionV1 {
    /// The output service durably accepted the complete exact occurrence.
    Accepted,
    /// The bounded service retained no occurrence, so the exact source must retry.
    SourceRetained,
}
/// Ownership-preserving result of settling one runtime lifecycle output.
#[derive(Debug)]
#[must_use = "lifecycle output settlement must be observed"]
pub(in crate::sumeragi) enum ProductionLifecycleOutputAdmissionSettlementV1<E> {
    /// Service I/O completed and the same lifecycle row is durably terminal.
    Completed,
    /// A terminal duplicate or typed recovered-Broadcast retransmit was
    /// consumed without repeating generic service I/O.
    AlreadyCompleted,
    /// The exact owner remains parked behind ordering, capacity, or service backpressure.
    Deferred(PendingLifecycleOutputAdmissionV1),
    /// Settlement failed with the move-only output owner retained intact.
    Failed {
        /// Closed failure classification.
        failure: ProductionLifecycleOutputAdmissionFailureV1<E>,
        /// Exact output retained for restart diagnosis or a safe pre-I/O retry.
        pending: PendingLifecycleOutputAdmissionV1,
    },
}

fn uses_pre_release_taira_direct_output_compatibility(
    network_id: &iroha_data_model::NetworkId,
    height: u64,
) -> bool {
    crate::sumeragi::v2_context::uses_pre_release_taira_nexus_projection(network_id) && height == 5
}

fn settle_pre_release_direct_output<E>(
    prepared: PreparedLifecycleAdmissionV1,
    execution: PreparedLifecycleOutputExecutionV1,
    execute: impl FnOnce(
        &AdapterEffect,
        &crate::sumeragi::v2_runtime::RuntimeEffectOwnership,
    ) -> Result<LifecycleOutputServiceDispositionV1, E>,
) -> ProductionLifecycleOutputAdmissionSettlementV1<E> {
    match execution.execute_with(execute) {
        Ok(LifecycleOutputServiceDispositionV1::Accepted) => {
            ProductionLifecycleOutputAdmissionSettlementV1::Completed
        }
        Ok(LifecycleOutputServiceDispositionV1::SourceRetained) => {
            ProductionLifecycleOutputAdmissionSettlementV1::Deferred(
                PendingLifecycleOutputAdmissionV1::reclaim_returned(prepared, execution),
            )
        }
        Err(error) => ProductionLifecycleOutputAdmissionSettlementV1::Failed {
            failure: ProductionLifecycleOutputAdmissionFailureV1::Service(error),
            pending: PendingLifecycleOutputAdmissionV1::reclaim_returned(prepared, execution),
        },
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AdmittedWorkLocation {
    address: ConcreteWorkAddress,
    digest: LifecycleDigest,
}
impl LifecycleCoordinator {
    /// Convert one exact claimed durable Validate lease into an external wait
    /// and return its sole move-only validation dispatch.
    ///
    /// Every fallible check runs before the closed registry request is
    /// detached. Failure returns the exact caller-held lease while leaving the
    /// coordinator and registry unchanged. Success consumes that lease after
    /// a pure coordinator copy has settled it to `Waiting`; the registry row
    /// remains installed and unchanged throughout body-store execution.
    ///
    /// Executable completion is consumed by the volatile same-address
    /// carrier-plus-Ready transaction below. A missing merge sidecar instead
    /// enters the sealed durable registration and same-row wake transaction.
    #[cfg_attr(not(test), allow(dead_code))]
    #[allow(clippy::result_large_err)]
    pub(super) fn begin_durable_validate_dispatch(
        &mut self,
        registry: &mut LifecycleWorkRegistryHolder,
        lease: TurnLease,
        verified: &VerifiedHeightContext,
    ) -> Result<DurableValidateDispatch, (DurableValidateDispatchError, TurnLease)> {
        if self.fault.is_some() {
            return Err((DurableValidateDispatchError::CoordinatorFaulted, lease));
        }
        if !claimed_durable_validate_record_is_exact(self, &lease) {
            return Err((DurableValidateDispatchError::StaleLease, lease));
        }
        let Some((&slot, _)) = lease.physical_slots().first_key_value() else {
            return Err((
                DurableValidateDispatchError::Registry(
                    DurableValidateExecutionError::InvalidLeaseShape,
                ),
                lease,
            ));
        };
        let prepared = match registry
            .registry
            .prepare_durable_validate_execution(&lease, slot, verified)
        {
            Ok(prepared) => prepared,
            Err(error) => {
                return Err((DurableValidateDispatchError::Registry(error), lease));
            }
        };
        if !self
            .durable_records
            .get(&lease.ordinal())
            .is_some_and(|metadata| prepared.matches_durable_payload(metadata.payload))
        {
            drop(prepared);
            return Err((
                DurableValidateDispatchError::Registry(
                    DurableValidateExecutionError::InvalidValidateShape,
                ),
                lease,
            ));
        }
        let source = prepared.durable_validation_wait_source();
        if !matches!(source, WaitSource::External(_)) {
            drop(prepared);
            return Err((DurableValidateDispatchError::InvalidWaitSource, lease));
        }
        let observed_generation = self.observed_generation.get(&source).copied().unwrap_or(0);
        if observed_generation == u64::MAX {
            drop(prepared);
            return Err((DurableValidateDispatchError::WaitGenerationExhausted, lease));
        }
        if self.records.iter().any(|(ordinal, record)| {
            *ordinal != lease.ordinal()
                && matches!(
                    record.state,
                    LifecycleState::Waiting(wait) if wait.source() == source
                )
        }) {
            drop(prepared);
            return Err((DurableValidateDispatchError::AliasedWaitSource, lease));
        }
        let wait_token = WaitToken::new(source, observed_generation);
        let mut next = self.stage_durable_transaction();
        next.reduce_settle_turn(lease.clone(), TurnOutcome::Blocked(wait_token), None);
        if !staged_durable_validate_wait_is_exact(self, &next, &lease, wait_token) {
            drop(prepared);
            return Err((DurableValidateDispatchError::InvalidStagedTransition, lease));
        }
        let dispatch = match prepared.seal_waiting_dispatch(wait_token) {
            Ok(dispatch) => dispatch,
            Err(prepared) => {
                drop(prepared);
                return Err((DurableValidateDispatchError::InvalidWaitSource, lease));
            }
        };
        *self = next;
        Ok(dispatch)
    }
    /// Atomically publish one exact executable Validate result across the
    /// volatile coordinator and concrete registry.
    ///
    /// All fallible checks and the complete logical Ready projection precede
    /// registry staging. The specialized registry guard then owns rollback
    /// until the adjacent, non-panicking coordinator swap and guard commit.
    /// Merge-sidecar deferral changes neither side and retains the full
    /// executed dispatch for its sealed service transaction.
    ///
    /// Waiting/Ready state and concrete physical carriers are intentionally
    /// excluded from LifecycleLedgerV1, so this volatile cut performs no
    /// lifecycle-ledger write. A crash before the swap restores or loses only
    /// volatile staging; a crash after it recovers the durable Validate row and
    /// revalidates the body-store marker.
    #[cfg_attr(not(test), allow(dead_code))]
    #[allow(clippy::result_large_err)]
    pub(super) fn complete_durable_validate_dispatch(
        &mut self,
        registry: &mut LifecycleWorkRegistryHolder,
        dispatch: ExecutedDurableValidateDispatch,
    ) -> Result<
        DurableValidateCompletionPublication,
        (
            DurableValidateCompletionPublicationError,
            ExecutedDurableValidateDispatch,
        ),
    > {
        if self.fault.is_some() {
            return Err((
                DurableValidateCompletionPublicationError::CoordinatorFaulted,
                dispatch,
            ));
        }
        let prepared = registry
            .registry
            .prepare_executed_durable_validate_completion(dispatch)?;
        let authority = prepared.authority();
        if !waiting_durable_validate_record_is_exact(self, authority) {
            return Err(
                prepared.fail(DurableValidateCompletionPublicationError::InvalidWaitingState)
            );
        }
        if authority.is_deferred_merge_sidecar() {
            return Ok(DurableValidateCompletionPublication::DeferredMergeSidecar(
                prepared.defer_merge_sidecar(),
            ));
        }
        let Some(ready_event) = authority.ready_event() else {
            return Err(
                prepared.fail(DurableValidateCompletionPublicationError::InvalidStagedTransition)
            );
        };
        let mut next = self.stage_durable_transaction();
        next.publish_ready(ready_event);
        if !staged_durable_validate_ready_is_exact(self, &next, authority) {
            return Err(
                prepared.fail(DurableValidateCompletionPublicationError::InvalidStagedTransition)
            );
        }
        let staged_registry = prepared.stage_executable_carrier()?;
        core::mem::swap(self, &mut next);
        let published = staged_registry.commit();
        Ok(match published {
            PublishedDurableValidateCompletion::Validated(published) => {
                DurableValidateCompletionPublication::PublishedValidated(published)
            }
            PublishedDurableValidateCompletion::Rejected(published) => {
                DurableValidateCompletionPublication::PublishedRejected(published)
            }
        })
    }
    /// Bind one exact Ready Validate carrier into a transient scheduler seal.
    ///
    /// The registry returns only a closed carrier classification. This method
    /// joins it to the coordinator's complete row identity and physical digest
    /// without exposing a caller-mintable demand bit or capacity class.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn attest_ready_validate_demand(
        &self,
        registry: &LifecycleWorkRegistryHolder,
        ordinal: u128,
    ) -> Result<AttestedReadyValidateDemand, ReadyValidateDemandAttestationError> {
        let Some(record) = self.records.get(&ordinal) else {
            return Err(ReadyValidateDemandAttestationError::InvalidCoordinatorIndex);
        };
        let readiness_index_is_exact = match record.state {
            LifecycleState::Ready => self.ready_index.contains(&ordinal),
            LifecycleState::Waiting(wait)
                if matches!(
                    wait.source(),
                    WaitSource::External(_) | WaitSource::Recovery(_)
                ) =>
            {
                !self.ready_index.contains(&ordinal)
            }
            LifecycleState::Waiting(_)
            | LifecycleState::Claimed(_)
            | LifecycleState::Terminal(_) => false,
        };
        if self.fault.is_some()
            || self.active_lease.is_some()
            || record.ordinal != ordinal
            || record.work_class != LifecycleWorkClass::Validate
            || record.key.phase() != LifecyclePhase::Validate
            || record.stage.kind() != LifecycleStageKind::ValidateBody
            || record.stage.predecessor_scope() != PredecessorScope::Independent
            || record.physical_slots.len() != 1
            || record.episode.slot_universe.len() != 1
            || record.episode.consumed_slots != record.episode.slot_universe
            || !record.episode.frozen_predecessors.is_empty()
            || self.episode_authority.universe_for(record.key).as_ref()
                != Some(&record.episode.universe)
            || !self.episode_authority.admits_slots(
                record.work_class.capacity_class(),
                &record.episode.slot_universe,
            )
            || !readiness_index_is_exact
            || self.key_index.get(&record.key) != Some(&ordinal)
            || self.owner_index.get(&record.owner.causal_root()) != Some(&record.owner)
            || self
                .records
                .values()
                .filter(|candidate| candidate.ordinal == ordinal)
                .count()
                != 1
            || self
                .records
                .values()
                .filter(|candidate| candidate.key == record.key)
                .count()
                != 1
            || self
                .key_index
                .values()
                .filter(|candidate| **candidate == ordinal)
                .count()
                != 1
            || self
                .owner_index
                .values()
                .filter(|owner| **owner == record.owner)
                .count()
                != 1
            || !self.durable_records.get(&ordinal).is_some_and(|metadata| {
                metadata.reconstruction_source == record.owner.causal_root().digest()
                    && durable_validate_payload_is_exact(record.key, metadata.payload)
                    && metadata.continuation == super::schema::DurableContinuation::None
            })
        {
            return Err(ReadyValidateDemandAttestationError::InvalidCoordinatorIndex);
        }
        let (&slot, &digest) = record
            .physical_slots
            .first_key_value()
            .expect("one-slot Ready Validate shape checked above");
        if !record.episode.slot_universe.contains(&slot)
            || slot.capacity_class() != Some(record.work_class.capacity_class())
        {
            return Err(ReadyValidateDemandAttestationError::InvalidCoordinatorIndex);
        }
        let address = ConcreteWorkAddress::new(record.owner, ordinal, slot)
            .ok_or(ReadyValidateDemandAttestationError::InvalidCoordinatorIndex)?;
        let seal = registry
            .registry
            .classify_ready_validate_carrier(address, digest)
            .map_err(ReadyValidateDemandAttestationError::Registry)?;
        if !self
            .durable_records
            .get(&ordinal)
            .is_some_and(|metadata| seal.matches_durable_payload(record.key, metadata.payload))
        {
            return Err(ReadyValidateDemandAttestationError::InvalidCoordinatorIndex);
        }
        AttestedReadyValidateDemand::from_registry_seal(record, seal)
            .ok_or(ReadyValidateDemandAttestationError::InvalidCoordinatorIndex)
    }
    /// Prepare one complete signed effect for mandatory replay-bound admission.
    ///
    /// Projection, candidate construction, and replay-authority attachment
    /// happen while the exact effect and pending binding remain inside one
    /// move-only owner.  Failure cannot produce a generic candidate.
    #[allow(clippy::result_large_err)]
    pub(super) fn prepare_direct_signed_lifecycle_admission(
        &self,
        verified: &VerifiedHeightContext,
        effect: AdapterEffect,
        pending: PendingRuntimeEffectBinding,
    ) -> Result<PreparedLifecycleAdmissionV1, PreparedLifecycleAdmissionErrorV1> {
        PreparedLifecycleAdmissionV1::direct_signed(self.active_context, verified, effect, pending)
    }
    /// Atomically admit one mandatory replay-authorized prepared owner.
    ///
    /// A first admission stages logical state, installs the exact
    /// coordinator-minted owner/ordinal/slot, publishes LedgerV1, and only then
    /// exposes both state changes. Registry installation is synchronously
    /// undone if publication fails. An exact recovered retry installs at its
    /// existing immutable address with the same ordering. Every other decision
    /// returns the complete prepared owner and leaves incumbent work untouched.
    // Production settlement retains or drops each returned prepared owner
    // according to its decision; Retry executes the incumbent entry and never
    // replaces it with the returned duplicate.
    pub(super) fn admit_prepared_lifecycle(
        &mut self,
        registry: &mut LifecycleWorkRegistryHolder,
        prepared: PreparedLifecycleAdmissionV1,
    ) -> AdapterEffectAdmissionTransaction {
        if !prepared.validates(self.active_context) {
            return AdapterEffectAdmissionTransaction::Failed {
                failure: AdapterEffectAdmissionFailure::Registry(RegistryError::CorruptWork),
                prepared,
            };
        }
        let candidate: CandidateAdmission = prepared.candidate().clone();
        let recovery_rebind_ordinal =
            self.key_index
                .get(&candidate.key)
                .copied()
                .filter(|ordinal| {
                    self.records.get(ordinal).is_some_and(|record| {
                        matches!(
                            record.state,
                            LifecycleState::Waiting(super::WaitToken {
                                source: WaitSource::Recovery(_),
                                ..
                            })
                        )
                    })
                });
        let mut next = self.stage_durable_transaction();
        let (decision, ordinal_reservation) =
            next.reduce_admit_with_durable_ordinals(AdmissionRequest::Candidate(candidate.clone()));
        let first_admission = matches!(decision, AdmissionDecision::Admitted { .. });
        let recovery_rebind = matches!(
            decision,
            AdmissionDecision::Retry { ordinal, .. }
                if recovery_rebind_ordinal == Some(ordinal)
                    && next.records.get(&ordinal).is_some_and(|record| {
                        record.state == LifecycleState::Ready
                    })
        );
        if first_admission || recovery_rebind {
            let location = match concrete_work_location(&next, decision, recovery_rebind_ordinal) {
                Ok(location) => location,
                Err(error) => {
                    return AdapterEffectAdmissionTransaction::Failed {
                        failure: AdapterEffectAdmissionFailure::Registry(error),
                        prepared,
                    };
                }
            };
            return match prepared.into_owner() {
                PreparedLifecycleAdmissionOwnerV1::LiveWal(live) => {
                    match registry.registry.install_live_wal_before_publication(
                        self.active_context,
                        &candidate,
                        location.address,
                        location.digest,
                        live,
                        || {
                            next.persist_durable_projection_with_ordinal_reservation(
                                ordinal_reservation.as_ref(),
                            )
                        },
                    ) {
                        Ok(()) => {
                            *self = next;
                            if first_admission {
                                AdapterEffectAdmissionTransaction::Admitted(decision)
                            } else {
                                AdapterEffectAdmissionTransaction::Rebound(decision)
                            }
                        }
                        Err(LiveWalRegistryPublicationErrorV1::Install(error, live)) => {
                            let prepared = PreparedLifecycleAdmissionV1::from_returned_live_wal(
                                self.active_context,
                                candidate,
                                live,
                            )
                            .expect("registry rollback returns the exact live-WAL admission owner");
                            AdapterEffectAdmissionTransaction::Failed {
                                failure: AdapterEffectAdmissionFailure::Registry(error),
                                prepared,
                            }
                        }
                        Err(LiveWalRegistryPublicationErrorV1::Publication(_, live)) => {
                            self.fault = Some(CoordinatorFault::DurabilityFailure);
                            let prepared = PreparedLifecycleAdmissionV1::from_returned_live_wal(
                                self.active_context,
                                candidate,
                                live,
                            )
                            .expect(
                                "publication rollback returns the exact live-WAL admission owner",
                            );
                            AdapterEffectAdmissionTransaction::Failed {
                                failure: AdapterEffectAdmissionFailure::Durability,
                                prepared,
                            }
                        }
                    }
                }
                PreparedLifecycleAdmissionOwnerV1::InvalidBodyReport(bound)
                | PreparedLifecycleAdmissionOwnerV1::DirectSigned(bound) => {
                    match registry.registry.install_bound_before_publication(
                        self.active_context,
                        &candidate,
                        location.address,
                        location.digest,
                        bound,
                        || {
                            next.persist_durable_projection_with_ordinal_reservation(
                                ordinal_reservation.as_ref(),
                            )
                        },
                    ) {
                        Ok(()) => {
                            *self = next;
                            if first_admission {
                                AdapterEffectAdmissionTransaction::Admitted(decision)
                            } else {
                                AdapterEffectAdmissionTransaction::Rebound(decision)
                            }
                        }
                        Err(BoundAdapterRegistryPublicationErrorV1::Install(error, bound)) => {
                            let prepared = PreparedLifecycleAdmissionV1::from_returned_bound(
                                self.active_context,
                                candidate,
                                bound,
                            )
                            .expect("registry rollback returns the exact prepared admission owner");
                            AdapterEffectAdmissionTransaction::Failed {
                                failure: AdapterEffectAdmissionFailure::Registry(error),
                                prepared,
                            }
                        }
                        Err(BoundAdapterRegistryPublicationErrorV1::Publication(_, bound)) => {
                            self.fault = Some(CoordinatorFault::DurabilityFailure);
                            let prepared = PreparedLifecycleAdmissionV1::from_returned_bound(
                                self.active_context,
                                candidate,
                                bound,
                            )
                            .expect(
                                "publication rollback returns the exact prepared admission owner",
                            );
                            AdapterEffectAdmissionTransaction::Failed {
                                failure: AdapterEffectAdmissionFailure::Durability,
                                prepared,
                            }
                        }
                    }
                }
                owner @ (PreparedLifecycleAdmissionOwnerV1::LocalBody(_)
                | PreparedLifecycleAdmissionOwnerV1::RemoteProposal(_)) => {
                    let validate = match owner {
                        PreparedLifecycleAdmissionOwnerV1::LocalBody(validate) => {
                            super::work_registry::PreparedDurableValidateAdmissionV1::LocalBody(
                                validate,
                            )
                        }
                        PreparedLifecycleAdmissionOwnerV1::RemoteProposal(validate) => {
                            super::work_registry::PreparedDurableValidateAdmissionV1::RemoteProposal(
                                validate,
                            )
                        }
                        PreparedLifecycleAdmissionOwnerV1::LiveWal(_)
                        | PreparedLifecycleAdmissionOwnerV1::InvalidBodyReport(_)
                        | PreparedLifecycleAdmissionOwnerV1::DirectSigned(_) => unreachable!(),
                    };
                    match registry
                        .registry
                        .install_durable_validate_before_publication(
                            self.active_context,
                            &candidate,
                            location.address,
                            location.digest,
                            validate,
                            || {
                                next.persist_durable_projection_with_ordinal_reservation(
                                    ordinal_reservation.as_ref(),
                                )
                            },
                        ) {
                        Ok(()) => {
                            *self = next;
                            if first_admission {
                                AdapterEffectAdmissionTransaction::Admitted(decision)
                            } else {
                                AdapterEffectAdmissionTransaction::Rebound(decision)
                            }
                        }
                        Err(DurableValidateRegistryPublicationErrorV1::Install(
                            error,
                            validate,
                        )) => {
                            let prepared =
                                PreparedLifecycleAdmissionV1::from_returned_durable_validate(
                                    self.active_context,
                                    candidate,
                                    validate,
                                )
                                .expect(
                                    "registry rollback returns the exact durable Validate owner",
                                );
                            AdapterEffectAdmissionTransaction::Failed {
                                failure: AdapterEffectAdmissionFailure::Registry(error),
                                prepared,
                            }
                        }
                        Err(DurableValidateRegistryPublicationErrorV1::Publication(
                            _,
                            validate,
                        )) => {
                            self.fault = Some(CoordinatorFault::DurabilityFailure);
                            let prepared =
                                PreparedLifecycleAdmissionV1::from_returned_durable_validate(
                                    self.active_context,
                                    candidate,
                                    validate,
                                )
                                .expect(
                                    "publication rollback returns the exact durable Validate owner",
                                );
                            AdapterEffectAdmissionTransaction::Failed {
                                failure: AdapterEffectAdmissionFailure::Durability,
                                prepared,
                            }
                        }
                    }
                }
            };
        }
        *self = next;
        AdapterEffectAdmissionTransaction::Returned { decision, prepared }
    }
}
impl ProductionLifecycleOwnerV1 {
    /// Prepare and atomically admit one post-fsync live-WAL Sign owner.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn settle_live_wal_sign_admission(
        &mut self,
        pending: PendingLiveWalSignAdmissionV1,
    ) -> ProductionLiveWalSignAdmissionSettlementV1 {
        let prepared = match pending.prepare(self.coordinator.active_context(), &self.verified) {
            Ok(prepared) => prepared,
            Err((failure, pending)) => {
                return ProductionLiveWalSignAdmissionSettlementV1::Failed {
                    failure: ProductionLiveWalSignAdmissionFailureV1::Projection(failure),
                    pending,
                };
            }
        };
        match self
            .coordinator
            .admit_prepared_lifecycle(&mut self.registry, prepared)
        {
            AdapterEffectAdmissionTransaction::Admitted(decision) => {
                ProductionLiveWalSignAdmissionSettlementV1::Admitted(decision)
            }
            AdapterEffectAdmissionTransaction::Rebound(decision) => {
                ProductionLiveWalSignAdmissionSettlementV1::Rebound(decision)
            }
            AdapterEffectAdmissionTransaction::Returned { decision, prepared } => {
                ProductionLiveWalSignAdmissionSettlementV1::Returned {
                    decision,
                    pending: PendingLiveWalSignAdmissionV1::reclaim_returned(prepared),
                }
            }
            AdapterEffectAdmissionTransaction::Failed { failure, prepared } => {
                let failure = match failure {
                    AdapterEffectAdmissionFailure::Registry(_) => {
                        ProductionLiveWalSignAdmissionFailureV1::Registry
                    }
                    AdapterEffectAdmissionFailure::Durability => {
                        ProductionLiveWalSignAdmissionFailureV1::Durability
                    }
                };
                ProductionLiveWalSignAdmissionSettlementV1::Failed {
                    failure,
                    pending: PendingLiveWalSignAdmissionV1::reclaim_returned(prepared),
                }
            }
        }
    }

    /// Prepare and atomically settle one origin-specific durable Validate owner.
    ///
    /// The pending owner can be projected only against this production owner's
    /// coheld logical and verified height contexts. The existing atomic
    /// coordinator/registry transaction remains the sole ordinal allocator.
    /// In particular, a returned `Retry` keeps the exact pending owner and the
    /// incumbent ordinal; resubmission cannot allocate a replacement ordinal.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn settle_durable_validate_admission(
        &mut self,
        pending: PendingDurableValidateAdmissionV1,
    ) -> ProductionDurableValidateAdmissionSettlementV1 {
        let prepared = match pending.prepare(self.coordinator.active_context(), &self.verified) {
            Ok(prepared) => prepared,
            Err((failure, pending)) => {
                return ProductionDurableValidateAdmissionSettlementV1::Failed {
                    failure: ProductionDurableValidateAdmissionFailureV1::Projection(failure),
                    pending,
                };
            }
        };
        match self
            .coordinator
            .admit_prepared_lifecycle(&mut self.registry, prepared)
        {
            AdapterEffectAdmissionTransaction::Admitted(decision) => {
                ProductionDurableValidateAdmissionSettlementV1::Admitted(decision)
            }
            AdapterEffectAdmissionTransaction::Rebound(decision) => {
                ProductionDurableValidateAdmissionSettlementV1::Rebound(decision)
            }
            AdapterEffectAdmissionTransaction::Returned { decision, prepared } => {
                ProductionDurableValidateAdmissionSettlementV1::Returned {
                    decision,
                    pending: PendingDurableValidateAdmissionV1::reclaim_returned(prepared),
                }
            }
            AdapterEffectAdmissionTransaction::Failed { failure, prepared } => {
                let failure = match failure {
                    AdapterEffectAdmissionFailure::Registry(_) => {
                        ProductionDurableValidateAdmissionFailureV1::Registry
                    }
                    AdapterEffectAdmissionFailure::Durability => {
                        ProductionDurableValidateAdmissionFailureV1::Durability
                    }
                };
                ProductionDurableValidateAdmissionSettlementV1::Failed {
                    failure,
                    pending: PendingDurableValidateAdmissionV1::reclaim_returned(prepared),
                }
            }
        }
    }

    /// Admit or rejoin one exact runtime output, execute its service effect in
    /// lifecycle ordinal order, and durably terminalize the same row.
    ///
    /// Live-WAL and rejected-Validate successors rejoin their already-installed
    /// concrete row. Only an absent complete signed output may invoke direct
    /// admission. Every branch before service I/O returns the same move-only
    /// owner; after service success, any publication ambiguity closes the
    /// lifecycle owner for restart rather than replaying volatile state.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn settle_lifecycle_output_admission<E>(
        &mut self,
        pending: PendingLifecycleOutputAdmissionV1,
        execute: impl FnOnce(
            &AdapterEffect,
            &crate::sumeragi::v2_runtime::RuntimeEffectOwnership,
        ) -> Result<LifecycleOutputServiceDispositionV1, E>,
    ) -> ProductionLifecycleOutputAdmissionSettlementV1<E> {
        let mut execution = pending.into_existing_execution();
        let initial_join = match self
            .registry
            .join_lifecycle_output(&self.coordinator, &execution)
        {
            Ok(join) => join,
            Err(error) => {
                return ProductionLifecycleOutputAdmissionSettlementV1::Failed {
                    failure: ProductionLifecycleOutputAdmissionFailureV1::Registry(error),
                    pending: execution.into_pending(),
                };
            }
        };
        match initial_join {
            LifecycleOutputRegistryJoinV1::Ready(_) => {}
            LifecycleOutputRegistryJoinV1::TerminalInstalledDuplicate(retirement) => {
                return self
                    .settle_terminal_installed_lifecycle_output_duplicate(execution, retirement);
            }
            LifecycleOutputRegistryJoinV1::RecoveredBroadcastOwned
            | LifecycleOutputRegistryJoinV1::TerminalDirectOutputDuplicate => {
                return ProductionLifecycleOutputAdmissionSettlementV1::AlreadyCompleted;
            }
            LifecycleOutputRegistryJoinV1::Deferred => {
                return ProductionLifecycleOutputAdmissionSettlementV1::Deferred(
                    execution.into_pending(),
                );
            }
            LifecycleOutputRegistryJoinV1::Missing => {
                let pending = execution.into_pending();
                let (prepared, returned_execution) = match pending
                    .prepare_direct_signed(self.coordinator.active_context(), &self.verified)
                {
                    Ok(prepared) => prepared,
                    Err(error) => {
                        return ProductionLifecycleOutputAdmissionSettlementV1::Failed {
                            failure: ProductionLifecycleOutputAdmissionFailureV1::Projection(
                                error.failure,
                            ),
                            pending: error.pending,
                        };
                    }
                };
                execution = returned_execution;
                // Reset-11 was committed before genuinely direct lifecycle
                // outputs acquired durable registry rows. Preserve that
                // authenticated producer behavior only after the current
                // projection has proved this is a direct signed output and
                // only when no WAL/recovery row exists. Existing rows retain
                // the durable current settlement path below.
                if uses_pre_release_taira_direct_output_compatibility(
                    &self.verified.context().network_id,
                    self.verified.context().height,
                ) {
                    return settle_pre_release_direct_output(prepared, execution, execute);
                }
                match self
                    .coordinator
                    .admit_prepared_lifecycle(&mut self.registry, prepared)
                {
                    AdapterEffectAdmissionTransaction::Admitted(AdmissionDecision::Admitted {
                        ..
                    })
                    | AdapterEffectAdmissionTransaction::Rebound(AdmissionDecision::Retry {
                        ..
                    }) => {}
                    AdapterEffectAdmissionTransaction::Admitted(_)
                    | AdapterEffectAdmissionTransaction::Rebound(_) => {
                        return ProductionLifecycleOutputAdmissionSettlementV1::Failed {
                            failure: ProductionLifecycleOutputAdmissionFailureV1::Registry(
                                RegistryError::InvalidAdmissionShape,
                            ),
                            pending: execution.into_pending(),
                        };
                    }
                    AdapterEffectAdmissionTransaction::Returned { decision, prepared } => {
                        let pending = PendingLifecycleOutputAdmissionV1::reclaim_returned(
                            prepared, execution,
                        );
                        return match decision {
                            AdmissionDecision::WaitForCapacity(_)
                            | AdmissionDecision::Retry { .. } => {
                                ProductionLifecycleOutputAdmissionSettlementV1::Deferred(pending)
                            }
                            AdmissionDecision::ReplayTerminal { .. }
                            | AdmissionDecision::StutterTerminal { .. } => {
                                ProductionLifecycleOutputAdmissionSettlementV1::AlreadyCompleted
                            }
                            AdmissionDecision::Admitted { .. }
                            | AdmissionDecision::NonCandidate
                            | AdmissionDecision::Rejected(_)
                            | AdmissionDecision::FailClosed(_) => {
                                ProductionLifecycleOutputAdmissionSettlementV1::Failed {
                                    failure: ProductionLifecycleOutputAdmissionFailureV1::Registry(
                                        RegistryError::InvalidAdmissionShape,
                                    ),
                                    pending,
                                }
                            }
                        };
                    }
                    AdapterEffectAdmissionTransaction::Failed { failure, prepared } => {
                        let pending = PendingLifecycleOutputAdmissionV1::reclaim_returned(
                            prepared, execution,
                        );
                        let failure = match failure {
                            AdapterEffectAdmissionFailure::Registry(error) => {
                                ProductionLifecycleOutputAdmissionFailureV1::Registry(error)
                            }
                            AdapterEffectAdmissionFailure::Durability => {
                                ProductionLifecycleOutputAdmissionFailureV1::Durability
                            }
                        };
                        return ProductionLifecycleOutputAdmissionSettlementV1::Failed {
                            failure,
                            pending,
                        };
                    }
                }
            }
        }
        let retirement = match self
            .registry
            .join_lifecycle_output(&self.coordinator, &execution)
        {
            Ok(LifecycleOutputRegistryJoinV1::Ready(retirement)) => retirement,
            Ok(LifecycleOutputRegistryJoinV1::Deferred) => {
                return ProductionLifecycleOutputAdmissionSettlementV1::Deferred(
                    execution.into_pending(),
                );
            }
            Ok(
                LifecycleOutputRegistryJoinV1::Missing
                | LifecycleOutputRegistryJoinV1::RecoveredBroadcastOwned
                | LifecycleOutputRegistryJoinV1::TerminalDirectOutputDuplicate
                | LifecycleOutputRegistryJoinV1::TerminalInstalledDuplicate(_),
            ) => {
                return ProductionLifecycleOutputAdmissionSettlementV1::Failed {
                    failure: ProductionLifecycleOutputAdmissionFailureV1::Registry(
                        RegistryError::CorruptWork,
                    ),
                    pending: execution.into_pending(),
                };
            }
            Err(error) => {
                return ProductionLifecycleOutputAdmissionSettlementV1::Failed {
                    failure: ProductionLifecycleOutputAdmissionFailureV1::Registry(error),
                    pending: execution.into_pending(),
                };
            }
        };
        match execution.execute_with(execute) {
            Ok(LifecycleOutputServiceDispositionV1::Accepted) => {}
            Ok(LifecycleOutputServiceDispositionV1::SourceRetained) => {
                return ProductionLifecycleOutputAdmissionSettlementV1::Deferred(
                    execution.into_pending(),
                );
            }
            Err(error) => {
                return ProductionLifecycleOutputAdmissionSettlementV1::Failed {
                    failure: ProductionLifecycleOutputAdmissionFailureV1::Service(error),
                    pending: execution.into_pending(),
                };
            }
        }
        let mut staged = self.coordinator.stage_durable_transaction();
        if staged
            .finish_terminal(retirement.ordinal(), super::TerminalOutcome::Advanced)
            .is_err()
            || !self.registry.lifecycle_output_terminal_is_exact(
                &self.coordinator,
                &staged,
                retirement,
                &execution,
            )
        {
            return ProductionLifecycleOutputAdmissionSettlementV1::Failed {
                failure: ProductionLifecycleOutputAdmissionFailureV1::Registry(
                    RegistryError::CorruptWork,
                ),
                pending: execution.into_pending(),
            };
        }
        if self
            .coordinator
            .persist_exact_staged_successor(&staged)
            .is_err()
        {
            self.coordinator.fault = Some(CoordinatorFault::DurabilityFailure);
            return ProductionLifecycleOutputAdmissionSettlementV1::Failed {
                failure: ProductionLifecycleOutputAdmissionFailureV1::Durability,
                pending: execution.into_pending(),
            };
        }
        self.registry
            .publish_lifecycle_output_terminal_after_fsync(retirement);
        self.coordinator = staged;
        ProductionLifecycleOutputAdmissionSettlementV1::Completed
    }

    /// Confirm the already-durable terminal frame, then retire only the stray
    /// process-local carrier installed at that same immutable address.
    fn settle_terminal_installed_lifecycle_output_duplicate<E>(
        &mut self,
        execution: PreparedLifecycleOutputExecutionV1,
        retirement: PreparedLifecycleOutputRegistryRetirementV1,
    ) -> ProductionLifecycleOutputAdmissionSettlementV1<E> {
        if !self.registry.lifecycle_output_terminal_installed_is_exact(
            &self.coordinator,
            retirement,
            &execution,
        ) {
            return ProductionLifecycleOutputAdmissionSettlementV1::Failed {
                failure: ProductionLifecycleOutputAdmissionFailureV1::Registry(
                    RegistryError::CorruptWork,
                ),
                pending: execution.into_pending(),
            };
        }
        if self
            .coordinator
            .persist_exact_staged_successor(&self.coordinator)
            .is_err()
        {
            self.coordinator.fault = Some(CoordinatorFault::DurabilityFailure);
            return ProductionLifecycleOutputAdmissionSettlementV1::Failed {
                failure: ProductionLifecycleOutputAdmissionFailureV1::Durability,
                pending: execution.into_pending(),
            };
        }
        self.registry
            .publish_lifecycle_output_terminal_after_fsync(retirement);
        ProductionLifecycleOutputAdmissionSettlementV1::AlreadyCompleted
    }
}
fn waiting_durable_validate_record_is_exact(
    coordinator: &LifecycleCoordinator,
    authority: DurableValidateCompletionAuthority,
) -> bool {
    let Some(record) = coordinator.records.get(&authority.ordinal()) else {
        return false;
    };
    let mut exact_slots = std::collections::BTreeSet::new();
    exact_slots.insert(authority.slot());
    let wait_token = authority.wait_token();
    coordinator.active_lease.is_none()
        && record.ordinal == authority.ordinal()
        && record.owner == authority.owner()
        && record.key == authority.lifecycle_key()
        && record.work_class == LifecycleWorkClass::Validate
        && record.key.phase() == LifecyclePhase::Validate
        && record.stage == authority.lifecycle_stage()
        && record.stage.kind() == LifecycleStageKind::ValidateBody
        && record.stage.predecessor_scope() == PredecessorScope::Independent
        && record.state == LifecycleState::Waiting(wait_token)
        && record.physical_slots.len() == 1
        && record.physical_slots.get(&authority.slot()) == Some(&authority.incumbent_digest())
        && record.episode.slot_universe == exact_slots
        && record.episode.consumed_slots == exact_slots
        && record.episode.frozen_predecessors.is_empty()
        && coordinator
            .episode_authority
            .universe_for(record.key)
            .as_ref()
            == Some(&record.episode.universe)
        && coordinator.episode_authority.admits_slots(
            record.work_class.capacity_class(),
            &record.episode.slot_universe,
        )
        && coordinator.key_index.get(&record.key) == Some(&record.ordinal)
        && coordinator.owner_index.get(&record.owner.causal_root()) == Some(&record.owner)
        && coordinator
            .records
            .values()
            .filter(|candidate| candidate.ordinal == record.ordinal)
            .count()
            == 1
        && coordinator
            .records
            .values()
            .filter(|candidate| candidate.key == record.key)
            .count()
            == 1
        && coordinator
            .key_index
            .values()
            .filter(|ordinal| **ordinal == record.ordinal)
            .count()
            == 1
        && coordinator
            .owner_index
            .values()
            .filter(|owner| **owner == record.owner)
            .count()
            == 1
        && !coordinator.ready_index.contains(&record.ordinal)
        && coordinator.observed_generation.get(&wait_token.source())
            == Some(&wait_token.observed_generation())
        && coordinator.records.iter().all(|(ordinal, candidate)| {
            *ordinal == record.ordinal
                || !matches!(
                    candidate.state,
                    LifecycleState::Waiting(wait) if wait.source() == wait_token.source()
                )
        })
        && coordinator
            .durable_records
            .get(&record.ordinal)
            .is_some_and(|metadata| {
                metadata.reconstruction_source == record.owner.causal_root().digest()
                    && durable_validate_payload_is_exact(record.key, metadata.payload)
                    && authority.matches_durable_payload(metadata.payload)
            })
}
#[allow(clippy::too_many_lines)]
fn staged_durable_validate_ready_is_exact(
    current: &LifecycleCoordinator,
    next: &LifecycleCoordinator,
    authority: DurableValidateCompletionAuthority,
) -> bool {
    let Some(replacement_digest) = authority.replacement_digest() else {
        return false;
    };
    let Some(next_generation) = authority.wait_token().observed_generation().checked_add(1) else {
        return false;
    };
    let mut expected_record = current
        .records
        .get(&authority.ordinal())
        .expect("waiting Validate preflight checked its exact record")
        .clone();
    expected_record.state = LifecycleState::Ready;
    expected_record
        .physical_slots
        .insert(authority.slot(), replacement_digest);
    let mut expected_ready = current.ready_index.clone();
    expected_ready.insert(authority.ordinal());
    let mut expected_observed = current.observed_generation.clone();
    expected_observed.insert(authority.wait_token().source(), next_generation);
    next.episode_authority == current.episode_authority
        && next.active_context == current.active_context
        && next.records.len() == current.records.len()
        && next.records.get(&authority.ordinal()) == Some(&expected_record)
        && current.records.iter().all(|(ordinal, record)| {
            *ordinal == authority.ordinal() || next.records.get(ordinal) == Some(record)
        })
        && next.key_index == current.key_index
        && next.owner_index == current.owner_index
        && next.ready_index == expected_ready
        && next.admission_waits == current.admission_waits
        && next.active_lease == current.active_lease
        && next.high_water == current.high_water
        && next.next_lease == current.next_lease
        && next.durable_records == current.durable_records
        && next.capacity_geometry == current.capacity_geometry
        && next.capacity_used == current.capacity_used
        && next.capacity_generation == current.capacity_generation
        && next.observed_generation == expected_observed
        && next.producer_debts == current.producer_debts
        && next.ledger_store.is_some() == current.ledger_store.is_some()
        && next.fault.is_none()
}
fn claimed_durable_validate_record_is_exact(
    coordinator: &LifecycleCoordinator,
    lease: &TurnLease,
) -> bool {
    let Some(record) = coordinator.records.get(&lease.ordinal()) else {
        return false;
    };
    let lease_slots = lease
        .physical_slots()
        .keys()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    coordinator.active_lease.as_ref() == Some(lease)
        && record.ordinal == lease.ordinal()
        && record.owner == lease.owner()
        && record.key == lease.key()
        && record.work_class == LifecycleWorkClass::Validate
        && record.work_class == lease.work_class()
        && record.key.phase() == LifecyclePhase::Validate
        && record.stage == lease.stage()
        && record.stage.kind() == LifecycleStageKind::ValidateBody
        && record.stage.predecessor_scope() == PredecessorScope::Independent
        && record.state == LifecycleState::Claimed(lease.id())
        && record.physical_slots.len() == 1
        && record.physical_slots == *lease.physical_slots()
        && record.episode.slot_universe == lease_slots
        && record.episode.consumed_slots == lease_slots
        && record.episode.frozen_predecessors.is_empty()
        && coordinator
            .episode_authority
            .universe_for(record.key)
            .as_ref()
            == Some(&record.episode.universe)
        && coordinator.episode_authority.admits_slots(
            record.work_class.capacity_class(),
            &record.episode.slot_universe,
        )
        && coordinator.key_index.get(&record.key) == Some(&record.ordinal)
        && coordinator.owner_index.get(&record.owner.causal_root()) == Some(&record.owner)
        && coordinator
            .records
            .values()
            .filter(|candidate| candidate.ordinal == record.ordinal)
            .count()
            == 1
        && coordinator
            .records
            .values()
            .filter(|candidate| candidate.key == record.key)
            .count()
            == 1
        && coordinator
            .key_index
            .values()
            .filter(|ordinal| **ordinal == record.ordinal)
            .count()
            == 1
        && coordinator
            .owner_index
            .values()
            .filter(|owner| **owner == record.owner)
            .count()
            == 1
        && !coordinator.ready_index.contains(&record.ordinal)
        && coordinator
            .durable_records
            .get(&record.ordinal)
            .is_some_and(|metadata| {
                metadata.reconstruction_source == record.owner.causal_root().digest()
                    && durable_validate_payload_is_exact(record.key, metadata.payload)
            })
}
#[allow(clippy::too_many_lines)]
fn staged_durable_validate_wait_is_exact(
    current: &LifecycleCoordinator,
    next: &LifecycleCoordinator,
    lease: &TurnLease,
    wait_token: WaitToken,
) -> bool {
    let mut expected_record = current
        .records
        .get(&lease.ordinal())
        .expect("claimed Validate preflight checked its record")
        .clone();
    expected_record.state = LifecycleState::Waiting(wait_token);
    let mut expected_observed = current.observed_generation.clone();
    expected_observed
        .entry(wait_token.source())
        .or_insert(wait_token.observed_generation());
    next.episode_authority == current.episode_authority
        && next.active_context == current.active_context
        && next.records.len() == current.records.len()
        && next.records.get(&lease.ordinal()) == Some(&expected_record)
        && current.records.iter().all(|(ordinal, record)| {
            *ordinal == lease.ordinal() || next.records.get(ordinal) == Some(record)
        })
        && next.key_index == current.key_index
        && next.owner_index == current.owner_index
        && next.ready_index == current.ready_index
        && next.admission_waits == current.admission_waits
        && next.active_lease.is_none()
        && next.high_water == current.high_water
        && next.next_lease == current.next_lease
        && next.durable_records == current.durable_records
        && next.capacity_geometry == current.capacity_geometry
        && next.capacity_used == current.capacity_used
        && next.capacity_generation == current.capacity_generation
        && next.observed_generation == expected_observed
        && next.producer_debts == current.producer_debts
        && next.ledger_store.is_some() == current.ledger_store.is_some()
        && next.fault.is_none()
}
fn concrete_work_location(
    coordinator: &LifecycleCoordinator,
    decision: AdmissionDecision,
    recovery_rebind_ordinal: Option<u128>,
) -> Result<AdmittedWorkLocation, RegistryError> {
    let (owner, ordinal) = match decision {
        AdmissionDecision::Admitted {
            owner,
            ordinal,
            producer_turn_ordinal: None,
        } => (owner, ordinal),
        AdmissionDecision::Retry { owner, ordinal, .. }
            if recovery_rebind_ordinal == Some(ordinal) =>
        {
            (owner, ordinal)
        }
        AdmissionDecision::Admitted {
            producer_turn_ordinal: Some(_),
            ..
        }
        | AdmissionDecision::Retry { .. }
        | AdmissionDecision::ReplayTerminal { .. }
        | AdmissionDecision::StutterTerminal { .. }
        | AdmissionDecision::NonCandidate
        | AdmissionDecision::WaitForCapacity(_)
        | AdmissionDecision::Rejected(_)
        | AdmissionDecision::FailClosed(_) => {
            return Err(RegistryError::InvalidAdmissionShape);
        }
    };
    let record = coordinator
        .records
        .get(&ordinal)
        .ok_or(RegistryError::InvalidAdmissionShape)?;
    if record.owner != owner || record.ordinal != ordinal || record.physical_slots.len() != 1 {
        return Err(RegistryError::InvalidAdmissionShape);
    }
    let (&slot, &digest) = record
        .physical_slots
        .first_key_value()
        .expect("one-slot admission was checked above");
    let address =
        ConcreteWorkAddress::new(owner, ordinal, slot).ok_or(RegistryError::InvalidAddress)?;
    Ok(AdmittedWorkLocation { address, digest })
}
#[cfg(test)]
mod tests {
    use super::super::{
        OwnerId, PhysicalSlotId,
        work_registry::{
            PreparedLocalBodyValidateReplayPreAdmission,
            PreparedRemoteProposalFetchReplayPreAdmission,
        },
    };
    use super::*;
    use crate::sumeragi::{
        v2::AdapterEquivocationEvidence,
        v2_body_store::V2BodyStore,
        v2_core::{EventTag, Generation},
        v2_runtime::{
            LocalProposalEffectOwnership, RuntimeEffectOwnership,
            bind_adapter_effect_batch_ownership,
        },
    };
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature, SignatureOf};
    use iroha_data_model::{
        NetworkId,
        block::{BlockHeader, BlockSignature, SignedBlock, consensus_v2 as wire},
        peer::PeerId,
    };
    use std::{cell::Cell, num::NonZeroU64};
    use tempfile::TempDir;
    struct Fixture {
        verified: VerifiedHeightContext,
        context: wire::HeightContext,
        round: wire::ConsensusRound,
        tag: EventTag,
        keys: Vec<KeyPair>,
    }
    #[derive(Clone, Copy)]
    enum DurableValidateOriginFixture {
        LocalBody,
        RemoteProposal,
    }
    impl Fixture {
        fn new() -> Self {
            let mut keys = (1_u8..=4)
                .map(|seed| {
                    KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                        .expect("deterministic concrete-admission BLS key")
                })
                .collect::<Vec<_>>();
            keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
            let proofs = keys
                .iter()
                .map(|key| {
                    iroha_crypto::bls_normal_pop_prove(key.private_key())
                        .expect("concrete-admission proof of possession")
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
                network_id: crate::sumeragi::synthetic_network_id(
                    "sumeragi-v2-concrete-admission-test",
                ),
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
                nexus_amx_context_hash: Hash::new(b"concrete admission nexus context"),
                execution_policy_hash: Hash::new(b"concrete admission execution policy"),
                da_layout: wire::DataAvailabilityLayout {
                    encoding: wire::PayloadEncoding::ReedSolomon16,
                    chunk_size_bytes: 1024,
                    data_shards: 1,
                    parity_shards: 1,
                    max_payload_size_bytes: 512 * 1024,
                    max_chunk_count: 1024,
                },
                leader_seed: [0xA7; 32],
            };
            let verified = VerifiedHeightContext::genesis(context.clone(), proofs)
                .expect("verified concrete-admission context");
            let round = wire::ConsensusRound {
                context_id: context.id(),
                height: context.height,
                view: 2,
            };
            Self {
                verified,
                context,
                round,
                tag: EventTag::new(1, 2, Generation::new(1)),
                keys,
            }
        }
        fn active_context(&self) -> super::super::LifecycleContext {
            let mut digest = [0_u8; 32];
            digest.copy_from_slice(self.context.id().0.as_ref());
            super::super::LifecycleContext::new(LifecycleDigest::new(digest), self.context.height)
        }
        fn coordinator(&self, consensus_capacity: usize) -> LifecycleCoordinator {
            LifecycleCoordinator::new(
                self.active_context(),
                0,
                super::super::schema::CapacityGeometry::new([
                    (super::super::CapacityClass::Consensus, consensus_capacity),
                    (super::super::CapacityClass::Effect, 64),
                    (super::super::CapacityClass::Serve, 64),
                    (super::super::CapacityClass::Producer, 64),
                ]),
            )
        }
        fn effect(&self, marker: u8) -> AdapterEffect {
            let subject = wire::BlockSubject {
                parent_block_hash: (self.context.height > 1)
                    .then(|| HashOf::from_untyped_unchecked(Hash::new([marker, 0]))),
                block_hash: HashOf::from_untyped_unchecked(Hash::new([marker, 1])),
                payload_hash: Hash::new([marker, 2]),
            };
            let commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
                Hash::new([marker, 3]),
                Hash::new([marker, 4]),
                Hash::new([marker, 5]),
                1,
                Hash::new([marker, 6]),
            );
            AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::Vote(wire::Vote {
                    round: self.round,
                    proposal_round: self.round,
                    phase: wire::GlobalPhase::Prepare,
                    subject,
                    execution_commitment: commitment,
                    signer: 0,
                    signature: vec![marker],
                }),
            ))
        }
        fn timeout_effect(&self, marker: u8) -> AdapterEffect {
            AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::TimeoutVote(wire::TimeoutVote {
                    round: self.round,
                    highest_prepare_qc: None,
                    signer: 0,
                    signature: vec![marker],
                }),
            ))
        }
        fn pair(
            &self,
            effect: AdapterEffect,
            source_ordinal: u128,
        ) -> (AdapterEffect, PendingRuntimeEffectBinding) {
            let ownership = bind_adapter_effect_batch_ownership(
                core::slice::from_ref(&effect),
                vec![RuntimeEffectOwnership::fresh_for_test(
                    self.tag,
                    source_ordinal,
                )],
            )
            .expect("bind exact concrete-admission fixture")
            .pop()
            .expect("one concrete-admission owner");
            let pending = ownership
                .exact_pending_adapter_effect_binding(&effect)
                .expect("mint pending concrete-admission binding");
            (effect, pending)
        }
        fn output_pending(
            &self,
            effect: AdapterEffect,
            source_ordinal: u128,
        ) -> PendingLifecycleOutputAdmissionV1 {
            let ownership = bind_adapter_effect_batch_ownership(
                core::slice::from_ref(&effect),
                vec![RuntimeEffectOwnership::fresh_for_test(
                    self.tag,
                    source_ordinal,
                )],
            )
            .expect("bind exact lifecycle output fixture")
            .pop()
            .expect("one lifecycle output owner");
            PendingLifecycleOutputAdmissionV1::seal_exact(effect, ownership)
                .unwrap_or_else(|_| panic!("seal exact lifecycle output fixture"))
        }
        #[allow(clippy::too_many_lines)]
        fn pending_durable_validate(
            &self,
            marker: u8,
            origin: DurableValidateOriginFixture,
        ) -> (PendingDurableValidateAdmissionV1, AdapterEffect) {
            let leader = self.context.leader(self.round.view);
            let leader_index = usize::try_from(leader).expect("fixture leader fits usize");
            let header = BlockHeader::new(
                NonZeroU64::new(self.context.height).expect("fixture height is non-zero"),
                None,
                None,
                None,
                5_000 + u64::from(marker),
                self.round.view,
            );
            let signature =
                SignatureOf::try_from_hash(self.keys[leader_index].private_key(), header.hash())
                    .expect("sign durable Validate fixture block");
            let block = SignedBlock::presigned(
                BlockSignature::new(u64::from(leader), signature),
                header,
                Vec::new(),
            );
            let body = block
                .encode_wire()
                .expect("encode durable Validate fixture block");
            let subject = wire::BlockSubject {
                parent_block_hash: None,
                block_hash: block.hash(),
                payload_hash: Hash::new(&body),
            };
            let chunks = wire::encode_payload_chunks(self.context.da_layout, &body)
                .expect("encode durable Validate fixture chunks");
            let manifest = wire::PayloadManifest::derive(
                &self.context,
                self.round,
                subject,
                u64::try_from(body.len()).expect("fixture body length fits u64"),
                &chunks,
            )
            .expect("derive durable Validate fixture manifest");
            let body_directory = TempDir::new().expect("temporary durable Validate body store");
            let mut body_store = V2BodyStore::open(body_directory.path(), self.context.clone())
                .expect("open durable Validate body store");
            let durable_receipt = body_store
                .store(manifest.clone(), body)
                .expect("fsync durable Validate fixture body");
            let store_effect = AdapterEffect::StoreBody {
                tag: self.tag,
                round: self.round,
                subject,
            };
            let validate_effect = AdapterEffect::ValidateBody {
                tag: self.tag,
                round: self.round,
                subject,
            };
            let source_ordinal = u128::from(marker);
            let pending = match origin {
                DurableValidateOriginFixture::LocalBody => {
                    let store_ownership = bind_adapter_effect_batch_ownership(
                        core::slice::from_ref(&store_effect),
                        vec![RuntimeEffectOwnership::fresh_for_test(
                            self.tag,
                            source_ordinal,
                        )],
                    )
                    .expect("bind local Store owner")
                    .pop()
                    .expect("one local Store owner");
                    let local = LocalProposalEffectOwnership::for_test(
                        store_ownership,
                        &store_effect,
                        &manifest,
                    )
                    .expect("seal local Store replay");
                    let validate_ownership = local
                        .exact_store_task_ownership(&store_effect, &manifest)
                        .expect("project local Store scheduling owner")
                        .rebind_as_inherited_adapter_effect(&validate_effect)
                        .expect("project local Validate owner");
                    let replay = local
                        .project_exact_validate(
                            &store_effect,
                            &manifest,
                            &durable_receipt,
                            &validate_effect,
                            &validate_ownership,
                        )
                        .unwrap_or_else(|_| panic!("project local Validate replay"));
                    PreparedLocalBodyValidateReplayPreAdmission::seal_exact_validate(
                        validate_effect.clone(),
                        validate_ownership,
                        durable_receipt,
                        replay,
                    )
                    .unwrap_or_else(|_| panic!("seal local Validate pre-admission"))
                    .into_pending_durable_validate_admission()
                }
                DurableValidateOriginFixture::RemoteProposal => {
                    let mut proposal = wire::Proposal {
                        round: self.round,
                        proposer: leader,
                        subject,
                        manifest: manifest.clone(),
                        justification: wire::ProposalJustification::ParentCommit(
                            wire::ParentCommitJustification { certificate: None },
                        ),
                        signature: Vec::new(),
                    };
                    proposal.signature = Signature::new(
                        self.keys[leader_index].private_key(),
                        &proposal.signature_preimage(),
                    )
                    .payload()
                    .to_vec();
                    let fetch_effect = AdapterEffect::FetchBody {
                        tag: self.tag,
                        round: self.round,
                        subject,
                        manifest: Some(manifest),
                        certified_sources: Vec::new(),
                        certificate: None,
                    };
                    let mut fetch_ownership = bind_adapter_effect_batch_ownership(
                        core::slice::from_ref(&fetch_effect),
                        vec![RuntimeEffectOwnership::fresh_for_test(
                            self.tag,
                            source_ordinal,
                        )],
                    )
                    .expect("bind remote Fetch owner")
                    .pop()
                    .expect("one remote Fetch owner");
                    let store_ownership = fetch_ownership
                        .rebind_as_inherited_adapter_effect(&store_effect)
                        .expect("project remote Store owner");
                    let validate_ownership = store_ownership
                        .rebind_as_inherited_adapter_effect(&validate_effect)
                        .expect("project remote Validate owner");
                    assert!(
                        fetch_ownership.bind_authenticated_remote_proposal_replay_for_test(
                            proposal,
                            &fetch_effect,
                        )
                    );
                    PreparedRemoteProposalFetchReplayPreAdmission::seal_exact_fetch(
                        fetch_effect,
                        fetch_ownership,
                    )
                    .unwrap_or_else(|_| panic!("seal remote Fetch pre-admission"))
                    .project_store(store_effect, store_ownership)
                    .unwrap_or_else(|_| panic!("project remote Store pre-admission"))
                    .bind_durable_body(durable_receipt)
                    .unwrap_or_else(|_| panic!("bind remote durable body"))
                    .project_validate(validate_effect.clone(), validate_ownership)
                    .unwrap_or_else(|_| panic!("project remote Validate pre-admission"))
                    .into_pending_durable_validate_admission()
                }
            };
            (pending, validate_effect)
        }
        fn production_owner(&self, effect_capacity: usize) -> ProductionLifecycleOwnerV1 {
            let payload_directory =
                TempDir::new().expect("temporary durable Validate payload store");
            let (payload_store, serve_payloads) = crate::sumeragi::v2_certified_serve_payload_store::CertifiedServePayloadStoreV1::open_lifecycle_fixture_for_test(
                payload_directory.path(),
                &self.context,
            )
            .expect("open empty durable Validate payload owner");
            let coordinator = LifecycleCoordinator::new(
                self.active_context(),
                0,
                super::super::schema::CapacityGeometry::new([
                    (super::super::CapacityClass::Consensus, 64),
                    (super::super::CapacityClass::Effect, effect_capacity),
                    (super::super::CapacityClass::Serve, 64),
                    (super::super::CapacityClass::Producer, 64),
                ]),
            );
            ProductionLifecycleOwnerV1 {
                verified: self.verified.clone(),
                coordinator,
                registry: LifecycleWorkRegistryHolder::empty(),
                recovered_lifecycle_outputs: None,
                payload_store,
                serve_payloads,
                body_store: None,
                body_store_identity: None,
                kura_binding: None,
                apply_service: None,
                adapter_startup: None,
                timeout_supersession_successor: None,
            }
        }
        fn admit(
            &self,
            coordinator: &mut LifecycleCoordinator,
            registry: &mut LifecycleWorkRegistryHolder,
            effect: AdapterEffect,
            pending: PendingRuntimeEffectBinding,
        ) -> AdapterEffectAdmissionTransaction {
            let prepared = coordinator
                .prepare_direct_signed_lifecycle_admission(&self.verified, effect, pending)
                .expect("fixture effect has one mandatory direct-signed replay origin");
            coordinator.admit_prepared_lifecycle(registry, prepared)
        }
    }
    fn consensus_slot() -> PhysicalSlotId {
        PhysicalSlotId::for_capacity(super::super::CapacityClass::Consensus, 0)
    }
    fn recovery_snapshot(coordinator: &LifecycleCoordinator) -> super::super::RecoverySnapshot {
        super::super::RecoverySnapshot {
            context: coordinator.active_context,
            high_water: coordinator.high_water,
            records: coordinator
                .records
                .values()
                .map(|record| super::super::RecoveredLifecycleRecord {
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
    #[test]
    fn owner_settlement_admits_both_durable_validate_origins() {
        let fixture = Fixture::new();
        for (origin, remote_proposal) in [
            (DurableValidateOriginFixture::LocalBody, false),
            (DurableValidateOriginFixture::RemoteProposal, true),
        ] {
            let mut owner = fixture.production_owner(64);
            let (pending, effect) = fixture.pending_durable_validate(0xB1, origin);
            assert!(pending.exactly_retains_for_test(&effect, remote_proposal));
            let ProductionDurableValidateAdmissionSettlementV1::Admitted(
                AdmissionDecision::Admitted { ordinal: 1, .. },
            ) = owner.settle_durable_validate_admission(pending)
            else {
                panic!("exact durable Validate origin must commit one fresh admission")
            };
            assert_eq!(owner.coordinator.high_water(), 1);
            assert_eq!(owner.coordinator.records.len(), 1);
            assert_eq!(owner.registry.registry.len(), 1);
            assert_eq!(
                owner.coordinator.records[&1].work_class,
                LifecycleWorkClass::Validate
            );
            assert_eq!(owner.coordinator.records[&1].state, LifecycleState::Ready);
        }
    }
    #[test]
    fn owner_settlement_rebinds_a_recovered_validate_at_the_same_ordinal() {
        let fixture = Fixture::new();
        let mut live = fixture.production_owner(64);
        let (pending, _) =
            fixture.pending_durable_validate(0xB2, DurableValidateOriginFixture::LocalBody);
        assert!(matches!(
            live.settle_durable_validate_admission(pending),
            ProductionDurableValidateAdmissionSettlementV1::Admitted(AdmissionDecision::Admitted {
                ordinal: 1,
                ..
            })
        ));
        let snapshot = recovery_snapshot(&live.coordinator);
        let mut recovered = fixture.production_owner(64);
        recovered.coordinator.high_water = snapshot.high_water;
        recovered.coordinator.reconcile_restart(snapshot);
        assert_eq!(recovered.coordinator.high_water(), 1);
        assert!(matches!(
            recovered.coordinator.records[&1].state,
            LifecycleState::Waiting(WaitToken {
                source: WaitSource::Recovery(_),
                ..
            })
        ));
        let (pending, _) =
            fixture.pending_durable_validate(0xB2, DurableValidateOriginFixture::LocalBody);
        assert!(matches!(
            recovered.settle_durable_validate_admission(pending),
            ProductionDurableValidateAdmissionSettlementV1::Rebound(AdmissionDecision::Retry {
                ordinal: 1,
                ..
            })
        ));
        assert_eq!(recovered.coordinator.high_water(), 1);
        assert_eq!(recovered.coordinator.records.len(), 1);
        assert_eq!(recovered.registry.registry.len(), 1);
        assert_eq!(
            recovered.coordinator.records[&1].state,
            LifecycleState::Ready
        );
    }
    #[test]
    fn owner_settlement_returns_the_exact_retry_without_reminting_an_ordinal() {
        let fixture = Fixture::new();
        let mut owner = fixture.production_owner(64);
        let (pending, _) =
            fixture.pending_durable_validate(0xB3, DurableValidateOriginFixture::RemoteProposal);
        assert!(matches!(
            owner.settle_durable_validate_admission(pending),
            ProductionDurableValidateAdmissionSettlementV1::Admitted(AdmissionDecision::Admitted {
                ordinal: 1,
                ..
            })
        ));
        let (pending, effect) =
            fixture.pending_durable_validate(0xB3, DurableValidateOriginFixture::RemoteProposal);
        let ProductionDurableValidateAdmissionSettlementV1::Returned {
            decision: first_decision @ AdmissionDecision::Retry { ordinal: 1, .. },
            pending,
        } = owner.settle_durable_validate_admission(pending)
        else {
            panic!("an exact live duplicate must return its remote-Proposal owner")
        };
        assert!(pending.exactly_retains_for_test(&effect, true));
        assert_eq!(owner.coordinator.high_water(), 1);
        assert_eq!(owner.coordinator.records.len(), 1);
        assert_eq!(owner.registry.registry.len(), 1);
        let ProductionDurableValidateAdmissionSettlementV1::Returned {
            decision: second_decision,
            pending,
        } = owner.settle_durable_validate_admission(pending)
        else {
            panic!("retrying the exact returned owner must remain non-mutating")
        };
        assert_eq!(second_decision, first_decision);
        assert!(pending.exactly_retains_for_test(&effect, true));
        assert_eq!(owner.coordinator.high_water(), 1);
        assert_eq!(owner.coordinator.records.len(), 1);
        assert_eq!(owner.registry.registry.len(), 1);
    }
    fn run_lifecycle_output_test_on_stack(body: impl FnOnce() + Send + 'static) {
        let handle = std::thread::Builder::new()
            .name("lifecycle-output-settlement".to_owned())
            .stack_size(32 * 1024 * 1024)
            .spawn(body)
            .expect("spawn lifecycle output settlement test");
        if let Err(payload) = handle.join() {
            std::panic::resume_unwind(payload);
        }
    }

    #[test]
    fn pre_release_taira_direct_output_compatibility_is_height_five_only() {
        const RESET11_NETWORK_ID: [u8; Hash::LENGTH] = [
            0x1e, 0x88, 0x19, 0xab, 0x7b, 0x55, 0xa4, 0xe7, 0xe4, 0x1e, 0xa3, 0xeb, 0x8e, 0x42,
            0xae, 0xe6, 0x6d, 0x77, 0xcc, 0x07, 0x46, 0x1b, 0xa3, 0xb7, 0x01, 0x81, 0x42, 0x84,
            0x25, 0x80, 0x92, 0x31,
        ];
        let reset11 = NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
            Hash::prehashed(RESET11_NETWORK_ID),
        ));
        let other = crate::sumeragi::synthetic_network_id("non-reset11-direct-output");

        assert!(uses_pre_release_taira_direct_output_compatibility(
            &reset11, 5
        ));
        assert!(!uses_pre_release_taira_direct_output_compatibility(
            &reset11, 6
        ));
        assert!(!uses_pre_release_taira_direct_output_compatibility(
            &other, 5
        ));
    }

    #[test]
    fn pre_release_direct_output_settlement_uses_service_without_registry_row() {
        run_lifecycle_output_test_on_stack(|| {
            let fixture = Fixture::new();
            let owner = fixture.production_owner(64);
            let effect = fixture.effect(0xCF);
            let (prepared, execution) = fixture
                .output_pending(effect.clone(), 0xCF)
                .prepare_direct_signed(fixture.active_context(), &fixture.verified)
                .expect("prepare exact direct lifecycle output");
            let called = Cell::new(0_u8);
            assert!(matches!(
                settle_pre_release_direct_output(prepared, execution, |observed, _ownership| {
                    assert_eq!(observed, &effect);
                    called.set(called.get().saturating_add(1));
                    Ok::<LifecycleOutputServiceDispositionV1, &'static str>(
                        LifecycleOutputServiceDispositionV1::Accepted,
                    )
                }),
                ProductionLifecycleOutputAdmissionSettlementV1::Completed
            ));
            assert_eq!(called.get(), 1);
            assert_eq!(owner.coordinator.high_water(), 0);
            assert!(owner.coordinator.records.is_empty());
            assert!(owner.registry.registry().is_empty());
        });
    }

    #[test]
    fn lifecycle_output_settlement_executes_once_and_terminalizes_the_same_row() {
        run_lifecycle_output_test_on_stack(|| {
            let fixture = Fixture::new();
            let mut owner = fixture.production_owner(64);
            let ledger = TempDir::new().expect("temporary lifecycle output ledger");
            owner
                .coordinator
                .attach_empty_test_ledger(ledger.path())
                .expect("attach lifecycle output ledger");
            let effect = fixture.effect(0xD1);
            let called = Cell::new(0_u8);
            assert!(matches!(
                owner.settle_lifecycle_output_admission(
                    fixture.output_pending(effect.clone(), 0xD1),
                    |observed, _ownership| {
                        assert_eq!(observed, &effect);
                        called.set(called.get().saturating_add(1));
                        Ok::<LifecycleOutputServiceDispositionV1, &'static str>(
                            LifecycleOutputServiceDispositionV1::Accepted,
                        )
                    },
                ),
                ProductionLifecycleOutputAdmissionSettlementV1::Completed
            ));
            assert_eq!(called.get(), 1);
            assert_eq!(owner.coordinator.high_water(), 1);
            assert_eq!(
                owner.coordinator.records[&1].state,
                LifecycleState::Terminal(super::super::TerminalOutcome::Advanced)
            );
            assert!(owner.registry.registry().is_empty());
            assert!(matches!(
                owner.settle_lifecycle_output_admission(
                    fixture.output_pending(effect, 0xD1),
                    |_effect, _ownership| -> Result<
                        LifecycleOutputServiceDispositionV1,
                        &'static str,
                    > {
                        panic!("terminal output duplicate must not repeat service I/O")
                    },
                ),
                ProductionLifecycleOutputAdmissionSettlementV1::AlreadyCompleted
            ));
            assert_eq!(owner.coordinator.high_water(), 1);
        });
    }

    #[test]
    fn lifecycle_output_terminal_duplicate_with_new_runtime_root_stutters_exactly() {
        run_lifecycle_output_test_on_stack(|| {
            let fixture = Fixture::new();
            let mut owner = fixture.production_owner(64);
            let ledger = TempDir::new().expect("temporary terminal duplicate ledger");
            owner
                .coordinator
                .attach_empty_test_ledger(ledger.path())
                .expect("attach terminal duplicate ledger");
            // H6 `InstallTimeout` recovery retransmits terminal timeout output,
            // not a producer-gated block Vote.
            let effect = fixture.timeout_effect(0xD6);
            let (prepared, initial_execution) = fixture
                .output_pending(effect.clone(), 0xD6)
                .prepare_direct_signed(owner.coordinator.active_context(), &owner.verified)
                .expect("prepare exact terminal TimeoutVote fixture");
            let candidate = prepared.candidate().clone();
            let AdmissionDecision::Admitted {
                ordinal: terminal_ordinal,
                producer_turn_ordinal: None,
                ..
            } = owner
                .coordinator
                .admit(AdmissionRequest::Candidate(candidate))
            else {
                panic!("terminal TimeoutVote fixture must admit one direct row")
            };
            owner
                .coordinator
                .finish_terminal(terminal_ordinal, super::super::TerminalOutcome::Advanced)
                .expect("terminalize the already-serviced timeout output");
            owner
                .coordinator
                .persist_durable_projection()
                .expect("publish the terminal timeout fixture");
            drop((prepared, initial_execution));

            let rebound_tag = EventTag::new(
                fixture.tag.height(),
                fixture.tag.view(),
                Generation::new(fixture.tag.generation().get().saturating_add(1)),
            );
            let rebound_ownership = bind_adapter_effect_batch_ownership(
                core::slice::from_ref(&effect),
                vec![RuntimeEffectOwnership::fresh_for_test(rebound_tag, 0xD7)],
            )
            .expect("bind byte-identical output under a fresh runtime root")
            .pop()
            .expect("one rebound output owner");
            let rebound_pending = rebound_ownership
                .exact_pending_adapter_effect_binding(&effect)
                .expect("derive the rebound output's exact pending binding");
            let rebound =
                PendingLifecycleOutputAdmissionV1::seal_exact(effect.clone(), rebound_ownership)
                    .unwrap_or_else(|_| panic!("seal byte-identical terminal output retry"));
            let record = &owner.coordinator.records[&terminal_ordinal];
            let (&slot, &digest) = record
                .physical_slots
                .first_key_value()
                .expect("terminal output retains one physical slot");
            assert_eq!(record.physical_slots.len(), 1);
            let address = ConcreteWorkAddress::new(record.owner, record.ordinal, slot)
                .expect("terminal output retains one exact address");
            let replay_authority = owner.coordinator.durable_records[&terminal_ordinal]
                .replay_authority
                .clone();
            let work = ConcreteLifecycleWork::from_candidate_for_test(
                effect.clone(),
                rebound_pending,
                replay_authority,
            )
            .unwrap_or_else(|(error, _, _)| {
                panic!("construct exact terminal-address carrier: {error:?}")
            });
            owner
                .registry
                .registry_for_test_mut()
                .install(address, digest, work)
                .unwrap_or_else(|(error, _)| {
                    panic!("install exact terminal-address carrier: {error:?}")
                });
            let rebound_execution = rebound.into_existing_execution();
            assert!(matches!(
                owner
                    .registry
                    .join_lifecycle_output(&owner.coordinator, &rebound_execution),
                Ok(LifecycleOutputRegistryJoinV1::TerminalInstalledDuplicate(retirement))
                    if retirement.ordinal() == terminal_ordinal
            ));
            let rebound = rebound_execution.into_pending();
            assert!(matches!(
                owner.settle_lifecycle_output_admission(
                    rebound,
                    |_effect, _ownership| -> Result<
                        LifecycleOutputServiceDispositionV1,
                        &'static str,
                    > {
                        panic!("exact terminal duplicate must not repeat service I/O")
                    },
                ),
                ProductionLifecycleOutputAdmissionSettlementV1::AlreadyCompleted
            ));
            assert_eq!(owner.coordinator.high_water(), terminal_ordinal);
            assert!(owner.registry.registry().is_empty());

            let mut drifted = effect;
            let AdapterEffect::Broadcast(message) = &mut drifted else {
                unreachable!("fixture output is one signed Broadcast")
            };
            let wire::ConsensusMessageV2Payload::TimeoutVote(vote) = &mut message.payload else {
                unreachable!("fixture output is one TimeoutVote")
            };
            vote.signature.push(0xFF);
            let drifted_ownership = bind_adapter_effect_batch_ownership(
                core::slice::from_ref(&drifted),
                vec![RuntimeEffectOwnership::fresh_for_test(rebound_tag, 0xD8)],
            )
            .expect("bind semantically drifted output")
            .pop()
            .expect("one drifted output owner");
            let drifted = PendingLifecycleOutputAdmissionV1::seal_exact(drifted, drifted_ownership)
                .unwrap_or_else(|_| panic!("seal drifted terminal output retry"));
            assert!(matches!(
                owner.settle_lifecycle_output_admission(
                    drifted,
                    |_effect, _ownership| -> Result<
                        LifecycleOutputServiceDispositionV1,
                        &'static str,
                    > {
                        panic!("semantic drift must fail before service I/O")
                    },
                ),
                ProductionLifecycleOutputAdmissionSettlementV1::Failed {
                    failure: ProductionLifecycleOutputAdmissionFailureV1::Registry(_),
                    ..
                }
            ));
            assert_eq!(owner.coordinator.high_water(), terminal_ordinal);
        });
    }

    #[test]
    fn lifecycle_output_settlement_defers_behind_an_older_ready_row() {
        run_lifecycle_output_test_on_stack(|| {
            let fixture = Fixture::new();
            let mut owner = fixture.production_owner(64);
            let ledger = TempDir::new().expect("temporary ordered output ledger");
            owner
                .coordinator
                .attach_empty_test_ledger(ledger.path())
                .expect("attach ordered output ledger");
            let blocker = fixture.effect(0xD2);
            let (blocker, blocker_pending) = fixture.pair(blocker, 0xD2);
            assert!(matches!(
                fixture.admit(
                    &mut owner.coordinator,
                    &mut owner.registry,
                    blocker,
                    blocker_pending,
                ),
                AdapterEffectAdmissionTransaction::Admitted(AdmissionDecision::Admitted {
                    ordinal: 1,
                    ..
                })
            ));
            let effect = fixture.effect(0xD3);
            assert!(matches!(
                owner.settle_lifecycle_output_admission(
                    fixture.output_pending(effect, 0xD3),
                    |_effect, _ownership| -> Result<
                        LifecycleOutputServiceDispositionV1,
                        &'static str,
                    > {
                        panic!("a later output must not overtake the first Ready row")
                    },
                ),
                ProductionLifecycleOutputAdmissionSettlementV1::Deferred(_)
            ));
            assert_eq!(owner.coordinator.high_water(), 2);
            assert_eq!(owner.registry.registry().len(), 2);
            assert_eq!(owner.coordinator.ready_index.first().copied(), Some(1));
        });
    }

    #[test]
    fn lifecycle_output_settlement_service_failure_retains_the_same_ready_owner() {
        run_lifecycle_output_test_on_stack(|| {
            let fixture = Fixture::new();
            let mut owner = fixture.production_owner(64);
            let ledger = TempDir::new().expect("temporary failed-output ledger");
            owner
                .coordinator
                .attach_empty_test_ledger(ledger.path())
                .expect("attach failed-output ledger");
            let effect = fixture.effect(0xD4);
            let ProductionLifecycleOutputAdmissionSettlementV1::Failed {
                failure: ProductionLifecycleOutputAdmissionFailureV1::Service("offline"),
                pending,
            } = owner.settle_lifecycle_output_admission(
                fixture.output_pending(effect.clone(), 0xD4),
                |_effect, _ownership| Err("offline"),
            )
            else {
                panic!("service failure must return the exact output owner")
            };
            assert_eq!(owner.coordinator.high_water(), 1);
            assert_eq!(owner.coordinator.records[&1].state, LifecycleState::Ready);
            assert_eq!(owner.registry.registry().len(), 1);
            let calls = Cell::new(0_u8);
            assert!(matches!(
                owner.settle_lifecycle_output_admission(pending, |observed, _ownership| {
                    assert_eq!(observed, &effect);
                    calls.set(calls.get().saturating_add(1));
                    Ok::<LifecycleOutputServiceDispositionV1, &'static str>(
                        LifecycleOutputServiceDispositionV1::Accepted,
                    )
                }),
                ProductionLifecycleOutputAdmissionSettlementV1::Completed
            ));
            assert_eq!(calls.get(), 1);
            assert_eq!(owner.coordinator.high_water(), 1);
            assert!(owner.registry.registry().is_empty());
        });
    }

    #[test]
    fn lifecycle_output_malformed_equivocation_fails_before_service_or_admission() {
        run_lifecycle_output_test_on_stack(|| {
            let fixture = Fixture::new();
            let mut owner = fixture.production_owner(64);
            let AdapterEffect::Broadcast(message) = fixture.effect(0xD8) else {
                unreachable!("fixture output is one Vote broadcast")
            };
            let wire::ConsensusMessageV2Payload::Vote(vote) = message.payload else {
                unreachable!("fixture output contains one Vote")
            };
            let effect = AdapterEffect::ReportEquivocation {
                evidence: AdapterEquivocationEvidence::vote_for_test(vote.clone(), vote),
            };
            let AdapterEffect::ReportEquivocation { evidence } = &effect else {
                unreachable!("fixture retains vote equivocation evidence")
            };
            assert!(evidence.validate_structure(&fixture.context).is_err());
            let calls = Cell::new(0_u8);
            let settlement = owner.settle_lifecycle_output_admission(
                fixture.output_pending(effect, 0xD8),
                |_effect, _ownership| {
                    calls.set(calls.get().saturating_add(1));
                    Ok::<LifecycleOutputServiceDispositionV1, ()>(
                        LifecycleOutputServiceDispositionV1::Accepted,
                    )
                },
            );
            match settlement {
                ProductionLifecycleOutputAdmissionSettlementV1::Failed {
                    failure:
                        ProductionLifecycleOutputAdmissionFailureV1::Projection(
                            AdapterEffectAdmissionError::UnboundEffect,
                        ),
                    pending: _,
                } => {}
                other => {
                    panic!("a non-conflicting pair must fail before output service: {other:?}")
                }
            };
            assert_eq!(calls.get(), 0);
            assert_eq!(owner.coordinator.high_water(), 0);
            assert!(owner.coordinator.records.is_empty());
            assert!(owner.registry.registry().is_empty());
        });
    }

    #[test]
    fn lifecycle_output_source_retained_retries_before_terminal_fsync() {
        run_lifecycle_output_test_on_stack(|| {
            let fixture = Fixture::new();
            let mut owner = fixture.production_owner(64);
            let ledger = TempDir::new().expect("temporary retained-output ledger");
            owner
                .coordinator
                .attach_empty_test_ledger(ledger.path())
                .expect("attach retained-output ledger");
            let effect = fixture.effect(0xD9);
            let calls = Cell::new(0_u8);
            let ProductionLifecycleOutputAdmissionSettlementV1::Deferred(pending) = owner
                .settle_lifecycle_output_admission(
                    fixture.output_pending(effect.clone(), 0xD9),
                    |observed, _ownership| {
                        assert_eq!(observed, &effect);
                        calls.set(calls.get().saturating_add(1));
                        Ok::<LifecycleOutputServiceDispositionV1, &'static str>(
                            LifecycleOutputServiceDispositionV1::SourceRetained,
                        )
                    },
                )
            else {
                panic!("source retention must return the exact Ready output owner")
            };
            assert_eq!(calls.get(), 1);
            assert_eq!(owner.coordinator.high_water(), 1);
            assert_eq!(owner.coordinator.records[&1].state, LifecycleState::Ready);
            assert_eq!(owner.registry.registry().len(), 1);
            assert!(matches!(
                owner.settle_lifecycle_output_admission(pending, |observed, _ownership| {
                    assert_eq!(observed, &effect);
                    calls.set(calls.get().saturating_add(1));
                    Ok::<LifecycleOutputServiceDispositionV1, &'static str>(
                        LifecycleOutputServiceDispositionV1::Accepted,
                    )
                }),
                ProductionLifecycleOutputAdmissionSettlementV1::Completed
            ));
            assert_eq!(calls.get(), 2);
            assert_eq!(
                owner.coordinator.records[&1].state,
                LifecycleState::Terminal(super::super::TerminalOutcome::Advanced)
            );
            assert!(owner.registry.registry().is_empty());
        });
    }

    #[test]
    fn lifecycle_output_settlement_terminal_fsync_failure_faults_closed() {
        run_lifecycle_output_test_on_stack(|| {
            let fixture = Fixture::new();
            let root = TempDir::new().expect("temporary output durability ledger");
            let mut owner = fixture.production_owner(64);
            owner
                .coordinator
                .attach_empty_test_ledger(root.path())
                .expect("attach output durability ledger");
            let effect = fixture.effect(0xD5);
            let ProductionLifecycleOutputAdmissionSettlementV1::Failed {
                failure: ProductionLifecycleOutputAdmissionFailureV1::Service("park"),
                pending,
            } = owner.settle_lifecycle_output_admission(
                fixture.output_pending(effect, 0xD5),
                |_effect, _ownership| Err("park"),
            )
            else {
                panic!("pre-service retry fixture must retain its admitted owner")
            };
            owner
                .coordinator
                .redirect_test_ledger_to_missing_parent(root.path());
            let calls = Cell::new(0_u8);
            let ProductionLifecycleOutputAdmissionSettlementV1::Failed {
                failure: ProductionLifecycleOutputAdmissionFailureV1::Durability,
                pending: _,
            } = owner.settle_lifecycle_output_admission(pending, |_effect, _ownership| {
                calls.set(calls.get().saturating_add(1));
                Ok::<LifecycleOutputServiceDispositionV1, &'static str>(
                    LifecycleOutputServiceDispositionV1::Accepted,
                )
            })
            else {
                panic!("terminal LedgerV1 failure must enter restart-required state")
            };
            assert_eq!(calls.get(), 1);
            assert_eq!(owner.coordinator.high_water(), 1);
            assert_eq!(owner.coordinator.records[&1].state, LifecycleState::Ready);
            assert_eq!(owner.registry.registry().len(), 1);
            assert_eq!(
                owner.coordinator.fault(),
                Some(CoordinatorFault::DurabilityFailure)
            );
        });
    }

    #[test]
    fn owner_settlement_projection_failure_returns_the_local_origin() {
        let fixture = Fixture::new();
        let mut owner = fixture.production_owner(64);
        owner.coordinator.active_context = super::super::LifecycleContext::new(
            LifecycleDigest::new([0xF1; 32]),
            fixture.context.height,
        );
        let (pending, effect) =
            fixture.pending_durable_validate(0xB4, DurableValidateOriginFixture::LocalBody);
        let ProductionDurableValidateAdmissionSettlementV1::Failed {
            failure:
                ProductionDurableValidateAdmissionFailureV1::Projection(
                    AdapterEffectAdmissionError::ForeignContext,
                ),
            pending,
        } = owner.settle_durable_validate_admission(pending)
        else {
            panic!("foreign owner context must return the exact local-body owner")
        };
        assert!(pending.exactly_retains_for_test(&effect, false));
        assert_eq!(owner.coordinator.high_water(), 0);
        assert!(owner.coordinator.records.is_empty());
        assert!(owner.registry.registry.is_empty());
    }
    #[test]
    fn owner_settlement_durability_failure_returns_owner_and_faults_closed() {
        let fixture = Fixture::new();
        let root = TempDir::new().expect("temporary durable Validate admission ledger");
        let mut owner = fixture.production_owner(64);
        owner
            .coordinator
            .attach_empty_test_ledger(root.path())
            .expect("attach empty durable Validate admission ledger");
        owner
            .coordinator
            .redirect_test_ledger_to_missing_parent(root.path());
        let (pending, effect) =
            fixture.pending_durable_validate(0xB5, DurableValidateOriginFixture::LocalBody);
        let ProductionDurableValidateAdmissionSettlementV1::Failed {
            failure: ProductionDurableValidateAdmissionFailureV1::Durability,
            pending,
        } = owner.settle_durable_validate_admission(pending)
        else {
            panic!("failed LedgerV1 publication must return the exact local owner")
        };
        assert!(pending.exactly_retains_for_test(&effect, false));
        assert_eq!(owner.coordinator.high_water(), 0);
        assert!(owner.coordinator.records.is_empty());
        assert!(owner.registry.registry.is_empty());
        assert_eq!(
            owner.coordinator.fault(),
            Some(CoordinatorFault::DurabilityFailure)
        );
    }
    #[test]
    fn occupied_address_returns_pair_and_leaves_coordinator_unchanged() {
        let fixture = Fixture::new();
        let effect = fixture.effect(1);
        let (incumbent_effect, incumbent_pending) = fixture.pair(effect.clone(), 90);
        let incumbent = ConcreteLifecycleWork::from_direct_signed_fixture_for_test(
            incumbent_effect,
            incumbent_pending,
        )
        .expect("construct exact incumbent");
        let digest = incumbent.digest();
        let owner = OwnerId::new(incumbent.causal_root(), 1);
        let address = ConcreteWorkAddress::new(owner, 1, consensus_slot())
            .expect("valid prospective address");
        let mut registry: super::super::LifecycleWorkRegistryHolder =
            LifecycleWorkRegistryHolder::empty();
        registry
            .registry
            .install(address, digest, incumbent)
            .expect("install simulated incumbent");
        let mut coordinator = fixture.coordinator(64);
        let records_before = coordinator.records.clone();
        let owners_before = coordinator.owner_index.clone();
        let capacity_before = coordinator.capacity_used.clone();
        let (effect, pending) = fixture.pair(effect.clone(), 91);
        let outcome = fixture.admit(&mut coordinator, &mut registry, effect.clone(), pending);
        let AdapterEffectAdmissionTransaction::Failed {
            failure: AdapterEffectAdmissionFailure::Registry(RegistryError::Occupied),
            prepared,
        } = outcome
        else {
            panic!("occupied exact address must return the incoming pair")
        };
        assert_eq!(coordinator.high_water(), 0);
        assert_eq!(coordinator.records, records_before);
        assert_eq!(coordinator.owner_index, owners_before);
        assert_eq!(coordinator.capacity_used, capacity_before);
        assert!(registry.registry.exactly_contains(address, &effect));
        assert!(prepared.exactly_binds_for_test(&effect));
    }
    #[test]
    fn capacity_wait_returns_the_same_pair_for_each_exact_retry() {
        let fixture = Fixture::new();
        let mut coordinator = fixture.coordinator(1);
        let mut registry = LifecycleWorkRegistryHolder::empty();
        let first = fixture.effect(2);
        let (effect, pending) = fixture.pair(first, 92);
        assert!(matches!(
            fixture.admit(&mut coordinator, &mut registry, effect, pending),
            AdapterEffectAdmissionTransaction::Admitted(AdmissionDecision::Admitted {
                ordinal: 1,
                ..
            })
        ));
        let waiting_effect = fixture.effect(3);
        let (effect, pending) = fixture.pair(waiting_effect.clone(), 93);
        let outcome = fixture.admit(&mut coordinator, &mut registry, effect, pending);
        let AdapterEffectAdmissionTransaction::Returned {
            decision: AdmissionDecision::WaitForCapacity(first_wait),
            prepared,
        } = outcome
        else {
            panic!("full exact capacity must return the waiting pair")
        };
        assert!(prepared.exactly_binds_for_test(&waiting_effect));
        assert_eq!(coordinator.high_water(), 1);
        assert_eq!(coordinator.admission_waits.len(), 1);
        let outcome = coordinator.admit_prepared_lifecycle(&mut registry, prepared);
        let AdapterEffectAdmissionTransaction::Returned {
            decision: AdmissionDecision::WaitForCapacity(second_wait),
            prepared,
        } = outcome
        else {
            panic!("unchanged generation must return the exact retry pair")
        };
        assert_eq!(second_wait, first_wait);
        assert!(prepared.exactly_binds_for_test(&waiting_effect));
        assert_eq!(coordinator.high_water(), 1);
        assert_eq!(coordinator.admission_waits.len(), 1);
        assert_eq!(registry.registry.len(), 1);
    }
    #[test]
    fn exhaustive_live_registry_census_rejects_volatile_drift_and_one_missing_carrier() {
        run_lifecycle_output_test_on_stack(|| {
            let fixture = Fixture::new();
            let mut coordinator = fixture.coordinator(64);
            let mut registry = LifecycleWorkRegistryHolder::empty();
            for (marker, source_ordinal) in [(0x31, 91), (0x32, 92)] {
                let (effect, pending) = fixture.pair(fixture.effect(marker), source_ordinal);
                assert!(matches!(
                    fixture.admit(&mut coordinator, &mut registry, effect, pending),
                    AdapterEffectAdmissionTransaction::Admitted(AdmissionDecision::Admitted { .. })
                ));
            }
            assert!(
                registry
                    .registry
                    .exactly_covers_all_live_work(&fixture.verified, &coordinator)
            );

            coordinator.ready_index.remove(&1);
            coordinator
                .records
                .get_mut(&1)
                .expect("first live row")
                .state = LifecycleState::Waiting(WaitToken::new(
                WaitSource::Capacity(super::super::CapacityClass::Consensus),
                0,
            ));
            assert!(
                !registry
                    .registry
                    .exactly_covers_all_live_work(&fixture.verified, &coordinator)
            );
            coordinator
                .records
                .get_mut(&1)
                .expect("first live row")
                .state = LifecycleState::Ready;
            coordinator.ready_index.insert(1);

            let recovery_source = WaitSource::Recovery(LifecycleDigest::new([0x33; 32]));
            coordinator.ready_index.remove(&1);
            coordinator
                .records
                .get_mut(&1)
                .expect("first live row")
                .state = LifecycleState::Waiting(WaitToken::new(recovery_source, 1));
            assert!(
                !registry
                    .registry
                    .exactly_covers_all_live_work(&fixture.verified, &coordinator)
            );
            coordinator.observed_generation.insert(recovery_source, 1);
            assert!(
                registry
                    .registry
                    .exactly_covers_all_live_work(&fixture.verified, &coordinator)
            );
            coordinator
                .records
                .get_mut(&1)
                .expect("first live row")
                .state = LifecycleState::Waiting(WaitToken::new(recovery_source, u64::MAX));
            coordinator
                .observed_generation
                .insert(recovery_source, u64::MAX);
            assert!(
                !registry
                    .registry
                    .exactly_covers_all_live_work(&fixture.verified, &coordinator)
            );
            coordinator.observed_generation.remove(&recovery_source);
            coordinator
                .records
                .get_mut(&1)
                .expect("first live row")
                .state = LifecycleState::Ready;
            coordinator.ready_index.insert(1);

            let removed_generation = coordinator
                .capacity_generation
                .remove(&super::super::CapacityClass::Producer)
                .expect("complete capacity generations");
            assert!(
                !registry
                    .registry
                    .exactly_covers_all_live_work(&fixture.verified, &coordinator)
            );
            coordinator
                .capacity_generation
                .insert(super::super::CapacityClass::Producer, removed_generation);

            coordinator
                .records
                .get_mut(&1)
                .expect("first live row")
                .episode
                .frozen_predecessors
                .insert(1);
            assert!(
                !registry
                    .registry
                    .exactly_covers_all_live_work(&fixture.verified, &coordinator)
            );
            coordinator
                .records
                .get_mut(&1)
                .expect("first live row")
                .episode
                .frozen_predecessors
                .clear();
            assert!(
                registry
                    .registry
                    .exactly_covers_all_live_work(&fixture.verified, &coordinator)
            );

            let record = &coordinator.records[&1];
            let (&slot, _) = record
                .physical_slots
                .first_key_value()
                .expect("admitted concrete work retains one physical slot");
            let address = ConcreteWorkAddress::new(record.owner, record.ordinal, slot)
                .expect("admitted concrete work retains a valid address");
            assert!(registry.registry.remove_exact_for_test(address));
            assert!(
                !registry
                    .registry
                    .exactly_covers_all_live_work(&fixture.verified, &coordinator)
            );
        });
    }

    #[test]
    fn admitted_location_rejects_causal_owner_and_digest_mismatch() {
        let fixture = Fixture::new();
        let effect = fixture.effect(4);
        let (effect, pending) = fixture.pair(effect, 94);
        let work = ConcreteLifecycleWork::from_direct_signed_fixture_for_test(effect, pending)
            .expect("exact work");
        let wrong_owner = OwnerId::new(
            super::super::CausalRoot::new(LifecycleDigest::new([0xF4; 32])),
            1,
        );
        let wrong_owner_address =
            ConcreteWorkAddress::new(wrong_owner, 1, consensus_slot()).expect("valid address");
        let published = Cell::new(false);
        let mut registry = LifecycleWorkRegistryHolder::empty();
        let error = registry
            .registry
            .install_before_publication(wrong_owner_address, work.digest(), work, || {
                published.set(true);
                Ok::<_, ()>(())
            })
            .expect_err("foreign causal owner must fail before publication");
        let RegistryPublicationError::Install(RegistryError::CausalOwnerMismatch, work) = error
        else {
            panic!("causal mismatch must return the exact work")
        };
        assert!(!published.get());
        let (effect, pending) = work.into_pair();
        assert!(pending.exactly_binds_adapter_effect(&effect));
        assert!(registry.registry.is_empty());
        let work = ConcreteLifecycleWork::from_direct_signed_fixture_for_test(effect, pending)
            .expect("returned exact work");
        let owner = OwnerId::new(work.causal_root(), 1);
        let address =
            ConcreteWorkAddress::new(owner, 1, consensus_slot()).expect("valid exact address");
        let error = registry
            .registry
            .install_before_publication(address, LifecycleDigest::new([0xD4; 32]), work, || {
                published.set(true);
                Ok::<_, ()>(())
            })
            .expect_err("foreign digest must fail before publication");
        let RegistryPublicationError::Install(RegistryError::DigestMismatch, work) = error else {
            panic!("digest mismatch must return the exact work")
        };
        assert!(!published.get());
        assert!(work.validate_exact());
        assert!(registry.registry.is_empty());
    }
    #[test]
    fn retry_and_terminal_decisions_never_replace_incumbent_work() {
        let fixture = Fixture::new();
        let original = fixture.effect(5);
        let mut coordinator = fixture.coordinator(64);
        let mut registry = LifecycleWorkRegistryHolder::empty();
        let (effect, pending) = fixture.pair(original.clone(), 95);
        assert!(matches!(
            fixture.admit(&mut coordinator, &mut registry, effect, pending),
            AdapterEffectAdmissionTransaction::Admitted(AdmissionDecision::Admitted {
                ordinal: 1,
                ..
            })
        ));
        let record = coordinator.records.get(&1).expect("first admitted record");
        let address = ConcreteWorkAddress::new(record.owner, 1, consensus_slot())
            .expect("exact incumbent address");
        assert!(registry.registry.exactly_contains(address, &original));
        let (effect, pending) = fixture.pair(original.clone(), 96);
        let outcome = fixture.admit(&mut coordinator, &mut registry, effect, pending);
        let AdapterEffectAdmissionTransaction::Returned {
            decision: AdmissionDecision::Retry { ordinal: 1, .. },
            prepared,
        } = outcome
        else {
            panic!("live duplicate must return a retry pair")
        };
        assert!(prepared.exactly_binds_for_test(&original));
        assert!(registry.registry.exactly_contains(address, &original));
        assert_eq!(registry.registry.len(), 1);
        coordinator
            .records
            .get_mut(&1)
            .expect("first admitted record")
            .state =
            super::super::LifecycleState::Terminal(super::super::TerminalOutcome::Cancelled);
        let (effect, pending) = fixture.pair(original.clone(), 97);
        let outcome = fixture.admit(&mut coordinator, &mut registry, effect, pending);
        let AdapterEffectAdmissionTransaction::Returned {
            decision: AdmissionDecision::StutterTerminal { .. },
            prepared,
        } = outcome
        else {
            panic!("terminal duplicate must return its pair")
        };
        assert!(prepared.exactly_binds_for_test(&original));
        assert!(registry.registry.exactly_contains(address, &original));
        assert_eq!(registry.registry.len(), 1);
        assert_eq!(coordinator.high_water(), 1);
    }
    #[test]
    fn recovered_retry_installs_exact_work_without_allocating() {
        let fixture = Fixture::new();
        let original = fixture.effect(8);
        let mut live = fixture.coordinator(64);
        let mut live_registry = LifecycleWorkRegistryHolder::empty();
        let (effect, pending) = fixture.pair(original.clone(), 100);
        let AdapterEffectAdmissionTransaction::Admitted(
            decision @ AdmissionDecision::Admitted { ordinal, .. },
        ) = fixture.admit(&mut live, &mut live_registry, effect, pending)
        else {
            panic!("fixture must admit one concrete effect")
        };
        assert_eq!(ordinal, 1);
        let snapshot = recovery_snapshot(&live);
        let mut recovered = LifecycleCoordinator::new(
            fixture.active_context(),
            live.high_water,
            super::super::schema::CapacityGeometry::new([
                (super::super::CapacityClass::Consensus, 64),
                (super::super::CapacityClass::Effect, 64),
                (super::super::CapacityClass::Serve, 64),
                (super::super::CapacityClass::Producer, 64),
            ]),
        );
        recovered.reconcile_restart(snapshot);
        assert!(matches!(
            recovered.records[&ordinal].state,
            LifecycleState::Waiting(super::super::WaitToken {
                source: WaitSource::Recovery(_),
                ..
            })
        ));
        assert!(recovered.records[&ordinal].physical_slots.is_empty());
        let mut registry = LifecycleWorkRegistryHolder::empty();
        let (effect, pending) = fixture.pair(original.clone(), 101);
        let outcome = fixture.admit(&mut recovered, &mut registry, effect, pending);
        let AdapterEffectAdmissionTransaction::Rebound(
            retry @ AdmissionDecision::Retry {
                ordinal: rebound_ordinal,
                ..
            },
        ) = outcome
        else {
            panic!("exact recovery retry must atomically install concrete work")
        };
        assert_eq!(rebound_ordinal, ordinal);
        assert_eq!(recovered.high_water, 1);
        assert_eq!(recovered.records[&ordinal].state, LifecycleState::Ready);
        let location = concrete_work_location(&recovered, retry, Some(ordinal))
            .expect("rebound work has one exact location");
        assert!(
            registry
                .registry
                .exactly_contains(location.address, &original)
        );
        assert_eq!(registry.registry.len(), 1);
        assert!(matches!(decision, AdmissionDecision::Admitted { .. }));
    }
    #[test]
    fn recovered_retry_publication_failure_rolls_back_work_and_ready_transition() {
        let fixture = Fixture::new();
        let original = fixture.effect(9);
        let mut live = fixture.coordinator(64);
        let mut live_registry = LifecycleWorkRegistryHolder::empty();
        let (effect, pending) = fixture.pair(original.clone(), 102);
        assert!(matches!(
            fixture.admit(&mut live, &mut live_registry, effect, pending),
            AdapterEffectAdmissionTransaction::Admitted(AdmissionDecision::Admitted {
                ordinal: 1,
                ..
            })
        ));
        let snapshot = recovery_snapshot(&live);
        let mut recovered = LifecycleCoordinator::new(
            fixture.active_context(),
            live.high_water,
            super::super::schema::CapacityGeometry::new([
                (super::super::CapacityClass::Consensus, 64),
                (super::super::CapacityClass::Effect, 64),
                (super::super::CapacityClass::Serve, 64),
                (super::super::CapacityClass::Producer, 64),
            ]),
        );
        recovered.reconcile_restart(snapshot);
        let root = TempDir::new().expect("temporary recovered-rebind ledger");
        recovered
            .attach_empty_test_ledger(root.path())
            .expect("persist recovered coordinator before rebind");
        recovered.redirect_test_ledger_to_missing_parent(root.path());
        let mut registry = LifecycleWorkRegistryHolder::empty();
        let (effect, pending) = fixture.pair(original.clone(), 103);
        let outcome = fixture.admit(&mut recovered, &mut registry, effect, pending);
        let AdapterEffectAdmissionTransaction::Failed {
            failure: AdapterEffectAdmissionFailure::Durability,
            prepared,
        } = outcome
        else {
            panic!("failed recovered rebind publication must return exact work")
        };
        assert!(prepared.exactly_binds_for_test(&original));
        assert!(registry.registry.is_empty());
        assert_eq!(recovered.high_water, 1);
        assert!(matches!(
            recovered.records[&1].state,
            LifecycleState::Waiting(super::super::WaitToken {
                source: WaitSource::Recovery(_),
                ..
            })
        ));
        assert!(recovered.records[&1].physical_slots.is_empty());
        assert_eq!(recovered.fault(), Some(CoordinatorFault::DurabilityFailure));
    }
    #[test]
    fn durable_publication_failure_rolls_back_registry_and_logical_admission() {
        let fixture = Fixture::new();
        let root = TempDir::new().expect("temporary concrete-admission ledger");
        let mut coordinator = fixture.coordinator(64);
        coordinator
            .attach_empty_test_ledger(root.path())
            .expect("attach empty lifecycle ledger");
        coordinator.redirect_test_ledger_to_missing_parent(root.path());
        let mut registry = LifecycleWorkRegistryHolder::empty();
        let original = fixture.effect(6);
        let (effect, pending) = fixture.pair(original.clone(), 98);
        let outcome = fixture.admit(&mut coordinator, &mut registry, effect, pending);
        let AdapterEffectAdmissionTransaction::Failed {
            failure: AdapterEffectAdmissionFailure::Durability,
            prepared,
        } = outcome
        else {
            panic!("failed durable publication must return the rolled-back pair")
        };
        assert!(prepared.exactly_binds_for_test(&original));
        assert_eq!(coordinator.high_water(), 0);
        assert!(coordinator.records.is_empty());
        assert!(coordinator.key_index.is_empty());
        assert!(coordinator.owner_index.is_empty());
        assert!(registry.registry.is_empty());
        assert_eq!(
            coordinator.fault(),
            Some(CoordinatorFault::DurabilityFailure)
        );
    }
    #[test]
    fn mandatory_binding_rejects_an_effect_owned_by_another_pending_slot() {
        let fixture = Fixture::new();
        let coordinator = fixture.coordinator(64);
        let actual_owner = fixture.effect(0xA1);
        let (_, pending) = fixture.pair(actual_owner.clone(), 0xA1);
        let submitted = fixture.effect(0xA2);
        let Err(PreparedLifecycleAdmissionErrorV1::Binding(error)) = coordinator
            .prepare_direct_signed_lifecycle_admission(
                &fixture.verified,
                submitted.clone(),
                pending,
            )
        else {
            panic!("a foreign pending slot must not mint prepared admission")
        };
        assert!(error.returns_foreign_owner_for_test(&submitted, &actual_owner));
        assert_eq!(coordinator.high_water(), 0);
        assert!(coordinator.records.is_empty());
    }
    #[test]
    fn prepared_admission_rejects_the_exact_effect_under_a_foreign_replay_origin() {
        let fixture = Fixture::new();
        let mut coordinator = fixture.coordinator(64);
        let mut registry = LifecycleWorkRegistryHolder::empty();
        let effect = fixture.effect(0xA6);
        let (_, pending) = fixture.pair(effect.clone(), 0xA6);
        let mut prepared = coordinator
            .prepare_direct_signed_lifecycle_admission(&fixture.verified, effect.clone(), pending)
            .expect("exact signed effect prepares one mandatory owner");
        assert!(prepared.replace_with_foreign_origin_for_test());
        let AdapterEffectAdmissionTransaction::Failed {
            failure: AdapterEffectAdmissionFailure::Registry(RegistryError::CorruptWork),
            prepared,
        } = coordinator.admit_prepared_lifecycle(&mut registry, prepared)
        else {
            panic!("foreign replay origin must fail before logical or concrete admission")
        };
        assert!(prepared.has_foreign_origin_for_test(&effect));
        assert_eq!(coordinator.high_water(), 0);
        assert!(coordinator.records.is_empty());
        assert!(registry.registry.is_empty());
    }
    #[test]
    fn bound_registry_transaction_rejects_a_foreign_candidate_owner_before_publication() {
        let fixture = Fixture::new();
        let coordinator = fixture.coordinator(64);
        let effect = fixture.effect(0xA3);
        let (_, pending) = fixture.pair(effect.clone(), 0xA3);
        let prepared = coordinator
            .prepare_direct_signed_lifecycle_admission(&fixture.verified, effect.clone(), pending)
            .expect("exact signed effect prepares one mandatory owner");
        let mut foreign_candidate = prepared.candidate().clone();
        foreign_candidate.causal_root =
            super::super::CausalRoot::new(LifecycleDigest::new([0xA4; 32]));
        let foreign_owner = OwnerId::new(foreign_candidate.causal_root, 1);
        let address = ConcreteWorkAddress::new(foreign_owner, 1, consensus_slot())
            .expect("foreign owner still has a structurally valid address");
        let published = Cell::new(false);
        let mut registry = LifecycleWorkRegistryHolder::empty();
        let PreparedLifecycleAdmissionOwnerV1::DirectSigned(bound) = prepared.into_owner() else {
            panic!("direct signed preparation must retain its bound owner")
        };
        let error = registry
            .registry
            .install_bound_before_publication(
                fixture.active_context(),
                &foreign_candidate,
                address,
                LifecycleDigest::new([0xA5; 32]),
                bound,
                || {
                    published.set(true);
                    Ok::<_, ()>(())
                },
            )
            .expect_err("foreign candidate owner must fail before registry publication");
        let BoundAdapterRegistryPublicationErrorV1::Install(RegistryError::CorruptWork, returned) =
            error
        else {
            panic!("foreign owner must return the complete bound adapter effect")
        };
        assert!(!published.get());
        assert!(returned.exactly_binds_for_test(&effect));
        assert!(registry.registry.is_empty());
    }
    #[test]
    fn prepared_admission_and_bound_effect_have_no_optional_or_clone_surface() {
        let source = [
            include_str!("v2_lifecycle_work_registry.rs"),
            include_str!("v2_lifecycle_work_registry_pre_admission.rs"),
            include_str!("v2_lifecycle_work_registry_live_validate_children.rs"),
        ]
        .concat();
        let bound = source
            .split_once("pub(super) struct BoundAdapterEffectV1 {")
            .expect("bound adapter effect has one declaration")
            .1
            .split_once('}')
            .expect("bound declaration is bounded")
            .0;
        for required in [
            "effect: AdapterEffect",
            "pending: PendingRuntimeEffectBinding",
            "replay_origin: BoundAdapterReplayOriginV1",
        ] {
            assert!(bound.contains(required), "bound owner omitted {required}");
        }
        assert!(!bound.contains("Option<"));
        let prepared = source
            .split_once("pub(in crate::sumeragi) struct PreparedLifecycleAdmissionV1 {")
            .expect("prepared lifecycle admission has one declaration")
            .1
            .split_once('}')
            .expect("prepared declaration is bounded")
            .0;
        assert!(prepared.contains("owner: PreparedLifecycleAdmissionOwnerV1"));
        assert!(prepared.contains("candidate: CandidateAdmission"));
        assert!(!prepared.contains("Option<"));
        for origin in [
            "LiveWal(BoundAdapterEffectV1)",
            "LocalBody(PreparedLocalBodyValidateReplayPreAdmission)",
            "RemoteProposal(PreparedRemoteProposalValidateReplayPreAdmission)",
            "InvalidBodyReport(BoundAdapterEffectV1)",
            "DirectSigned(BoundAdapterEffectV1)",
        ] {
            assert!(source.contains(origin), "prepared owner omitted {origin}");
        }
        for declaration in [
            "pub(in crate::sumeragi) struct PreparedLiveValidateReportRegistryWork {",
            "pub(in crate::sumeragi) struct PreparedLiveValidateApplyRegistryWork {",
            "pub(in crate::sumeragi) struct PreparedLiveValidateSignRegistryWork {",
        ] {
            let body = source
                .split_once(declaration)
                .expect("live publication wrapper has one declaration")
                .1
                .split_once('}')
                .expect("live publication wrapper declaration is bounded")
                .0;
            assert!(body.contains("admission: PreparedLifecycleAdmissionV1"));
            assert!(!body.contains("bound: BoundAdapterEffectV1"));
            assert!(!body.contains("work: ConcreteLifecycleWork"));
        }
        for declaration in [
            "pub(super) struct BoundAdapterEffectV1",
            "pub(in crate::sumeragi) struct PreparedLifecycleAdmissionV1",
        ] {
            let prefix = source
                .split_once(declaration)
                .expect("move-only declaration exists")
                .0;
            let attributes = prefix.rsplit_once("///").map_or(prefix, |(_, tail)| tail);
            assert!(
                !attributes.contains("derive(Clone"),
                "{declaration} must remain move-only"
            );
        }
    }
    #[test]
    fn projection_failure_returns_the_mandatory_bound_owner() {
        let fixture = Fixture::new();
        let foreign_context = super::super::LifecycleContext::new(
            LifecycleDigest::new([0xFF; 32]),
            fixture.context.height,
        );
        let coordinator = LifecycleCoordinator::new(
            foreign_context,
            0,
            super::super::schema::CapacityGeometry::new(
                super::super::CapacityClass::ALL.map(|class| (class, 64)),
            ),
        );
        let original = fixture.effect(7);
        let (effect, pending) = fixture.pair(original.clone(), 99);
        let Err(PreparedLifecycleAdmissionErrorV1::Projection {
            failure: AdapterEffectAdmissionError::ForeignContext,
            bound,
        }) = coordinator.prepare_direct_signed_lifecycle_admission(
            &fixture.verified,
            effect,
            pending,
        )
        else {
            panic!("foreign projection must return the mandatory bound owner")
        };
        assert!(bound.exactly_binds_for_test(&original));
        assert_eq!(coordinator.high_water(), 0);
        assert!(coordinator.records.is_empty());
    }
}
