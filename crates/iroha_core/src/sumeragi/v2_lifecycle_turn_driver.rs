//! Unified lifecycle ownership for one outer Completion or Ingress turn.

use super::*;
use crate::sumeragi::v2::ReadyDurableValidateAdapterPublicationKind;
use crate::sumeragi::v2_lifecycle_coordinator::{
    AdmissionDecision, CertifiedServeSchedulerObservationV1, LifecycleCoordinator,
    LifecycleLedgerV1, LifecycleState, LifecycleWorkClass, PhysicalSlotId, TurnLease, TurnPlan,
    claim_certified_serve_turn_v1,
};
#[cfg(test)]
pub(in crate::sumeragi) use crate::sumeragi::v2_runner::ordinary_ingress_consumer::ProductionPreparedCertifiedServeTestSettlementV1;
#[cfg(test)]
use crate::sumeragi::v2_runner::ordinary_ingress_consumer::settle_prepared_certified_serve_for_test;
use crate::sumeragi::v2_runner::ordinary_ingress_consumer::{
    CurrentCertifiedServePreAdmissionV1, PreparedDequeuedV2IngressV1,
    ProductionPreparedCertifiedServeV1, ProductionPreparedOrdinaryIngressConsumptionV1,
    consume_prepared_dequeued_v2_ingress, prepare_current_certified_serve_pre_admission,
};

/// Closed result of servicing one recovered Sign completion.
///
/// Each variant names the sole settlement method selected from one
/// publication-inert adapter preview. Callers cannot recover the guarded
/// completion or select a second settler.
#[must_use = "the selected recovered Sign settlement must be observed"]
pub(in crate::sumeragi) enum ProductionRecoveredLifecycleSignCompletionSelectionV1 {
    /// The guarded signature produced exactly one Broadcast.
    Broadcast(ProductionRecoveredLifecycleSignBroadcastSettlementV1),
    /// A Proposal first required its exact Prepare-intent WAL append.
    ProposalPrepareWal(ProductionRecoveredLifecycleProposalBroadcastAndSignSettlementV1),
    /// A Prepare Vote produced its Broadcast and adjacent Commit Sign.
    VoteBroadcastAndSign(ProductionRecoveredLifecycleVoteBroadcastAndSignSettlementV1),
    /// A WAL-ahead Proposal produced its Broadcast and adjacent Prepare Sign.
    ProposalBroadcastAndSign(ProductionRecoveredLifecycleProposalBroadcastAndSignSettlementV1),
    /// The parked completion could not be classified without changing owner.
    RestartRequired,
}

impl ProductionRecoveredLifecycleSignCompletionSelectionV1 {
    /// Return whether the selected settlement lost authority or crossed publication.
    fn restart_required(&self) -> bool {
        match self {
            Self::Broadcast(settlement) => matches!(
                settlement,
                ProductionRecoveredLifecycleSignBroadcastSettlementV1::None
                    | ProductionRecoveredLifecycleSignBroadcastSettlementV1::RestartRequired
            ),
            Self::ProposalPrepareWal(settlement)
            | Self::ProposalBroadcastAndSign(settlement) => matches!(
                settlement,
                ProductionRecoveredLifecycleProposalBroadcastAndSignSettlementV1::None
                    | ProductionRecoveredLifecycleProposalBroadcastAndSignSettlementV1::RestartRequired
            ),
            Self::VoteBroadcastAndSign(settlement) => matches!(
                settlement,
                ProductionRecoveredLifecycleVoteBroadcastAndSignSettlementV1::None
                    | ProductionRecoveredLifecycleVoteBroadcastAndSignSettlementV1::RestartRequired
            ),
            Self::RestartRequired => true,
        }
    }
}

/// Closed diagnostic for one lifecycle-selected Completion turn.
///
/// Guarded completions, deferred Apply state, and capacity waits remain inside
/// the launched owner. This enum contains no request, result, selector, queue,
/// executor, service, or acknowledgement authority.
#[allow(variant_size_differences)]
#[must_use = "the lifecycle-selected Completion result must be observed"]
pub(in crate::sumeragi) enum ProductionLifecycleCompletionSelectionV1 {
    /// The exact missing-sidecar Apply owner remains parked for another turn.
    RecoveredDecisionApplyDeferred,
    /// The unchanged deferred Apply command re-entered its dedicated FIFO.
    RecoveredDecisionApplyRequeued,
    /// Deferred Apply ownership changed and process restart is required.
    RecoveredDecisionApplyRestartRequired,
    /// One recovered Apply worker result was durably settled.
    RecoveredDecisionApplyApplied,
    /// One recovered Apply worker result became the retained sidecar owner.
    RecoveredDecisionApplyCompletionDeferred,
    /// Recovered Apply settlement requires cold restart.
    RecoveredDecisionApplyCompletionRestartRequired,
    /// One parked recovered Sign used exactly one successor-family settler.
    RecoveredLifecycleSignCompletion(ProductionRecoveredLifecycleSignCompletionSelectionV1),
    /// One complete recovered Apply/Sign/Fetch census used a joint physical cut.
    RecoveredIoDispatch(
        Result<
            ProductionRecoveredCompletionDispatchV1,
            ProductionRecoveredCompletionDispatchErrorV1,
        >,
    ),
    /// One parked recovered Decision Fetch body entered its Store settlement.
    RecoveredDecisionFetchCompletion(ProductionRecoveredDecisionFetchStoreSettlementV1),
<<<<<<< HEAD
    /// One ordinary certified-Fetch persistence completed its exact Phase-B cut.
    CertifiedFetchBodyCompleted,
    /// One ordinary certified-Fetch completion retained its complete pre-ledger owner.
    CertifiedFetchBodyRetry,
    /// One ordinary durable Validate completion published its same-address Ready carrier.
    DurableValidateCompletionPublished,
    /// The exact durable Validate dispatch re-entered its dedicated FIFO.
    DurableValidateRetryQueued,
    /// Durable Validate retry is backpressured under its unchanged keyed owner.
    DurableValidateRetryPending,
    /// The ordinary pipeline rolled back its claim because worker capacity was unavailable.
    CertifiedBodyPipelineCapacityPending,
    /// One ordinary body-pipeline parent published its adjacent durable successor.
    CertifiedBodyPipelineAdvanced,
    /// One lifecycle-owned Serve reached LedgerV1, reply delivery, and worker acknowledgement.
    CertifiedServeCompleted,
=======
    /// Ordinary certified-Fetch Phase B published its durable Ready carrier.
    CertifiedFetchBodyPersisted,
    /// Ordinary certified-Fetch Phase B retained the complete pre-ledger owner.
    CertifiedFetchBodyPersistenceRetry,
    /// Ordinary certified-Fetch Phase B crossed the durability boundary and must restart.
    CertifiedFetchBodyPersistenceRestartRequired,
    /// One claimed Serve reached LedgerV1, reply delivery, and released its live lease.
    CertifiedServeClaimedCompleted,
    /// One terminal Serve replay was verified, delivered, and acknowledged without a live lease.
    CertifiedServeReplayCompleted,
>>>>>>> origin/optimizations
    /// One durable recovered Broadcast entered its typed refanout transaction.
    RecoveredLifecycleBroadcastRefanout(
        Result<
            ProductionRecoveredLifecycleSignedBroadcastRefanoutV1,
            ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1,
        >,
    ),
    /// An exact lifecycle completion was selected but its service owner changed.
    RestartRequired,
}

impl ProductionLifecycleCompletionSelectionV1 {
    /// Return whether the consumed Completion turn requires a cold restart.
    pub(in crate::sumeragi) fn restart_required(&self) -> bool {
        match self {
            Self::RecoveredDecisionApplyRestartRequired
            | Self::RecoveredDecisionApplyCompletionRestartRequired
            | Self::CertifiedFetchBodyPersistenceRestartRequired
            | Self::RestartRequired => true,
            Self::RecoveredLifecycleSignCompletion(selection) => selection.restart_required(),
            Self::RecoveredIoDispatch(result) => result.is_err(),
            Self::RecoveredDecisionFetchCompletion(settlement) => matches!(
                settlement,
                ProductionRecoveredDecisionFetchStoreSettlementV1::None
                    | ProductionRecoveredDecisionFetchStoreSettlementV1::RestartRequired
            ),
            Self::RecoveredLifecycleBroadcastRefanout(result) => matches!(
                result,
                Err(_) | Ok(ProductionRecoveredLifecycleSignedBroadcastRefanoutV1::RestartRequired)
            ),
            Self::RecoveredDecisionApplyDeferred
            | Self::RecoveredDecisionApplyRequeued
            | Self::RecoveredDecisionApplyApplied
            | Self::RecoveredDecisionApplyCompletionDeferred
<<<<<<< HEAD
            | Self::CertifiedFetchBodyCompleted
            | Self::CertifiedFetchBodyRetry
            | Self::DurableValidateCompletionPublished
            | Self::DurableValidateRetryQueued
            | Self::DurableValidateRetryPending
            | Self::CertifiedBodyPipelineCapacityPending
            | Self::CertifiedBodyPipelineAdvanced
            | Self::CertifiedServeCompleted => false,
=======
            | Self::CertifiedFetchBodyPersisted
            | Self::CertifiedFetchBodyPersistenceRetry
            | Self::CertifiedServeClaimedCompleted
            | Self::CertifiedServeReplayCompleted => false,
>>>>>>> origin/optimizations
        }
    }
}
/// Outcome of one borrow-bound outer Completion turn.
///
/// `PassThrough` returns the exact current cursor borrow. The cursor therefore
/// cannot advance before the ordinary completion owner runs. `Selected` keeps
/// no cursor authority; the consumed turn advances only after lifecycle work
/// was classified.
#[allow(variant_size_differences)]
#[must_use = "ordinary pass-through must retain the current Completion turn"]
pub(in crate::sumeragi) enum ProductionLifecycleCompletionTurnV1<'cursor> {
    /// No exact recovered lifecycle work owned this turn.
    PassThrough(LifecycleCurrentRunnerTurn<'cursor>),
    /// The launched lifecycle owner serviced the selected recovered work.
    Selected(ProductionLifecycleCompletionSelectionV1),
}

/// Opaque Completion cursor whose parked and physical lifecycle heads were empty.
///
/// Only the lifecycle Ready dispatcher may consume this cursor. Separating it
/// from physical-head classification prevents an existing lease from reaching
/// a second Ready-work claim.
#[must_use = "a physically empty Completion turn must be dispatched or returned"]
pub(in crate::sumeragi) struct ProductionLifecycleReadyCompletionTurnV1<'cursor> {
    runner: LifecycleCurrentRunnerTurn<'cursor>,
}

