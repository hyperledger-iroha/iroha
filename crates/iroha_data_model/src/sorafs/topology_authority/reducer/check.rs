//! Challenge-bearing pure predicates; genuine native execution/finality remains mandatory outside.
use super::*;

pub(super) fn evaluate<L: TopologyIndexedReadV1 + ?Sized>(
    model: &TopologyStateViewV1<'_, L>,
    request: &TopologyCheckV1,
    context: &TopologyContextClaimV1,
) -> Result<(), TopologyPreparationErrorV1<L::Error>> {
    if request.challenge == [0; 32]
        || request.network_id != model.network
        || request.floor.height == 0
        || request.floor.height >= context.execution.height
        || request.floor.block_hash == [0; 32]
        || context.floor != Some(request.floor)
        || request.expected_operator == context.execution.authority
    {
        return Err(Error::Binding.into());
    }
    operation::reviewed(model, &request.reviewed, context)?;
    let (expected, reserved) = match &request.phase {
        TopologyCheckPhaseV1::Current(audit) => {
            return if **audit == model.root.audit {
                Ok(())
            } else {
                Err(Error::Conflict.into())
            };
        }
        TopologyCheckPhaseV1::BeforeProvider(row)
        | TopologyCheckPhaseV1::AfterProvider(row)
        | TopologyCheckPhaseV1::BeforeCommit(row) => (row, true),
        TopologyCheckPhaseV1::AfterCommit(row) | TopologyCheckPhaseV1::BeforeRelease(row) => {
            (row, false)
        }
    };
    let row = model
        .operation(&request.reviewed.request.operation_id)?
        .ok_or(Error::Conflict)?;
    if row != expected.as_ref()
        || row.reviewed != request.reviewed
        || row.reserved.authority != request.expected_operator
        || row.execution.authority != request.expected_operator
    {
        return Err(Error::Conflict.into());
    }
    let now = context.execution.recorded_at_unix_ms;
    if now < row.execution.recorded_at_unix_ms {
        return Err(Error::Time.into());
    }
    if reserved {
        if row.outcome != TopologyOutcomeV1::Reserved
            || model.root.active != Some(row.reviewed.request.operation_id)
            || row.revision != model.root.operation_head.revision
            || row.reviewed.intent.previous_audit != model.root.audit
            || digest(b"iroha.sorafs.topology.operation.v1\0", row)?
                != model.root.operation_head.digest
        {
            return Err(Error::Conflict.into());
        }
        if now >= row.reservation.expires_at_unix_ms {
            return Err(Error::Time.into());
        }
    } else if !matches!(row.outcome, TopologyOutcomeV1::Completed(_)) {
        return Err(Error::Conflict.into());
    }
    Ok(())
}
