/// Borrow of the actual publication retained by the Native process after all
/// original published instances have completed their local Apply settlement.
/// Global finality bytes alone cannot construct this capability.
pub(in crate::sumeragi) struct NativeFinalizedOutputAuthority<'published> {
    published: &'published super::v2_apply::PublishedNativeCarrier,
}

impl NativeFinalizedOutputAuthority<'_> {
    /// Authenticate the exact State and durable artifact before touching output.
    pub(in crate::sumeragi) fn authenticate(
        &self,
        state: &State,
        receipt: &KuraV2CommitReceipt,
        artifact: &wire::finality::V2FinalityArtifact,
    ) -> Result<(), String> {
        let original = self.published.receipt();
        if !self.published.matches_state(state)
            || self.published.artifact() != artifact
            || original.height() != receipt.height()
            || original.block_hash() != receipt.block_hash()
            || original.context_id() != receipt.context_id()
            || original.subject() != receipt.subject()
            || original.certificate() != receipt.certificate()
            || original.artifact_hash() != receipt.artifact_hash()
        {
            return Err(
                "Native output handoff differs from its original published State or finality"
                    .into(),
            );
        }
        Ok(())
    }
}

/// Keep the height alive until the real published carrier has crossed every
/// original Native Apply completion. Successor instances may remain active.
pub(super) fn preflight_finalized_native_rollover(
    executor: &V2EffectExecutor<SerializedV2Runtime>,
    services: &mut ProductionV2Services,
    native: &mut NativeRunnerProcess,
) -> Result<bool, V2RunnerError> {
    if !executor.ready_to_finish() {
        return Ok(false);
    }
    let (receipt, artifact) = executor.durable_finality().ok_or_else(|| {
        V2RunnerError::Service("ready executor lost its original durable finality".into())
    })?;
    native.preflight_publication(services, receipt, artifact)
}

/// Seal the global output corridor under the original publication. Native lane
/// transport remains owned by the process across this global height boundary.
pub(in crate::sumeragi) fn rollover_finalized_height_outputs_for_lifecycle(
    _permit: super::v2_lifecycle_coordinator::ProductionLifecycleOutputRolloverPermitV1,
    native: &mut NativeRunnerProcess,
    services: &ProductionV2Services,
    receipt: &KuraV2CommitReceipt,
    artifact: &wire::finality::V2FinalityArtifact,
    successor: &wire::HeightContext,
    _control_queue_capacity: usize,
) -> Result<(), String> {
    if artifact.height.checked_add(1) != Some(successor.height)
        || artifact.height_context.network_id != successor.network_id
        || successor.parent_commit_qc.as_ref() != Some(&artifact.commit_qc)
    {
        return Err("Native output handoff does not name the immediate certified successor".into());
    }
    let authority = native
        .finalized_output_authority(receipt, artifact)
        .map_err(|error| error.to_string())?;
    // Every exact global output is independently reconstructed or explicitly
    // superseded by this finality. An unsupported/manual/foreign output refuses
    // before the pending corridor changes; it is never silently discarded.
    services
        .handoff_native_height_output_to_durable_reconstruction(receipt, artifact, &authority)?;
    let handoff = services.seal_native_height_output_handoff(receipt, artifact, &authority)?;
    if !handoff.matches_finality_artifact(artifact)
        || !handoff.authorizes_immediate_successor(successor)
    {
        return Err("sealed Native output handoff lost its exact successor binding".into());
    }
    native
        .complete_output_handoff(receipt, artifact)
        .map_err(|error| error.to_string())
}