/// Result of classifying only parked and physical Completion owners.
///
/// `Ready` proves no physical lifecycle completion was available, but does not
/// itself authorize a new lifecycle claim.
#[allow(variant_size_differences)]
#[must_use = "the lifecycle Completion pre-gate result must be observed"]
pub(in crate::sumeragi) enum ProductionLifecycleCompletionPreGateV1<'cursor> {
    /// One parked or physical lifecycle completion was settled or retained.
    Selected(ProductionLifecycleCompletionSelectionV1),
    /// The physical head belongs to the ordinary one-item completion drain.
    Ordinary(LifecycleCurrentRunnerTurn<'cursor>),
    /// No parked or physical completion exists; Ready dispatch remains gated.
    Ready(ProductionLifecycleReadyCompletionTurnV1<'cursor>),
}

/// Closed diagnostic for one lifecycle-selected Ingress turn.
#[must_use = "the lifecycle-selected Ingress result must be observed"]
pub(in crate::sumeragi) enum ProductionLifecycleIngressSelectionV1 {
    /// An ordinary certified-Fetch selector waits on its exact I/O generation.
    CertifiedFetchCapacityPending,
    /// Ordinary certified-Fetch Phase A queued one durable body persistence command.
    CertifiedFetchQueued,
    /// Ordinary certified-Fetch Phase A retained the queue occurrence for retry.
    CertifiedFetchRetry,
    /// Existing Ready lifecycle work retained priority over ordinary Fetch Phase A.
    CertifiedFetchCompetingReady,
    /// An externally Waiting recovered Fetch retains its selector on the I/O generation.
    RecoveredDecisionFetchCapacityPending,
    /// A Serve request waits for capacity before any lifecycle lease is claimed.
    CertifiedServeCapacityPending,
    /// A Serve request stayed parked while an authenticated Ready Producer retained priority.
    CertifiedServeCompetingReady,
    /// Recovered Phase A woke and claimed its Fetch before queueing body persistence.
    RecoveredDecisionFetchQueued,
    /// Ordinary Phase A queued its exact certified-Fetch persistence command.
    CertifiedFetchQueued,
    /// One lifecycle-owned Serve entered the dedicated auxiliary worker.
    CertifiedServeQueued,
    /// A terminal Serve replay entered the worker without claiming a live lease.
    CertifiedServeReplayQueued,
    /// One exact replay or typed negative reached its lifecycle terminal.
    CertifiedServeTerminal,
    /// An externally Waiting recovered Fetch retained its response before command preparation.
    RecoveredDecisionFetchPreparationRetry,
    /// A recovered Fetch response stayed parked while direct Ready work retained priority.
    RecoveredDecisionFetchCompetingReady,
    /// Serve retained its pre-claim request cut for a later retry.
    CertifiedServeRetry,
    /// The selected recovered owner changed and process restart is required.
    RestartRequired,
}

/// Opaque move-only ordinary ingress handoff selected and removed by the queue.
///
/// The exact inbound carrier, ordinary dequeue disposition, and any stateful
/// Certified-Serve result cannot be separated at the runner-facing boundary.
/// The lifecycle height driver routes this token through the activated shared
/// runner consumer. Drop closes consensus admission so a prepared Serve
/// placeholder or staged negative can never be silently abandoned.
#[must_use = "the exact ordinary ingress handoff must be consumed by the runner"]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::sumeragi) struct ProductionPreparedOrdinaryIngressTurnV1 {
    #[cfg(test)]
    drop_order_probe: Option<PreparedOrdinaryIngressDropOrderProbe>,
    handoff: Option<PreparedDequeuedV2IngressV1>,
}

#[cfg(test)]
struct PreparedOrdinaryIngressDropOrderProbe {
    output_guard: std::sync::Arc<ConsensusOutputGuard>,
    observed: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

#[cfg(test)]
impl Drop for PreparedOrdinaryIngressDropOrderProbe {
    fn drop(&mut self) {
        assert!(
            self.output_guard.restart_required(),
            "ordinary token must close output before any retained field is released"
        );
        self.observed
            .store(true, std::sync::atomic::Ordering::Release);
    }
}

impl Drop for ProductionPreparedOrdinaryIngressTurnV1 {
    fn drop(&mut self) {
        if let Some(handoff) = self.handoff.as_ref() {
            handoff.close_output_for_restart();
        }
    }
}

impl ProductionPreparedOrdinaryIngressTurnV1 {
    /// Return the retained queue-minted physical ordinal for ownership tests.
    #[cfg(test)]
    pub(in crate::sumeragi) fn physical_ordinal_for_test(&self) -> u64 {
        self.handoff
            .as_ref()
            .expect("prepared ordinary turn retains its runner handoff")
            .physical_ordinal_for_test()
    }

    /// Report whether this test token retains a prepared Certified-Serve result.
    #[cfg(test)]
    pub(in crate::sumeragi) fn has_prepared_serve_for_test(&self) -> bool {
        self.handoff
            .as_ref()
            .is_some_and(PreparedDequeuedV2IngressV1::has_prepared_serve_for_test)
    }
}

fn prepared_ordinary_ingress_turn(
    receiver: std::sync::Arc<FairV2Ingress>,
    inbound: InboundBlockMessage,
    disposition: FairV2IngressDequeueDisposition,
    prepared_serve: Option<ProductionPreparedCertifiedServeV1>,
    terminal_subject: Option<iroha_data_model::block::consensus_v2::BlockSubject>,
    output_guard: std::sync::Arc<ConsensusOutputGuard>,
) -> ProductionPreparedOrdinaryIngressTurnV1 {
    ProductionPreparedOrdinaryIngressTurnV1 {
        #[cfg(test)]
        drop_order_probe: None,
        handoff: Some(PreparedDequeuedV2IngressV1::new(
            receiver,
            inbound,
            disposition,
            prepared_serve,
            terminal_subject,
            output_guard,
        )),
    }
}

/// Outcome of one borrow-bound outer Ingress turn.
///
/// `PassThrough` retains the real cursor only when no fair winner exists (or a
/// foreign runner turn was supplied). `Ordinary` owns the one exact row already
/// removed under the same queue service episode; no second selection is legal.
#[allow(variant_size_differences)]
#[must_use = "ordinary pass-through must retain the current Ingress turn"]
pub(in crate::sumeragi) enum ProductionLifecycleIngressTurnV1<'cursor> {
    /// No fair ingress winner exists, so the real runner cursor is unchanged.
    PassThrough(LifecycleCurrentRunnerTurn<'cursor>),
    /// The queue physically removed the exact ordinary fair winner once.
    Ordinary(ProductionPreparedOrdinaryIngressTurnV1),
    /// Recovered Decision Fetch work consumed this Ingress turn.
    Selected(ProductionLifecycleIngressSelectionV1),
}

fn lifecycle_context_for_ingress(
    context: &iroha_data_model::block::consensus_v2::HeightContext,
) -> super::super::schema::LifecycleContext {
    let mut digest = [0_u8; 32];
    digest.copy_from_slice(context.id().0.as_ref());
    super::super::schema::LifecycleContext::new(
        super::super::schema::LifecycleDigest::new(digest),
        context.height,
    )
}

fn selected_ingress_is_current_certified_serve(
    inbound: &InboundBlockMessage,
    active_height: iroha_data_model::block::consensus_v2::Height,
) -> bool {
    let crate::sumeragi::message::BlockMessage::V2(message) = inbound.message() else {
        return false;
    };
    matches!(
        &message.payload,
        iroha_data_model::block::consensus_v2::ConsensusMessageV2Payload::CertifiedBodyRequest(
            request
        ) if request.round.height == active_height
    )
}

fn selected_ingress_is_certified_body_response(inbound: &InboundBlockMessage) -> bool {
    matches!(
        inbound.message(),
        crate::sumeragi::message::BlockMessage::V2(
            iroha_data_model::block::consensus_v2::ConsensusMessageV2 {
                payload:
                    iroha_data_model::block::consensus_v2::ConsensusMessageV2Payload::CertifiedBodyResponse(
                        _,
                    ),
                ..
            },
        )
    )
}

fn prepare_and_dispatch_current_certified_serve<'cursor>(
    owner: &mut ProductionLifecycleOwnerV1,
    executor: &V2EffectExecutor<SerializedV2Runtime>,
    services: &ProductionV2Services,
    pending_capacity: &mut Option<PendingIngressCapacityV1>,
    receiver: &std::sync::Arc<FairV2Ingress>,
    cut: FairIngressTurnCut<'_>,
    runner: LifecycleCurrentRunnerTurn<'cursor>,
    terminal_subject: Option<iroha_data_model::block::consensus_v2::BlockSubject>,
) -> ProductionLifecycleIngressTurnV1<'cursor> {
    let classified = prepare_current_certified_serve_pre_admission(
        cut.selected_occurrence().inbound(),
        executor.context().height,
        terminal_subject,
        |request, sender| {
            executor
                .authenticate_certified_body_request(request, sender)
                .map_err(|error| error.to_string())
        },
    );
    let (authenticated, negative) = match classified {
        CurrentCertifiedServePreAdmissionV1::Authenticated { request } => (request, None),
        CurrentCertifiedServePreAdmissionV1::AuthenticatedNegative { request } => (
            request,
            Some(
                crate::sumeragi::v2_certified_serve_payload_store::CertifiedServePayloadNegativeOutcome::Cancelled,
            ),
        ),
        CurrentCertifiedServePreAdmissionV1::Negative { reason } => {
            return dequeue_prepared_ordinary_ingress(
                receiver,
                cut,
                runner,
                Some(ProductionPreparedCertifiedServeV1::Rejected(reason)),
                terminal_subject,
                services,
            );
        }
        CurrentCertifiedServePreAdmissionV1::Service(_reason) => {
            services
                .lifecycle_output_guard()
                .close_admission_for_restart();
            drop(cut);
            drop(runner);
            return ProductionLifecycleIngressTurnV1::Selected(
                ProductionLifecycleIngressSelectionV1::RestartRequired,
            );
        }
    };
    if !services.lifecycle_certified_serve_is_locally_authorized(&authenticated) {
        return dequeue_prepared_ordinary_ingress(
            receiver,
            cut,
            runner,
            Some(ProductionPreparedCertifiedServeV1::Rejected(
                "local validator has no certified retention authority".to_owned(),
            )),
            terminal_subject,
            services,
        );
    }

    let expected_context = lifecycle_context_for_ingress(executor.context());
    let lifecycle_cut = match cut.narrow_to_lifecycle(expected_context) {
        Ok(FairIngressTurnContextCut::Lifecycle(cut)) => cut,
        Ok(FairIngressTurnContextCut::Ordinary(cut)) => {
            iroha_logger::error!(
                "authenticated current Certified-Serve lost its active lifecycle context"
            );
            services
                .lifecycle_output_guard()
                .close_admission_for_restart();
            drop(cut);
            drop(runner);
            return ProductionLifecycleIngressTurnV1::Selected(
                ProductionLifecycleIngressSelectionV1::RestartRequired,
            );
        }
        Err((_error, retained)) => {
            services
                .lifecycle_output_guard()
                .close_admission_for_restart();
            drop(retained);
            drop(runner);
            return ProductionLifecycleIngressTurnV1::Selected(
                ProductionLifecycleIngressSelectionV1::RestartRequired,
            );
        }
    };
    let selector = match executor.capture_lifecycle_ingress_selector(lifecycle_cut) {
        Ok(selector) => selector,
        Err(_) => {
            services
                .lifecycle_output_guard()
                .close_admission_for_restart();
            drop(runner);
            return ProductionLifecycleIngressTurnV1::Selected(
                ProductionLifecycleIngressSelectionV1::RestartRequired,
            );
        }
    };
    let (dequeue, target) =
        match selector.into_locked_certified_serve_dequeue(receiver, &authenticated) {
            Ok(prepared) => prepared,
            Err(error) => {
                let reason = error.detail();
                iroha_logger::error!(reason, "Certified-Serve exact dequeue failed closed");
                services
                    .lifecycle_output_guard()
                    .close_admission_for_restart();
                drop(runner);
                return ProductionLifecycleIngressTurnV1::Selected(
                    ProductionLifecycleIngressSelectionV1::RestartRequired,
                );
            }
        };
    let ready_ledger = match LifecycleLedgerV1::from_coordinator(&owner.coordinator) {
        Ok(ledger) => ledger,
        Err(error) => {
            iroha_logger::error!(
                ?error,
                "Certified-Serve Ready-Producer ledger census failed closed"
            );
            services
                .lifecycle_output_guard()
                .close_admission_for_restart();
            drop(target);
            drop(dequeue);
            drop(runner);
            return ProductionLifecycleIngressTurnV1::Selected(
                ProductionLifecycleIngressSelectionV1::RestartRequired,
            );
        }
    };
    match owner.registry.registry().attest_ready_producer_turn_census(
        &owner.verified,
        &owner.coordinator,
        &ready_ledger,
    ) {
        Ok(Some(_attestation)) => {
            drop(target);
            drop(dequeue);
            drop(runner);
            return ProductionLifecycleIngressTurnV1::Selected(
                ProductionLifecycleIngressSelectionV1::CertifiedServeCompetingReady,
            );
        }
        Ok(None) => {}
        Err(error) => {
            iroha_logger::error!(
                ?error,
                "Certified-Serve Ready-Producer census failed closed"
            );
            services
                .lifecycle_output_guard()
                .close_admission_for_restart();
            drop(target);
            drop(dequeue);
            drop(runner);
            return ProductionLifecycleIngressTurnV1::Selected(
                ProductionLifecycleIngressSelectionV1::RestartRequired,
            );
        }
    }
    let local_signer = services.lifecycle_local_signer().clone();
    let mut reservation = match services.capture_lifecycle_certified_serve_capacity(target) {
        Ok(crate::sumeragi::v2_worker::LifecycleCertifiedServeCapacityCaptureV1::Reserved(
            reservation,
        )) => reservation,
        Ok(crate::sumeragi::v2_worker::LifecycleCertifiedServeCapacityCaptureV1::Unavailable(
            wait,
        )) => {
            assert!(pending_capacity.is_none());
            *pending_capacity = Some(PendingIngressCapacityV1::CertifiedServe(wait));
            drop(dequeue);
            drop(runner);
            return ProductionLifecycleIngressTurnV1::Selected(
                ProductionLifecycleIngressSelectionV1::CertifiedServeCapacityPending,
            );
        }
        Err(_) => {
            services
                .lifecycle_output_guard()
                .close_admission_for_restart();
            drop(dequeue);
            drop(runner);
            return ProductionLifecycleIngressTurnV1::Selected(
                ProductionLifecycleIngressSelectionV1::RestartRequired,
            );
        }
    };
    let target = match reservation.take_certified_serve_target(&authenticated) {
        Ok(target) => target,
        Err(()) => {
            drop(reservation);
            drop(dequeue);
            drop(runner);
            return ProductionLifecycleIngressTurnV1::Selected(
                ProductionLifecycleIngressSelectionV1::RestartRequired,
            );
        }
    };
    let admission = owner.admit_selected_certified_serve(target, &local_signer, &authenticated);
    let continuation = match admission.into_safe_continuation() {
        Ok(continuation) => continuation,
        Err(_restart) => {
            drop(reservation);
            drop(dequeue);
            drop(runner);
            return ProductionLifecycleIngressTurnV1::Selected(
                ProductionLifecycleIngressSelectionV1::RestartRequired,
            );
        }
    };
    let decision = continuation.decision();
    let failed_before_publication = continuation.failure().is_some() && decision.is_none();
    let (target, terminal_replay) = continuation.into_target_and_terminal_replay();
    if reservation
        .restore_certified_serve_target(target, &authenticated)
        .is_err()
    {
        drop(reservation);
        drop(dequeue);
        drop(runner);
        return ProductionLifecycleIngressTurnV1::Selected(
            ProductionLifecycleIngressSelectionV1::RestartRequired,
        );
    }
    if failed_before_publication {
        reservation.abort_certified_serve_before_plan();
        drop(dequeue);
        drop(runner);
        return ProductionLifecycleIngressTurnV1::Selected(
            ProductionLifecycleIngressSelectionV1::CertifiedServeRetry,
        );
    }
    if matches!(decision, Some(AdmissionDecision::StutterTerminal { .. })) {
        if terminal_replay.is_some() {
            drop(reservation);
            drop(dequeue);
            drop(runner);
            return ProductionLifecycleIngressTurnV1::Selected(
                ProductionLifecycleIngressSelectionV1::RestartRequired,
            );
        }
        reservation.abort_certified_serve_before_plan();
        let (inbound, disposition) = dequeue.commit();
        assert_eq!(disposition, FairV2IngressDequeueDisposition::Admit);
        drop(inbound);
        drop(runner);
        return ProductionLifecycleIngressTurnV1::Selected(
            ProductionLifecycleIngressSelectionV1::CertifiedServeTerminal,
        );
    }
    if matches!(decision, Some(AdmissionDecision::ReplayTerminal { .. })) {
        let Some(terminal_replay) = terminal_replay else {
            drop(reservation);
            drop(dequeue);
            drop(runner);
            return ProductionLifecycleIngressTurnV1::Selected(
                ProductionLifecycleIngressSelectionV1::RestartRequired,
            );
        };
        let (inbound, disposition) = dequeue.commit();
        assert_eq!(disposition, FairV2IngressDequeueDisposition::Admit);
        let task =
            match crate::sumeragi::v2_worker::LifecycleCertifiedServeTaskV1::from_terminal_replay(
                terminal_replay,
                authenticated,
                inbound,
            ) {
                Ok(task) => task,
                Err(_) => {
                    drop(reservation);
                    drop(runner);
                    return ProductionLifecycleIngressTurnV1::Selected(
                        ProductionLifecycleIngressSelectionV1::RestartRequired,
                    );
                }
            };
        if !reservation.preflight_lifecycle_certified_serve(&task) {
            drop(reservation);
            drop(runner);
            return ProductionLifecycleIngressTurnV1::Selected(
                ProductionLifecycleIngressSelectionV1::RestartRequired,
            );
        }
        reservation.commit_lifecycle_certified_serve(task);
        drop(runner);
        return ProductionLifecycleIngressTurnV1::Selected(
            ProductionLifecycleIngressSelectionV1::CertifiedServeReplayQueued,
        );
    }
    if terminal_replay.is_some() {
        drop(reservation);
        drop(dequeue);
        drop(runner);
        return ProductionLifecycleIngressTurnV1::Selected(
            ProductionLifecycleIngressSelectionV1::RestartRequired,
        );
    }
    if !matches!(
        decision,
        Some(AdmissionDecision::Admitted { .. } | AdmissionDecision::Retry { .. })
    ) {
        drop(reservation);
        drop(dequeue);
        drop(runner);
        return ProductionLifecycleIngressTurnV1::Selected(
            ProductionLifecycleIngressSelectionV1::RestartRequired,
        );
    }

    let ledger = match LifecycleLedgerV1::from_coordinator(&owner.coordinator) {
        Ok(ledger) => ledger,
        Err(_) => {
            drop(reservation);
            drop(dequeue);
            drop(runner);
            return ProductionLifecycleIngressTurnV1::Selected(
                ProductionLifecycleIngressSelectionV1::RestartRequired,
            );
        }
    };
    let registry = owner.registry.registry();
    let attestation = match registry.attest_ready_certified_serve_request(
        &owner.coordinator,
        &ledger,
        &authenticated,
    ) {
        Ok(attestation) => attestation,
        Err(_) => {
            drop(reservation);
            drop(dequeue);
            drop(runner);
            return ProductionLifecycleIngressTurnV1::Selected(
                ProductionLifecycleIngressSelectionV1::RestartRequired,
            );
        }
    };
    let observation = CertifiedServeSchedulerObservationV1::from_live_cuts(
        attestation,
        &reservation,
        &dequeue,
        &runner,
    );
    let dispatch = match claim_certified_serve_turn_v1(
        &mut owner.coordinator,
        registry,
        &ledger,
        vec![observation],
    ) {
        Ok(dispatch) => dispatch,
        Err(_) => {
            drop(reservation);
            drop(dequeue);
            drop(runner);
            return ProductionLifecycleIngressTurnV1::Selected(
                ProductionLifecycleIngressSelectionV1::RestartRequired,
            );
        }
    };

    if let Some(outcome) = negative {
        if owner
            .settle_certified_serve_negative(
                dispatch.lease().clone(),
                dispatch.authenticated_request(),
                outcome,
            )
            .is_err()
        {
            drop(dispatch);
            drop(reservation);
            drop(dequeue);
            drop(runner);
            return ProductionLifecycleIngressTurnV1::Selected(
                ProductionLifecycleIngressSelectionV1::RestartRequired,
            );
        }
        drop(dispatch);
        reservation.abort_certified_serve_before_plan();
        let (inbound, disposition) = dequeue.commit();
        assert_eq!(disposition, FairV2IngressDequeueDisposition::Admit);
        drop(inbound);
        drop(runner);
        return ProductionLifecycleIngressTurnV1::Selected(
            ProductionLifecycleIngressSelectionV1::CertifiedServeTerminal,
        );
    }

    let (inbound, disposition) = dequeue.commit();
    assert_eq!(disposition, FairV2IngressDequeueDisposition::Admit);
    let task = match crate::sumeragi::v2_worker::LifecycleCertifiedServeTaskV1::from_dequeued(
        dispatch, inbound,
    ) {
        Ok(task) => task,
        Err(_) => {
            drop(reservation);
            drop(runner);
            return ProductionLifecycleIngressTurnV1::Selected(
                ProductionLifecycleIngressSelectionV1::RestartRequired,
            );
        }
    };
    if !reservation.preflight_lifecycle_certified_serve(&task) {
        drop(reservation);
        drop(runner);
        return ProductionLifecycleIngressTurnV1::Selected(
            ProductionLifecycleIngressSelectionV1::RestartRequired,
        );
    }
    reservation.commit_lifecycle_certified_serve(task);
    drop(runner);
    ProductionLifecycleIngressTurnV1::Selected(
        ProductionLifecycleIngressSelectionV1::CertifiedServeQueued,
    )
}

