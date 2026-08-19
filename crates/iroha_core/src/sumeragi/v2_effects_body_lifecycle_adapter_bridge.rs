impl V2EffectExecutor<SerializedV2Runtime> {
    /// Preview an ordinary certified-Fetch Store successor on the serialized adapter.
    pub(in crate::sumeragi) fn prepare_certified_fetch_store_adapter(
        &mut self,
        tag: EventTag,
        manifest: &wire::PayloadManifest,
    ) -> Result<super::v2::PreparedCertifiedFetchStoreAdapterV1<'_>, super::v2::AdapterError> {
        self.runtime.prepare_certified_fetch_store(tag, manifest)
    }
    /// Preview an ordinary durable-Store Validate successor on the serialized adapter.
    pub(in crate::sumeragi) fn prepare_durable_store_validate_adapter(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        receipt: &DurableBodyReceipt,
    ) -> Result<super::v2::PreparedDurableStoreValidateAdapterV1<'_>, super::v2::AdapterError> {
        self.runtime
            .prepare_durable_store_validate(tag, round, subject, receipt)
    }
    /// Preview one exact lifecycle-owned Ready Validate on the serialized adapter.
    pub(in crate::sumeragi) fn prepare_ready_durable_validate_adapter<'registry>(
        &mut self,
        prepared: super::v2_lifecycle_coordinator::PreparedReadyDurableValidateExecution<'registry>,
    ) -> Result<
        super::v2_lifecycle_coordinator::PreparedReadyDurableValidateAdapterPreview<'registry, '_>,
        super::v2_lifecycle_coordinator::PreparedReadyDurableValidateExecution<'registry>,
    > {
        self.runtime
            .prepare_ready_durable_validate_adapter(prepared)
    }
    /// Return the exact serialized adapter generation for lifecycle fence waits.
    pub(in crate::sumeragi) const fn lifecycle_reducer_fence_generation(&self) -> u64 {
        self.runtime.lifecycle_reducer_fence_generation()
    }
}
