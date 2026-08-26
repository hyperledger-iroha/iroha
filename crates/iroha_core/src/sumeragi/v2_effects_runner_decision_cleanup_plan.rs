impl<R: EffectRuntime> V2EffectExecutor<R> {
    fn plan_runner_decision_cleanup(
        &self,
        before: Option<DurableDecision>,
        after: Option<DurableDecision>,
    ) -> Result<Option<PendingRunnerDecisionCleanup>, EffectExecutorError> {
        let Some(decision) = after else {
            return Ok(None);
        };
        if before.is_some_and(|existing| existing != decision) {
            return Err(EffectExecutorError::Contract(
                "one runtime step changed an already durable Decision".to_owned(),
            ));
        }
        // A live completion can install Decision immediately before the scheduler turn which
        // emits its Apply, so both observations are already `Some` even though the runner has
        // not retired its process-local proposal owners. Executor Decision protection is still
        // absent at that exact cut. Cold recovery deliberately stays exempt: startup has no
        // process-local owner and live clocks remain unarmed until recovery has drained.
        let installed_by_step = before.is_none();
        let live_unreconciled_decision =
            self.runtime.lifecycle_live_clocks_are_armed() && self.protected_decision.is_none();
        if !installed_by_step && !live_unreconciled_decision {
            return Ok(None);
        }
        let frontier = self
            .runtime
            .reconciliation_frontier()
            .map_err(EffectExecutorError::Runtime)?;
        if frontier.decision != Some(decision) {
            return Err(EffectExecutorError::Contract(
                "new Decision changed across its post-step reconciliation frontier".to_owned(),
            ));
        }
        let owner_tag = frontier.tag.ok_or_else(|| {
            EffectExecutorError::Contract(
                "new Decision omitted its exact local runner owner".to_owned(),
            )
        })?;
        if owner_tag.height() != decision.0.height {
            return Err(EffectExecutorError::Contract(
                "new Decision changed height across its local runner owner".to_owned(),
            ));
        }
        Ok(Some(PendingRunnerDecisionCleanup {
            decision,
            owner_tag,
        }))
    }
}