fn dequeue_prepared_ordinary_ingress<'cursor>(
    receiver: &std::sync::Arc<FairV2Ingress>,
    cut: FairIngressTurnCut<'_>,
    runner: LifecycleCurrentRunnerTurn<'cursor>,
    prepared_serve: Option<ProductionPreparedCertifiedServeV1>,
    terminal_subject: Option<iroha_data_model::block::consensus_v2::BlockSubject>,
    services: &ProductionV2Services,
) -> ProductionLifecycleIngressTurnV1<'cursor> {
    let output_guard = services.lifecycle_output_guard();
    let Some(operation) = output_guard.begin_fail_stop_operation() else {
        drop(cut);
        drop(runner);
        return ProductionLifecycleIngressTurnV1::Selected(
            ProductionLifecycleIngressSelectionV1::RestartRequired,
        );
    };
    match cut.dequeue_exact_retaining() {
        Ok((inbound, disposition)) => {
            let turn = prepared_ordinary_ingress_turn(
                std::sync::Arc::clone(receiver),
                inbound,
                disposition,
                prepared_serve,
                terminal_subject,
                std::sync::Arc::clone(&output_guard),
            );
            operation.complete();
            drop(runner);
            ProductionLifecycleIngressTurnV1::Ordinary(turn)
        }
        Err((error, retained)) => {
            iroha_logger::error!(
                ?error,
                "Sumeragi v2 ordinary ingress exact dequeue failed closed"
            );
            drop(operation);
            drop(retained);
            drop(runner);
            ProductionLifecycleIngressTurnV1::Selected(
                ProductionLifecycleIngressSelectionV1::RestartRequired,
            )
        }
    }
}

impl LaunchedProductionLifecycleV1 {
    fn settle_parked_certified_fetch_body_persistence(
        &mut self,
    ) -> ProductionLifecycleCompletionSelectionV1 {
        let Self {
            owner,
            executor,
            services,
            pending_lifecycle_completion,
            leader_wire_ingress_binding,
            ..
        } = self;
        let Some(completion) =
            PendingLifecycleCompletionV1::take_certified_fetch(pending_lifecycle_completion)
        else {
            services
                .lifecycle_output_guard()
                .close_admission_for_restart();
            return ProductionLifecycleCompletionSelectionV1::RestartRequired;
        };
        match owner.coordinator.complete_certified_fetch_body_persistence(
            &mut owner.registry,
            executor,
            services,
            &leader_wire_ingress_binding.ingress,
            completion,
        ) {
            Ok(()) => ProductionLifecycleCompletionSelectionV1::CertifiedFetchBodyPersisted,
            Err(CertifiedFetchBodyPersistenceCompletionError::Retry(error)) => {
                iroha_logger::debug!(
                    reason = error.reason(),
                    detail = %error.detail(),
                    "ordinary certified-Fetch Phase B retained its exact owner"
                );
                assert!(pending_lifecycle_completion.is_none());
                *pending_lifecycle_completion = Some(PendingLifecycleCompletionV1::CertifiedFetch(
                    error.into_completion(),
                ));
                ProductionLifecycleCompletionSelectionV1::CertifiedFetchBodyPersistenceRetry
            }
            Err(CertifiedFetchBodyPersistenceCompletionError::RestartRequired(error)) => {
                iroha_logger::error!(
                    reason = error.reason(),
                    detail = %error.detail(),
                    work_id = error.work_id().get(),
                    physical_admission_ordinal = error.physical_admission_ordinal(),
                    "ordinary certified-Fetch Phase B crossed its fail-stop boundary"
                );
                services
                    .lifecycle_output_guard()
                    .close_admission_for_restart();
                drop(error);
                ProductionLifecycleCompletionSelectionV1::CertifiedFetchBodyPersistenceRestartRequired
            }
        }
    }

