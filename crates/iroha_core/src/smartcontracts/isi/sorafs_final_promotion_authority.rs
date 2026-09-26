//! Deployment-scoped custody and durable operation CAS with native permission enforcement.
//!
//! All validation and canonical encoding precede transactional publication. Control changes also
//! terminalize the active operation slot; ordinary operations never change the custody digest.
use super::Execute;
use crate::{
    query::{
        final_promotion_authority::*,
        signer_custody_history::{
            self, AccountPurpose, NativeControl, ReceiptPurpose, read_control,
        },
    },
    state::{StateTransaction, WorldReadOnly},
};
use iroha_data_model::{
    account::AccountId,
    isi::{
        error::{InstructionExecutionError, InvalidParameterError},
        sorafs::MutateSorafsFinalPromotionAuthority,
    },
    sorafs::final_promotion_authority::{
        FINAL_PROMOTION_MAX_OPERATIONS_V1, FINAL_PROMOTION_RESERVATION_MS_V1,
        FinalPromotionAuthorityActionV1 as Action, FinalPromotionExecutionV1,
        FinalPromotionOperationOriginV1, FinalPromotionOperationOutcomeV1,
        FinalPromotionOperationRecordV1,
    },
};
use iroha_model_base::state_path::StatePath;
#[cfg(test)]
use mv::storage::StorageReadOnly;
use sorafs_manifest::signer::{
    custody::{
        SignerCustodyAnchorV1, SignerCustodyUseContextV1, VerifiedSignerCustodyV1,
        verify_signer_custody_use_v1,
    },
    protocol::{
        SignerKeyAlgorithmV1, SignerOperationActionV1, SignerOperationCustodyV1,
        SignerOperationReservationV1, SignerPurposeBindingV1, SignerRoleV1,
    },
};

mod control;
mod observation_check;
mod operation;
type Writes = Vec<(StatePath, Vec<u8>)>;

