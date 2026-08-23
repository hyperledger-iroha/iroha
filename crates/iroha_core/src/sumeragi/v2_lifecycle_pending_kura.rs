//! No-clock lifecycle ownership for one interrupted canonical Kura tip.

use thiserror::Error;

use super::*;
use crate::sumeragi::{
    Queue,
    v2_lifecycle_coordinator::{
        AttemptedProducerTurnV1, ClaimedProducerTurnV1, ProducerTurnSchedulerClaimErrorV1,
        ProducerTurnTerminalSettlementErrorV1,
    },
};

/// Result of one bounded closed-ingress interrupted-tip recovery turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use = "pending Kura recovery progress must be observed"]
pub(in crate::sumeragi) enum ProductionPendingKuraApplyRecoveryProgressV1 {
    /// Local completion or reducer work advanced, but Apply is not yet final.
    Advanced {
        /// Number of service completions accepted during this turn.
        completions: usize,
        /// Number of lifecycle actions or reducer effects dispatched this turn.
        effects: usize,
        /// Total bounded reducer-recovery scheduler attempts for this height.
        attempts: u64,
        /// Exact authenticated interrupted-tip stage after the turn.
        stage: crate::sumeragi::v2_effects::PendingKuraApplyRecoveryStage,
    },
    /// No local completion or reducer effect was ready this turn.
    Waiting {
        /// Total bounded reducer-recovery scheduler attempts for this height.
        attempts: u64,
        /// Exact authenticated interrupted-tip stage after the turn.
        stage: crate::sumeragi::v2_effects::PendingKuraApplyRecoveryStage,
    },
    /// The exact local Apply and reducer finality boundary are complete.
    Completed {
        /// Total bounded reducer-recovery scheduler attempts for this height.
        attempts: u64,
    },
}