    /// Classify parked and physical lifecycle owners before any fresh Ready dispatch.
    ///
    /// Classification precedes cursor consumption. Ordinary work returns the
    /// same borrow-bound turn, while a recovered class is dispatched, drained,
    /// or settled internally without exposing mutually exclusive methods.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::sumeragi) fn drive_completion_pre_gate<'cursor>(
        &mut self,
        runner: LifecycleCurrentRunnerTurn<'cursor>,
        lane_work: &mut V2LaneWorkAdapter,
    ) -> ProductionLifecycleCompletionPreGateV1<'cursor> {
        if !self.runner_turn_matches(
            &runner,
            crate::sumeragi::v2_runner::LifecycleRunnerRankTarget::Completion,
        ) {
            return ProductionLifecycleCompletionPreGateV1::Ordinary(runner);
        }

        if let Some(pending) = self.pending_lifecycle_completion.take() {
            let selected = match pending {
                PendingLifecycleCompletionV1::RecoveredDecisionApplyDeferred(deferred) => {
                    match self.drive_recovered_decision_apply_deferred(deferred, lane_work) {
                        ProductionRecoveredDecisionApplyRetryV1::Requeued => {
                            ProductionLifecycleCompletionSelectionV1::RecoveredDecisionApplyRequeued
                        }
                        ProductionRecoveredDecisionApplyRetryV1::Unavailable(deferred) => {
                            assert!(self.pending_lifecycle_completion.is_none());
                            self.pending_lifecycle_completion = Some(
                                PendingLifecycleCompletionV1::RecoveredDecisionApplyDeferred(
                                    deferred,
                                ),
                            );
                            ProductionLifecycleCompletionSelectionV1::RecoveredDecisionApplyDeferred
                        }
                        ProductionRecoveredDecisionApplyRetryV1::RestartRequired => {
                            ProductionLifecycleCompletionSelectionV1::RecoveredDecisionApplyRestartRequired
                        }
                    }
                }
                PendingLifecycleCompletionV1::CertifiedFetch(completion) => {
                    self.pending_lifecycle_completion =
                        Some(PendingLifecycleCompletionV1::CertifiedFetch(completion));
                    self.settle_parked_certified_fetch_body_persistence()
                }
                PendingLifecycleCompletionV1::RecoveredDecisionFetch(completion) => {
                    self.pending_lifecycle_completion = Some(
                        PendingLifecycleCompletionV1::RecoveredDecisionFetch(completion),
                    );
                    ProductionLifecycleCompletionSelectionV1::RecoveredDecisionFetchCompletion(
                        self.settle_recovered_decision_fetch_store(),
                    )
                }
                PendingLifecycleCompletionV1::RecoveredSign(completion) => {
                    self.pending_lifecycle_completion =
                        Some(PendingLifecycleCompletionV1::RecoveredSign(completion));
                    ProductionLifecycleCompletionSelectionV1::RecoveredLifecycleSignCompletion(
                        self.settle_parked_recovered_sign_completion(),
                    )
                }
            };
            return ProductionLifecycleCompletionPreGateV1::Selected(selected);
        }

<<<<<<< HEAD
        let parked_completion_owners =
            usize::from(self.recovered_lifecycle_sign_completion.is_some())
                + usize::from(self.recovered_decision_fetch_body_completion.is_some())
                + usize::from(self.certified_fetch_body_completion.is_some())
                + usize::from(self.lifecycle_durable_validate_completion.is_some());
        if parked_completion_owners > 1 {
            self.close_output_for_restart();
            return ProductionLifecycleCompletionTurnV1::Selected(
                ProductionLifecycleCompletionSelectionV1::RestartRequired,
            );
        }
        if self.recovered_lifecycle_sign_completion.is_some() {
            let selected = self.settle_parked_recovered_sign_completion();
            return ProductionLifecycleCompletionTurnV1::Selected(
                ProductionLifecycleCompletionSelectionV1::RecoveredLifecycleSignCompletion(
                    selected,
                ),
            );
        }
        if self.recovered_decision_fetch_body_completion.is_some() {
            let selected = self.settle_recovered_decision_fetch_store();
            return ProductionLifecycleCompletionTurnV1::Selected(
                ProductionLifecycleCompletionSelectionV1::RecoveredDecisionFetchCompletion(
                    selected,
                ),
            );
        }
        if self.certified_fetch_body_completion.is_some() {
            let selected = self.settle_certified_fetch_body_completion();
            return ProductionLifecycleCompletionTurnV1::Selected(selected);
        }
        if self.lifecycle_durable_validate_completion.is_some() {
            let selected = self.settle_parked_lifecycle_durable_validate_completion();
            return ProductionLifecycleCompletionTurnV1::Selected(selected);
        }

        match self.services.take_next_recovered_lifecycle_completion() {
            Ok(RecoveredLifecycleCompletionTakeV1::PassThrough) => {
                return ProductionLifecycleCompletionTurnV1::PassThrough(runner);
=======
        match self.services.take_next_lifecycle_completion() {
            Ok(LifecycleCompletionTakeV1::PassThrough) => {
                return ProductionLifecycleCompletionPreGateV1::Ordinary(runner);
>>>>>>> origin/optimizations
            }
            Ok(LifecycleCompletionTakeV1::CertifiedFetch(completion)) => {
                assert!(self.pending_lifecycle_completion.is_none());
                self.pending_lifecycle_completion =
                    Some(PendingLifecycleCompletionV1::CertifiedFetch(completion));
                let selected = self.settle_parked_certified_fetch_body_persistence();
                return ProductionLifecycleCompletionPreGateV1::Selected(selected);
            }
            Ok(LifecycleCompletionTakeV1::Apply(completion)) => {
                let selected = match self
                    .settle_recovered_decision_apply_completion_owner(completion, lane_work)
                {
                    Ok(ProductionRecoveredDecisionApplyCompletionV1::Applied) => {
                        ProductionLifecycleCompletionSelectionV1::RecoveredDecisionApplyApplied
                    }
                    Ok(ProductionRecoveredDecisionApplyCompletionV1::Deferred(deferred)) => {
                        assert!(self.pending_lifecycle_completion.is_none());
                        self.pending_lifecycle_completion = Some(
                            PendingLifecycleCompletionV1::RecoveredDecisionApplyDeferred(deferred),
                        );
                        ProductionLifecycleCompletionSelectionV1::RecoveredDecisionApplyCompletionDeferred
                    }
                    Err(reason) => {
                        iroha_logger::error!(
                            %reason,
                            "recovered Decision Apply completion settlement failed closed"
                        );
                        self.close_output_for_restart();
                        ProductionLifecycleCompletionSelectionV1::RecoveredDecisionApplyCompletionRestartRequired
                    }
                };
                return ProductionLifecycleCompletionPreGateV1::Selected(selected);
            }
            Ok(LifecycleCompletionTakeV1::Sign(completion)) => {
                assert!(self.pending_lifecycle_completion.is_none());
                self.pending_lifecycle_completion =
                    Some(PendingLifecycleCompletionV1::RecoveredSign(completion));
                let selected = self.settle_parked_recovered_sign_completion();
                return ProductionLifecycleCompletionPreGateV1::Selected(
                    ProductionLifecycleCompletionSelectionV1::RecoveredLifecycleSignCompletion(
                        selected,
                    ),
                );
            }
            Ok(LifecycleCompletionTakeV1::DecisionFetch(completion)) => {
                assert!(self.pending_lifecycle_completion.is_none());
                self.pending_lifecycle_completion = Some(
                    PendingLifecycleCompletionV1::RecoveredDecisionFetch(completion),
                );
                let selected = self.settle_recovered_decision_fetch_store();
                return ProductionLifecycleCompletionPreGateV1::Selected(
                    ProductionLifecycleCompletionSelectionV1::RecoveredDecisionFetchCompletion(
                        selected,
                    ),
                );
            }
<<<<<<< HEAD
            Ok(RecoveredLifecycleCompletionTakeV1::DurableValidate(completion)) => {
                assert!(self.lifecycle_durable_validate_completion.is_none());
                self.lifecycle_durable_validate_completion = Some(completion);
                let selected = self.settle_parked_lifecycle_durable_validate_completion();
                return ProductionLifecycleCompletionTurnV1::Selected(selected);
            }
            Ok(RecoveredLifecycleCompletionTakeV1::CertifiedFetch(completion)) => {
                assert!(self.certified_fetch_body_completion.is_none());
                self.certified_fetch_body_completion = Some(completion);
                let selected = self.settle_certified_fetch_body_completion();
                return ProductionLifecycleCompletionTurnV1::Selected(selected);
            }
            Ok(RecoveredLifecycleCompletionTakeV1::CertifiedServe(completion)) => {
=======
            Ok(LifecycleCompletionTakeV1::CertifiedServe(completion)) => {
>>>>>>> origin/optimizations
                let selected = match completion
                    .settle_deliver_and_acknowledge(&mut self.owner, &self.services)
                {
                    Ok(
                        crate::sumeragi::v2_worker::LifecycleCertifiedServeCompletionSettlementV1::Claimed,
                    ) => ProductionLifecycleCompletionSelectionV1::CertifiedServeClaimedCompleted,
                    Ok(
                        crate::sumeragi::v2_worker::LifecycleCertifiedServeCompletionSettlementV1::TerminalReplay,
                    ) => ProductionLifecycleCompletionSelectionV1::CertifiedServeReplayCompleted,
                    Err(reason) => {
                        iroha_logger::error!(
                            %reason,
                            "lifecycle Certified-Serve completion failed closed"
                        );
                        self.close_output_for_restart();
                        ProductionLifecycleCompletionSelectionV1::RestartRequired
                    }
                };
                return ProductionLifecycleCompletionPreGateV1::Selected(selected);
            }
            Ok(LifecycleCompletionTakeV1::None) => {}
            Err(reason) => {
                iroha_logger::error!(
                    %reason,
                    "Sumeragi v2 lifecycle Completion physical-head classification failed closed"
                );
                self.close_output_for_restart();
                return ProductionLifecycleCompletionPreGateV1::Selected(
                    ProductionLifecycleCompletionSelectionV1::RestartRequired,
                );
            }
        }

<<<<<<< HEAD
        let reducer_fence_generation = self.executor.lifecycle_reducer_fence_generation();
        let selected = match self
            .owner
            .classify_completion_ready_work(reducer_fence_generation)
        {
=======
        ProductionLifecycleCompletionPreGateV1::Ready(ProductionLifecycleReadyCompletionTurnV1 {
            runner,
        })
    }

    /// Dispatch fresh Ready work only after the caller proves Producer claims are eligible.
    pub(in crate::sumeragi) fn drive_ready_completion_turn<'cursor>(
        &mut self,
        ready: ProductionLifecycleReadyCompletionTurnV1<'cursor>,
    ) -> ProductionLifecycleCompletionTurnV1<'cursor> {
        let ProductionLifecycleReadyCompletionTurnV1 { runner } = ready;
        let selected = match self.owner.classify_completion_ready_work() {
>>>>>>> origin/optimizations
            super::super::ProductionCompletionReadyWorkV1::None
            | super::super::ProductionCompletionReadyWorkV1::PassThrough => {
                return ProductionLifecycleCompletionTurnV1::PassThrough(runner);
            }
            super::super::ProductionCompletionReadyWorkV1::Invalid => {
                iroha_logger::error!("Sumeragi v2 lifecycle Completion Ready census failed closed");
                self.close_output_for_restart();
                ProductionLifecycleCompletionSelectionV1::RestartRequired
            }
            super::super::ProductionCompletionReadyWorkV1::RecoveredIo => {
                let result = {
                    let Self {
                        owner,
                        executor,
                        services,
                        ..
                    } = self;
                    owner.dispatch_recovered_completion_with_runner_debt(
                        services,
                        executor,
                        runner.debt(),
                    )
                };
                if let Err(error) = &result {
                    iroha_logger::error!(
                        ?error,
                        "Sumeragi v2 recovered Completion dispatch failed closed"
                    );
                    self.close_output_for_restart();
                }
                ProductionLifecycleCompletionSelectionV1::RecoveredIoDispatch(result)
            }
            super::super::ProductionCompletionReadyWorkV1::CertifiedBodyPipeline => {
                match self.advance_certified_body_pipeline() {
                    Ok(true) => {
                        ProductionLifecycleCompletionSelectionV1::CertifiedBodyPipelineAdvanced
                    }
                    Ok(false) => {
                        ProductionLifecycleCompletionSelectionV1::CertifiedBodyPipelineCapacityPending
                    }
                    Err(error) => {
                        iroha_logger::error!(
                            %error,
                            "ordinary certified-body lifecycle transition failed closed"
                        );
                        self.close_output_for_restart();
                        ProductionLifecycleCompletionSelectionV1::RestartRequired
                    }
                }
            }
            super::super::ProductionCompletionReadyWorkV1::RecoveredLifecycleBroadcast => {
                let result = {
                    let Self {
                        owner, services, ..
                    } = self;
                    owner.refanout_recovered_lifecycle_signed_broadcast_with_runner_debt(
                        services,
                        runner.debt(),
                    )
                };
                if let Err(error) = &result {
                    iroha_logger::error!(
                        ?error,
                        "Sumeragi v2 recovered Broadcast refanout failed closed"
                    );
                    self.close_output_for_restart();
                }
                ProductionLifecycleCompletionSelectionV1::RecoveredLifecycleBroadcastRefanout(
                    result,
                )
            }
        };
        ProductionLifecycleCompletionTurnV1::Selected(selected)
    }

    /// Service one exact outer Completion turn through the lifecycle owner.
    ///
    /// Direct callers retain the historical full turn. The active-height
    /// driver uses the pre-gate and Ready dispatcher separately so an existing
    /// non-Producer lease cannot reach this fresh-claim branch.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::sumeragi) fn drive_completion_turn<'cursor>(
        &mut self,
        runner: LifecycleCurrentRunnerTurn<'cursor>,
        lane_work: &mut V2LaneWorkAdapter,
    ) -> ProductionLifecycleCompletionTurnV1<'cursor> {
        match self.drive_completion_pre_gate(runner, lane_work) {
            ProductionLifecycleCompletionPreGateV1::Selected(selected) => {
                ProductionLifecycleCompletionTurnV1::Selected(selected)
            }
            ProductionLifecycleCompletionPreGateV1::Ordinary(runner) => {
                ProductionLifecycleCompletionTurnV1::PassThrough(runner)
            }
            ProductionLifecycleCompletionPreGateV1::Ready(ready) => {
                self.drive_ready_completion_turn(ready)
            }
        }
    }

    /// Service one exact outer Ingress turn through recovered Fetch Phase A.
    ///
    /// A retained capacity wait is classified before any fresh queue probe.
    /// Fresh selection accepts the queue-owned recovered winner or transfers
    /// the exact ordinary winner to the activated shared runner consumer.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::sumeragi) fn drive_ingress_turn<'cursor>(
        &mut self,
        runner: LifecycleCurrentRunnerTurn<'cursor>,
    ) -> ProductionLifecycleIngressTurnV1<'cursor> {
        if !self.runner_turn_matches(
            &runner,
            crate::sumeragi::v2_runner::LifecycleRunnerRankTarget::Ingress,
        ) {
            return ProductionLifecycleIngressTurnV1::PassThrough(runner);
        }
