//! Execute a fresh challenge without allocating a transition or changing native authority.
use super::*;
use iroha_data_model::sorafs::final_promotion_authority::FinalPromotionCheckV1;

pub(super) fn execute(
    request: &FinalPromotionCheckV1,
    observer: &AccountId,
    tx: &StateTransaction<'_, '_>,
    control: &NativeControl<ReceiptPurpose>,
    head: FinalPromotionOperationHeadV1,
) -> Result<(), Error> {
    let height = u64::try_from(tx.block_hashes().len())
        .map_err(|_| Error::HeightUnavailable)?
        .checked_add(1)
        .ok_or(Error::HeightUnavailable)?;
    if height != tx._curr_block.height().get() {
        return Err(Error::HeightUnavailable);
    }
    check::check_floor(tx, request, height)?;
    let custody = use_current(tx, control, request.request.original_custody)?;
    let deployment = &control.record.deployment_id;
    let selected = read_operation_slot(tx.world(), deployment, request.request.operation_id)?;
    // Check Current can name an old or unused ID. It must still reject a corrupted other active
    // slot, and a current slot must remain bound to this exact custody/control history.
    let other_active = head
        .active_operation
        .filter(|id| *id != request.request.operation_id)
        .map(|id| read_operation_slot(tx.world(), deployment, id))
        .transpose()?
        .flatten();
    if let Some(active_id) = head.active_operation {
        let active = if active_id == request.request.operation_id {
            selected.as_ref()
        } else {
            other_active.as_ref()
        }
        .ok_or(Error::CorruptHistory)?;
        if active.index.head != head
            || active.record.outcome != FinalPromotionOperationOutcomeV1::Reserved
            || active.record.custody.control_state_digest != control.index.digest
            || control
                .state
                .active_head
                .is_none_or(|value| value.record_digest != active.record.custody.record_digest)
        {
            return Err(Error::CorruptHistory);
        }
    }
    check::check_subject(
        request,
        observer,
        &custody,
        &head,
        selected.as_ref().map(|row| &row.record),
        tx.block_unix_timestamp_ms(),
    )
}
