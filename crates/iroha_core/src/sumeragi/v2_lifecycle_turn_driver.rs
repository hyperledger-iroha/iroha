//! Unified lifecycle ownership for one outer Completion or Ingress turn.

use super::*;

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
#[must_use = "the lifecycle-selected Completion result must be observed"]
pub(in crate::sumeragi) enum ProductionLifecycleCompletionSelectionV1 {
    /// The exact missing-sidecar Apply owner remains parked for another turn.
    RecoveredDecisionApplyDeferred,
    /// The unchanged deferred Apply command re-entered its dedicated FIFO.
    RecoveredDecisionApplyRequeued,
    /// Deferred Apply ownership changed and process restart is required.
    RecoveredDecisionApplyRestartRequired,
    /// One recovered Apply command was dispatched or met bounded capacity.
    RecoveredDecisionApplyDispatch(
        Result<
            ProductionRecoveredDecisionApplyDispatchV1,
            ProductionRecoveredDecisionApplyDispatchErrorV1,
        >,
    ),
    /// One recovered Apply worker result was durably settled.
    RecoveredDecisionApplyApplied,
    /// One recovered Apply worker result became the retained sidecar owner.
    RecoveredDecisionApplyCompletionDeferred,
    /// Recovered Apply settlement requires cold restart.
    RecoveredDecisionApplyCompletionRestartRequired,
    /// One recovered Sign command was dispatched or met bounded capacity.
    RecoveredLifecycleSignDispatch(
        Result<
            ProductionRecoveredLifecycleSignDispatchV1,
            ProductionRecoveredLifecycleSignDispatchErrorV1,
        >,
    ),
    /// One parked recovered Sign used exactly one successor-family settler.
    RecoveredLifecycleSignCompletion(ProductionRecoveredLifecycleSignCompletionSelectionV1),
    /// One recovered Decision Fetch request was dispatched or met output capacity.
    RecoveredDecisionFetchDispatch(
        Result<
            ProductionRecoveredDecisionFetchDispatchV1,
            ProductionRecoveredDecisionFetchDispatchErrorV1,
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
    /// The selected recovered owner changed and process restart is required.
    RestartRequired,
}

/// Outcome of one borrow-bound outer Ingress turn.
///
/// The pass-through variant deliberately retains the real runner cursor. It
/// does not claim an ordinary queue witness that the current read-only selector
/// cannot preserve.
#[must_use = "ordinary pass-through must retain the current Ingress turn"]
pub(in crate::sumeragi) enum ProductionLifecycleIngressTurnV1<'cursor> {
    /// The exact fair winner belongs to the ordinary/stateful ingress owner.
    PassThrough(LifecycleCurrentRunnerTurn<'cursor>),
    /// Recovered Decision Fetch work consumed this Ingress turn.
    Selected(ProductionLifecycleIngressSelectionV1),
}

#[derive(Clone, Copy)]
enum RecoveredSignCompletionClassV1 {
    Broadcast,
    ProposalPrepareWal,
    VoteBroadcastAndSign,
    ProposalBroadcastAndSign,
}

impl LaunchedProductionLifecycleV1 {
    /// Service one exact outer Completion turn through the lifecycle owner.
    ///
    /// Classification precedes cursor consumption. Ordinary work returns the
    /// same borrow-bound turn, while a recovered class is dispatched, drained,
    /// or settled internally without exposing mutually exclusive methods.
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
            super::super::ProductionCompletionReadyWorkV1::RecoveredDecisionApply => {
                let Self {
                    owner,
                    executor,
                    services,
                    ..
                } = self;
                ProductionLifecycleCompletionSelectionV1::RecoveredDecisionApplyDispatch(
                    owner.dispatch_recovered_decision_apply_with_runner_debt(
                        services,
                        executor,
                        runner.debt(),
                    ),
                )
            }
            super::super::ProductionCompletionReadyWorkV1::RecoveredLifecycleSign => {
                let Self {
                    owner,
                    executor,
                    services,
                    ..
                } = self;
                ProductionLifecycleCompletionSelectionV1::RecoveredLifecycleSignDispatch(
                    owner.dispatch_recovered_lifecycle_sign_with_runner_debt(
                        services,
                        executor,
                        runner.debt(),
                    ),
                )
            }
            super::super::ProductionCompletionReadyWorkV1::RecoveredDecisionFetch => {
                let Self {
                    owner,
                    executor,
                    services,
                    ..
                } = self;
                ProductionLifecycleCompletionSelectionV1::RecoveredDecisionFetchDispatch(
                    owner.dispatch_recovered_decision_fetch_with_runner_debt(
                        services,
                        executor,
                        runner.debt(),
                    ),
                )
            }
            super::super::ProductionCompletionReadyWorkV1::RecoveredLifecycleBroadcast => {
                let Self {
                    owner, services, ..
                } = self;
                ProductionLifecycleCompletionSelectionV1::RecoveredLifecycleBroadcastRefanout(
                    owner.refanout_recovered_lifecycle_signed_broadcast_with_runner_debt(
                        services,
                        runner.debt(),
                    ),
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

        let selector = match self
            .executor
            .prepare_next_recovered_decision_fetch_ingress_selector(
                &self.leader_wire_ingress_binding.ingress,
            ) {
            Ok(Some(selector)) => selector,
            Ok(None) | Err(_) => {
                // TODO: Replace this read-only boundary with one queue-owned
                // turn preparation that retains the selected ordinary witness,
                // dequeue service guard, and any stateful Certified-Serve
                // preparation/capacity wait. The present selector cannot mint
                // those owners, so returning the unchanged runner turn is the
                // strongest sound boundary and must not claim atomic ordinary
                // pass-through parity.
                return ProductionLifecycleIngressTurnV1::PassThrough(runner);
            }
        };
        self.drive_recovered_ingress_selector(selector, runner)
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
                ProductionLifecycleIngressSelectionV1::Retry
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
            RecoveredSignCompletionClassV1::Broadcast => {
                ProductionRecoveredLifecycleSignCompletionSelectionV1::Broadcast(
                    self.settle_recovered_lifecycle_sign_broadcast(),
                )
            }
            RecoveredSignCompletionClassV1::ProposalPrepareWal => {
                ProductionRecoveredLifecycleSignCompletionSelectionV1::ProposalPrepareWal(
                    self.settle_recovered_lifecycle_proposal_prepare_wal(),
                )
            }
            RecoveredSignCompletionClassV1::VoteBroadcastAndSign => {
                ProductionRecoveredLifecycleSignCompletionSelectionV1::VoteBroadcastAndSign(
                    self.settle_recovered_lifecycle_vote_broadcast_and_sign(),
                )
            }
            RecoveredSignCompletionClassV1::ProposalBroadcastAndSign => {
                ProductionRecoveredLifecycleSignCompletionSelectionV1::ProposalBroadcastAndSign(
                    self.settle_recovered_lifecycle_proposal_broadcast_and_sign(),
                )
            }
        }
    }