<<<<<<< HEAD
        let parked_ingress_owners = usize::from(self.recovered_ingress_capacity_wait.is_some())
            + usize::from(self.certified_fetch_ingress_capacity_wait.is_some())
            + usize::from(self.certified_serve_capacity_wait.is_some());
        if parked_ingress_owners > 1 {
            self.close_output_for_restart();
            return ProductionLifecycleIngressTurnV1::Selected(
                ProductionLifecycleIngressSelectionV1::RestartRequired,
            );
        }

        if let Some(wait) = self.recovered_ingress_capacity_wait.take() {
            match wait.retry(&self.services, &self.executor) {
=======
        if let Some(pending) = self.pending_ingress_capacity.take() {
            match pending {
                PendingIngressCapacityV1::CertifiedServe(wait) => match wait.status(&self.services) {
                    crate::sumeragi::v2_worker::LifecycleIoCapacityWaitStatus::SamePending => {
                        assert!(self.pending_ingress_capacity.is_none());
                        self.pending_ingress_capacity =
                            Some(PendingIngressCapacityV1::CertifiedServe(wait));
                        return ProductionLifecycleIngressTurnV1::Selected(
                            ProductionLifecycleIngressSelectionV1::CertifiedServeCapacityPending,
                        );
                    }
                    crate::sumeragi::v2_worker::LifecycleIoCapacityWaitStatus::Released => {
                        drop(wait);
                    }
                    crate::sumeragi::v2_worker::LifecycleIoCapacityWaitStatus::GenerationExhausted
                    | crate::sumeragi::v2_worker::LifecycleIoCapacityWaitStatus::ForeignOrDisconnected => {
                        self.close_output_for_restart();
                        return ProductionLifecycleIngressTurnV1::Selected(
                            ProductionLifecycleIngressSelectionV1::RestartRequired,
                        );
                    }
                },
                pending => {
                    let (kind, retry) = match pending {
                        PendingIngressCapacityV1::CertifiedFetch(wait) => (
                            PendingIngressCapacityKindV1::CertifiedFetch,
                            wait.retry(&self.services, &self.executor),
                        ),
                        PendingIngressCapacityV1::RecoveredDecisionFetch(wait) => (
                            PendingIngressCapacityKindV1::RecoveredDecisionFetch,
                            wait.retry(&self.services, &self.executor),
                        ),
                        PendingIngressCapacityV1::CertifiedServe(_) => unreachable!(
                            "the Certified-Serve capacity owner was handled before Fetch retry"
                        ),
                    };
                    match retry {
>>>>>>> origin/optimizations
                super::super::ProductionIngressCapacityRetry::Pending(wait) => {
                    assert!(self.pending_ingress_capacity.is_none());
                    self.pending_ingress_capacity = Some(match kind {
                        PendingIngressCapacityKindV1::CertifiedFetch => {
                            PendingIngressCapacityV1::CertifiedFetch(wait)
                        }
                        PendingIngressCapacityKindV1::RecoveredDecisionFetch => {
                            PendingIngressCapacityV1::RecoveredDecisionFetch(wait)
                        }
                    });
                    return ProductionLifecycleIngressTurnV1::Selected(match kind {
                        PendingIngressCapacityKindV1::CertifiedFetch => {
                            ProductionLifecycleIngressSelectionV1::CertifiedFetchCapacityPending
                        }
                        PendingIngressCapacityKindV1::RecoveredDecisionFetch => {
                            ProductionLifecycleIngressSelectionV1::RecoveredDecisionFetchCapacityPending
                        }
                    });
                }
                super::super::ProductionIngressCapacityRetry::Released(selector) => {
                    return match kind {
                        PendingIngressCapacityKindV1::CertifiedFetch => {
                            self.drive_certified_fetch_ingress_selector(selector, runner)
                        }
                        PendingIngressCapacityKindV1::RecoveredDecisionFetch => {
                            self.drive_recovered_ingress_selector(selector, runner)
                        }
                    };
                }
                super::super::ProductionIngressCapacityRetry::RestartRequired => {
                    self.close_output_for_restart();
                    return ProductionLifecycleIngressTurnV1::Selected(
                        ProductionLifecycleIngressSelectionV1::RestartRequired,
                    );
                }
<<<<<<< HEAD
            }
        }

        if let Some(wait) = self.certified_fetch_ingress_capacity_wait.take() {
            match wait.retry(&self.services, &self.executor) {
                super::super::ProductionIngressCapacityRetry::Pending(wait) => {
                    assert!(self.certified_fetch_ingress_capacity_wait.is_none());
                    self.certified_fetch_ingress_capacity_wait = Some(wait);
                    return ProductionLifecycleIngressTurnV1::Selected(
                        ProductionLifecycleIngressSelectionV1::CapacityPending,
                    );
                }
                super::super::ProductionIngressCapacityRetry::Released(selector) => {
                    return self.drive_certified_fetch_ingress_selector(selector, runner);
                }
                super::super::ProductionIngressCapacityRetry::RestartRequired => {
                    self.close_output_for_restart();
                    return ProductionLifecycleIngressTurnV1::Selected(
                        ProductionLifecycleIngressSelectionV1::RestartRequired,
                    );
                }
            }
        }

        if let Some(wait) = self.certified_serve_capacity_wait.take() {
            match wait.status(&self.services) {
                crate::sumeragi::v2_worker::LifecycleIoCapacityWaitStatus::SamePending => {
                    assert!(self.certified_serve_capacity_wait.is_none());
                    self.certified_serve_capacity_wait = Some(wait);
                    return ProductionLifecycleIngressTurnV1::Selected(
                        ProductionLifecycleIngressSelectionV1::CapacityPending,
                    );
                }
                crate::sumeragi::v2_worker::LifecycleIoCapacityWaitStatus::Released => {
                    drop(wait);
                }
                crate::sumeragi::v2_worker::LifecycleIoCapacityWaitStatus::GenerationExhausted
                | crate::sumeragi::v2_worker::LifecycleIoCapacityWaitStatus::ForeignOrDisconnected => {
                    self.close_output_for_restart();
                    return ProductionLifecycleIngressTurnV1::Selected(
                        ProductionLifecycleIngressSelectionV1::RestartRequired,
                    );
=======
                    }
>>>>>>> origin/optimizations
                }
            }
        }

        let terminal_subject = match self.executor.lifecycle_terminal_subject() {
            Ok(subject) => subject,
            Err(error) => {
                iroha_logger::error!(
                    %error,
                    "Sumeragi v2 ingress terminal-subject projection failed closed"
                );
                self.close_output_for_restart();
                drop(runner);
                return ProductionLifecycleIngressTurnV1::Selected(
                    ProductionLifecycleIngressSelectionV1::RestartRequired,
                );
            }
        };
        let ingress = std::sync::Arc::clone(&self.leader_wire_ingress_binding.ingress);
        let cut = {
            let executor = &self.executor;
            ingress.capture_next_ingress_turn_cut(|occurrence| {
                crate::sumeragi::v2_effects::v2_ingress_head_can_drain(
                    occurrence.inbound(),
                    executor,
                    terminal_subject,
                )
            })
        };
        let Some(cut) = (match cut {
            Ok(cut) => cut,
            Err(error) => {
                iroha_logger::error!(
                    ?error,
                    "Sumeragi v2 fair-ingress turn-cut capture failed closed"
                );
                self.close_output_for_restart();
                drop(runner);
                return ProductionLifecycleIngressTurnV1::Selected(
                    ProductionLifecycleIngressSelectionV1::RestartRequired,
                );
            }
        }) else {
            return ProductionLifecycleIngressTurnV1::PassThrough(runner);
        };

        if cut.selected_disposition() == FairV2IngressDequeueDisposition::RetireObsolete {
            return dequeue_prepared_ordinary_ingress(
                &ingress,
                cut,
                runner,
                None,
                terminal_subject,
                &self.services,
            );
        }

        if selected_ingress_is_current_certified_serve(
            cut.selected_occurrence().inbound(),
            self.executor.context().height,
        ) {
            let Self {
                owner,
                executor,
                services,
                pending_ingress_capacity,
                ..
            } = self;
            return prepare_and_dispatch_current_certified_serve(
                owner,
                executor,
                services,
                pending_ingress_capacity,
                &ingress,
                cut,
                runner,
                terminal_subject,
            );
        }

        if !selected_ingress_is_certified_body_response(cut.selected_occurrence().inbound()) {
            return dequeue_prepared_ordinary_ingress(
                &ingress,
                cut,
                runner,
                None,
                terminal_subject,
                &self.services,
            );
        }

        let expected_context = lifecycle_context_for_ingress(self.executor.context());
        let contextual = match cut.narrow_to_lifecycle(expected_context) {
            Ok(contextual) => contextual,
            Err((_error, retained)) => {
                self.close_output_for_restart();
                drop(retained);
                drop(runner);
                return ProductionLifecycleIngressTurnV1::Selected(
                    ProductionLifecycleIngressSelectionV1::RestartRequired,
                );
            }
        };
        match contextual {
            FairIngressTurnContextCut::Ordinary(cut) => dequeue_prepared_ordinary_ingress(
                &ingress,
                cut,
                runner,
                None,
                terminal_subject,
                &self.services,
            ),
            FairIngressTurnContextCut::Lifecycle(cut) => {
                let recovered = match self.executor.selected_cut_is_recovered_decision_fetch(&cut) {
                    Ok(recovered) => recovered,
                    Err(error) => {
                        let reason = error.detail();
                        iroha_logger::error!(
                            %reason,
                            "Sumeragi v2 recovered Fetch selection failed closed"
                        );
                        self.close_output_for_restart();
                        drop(cut);
                        drop(runner);
                        return ProductionLifecycleIngressTurnV1::Selected(
                            ProductionLifecycleIngressSelectionV1::RestartRequired,
                        );
                    }
                };
                if !recovered {
                    let selector = match self.executor.capture_lifecycle_ingress_selector(cut) {
                        Ok(selector) => selector,
                        Err(error) => {
<<<<<<< HEAD
                            iroha_logger::error!(
                                ?error,
                                "ordinary certified-Fetch selection failed closed"
=======
                            let reason = error.detail();
                            iroha_logger::error!(
                                %reason,
                                "ordinary certified-Fetch selector capture failed closed"
>>>>>>> origin/optimizations
                            );
                            self.close_output_for_restart();
                            drop(runner);
                            return ProductionLifecycleIngressTurnV1::Selected(
                                ProductionLifecycleIngressSelectionV1::RestartRequired,
                            );
                        }
                    };
                    return self.drive_certified_fetch_ingress_selector(selector, runner);
                }
                let selector = match self
                    .executor
                    .prepare_recovered_decision_fetch_from_selected_cut(cut)
                {
                    Ok(selector) => selector,
                    Err(error) => {
                        let reason = error.detail();
                        iroha_logger::error!(
                            %reason,
                            "Sumeragi v2 recovered Fetch preparation failed closed"
                        );
                        self.close_output_for_restart();
                        drop(runner);
                        return ProductionLifecycleIngressTurnV1::Selected(
                            ProductionLifecycleIngressSelectionV1::RestartRequired,
                        );
                    }
                };
                self.drive_recovered_ingress_selector(selector, runner)
            }
        }
    }

    fn drive_certified_fetch_ingress_selector<'cursor>(
        &mut self,
        selector: PreparedLifecycleIngressSelector,
        runner: LifecycleCurrentRunnerTurn<'cursor>,
    ) -> ProductionLifecycleIngressTurnV1<'cursor> {
        let mode = self.executor.lifecycle_mode_rank_snapshot();
        match self
            .owner
            .plan_ingress_turn(&self.services, &self.executor, mode, selector, runner)
        {
            Ok(super::super::ProductionIngressTurnPreparation::CapacityWait(wait)) => {
                assert!(self.certified_fetch_ingress_capacity_wait.is_none());
                self.certified_fetch_ingress_capacity_wait = Some(wait);
                ProductionLifecycleIngressTurnV1::Selected(
                    ProductionLifecycleIngressSelectionV1::CapacityPending,
                )
            }
            Ok(super::super::ProductionIngressTurnPreparation::Queued(queued)) => {
                let ordinal = queued.ordinal();
                if self.owner.coordinator.active_lease.is_some()
                    || self
                        .owner
                        .coordinator
                        .records
                        .get(&ordinal)
                        .is_none_or(|record| !matches!(record.state, LifecycleState::Waiting(_)))
                {
                    iroha_logger::error!(
                        ordinal,
                        "queued certified-Fetch lost its blocked lifecycle row"
                    );
                    self.close_output_for_restart();
                    ProductionLifecycleIngressTurnV1::Selected(
                        ProductionLifecycleIngressSelectionV1::RestartRequired,
                    )
                } else {
                    ProductionLifecycleIngressTurnV1::Selected(
                        ProductionLifecycleIngressSelectionV1::CertifiedFetchQueued,
                    )
                }
            }
            Err(_) => {
                self.close_output_for_restart();
                ProductionLifecycleIngressTurnV1::Selected(
                    ProductionLifecycleIngressSelectionV1::RestartRequired,
                )
            }
        }
    }

    fn settle_certified_fetch_body_completion(
        &mut self,
    ) -> ProductionLifecycleCompletionSelectionV1 {
        let Some(completion) = self.certified_fetch_body_completion.take() else {
            self.close_output_for_restart();
            return ProductionLifecycleCompletionSelectionV1::RestartRequired;
        };
        let Self {
            owner,
            executor,
            services,
            leader_wire_ingress_binding,
            ..
        } = self;
        let ProductionLifecycleOwnerV1 {
            coordinator,
            registry,
            ..
        } = owner;
        match coordinator.complete_certified_fetch_body_persistence(
            registry,
            executor,
            services,
            &leader_wire_ingress_binding.ingress,
            completion,
        ) {
            Ok(()) => ProductionLifecycleCompletionSelectionV1::CertifiedFetchBodyCompleted,
            Err(super::super::CertifiedFetchBodyPersistenceCompletionError::Retry(error)) => {
                assert!(self.certified_fetch_body_completion.is_none());
                self.certified_fetch_body_completion = Some(error.into_completion());
                ProductionLifecycleCompletionSelectionV1::CertifiedFetchBodyRetry
            }
            Err(super::super::CertifiedFetchBodyPersistenceCompletionError::RestartRequired(
                error,
            )) => {
                iroha_logger::error!(
                    reason = error.reason(),
                    detail = %error.detail(),
                    "certified-Fetch Phase B requires cold restart"
                );
                self.close_output_for_restart();
                ProductionLifecycleCompletionSelectionV1::RestartRequired
            }
            Err(
                super::super::CertifiedFetchBodyPersistenceCompletionError::RestartRequiredAfterCommit(
                    error,
                ),
            ) => {
                iroha_logger::error!(
                    %error,
                    "certified-Fetch durable ingress terminal requires cold restart"
                );
                self.close_output_for_restart();
                ProductionLifecycleCompletionSelectionV1::RestartRequired
            }
        }
    }

    fn settle_parked_lifecycle_durable_validate_completion(
        &mut self,
    ) -> ProductionLifecycleCompletionSelectionV1 {
        let completion = self
            .lifecycle_durable_validate_completion
            .take()
            .expect("selected durable Validate completion remains parked");
        match completion.kind() {
            LifecycleDurableValidateCompletionKindV1::Retry => {
                if let Some(reason) = completion.retry_reason() {
                    iroha_logger::debug!(
                        %reason,
                        "ordinary durable Validate retained its exact worker retry"
                    );
                }
                match completion.retry() {
                    LifecycleDurableValidateRetryV1::Requeued => {
                        ProductionLifecycleCompletionSelectionV1::DurableValidateRetryQueued
                    }
                    LifecycleDurableValidateRetryV1::Unavailable(completion) => {
                        assert!(self.lifecycle_durable_validate_completion.is_none());
                        self.lifecycle_durable_validate_completion = Some(completion);
                        ProductionLifecycleCompletionSelectionV1::DurableValidateRetryPending
                    }
                    LifecycleDurableValidateRetryV1::RestartRequired => {
                        self.close_output_for_restart();
                        ProductionLifecycleCompletionSelectionV1::RestartRequired
                    }
                }
            }
            LifecycleDurableValidateCompletionKindV1::Executed => {
                let published = completion.publish_executed(|dispatch| {
                    match self.owner.coordinator.complete_durable_validate_dispatch(
                        &mut self.owner.registry,
                        dispatch,
                    ) {
                        Ok(
                            super::super::work_registry::DurableValidateCompletionPublication::PublishedValidated(_)
                            | super::super::work_registry::DurableValidateCompletionPublication::PublishedRejected(_),
                        ) => Ok(()),
                        Ok(
                            super::super::work_registry::DurableValidateCompletionPublication::DeferredMergeSidecar(
                                deferred,
                            ),
                        ) => Err(deferred.into_executed_dispatch()),
                        Err((_error, dispatch)) => Err(dispatch),
                    }
                });
                match published {
                    Ok(()) => {
                        ProductionLifecycleCompletionSelectionV1::DurableValidateCompletionPublished
                    }
                    Err(completion) => {
                        drop(completion);
                        self.close_output_for_restart();
                        ProductionLifecycleCompletionSelectionV1::RestartRequired
                    }
                }
            }
        }
    }

    fn publish_ready_durable_validate(
        &mut self,
        lease: TurnLease,
        slot: PhysicalSlotId,
        operation: crate::sumeragi::output_guard::ConsensusFailStopOperation<'_>,
    ) -> Result<bool, String> {
        let Self {
            owner, executor, ..
        } = self;
        let ProductionLifecycleOwnerV1 {
            verified,
            coordinator,
            registry,
            ..
        } = owner;
        let prepared = registry
            .registry_mut()
            .prepare_ready_durable_validate_execution(&lease, slot, verified)
            .map_err(|error| format!("ready durable Validate execution: {error:?}"))?;
        let preview = executor
            .prepare_ready_durable_validate_adapter(prepared)
            .map_err(|_| "ready durable Validate adapter preview failed".to_owned())?;
        match preview.kind() {
            ReadyDurableValidateAdapterPublicationKind::ValidatedInactive
            | ReadyDurableValidateAdapterPublicationKind::ValidatedNoEffect
            | ReadyDurableValidateAdapterPublicationKind::RejectedInactive
            | ReadyDurableValidateAdapterPublicationKind::RejectedNoEffect => {
                coordinator
                    .prepare_sealed_validate_no_successor_transition(&lease, preview)
                    .map_err(|_| "ready durable Validate no-successor staging failed".to_owned())?
                    .persist_and_publish()
                    .map_err(|error| {
                        format!("ready durable Validate no-successor LedgerV1: {error}")
                    })?;
            }
            ReadyDurableValidateAdapterPublicationKind::ValidatedApply => {
                let apply = preview
                    .seal_decision_validate_apply_replay()
                    .map_err(|_| "ready durable Validate Apply replay seal failed".to_owned())?;
                coordinator
                    .prepare_sealed_validate_apply_transition(&lease, verified, apply)
                    .map_err(|_| "ready durable Validate Apply staging failed".to_owned())?
                    .persist_and_publish()
                    .map_err(|error| {
                        format!("ready durable Validate Apply publication: {error:?}")
                    })?;
            }
            ReadyDurableValidateAdapterPublicationKind::ValidatedPersist => {
                let publication = preview
                    .seal_live_wal_validate_sign()
                    .map_err(|_| "ready durable Validate Sign WAL seal failed".to_owned())?;
                coordinator
                    .prepare_sealed_validate_sign_transition(&lease, verified, publication)
                    .map_err(|_| "ready durable Validate Sign staging failed".to_owned())?
                    .persist_and_publish()
                    .map_err(|_| "ready durable Validate Sign publication failed".to_owned())?;
            }
            ReadyDurableValidateAdapterPublicationKind::RejectedReport => {
                let report = preview
                    .seal_invalid_body_report_replay()
                    .map_err(|_| "ready durable Validate report replay seal failed".to_owned())?;
                coordinator
                    .prepare_sealed_validate_report_transition(&lease, verified, report)
                    .map_err(|_| "ready durable Validate report staging failed".to_owned())?
                    .persist_and_publish()
                    .map_err(|error| {
                        format!("ready durable Validate report publication: {error:?}")
                    })?;
            }
            ReadyDurableValidateAdapterPublicationKind::ValidatedBusy
            | ReadyDurableValidateAdapterPublicationKind::RejectedBusy => {
                let wait = preview
                    .busy_wait_token(coordinator.active_context)
                    .ok_or_else(|| {
                        "ready durable Validate Busy branch lost its fence wait".to_owned()
                    })?;
                drop(preview);
                if !coordinator.settle_sealed_validate_busy(lease, wait) {
                    return Err(
                        "ready durable Validate Busy settlement changed ownership".to_owned()
                    );
                }
                operation.complete();
                return Ok(false);
            }
        }
        operation.complete();
        Ok(true)
    }

    fn advance_certified_body_pipeline(&mut self) -> Result<bool, String> {
        let reducer_fence_generation = self.executor.lifecycle_reducer_fence_generation();
        let lease = match self
            .owner
            .plan_direct_registry_turn(reducer_fence_generation)
        {
            Ok(TurnPlan::Execute(lease)) => lease,
            Ok(TurnPlan::Idle | TurnPlan::Waiting(_) | TurnPlan::FailClosed(_)) => {
                return Err("ordinary body pipeline did not produce one exact lease".to_owned());
            }
            Err(error) => return Err(format!("ordinary body scheduler census: {error:?}")),
        };
        let Some((&slot, _)) = lease.physical_slots().first_key_value() else {
            return Err("ordinary body lease lost its sole physical slot".to_owned());
        };
        if lease.work_class() == LifecycleWorkClass::Validate {
            let requires_io = self
                .owner
                .registry
                .registry_mut()
                .claimed_validate_requires_io_dispatch(&lease)
                .map_err(|error| format!("durable Validate carrier classification: {error:?}"))?;
            if requires_io {
                let key = LifecycleDurableValidateDispatchKeyV1::from_claimed(
                    self.owner.coordinator.active_context,
                    &lease,
                )
                .ok_or_else(|| "durable Validate lease lost its exact dispatch key".to_owned())?;
                let reservation = match self
                    .services
                    .capture_lifecycle_durable_validate_capacity(key)
                    .map_err(|error| {
                        format!("durable Validate worker capacity capture: {error:?}")
                    })? {
                    LifecycleDurableValidateCapacityCaptureV1::Reserved(reservation) => reservation,
                    LifecycleDurableValidateCapacityCaptureV1::Unavailable => {
                        if !Self::rollback_unpublished_body_claim(
                            &mut self.owner.coordinator,
                            &lease,
                        ) {
                            return Err(
                                "backpressured durable Validate claim did not roll back exactly"
                                    .to_owned(),
                            );
                        }
                        return Ok(false);
                    }
                };
                let dispatch = match self.owner.coordinator.begin_durable_validate_dispatch(
                    &mut self.owner.registry,
                    lease.clone(),
                    &self.owner.verified,
                ) {
                    Ok(dispatch) => dispatch,
                    Err((error, returned_lease)) => {
                        if !Self::rollback_unpublished_body_claim(
                            &mut self.owner.coordinator,
                            &returned_lease,
                        ) {
                            return Err(format!(
                                "durable Validate dispatch failed and claim rollback failed: {error:?}"
                            ));
                        }
                        reservation.cancel_uncommitted();
                        return Err(format!("durable Validate dispatch: {error:?}"));
                    }
                };
                if !reservation.preflight(&dispatch) {
                    return Err(
                        "durable Validate dispatch changed after worker reservation".to_owned()
                    );
                }
                reservation.commit(dispatch);
                return Ok(true);
            }
        }
        let output_guard = self.services.lifecycle_output_guard();
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "ordinary body publication crossed closed output".to_owned())?;
        if lease.work_class() == LifecycleWorkClass::Validate {
            return self.publish_ready_durable_validate(lease, slot, operation);
        }
        let ProductionLifecycleOwnerV1 {
            verified,
            coordinator,
            registry,
            ..
        } = &mut self.owner;
        match lease.work_class() {
            LifecycleWorkClass::Fetch => {
                let prepared = registry
                    .registry_mut()
                    .prepare_certified_fetch_execution(&lease, slot)
                    .map_err(|error| format!("certified Fetch execution: {error:?}"))?;
                let (tag, manifest) = prepared.adapter_preview_inputs();
                let adapter = self
                    .executor
                    .prepare_certified_fetch_store_adapter(tag, manifest)
                    .map_err(|error| format!("certified Fetch adapter preview: {error}"))?;
                let successor = prepared
                    .seal_store_successor(adapter.store_effect())
                    .map_err(|error| format!("certified Fetch Store seal: {error:?}"))?;
                let transition = coordinator
                    .prepare_sealed_fetch_store_transition(&lease, verified, successor)
                    .map_err(|error| format!("certified Fetch Store transition: {error:?}"))?;
                transition
                    .persist_exact_successor()
                    .map_err(|error| format!("certified Fetch Store LedgerV1: {error}"))?;
                transition.commit_after_publication();
                adapter.commit_after_durable_settlement();
            }
            LifecycleWorkClass::Store => {
                let prepared = registry
                    .registry_mut()
                    .prepare_durable_store_execution(&lease, slot, verified)
                    .map_err(|error| format!("durable Store execution: {error:?}"))?;
                let (tag, round, subject) = prepared.adapter_preview_inputs();
                let adapter = self
                    .executor
                    .prepare_durable_store_validate_adapter(
                        tag,
                        round,
                        subject,
                        prepared.durable_body_receipt(),
                    )
                    .map_err(|error| format!("durable Store adapter preview: {error}"))?;
                let successor = prepared
                    .seal_validate_successor(adapter.validate_effect())
                    .map_err(|error| format!("durable Store Validate seal: {error:?}"))?;
                let transition = coordinator
                    .prepare_sealed_store_validate_transition(&lease, verified, successor)
                    .map_err(|error| format!("durable Store Validate transition: {error:?}"))?;
                transition
                    .persist_exact_successor()
                    .map_err(|error| format!("durable Store Validate LedgerV1: {error}"))?;
                transition.commit_after_publication();
                adapter.commit_after_durable_settlement();
            }
            _ => return Err("ordinary body scheduler selected a foreign work class".to_owned()),
        }
        operation.complete();
        Ok(true)
    }

    fn rollback_unpublished_body_claim(
        coordinator: &mut LifecycleCoordinator,
        lease: &TurnLease,
    ) -> bool {
        match lease.output_reservation() {
            Some(reservation) => {
                coordinator.rollback_unpublished_reserved_turn(lease, reservation.class())
            }
            None => coordinator.rollback_unpublished_turn(lease),
        }
    }

    fn drive_recovered_ingress_selector<'cursor>(
        &mut self,
        selector: PreparedLifecycleIngressSelector,
        runner: LifecycleCurrentRunnerTurn<'cursor>,
    ) -> ProductionLifecycleIngressTurnV1<'cursor> {
        let result = self
            .owner
            .persist_recovered_decision_fetch_response_after_runner(
                &self.services,
                &mut self.executor,
                selector,
                &runner,
            );
        let selected = match result {
            Ok(ProductionRecoveredDecisionFetchPersistenceV1::CapacityWait(wait)) => {
                assert!(self.pending_ingress_capacity.is_none());
                self.pending_ingress_capacity =
                    Some(PendingIngressCapacityV1::RecoveredDecisionFetch(wait));
                ProductionLifecycleIngressSelectionV1::RecoveredDecisionFetchCapacityPending
            }
            Ok(ProductionRecoveredDecisionFetchPersistenceV1::Queued { ordinal }) => {
                if self
                    .owner
                    .coordinator
                    .active_lease
                    .as_ref()
                    .map(|lease| lease.ordinal())
                    != Some(ordinal)
                {
                    iroha_logger::error!(
                        ordinal,
                        "queued recovered Fetch lost its active lifecycle lease"
                    );
                    self.close_output_for_restart();
                    ProductionLifecycleIngressSelectionV1::RestartRequired
                } else {
                    ProductionLifecycleIngressSelectionV1::RecoveredDecisionFetchQueued
                }
            }
            Err(ProductionRecoveredDecisionFetchPersistenceErrorV1::CommandPreparation {
                failure,
                prepared,
            }) => {
                let reason = failure.detail();
                iroha_logger::warn!(
                    %reason,
                    "recovered Fetch persistence preparation retained its selector for retry"
                );
                drop(prepared);
                ProductionLifecycleIngressSelectionV1::RecoveredDecisionFetchPreparationRetry
            }
            Err(ProductionRecoveredDecisionFetchPersistenceErrorV1::CompetingReadyWork(
                prepared,
            )) => {
                drop(prepared);
                ProductionLifecycleIngressSelectionV1::RecoveredDecisionFetchCompetingReady
            }
            Err(ProductionRecoveredDecisionFetchPersistenceErrorV1::InFlightSelectedWork(
                prepared,
            )) => {
                drop(prepared);
                iroha_logger::error!(
                    "externally Waiting recovered Fetch found an unauthenticated in-flight worker key"
                );
                self.close_output_for_restart();
                ProductionLifecycleIngressSelectionV1::RestartRequired
            }
            Err(ProductionRecoveredDecisionFetchPersistenceErrorV1::Service {
                failure,
                prepared,
            }) => {
                let reason = match failure {
                    crate::sumeragi::v2_worker::LifecycleIoCapacityCaptureFailure::InvalidTarget => {
                        "invalid persistence target"
                    }
                    crate::sumeragi::v2_worker::LifecycleIoCapacityCaptureFailure::ForeignContext => {
                        "foreign lifecycle context"
                    }
                    crate::sumeragi::v2_worker::LifecycleIoCapacityCaptureFailure::OutputClosed => {
                        "consensus output closed"
                    }
                    crate::sumeragi::v2_worker::LifecycleIoCapacityCaptureFailure::Disconnected => {
                        "lifecycle I/O worker disconnected"
                    }
                    crate::sumeragi::v2_worker::LifecycleIoCapacityCaptureFailure::PositionOverflow => {
                        "persistence queue position overflow"
                    }
                    crate::sumeragi::v2_worker::LifecycleIoCapacityCaptureFailure::GenerationExhausted => {
                        "capacity generation exhausted"
                    }
                };
                iroha_logger::error!(reason, "recovered Fetch capacity capture failed closed");
                drop(prepared);
                self.close_output_for_restart();
                ProductionLifecycleIngressSelectionV1::RestartRequired
            }
            Err(ProductionRecoveredDecisionFetchPersistenceErrorV1::Claim(error)) => {
                let reason = match error {
                    crate::sumeragi::v2_effects::RecoveredDecisionFetchResponseClaimErrorV1::InvalidOwnerIndex => {
                        "invalid request owner index"
                    }
                    crate::sumeragi::v2_effects::RecoveredDecisionFetchResponseClaimErrorV1::ForeignOwner => {
                        "foreign request owner"
                    }
                    crate::sumeragi::v2_effects::RecoveredDecisionFetchResponseClaimErrorV1::ConflictingClaim => {
                        "conflicting response-family claim"
                    }
                };
                iroha_logger::error!(reason, "recovered Fetch claim failed closed");
                self.close_output_for_restart();
                ProductionLifecycleIngressSelectionV1::RestartRequired
            }
            Err(
                ProductionRecoveredDecisionFetchPersistenceErrorV1::InvalidClaimedCarrier
                | ProductionRecoveredDecisionFetchPersistenceErrorV1::ForeignOwner
                | ProductionRecoveredDecisionFetchPersistenceErrorV1::InvalidReservedCommand,
            ) => {
                self.close_output_for_restart();
                ProductionLifecycleIngressSelectionV1::RestartRequired
            }
        };
        drop(runner);
        ProductionLifecycleIngressTurnV1::Selected(selected)
    }

    fn drive_certified_fetch_ingress_selector<'cursor>(
        &mut self,
        selector: PreparedLifecycleIngressSelector,
        runner: LifecycleCurrentRunnerTurn<'cursor>,
    ) -> ProductionLifecycleIngressTurnV1<'cursor> {
        let mode = self.executor.lifecycle_mode_rank_snapshot();
        let result =
            self.owner
                .plan_ingress_turn(&self.services, &self.executor, mode, selector, runner);
        let selected = match result {
            Ok(ProductionIngressTurnPreparation::CapacityWait(wait)) => {
                assert!(self.pending_ingress_capacity.is_none());
                self.pending_ingress_capacity =
                    Some(PendingIngressCapacityV1::CertifiedFetch(wait));
                ProductionLifecycleIngressSelectionV1::CertifiedFetchCapacityPending
            }
            Ok(ProductionIngressTurnPreparation::Queued(_queued)) => {
                ProductionLifecycleIngressSelectionV1::CertifiedFetchQueued
            }
            Err(
                ProductionIngressSchedulerInputsError::UnsettledLease { .. }
                | ProductionIngressSchedulerInputsError::CompetingReadyWork,
            ) => ProductionLifecycleIngressSelectionV1::CertifiedFetchCompetingReady,
            Err(
                ProductionIngressSchedulerInputsError::StaleModeObservation
                | ProductionIngressSchedulerInputsError::CommandPreparation { .. }
                | ProductionIngressSchedulerInputsError::InFlightSelectedWork { .. }
                | ProductionIngressSchedulerInputsError::Service { .. },
            ) => ProductionLifecycleIngressSelectionV1::CertifiedFetchRetry,
            Err(
                ProductionIngressSchedulerInputsError::CoordinatorFaulted { .. }
                | ProductionIngressSchedulerInputsError::ForeignModeObservation
                | ProductionIngressSchedulerInputsError::ForeignOutputGuard
                | ProductionIngressSchedulerInputsError::ForeignRunnerObservation
                | ProductionIngressSchedulerInputsError::BodyStoreNotBound
                | ProductionIngressSchedulerInputsError::InvalidSelectedCarrier
                | ProductionIngressSchedulerInputsError::InvalidReservedCommand
                | ProductionIngressSchedulerInputsError::UnexpectedPlan
                | ProductionIngressSchedulerInputsError::SettlementFault { .. },
            ) => {
                self.close_output_for_restart();
                ProductionLifecycleIngressSelectionV1::RestartRequired
            }
        };
        ProductionLifecycleIngressTurnV1::Selected(selected)
    }

    fn settle_parked_recovered_sign_completion(
        &mut self,
    ) -> ProductionRecoveredLifecycleSignCompletionSelectionV1 {
        let Some(class) = self.classify_parked_recovered_sign_completion() else {
            self.close_output_for_restart();
            return ProductionRecoveredLifecycleSignCompletionSelectionV1::RestartRequired;
        };
        match class {
            crate::sumeragi::v2::RecoveredLifecycleSignAdapterSettlementFamilyV1::Broadcast => {
                ProductionRecoveredLifecycleSignCompletionSelectionV1::Broadcast(
                    self.settle_recovered_lifecycle_sign_broadcast(),
                )
            }
            crate::sumeragi::v2::RecoveredLifecycleSignAdapterSettlementFamilyV1::ProposalPrepareWal => {
                ProductionRecoveredLifecycleSignCompletionSelectionV1::ProposalPrepareWal(
                    self.settle_recovered_lifecycle_proposal_prepare_wal(),
                )
            }
            crate::sumeragi::v2::RecoveredLifecycleSignAdapterSettlementFamilyV1::VoteBroadcastAndSign => {
                ProductionRecoveredLifecycleSignCompletionSelectionV1::VoteBroadcastAndSign(
                    self.settle_recovered_lifecycle_vote_broadcast_and_sign(),
                )
            }
            crate::sumeragi::v2::RecoveredLifecycleSignAdapterSettlementFamilyV1::ProposalBroadcastAndSign => {
                ProductionRecoveredLifecycleSignCompletionSelectionV1::ProposalBroadcastAndSign(
                    self.settle_recovered_lifecycle_proposal_broadcast_and_sign(),
                )
            }
        }
    }

    fn classify_parked_recovered_sign_completion(
        &mut self,
    ) -> Option<crate::sumeragi::v2::RecoveredLifecycleSignAdapterSettlementFamilyV1> {
        let completion = self
            .pending_lifecycle_completion
            .as_ref()?
            .recovered_sign()?;
        let authority = completion.project_adapter_completion_authority()?;
        let preview = self
            .executor
            .prepare_recovered_lifecycle_sign_completion(authority)
            .ok()?;
        let class = preview.settlement_family();
        drop(preview);
        class
    }

    fn runner_turn_matches(
        &self,
        runner: &LifecycleCurrentRunnerTurn<'_>,
        target: crate::sumeragi::v2_runner::LifecycleRunnerRankTarget,
    ) -> bool {
        let context = self.owner.verified.context();
        runner.target() == target
            && runner.height() == context.height
            && runner.context_id() == context.id()
    }

    #[track_caller]
    fn close_output_for_restart(&self) {
        self.services
            .lifecycle_output_guard()
            .close_admission_for_restart();
    }
}

