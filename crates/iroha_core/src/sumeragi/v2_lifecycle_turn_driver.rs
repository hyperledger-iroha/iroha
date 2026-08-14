//! Unified lifecycle ownership for one outer Completion or Ingress turn.

use super::*;
#[cfg(test)]
pub(in crate::sumeragi) use crate::sumeragi::v2_runner::ordinary_ingress_consumer::ProductionPreparedCertifiedServeTestSettlementV1;
#[cfg(test)]
use crate::sumeragi::v2_runner::ordinary_ingress_consumer::settle_prepared_certified_serve_for_test;
use crate::sumeragi::v2_runner::ordinary_ingress_consumer::{
    PreparedDequeuedV2IngressV1, ProductionPreparedCertifiedServeV1,
    ProductionPreparedOrdinaryIngressConsumptionV1, consume_prepared_dequeued_v2_ingress,
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

/// Closed diagnostic for one lifecycle-selected Ingress turn.
#[must_use = "the lifecycle-selected Ingress result must be observed"]
pub(in crate::sumeragi) enum ProductionLifecycleIngressSelectionV1 {
    /// The retained I/O generation has not advanced; every owner remains parked.
    CapacityPending,
    /// Recovered Phase A queued its exact body-persistence command.
    RecoveredDecisionFetchQueued,
    /// A retryable pre-publication cut retained the physical response in ingress.
    Retry,
    /// The exact ordinary winner retained its physical carrier under backpressure.
    OrdinaryRetained,
    /// The selected recovered owner changed and process restart is required.
    RestartRequired,
}

/// Opaque move-only ordinary ingress handoff selected and removed by the queue.
///
/// The exact inbound carrier, ordinary dequeue disposition, and any stateful
/// Certified-Serve result cannot be separated at the runner-facing boundary.
/// Until the future runner cutover routes this token through the activated
/// lifecycle consumer, Drop closes consensus admission so a prepared Serve
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

fn prepare_and_dequeue_current_certified_serve<'cursor>(
    executor: &V2EffectExecutor<SerializedV2Runtime>,
    services: &mut ProductionV2Services,
    receiver: &std::sync::Arc<FairV2Ingress>,
    cut: FairIngressTurnCut<'_>,
    runner: LifecycleCurrentRunnerTurn<'cursor>,
    terminal_subject: Option<iroha_data_model::block::consensus_v2::BlockSubject>,
) -> ProductionLifecycleIngressTurnV1<'cursor> {
    let output_guard = services.lifecycle_output_guard();
    let Some(operation) = output_guard.begin_fail_stop_operation() else {
        drop(cut);
        drop(runner);
        return ProductionLifecycleIngressTurnV1::Selected(
            ProductionLifecycleIngressSelectionV1::RestartRequired,
        );
    };

    let prepared = {
        let inbound = cut.selected_occurrence().inbound();
        let message = match inbound.message() {
            crate::sumeragi::message::BlockMessage::V2(message) => message,
            _ => unreachable!("current Serve classifier selected one v2 request"),
        };
        if let Err(error) = message.validate_version() {
            ProductionPreparedCertifiedServeV1::Service(format!(
                "reserved certified-body ingress crossed version validation: {error}"
            ))
        } else {
            let iroha_data_model::block::consensus_v2::ConsensusMessageV2Payload::CertifiedBodyRequest(
                request,
            ) = &message.payload
            else {
                unreachable!("current Serve classifier selected one CertifiedBodyRequest");
            };
            let superseded =
                crate::sumeragi::v2_effects::certified_body_request_is_superseded_after_decision(
                    request,
                    terminal_subject,
                    executor.context().height,
                );
            let ownership = (|| {
                let sender = inbound.sender().ok_or_else(|| {
                    "reserved certified-body ingress lost its authenticated sender".to_owned()
                })?;
                let authenticated_via = inbound.via().ok_or_else(|| {
                    "reserved certified-body ingress lost its authenticated source".to_owned()
                })?;
                let reply_routes = inbound.reply_routes().ok_or_else(|| {
                    "reserved certified-body ingress lost its reply capability".to_owned()
                })?;
                let ownership = inbound.ingress_ownership().ok_or_else(|| {
                    "reserved certified-body ingress lost its ownership evidence".to_owned()
                })?;
                if reply_routes.semantic_target() != sender
                    || !ownership.validate_exact()
                    || !ownership.matches_message(inbound.message())
                    || !ownership.matches_semantic_origin(Some(sender))
                    || !ownership.matches_reply_routes(Some(reply_routes))
                {
                    return Err(
                        "reserved certified-body ingress changed its transport ownership"
                            .to_owned(),
                    );
                }
                Ok((sender, authenticated_via))
            })();
            match ownership {
                Err(reason) => ProductionPreparedCertifiedServeV1::Service(reason),
                Ok((sender, authenticated_via)) => {
                    match executor.authenticate_certified_body_request(request.clone(), sender) {
                        Err(error) => match services.stage_certified_serve_rejection(
                            HashOf::new(request),
                            CertifiedServeNegativeOutcome::InvalidCertificate,
                        ) {
                            Ok(()) => {
                                ProductionPreparedCertifiedServeV1::Rejected(error.to_string())
                            }
                            Err(reason) => ProductionPreparedCertifiedServeV1::Service(reason),
                        },
                        Ok(authenticated) if superseded => {
                            let decided = terminal_subject
                                .expect("superseded Serve has one durable Decision subject");
                            match services.stage_certified_serve_rejection(
                                authenticated.request_hash(),
                                CertifiedServeNegativeOutcome::SupersededByDurableDecision(decided),
                            ) {
                                Ok(()) => ProductionPreparedCertifiedServeV1::Rejected(
                                    "certified body request was superseded by durable Decision"
                                        .to_owned(),
                                ),
                                Err(reason) => ProductionPreparedCertifiedServeV1::Service(reason),
                            }
                        }
                        Ok(authenticated) => {
                            match services
                                .prepare_certified_request(authenticated_via, authenticated)
                            {
                                Ok(admission) => {
                                    ProductionPreparedCertifiedServeV1::Admitted(admission)
                                }
                                Err(CertifiedServePrepareError::Backpressure) => {
                                    operation.complete();
                                    drop(cut);
                                    drop(runner);
                                    return ProductionLifecycleIngressTurnV1::Selected(
                                        ProductionLifecycleIngressSelectionV1::OrdinaryRetained,
                                    );
                                }
                                Err(CertifiedServePrepareError::Rejected(reason)) => {
                                    ProductionPreparedCertifiedServeV1::Rejected(reason)
                                }
                                Err(CertifiedServePrepareError::Service(reason)) => {
                                    ProductionPreparedCertifiedServeV1::Service(reason)
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    match cut.dequeue_exact_retaining() {
        Ok((inbound, disposition)) => {
            let turn = prepared_ordinary_ingress_turn(
                std::sync::Arc::clone(receiver),
                inbound,
                disposition,
                Some(prepared),
                terminal_subject,
                std::sync::Arc::clone(&output_guard),
            );
            operation.complete();
            drop(runner);
            ProductionLifecycleIngressTurnV1::Ordinary(turn)
        }
        Err((_error, retained)) => {
            // Close output while the exact queue service episode is still held.
            drop(operation);
            drop(retained);
            drop(runner);
            ProductionLifecycleIngressTurnV1::Selected(
                ProductionLifecycleIngressSelectionV1::RestartRequired,
            )
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
        Err((_error, retained)) => {
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
    /// Service one exact outer Completion turn through the lifecycle owner.
    ///
    /// Classification precedes cursor consumption. Ordinary work returns the
    /// same borrow-bound turn, while a recovered class is dispatched, drained,
    /// or settled internally without exposing mutually exclusive methods.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::sumeragi) fn drive_completion_turn<'cursor>(
        &mut self,
        runner: LifecycleCurrentRunnerTurn<'cursor>,
        lane_work: &mut V2LaneWorkAdapter,
    ) -> ProductionLifecycleCompletionTurnV1<'cursor> {
        if !self.runner_turn_matches(
            &runner,
            crate::sumeragi::v2_runner::LifecycleRunnerRankTarget::Completion,
        ) {
            return ProductionLifecycleCompletionTurnV1::PassThrough(runner);
        }

        if let Some(deferred) = self.recovered_decision_apply_deferred.take() {
            let selected = match self.drive_recovered_decision_apply_deferred(deferred, lane_work) {
                ProductionRecoveredDecisionApplyRetryV1::Requeued => {
                    ProductionLifecycleCompletionSelectionV1::RecoveredDecisionApplyRequeued
                }
                ProductionRecoveredDecisionApplyRetryV1::Unavailable(deferred) => {
                    assert!(self.recovered_decision_apply_deferred.is_none());
                    self.recovered_decision_apply_deferred = Some(deferred);
                    ProductionLifecycleCompletionSelectionV1::RecoveredDecisionApplyDeferred
                }
                ProductionRecoveredDecisionApplyRetryV1::RestartRequired => {
                    ProductionLifecycleCompletionSelectionV1::RecoveredDecisionApplyRestartRequired
                }
            };
            return ProductionLifecycleCompletionTurnV1::Selected(selected);
        }

        if self.recovered_lifecycle_sign_completion.is_some()
            && self.recovered_decision_fetch_body_completion.is_some()
        {
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

        match self.services.take_next_recovered_lifecycle_completion() {
            Ok(RecoveredLifecycleCompletionTakeV1::PassThrough) => {
                return ProductionLifecycleCompletionTurnV1::PassThrough(runner);
            }
            Ok(RecoveredLifecycleCompletionTakeV1::Apply(completion)) => {
                let selected = match self
                    .settle_recovered_decision_apply_completion_owner(completion, lane_work)
                {
                    Ok(ProductionRecoveredDecisionApplyCompletionV1::Applied) => {
                        ProductionLifecycleCompletionSelectionV1::RecoveredDecisionApplyApplied
                    }
                    Ok(ProductionRecoveredDecisionApplyCompletionV1::Deferred(deferred)) => {
                        assert!(self.recovered_decision_apply_deferred.is_none());
                        self.recovered_decision_apply_deferred = Some(deferred);
                        ProductionLifecycleCompletionSelectionV1::RecoveredDecisionApplyCompletionDeferred
                    }
                    Ok(ProductionRecoveredDecisionApplyCompletionV1::None) | Err(_) => {
                        self.close_output_for_restart();
                        ProductionLifecycleCompletionSelectionV1::RecoveredDecisionApplyCompletionRestartRequired
                    }
                };
                return ProductionLifecycleCompletionTurnV1::Selected(selected);
            }
            Ok(RecoveredLifecycleCompletionTakeV1::Sign(completion)) => {
                assert!(self.recovered_lifecycle_sign_completion.is_none());
                self.recovered_lifecycle_sign_completion = Some(completion);
                let selected = self.settle_parked_recovered_sign_completion();
                return ProductionLifecycleCompletionTurnV1::Selected(
                    ProductionLifecycleCompletionSelectionV1::RecoveredLifecycleSignCompletion(
                        selected,
                    ),
                );
            }
            Ok(RecoveredLifecycleCompletionTakeV1::DecisionFetch(completion)) => {
                assert!(self.recovered_decision_fetch_body_completion.is_none());
                self.recovered_decision_fetch_body_completion = Some(completion);
                let selected = self.settle_recovered_decision_fetch_store();
                return ProductionLifecycleCompletionTurnV1::Selected(
                    ProductionLifecycleCompletionSelectionV1::RecoveredDecisionFetchCompletion(
                        selected,
                    ),
                );
            }
            Ok(RecoveredLifecycleCompletionTakeV1::None) => {}
            Err(_) => {
                self.close_output_for_restart();
                return ProductionLifecycleCompletionTurnV1::Selected(
                    ProductionLifecycleCompletionSelectionV1::RestartRequired,
                );
            }
        }

        let selected = match self.owner.classify_completion_ready_work() {
            super::super::ProductionCompletionReadyWorkV1::None
            | super::super::ProductionCompletionReadyWorkV1::PassThrough => {
                return ProductionLifecycleCompletionTurnV1::PassThrough(runner);
            }
            super::super::ProductionCompletionReadyWorkV1::Invalid => {
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
                if result.is_err() {
                    self.close_output_for_restart();
                }
                ProductionLifecycleCompletionSelectionV1::RecoveredIoDispatch(result)
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
                if result.is_err() {
                    self.close_output_for_restart();
                }
                ProductionLifecycleCompletionSelectionV1::RecoveredLifecycleBroadcastRefanout(
                    result,
                )
            }
        };
        ProductionLifecycleCompletionTurnV1::Selected(selected)
    }

    /// Service one exact outer Ingress turn through recovered Fetch Phase A.
    ///
    /// A retained capacity wait is classified before any fresh queue probe.
    /// Fresh selection accepts only the existing queue-owned recovered winner;
    /// every other winner returns the unchanged current turn.
    // TODO: Route the ordinary winner selected by `v2_runner::run_inner` through this
    // activated consumer at the atomic process-lifetime scheduler cutover.
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

        if let Some(wait) = self.recovered_ingress_capacity_wait.take() {
            match wait.retry(&self.services, &self.executor) {
                super::super::ProductionIngressCapacityRetry::Pending(wait) => {
                    assert!(self.recovered_ingress_capacity_wait.is_none());
                    self.recovered_ingress_capacity_wait = Some(wait);
                    return ProductionLifecycleIngressTurnV1::Selected(
                        ProductionLifecycleIngressSelectionV1::CapacityPending,
                    );
                }
                super::super::ProductionIngressCapacityRetry::Released(selector) => {
                    return self.drive_recovered_ingress_selector(selector, runner);
                }
                super::super::ProductionIngressCapacityRetry::RestartRequired => {
                    self.close_output_for_restart();
                    return ProductionLifecycleIngressTurnV1::Selected(
                        ProductionLifecycleIngressSelectionV1::RestartRequired,
                    );
                }
            }
        }

        let terminal_subject = match self.executor.lifecycle_terminal_subject() {
            Ok(subject) => subject,
            Err(_) => {
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
            Err(_) => {
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
                executor, services, ..
            } = self;
            return prepare_and_dequeue_current_certified_serve(
                executor,
                services,
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
                    Err(_) => {
                        self.close_output_for_restart();
                        drop(cut);
                        drop(runner);
                        return ProductionLifecycleIngressTurnV1::Selected(
                            ProductionLifecycleIngressSelectionV1::RestartRequired,
                        );
                    }
                };
                if !recovered {
                    return dequeue_prepared_ordinary_ingress(
                        &ingress,
                        cut.into_ordinary_turn_cut(),
                        runner,
                        None,
                        terminal_subject,
                        &self.services,
                    );
                }
                let selector = match self
                    .executor
                    .prepare_recovered_decision_fetch_from_selected_cut(cut)
                {
                    Ok(selector) => selector,
                    Err(_) => {
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
            );
        let selected = match result {
            Ok(ProductionRecoveredDecisionFetchPersistenceV1::CapacityWait(wait)) => {
                assert!(self.recovered_ingress_capacity_wait.is_none());
                self.recovered_ingress_capacity_wait = Some(wait);
                ProductionLifecycleIngressSelectionV1::CapacityPending
            }
            Ok(ProductionRecoveredDecisionFetchPersistenceV1::Queued { .. }) => {
                ProductionLifecycleIngressSelectionV1::RecoveredDecisionFetchQueued
            }
            Err(
                ProductionRecoveredDecisionFetchPersistenceErrorV1::CommandPreparation(prepared)
                | ProductionRecoveredDecisionFetchPersistenceErrorV1::InFlightSelectedWork(prepared),
            ) => {
                drop(prepared);
                ProductionLifecycleIngressSelectionV1::Retry
            }
            Err(ProductionRecoveredDecisionFetchPersistenceErrorV1::Service {
                prepared, ..
            }) => {
                drop(prepared);
                self.close_output_for_restart();
                ProductionLifecycleIngressSelectionV1::RestartRequired
            }
            Err(
                ProductionRecoveredDecisionFetchPersistenceErrorV1::ForeignRunnerObservation
                | ProductionRecoveredDecisionFetchPersistenceErrorV1::InvalidClaimedCarrier
                | ProductionRecoveredDecisionFetchPersistenceErrorV1::ForeignOwner
                | ProductionRecoveredDecisionFetchPersistenceErrorV1::InvalidReservedCommand
                | ProductionRecoveredDecisionFetchPersistenceErrorV1::Claim(_),
            ) => {
                self.close_output_for_restart();
                ProductionLifecycleIngressSelectionV1::RestartRequired
            }
        };
        drop(runner);
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
        let completion = self.recovered_lifecycle_sign_completion.as_ref()?;
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

    fn close_output_for_restart(&self) {
        self.services
            .lifecycle_output_guard()
            .close_admission_for_restart();
    }
}

impl ActivatedProductionLifecycleV1 {
    /// Forward one Completion turn without exposing the launched stack.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::sumeragi) fn drive_completion_turn<'cursor>(
        &mut self,
        runner: LifecycleCurrentRunnerTurn<'cursor>,
        lane_work: &mut V2LaneWorkAdapter,
    ) -> ProductionLifecycleCompletionTurnV1<'cursor> {
        self.launched.drive_completion_turn(runner, lane_work)
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

    /// Inspect whether the test service retains no Serve barrier or dormant owner.
    #[cfg(test)]
    pub(in crate::sumeragi) fn certified_serve_owners_are_clear_for_test(
        &self,
    ) -> Result<bool, String> {
        Ok(self
            .launched
            .services
            .certified_serve_barrier_request_hash()?
            .is_none()
            && self
                .launched
                .services
                .dormant_certified_serve_ingress_scheduler_ordinal()?
                .is_none())
    }

    /// Match the retained test Serve barrier to one exact request hash.
    #[cfg(test)]
    pub(in crate::sumeragi) fn certified_serve_barrier_matches_for_test(
        &self,
        expected: iroha_crypto::HashOf<iroha_data_model::block::consensus_v2::CertifiedBodyRequest>,
    ) -> Result<bool, String> {
        Ok(self
            .launched
            .services
            .certified_serve_barrier_request_hash()?
            == Some(expected))
    }

    /// Claim and immediately finish one finite local-producer episode in tests.
    #[cfg(test)]
    pub(in crate::sumeragi) fn discharge_certified_serve_producer_episode_for_test(
        &self,
    ) -> Result<bool, String> {
        let episode = self
            .launched
            .services
            .try_begin_certified_serve_producer_episode()?;
        let claimed = episode.is_some();
        drop(episode);
        Ok(claimed)
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
        let ingress = Arc::new(FairV2Ingress::new(4, 1024 * 1024, 512 * 1024, 0, 0));
        ingress
            .configure_roster([peer.clone()])
            .expect("configure one exact validator lane");
        ingress.open().expect("open exact ordinary-token ingress");
        let message = crate::sumeragi::v2_worker::tests::lane_commit_qc_block_message(peer.clone());
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(message, Some(peer))),
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
