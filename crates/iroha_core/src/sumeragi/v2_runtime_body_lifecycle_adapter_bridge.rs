impl SerializedV2Runtime<SumeragiV2Adapter> {
    /// Freeze the serialized shell around one ordinary certified-Fetch Store preview.
    pub(in crate::sumeragi) fn prepare_certified_fetch_store(
        &mut self,
        tag: EventTag,
        manifest: &wire::PayloadManifest,
    ) -> Result<super::v2::PreparedCertifiedFetchStoreAdapterV1<'_>, AdapterError> {
        if self.fail_closed
            || self.ingress.len() != 0
            || self.pending_effect_ownership.is_some()
            || self.last_scheduler_ownership.is_some()
            || !self.pending_leader_wire_terminals.is_empty()
        {
            return Err(AdapterError::DirectCertifiedBodyAvailableContractViolation);
        }
        self.driver.prepare_certified_fetch_store(tag, manifest)
    }
    /// Freeze the serialized shell around one ordinary Store Validate preview.
    pub(in crate::sumeragi) fn prepare_durable_store_validate(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        receipt: &DurableBodyReceipt,
    ) -> Result<super::v2::PreparedDurableStoreValidateAdapterV1<'_>, AdapterError> {
        if self.fail_closed
            || self.ingress.len() != 0
            || self.pending_effect_ownership.is_some()
            || self.last_scheduler_ownership.is_some()
            || !self.pending_leader_wire_terminals.is_empty()
        {
            return Err(AdapterError::DirectBodyStoredContractViolation);
        }
        self.driver
            .prepare_durable_store_validate(tag, round, subject, receipt)
    }
    /// Freeze the serialized shell around one registry-owned Ready Validate.
    pub(in crate::sumeragi) fn prepare_ready_durable_validate_adapter<'registry>(
        &mut self,
        prepared: super::v2_lifecycle_coordinator::PreparedReadyDurableValidateExecution<'registry>,
    ) -> Result<
        super::v2_lifecycle_coordinator::PreparedReadyDurableValidateAdapterPreview<'registry, '_>,
        super::v2_lifecycle_coordinator::PreparedReadyDurableValidateExecution<'registry>,
    > {
        if self.fail_closed
            || self.ingress.len() != 0
            || self.pending_effect_ownership.is_some()
            || self.last_scheduler_ownership.is_some()
            || !self.pending_leader_wire_terminals.is_empty()
        {
            return Err(prepared);
        }
        prepared
            .prepare_adapter_preview(&mut self.driver)
            .map_err(|error| error.into_registry())
    }
    /// Return the adapter generation which wakes lifecycle reducer-fence waits.
    pub(in crate::sumeragi) const fn lifecycle_reducer_fence_generation(&self) -> u64 {
        self.driver.reducer_fence_generation()
    }
}