impl ActivatedProductionLifecycleV1 {
    /// Claim the oldest lifecycle-owned ProducerTurn at the exact bounded
    /// local-proposal point held by the serialized runner.
    pub(in crate::sumeragi) fn claim_producer_turn_for_local_proposal(
        &mut self,
        _runner: &mut crate::sumeragi::v2_runner::ProductionLifecycleActiveRunnerBorrowV1,
    ) -> Result<
        Option<super::super::work_registry::ClaimedProducerTurnV1>,
        super::super::scheduler_inputs::ProducerTurnSchedulerClaimErrorV1,
    > {
        let mode = self.launched.executor.lifecycle_mode_rank_snapshot();
        self.launched
            .owner
            .claim_producer_turn_at_bounded_producer_point(&mode)
    }

    /// Durably terminalize one ProducerTurn after its single bounded local
    /// proposal attempt returned successfully.
    pub(in crate::sumeragi) fn settle_producer_turn_after_local_proposal(
        &mut self,
        _runner: &mut crate::sumeragi::v2_runner::ProductionLifecycleActiveRunnerBorrowV1,
        attempted: super::super::work_registry::AttemptedProducerTurnV1,
    ) -> Result<(), super::super::projection::ProducerTurnTerminalSettlementErrorV1> {
        self.launched.owner.settle_producer_turn_advanced(attempted)
    }

