//! Closed-ingress setup and interrupted-tip installation before activation.

use std::sync::Arc;

use thiserror::Error;

use super::LaunchedProductionLifecycleV1;
use crate::sumeragi::output_guard::ConsensusOutputGuard;

/// Fail-stop rejection of runner setup outside the sealed preactivation state.
#[derive(Debug, Error)]
#[allow(variant_size_differences)]
#[must_use = "failed lifecycle preactivation setup requires process restart"]
pub(in crate::sumeragi) enum ProductionLifecyclePreActivationErrorV1 {
    /// The canonical output corridor was already fail-stop closed.
    #[error("canonical consensus output is closed")]
    OutputClosed,
    /// Executor and service process identities no longer describe one stack.
    #[error("launched lifecycle lost its exact executor/service ownership")]
    OwnershipMismatch,
    /// The exact launched ingress was opened before the activation transition.
    #[error("launched lifecycle ingress opened before runner setup completed")]
    IngressAlreadyOpen,
    /// The one-shot completion observer was consumed before activation.
    #[error("launched lifecycle lost its preactivation completion-observer authority")]
    CompletionObserverMissing,
    /// Runner setup armed clocks reserved for the consuming activation boundary.
    #[error("launched lifecycle armed live clocks during preactivation setup")]
    ClocksAlreadyArmed,
    /// The future activation could not lend the exact launched ingress.
    #[error("launched lifecycle could not open its preactivation recovery ingress: {0}")]
    RecoveryIngress(String),
    /// The reducer could not expose its exact current local-Proposal directive.
    #[error("launched lifecycle could not project its local Proposal directive: {0}")]
    LocalProposalDirective(#[source] crate::sumeragi::v2_effects::EffectExecutorError),
    /// The recovered Proposal owner no longer matches the reducer's current lock/view.
    #[error("recovered local Proposal ownership changed before runner initialization")]
    RecoveredProposalMismatch,
    /// The runner-local scheduling state already owns unrelated live work.
    #[error("runner local Proposal state was not pristine during recovered initialization")]
    RunnerProposalStateNotPristine,
}

/// Fail-stop error while installing the exact interrupted-tip replay.
#[derive(Debug, Error)]
#[must_use = "failed pending Kura installation requires process restart"]
pub(in crate::sumeragi) enum ProductionPendingKuraApplyInstallErrorV1 {
    /// The launched owner did not retain one pending-tip replay seal.
    #[error("launched lifecycle has no pending Kura replay authority")]
    MissingReplay,
    /// The stack was no longer in the exact closed-ingress setup state.
    #[error(transparent)]
    Setup(#[from] ProductionLifecyclePreActivationErrorV1),
    /// Exact Decision/body/WAL recovery verification or local dispatch failed.
    #[error(transparent)]
    Effect(#[from] crate::sumeragi::v2_effects::EffectExecutorError),
}

/// Panic/error guard for setup which may itself enter fail-stop service code.
///
/// Unlike [`crate::sumeragi::output_guard::ConsensusFailStopOperation`], this
/// scope holds no read permit across the callback. Nested executor/service
/// failures may therefore activate restart synchronously without self-deadlock.
#[must_use = "preactivation setup must disarm its fail-stop scope on success"]
pub(super) struct ProductionLifecyclePreActivationFailStopScopeV1 {
    output_guard: Arc<ConsensusOutputGuard>,
    armed: bool,
}

trait ProductionLifecycleCanonicalRecoveryActivationV1 {
    fn open_canonical_recovery_ingress<'activation>(
        &'activation mut self,
        launched_ingress: &Arc<crate::sumeragi::FairV2Ingress>,
    ) -> Result<
        crate::sumeragi::v2_runner::ProductionLifecycleCanonicalRecoveryIngressV1<'activation>,
        crate::sumeragi::v2_runner::V2RunnerError,
    >;
}

impl ProductionLifecycleCanonicalRecoveryActivationV1
    for crate::sumeragi::v2_runner::ProductionLifecycleRunnerActivationV1
{
    fn open_canonical_recovery_ingress<'activation>(
        &'activation mut self,
        launched_ingress: &Arc<crate::sumeragi::FairV2Ingress>,
    ) -> Result<
        crate::sumeragi::v2_runner::ProductionLifecycleCanonicalRecoveryIngressV1<'activation>,
        crate::sumeragi::v2_runner::V2RunnerError,
    > {
        crate::sumeragi::v2_runner::ProductionLifecycleRunnerActivationV1::open_canonical_recovery_ingress(
            self,
            launched_ingress,
        )
    }
}

impl ProductionLifecycleCanonicalRecoveryActivationV1
    for crate::sumeragi::v2_runner::ProductionLifecycleCompleteTipRunnerActivationV1
{
    fn open_canonical_recovery_ingress<'activation>(
        &'activation mut self,
        launched_ingress: &Arc<crate::sumeragi::FairV2Ingress>,
    ) -> Result<
        crate::sumeragi::v2_runner::ProductionLifecycleCanonicalRecoveryIngressV1<'activation>,
        crate::sumeragi::v2_runner::V2RunnerError,
    > {
        crate::sumeragi::v2_runner::ProductionLifecycleCompleteTipRunnerActivationV1::open_canonical_recovery_ingress(
            self,
            launched_ingress,
        )
    }
}

