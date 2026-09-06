impl<R: EffectRuntime> V2EffectExecutor<R> {
    /// Consume startup or reducer effects in their exact emitted order for
    /// fixture drivers which have no runner-owned Decision cleanup.
    #[cfg(test)]
    pub(crate) fn consume_effects<S: V2EffectServices>(
        &mut self,
        effects: Vec<AdapterEffect>,
        services: &mut S,
    ) -> Result<usize, EffectExecutorError> {
        self.consume_effects_with_runner_decision_cleanup(effects, services, None, None)
    }

    #[cfg(test)]
    fn consume_pacemaker_effects<S: V2EffectServices>(
        &mut self,
        effects: Vec<AdapterEffect>,
        services: &mut S,
    ) -> Result<usize, EffectExecutorError> {
        self.consume_pacemaker_effects_with_runner_decision_cleanup(effects, services, None)
    }
}