    /// Forward one Completion turn without exposing the launched stack.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::sumeragi) fn drive_completion_turn<'cursor>(
        &mut self,
        runner: LifecycleCurrentRunnerTurn<'cursor>,
        lane_work: &mut V2LaneWorkAdapter,
    ) -> ProductionLifecycleCompletionTurnV1<'cursor> {
        self.launched.drive_completion_turn(runner, lane_work)
    }

    /// Classify parked and physical Completion owners without claiming fresh Ready work.
    pub(in crate::sumeragi) fn drive_completion_pre_gate<'cursor>(
        &mut self,
        runner: LifecycleCurrentRunnerTurn<'cursor>,
        lane_work: &mut V2LaneWorkAdapter,
    ) -> ProductionLifecycleCompletionPreGateV1<'cursor> {
        self.launched.drive_completion_pre_gate(runner, lane_work)
    }

    /// Consume a physically empty Completion cursor through fresh Ready dispatch.
    pub(in crate::sumeragi) fn drive_ready_completion_turn<'cursor>(
        &mut self,
        ready: ProductionLifecycleReadyCompletionTurnV1<'cursor>,
    ) -> ProductionLifecycleCompletionTurnV1<'cursor> {
        self.launched.drive_ready_completion_turn(ready)
    }

    /// Forward one Ingress turn without exposing the launched stack.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::sumeragi) fn drive_ingress_turn<'cursor>(
        &mut self,
        runner: LifecycleCurrentRunnerTurn<'cursor>,
    ) -> ProductionLifecycleIngressTurnV1<'cursor> {
        self.launched.drive_ingress_turn(runner)
    }

    /// Consume one opaque ordinary handoff through the exact runner-owned tail.
    ///
    /// The activated lifecycle supplies only its retained ingress, executor,
    /// and services. Every lane, Kura, historical-Serve, block-sync, and NPoS
    /// dependency remains borrowed from the serialized runner, and no token
    /// field or callback crosses this boundary.
    #[cfg_attr(not(test), allow(dead_code))]
    #[allow(clippy::too_many_arguments)]
    pub(in crate::sumeragi) fn consume_prepared_ordinary_ingress_turn(
        &mut self,
        _runner: &mut crate::sumeragi::v2_runner::ProductionLifecycleActiveRunnerBorrowV1,
        mut turn: ProductionPreparedOrdinaryIngressTurnV1,
        lane_work: &mut V2LaneWorkAdapter,
        kura: &Kura,
        local_key: &KeyPair,
        block_sync_server: &mut crate::sumeragi::v2_block_sync::V2BlockSyncServer,
        block_sync: &mut crate::sumeragi::v2_block_sync::V2BlockSyncDiscovery,
        block_sync_request: &mut Option<
            iroha_crypto::HashOf<iroha_data_model::block::consensus_v2::CommitCertificateRequest>,
        >,
        npos_vrf: &mut crate::sumeragi::v2_npos::V2NposVrfLifecycle,
    ) -> Result<
        ProductionPreparedOrdinaryIngressConsumptionV1,
        crate::sumeragi::v2_runner::V2RunnerError,
    > {
        let Some(handoff) = turn.handoff.take() else {
            self.launched.close_output_for_restart();
            return Err(crate::sumeragi::v2_runner::V2RunnerError::Service(
                "ordinary ingress token lost its exact runner handoff".to_owned(),
            ));
        };
        let LaunchedProductionLifecycleV1 {
            executor,
            services,
            leader_wire_ingress_binding,
            ..
        } = &mut self.launched;
        consume_prepared_dequeued_v2_ingress(
            handoff,
            &leader_wire_ingress_binding.ingress,
            executor,
            services,
            lane_work,
            kura,
            local_key,
            block_sync_server,
            block_sync,
            block_sync_request,
            npos_vrf,
        )
    }

    /// Hold the sole test auxiliary admission unit through one ingress drive.
    #[cfg(test)]
    pub(in crate::sumeragi) fn hold_auxiliary_io_admission_for_test(
        &self,
    ) -> Result<crate::sumeragi::v2_worker::ProductionAuxiliaryIoAdmissionHoldV1, String> {
        self.launched
            .services
            .hold_auxiliary_io_admission_for_test()
    }

    /// Settle one prepared current-height Serve handoff without runner output.
    #[cfg(test)]
    pub(in crate::sumeragi) fn settle_prepared_certified_serve_for_test(
        &mut self,
        _runner: &mut crate::sumeragi::v2_runner::ProductionLifecycleActiveRunnerBorrowV1,
        mut turn: ProductionPreparedOrdinaryIngressTurnV1,
    ) -> Result<ProductionPreparedCertifiedServeTestSettlementV1, String> {
        let Some(handoff) = turn.handoff.take() else {
            self.launched.close_output_for_restart();
            return Err("prepared Serve token lost its exact runner handoff".to_owned());
        };
        settle_prepared_certified_serve_for_test(handoff, &mut self.launched.services)
    }
}