impl ProductionLifecyclePreActivationFailStopScopeV1 {
    pub(super) fn new(output_guard: Arc<ConsensusOutputGuard>) -> Self {
        Self {
            output_guard,
            armed: true,
        }
    }

    pub(super) fn complete(mut self) {
        self.armed = false;
    }
}

impl Drop for ProductionLifecyclePreActivationFailStopScopeV1 {
    fn drop(&mut self) {
        if self.armed {
            self.output_guard.close_admission_for_restart();
        }
    }
}

fn missing_pending_kura_replay(
    output_guard: &ConsensusOutputGuard,
) -> ProductionPendingKuraApplyInstallErrorV1 {
    output_guard.close_admission_for_restart();
    ProductionPendingKuraApplyInstallErrorV1::MissingReplay
}

impl LaunchedProductionLifecycleV1 {
    fn with_runner_setup_transaction<R, E>(
        &mut self,
        operation: impl FnOnce(
            &mut crate::sumeragi::v2_effects::V2EffectExecutor<
                crate::sumeragi::v2_runtime::SerializedV2Runtime,
            >,
            &mut crate::sumeragi::v2_worker::ProductionV2Services,
        ) -> Result<R, E>,
    ) -> Result<R, E>
    where
        E: From<ProductionLifecyclePreActivationErrorV1>,
    {
        let output_guard = self.services.lifecycle_output_guard();
        let initial_admission = output_guard
            .acquire()
            .ok_or_else(|| E::from(ProductionLifecyclePreActivationErrorV1::OutputClosed))?;
        let setup = ProductionLifecyclePreActivationFailStopScopeV1::new(Arc::clone(&output_guard));
        drop(initial_admission);
        let preflight_failure = if !self
            .services
            .matches_lifecycle_executor_output_guard(&self.executor)
        {
            Some(ProductionLifecyclePreActivationErrorV1::OwnershipMismatch)
        } else if self.leader_wire_ingress_binding.ingress.state.lock().open {
            Some(ProductionLifecyclePreActivationErrorV1::IngressAlreadyOpen)
        } else if self.completion_observer_activation.is_none() {
            Some(ProductionLifecyclePreActivationErrorV1::CompletionObserverMissing)
        } else if !self.executor.lifecycle_live_clocks_are_unarmed() {
            Some(ProductionLifecyclePreActivationErrorV1::ClocksAlreadyArmed)
        } else {
            None
        };
        if let Some(error) = preflight_failure {
            return Err(E::from(error));
        }
        let value = operation(&mut self.executor, &mut self.services)?;
        let postflight_failure = if !self
            .services
            .matches_lifecycle_executor_output_guard(&self.executor)
        {
            Some(ProductionLifecyclePreActivationErrorV1::OwnershipMismatch)
        } else if self.leader_wire_ingress_binding.ingress.state.lock().open {
            Some(ProductionLifecyclePreActivationErrorV1::IngressAlreadyOpen)
        } else if self.completion_observer_activation.is_none() {
            Some(ProductionLifecyclePreActivationErrorV1::CompletionObserverMissing)
        } else if !self.executor.lifecycle_live_clocks_are_unarmed() {
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

    fn with_canonical_body_recovery_ingress_transaction<R, E, Activation>(
        &mut self,
        _runner: &mut crate::sumeragi::v2_runner::ProductionLifecyclePreActivationRunnerBorrowV1,
        activation: &mut Activation,
        operation: impl FnOnce(
            &crate::sumeragi::v2_runner::ProductionLifecycleCanonicalRecoveryIngressV1<'_>,
            &mut crate::sumeragi::v2_effects::V2EffectExecutor<
                crate::sumeragi::v2_runtime::SerializedV2Runtime,
            >,
            &mut crate::sumeragi::v2_worker::ProductionV2Services,
        ) -> Result<R, E>,
    ) -> Result<R, E>
    where
        Activation: ProductionLifecycleCanonicalRecoveryActivationV1,
        E: From<ProductionLifecyclePreActivationErrorV1>,
    {
        let launched_ingress = Arc::clone(&self.leader_wire_ingress_binding.ingress);
        self.with_runner_setup_transaction(move |executor, services| {
            let aperture = activation
                .open_canonical_recovery_ingress(&launched_ingress)
                .map_err(|error| {
                    E::from(ProductionLifecyclePreActivationErrorV1::RecoveryIngress(
                        error.to_string(),
                    ))
                })?;
            let result = operation(&aperture, executor, services);
            if !aperture.close_and_verify() {
                return Err(E::from(
                    ProductionLifecyclePreActivationErrorV1::RecoveryIngress(
                        "temporary recovery ingress did not close exactly".to_owned(),
                    ),
                ));
            }
            result
        })
    }

    /// Borrow executor and services for closed-ingress runner setup.
    ///
    /// The lifecycle owner, adapter, body store, and ingress gates remain
    /// inside this opaque stack. A stale/open ingress or consumed observer
    /// closes canonical output and rejects the setup before the callback runs.
    #[allow(dead_code, clippy::type_complexity)]
    pub(in crate::sumeragi) fn with_runner_setup<R, E>(
        &mut self,
        _runner: &mut crate::sumeragi::v2_runner::ProductionLifecyclePreActivationRunnerBorrowV1,
        operation: impl FnOnce(
            &mut crate::sumeragi::v2_effects::V2EffectExecutor<
                crate::sumeragi::v2_runtime::SerializedV2Runtime,
            >,
            &mut crate::sumeragi::v2_worker::ProductionV2Services,
        ) -> Result<R, E>,
    ) -> Result<R, E>
    where
        E: From<ProductionLifecyclePreActivationErrorV1>,
    {
        self.with_runner_setup_transaction(operation)
    }

    /// Temporarily open the future activation's exact ingress for body recovery.
    ///
    /// This preserves the legacy admission boundary: normal fair ingress is
    /// temporarily open, while the runner callback must use only the canonical
    /// executed-body recovery predicate. The aperture closes before setup
    /// postflight, status remains unpublished, and live clocks stay unarmed.
    // TODO: Replace both legacy canonical-body recovery openings in
    // `v2_runner::run_inner` with this typed aperture at the atomic lifecycle
    // owner cutover.
    #[allow(dead_code, clippy::type_complexity)]
    pub(in crate::sumeragi) fn with_canonical_body_recovery_ingress<R, E>(
        &mut self,
        runner: &mut crate::sumeragi::v2_runner::ProductionLifecyclePreActivationRunnerBorrowV1,
        activation: &mut crate::sumeragi::v2_runner::ProductionLifecycleRunnerActivationV1,
        operation: impl FnOnce(
            &crate::sumeragi::v2_runner::ProductionLifecycleCanonicalRecoveryIngressV1<'_>,
            &mut crate::sumeragi::v2_effects::V2EffectExecutor<
                crate::sumeragi::v2_runtime::SerializedV2Runtime,
            >,
            &mut crate::sumeragi::v2_worker::ProductionV2Services,
        ) -> Result<R, E>,
    ) -> Result<R, E>
    where
        E: From<ProductionLifecyclePreActivationErrorV1>,
    {
        self.with_canonical_body_recovery_ingress_transaction(runner, activation, operation)
    }

    /// Lend the same recovery aperture to a still-sealed CompleteTip successor.
    #[allow(dead_code, clippy::type_complexity)]
    pub(in crate::sumeragi) fn with_complete_tip_canonical_body_recovery_ingress<R, E>(
        &mut self,
        runner: &mut crate::sumeragi::v2_runner::ProductionLifecyclePreActivationRunnerBorrowV1,
        activation: &mut crate::sumeragi::v2_runner::ProductionLifecycleCompleteTipRunnerActivationV1,
        operation: impl FnOnce(
            &crate::sumeragi::v2_runner::ProductionLifecycleCanonicalRecoveryIngressV1<'_>,
            &mut crate::sumeragi::v2_effects::V2EffectExecutor<
                crate::sumeragi::v2_runtime::SerializedV2Runtime,
            >,
            &mut crate::sumeragi::v2_worker::ProductionV2Services,
        ) -> Result<R, E>,
    ) -> Result<R, E>
    where
        E: From<ProductionLifecyclePreActivationErrorV1>,
    {
        self.with_canonical_body_recovery_ingress_transaction(runner, activation, operation)
    }

    /// Join one recovered local Proposal attempt to runner-local scheduling state.
    ///
    /// The WAL-authenticated attempt never exposes its tag, round, subject, or
    /// original Sign effect. This one-shot comparison takes the opaque owner,
    /// snapshots the reducer's exact directive under the closed-ingress setup
    /// transaction, and mutates the affine modular-runner proposal state before
    /// clearing the activation blocker. A mismatch or non-pristine runner state
    /// consumes the stale owner and fail-stop closes canonical output through
    /// [`Self::with_runner_setup`]. The production runner will mint and retain
    /// this state only at its later atomic lifecycle cutover.
    #[allow(dead_code, clippy::result_large_err)]
    pub(in crate::sumeragi) fn initialize_recovered_local_proposal(
        &mut self,
        mut runner: crate::sumeragi::v2_runner::ProductionLifecyclePreActivationRunnerBorrowV1,
    ) -> Result<
        (
            crate::sumeragi::v2::LocalProposalDirective,
            super::ProductionLifecyclePreparedLocalProposalStateV1,
        ),
        ProductionLifecyclePreActivationErrorV1,
    > {
        let recovered = self.recovered_local_proposal_attempt.take();
        let (context_id, directive, runner) =
            self.with_runner_setup_transaction(move |executor, _services| {
                let directive = executor
                    .local_proposal_directive()
                    .map_err(ProductionLifecyclePreActivationErrorV1::LocalProposalDirective)?;
                match recovered {
                    Some(recovered) if recovered.exactly_matches_directive(directive) => {
                        if !runner.bind_recovered_local_proposal(directive) {
                            return Err(
                            ProductionLifecyclePreActivationErrorV1::RunnerProposalStateNotPristine,
                        );
                        }
                    }
                    Some(_) => {
                        return Err(
                            ProductionLifecyclePreActivationErrorV1::RecoveredProposalMismatch,
                        );
                    }
                    None if !runner.local_proposal_state_is_pristine() => {
                        return Err(
                            ProductionLifecyclePreActivationErrorV1::RunnerProposalStateNotPristine,
                        );
                    }
                    None => {}
                }
                Ok((executor.context().id(), directive, runner))
            })?;
        let prepared = super::ProductionLifecyclePreparedLocalProposalStateV1 {
            runner,
            context_id,
            directive,
        };
        Ok((directive, prepared))
    }

    /// Install one opaque recovered-attempt fixture without exposing its parts.
    #[cfg(test)]
    pub(in crate::sumeragi) fn retain_recovered_local_proposal_attempt_for_test(
        &mut self,
        recovered: crate::sumeragi::v2::RecoveredLifecycleLocalProposalAttemptV1,
    ) {
        assert!(self.recovered_local_proposal_attempt.is_none());
        self.recovered_local_proposal_attempt = Some(recovered);
    }

    /// Install the sole pending-Kura Decision Fetch while ingress and clocks stay closed.
    ///
    /// The replay seal is taken exactly once and consumed inside the existing
    /// non-permit fail-stop setup scope. Verification reconstructs the exact
    /// pending-tip evidence before the effect may enter the local-only recovery
    /// pipeline; neither the Fetch nor its WAL evidence crosses this boundary.
    #[allow(dead_code, clippy::result_large_err)]
    pub(in crate::sumeragi) fn install_pending_kura_apply(
        &mut self,
        runner: &mut crate::sumeragi::v2_runner::ProductionLifecyclePreActivationRunnerBorrowV1,
    ) -> Result<
        Option<crate::sumeragi::v2_effects::VerifiedPendingGenesisNexusAmxContext>,
        ProductionPendingKuraApplyInstallErrorV1,
    > {
        let Some(replay) = self.pending_kura_apply_replay.take() else {
            let output_guard = self.services.lifecycle_output_guard();
            return Err(missing_pending_kura_replay(output_guard.as_ref()));
        };
        self.with_runner_setup(runner, move |executor, services| {
            replay
                .install(executor, services)
                .map_err(ProductionPendingKuraApplyInstallErrorV1::Effect)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_pending_kura_replay_closes_canonical_output() {
        let output_guard = ConsensusOutputGuard::isolated();
        assert!(matches!(
            missing_pending_kura_replay(output_guard.as_ref()),
            ProductionPendingKuraApplyInstallErrorV1::MissingReplay
        ));
        assert!(output_guard.restart_required());
    }
}