/// Fail-stop failure while driving the closed-ingress interrupted-tip replay.
#[derive(Debug, Error)]
#[allow(variant_size_differences)]
#[must_use = "failed pending Kura recovery requires process restart"]
pub(in crate::sumeragi) enum ProductionPendingKuraApplyRecoveryErrorV1 {
    /// The stack was no longer in its exact closed-ingress setup state.
    #[error(transparent)]
    Setup(#[from] ProductionLifecyclePreActivationErrorV1),
    /// The replay was not installed, or its terminal evidence changed.
    #[error("launched lifecycle has no exact installed pending Kura recovery evidence")]
    MissingEvidence,
    /// The replay reached Completed without a fully drained finality owner.
    #[error("pending Kura recovery completed without exact drained finality")]
    IncompleteFinality,
    /// Local completion or reducer replay failed closed.
    #[error(transparent)]
    Effect(#[from] crate::sumeragi::v2_effects::EffectExecutorError),
    /// The closed-ingress lifecycle subturn observed a foreign or invalid owner.
    #[error("pending Kura lifecycle Apply subturn failed closed: {0}")]
    Lifecycle(&'static str),
}

/// Installed interrupted-tip lifecycle before its no-clock ingress opens.
///
/// This state retains the authenticated expected tip across local Apply and
/// lane startup. It cannot enter ordinary activation or local proposal work.
#[must_use = "installed pending Kura lifecycle must recover, activate, or shut down"]
pub(in crate::sumeragi) struct PendingKuraProductionLifecycleV1 {
    installed: crate::sumeragi::v2::InstalledPendingKuraApplyV1,
    launched: LaunchedProductionLifecycleV1,
}

/// Installed interrupted tip joined to its exact activated lane adapter.
///
/// The lane adapter cannot leave this state independently. No-clock activation
/// consumes the join and retains that same adapter inside the active lifecycle.
#[must_use = "prepared pending Kura lane recovery must activate or shut down"]
pub(in crate::sumeragi) struct PreparedPendingKuraLaneRecoveryV1 {
    installed: crate::sumeragi::v2::InstalledPendingKuraApplyV1,
    lane_work: V2LaneWorkAdapter,
    launched: LaunchedProductionLifecycleV1,
}

/// Activated interrupted-tip height with no pacemaker or successor authority.
///
/// The exact runner ingress is open only for the already-decided lane/output
/// recovery. This state deliberately has no ordinary Completion/Ingress turn
/// driver, local-Proposal owner, or clock transition. Its sole status write is
/// the recovered current-height snapshot published at activation.
#[must_use = "pending Kura lifecycle must be finalized or shut down explicitly"]
pub(in crate::sumeragi) struct PendingKuraActivatedProductionLifecycleV1 {
    // Close readiness and ingress before the launched stack retires durable gates.
    runner_activation: crate::sumeragi::v2_runner::ProductionLifecycleActivatedRunnerAuthorityV1,
    // Retain the exact applied tip until readiness and ingress retire.
    installed: crate::sumeragi::v2::InstalledPendingKuraApplyV1,
    // The exact activated lane adapter cannot separate while ingress is live.
    lane_work: V2LaneWorkAdapter,
    launched: LaunchedProductionLifecycleV1,
}

impl LaunchedProductionLifecycleV1 {
    /// Install pending-Kura provenance while ingress and clocks stay closed.
    ///
    /// The consuming transition retains the authenticated expected tip beside
    /// the launched stack. Verification rejoins it to the storage-only runtime
    /// owner and its exact recovered Ready Apply carrier without opening
    /// ordinary ingress or replaying already revalidated body stages.
    #[allow(dead_code, clippy::result_large_err)]
    pub(in crate::sumeragi) fn install_pending_kura_apply(
        mut self,
        runner: &mut crate::sumeragi::v2_runner::ProductionLifecyclePreActivationRunnerBorrowV1,
    ) -> Result<PendingKuraProductionLifecycleV1, ProductionPendingKuraApplyInstallErrorV1> {
        let Some(replay) = self.pending_kura_apply_replay.take() else {
            let output_guard = self.services.lifecycle_output_guard();
            return Err(super::preactivation::missing_pending_kura_replay(
                output_guard.as_ref(),
            ));
        };
        let installed = self.with_runner_setup(runner, move |executor, _services| {
            replay
                .install(executor)
                .map_err(ProductionPendingKuraApplyInstallErrorV1::Effect)
        })?;
        Ok(PendingKuraProductionLifecycleV1 {
            installed,
            launched: self,
        })
    }
}

impl PendingKuraProductionLifecycleV1 {
    /// Run the sole pending-Kura Completion subturn under a closed setup cut.
    ///
    /// Keeping this authority on the already-installed pending-Kura wrapper
    /// prevents ordinary or complete-tip preactivation states from borrowing
    /// the lifecycle owner before activation.
    #[allow(clippy::type_complexity)]
    fn with_lifecycle_setup_transaction<R, E>(
        &mut self,
        _runner: &mut crate::sumeragi::v2_runner::ProductionLifecyclePreActivationRunnerBorrowV1,
        operation: impl FnOnce(
            &mut ProductionLifecycleOwnerV1,
            &mut V2EffectExecutor<SerializedV2Runtime>,
            &mut ProductionV2Services,
        ) -> Result<R, E>,
    ) -> Result<R, E>
    where
        E: From<ProductionLifecyclePreActivationErrorV1>,
    {
        let launched = &mut self.launched;
        let output_guard = launched.services.lifecycle_output_guard();
        let initial_admission = output_guard
            .acquire()
            .ok_or_else(|| E::from(ProductionLifecyclePreActivationErrorV1::OutputClosed))?;
        let setup = super::preactivation::ProductionLifecyclePreActivationFailStopScopeV1::new(
            Arc::clone(&output_guard),
        );
        drop(initial_admission);
        let preflight_failure = if !launched
            .services
            .matches_lifecycle_executor_output_guard(&launched.executor)
        {
            Some(ProductionLifecyclePreActivationErrorV1::OwnershipMismatch)
        } else if launched
            .leader_wire_ingress_binding
            .ingress
            .state
            .lock()
            .open
        {
            Some(ProductionLifecyclePreActivationErrorV1::IngressAlreadyOpen)
        } else if launched.completion_observer_activation.is_none() {
            Some(ProductionLifecyclePreActivationErrorV1::CompletionObserverMissing)
        } else if !launched.executor.lifecycle_live_clocks_are_unarmed() {
            Some(ProductionLifecyclePreActivationErrorV1::ClocksAlreadyArmed)
        } else {
            None
        };
        if let Some(error) = preflight_failure {
            return Err(E::from(error));
        }
        let value = operation(
            &mut launched.owner,
            &mut launched.executor,
            &mut launched.services,
        )?;
        let postflight_failure = if !launched
            .services
            .matches_lifecycle_executor_output_guard(&launched.executor)
        {
            Some(ProductionLifecyclePreActivationErrorV1::OwnershipMismatch)
        } else if launched
            .leader_wire_ingress_binding
            .ingress
            .state
            .lock()
            .open
        {
            Some(ProductionLifecyclePreActivationErrorV1::IngressAlreadyOpen)
        } else if launched.completion_observer_activation.is_none() {
            Some(ProductionLifecyclePreActivationErrorV1::CompletionObserverMissing)
        } else if !launched.executor.lifecycle_live_clocks_are_unarmed() {
            Some(ProductionLifecyclePreActivationErrorV1::ClocksAlreadyArmed)
        } else {
            None
        };
        if let Some(error) = postflight_failure {
            return Err(E::from(error));
        }
        let final_admission = output_guard
            .acquire()
            .ok_or_else(|| E::from(ProductionLifecyclePreActivationErrorV1::OutputClosed))?;
        setup.complete();
        drop(final_admission);
        Ok(value)
    }

    /// Borrow executor and services while interrupted-tip ingress stays closed.
    #[allow(dead_code, clippy::type_complexity)]
    pub(in crate::sumeragi) fn with_runner_setup<R, E>(
        &mut self,
        runner: &mut crate::sumeragi::v2_runner::ProductionLifecyclePreActivationRunnerBorrowV1,
        operation: impl FnOnce(
            &mut V2EffectExecutor<SerializedV2Runtime>,
            &mut ProductionV2Services,
        ) -> Result<R, E>,
    ) -> Result<R, E>
    where
        E: From<ProductionLifecyclePreActivationErrorV1>,
    {
        self.launched.with_runner_setup(runner, operation)
    }

    /// Temporarily open the exact ingress for canonical startup body recovery.
    #[allow(dead_code, clippy::type_complexity)]
    pub(in crate::sumeragi) fn with_canonical_body_recovery_ingress<R, E>(
        &mut self,
        runner: &mut crate::sumeragi::v2_runner::ProductionLifecyclePreActivationRunnerBorrowV1,
        activation: &mut crate::sumeragi::v2_runner::ProductionLifecyclePendingKuraRunnerActivationV1,
        operation: impl FnOnce(
            &crate::sumeragi::v2_runner::ProductionLifecycleCanonicalRecoveryIngressV1<'_>,
            &mut V2EffectExecutor<SerializedV2Runtime>,
            &mut ProductionV2Services,
        ) -> Result<R, E>,
    ) -> Result<R, E>
    where
        E: From<ProductionLifecyclePreActivationErrorV1>,
    {
        self.launched
            .with_pending_kura_canonical_body_recovery_ingress(runner, activation, operation)
    }

    /// Drive one bounded local-only interrupted-tip recovery turn.
    ///
    /// The ingress remains closed and live clocks remain unarmed throughout.
    /// A completed result proves that the exact replayed Apply finality is
    /// drained and ready for the later no-clock lane-recovery state.
    #[allow(dead_code, clippy::result_large_err)]
    pub(in crate::sumeragi) fn drive_apply_recovery_turn(
        &mut self,
        runner: &mut crate::sumeragi::v2_runner::ProductionLifecyclePreActivationRunnerBorrowV1,
    ) -> Result<
        ProductionPendingKuraApplyRecoveryProgressV1,
        ProductionPendingKuraApplyRecoveryErrorV1,
    > {
        if self.launched.pending_kura_apply_replay.is_some() {
            self.launched
                .services
                .lifecycle_output_guard()
                .close_admission_for_restart();
            return Err(ProductionPendingKuraApplyRecoveryErrorV1::MissingEvidence);
        }
        self.with_lifecycle_setup_transaction(runner, |owner, executor, services| {
            use crate::sumeragi::v2_effects::PendingTipRecoveryAttemptResult as AttemptResult;

            executor.begin_pending_tip_recovery_attempt();
            let stage = executor
                .pending_kura_apply_recovery_evidence()
                .map(|evidence| evidence.stage())
                .ok_or(ProductionPendingKuraApplyRecoveryErrorV1::MissingEvidence)?;
            if stage == crate::sumeragi::v2_effects::PendingKuraApplyRecoveryStage::Completed {
                if !executor.ready_to_finish() {
                    return Err(ProductionPendingKuraApplyRecoveryErrorV1::IncompleteFinality);
                }
                let attempts =
                    executor.settle_pending_tip_recovery_attempt(AttemptResult::Completed);
                return Ok(ProductionPendingKuraApplyRecoveryProgressV1::Completed { attempts });
            }

            use crate::sumeragi::v2_effects::PendingKuraApplyRecoveryStage as Stage;
            let mut completions = 0_usize;
            let mut effects = 0_usize;
            if matches!(stage, Stage::Apply | Stage::ApplicationDispatched) {
                let mut dispatch_ready_apply = false;
                match services.take_next_lifecycle_completion() {
                    Ok(LifecycleCompletionTakeV1::None) => {
                        dispatch_ready_apply = stage == Stage::Apply;
                    }
                    Ok(LifecycleCompletionTakeV1::PassThrough) => {
                        completions = completions.saturating_add(
                            services.drain_one_ordinary_completion_after_lifecycle_pass_through(
                                executor,
                            )?,
                        );
                        dispatch_ready_apply = stage == Stage::Apply;
                    }
                    Ok(LifecycleCompletionTakeV1::Apply(completion))
                        if stage == Stage::ApplicationDispatched =>
                    {
                        if !matches!(
                            completion.result(),
                            LifecycleDecisionApplyWorkerResultV1::Applied(_)
                        ) {
                            drop(completion);
                            return Err(ProductionPendingKuraApplyRecoveryErrorV1::Lifecycle(
                                "typed Apply deferred before lane recovery",
                            ));
                        }
                        match super::settle_pending_kura_applied_decision_apply_completion(
                            owner, executor, completion,
                        ) {
                            Ok(ProductionLifecycleDecisionApplyCompletionV1::Applied) => {
                                completions = completions.saturating_add(1);
                            }
                            Ok(ProductionLifecycleDecisionApplyCompletionV1::Deferred(
                                deferred,
                            )) => {
                                drop(deferred);
                                return Err(ProductionPendingKuraApplyRecoveryErrorV1::Lifecycle(
                                    "typed Apply settlement deferred before lane recovery",
                                ));
                            }
                            Err(_) => {
                                return Err(ProductionPendingKuraApplyRecoveryErrorV1::Lifecycle(
                                    "typed Apply terminal settlement",
                                ));
                            }
                        }
                    }
                    Ok(LifecycleCompletionTakeV1::Apply(completion)) => {
                        drop(completion);
                        return Err(ProductionPendingKuraApplyRecoveryErrorV1::Lifecycle(
                            "typed Apply completed before dispatch-stage ownership",
                        ));
                    }
                    Ok(LifecycleCompletionTakeV1::CertifiedFetch(completion)) => {
                        drop(completion);
                        return Err(ProductionPendingKuraApplyRecoveryErrorV1::Lifecycle(
                            "foreign lifecycle Fetch completion",
                        ));
                    }
                    Ok(LifecycleCompletionTakeV1::DecisionFetch(completion)) => {
                        drop(completion);
                        return Err(ProductionPendingKuraApplyRecoveryErrorV1::Lifecycle(
                            "foreign lifecycle Fetch completion",
                        ));
                    }
                    Ok(LifecycleCompletionTakeV1::Sign(completion)) => {
                        drop(completion);
                        return Err(ProductionPendingKuraApplyRecoveryErrorV1::Lifecycle(
                            "foreign lifecycle Sign completion",
                        ));
                    }
                    Ok(LifecycleCompletionTakeV1::Validate(completion)) => {
                        drop(completion);
                        return Err(ProductionPendingKuraApplyRecoveryErrorV1::Lifecycle(
                            "foreign lifecycle Validate completion",
                        ));
                    }
                    Ok(LifecycleCompletionTakeV1::CertifiedServe(completion)) => {
                        drop(completion);
                        return Err(ProductionPendingKuraApplyRecoveryErrorV1::Lifecycle(
                            "foreign lifecycle Serve completion",
                        ));
                    }
                    Err(_) => {
                        return Err(ProductionPendingKuraApplyRecoveryErrorV1::Lifecycle(
                            "physical completion classification",
                        ));
                    }
                }
                if executor
                    .pending_kura_apply_recovery_evidence()
                    .is_some_and(|evidence| evidence.stage() == Stage::Completed)
                {
                    if !executor.ready_to_finish() {
                        return Err(ProductionPendingKuraApplyRecoveryErrorV1::IncompleteFinality);
                    }
                    let attempts =
                        executor.settle_pending_tip_recovery_attempt(AttemptResult::Completed);
                    return Ok(ProductionPendingKuraApplyRecoveryProgressV1::Completed { attempts });
                }
                if stage == Stage::ApplicationDispatched {
                    return if completions == 0 {
                        let attempts =
                            executor.settle_pending_tip_recovery_attempt(AttemptResult::Waiting);
                        Ok(ProductionPendingKuraApplyRecoveryProgressV1::Waiting {
                            attempts,
                            stage,
                        })
                    } else {
                        let attempts =
                            executor.settle_pending_tip_recovery_attempt(AttemptResult::Advanced);
                        Ok(ProductionPendingKuraApplyRecoveryProgressV1::Advanced {
                            completions,
                            effects,
                            attempts,
                            stage,
                        })
                    };
                }
                if dispatch_ready_apply {
                    let recovered_apply_ordinal = executor
                        .pending_kura_apply_recovery_evidence()
                        .ok_or(ProductionPendingKuraApplyRecoveryErrorV1::MissingEvidence)?
                        .recovered_apply_ordinal();
                    let fence = executor.lifecycle_reducer_fence_observation();
                    match owner.classify_completion_ready_work(fence) {
                        super::super::ProductionCompletionReadyWorkV1::None => {}
                        super::super::ProductionCompletionReadyWorkV1::CompletionIo => {
                            match owner.dispatch_completion_requiring_ready_ordinal(
                                services,
                                executor,
                                0,
                                recovered_apply_ordinal,
                            ) {
                                Ok(ProductionCompletionDispatchV1::ApplyQueued { ordinal })
                                    if ordinal == recovered_apply_ordinal =>
                                {
                                    effects = effects.saturating_add(1);
                                }
                                Ok(ProductionCompletionDispatchV1::CapacityUnavailable {
                                    protected_live_apply_ordinal: None,
                                }) => {
                                    return if completions == 0 {
                                        let attempts = executor
                                            .settle_pending_tip_recovery_attempt(
                                                AttemptResult::Waiting,
                                            );
                                        Ok(ProductionPendingKuraApplyRecoveryProgressV1::Waiting {
                                            attempts,
                                            stage,
                                        })
                                    } else {
                                        let attempts = executor
                                            .settle_pending_tip_recovery_attempt(
                                                AttemptResult::Advanced,
                                            );
                                        Ok(ProductionPendingKuraApplyRecoveryProgressV1::Advanced {
                                            completions,
                                            effects,
                                            attempts,
                                            stage,
                                        })
                                    };
                                }
                                Ok(_) | Err(_) => {
                                    return Err(
                                        ProductionPendingKuraApplyRecoveryErrorV1::Lifecycle(
                                            "exact pending Apply dispatch",
                                        ),
                                    );
                                }
                            }
                            let evidence = executor
                                .pending_kura_apply_recovery_evidence()
                                .ok_or(
                                    ProductionPendingKuraApplyRecoveryErrorV1::MissingEvidence,
                                )?;
                            if evidence.stage() != Stage::ApplicationDispatched {
                                return Err(ProductionPendingKuraApplyRecoveryErrorV1::Lifecycle(
                                    "queued pending Apply did not advance its exact stage",
                                ));
                            }
                            let stage = evidence.stage();
                            let attempts = executor
                                .settle_pending_tip_recovery_attempt(AttemptResult::Advanced);
                            return Ok(ProductionPendingKuraApplyRecoveryProgressV1::Advanced {
                                completions,
                                effects,
                                attempts,
                                stage,
                            });
                        }
                        super::super::ProductionCompletionReadyWorkV1::RecoveredLifecycleBroadcast => {
                            return Err(ProductionPendingKuraApplyRecoveryErrorV1::Lifecycle(
                                "recovered Broadcast preceded the exact pending Apply",
                            ));
                        }
                        super::super::ProductionCompletionReadyWorkV1::PassThrough => {
                            return Err(ProductionPendingKuraApplyRecoveryErrorV1::Lifecycle(
                                "ordinary Ready work or an active lease preceded the exact pending Apply",
                            ));
                        }
                        super::super::ProductionCompletionReadyWorkV1::Invalid => {
                            return Err(ProductionPendingKuraApplyRecoveryErrorV1::Lifecycle(
                                "pending Apply Ready census was invalid",
                            ));
                        }
                    }
                }
            } else {
                return Err(ProductionPendingKuraApplyRecoveryErrorV1::Lifecycle(
                    "pending Apply recovery entered a pre-Apply stage",
                ));
            }
            if executor
                .pending_kura_apply_recovery_evidence()
                .is_some_and(|evidence| evidence.stage() == Stage::Completed)
            {
                if !executor.ready_to_finish() {
                    return Err(ProductionPendingKuraApplyRecoveryErrorV1::IncompleteFinality);
                }
                let attempts =
                    executor.settle_pending_tip_recovery_attempt(AttemptResult::Completed);
                return Ok(ProductionPendingKuraApplyRecoveryProgressV1::Completed { attempts });
            }
            let evidence = executor
                .pending_kura_apply_recovery_evidence()
                .ok_or(ProductionPendingKuraApplyRecoveryErrorV1::MissingEvidence)?;
            if evidence.stage()
                == crate::sumeragi::v2_effects::PendingKuraApplyRecoveryStage::Completed
            {
                if !executor.ready_to_finish() {
                    return Err(ProductionPendingKuraApplyRecoveryErrorV1::IncompleteFinality);
                }
                let attempts =
                    executor.settle_pending_tip_recovery_attempt(AttemptResult::Completed);
                return Ok(ProductionPendingKuraApplyRecoveryProgressV1::Completed { attempts });
            }
            let stage = evidence.stage();
            if completions == 0 && effects == 0 {
                let attempts =
                    executor.settle_pending_tip_recovery_attempt(AttemptResult::Waiting);
                Ok(ProductionPendingKuraApplyRecoveryProgressV1::Waiting { attempts, stage })
            } else {
                let attempts =
                    executor.settle_pending_tip_recovery_attempt(AttemptResult::Advanced);
                Ok(ProductionPendingKuraApplyRecoveryProgressV1::Advanced {
                    completions,
                    effects,
                    attempts,
                    stage,
                })
            }
        })
    }

    /// Construct and authenticate lane recovery after the local Apply completes.
    ///
    /// The consuming result owns the adapter only after it proves that it
    /// shares this exact context, State, Kura, output guard, output handoff,
    /// and Queue, and after its one-shot startup activation succeeds.
    #[allow(dead_code, clippy::type_complexity)]
    pub(in crate::sumeragi) fn prepare_lane_recovery<E>(
        mut self,
        runner: &mut crate::sumeragi::v2_runner::ProductionLifecyclePreActivationRunnerBorrowV1,
        queue: &Arc<Queue>,
        operation: impl FnOnce(
            crate::sumeragi::v2_recovery::PendingKuraApply,
            &mut V2EffectExecutor<SerializedV2Runtime>,
            &mut ProductionV2Services,
        ) -> Result<V2LaneWorkAdapter, E>,
    ) -> Result<PreparedPendingKuraLaneRecoveryV1, E>
    where
        E: From<ProductionLifecyclePreActivationErrorV1>
            + From<crate::sumeragi::v2_lane_work::V2LaneWorkError>,
    {
        let expected = self.installed.expected();
        let queue = Arc::clone(queue);
        let lane_work = self
            .launched
            .with_runner_setup(runner, |executor, services| {
                if !services.matches_installed_pending_kura_tip(expected) {
                    return Err(E::from(
                        ProductionLifecyclePreActivationErrorV1::OwnershipMismatch,
                    ));
                }
                let mut lane_work = operation(expected, executor, services)?;
                if !services.matches_lifecycle_lane_work(&lane_work) {
                    return Err(E::from(
                        ProductionLifecyclePreActivationErrorV1::OwnershipMismatch,
                    ));
                }
                lane_work.install_lane_drain_queue(Arc::clone(&queue))?;
                lane_work.activate_after_lane_drain_queue_install(&queue)?;
                Ok(lane_work)
            })?;
        // A replayed height-one projection is pre-Apply authority. The exact
        // applied State/Kura tip above supersedes it before live lane work.
        let _ = self.installed.take_genesis();
        let Self {
            installed,
            launched,
        } = self;
        Ok(PreparedPendingKuraLaneRecoveryV1 {
            installed,
            lane_work,
            launched,
        })
    }

    /// Consume an installed interrupted tip during operator shutdown.
    #[allow(dead_code, clippy::result_large_err)]
    pub(in crate::sumeragi) fn into_clean_shutdown(
        self,
        runner: crate::sumeragi::v2_runner::ProductionLifecyclePendingKuraRunnerActivationV1,
    ) -> Result<(), ProductionLifecycleShutdownErrorV1> {
        let Self {
            installed,
            launched,
        } = self;
        let output_guard = launched.services.lifecycle_output_guard();
        let operation = output_guard.begin_fail_stop_operation();
        let runner_retirement =
            runner.retire_unpublished(&launched.leader_wire_ingress_binding.ingress);
        drop(installed);
        launched.finish_clean_shutdown(operation, runner_retirement)
    }
}

impl PreparedPendingKuraLaneRecoveryV1 {
    /// Borrow the exact activated lane adapter with its still-closed lifecycle stack.
    #[allow(dead_code, clippy::type_complexity)]
    pub(in crate::sumeragi) fn with_runner_setup<R, E>(
        &mut self,
        runner: &mut crate::sumeragi::v2_runner::ProductionLifecyclePreActivationRunnerBorrowV1,
        operation: impl FnOnce(
            &mut V2LaneWorkAdapter,
            &mut V2EffectExecutor<SerializedV2Runtime>,
            &mut ProductionV2Services,
        ) -> Result<R, E>,
    ) -> Result<R, E>
    where
        E: From<ProductionLifecyclePreActivationErrorV1>,
    {
        let lane_work = &mut self.lane_work;
        self.launched
            .with_runner_setup(runner, |executor, services| {
                operation(lane_work, executor, services)
            })
    }

    /// Open the exact ingress for an already-applied tip without clocks/status.
    #[allow(dead_code, clippy::result_large_err)]
    pub(in crate::sumeragi) fn activate_no_clock(
        self,
        runner: crate::sumeragi::v2_runner::ProductionLifecyclePendingKuraRunnerActivationV1,
    ) -> Result<PendingKuraActivatedProductionLifecycleV1, ProductionLifecycleActivationErrorV1>
    {
        // Error unwinding must release lane work before the launched service
        // and durable ingress owners.
        let Self {
            mut launched,
            installed,
            lane_work,
        } = self;
        let pending_ready = launched.pending_kura_apply_replay.is_none()
            && launched.recovered_local_proposal_attempt.is_none()
            && launched.executor.lifecycle_live_clocks_are_unarmed()
            && launched.executor.ready_to_finish()
            && launched
                .services
                .matches_installed_pending_kura_tip(installed.expected())
            && launched
                .executor
                .pending_kura_apply_recovery_evidence()
                .is_some_and(|evidence| {
                    evidence.stage()
                        == crate::sumeragi::v2_effects::PendingKuraApplyRecoveryStage::Completed
                });
        if !pending_ready {
            launched
                .services
                .lifecycle_output_guard()
                .close_admission_for_restart();
            return Err(ProductionLifecycleActivationErrorV1::PendingKuraApplyNotReady);
        }
        let output_guard = launched.services.lifecycle_output_guard();
        let activation = output_guard
            .begin_fail_stop_operation()
            .ok_or(ProductionLifecycleActivationErrorV1::OutputClosed)?;
        let status = launched
            .executor
            .pending_kura_activation_status_snapshot()
            .map_err(ProductionLifecycleActivationErrorV1::Status)?;
        let observer = launched
            .completion_observer_activation
            .take()
            .ok_or_else(|| {
                ProductionLifecycleActivationErrorV1::CompletionObserver(
                    "launched pending Kura lifecycle lost its one-shot observer permit".to_owned(),
                )
            })?;
        launched
            .services
            .activate_effect_completion_observer(observer)
            .map_err(ProductionLifecycleActivationErrorV1::CompletionObserver)?;
        let runner_activation = runner
            .open_and_publish_recovered_height(
                &launched.leader_wire_ingress_binding.ingress,
                status,
            )
            .map_err(|error| ProductionLifecycleActivationErrorV1::Runner(error.to_string()))?;
        activation.complete();
        Ok(PendingKuraActivatedProductionLifecycleV1 {
            runner_activation,
            installed,
            lane_work,
            launched,
        })
    }

    /// Consume a prepared interrupted-tip height during operator shutdown.
    #[allow(dead_code, clippy::result_large_err)]
    pub(in crate::sumeragi) fn into_clean_shutdown(
        self,
        runner: crate::sumeragi::v2_runner::ProductionLifecyclePendingKuraRunnerActivationV1,
    ) -> Result<(), ProductionLifecycleShutdownErrorV1> {
        let Self {
            installed,
            lane_work,
            launched,
        } = self;
        let output_guard = launched.services.lifecycle_output_guard();
        let operation = output_guard.begin_fail_stop_operation();
        let runner_retirement =
            runner.retire_unpublished(&launched.leader_wire_ingress_binding.ingress);
        drop(lane_work);
        drop(installed);
        launched.finish_clean_shutdown(operation, runner_retirement)
    }
}

impl PendingKuraActivatedProductionLifecycleV1 {
    /// Borrow only the exact executor/service pair for decided-lane recovery.
    #[allow(dead_code, clippy::type_complexity)]
    pub(in crate::sumeragi) fn with_runner_runtime<R>(
        &mut self,
        _runner: &mut crate::sumeragi::v2_runner::ProductionLifecycleActiveRunnerBorrowV1,
        operation: impl FnOnce(
            &mut V2EffectExecutor<SerializedV2Runtime>,
            &mut ProductionV2Services,
            &mut V2LaneWorkAdapter,
        ) -> R,
    ) -> R {
        operation(
            &mut self.launched.executor,
            &mut self.launched.services,
            &mut self.lane_work,
        )
    }

    /// Settle at most one lifecycle-owned Certified-Serve completion without
    /// exposing or consuming any non-Serve completion head.
    pub(in crate::sumeragi) fn settle_certified_serve_completion_for_no_clock_recovery(
        &mut self,
        _runner: &mut crate::sumeragi::v2_runner::ProductionLifecycleActiveRunnerBorrowV1,
    ) -> Result<bool, String> {
        let completion = self
            .launched
            .services
            .drain_lifecycle_certified_serve_completion()?
            .into_completion();
        let Some(completion) = completion else {
            return Ok(false);
        };
        let _settlement = completion
            .settle_deliver_and_acknowledge(&mut self.launched.owner, &self.launched.services)
            .map_err(|_| "pending Kura Certified-Serve settlement requires restart".to_owned())?;
        Ok(true)
    }

    /// Claim the oldest lifecycle-owned ProducerTurn at this no-clock
    /// recovery loop's single bounded service point.
    pub(in crate::sumeragi) fn claim_producer_turn_for_no_clock_recovery(
        &mut self,
        _runner: &mut crate::sumeragi::v2_runner::ProductionLifecycleActiveRunnerBorrowV1,
    ) -> Result<Option<ClaimedProducerTurnV1>, ProducerTurnSchedulerClaimErrorV1> {
        let mode = self.launched.executor.lifecycle_mode_rank_snapshot();
        self.launched
            .owner
            .claim_producer_turn_at_bounded_producer_point(&mode)
    }

    /// Durably terminalize one no-clock ProducerTurn after the bounded
    /// recovery service pass completed successfully.
    pub(in crate::sumeragi) fn settle_producer_turn_after_no_clock_recovery(
        &mut self,
        _runner: &mut crate::sumeragi::v2_runner::ProductionLifecycleActiveRunnerBorrowV1,
        attempted: AttemptedProducerTurnV1,
    ) -> Result<(), ProducerTurnTerminalSettlementErrorV1> {
        self.launched.owner.settle_producer_turn_advanced(attempted)
    }

    /// Close new physical ingress while retaining the no-clock lifecycle for
    /// a finite terminal-recovery drain before finalized rollover.
    pub(in crate::sumeragi) fn close_runner_ingress_for_finalized_drain(
        &self,
        _runner: &mut crate::sumeragi::v2_runner::ProductionLifecycleActiveRunnerBorrowV1,
        receiver: &Arc<FairV2Ingress>,
    ) -> Result<(), crate::sumeragi::v2_runner::V2RunnerError> {
        self.runner_activation.close_ingress(receiver)?;
        if !Arc::ptr_eq(receiver, &self.launched.leader_wire_ingress_binding.ingress) {
            return Err(
                crate::sumeragi::v2_runner::V2RunnerError::LifecycleActivationIngressMismatch,
            );
        }
        Ok(())
    }

    /// Consume a live interrupted-tip height during orderly operator shutdown.
    #[allow(dead_code, clippy::result_large_err)]
    pub(in crate::sumeragi) fn into_clean_shutdown(
        self,
        _runner: &mut crate::sumeragi::v2_runner::ProductionLifecycleActiveRunnerBorrowV1,
    ) -> Result<(), ProductionLifecycleShutdownErrorV1> {
        let Self {
            launched,
            installed,
            lane_work,
            runner_activation,
        } = self;
        let output_guard = launched.services.lifecycle_output_guard();
        let operation = output_guard.begin_fail_stop_operation();
        let runner_retirement =
            runner_activation.retire(&launched.leader_wire_ingress_binding.ingress);
        drop(lane_work);
        drop(installed);
        launched.finish_clean_shutdown(operation, runner_retirement)
    }

    /// Consume the no-clock height into the shared finalized-output rollover.
    #[allow(dead_code, clippy::result_large_err)]
    pub(in crate::sumeragi) fn into_finalized_rollover(
        mut self,
        _runner: &mut crate::sumeragi::v2_runner::ProductionLifecycleActiveRunnerBorrowV1,
    ) -> Result<
        (FinalizedProductionLifecycleRolloverV1, V2LaneWorkAdapter),
        ProductionLifecycleFinalizationErrorV1,
    > {
        if !self.launched.executor.ready_to_finish()
            || self.launched.pending_kura_apply_replay.is_some()
            || self.launched.recovered_local_proposal_attempt.is_some()
            || self
                .launched
                .executor
                .pending_kura_apply_recovery_evidence()
                .is_none_or(|evidence| {
                    evidence.stage()
                        != crate::sumeragi::v2_effects::PendingKuraApplyRecoveryStage::Completed
                })
            || self.launched.pending_lifecycle_completion.is_some()
            || self.launched.pending_ingress_capacity.is_some()
            || self.launched.completion_observer_activation.is_some()
            || !self
                .launched
                .services
                .matches_installed_pending_kura_tip(self.installed.expected())
            || !self
                .launched
                .services
                .matches_lifecycle_lane_work(&self.lane_work)
            || !self
                .launched
                .owner
                .registry
                .registry_mut()
                .exactly_covers_finalization_work(&self.launched.owner.coordinator)
        {
            return Err(ProductionLifecycleFinalizationErrorV1::NotReady);
        }

        let Self {
            mut launched,
            installed,
            lane_work,
            runner_activation,
        } = self;
        runner_activation
            .retire(&launched.leader_wire_ingress_binding.ingress)
            .map_err(|error| ProductionLifecycleFinalizationErrorV1::Runner(error.to_string()))?;
        drop(installed);
        launched
            .leader_wire_ingress_binding
            .retire()
            .map_err(ProductionLifecycleFinalizationErrorV1::Ingress)?;
        let retired_ingress = ProductionLifecycleRetiredIngressPermitV1 {
            _seal: ProductionLifecycleRetiredIngressPermitSealV1,
        };

        let LaunchedProductionLifecycleV1 {
            owner,
            executor,
            services,
            pending_kura_apply_replay,
            recovered_local_proposal_attempt,
            pending_lifecycle_completion,
            pending_ingress_capacity,
            completion_observer_activation,
            leader_wire_ingress_binding,
        } = launched;
        debug_assert!(pending_kura_apply_replay.is_none());
        debug_assert!(recovered_local_proposal_attempt.is_none());
        debug_assert!(pending_lifecycle_completion.is_none());
        debug_assert!(pending_ingress_capacity.is_none());
        debug_assert!(completion_observer_activation.is_none());
        drop(pending_kura_apply_replay);
        drop(recovered_local_proposal_attempt);
        drop(pending_lifecycle_completion);
        drop(pending_ingress_capacity);
        drop(completion_observer_activation);
        drop(leader_wire_ingress_binding);

        let (runtime, receipt, artifact) = executor
            .into_finalized_parts()
            .map_err(ProductionLifecycleFinalizationErrorV1::Executor)?;
        let output_guard = services.lifecycle_output_guard();
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or(ProductionLifecycleFinalizationErrorV1::OutputClosed)?;
        let finalized = runtime
            .into_driver()
            .finish_height(&receipt, &artifact)
            .map_err(ProductionLifecycleFinalizationErrorV1::Adapter)?;
        operation.complete();

        Ok((
            FinalizedProductionLifecycleRolloverV1 {
                owner,
                services,
                receipt,
                artifact,
                finalized_adapter: finalized,
                retired_ingress,
            },
            lane_work,
        ))
    }
}