#[cfg(test)]
mod ordinary_ingress_token_tests {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    use iroha_crypto::KeyPair;
    use iroha_data_model::peer::PeerId;

    use super::*;

    #[test]
    fn armed_token_closes_output_before_releasing_dequeued_carrier_and_serve_result() {
        let peer = PeerId::from(KeyPair::random().public_key().clone());
        let ingress = Arc::new(FairV2Ingress::new(7, 1024 * 1024, 512 * 1024, 0, 0));
        ingress
            .configure_roster([peer.clone()])
            .expect("configure one exact validator lane");
        ingress.open().expect("open exact ordinary-token ingress");
        let message = crate::sumeragi::v2_worker::tests::lane_commit_qc_block_message(peer.clone());
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::from_authenticated_peer(message, peer)),
            Ok(crate::sumeragi::FairV2IngressPushDisposition::Enqueued)
        ));
        let cut = ingress
            .capture_next_ingress_turn_cut(|_| true)
            .expect("capture exact ordinary winner")
            .expect("ordinary winner exists");
        let (inbound, disposition) = cut
            .dequeue_exact_retaining()
            .unwrap_or_else(|_| panic!("dequeue the exact selected ordinary winner"));
        let output_guard = ConsensusOutputGuard::isolated();
        let observed = Arc::new(AtomicBool::new(false));
        let mut turn = prepared_ordinary_ingress_turn(
            Arc::clone(&ingress),
            inbound,
            disposition,
            Some(ProductionPreparedCertifiedServeV1::Rejected(
                "durable negative".to_owned(),
            )),
            None,
            Arc::clone(&output_guard),
        );
        turn.drop_order_probe = Some(PreparedOrdinaryIngressDropOrderProbe {
            output_guard: Arc::clone(&output_guard),
            observed: Arc::clone(&observed),
        });
        assert_eq!(turn.physical_ordinal_for_test(), 1);
        assert!(turn.has_prepared_serve_for_test());
        assert!(!output_guard.restart_required());

        drop(turn);

        assert!(output_guard.restart_required());
        assert!(observed.load(Ordering::Acquire));
        assert_eq!(ingress.len(), 0, "the token owns the already-drained row");
    }
}