    fn classify_parked_recovered_sign_completion(
        &mut self,
    ) -> Option<RecoveredSignCompletionClassV1> {
        let completion = self.recovered_lifecycle_sign_completion.as_ref()?;
        let authority = completion.project_adapter_completion_authority()?;
        let preview = self
            .executor
            .prepare_recovered_lifecycle_sign_completion(authority)
            .ok()?;
        let class = match preview.shape() {
            crate::sumeragi::v2::RecoveredLifecycleSignAdapterSuccessorShapeV1::Broadcast => {
                RecoveredSignCompletionClassV1::Broadcast
            }
            crate::sumeragi::v2::RecoveredLifecycleSignAdapterSuccessorShapeV1::ProposalPrepareWal => {
                RecoveredSignCompletionClassV1::ProposalPrepareWal
            }
            crate::sumeragi::v2::RecoveredLifecycleSignAdapterSuccessorShapeV1::BroadcastAndSign
                if preview.is_vote_broadcast_and_sign_shape() =>
            {
                RecoveredSignCompletionClassV1::VoteBroadcastAndSign
            }
            crate::sumeragi::v2::RecoveredLifecycleSignAdapterSuccessorShapeV1::BroadcastAndSign => {
                RecoveredSignCompletionClassV1::ProposalBroadcastAndSign
            }
        };
        drop(preview);
        Some(class)
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
    pub(in crate::sumeragi) fn drive_completion_turn<'cursor>(
        &mut self,
        runner: LifecycleCurrentRunnerTurn<'cursor>,
        lane_work: &mut V2LaneWorkAdapter,
    ) -> ProductionLifecycleCompletionTurnV1<'cursor> {
        self.launched.drive_completion_turn(runner, lane_work)
    }

    /// Forward one Ingress turn without exposing the launched stack.
    pub(in crate::sumeragi) fn drive_ingress_turn<'cursor>(
        &mut self,
        runner: LifecycleCurrentRunnerTurn<'cursor>,
    ) -> ProductionLifecycleIngressTurnV1<'cursor> {
        self.launched.drive_ingress_turn(runner)
    }
}
