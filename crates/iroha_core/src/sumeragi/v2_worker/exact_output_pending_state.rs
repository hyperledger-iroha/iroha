impl ProductionV2Services {
    /// Return whether the bounded corridor has dispatchable fanout work, a
    /// writer-flush witness, or a sidecar receipt awaiting lane delivery.
    pub(crate) fn has_pending_exact_output(&self) -> Result<bool, String> {
        self.lock_pending_exact_output().map(|pending| {
            if self.exact_output_handoff_owner.is_sealed() {
                debug_assert!(!pending.is_pending());
                false
            } else {
                pending.is_pending()
            }
        })
    }
}
