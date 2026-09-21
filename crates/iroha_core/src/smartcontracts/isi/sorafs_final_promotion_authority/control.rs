//! Governed receipt control with its additional atomic operation-slot invalidation.
use super::*;

pub(super) fn prepare(
    instruction: &MutateSorafsFinalPromotionAuthority,
    authority: &AccountId,
    tx: &StateTransaction<'_, '_>,
    current: Option<&NativeControl<ReceiptPurpose>>,
    head: FinalPromotionOperationHeadV1,
    request_digest: [u8; 32],
    writes: &mut Writes,
) -> Result<(), Error> {
    use signer_custody_history::{ControlAction, ControlTransition, prepare_control};
    let action = match &instruction.action {
        Action::Configure(bytes) => ControlAction::Configure(bytes),
        Action::Enroll(bytes) => ControlAction::Enroll(bytes),
        Action::Revoke(value) => ControlAction::Revoke {
            signer: value.signer,
            attester: value.attester,
        },
        _ => return Err(Error::Invalid),
    };
    let transition = ControlTransition {
        deployment: &instruction.deployment_id,
        expected_revision: instruction.expected_control_revision,
        expected_digest: instruction.expected_control_digest,
        request_digest,
        action,
    };
    signer_custody_history::control_revision::<ReceiptPurpose>(&transition)?;
    if matches!(action, ControlAction::Enroll(_)) {
        // Receipt custody additionally requires the operation journal at the committed
        // predecessor to be coherent; retain this check after the common capacity gate.
        committed_control(tx, current.ok_or(Error::Conflict)?)?;
    }
    let prepared = prepare_control::<ReceiptPurpose>(tx, authority, current, transition)?;
    writes.extend(prepared);
    if let Some(id) = head.active_operation {
        let active = read_operation_slot(tx.world(), &instruction.deployment_id, id)?
            .ok_or(Error::CorruptHistory)?;
        if active.index.head != head
            || active.record.outcome != FinalPromotionOperationOutcomeV1::Reserved
        {
            return Err(Error::CorruptHistory);
        }
        let mut invalidated = active.record.clone();
        invalidated.revision = head.revision.checked_add(1).ok_or(Error::Capacity)?;
        invalidated.predecessor_digest = head.digest;
        invalidated.request_digest = request_digest;
        invalidated.execution = super::execution(tx, authority, Some(&active.record.execution))?;
        invalidated.outcome = FinalPromotionOperationOutcomeV1::Invalidated;
        stage_operation(tx, head, Some(&active.record), invalidated, writes)?;
    }
    Ok(())
}
