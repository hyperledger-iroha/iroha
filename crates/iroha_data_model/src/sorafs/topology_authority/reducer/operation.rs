//! Exact candidate reservation, owner-preserving terminalization and no-write recovery observation.
use super::*;
use sorafs_manifest::signer::{
    protocol::SignerOperationActionV1, topology::subject::prepare_topology_approval_v1,
};

pub(super) fn reviewed<L: TopologyIndexedReadV1 + ?Sized>(
    model: &TopologyStateViewV1<'_, L>,
    reviewed: &TopologyReserveV1,
    context: &TopologyContextClaimV1,
) -> Result<VerifiedSignerCustodyV1, TopologyPreparationErrorV1<L::Error>> {
    if reviewed.subject.chain_discriminant != model.chain_discriminant {
        return Err(Error::Binding.into());
    }
    let custody = model.use_current(context)?;
    let prepared = prepare_topology_approval_v1(&reviewed.subject, &custody.statement().binding)
        .map_err(|_| Error::Binding)?;
    let request = SignerTopologyRequestV1::new(&custody, reviewed.request.operation_id, &prepared)
        .map_err(|_| Error::Binding)?;
    if request != reviewed.request
        || reviewed.intent.action != SignerOperationActionV1::Sign
        || reviewed.intent.operation_id != request.operation_id
        || reviewed.intent.request_digest != request.digest().map_err(|_| Error::Invalid)?
        || reviewed.intent.digest().is_err()
    {
        return Err(Error::Binding.into());
    }
    let now = context.execution.recorded_at_unix_ms;
    if now < reviewed.subject.reviewed_at_unix_ms || now >= reviewed.subject.expires_at_unix_ms {
        return Err(Error::Time.into());
    }
    Ok(custody)
}
pub(super) fn prepare<L: TopologyIndexedReadV1 + ?Sized>(
    model: &TopologyStateViewV1<'_, L>,
    action: &TopologyActionV1,
    context: &TopologyContextClaimV1,
    transition_digest: [u8; 32],
) -> Result<Option<TopologyOperationRecordV1>, TopologyPreparationErrorV1<L::Error>> {
    match action {
        TopologyActionV1::Reserve(request) => reserve(model, request, context, transition_digest),
        TopologyActionV1::Complete(request) => complete(model, request, context, transition_digest),
        TopologyActionV1::Expire(request) => expire(model, request, context, transition_digest),
        _ => Err(Error::Invalid.into()),
    }
}
fn reserve<L: TopologyIndexedReadV1 + ?Sized>(
    model: &TopologyStateViewV1<'_, L>,
    request: &TopologyReserveV1,
    context: &TopologyContextClaimV1,
    transition_digest: [u8; 32],
) -> Result<Option<TopologyOperationRecordV1>, TopologyPreparationErrorV1<L::Error>> {
    let custody = reviewed(model, request, context)?;
    if let Some(existing) = model.operation(&request.request.operation_id)? {
        if existing.outcome == TopologyOutcomeV1::Reserved
            && existing.reviewed == *request
            && existing.reserved.authority == context.execution.authority
            && model.root.active == Some(request.request.operation_id)
            && context.execution.recorded_at_unix_ms < existing.reservation.expires_at_unix_ms
        {
            return Ok(None);
        }
        return Err(Error::Conflict.into());
    }
    if model.root.active.is_some() || request.intent.previous_audit != model.root.audit {
        return Err(Error::Conflict.into());
    }
    if model.root.operation_count >= TOPOLOGY_OPERATION_LIMIT_V1
        || model
            .root
            .operation_head
            .revision
            .checked_add(2)
            .is_none_or(|revision| revision > 2 * TOPOLOGY_OPERATION_LIMIT_V1)
    {
        return Err(Error::Capacity.into());
    }
    let fence = model.root.fence.checked_add(1).ok_or(Error::Capacity)?;
    let now = context.execution.recorded_at_unix_ms;
    let expires_at_unix_ms = now
        .checked_add(TOPOLOGY_RESERVATION_MS_V1)
        .ok_or(Error::Time)?
        .min(custody.statement().expires_at_unix_ms)
        .min(
            model
                .state
                .ok_or(Error::Custody)?
                .policy
                .active_until_unix_ms,
        )
        .min(request.subject.expires_at_unix_ms);
    if expires_at_unix_ms <= now || expires_at_unix_ms == u64::MAX {
        return Err(Error::Time.into());
    }
    let reservation_id = digest(
        b"iroha.sorafs.topology.reservation.v1\0",
        &(
            model.deployment.to_owned(),
            request.clone(),
            fence,
            expires_at_unix_ms,
            context.execution.clone(),
        ),
    )?;
    Ok(Some(TopologyOperationRecordV1 {
        deployment_id: model.deployment.to_owned(),
        revision: model
            .root
            .operation_head
            .revision
            .checked_add(1)
            .ok_or(Error::Capacity)?,
        predecessor_digest: model.root.operation_head.digest,
        transition_digest,
        execution: context.execution.clone(),
        reserved: context.execution.clone(),
        reviewed: request.clone(),
        reservation: SignerOperationReservationV1 {
            reservation_id,
            fence,
            expires_at_unix_ms,
        },
        outcome: TopologyOutcomeV1::Reserved,
    }))
}
fn terminal<L: TopologyIndexedReadV1 + ?Sized>(
    model: &TopologyStateViewV1<'_, L>,
    old: &TopologyOperationRecordV1,
    context: &TopologyContextClaimV1,
    transition_digest: [u8; 32],
    outcome: TopologyOutcomeV1,
) -> Result<TopologyOperationRecordV1, TopologyPreparationErrorV1<L::Error>> {
    if old.outcome != TopologyOutcomeV1::Reserved
        || model.root.active != Some(old.reviewed.request.operation_id)
        || old.revision != model.root.operation_head.revision
        || digest(b"iroha.sorafs.topology.operation.v1\0", old)? != model.root.operation_head.digest
    {
        return Err(Error::Conflict.into());
    }
    let mut row = old.clone();
    row.revision = model
        .root
        .operation_head
        .revision
        .checked_add(1)
        .ok_or(Error::Capacity)?;
    if row.revision > 2 * TOPOLOGY_OPERATION_LIMIT_V1 {
        return Err(Error::Capacity.into());
    }
    row.predecessor_digest = model.root.operation_head.digest;
    row.transition_digest = transition_digest;
    row.execution = context.execution.clone();
    row.outcome = outcome;
    Ok(row)
}
fn complete<L: TopologyIndexedReadV1 + ?Sized>(
    model: &TopologyStateViewV1<'_, L>,
    request: &TopologyCompleteV1,
    context: &TopologyContextClaimV1,
    transition_digest: [u8; 32],
) -> Result<Option<TopologyOperationRecordV1>, TopologyPreparationErrorV1<L::Error>> {
    let old = model
        .operation(&request.request.operation_id)?
        .ok_or(Error::Conflict)?;
    if old.reviewed.request != request.request
        || old.reviewed.intent != request.intent
        || old.reservation != request.reservation
        || old.reserved.authority != context.execution.authority
    {
        return Err(Error::Conflict.into());
    }
    let outcome = TopologyOutcomeV1::Completed(TopologyCompletionV1 {
        commitment: request.commitment,
        signatures_digest: request.signatures_digest,
    });
    if old.outcome == outcome {
        return Ok(None);
    } // Observation only; fresh Check still gates release.
    reviewed(model, &old.reviewed, context)?;
    if context.execution.height <= old.reserved.height
        || context.execution.recorded_at_unix_ms >= old.reservation.expires_at_unix_ms
    {
        return Err(Error::Time.into());
    }
    if request.intent.previous_audit != model.root.audit
        || model.root.audit.sequence.checked_add(1) != Some(request.commitment.audit.sequence)
        || request.commitment.audit.digest == [0; 32]
        || request.commitment.audit.digest == model.root.audit.digest
        || request.commitment.response_digest == [0; 32]
        || request.signatures_digest == [0; 32]
    {
        return Err(Error::Conflict.into());
    }
    Ok(Some(terminal(
        model,
        old,
        context,
        transition_digest,
        outcome,
    )?))
}
fn expire<L: TopologyIndexedReadV1 + ?Sized>(
    model: &TopologyStateViewV1<'_, L>,
    request: &TopologyExpireV1,
    context: &TopologyContextClaimV1,
    transition_digest: [u8; 32],
) -> Result<Option<TopologyOperationRecordV1>, TopologyPreparationErrorV1<L::Error>> {
    let old = model
        .operation(&request.operation_id)?
        .ok_or(Error::Conflict)?;
    if old.reservation != request.reservation {
        return Err(Error::Conflict.into());
    }
    if old.outcome == TopologyOutcomeV1::Expired
        && old.execution.authority == context.execution.authority
    {
        return Ok(None);
    }
    if context.execution.recorded_at_unix_ms < request.reservation.expires_at_unix_ms {
        return Err(Error::Time.into());
    }
    Ok(Some(terminal(
        model,
        old,
        context,
        transition_digest,
        TopologyOutcomeV1::Expired,
    )?))
}
pub(super) fn invalidate<L: TopologyIndexedReadV1 + ?Sized>(
    model: &TopologyStateViewV1<'_, L>,
    context: &TopologyContextClaimV1,
    transition_digest: [u8; 32],
) -> Result<Option<TopologyOperationRecordV1>, TopologyPreparationErrorV1<L::Error>> {
    let Some(id) = model.root.active else {
        return Ok(None);
    };
    let old = model.operation(&id)?.ok_or(Error::History)?;
    Ok(Some(terminal(
        model,
        old,
        context,
        transition_digest,
        TopologyOutcomeV1::Invalidated,
    )?))
}
