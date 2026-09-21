//! Native exclusive reservation, timely completion and explicit expiration.
use super::*;
use iroha_data_model::sorafs::final_promotion_authority::FinalPromotionCompletedV1;

pub(super) fn prepare(
    instruction: &MutateSorafsFinalPromotionAuthority,
    authority: &AccountId,
    tx: &StateTransaction<'_, '_>,
    control: &NativeControl<ReceiptPurpose>,
    head: FinalPromotionOperationHeadV1,
    request_digest: [u8; 32],
    writes: &mut Writes,
) -> Result<(), Error> {
    let previous = if head.revision == 0 {
        None
    } else {
        Some(read_operation_record(
            tx.world(),
            &instruction.deployment_id,
            head.revision,
        )?)
    };
    let execution = execution(
        tx,
        authority,
        previous.as_ref().map(|row| &row.record.execution),
    )?;
    match &instruction.action {
        Action::Reserve(request) => {
            if request.intent.action != SignerOperationActionV1::Sign
                || request.intent.digest().is_err()
            {
                return Err(Error::Invalid);
            }
            let custody = use_current(tx, control, request.custody)?;
            let parent_height =
                u64::try_from(tx.block_hashes().len()).map_err(|_| Error::HeightUnavailable)?;
            let committed =
                crate::query::final_promotion_authority::operation::read_operation_head_at(
                    tx.world(),
                    &instruction.deployment_id,
                    parent_height,
                )?;
            if committed.audit != request.intent.previous_audit {
                return Err(Error::Conflict);
            }
            if let Some(existing) = read_operation_slot(
                tx.world(),
                &instruction.deployment_id,
                request.intent.operation_id,
            )? {
                if existing.record.outcome == FinalPromotionOperationOutcomeV1::Reserved
                    && existing.index.head == head
                    && existing.record.request_digest == request_digest
                    && existing.record.reserved.authority == *authority
                    && execution.recorded_at_unix_ms
                        < existing.record.reservation.expires_at_unix_ms
                {
                    return Ok(());
                }
                return Err(Error::Conflict);
            }
            if head.active_operation.is_some() || request.intent.previous_audit != head.audit {
                return Err(Error::Conflict);
            }
            // Every admitted identity permanently owns room for both its reservation and terminal
            // row. Exhaustion cannot strand an active slot or consume emergency revocation space.
            if head.total_admissions >= FINAL_PROMOTION_MAX_OPERATIONS_V1
                || head
                    .revision
                    .checked_add(2)
                    .is_none_or(|revision| revision > FINAL_PROMOTION_MAX_OPERATIONS_V1 * 2)
            {
                return Err(Error::Capacity);
            }
            let fence = head.fence.checked_add(1).ok_or(Error::Capacity)?;
            let expires_at_unix_ms = execution
                .recorded_at_unix_ms
                .checked_add(FINAL_PROMOTION_RESERVATION_MS_V1)
                .ok_or(Error::ReservationTime)?
                .min(custody.statement().expires_at_unix_ms)
                .min(control.state.policy.active_until_unix_ms);
            if expires_at_unix_ms <= execution.recorded_at_unix_ms || expires_at_unix_ms == u64::MAX
            {
                return Err(Error::ReservationTime);
            }
            let reservation_id = digest(
                b"iroha.sorafs.final-promotion.reservation.v1\0",
                &(
                    instruction.deployment_id.clone(),
                    request.intent,
                    request.custody,
                    fence,
                    expires_at_unix_ms,
                    execution.clone(),
                ),
            )?;
            let record = FinalPromotionOperationRecordV1 {
                deployment_id: instruction.deployment_id.clone(),
                revision: head.revision.checked_add(1).ok_or(Error::Capacity)?,
                predecessor_digest: head.digest,
                request_digest,
                execution: execution.clone(),
                intent: request.intent,
                custody: request.custody,
                reservation: SignerOperationReservationV1 {
                    reservation_id,
                    fence,
                    expires_at_unix_ms,
                },
                reserved: execution,
                outcome: FinalPromotionOperationOutcomeV1::Reserved,
            };
            stage_operation(
                tx,
                head,
                previous.as_ref().map(|row| &row.record),
                record,
                writes,
            )
        }
        Action::Complete(request) => {
            let active = read_operation_slot(
                tx.world(),
                &instruction.deployment_id,
                request.intent.operation_id,
            )?
            .ok_or(Error::Conflict)?;
            // This is an idempotent observation of an already committed request, not a new commit
            // or permission to release expired custody. Recovery separately rechecks eligibility.
            if matches!(
                active.record.outcome,
                FinalPromotionOperationOutcomeV1::Completed(_)
            ) && active.record.request_digest == request_digest
                && active.record.reserved.authority == *authority
            {
                return Ok(());
            }
            use_current(tx, control, request.custody)?;
            if active.index.head != head
                || head.active_operation != Some(request.intent.operation_id)
                || active.record.outcome != FinalPromotionOperationOutcomeV1::Reserved
                || active.record.intent != request.intent
                || active.record.custody != request.custody
                || active.record.reservation != request.reservation
                || active.record.reserved.authority != *authority
                || request.intent.previous_audit != head.audit
            {
                return Err(Error::Conflict);
            }
            if execution.height <= active.record.reserved.height
                || execution.recorded_at_unix_ms >= request.reservation.expires_at_unix_ms
            {
                return Err(Error::ReservationTime);
            }
            let mut record = active.record.clone();
            record.revision = head.revision.checked_add(1).ok_or(Error::Capacity)?;
            record.predecessor_digest = head.digest;
            record.request_digest = request_digest;
            record.execution = execution;
            record.outcome =
                FinalPromotionOperationOutcomeV1::Completed(FinalPromotionCompletedV1 {
                    commitment: request.commitment,
                    signatures_digest: request.signatures_digest,
                });
            stage_operation(tx, head, Some(&active.record), record, writes)
        }
        Action::Expire(request) => {
            let active =
                read_operation_slot(tx.world(), &instruction.deployment_id, request.operation_id)?
                    .ok_or(Error::Conflict)?;
            if active.record.outcome == FinalPromotionOperationOutcomeV1::Expired
                && active.record.request_digest == request_digest
                && active.record.execution.authority == *authority
            {
                return Ok(());
            }
            if active.index.head != head
                || head.active_operation != Some(request.operation_id)
                || active.record.outcome != FinalPromotionOperationOutcomeV1::Reserved
                || active.record.reservation != request.reservation
            {
                return Err(Error::Conflict);
            }
            if execution.recorded_at_unix_ms < request.reservation.expires_at_unix_ms {
                return Err(Error::ReservationTime);
            }
            let mut record = active.record.clone();
            record.revision = head.revision.checked_add(1).ok_or(Error::Capacity)?;
            record.predecessor_digest = head.digest;
            record.request_digest = request_digest;
            record.execution = execution;
            record.outcome = FinalPromotionOperationOutcomeV1::Expired;
            stage_operation(tx, head, Some(&active.record), record, writes)
        }
        _ => Err(Error::Invalid),
    }
}
