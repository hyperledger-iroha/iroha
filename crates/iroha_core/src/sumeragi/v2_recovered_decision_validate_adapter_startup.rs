impl ProductionLifecycleAdapterStartupV1 {
/// Rebuild the reducer's already-fsynced recovered Store-to-Validate crash cut.
///
/// The returned projection keeps the recovered WAL locator, CommitQC,
/// BodyFrame, Store predecessor, Validate pending owner, and candidate
/// sealed together for the lifecycle registry startup transaction.
pub(in crate::sumeragi) fn advance_recovered_decision_store_validate(
    self,
    verified: &VerifiedHeightContext,
    fetch: &AuthenticatedRecoveredWalDecisionFetchProjection,
    store: &RecoveredDecisionFetchStoreProjectionV1,
) -> Result<(Self, RecoveredDecisionValidateProjectionV1), &'static str> {
    let ProductionLifecycleAdapterStartupStateV1::Recovered {
        mut adapter,
        effects,
        pending_kura_apply: None,
        local_proposal_attempt: None,
        leader_wire_launch_prepared: false,
    } = self.state
    else {
        return Err("recovered Decision Validate adapter startup is not pristine");
    };
    if !effects.is_empty()
        || &adapter.wire_context != verified.context()
        || adapter.proofs_of_possession.as_slice() != verified.proofs_of_possession()
        || adapter.current_tag().height() != verified.context().height
    {
        return Err("recovered Decision Validate adapter startup changed its exact context");
    }
    let (tag, round, subject) = store
        .adapter_preview_inputs()
        .ok_or("recovered Decision Validate Store effect is inconsistent")?;
    let preview = match adapter
        .prepare_durable_store_validate(tag, round, subject, store.durable_body_receipt())
        .map_err(|_| "recovered Decision Validate reducer preview is inconsistent")?
    {
        DurableStoreValidateAdapterPreparationV1::Applied(preview) => preview,
        DurableStoreValidateAdapterPreparationV1::Blocked(_) => {
            return Err("recovered Decision Validate reducer preview is blocked");
        }
        DurableStoreValidateAdapterPreparationV1::Inactive => {
            return Err("recovered Decision Validate reducer preview is inactive");
        }
    };
    let projection = fetch
        .project_decision_store_validate_successor(verified, store, preview.validate_effect())
        .ok_or("recovered Decision Validate projection is inconsistent")?;
    preview.commit_after_durable_publication();
    Ok((Self::recovered(adapter, effects), projection))
}
}
