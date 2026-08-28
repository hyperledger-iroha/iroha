//! Unified lifecycle ownership for one outer Completion or Ingress turn.

use super::super::{
    ingress_position::FairIngressQueueCutError, selector::CertifiedServeExactDequeueErrorV1,
};
use super::*;
use crate::sumeragi::v2_lifecycle_coordinator::{
    AdmissionDecision, CertifiedServeSchedulerObservationV1, LifecycleIngressSelectorError,
    LifecycleLedgerV1, LifecycleValidateSidecarDriveV1, ProductionIngressCapacityStatus,
    ReadyValidateSuccessorDispatchV1, SelectedCertifiedResponsePriorityV1, WaitSource, WaitToken,
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
use crate::sumeragi::v2_runtime::PreTimeoutLockedPrepareQcCutV1;

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
    /// Certified progress superseded the exact old signer fence and its
    /// claimed lifecycle row was durably cancelled.
    Superseded,
    /// Active serialized-runtime mutation debt retained the guarded completion.
    Retry,
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
            Self::Superseded | Self::Retry => false,
            Self::RestartRequired => true,
        }
    }
}

enum ParkedRecoveredLifecycleSignCompletionClassV1 {
    Settlement(crate::sumeragi::v2::RecoveredLifecycleSignAdapterSettlementFamilyV1),
    Superseded,
    Retry,
    RestartRequired,
}

/// Closed diagnostic for one lifecycle-selected Completion turn.
///
/// Guarded completions, deferred Apply state, and capacity waits remain inside
/// the launched owner. This enum contains no request, result, selector, queue,
/// executor, service, or acknowledgement authority.
#[allow(variant_size_differences)]
#[must_use = "the lifecycle-selected Completion result must be observed"]
pub(in crate::sumeragi) enum ProductionLifecycleCompletionSelectionV1 {
    /// One executed lifecycle Validate published its exact Ready replacement.
    LifecycleValidatePublished {
        /// Exact lifecycle ordinal whose executed Validate carrier became Ready.
        ordinal: u128,
    },
    /// A missing-sidecar Validate remains parked under its immutable registration owner.
    LifecycleValidateDeferred,
    /// A registered sidecar wait is externally parked and ordinary ingress may resume.
    LifecycleValidateSidecarWaiting,
    /// The exact sidecar became durable and woke the same Validate row without a new ordinal.
    LifecycleValidateSidecarWoken {
        /// Exact lifecycle ordinal of the same sidecar-backed Ready row.
        ordinal: u128,
    },
    /// The exact retained Validate successor could not reserve worker/output capacity.
    LifecycleValidateSuccessorCapacityPending {
        /// Exact unchanged successor ordinal.
        ordinal: u128,
    },
    /// The exact retained Validate successor remains parked on its reducer fence.
    LifecycleValidateSuccessorFencePending {
        /// Exact waiting successor ordinal.
        ordinal: u128,
        /// Exact reducer-fence generation which still owns the retry.
        wait: WaitToken,
    },
    /// The exact missing-sidecar Apply owner remains parked for another turn.
    LifecycleDecisionApplyDeferred,
    /// The unchanged deferred Apply command re-entered its dedicated FIFO.
    LifecycleDecisionApplyRequeued,
    /// Deferred Apply ownership changed and process restart is required.
    LifecycleDecisionApplyRestartRequired,
    /// One lifecycle Decision Apply worker result was durably settled.
    LifecycleDecisionApplyApplied,
    /// One lifecycle Decision Apply worker result became the retained sidecar owner.
    LifecycleDecisionApplyCompletionDeferred,
    /// Lifecycle Decision Apply settlement requires cold restart.
    LifecycleDecisionApplyCompletionRestartRequired,
    /// One parked recovered Sign used exactly one successor-family settler.
    RecoveredLifecycleSignCompletion(ProductionRecoveredLifecycleSignCompletionSelectionV1),
    /// One complete lifecycle Apply/recovered Sign/Fetch census used a joint physical cut.
    CompletionIoDispatch(
        Result<ProductionCompletionDispatchV1, ProductionCompletionDispatchErrorV1>,
    ),
    /// One parked recovered Decision Fetch body entered its Store settlement.
    RecoveredDecisionFetchCompletion(ProductionRecoveredDecisionFetchStoreSettlementV1),
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
    /// One durable recovered Broadcast entered its typed refanout transaction.
    RecoveredLifecycleBroadcastRefanout(
        Result<
            ProductionRecoveredLifecycleSignedBroadcastRefanoutV1,
            ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1,
        >,
    ),
    /// One exact direct Broadcast was accepted and durably terminalized after Apply.
    ApplyTerminalDirectBroadcastCompleted,
    /// The exact post-Apply direct Broadcast retained its source for retry.
    ApplyTerminalDirectBroadcastDeferred,
    /// An exact lifecycle completion was selected but its service owner changed.
    RestartRequired,
}