fn rejected(error: Error) -> InstructionExecutionError {
    InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
        error.to_string(),
    ))
}
fn execution(
    tx: &StateTransaction<'_, '_>,
    authority: &AccountId,
    previous: Option<&FinalPromotionExecutionV1>,
) -> Result<FinalPromotionExecutionV1, Error> {
    signer_custody_history::execution(tx, authority, previous).map_err(Into::into)
}
fn immutable(
    world: &impl WorldReadOnly,
    writes: &mut Writes,
    key: StatePath,
    bytes: Vec<u8>,
) -> Result<(), Error> {
    signer_custody_history::immutable(world, writes, key, bytes).map_err(Into::into)
}
fn committed_control(
    tx: &StateTransaction<'_, '_>,
    current: &NativeControl<ReceiptPurpose>,
) -> Result<SignerCustodyAnchorV1, Error> {
    let parent = u64::try_from(tx.block_hashes().len()).map_err(|_| Error::HeightUnavailable)?;
    let snapshot =
        read_final_promotion_authority_at_v1(tx, &current.state.policy.binding, parent, None)?
            .ok_or(Error::Conflict)?;
    if snapshot.control_record.revision != current.index.revision
        || snapshot.custody_anchor.state_digest != current.index.digest
        || snapshot.control != current.state
        || snapshot.control_record != current.record
    {
        return Err(Error::Conflict);
    }
    Ok(snapshot.custody_anchor)
}
fn use_current(
    tx: &StateTransaction<'_, '_>,
    current: &NativeControl<ReceiptPurpose>,
    expected: SignerOperationCustodyV1,
) -> Result<VerifiedSignerCustodyV1, Error> {
    let anchor = committed_control(tx, current)?;
    let active_head = current.state.active_head.ok_or(Error::Custody)?;
    if expected.record_digest != active_head.record_digest
        || expected.control_state_digest != current.index.digest
    {
        return Err(Error::Conflict);
    }
    let now = tx.block_unix_timestamp_ms();
    verify_signer_custody_use_v1(
        current.record.enrollment.as_deref().ok_or(Error::Custody)?,
        &current.state.policy.binding,
        &current.state.policy.custody_trust(),
        &SignerCustodyUseContextV1 {
            now_unix_ms: now,
            anchor_observed_at_unix_ms: now,
            current_anchor: anchor,
            active_head,
            signer_revoked: current.state.signer_revoked,
            attester_revoked: current.state.attester_revoked,
        },
    )
    .map_err(|_| Error::Custody)
}
/// Consume the executor's one-use direct signed source and current role-15 custody.
///
/// A transaction or Check result alone cannot populate a native origin. The executor supplies
/// this token only for the sole direct instruction in the exact signed outer Network entry.
fn direct_operation_source(
    tx: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    deployment: &str,
) -> Result<FinalPromotionOperationOriginV1, Error> {
    let origin = tx
        .current_direct_final_promotion_operation_origin
        .take()
        .ok_or(Error::Source)?;
    let outer = tx.current_network_entrypoint_hash.ok_or(Error::Source)?;
    if origin.entry_hash == [0; 32]
        || *outer.as_ref() != origin.entry_hash
        || tx
            .current_entrypoint_index
            .and_then(|index| u32::try_from(index).ok())
            != Some(origin.entry_index)
        || tx.tx_call_hash != Some(iroha_crypto::Hash::from(outer))
        || tx.current_tx_hash.is_none()
    {
        return Err(Error::Source);
    }
    let account = read_control::<AccountPurpose>(tx.world(), deployment)?.ok_or(Error::Custody)?;
    let binding = &account.state.policy.binding;
    let SignerPurposeBindingV1::FinalPromotionAccountTransaction { deployment_id } =
        &binding.purpose
    else {
        return Err(Error::Custody);
    };
    if deployment_id != deployment
        || binding.role != SignerRoleV1::FinalPromotionAccountTransaction
        || binding.algorithm != SignerKeyAlgorithmV1::Ed25519
        || AccountId::new(binding.public_key.clone()) != *authority
    {
        return Err(Error::Custody);
    }
    let anchor = signer_custody_history::committed_control::<AccountPurpose>(tx, &account)?;
    let now = tx.block_unix_timestamp_ms();
    // The committed parent State is observed at this transaction's logical block time. This
    // does not renew the signed enrollment: its expiry and active-head anchor remain unchanged.
    verify_signer_custody_use_v1(
        account.record.enrollment.as_deref().ok_or(Error::Custody)?,
        binding,
        &account.state.policy.custody_trust(),
        &SignerCustodyUseContextV1 {
            now_unix_ms: now,
            anchor_observed_at_unix_ms: now,
            current_anchor: anchor,
            active_head: account.state.active_head.ok_or(Error::Custody)?,
            signer_revoked: account.state.signer_revoked,
            attester_revoked: account.state.attester_revoked,
        },
    )
    .map_err(|_| Error::Custody)?;
    Ok(origin)
}
fn stage_operation(
    tx: &StateTransaction<'_, '_>,
    head: FinalPromotionOperationHeadV1,
    previous: Option<&FinalPromotionOperationRecordV1>,
    record: FinalPromotionOperationRecordV1,
    writes: &mut Writes,
) -> Result<(), Error> {
    let next = crate::query::final_promotion_authority::operation::successor_head(
        head, previous, &record,
    )?;
    let index = OperationIndexV1 {
        head: next,
        height: record.execution.height,
        ordinal: record.execution.ordinal,
    };
    let index_bytes = encode(&index)?;
    immutable(
        tx.world(),
        writes,
        operation_record_key(&record.deployment_id, record.revision)?,
        encode(&record)?,
    )?;
    immutable(
        tx.world(),
        writes,
        operation_height_key(&record.deployment_id, index.height, index.ordinal)?,
        index_bytes.clone(),
    )?;
    if record.outcome == FinalPromotionOperationOutcomeV1::Reserved {
        immutable(
            tx.world(),
            writes,
            operation_admission_key(&record.deployment_id, record.intent.operation_id)?,
            index_bytes.clone(),
        )?;
    }
    writes.push((
        operation_slot_key(&record.deployment_id, record.intent.operation_id)?,
        index_bytes,
    ));
    writes.push((operation_head_key(&record.deployment_id)?, encode(&next)?));
    Ok(())
}
fn apply(
    instruction: MutateSorafsFinalPromotionAuthority,
    authority: &AccountId,
    tx: &mut StateTransaction<'_, '_>,
) -> Result<(), Error> {
    if !valid_deployment(&instruction.deployment_id) {
        return Err(Error::BindingMismatch);
    }
    let current = read_control::<ReceiptPurpose>(tx.world(), &instruction.deployment_id)?;
    let head = read_operation_head(tx.world(), &instruction.deployment_id)?;
    if current.as_ref().map_or(0, |value| value.index.revision)
        != instruction.expected_control_revision
        || current.as_ref().map_or([0; 32], |value| value.index.digest)
            != instruction.expected_control_digest
    {
        return Err(Error::Conflict);
    }
    if let Some(current) = &current {
        validate_binding(
            tx,
            &current.state.policy.binding,
            &instruction.deployment_id,
        )?;
    } else if head.revision != 0 {
        return Err(Error::CorruptHistory);
    }
    if let Action::Check(check) = &instruction.action {
        // Check has no mutation request digest, but retains the same canonical instruction bound.
        encode(&instruction)?;
        return observation_check::execute(
            check,
            authority,
            tx,
            current.as_ref().ok_or(Error::Conflict)?,
            head,
        );
    }
    let request_digest = final_promotion_authority_request_digest_v1(&instruction, authority)?;
    let mut writes = Writes::new();
    match &instruction.action {
        Action::Configure(_) | Action::Enroll(_) | Action::Revoke(_) => control::prepare(
            &instruction,
            authority,
            tx,
            current.as_ref(),
            head,
            request_digest,
            &mut writes,
        )?,
        _ => operation::prepare(
            &instruction,
            authority,
            tx,
            current.as_ref().ok_or(Error::Conflict)?,
            head,
            request_digest,
            &mut writes,
        )?,
    }
    for (key, bytes) in writes {
        tx.world.smart_contract_state.insert(key, bytes);
    }
    Ok(())
}
impl Execute for MutateSorafsFinalPromotionAuthority {
    fn execute(
        self,
        authority: &AccountId,
        tx: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        if !valid_deployment(&self.deployment_id)
            || !check::authorized(tx.world(), authority, &self.deployment_id, &self.action)
        {
            return Err(rejected(Error::BindingMismatch));
        }
        apply(self, authority, tx).map_err(rejected)
    }
}

#[cfg(test)]
mod tests;