impl ProductionLifecycleCompletionSelectionV1 {
    /// Return whether the consumed Completion turn requires a cold restart.
    pub(in crate::sumeragi) fn restart_required(&self) -> bool {
        match self {
            Self::LifecycleDecisionApplyRestartRequired
            | Self::LifecycleDecisionApplyCompletionRestartRequired
            | Self::CertifiedFetchBodyPersistenceRestartRequired
            | Self::RestartRequired => true,
            Self::RecoveredLifecycleSignCompletion(selection) => selection.restart_required(),
            Self::CompletionIoDispatch(result) => result.is_err(),
            Self::RecoveredDecisionFetchCompletion(settlement) => matches!(
                settlement,
                ProductionRecoveredDecisionFetchStoreSettlementV1::None
                    | ProductionRecoveredDecisionFetchStoreSettlementV1::RestartRequired
            ),
            Self::RecoveredLifecycleBroadcastRefanout(result) => matches!(
                result,
                Err(_) | Ok(ProductionRecoveredLifecycleSignedBroadcastRefanoutV1::RestartRequired)
            ),
            Self::LifecycleDecisionApplyDeferred
            | Self::LifecycleDecisionApplyRequeued
            | Self::LifecycleDecisionApplyApplied
            | Self::LifecycleDecisionApplyCompletionDeferred
            | Self::ApplyTerminalDirectBroadcastCompleted
            | Self::ApplyTerminalDirectBroadcastDeferred
            | Self::LifecycleValidatePublished { .. }
            | Self::LifecycleValidateDeferred
            | Self::LifecycleValidateSidecarWaiting
            | Self::LifecycleValidateSidecarWoken { .. }
            | Self::LifecycleValidateSuccessorCapacityPending { .. }
            | Self::LifecycleValidateSuccessorFencePending { .. }
            | Self::CertifiedFetchBodyPersisted
            | Self::CertifiedFetchBodyPersistenceRetry
            | Self::CertifiedServeClaimedCompleted
            | Self::CertifiedServeReplayCompleted => false,
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

/// Closed Ready-work class permitted after the exact Apply terminal.
///
/// General Completion I/O passes through to the terminal Runtime fence. Only an authenticated
/// recovered Broadcast may mutate the lifecycle owner, while an invalid census requires restart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ProductionApplyTerminalReadyWorkV1 {
    /// No post-Apply output handoff owns this Completion cursor.
    PassThrough,
    /// One retained direct Broadcast must use its exact pending-output owner.
    RetainedDirectOutput,
    /// One authenticated recovered Broadcast may enter its exact-output wait.
    RecoveredLifecycleBroadcast,
    /// The Ready census is invalid and the lifecycle must restart.
    RestartRequired,
}

/// Narrow the complete Ready census to work legal after Apply terminal settlement.
pub(super) const fn classify_apply_terminal_ready_work(
    ready_work: super::super::ProductionCompletionReadyWorkV1,
) -> ProductionApplyTerminalReadyWorkV1 {
    match ready_work {
        super::super::ProductionCompletionReadyWorkV1::RecoveredLifecycleBroadcast => {
            ProductionApplyTerminalReadyWorkV1::RecoveredLifecycleBroadcast
        }
        super::super::ProductionCompletionReadyWorkV1::RetainedDirectOutput => {
            ProductionApplyTerminalReadyWorkV1::RetainedDirectOutput
        }
        super::super::ProductionCompletionReadyWorkV1::Invalid => {
            ProductionApplyTerminalReadyWorkV1::RestartRequired
        }
        super::super::ProductionCompletionReadyWorkV1::None
        | super::super::ProductionCompletionReadyWorkV1::PassThrough
        | super::super::ProductionCompletionReadyWorkV1::CompletionIo => {
            ProductionApplyTerminalReadyWorkV1::PassThrough
        }
    }
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
    /// Certified-response classification or Phase A retained the queue occurrence for retry.
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

/// Closed fair-ingress result under one runtime-frozen pre-timeout cut.
#[must_use = "the fixed-cut pre-timeout ingress result must be consumed"]
pub(in crate::sumeragi) enum ProductionPreTimeoutLockedPrepareQcIngressTurnV1 {
    /// No pre-cut exact target or ordinary obsolete predecessor is selectable.
    Empty,
    /// One WAL-obsolete pre-cut predecessor crossed the ordinary dequeue tail.
    ObsoletePredecessor(ProductionPreparedOrdinaryIngressTurnV1),
    /// One exact deeply-previewed Prepare vote/QC carrier crossed the ordinary
    /// dequeue tail.
    ExactPrepareProgress(ProductionPreparedOrdinaryIngressTurnV1),
    /// Queue ownership or exact dequeue authority failed closed.
    RestartRequired,
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
    let cut = match cut.fence_producer_publication_retaining() {
        Ok(cut) => cut,
        Err((FairIngressQueueCutError::QueueCutChanged, retained)) => {
            iroha_logger::debug!(
                "Certified-Serve fair-ingress ownership changed before authentication; retrying"
            );
            drop(retained);
            drop(runner);
            return ProductionLifecycleIngressTurnV1::Selected(
                ProductionLifecycleIngressSelectionV1::CertifiedServeRetry,
            );
        }
        Err((error, retained)) => {
            iroha_logger::error!(
                ?error,
                "Certified-Serve fair-ingress publication fence failed closed"
            );
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
        Err((FairIngressQueueCutError::QueueCutChanged, retained)) => {
            iroha_logger::debug!(
                "Certified-Serve fair-ingress ownership changed during authentication; retrying"
            );
            drop(retained);
            drop(runner);
            return ProductionLifecycleIngressTurnV1::Selected(
                ProductionLifecycleIngressSelectionV1::CertifiedServeRetry,
            );
        }
        Err((error, retained)) => {
            iroha_logger::error!(
                ?error,
                "authenticated current Certified-Serve cut failed structural narrowing"
            );
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
    let selector = match executor.capture_fenced_certified_serve_ingress_selector(lifecycle_cut) {
        Ok(selector) => selector,
        Err(LifecycleIngressSelectorError::QueueCutChanged) => {
            iroha_logger::debug!(
                "Certified-Serve fair-ingress census changed during classification; retrying"
            );
            drop(runner);
            return ProductionLifecycleIngressTurnV1::Selected(
                ProductionLifecycleIngressSelectionV1::CertifiedServeRetry,
            );
        }
        Err(error) => {
            let reason = error.detail();
            iroha_logger::error!(
                %reason,
                "authenticated current Certified-Serve selector capture failed closed"
            );
            services
                .lifecycle_output_guard()
                .close_admission_for_restart();
            drop(runner);
            return ProductionLifecycleIngressTurnV1::Selected(
                ProductionLifecycleIngressSelectionV1::RestartRequired,
            );
        }
    };
    let (dequeue, target) = match selector.into_locked_certified_serve_dequeue(&authenticated) {
        Ok(prepared) => prepared,
        Err(CertifiedServeExactDequeueErrorV1::Queue(
            FairIngressQueueCutError::QueueCutChanged,
        )) => {
            iroha_logger::debug!(
                "Certified-Serve fair-ingress census changed before exact dequeue; retrying"
            );
            drop(runner);
            return ProductionLifecycleIngressTurnV1::Selected(
                ProductionLifecycleIngressSelectionV1::CertifiedServeRetry,
            );
        }
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

enum PreparedOrdinaryIngressDequeueV1 {
    Prepared(ProductionPreparedOrdinaryIngressTurnV1),
    RestartRequired,
}

fn prepare_ordinary_ingress_dequeue(
    receiver: &std::sync::Arc<FairV2Ingress>,
    cut: FairIngressTurnCut<'_>,
    prepared_serve: Option<ProductionPreparedCertifiedServeV1>,
    terminal_subject: Option<iroha_data_model::block::consensus_v2::BlockSubject>,
    services: &ProductionV2Services,
) -> PreparedOrdinaryIngressDequeueV1 {
    let output_guard = services.lifecycle_output_guard();
    let Some(operation) = output_guard.begin_fail_stop_operation() else {
        drop(cut);
        return PreparedOrdinaryIngressDequeueV1::RestartRequired;
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
            PreparedOrdinaryIngressDequeueV1::Prepared(turn)
        }
        Err((error, retained)) => {
            iroha_logger::error!(
                ?error,
                "Sumeragi v2 ordinary ingress exact dequeue failed closed"
            );
            drop(operation);
            drop(retained);
            PreparedOrdinaryIngressDequeueV1::RestartRequired
        }
    }
}

fn dequeue_prepared_ordinary_ingress<'cursor>(
    receiver: &std::sync::Arc<FairV2Ingress>,
    cut: FairIngressTurnCut<'_>,
    runner: LifecycleCurrentRunnerTurn<'cursor>,
    prepared_serve: Option<ProductionPreparedCertifiedServeV1>,
    terminal_subject: Option<iroha_data_model::block::consensus_v2::BlockSubject>,
    services: &ProductionV2Services,
) -> ProductionLifecycleIngressTurnV1<'cursor> {
    let prepared =
        prepare_ordinary_ingress_dequeue(receiver, cut, prepared_serve, terminal_subject, services);
    drop(runner);
    match prepared {
        PreparedOrdinaryIngressDequeueV1::Prepared(turn) => {
            ProductionLifecycleIngressTurnV1::Ordinary(turn)
        }
        PreparedOrdinaryIngressDequeueV1::RestartRequired => {
            ProductionLifecycleIngressTurnV1::Selected(
                ProductionLifecycleIngressSelectionV1::RestartRequired,
            )
        }
    }
}

impl LaunchedProductionLifecycleV1 {
    fn drive_registered_lifecycle_validate_sidecar(
        &mut self,
        registration: RegisteredLifecycleValidateSidecarWaitV1,
        lane_work: &mut V2LaneWorkAdapter,
    ) -> ProductionLifecycleCompletionSelectionV1 {
        match registration.drive(&mut self.owner.coordinator, &self.owner.registry, lane_work) {
            LifecycleValidateSidecarDriveV1::Waiting(registration) => {
                assert!(self.pending_lifecycle_completion.is_none());
                self.pending_lifecycle_completion = Some(
                    PendingLifecycleCompletionV1::RegisteredDeferredValidate(registration),
                );
                ProductionLifecycleCompletionSelectionV1::LifecycleValidateSidecarWaiting
            }
            LifecycleValidateSidecarDriveV1::Woken(successor) => {
                let ordinal = successor.lifecycle_ordinal();
                assert!(self.pending_lifecycle_completion.is_none());
                self.pending_lifecycle_completion = Some(
                    PendingLifecycleCompletionV1::ReadyValidateSuccessor(successor),
                );
                ProductionLifecycleCompletionSelectionV1::LifecycleValidateSidecarWoken { ordinal }
            }
            LifecycleValidateSidecarDriveV1::RestartRequired(error) => {
                iroha_logger::error!(
                    %error,
                    "lifecycle Validate sidecar registration failed closed"
                );
                self.close_output_for_restart();
                ProductionLifecycleCompletionSelectionV1::RestartRequired
            }
        }
    }

    fn register_and_drive_lifecycle_validate_sidecar(
        &mut self,
        completion: PreparedDeferredLifecycleValidateCompletionV1,
        lane_work: &mut V2LaneWorkAdapter,
    ) -> ProductionLifecycleCompletionSelectionV1 {
        let registration = match RegisteredLifecycleValidateSidecarWaitV1::register_live(
            &self.owner.coordinator,
            &self.owner.registry,
            completion,
        ) {
            Ok(registration) => registration,
            Err((error, completion)) => {
                iroha_logger::error!(
                    %error,
                    "lifecycle Validate sidecar registration could not cross its durable boundary"
                );
                drop(completion);
                self.close_output_for_restart();
                return ProductionLifecycleCompletionSelectionV1::RestartRequired;
            }
        };
        self.drive_registered_lifecycle_validate_sidecar(registration, lane_work)
    }

    fn settle_parked_lifecycle_validate_completion(
        &mut self,
    ) -> ProductionLifecycleCompletionSelectionV1 {
        let Self {
            owner,
            services,
            pending_lifecycle_completion,
            ..
        } = self;
        let Some(completion) =
            PendingLifecycleCompletionV1::take_validate(pending_lifecycle_completion)
        else {
            services
                .lifecycle_output_guard()
                .close_admission_for_restart();
            return ProductionLifecycleCompletionSelectionV1::RestartRequired;
        };
        let (dispatch, ack) = completion.into_publication_parts();
        match owner.coordinator.complete_durable_validate_dispatch(
            &mut owner.registry,
            dispatch,
        ) {
            Ok(crate::sumeragi::v2_lifecycle_coordinator::DurableValidateCompletionPublication::PublishedValidated(
                published,
            )) => {
                let ordinal = published.lifecycle_ordinal();
                assert!(pending_lifecycle_completion.is_none());
                *pending_lifecycle_completion = Some(
                    PendingLifecycleCompletionV1::ReadyValidateSuccessor(
                        ReadyValidateSuccessorV1::from_validated(published),
                    ),
                );
                ack.acknowledge_after_publication();
                ProductionLifecycleCompletionSelectionV1::LifecycleValidatePublished { ordinal }
            }
            Ok(crate::sumeragi::v2_lifecycle_coordinator::DurableValidateCompletionPublication::PublishedRejected(
                published,
            )) => {
                let ordinal = published.lifecycle_ordinal();
                assert!(pending_lifecycle_completion.is_none());
                *pending_lifecycle_completion = Some(
                    PendingLifecycleCompletionV1::ReadyValidateSuccessor(
                        ReadyValidateSuccessorV1::from_rejected(published),
                    ),
                );
                ack.acknowledge_after_publication();
                ProductionLifecycleCompletionSelectionV1::LifecycleValidatePublished { ordinal }
            }
            Ok(
                crate::sumeragi::v2_lifecycle_coordinator::DurableValidateCompletionPublication::DeferredMergeSidecar(
                    deferred,
                ),
            ) => {
                assert!(pending_lifecycle_completion.is_none());
                *pending_lifecycle_completion = Some(
                    PendingLifecycleCompletionV1::DeferredValidate(ack.bind_deferred(deferred)),
                );
                ProductionLifecycleCompletionSelectionV1::LifecycleValidateDeferred
            }
            Err((error, dispatch)) => {
                iroha_logger::error!(
                    ?error,
                    "lifecycle Validate publication invariant failed closed"
                );
                drop((dispatch, ack));
                services
                    .lifecycle_output_guard()
                    .close_admission_for_restart();
                ProductionLifecycleCompletionSelectionV1::RestartRequired
            }
        }
    }

    fn settle_ready_validate_successor(
        &mut self,
        successor: ReadyValidateSuccessorV1,
        runner_debt: u64,
    ) -> ProductionLifecycleCompletionSelectionV1 {
        let ordinal = successor.lifecycle_ordinal();
        let result = self.owner.dispatch_ready_validate_successor(
            &mut self.services,
            &mut self.executor,
            successor,
            runner_debt,
        );
        match result {
            Ok(ReadyValidateSuccessorDispatchV1::Resolved(dispatch)) => {
                ProductionLifecycleCompletionSelectionV1::CompletionIoDispatch(Ok(dispatch))
            }
            Ok(ReadyValidateSuccessorDispatchV1::CapacityUnavailable(successor)) => {
                assert!(self.pending_lifecycle_completion.is_none());
                self.pending_lifecycle_completion = Some(
                    PendingLifecycleCompletionV1::ReadyValidateSuccessor(successor),
                );
                ProductionLifecycleCompletionSelectionV1::LifecycleValidateSuccessorCapacityPending {
                    ordinal,
                }
            }
            Ok(ReadyValidateSuccessorDispatchV1::ReducerFencePending { successor, wait }) => {
                assert!(self.pending_lifecycle_completion.is_none());
                self.pending_lifecycle_completion = Some(
                    PendingLifecycleCompletionV1::ReadyValidateSuccessor(successor),
                );
                ProductionLifecycleCompletionSelectionV1::LifecycleValidateSuccessorFencePending {
                    ordinal,
                    wait,
                }
            }
            Err(error) => {
                iroha_logger::error!(
                    ?error,
                    ordinal,
                    "Ready Validate successor failed closed before physical Completion"
                );
                self.close_output_for_restart();
                ProductionLifecycleCompletionSelectionV1::CompletionIoDispatch(Err(error))
            }
        }
    }

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
            Err(CertifiedFetchBodyPersistenceCompletionError::RestartRequiredBeforeLedger(
                error,
            )) => {
                iroha_logger::error!(
                    reason = error.reason(),
                    detail = error.detail(),
                    work_id = error.work_id().get(),
                    "ordinary certified-Fetch Phase B found invalid productive ingress"
                );
                services
                    .lifecycle_output_guard()
                    .close_admission_for_restart();
                drop(error);
                ProductionLifecycleCompletionSelectionV1::CertifiedFetchBodyPersistenceRestartRequired
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
            Err(CertifiedFetchBodyPersistenceCompletionError::RestartRequiredAfterDequeue(
                error,
            )) => {
                iroha_logger::error!(
                    %error,
                    "ordinary certified-Fetch queue handoff requires cold restart"
                );
                services
                    .lifecycle_output_guard()
                    .close_admission_for_restart();
                ProductionLifecycleCompletionSelectionV1::CertifiedFetchBodyPersistenceRestartRequired
            }
            Err(CertifiedFetchBodyPersistenceCompletionError::RestartRequiredAfterCommit(
                error,
            )) => {
                iroha_logger::error!(
                    %error,
                    "ordinary certified-Fetch post-commit terminal requires cold restart"
                );
                services
                    .lifecycle_output_guard()
                    .close_admission_for_restart();
                ProductionLifecycleCompletionSelectionV1::CertifiedFetchBodyPersistenceRestartRequired
            }
        }
    }

    /// Classify parked and physical lifecycle owners before any fresh Ready dispatch.
    ///
    /// Classification precedes cursor consumption. Ordinary work returns the
    /// same borrow-bound turn, while a recovered class is dispatched, drained,
    /// or settled internally without exposing mutually exclusive methods.
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

        let current_validate_fence_wait =
            self.pending_lifecycle_completion
                .as_ref()
                .and_then(|pending| match pending {
                    PendingLifecycleCompletionV1::ReadyValidateSuccessor(successor) => {
                        successor.reducer_fence_wait()
                    }
                    _ => None,
                });
        if let Some(wait) = current_validate_fence_wait {
            let fence = self.executor.lifecycle_reducer_fence_observation();
            if fence.source() == wait.source() && fence.generation() <= wait.observed_generation() {
                match self
                    .services
                    .prepare_ordinary_completion_behind_validate_fence()
                {
                    Ok(true) => {
                        return ProductionLifecycleCompletionPreGateV1::Ordinary(runner);
                    }
                    Ok(false) => {}
                    Err(reason) => {
                        iroha_logger::error!(
                            %reason,
                            "ordinary Completion fence bypass classification failed closed"
                        );
                        self.close_output_for_restart();
                        return ProductionLifecycleCompletionPreGateV1::Selected(
                            ProductionLifecycleCompletionSelectionV1::RestartRequired,
                        );
                    }
                }
            }
        }

        if let Some(pending) = self.pending_lifecycle_completion.take() {
            let selected = match pending {
                PendingLifecycleCompletionV1::LifecycleDecisionApplyDeferred(deferred) => {
                    match self.drive_lifecycle_decision_apply_deferred(deferred, lane_work) {
                        ProductionLifecycleDecisionApplyRetryV1::Requeued => {
                            ProductionLifecycleCompletionSelectionV1::LifecycleDecisionApplyRequeued
                        }
                        ProductionLifecycleDecisionApplyRetryV1::Unavailable(deferred) => {
                            assert!(self.pending_lifecycle_completion.is_none());
                            self.pending_lifecycle_completion = Some(
                                PendingLifecycleCompletionV1::LifecycleDecisionApplyDeferred(
                                    deferred,
                                ),
                            );
                            ProductionLifecycleCompletionSelectionV1::LifecycleDecisionApplyDeferred
                        }
                        ProductionLifecycleDecisionApplyRetryV1::RestartRequired => {
                            ProductionLifecycleCompletionSelectionV1::LifecycleDecisionApplyRestartRequired
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
                PendingLifecycleCompletionV1::Validate(completion) => {
                    self.pending_lifecycle_completion =
                        Some(PendingLifecycleCompletionV1::Validate(completion));
                    self.settle_parked_lifecycle_validate_completion()
                }
                PendingLifecycleCompletionV1::ReadyValidateSuccessor(published) => self
                    .settle_ready_validate_successor(published, runner.debt()),
                PendingLifecycleCompletionV1::DeferredValidate(deferred) => {
                    self.register_and_drive_lifecycle_validate_sidecar(deferred, lane_work)
                }
                PendingLifecycleCompletionV1::RegisteredDeferredValidate(registration) => {
                    self.drive_registered_lifecycle_validate_sidecar(registration, lane_work)
                }
            };
            return ProductionLifecycleCompletionPreGateV1::Selected(selected);
        }

        match self.services.take_next_lifecycle_completion() {
            Ok(LifecycleCompletionTakeV1::PassThrough) => {
                return ProductionLifecycleCompletionPreGateV1::Ordinary(runner);
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
                    .settle_lifecycle_decision_apply_completion_owner(completion, lane_work)
                {
                    Ok(ProductionLifecycleDecisionApplyCompletionV1::Applied) => {
                        ProductionLifecycleCompletionSelectionV1::LifecycleDecisionApplyApplied
                    }
                    Ok(ProductionLifecycleDecisionApplyCompletionV1::Deferred(deferred)) => {
                        assert!(self.pending_lifecycle_completion.is_none());
                        self.pending_lifecycle_completion = Some(
                            PendingLifecycleCompletionV1::LifecycleDecisionApplyDeferred(deferred),
                        );
                        ProductionLifecycleCompletionSelectionV1::LifecycleDecisionApplyCompletionDeferred
                    }
                    Err(reason) => {
                        iroha_logger::error!(
                            %reason,
                            "lifecycle Decision Apply completion settlement failed closed"
                        );
                        self.close_output_for_restart();
                        ProductionLifecycleCompletionSelectionV1::LifecycleDecisionApplyCompletionRestartRequired
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
            Ok(LifecycleCompletionTakeV1::Validate(completion)) => {
                assert!(self.pending_lifecycle_completion.is_none());
                self.pending_lifecycle_completion =
                    Some(PendingLifecycleCompletionV1::Validate(completion));
                let selected = self.settle_parked_lifecycle_validate_completion();
                return ProductionLifecycleCompletionPreGateV1::Selected(selected);
            }
            Ok(LifecycleCompletionTakeV1::CertifiedServe(completion)) => {
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

        ProductionLifecycleCompletionPreGateV1::Ready(ProductionLifecycleReadyCompletionTurnV1 {
            runner,
        })
    }

    /// Park only an authenticated recovered Broadcast after exact Apply settlement.
    ///
    /// Completion I/O is deliberately returned without dispatch so the opaque cursor advances to
    /// the active-height driver's terminal Runtime fence. Invalid census state closes output and
    /// requires restart.
    pub(in crate::sumeragi) fn drive_apply_terminal_ready_broadcast_turn<'cursor>(
        &mut self,
        ready: ProductionLifecycleReadyCompletionTurnV1<'cursor>,
        _permit: crate::sumeragi::v2_runner::LifecycleApplyTerminalReadyBroadcastPermitV1,
    ) -> ProductionLifecycleCompletionTurnV1<'cursor> {
        let ProductionLifecycleReadyCompletionTurnV1 { runner } = ready;
        let fence = self.executor.lifecycle_reducer_fence_observation();
        let selected = match classify_apply_terminal_ready_work(
            self.owner.classify_completion_ready_work(fence),
        ) {
            ProductionApplyTerminalReadyWorkV1::PassThrough => {
                return ProductionLifecycleCompletionTurnV1::PassThrough(runner);
            }
            ProductionApplyTerminalReadyWorkV1::RestartRequired => {
                iroha_logger::error!(
                    "Sumeragi v2 post-Apply recovered-Broadcast Ready census failed closed"
                );
                self.close_output_for_restart();
                ProductionLifecycleCompletionSelectionV1::RestartRequired
            }
            ProductionApplyTerminalReadyWorkV1::RetainedDirectOutput => {
                let prepared = self.owner.prepare_apply_terminal_direct_broadcast();
                let Some(prepared) = prepared else {
                    iroha_logger::error!(
                        "Sumeragi v2 post-Apply direct Broadcast lost its exact Ready attestation"
                    );
                    self.close_output_for_restart();
                    return ProductionLifecycleCompletionTurnV1::Selected(
                        ProductionLifecycleCompletionSelectionV1::RestartRequired,
                    );
                };
                let result = {
                    let Self {
                        owner,
                        executor,
                        services,
                        ..
                    } = self;
                    executor.settle_apply_terminal_direct_broadcast(owner, services, prepared)
                };
                match result {
                    Ok(
                        crate::sumeragi::v2_effects::ProductionApplyTerminalDirectBroadcastSettlementV1::Completed,
                    ) => ProductionLifecycleCompletionSelectionV1::ApplyTerminalDirectBroadcastCompleted,
                    Ok(
                        crate::sumeragi::v2_effects::ProductionApplyTerminalDirectBroadcastSettlementV1::SourceRetained,
                    ) => ProductionLifecycleCompletionSelectionV1::ApplyTerminalDirectBroadcastDeferred,
                    Err(error) => {
                        iroha_logger::error!(
                            %error,
                            "Sumeragi v2 post-Apply direct Broadcast settlement failed closed"
                        );
                        self.close_output_for_restart();
                        ProductionLifecycleCompletionSelectionV1::RestartRequired
                    }
                }
            }
            ProductionApplyTerminalReadyWorkV1::RecoveredLifecycleBroadcast => {
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
                        "Sumeragi v2 post-Apply recovered Broadcast refanout failed closed"
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

    /// Dispatch fresh Ready work only after the caller proves Producer claims are eligible.
    pub(in crate::sumeragi) fn drive_ready_completion_turn<'cursor>(
        &mut self,
        ready: ProductionLifecycleReadyCompletionTurnV1<'cursor>,
    ) -> ProductionLifecycleCompletionTurnV1<'cursor> {
        self.drive_ready_completion_turn_with_required_ordinal(ready, None)
    }

    /// Dispatch only when the complete authenticated census naturally selects
    /// the exact live Apply child retained by the runner claim state.
    pub(in crate::sumeragi) fn drive_ready_completion_turn_requiring_ordinal<'cursor>(
        &mut self,
        ready: ProductionLifecycleReadyCompletionTurnV1<'cursor>,
        required_ordinal: u128,
    ) -> ProductionLifecycleCompletionTurnV1<'cursor> {
        self.drive_ready_completion_turn_with_required_ordinal(ready, Some(required_ordinal))
    }

    fn drive_ready_completion_turn_with_required_ordinal<'cursor>(
        &mut self,
        ready: ProductionLifecycleReadyCompletionTurnV1<'cursor>,
        required_ordinal: Option<u128>,
    ) -> ProductionLifecycleCompletionTurnV1<'cursor> {
        let ProductionLifecycleReadyCompletionTurnV1 { runner } = ready;
        let fence = self.executor.lifecycle_reducer_fence_observation();
        let selected = match self.owner.classify_completion_ready_work(fence) {
            super::super::ProductionCompletionReadyWorkV1::None
            | super::super::ProductionCompletionReadyWorkV1::PassThrough
            | super::super::ProductionCompletionReadyWorkV1::RetainedDirectOutput => {
                return ProductionLifecycleCompletionTurnV1::PassThrough(runner);
            }
            super::super::ProductionCompletionReadyWorkV1::Invalid => {
                iroha_logger::error!("Sumeragi v2 lifecycle Completion Ready census failed closed");
                self.close_output_for_restart();
                ProductionLifecycleCompletionSelectionV1::RestartRequired
            }
            super::super::ProductionCompletionReadyWorkV1::CompletionIo => {
                let result = {
                    let Self {
                        owner,
                        executor,
                        services,
                        ..
                    } = self;
                    match required_ordinal {
                        Some(ordinal) => owner.dispatch_completion_requiring_ready_ordinal(
                            services,
                            executor,
                            runner.debt(),
                            ordinal,
                        ),
                        None => owner.dispatch_completion_with_runner_debt(
                            services,
                            executor,
                            runner.debt(),
                        ),
                    }
                };
                if let Err(error) = &result {
                    iroha_logger::error!(
                        ?error,
                        "Sumeragi v2 lifecycle Completion dispatch failed closed"
                    );
                    self.close_output_for_restart();
                }
                ProductionLifecycleCompletionSelectionV1::CompletionIoDispatch(result)
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

    /// Compose the split Completion pre-gate and Ready dispatcher for tests.
    ///
    /// Production callers must consume the sealed split API so an existing
    /// non-Producer lease cannot reach the fresh-claim branch.
    #[cfg(test)]
    pub(in crate::sumeragi) fn drive_completion_turn_for_test<'cursor>(
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

    /// Move at most one fair-ingress row under an already-frozen timeout cut.
    ///
    /// The exact Prepare-progress predicate is a read-only runtime preview.
    /// Ordinary obsolete retirement remains enabled only to release a durable
    /// leader-wire predecessor; every non-obsolete row must match the exact
    /// preview, and the queue's Blocked verdict is never bypassed.
    pub(in crate::sumeragi) fn prepare_pre_timeout_locked_prepare_qc_ingress_turn(
        &mut self,
        cut: &PreTimeoutLockedPrepareQcCutV1,
    ) -> ProductionPreTimeoutLockedPrepareQcIngressTurnV1 {
        let terminal_subject = match self.executor.lifecycle_terminal_subject() {
            Ok(subject) => subject,
            Err(error) => {
                iroha_logger::error!(
                    %error,
                    "pre-timeout PrepareQC terminal-subject projection failed closed"
                );
                self.close_output_for_restart();
                return ProductionPreTimeoutLockedPrepareQcIngressTurnV1::RestartRequired;
            }
        };
        let ingress = std::sync::Arc::clone(&self.leader_wire_ingress_binding.ingress);
        let captured = {
            let executor = &self.executor;
            ingress.capture_next_ingress_turn_cut_before_with_obsolete_retirement(
                cut.physical_cut(),
                |occurrence| {
                    let crate::sumeragi::message::BlockMessage::V2(message) =
                        occurrence.inbound().message()
                    else {
                        return false;
                    };
                    executor.wire_previews_pre_timeout_locked_prepare_qc(cut, &message.payload)
                },
            )
        };
        let Some(captured) = (match captured {
            Ok(captured) => captured,
            Err(error) => {
                iroha_logger::error!(
                    ?error,
                    "pre-timeout PrepareQC fair-ingress cut failed closed"
                );
                self.close_output_for_restart();
                return ProductionPreTimeoutLockedPrepareQcIngressTurnV1::RestartRequired;
            }
        }) else {
            return ProductionPreTimeoutLockedPrepareQcIngressTurnV1::Empty;
        };
        let obsolete =
            captured.selected_disposition() == FairV2IngressDequeueDisposition::RetireObsolete;
        match prepare_ordinary_ingress_dequeue(
            &ingress,
            captured,
            None,
            terminal_subject,
            &self.services,
        ) {
            PreparedOrdinaryIngressDequeueV1::Prepared(turn) if obsolete => {
                ProductionPreTimeoutLockedPrepareQcIngressTurnV1::ObsoletePredecessor(turn)
            }
            PreparedOrdinaryIngressDequeueV1::Prepared(turn) => {
                ProductionPreTimeoutLockedPrepareQcIngressTurnV1::ExactPrepareProgress(turn)
            }
            PreparedOrdinaryIngressDequeueV1::RestartRequired => {
                ProductionPreTimeoutLockedPrepareQcIngressTurnV1::RestartRequired
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
                    }
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
            Err((FairIngressQueueCutError::QueueCutChanged, retained)) => {
                iroha_logger::debug!(
                    "certified-response fair-ingress ownership changed during narrowing; retrying"
                );
                drop(retained);
                drop(runner);
                return ProductionLifecycleIngressTurnV1::Selected(
                    ProductionLifecycleIngressSelectionV1::CertifiedFetchRetry,
                );
            }
            Err((error, retained)) => {
                iroha_logger::error!(
                    ?error,
                    "certified-response fair-ingress cut failed structural narrowing"
                );
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
                let selected_priority = match self
                    .executor
                    .classify_selected_certified_response_priority(&cut)
                {
                    Ok(selected_priority) => selected_priority,
                    Err(LifecycleIngressSelectorError::QueueCutChanged) => {
                        iroha_logger::debug!(
                            "certified-response fair-ingress census changed during priority classification; retrying"
                        );
                        drop(cut);
                        drop(runner);
                        return ProductionLifecycleIngressTurnV1::Selected(
                            ProductionLifecycleIngressSelectionV1::CertifiedFetchRetry,
                        );
                    }
                    Err(error) => {
                        let reason = error.detail();
                        iroha_logger::error!(
                            %reason,
                            "Sumeragi v2 certified-response priority classification failed closed"
                        );
                        self.close_output_for_restart();
                        drop(cut);
                        drop(runner);
                        return ProductionLifecycleIngressTurnV1::Selected(
                            ProductionLifecycleIngressSelectionV1::RestartRequired,
                        );
                    }
                };
                match selected_priority {
                    SelectedCertifiedResponsePriorityV1::DefinitelyNonPriority => {
                        dequeue_prepared_ordinary_ingress(
                            &ingress,
                            cut.into_ordinary_turn_cut(),
                            runner,
                            None,
                            terminal_subject,
                            &self.services,
                        )
                    }
                    SelectedCertifiedResponsePriorityV1::OrdinaryClaimed => {
                        let selector = match self.executor.capture_lifecycle_ingress_selector(cut) {
                            Ok(selector) => selector,
                            Err(LifecycleIngressSelectorError::QueueCutChanged) => {
                                iroha_logger::debug!(
                                    "ordinary certified-Fetch fair-ingress census changed during selector capture; retrying"
                                );
                                drop(runner);
                                return ProductionLifecycleIngressTurnV1::Selected(
                                    ProductionLifecycleIngressSelectionV1::CertifiedFetchRetry,
                                );
                            }
                            Err(error) => {
                                let reason = error.detail();
                                iroha_logger::error!(
                                    %reason,
                                    "ordinary certified-Fetch selector capture failed closed"
                                );
                                self.close_output_for_restart();
                                drop(runner);
                                return ProductionLifecycleIngressTurnV1::Selected(
                                    ProductionLifecycleIngressSelectionV1::RestartRequired,
                                );
                            }
                        };
                        self.drive_certified_fetch_ingress_selector(selector, runner)
                    }
                    SelectedCertifiedResponsePriorityV1::RecoveredClaimed => {
                        let selector = match self
                            .executor
                            .prepare_recovered_decision_fetch_from_selected_cut(cut)
                        {
                            Ok(selector) => selector,
                            Err(LifecycleIngressSelectorError::QueueCutChanged) => {
                                iroha_logger::debug!(
                                    "recovered certified-Fetch fair-ingress census changed during selector capture; retrying"
                                );
                                drop(runner);
                                return ProductionLifecycleIngressTurnV1::Selected(
                                    ProductionLifecycleIngressSelectionV1::CertifiedFetchRetry,
                                );
                            }
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
        if let Err(error) = &result {
            iroha_logger::error!(
                reason = error.reason(),
                "Sumeragi v2 certified Fetch ingress planning did not complete"
            );
        }
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
                | ProductionIngressSchedulerInputsError::CertifiedFetchAdmissionPreparation {
                    ..
                }
                | ProductionIngressSchedulerInputsError::CertifiedFetchAdmissionDeferred { .. }
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
                | ProductionIngressSchedulerInputsError::CertifiedFetchAdmissionSettlement
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
        match self.classify_parked_recovered_sign_completion() {
            ParkedRecoveredLifecycleSignCompletionClassV1::Superseded => {
                if self.settle_superseded_recovered_lifecycle_sign() {
                    ProductionRecoveredLifecycleSignCompletionSelectionV1::Superseded
                } else {
                    self.close_output_for_restart();
                    ProductionRecoveredLifecycleSignCompletionSelectionV1::RestartRequired
                }
            }
            ParkedRecoveredLifecycleSignCompletionClassV1::Retry => {
                ProductionRecoveredLifecycleSignCompletionSelectionV1::Retry
            }
            ParkedRecoveredLifecycleSignCompletionClassV1::RestartRequired => {
                self.close_output_for_restart();
                ProductionRecoveredLifecycleSignCompletionSelectionV1::RestartRequired
            }
            ParkedRecoveredLifecycleSignCompletionClassV1::Settlement(
                crate::sumeragi::v2::RecoveredLifecycleSignAdapterSettlementFamilyV1::Broadcast,
            ) => {
                ProductionRecoveredLifecycleSignCompletionSelectionV1::Broadcast(
                    self.settle_recovered_lifecycle_sign_broadcast(),
                )
            }
            ParkedRecoveredLifecycleSignCompletionClassV1::Settlement(
                crate::sumeragi::v2::RecoveredLifecycleSignAdapterSettlementFamilyV1::ProposalPrepareWal,
            ) => {
                ProductionRecoveredLifecycleSignCompletionSelectionV1::ProposalPrepareWal(
                    self.settle_recovered_lifecycle_proposal_prepare_wal(),
                )
            }
            ParkedRecoveredLifecycleSignCompletionClassV1::Settlement(
                crate::sumeragi::v2::RecoveredLifecycleSignAdapterSettlementFamilyV1::VoteBroadcastAndSign,
            ) => {
                ProductionRecoveredLifecycleSignCompletionSelectionV1::VoteBroadcastAndSign(
                    self.settle_recovered_lifecycle_vote_broadcast_and_sign(),
                )
            }
            ParkedRecoveredLifecycleSignCompletionClassV1::Settlement(
                crate::sumeragi::v2::RecoveredLifecycleSignAdapterSettlementFamilyV1::ProposalBroadcastAndSign,
            ) => {
                ProductionRecoveredLifecycleSignCompletionSelectionV1::ProposalBroadcastAndSign(
                    self.settle_recovered_lifecycle_proposal_broadcast_and_sign(),
                )
            }
        }
    }

    fn classify_parked_recovered_sign_completion(
        &mut self,
    ) -> ParkedRecoveredLifecycleSignCompletionClassV1 {
        let Some(completion) = self
            .pending_lifecycle_completion
            .as_ref()
            .and_then(PendingLifecycleCompletionV1::recovered_sign)
        else {
            iroha_logger::error!("recovered Sign classifier lost its parked completion");
            return ParkedRecoveredLifecycleSignCompletionClassV1::RestartRequired;
        };
        let Some(authority) = completion.project_adapter_completion_authority() else {
            iroha_logger::error!(
                ordinal = completion.dispatch_key().lifecycle_ordinal(),
                "recovered Sign classifier rejected an inexact guarded worker result"
            );
            return ParkedRecoveredLifecycleSignCompletionClassV1::RestartRequired;
        };
        let preview = match self
            .executor
            .prepare_recovered_lifecycle_sign_completion(authority)
        {
            Ok(preview) => preview,
            Err(crate::sumeragi::v2::AdapterError::RecoveredLifecycleSignCompletionSuperseded) => {
                return ParkedRecoveredLifecycleSignCompletionClassV1::Superseded;
            }
            Err(crate::sumeragi::v2::AdapterError::RecoveredLifecycleSignCompletionRuntimeDebt) => {
                return ParkedRecoveredLifecycleSignCompletionClassV1::Retry;
            }
            Err(error) => {
                iroha_logger::error!(
                    %error,
                    ordinal = completion.dispatch_key().lifecycle_ordinal(),
                    "recovered Sign adapter classification failed closed"
                );
                return ParkedRecoveredLifecycleSignCompletionClassV1::RestartRequired;
            }
        };
        let class = preview
            .settlement_family()
            .map(ParkedRecoveredLifecycleSignCompletionClassV1::Settlement);
        drop(preview);
        class.unwrap_or_else(|| {
            iroha_logger::error!(
                ordinal = completion.dispatch_key().lifecycle_ordinal(),
                "recovered Sign adapter preview had no closed settlement family"
            );
            ParkedRecoveredLifecycleSignCompletionClassV1::RestartRequired
        })
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
    /// Reconcile current-height capacity observations retained across a terminal barrier.
    ///
    /// A capacity-blocked Serve has not committed its fair-ingress dequeue, so the direct
    /// decided-lane recovery driver remains its sole consumer after this handoff. This includes
    /// repair of a terminal-ready executor whose process-local claim returned to `Eligible`.
    /// A completed Serve, by contrast, must publish its lifecycle terminal before finalization.
    /// An ordinary capacity-blocked Fetch likewise has not committed its dequeue or admitted a
    /// new durable row, so its service-generation observation can be dropped while sealed height
    /// retirement owns the queued occurrence and any older recovered row. A recovered Decision
    /// Fetch already owns a durable row before capacity capture and therefore remains fail-closed.
    pub(in crate::sumeragi) fn reconcile_decided_lane_certified_serve(
        &mut self,
        _runner: &mut crate::sumeragi::v2_runner::ProductionLifecycleActiveRunnerBorrowV1,
        _permit: crate::sumeragi::v2_runner::LifecycleDecidedLaneRecoveryPermitV1,
    ) -> Result<bool, String> {
        let completion = match self
            .launched
            .services
            .drain_lifecycle_certified_serve_completion()
        {
            Ok(completion) => completion.into_completion(),
            Err(error) => {
                self.launched
                    .services
                    .lifecycle_output_guard()
                    .close_admission_for_restart();
                return Err(error);
            }
        };
        let mut progressed = false;
        if let Some(completion) = completion {
            if completion
                .settle_deliver_and_acknowledge(&mut self.launched.owner, &self.launched.services)
                .is_err()
            {
                self.launched
                    .services
                    .lifecycle_output_guard()
                    .close_admission_for_restart();
                return Err(
                    "terminal-barrier Certified-Serve settlement requires restart".to_owned(),
                );
            }
            progressed = true;
        }

        let Some(pending) = self.launched.pending_ingress_capacity.take() else {
            return Ok(progressed);
        };
        match pending {
            PendingIngressCapacityV1::CertifiedServe(wait) => match wait.status(&self.launched.services) {
                crate::sumeragi::v2_worker::LifecycleIoCapacityWaitStatus::SamePending
                | crate::sumeragi::v2_worker::LifecycleIoCapacityWaitStatus::Released => {
                    // The lifecycle dequeue was never committed. Dropping only its same-service
                    // capacity observation transfers the still-queued request to the sealed
                    // decided-lane CurrentServe path.
                    drop(wait);
                    Ok(true)
                }
                crate::sumeragi::v2_worker::LifecycleIoCapacityWaitStatus::GenerationExhausted
                | crate::sumeragi::v2_worker::LifecycleIoCapacityWaitStatus::ForeignOrDisconnected => {
                    self.launched.pending_ingress_capacity =
                        Some(PendingIngressCapacityV1::CertifiedServe(wait));
                    self.launched
                        .services
                        .lifecycle_output_guard()
                        .close_admission_for_restart();
                    Err(
                        "terminal-barrier Certified-Serve capacity owner lost its exact service"
                            .to_owned(),
                    )
                }
            },
            PendingIngressCapacityV1::CertifiedFetch(wait) => {
                match wait.capacity_status(&self.launched.services) {
                    ProductionIngressCapacityStatus::Pending
                    | ProductionIngressCapacityStatus::Released => {
                        // This attempt captured capacity before admitting a new
                        // durable row and never dequeued fair ingress. Sealed
                        // height retirement owns both that occurrence and any
                        // pre-existing row after this observation is dropped.
                        drop(wait);
                        Ok(true)
                    }
                    ProductionIngressCapacityStatus::GenerationExhausted
                    | ProductionIngressCapacityStatus::RestartRequired => {
                        self.launched.pending_ingress_capacity =
                            Some(PendingIngressCapacityV1::CertifiedFetch(wait));
                        self.launched
                            .services
                            .lifecycle_output_guard()
                            .close_admission_for_restart();
                        Err(
                            "terminal-barrier Certified-Fetch capacity owner lost its exact service"
                                .to_owned(),
                        )
                    }
                }
            }
            pending @ PendingIngressCapacityV1::RecoveredDecisionFetch(_) => {
                self.launched.pending_ingress_capacity = Some(pending);
                self.launched
                    .services
                    .lifecycle_output_guard()
                    .close_admission_for_restart();
                Err(
                    "terminal barrier retained a recovered Decision-Fetch ingress-capacity owner"
                        .to_owned(),
                )
            }
        }
    }

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

    /// Compose the split Completion API without exposing the launched stack in tests.
    #[cfg(test)]
    pub(in crate::sumeragi) fn drive_completion_turn_for_test<'cursor>(
        &mut self,
        runner: LifecycleCurrentRunnerTurn<'cursor>,
        lane_work: &mut V2LaneWorkAdapter,
    ) -> ProductionLifecycleCompletionTurnV1<'cursor> {
        self.launched
            .drive_completion_turn_for_test(runner, lane_work)
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

    /// Consume a physically empty Completion cursor through the sealed post-Apply Broadcast path.
    pub(in crate::sumeragi) fn drive_apply_terminal_ready_broadcast_turn<'cursor>(
        &mut self,
        ready: ProductionLifecycleReadyCompletionTurnV1<'cursor>,
        permit: crate::sumeragi::v2_runner::LifecycleApplyTerminalReadyBroadcastPermitV1,
    ) -> ProductionLifecycleCompletionTurnV1<'cursor> {
        self.launched
            .drive_apply_terminal_ready_broadcast_turn(ready, permit)
    }

    /// Consume a physically empty Completion cursor only when the full
    /// schedulable census naturally selects the retained live Apply child.
    pub(in crate::sumeragi) fn drive_ready_completion_turn_requiring_ordinal<'cursor>(
        &mut self,
        ready: ProductionLifecycleReadyCompletionTurnV1<'cursor>,
        required_ordinal: u128,
    ) -> ProductionLifecycleCompletionTurnV1<'cursor> {
        self.launched
            .drive_ready_completion_turn_requiring_ordinal(ready, required_ordinal)
    }

    /// Forward one Ingress turn without exposing the launched stack.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::sumeragi) fn drive_ingress_turn<'cursor>(
        &mut self,
        runner: LifecycleCurrentRunnerTurn<'cursor>,
    ) -> ProductionLifecycleIngressTurnV1<'cursor> {
        self.launched.drive_ingress_turn(runner)
    }

    /// Forward one fixed-cut pre-timeout ingress preparation without exposing
    /// the launched executor or queue owner.
    pub(in crate::sumeragi) fn prepare_pre_timeout_locked_prepare_qc_ingress_turn(
        &mut self,
        cut: &PreTimeoutLockedPrepareQcCutV1,
    ) -> ProductionPreTimeoutLockedPrepareQcIngressTurnV1 {
        self.launched
            .prepare_pre_timeout_locked_prepare_qc_ingress_turn(cut)
    }

    /// Emit a read-only, rate-limited ownership census when a non-empty fair
    /// ingress has retained its oldest owner for a full liveness interval.
    ///
    /// These owners are intentionally private to the launched lifecycle and
    /// worker service. Keeping the diagnostic here preserves that boundary
    /// while making a stale claim, parked capacity wait, blocked I/O FIFO, and
    /// selector barrier distinguishable in one operator record.
    pub(in crate::sumeragi) fn log_scheduler_stall_diagnostic(
        &mut self,
        _runner: &mut crate::sumeragi::v2_runner::ProductionLifecycleActiveRunnerBorrowV1,
        producer_claim: crate::sumeragi::v2_runner::LifecycleProducerClaimDispositionV1,
        terminal_finalization_cut_active: bool,
        finalized_ingress_closed: bool,
        ingress: &FairV2Ingress,
        ingress_oldest_age: Option<Duration>,
        ingress_service_idle_age: Option<Duration>,
        last_advance_executor_yield: Option<(
            &'static str,
            crate::sumeragi::v2_runner::AdvanceExecutorYieldV1,
            Duration,
        )>,
    ) {
        use crate::sumeragi::v2_runner::LifecycleProducerClaimDispositionV1 as Claim;

        let (
            producer_claim_kind,
            producer_claim_ordinal,
            producer_claim_child_ordinal,
            producer_claim_wait_source,
            producer_claim_wait_generation,
        ) = match producer_claim {
            Claim::Eligible => ("Eligible", None, None, None, None),
            Claim::AwaitingValidateSuccessor { ordinal } => {
                ("AwaitingValidateSuccessor", Some(ordinal), None, None, None)
            }
            Claim::AwaitingValidateFence { ordinal, wait } => {
                let source = match wait.source() {
                    WaitSource::Capacity(_) => "Capacity",
                    WaitSource::External(_) => "External",
                    WaitSource::Recovery(_) => "Recovery",
                    WaitSource::ProducerTurn(_) => "ProducerTurn",
                };
                (
                    "AwaitingValidateFence",
                    Some(ordinal),
                    None,
                    Some(source),
                    Some(wait.observed_generation()),
                )
            }
            Claim::AwaitingLiveApplyQueue {
                parent_ordinal,
                child_ordinal,
            } => (
                "AwaitingLiveApplyQueue",
                Some(parent_ordinal),
                Some(child_ordinal),
                None,
                None,
            ),
            Claim::AwaitingCompletion => ("AwaitingCompletion", None, None, None, None),
            Claim::AwaitingValidateSidecar => ("AwaitingValidateSidecar", None, None, None, None),
            Claim::AwaitingApplyCompletion => ("AwaitingApplyCompletion", None, None, None, None),
            Claim::ApplyTerminalSettled => ("ApplyTerminalSettled", None, None, None, None),
            Claim::AwaitingReplayCompletion => ("AwaitingReplayCompletion", None, None, None, None),
        };
        let executor_ready_to_finish_blockers = self.launched.executor.ready_to_finish_blockers();
        let executor_ready_to_finish = executor_ready_to_finish_blockers.is_empty();
        let validate_retry_census = self
            .launched
            .executor
            .durable_validate_retry_finalization_diagnostic();
        let registry_exactly_covers_finalization_work = self
            .launched
            .owner
            .registry
            .registry()
            .exactly_covers_finalization_work(&self.launched.owner.coordinator);
        let finalization_scheduler = self.launched.owner.finalization_scheduler_diagnostic(
            self.launched.executor.lifecycle_reducer_fence_observation(),
        );
        let store_marker_census = self
            .launched
            .verify_published_store_marker_finalization_census();
        let active_lease = self
            .launched
            .owner
            .lifecycle_active_lease_scheduler_snapshot();
        let (active_lease_ordinal, active_lease_work_class, active_lease_stage) = active_lease
            .map_or((None, None, None), |(ordinal, work_class, stage)| {
                (Some(ordinal), Some(work_class), Some(stage))
            });
        let pending_completion =
            self.launched
                .pending_lifecycle_completion
                .as_ref()
                .map(|pending| match pending {
                    PendingLifecycleCompletionV1::LifecycleDecisionApplyDeferred(_) => {
                        "LifecycleDecisionApplyDeferred"
                    }
                    PendingLifecycleCompletionV1::CertifiedFetch(_) => "CertifiedFetch",
                    PendingLifecycleCompletionV1::RecoveredDecisionFetch(_) => {
                        "RecoveredDecisionFetch"
                    }
                    PendingLifecycleCompletionV1::RecoveredSign(_) => "RecoveredSign",
                    PendingLifecycleCompletionV1::Validate(_) => "Validate",
                    PendingLifecycleCompletionV1::ReadyValidateSuccessor(_) => {
                        "ReadyValidateSuccessor"
                    }
                    PendingLifecycleCompletionV1::DeferredValidate(_) => "DeferredValidate",
                    PendingLifecycleCompletionV1::RegisteredDeferredValidate(_) => {
                        "RegisteredDeferredValidate"
                    }
                });
        let pending_capacity = self
            .launched
            .pending_ingress_capacity
            .as_ref()
            .map(|pending| match pending {
                PendingIngressCapacityV1::CertifiedFetch(wait) => (
                    "CertifiedFetch",
                    format!("{:?}", wait.capacity_status(&self.launched.services)),
                ),
                PendingIngressCapacityV1::RecoveredDecisionFetch(wait) => (
                    "RecoveredDecisionFetch",
                    format!("{:?}", wait.capacity_status(&self.launched.services)),
                ),
                PendingIngressCapacityV1::CertifiedServe(wait) => (
                    "CertifiedServe",
                    format!("{:?}", wait.status(&self.launched.services)),
                ),
            });
        let (pending_capacity_kind, pending_capacity_status) = pending_capacity
            .as_ref()
            .map_or((None, None), |(kind, status)| {
                (Some(*kind), Some(status.as_str()))
            });
        let executor_status = self.launched.executor.status();
        let io = self.launched.services.lifecycle_io_scheduler_snapshot();
        let exact_output = self.launched.services.exact_output_scheduler_snapshot();
        let recovered_lifecycle_outputs = self.launched.owner.recovered_lifecycle_output_count();
        let fair_selector = ingress
            .scheduler_stall_diagnostic()
            .unwrap_or_else(|error| format!("unavailable: {error}"));
        iroha_logger::warn!(
            height = self.launched.executor.context().height,
            context_id = ?self.launched.executor.context().id(),
            producer_claim_kind,
            terminal_finalization_cut_active,
            finalized_ingress_closed,
            ?producer_claim_ordinal,
            ?producer_claim_child_ordinal,
            ?producer_claim_wait_source,
            ?producer_claim_wait_generation,
            ?active_lease_ordinal,
            ?active_lease_work_class,
            ?active_lease_stage,
            ?ingress_oldest_age,
            ?ingress_service_idle_age,
            ?last_advance_executor_yield,
            recovered_lifecycle_outputs,
            executor_ready_to_finish,
            ?executor_ready_to_finish_blockers,
            validate_retry_census = %validate_retry_census,
            registry_exactly_covers_finalization_work,
            pending_kura_apply_replay = self.launched.pending_kura_apply_replay.is_some(),
            recovered_local_proposal_attempt = self
                .launched
                .recovered_local_proposal_attempt
                .is_some(),
            pending_lifecycle_completion_present = self
                .launched
                .pending_lifecycle_completion
                .is_some(),
            pending_ingress_capacity_present = self.launched.pending_ingress_capacity.is_some(),
            completion_observer_activation_present = self
                .launched
                .completion_observer_activation
                .is_some(),
            ?store_marker_census,
            finalization_scheduler = %finalization_scheduler,
            pending_completion = ?pending_completion,
            pending_capacity_kind = ?pending_capacity_kind,
            pending_capacity_status = ?pending_capacity_status,
            pending_live_wal_sign = self
                .launched
                .executor
                .has_pending_live_wal_sign_admission(),
            pending_lifecycle_output_admission = self
                .launched
                .executor
                .has_pending_lifecycle_output_admissions(),
            pending_durable_validate_admission = self
                .launched
                .executor
                .has_pending_durable_validate_admissions(),
            pending_signatures = executor_status.pending_signatures,
            pending_fetches = executor_status.pending_fetches,
            pending_stores = executor_status.pending_stores,
            pending_validations = executor_status.pending_validations,
            pending_outputs = executor_status.pending_outputs,
            pending_applications = executor_status.pending_applications,
            queued_runtime_completions = executor_status.queued_runtime_completions,
            ?io,
            ?exact_output,
            fair_selector = %fair_selector,
            "Sumeragi v2 scheduler/finalization stall ownership census"
        );
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
        npos_beacon: &mut crate::sumeragi::v2_beacon::V2GlobalBeaconLifecycle,
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
            npos_beacon,
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
