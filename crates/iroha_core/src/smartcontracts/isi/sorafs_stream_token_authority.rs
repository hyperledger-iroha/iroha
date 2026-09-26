//! Native provider-scoped role-11 operations and no-write challenged Check predicate.
//!
//! A successful native Check alone does not authenticate its signed finalization, earlier
//! operation execution, private receipt, or stream-token release.

use super::Execute;
use crate::{
    query::{
        signer_finality::verify_signer_finality_v1,
        stream_token_authority::{
            self as journal, Error, OperationHeadV1, OperationRecordV1,
            STREAM_TOKEN_NATIVE_MAX_OPERATIONS_V1, STREAM_TOKEN_NATIVE_RESERVATION_MS_V1,
        },
        stream_token_custody::{
            read_active, read_stream_token_custody_control_at_v1, validate_state_binding,
        },
    },
    state::{StateReadOnly, StateTransaction, WorldReadOnly},
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    account::AccountId,
    isi::{
        error::{InstructionExecutionError, InvalidParameterError},
        sorafs::MutateSorafsStreamTokenAuthority,
    },
    permission::Permission,
    sorafs::{
        capacity::ProviderId,
        stream_token_authority::{
            StreamTokenAuthorityActionV1 as Action, StreamTokenCompleteV1, StreamTokenExecutionV1,
            StreamTokenNativeOperationV1, StreamTokenOperationV1, StreamTokenOutcomeV1,
            validate_stream_token_complete_request_claim_v1, validate_stream_token_expire_claim_v1,
            validate_stream_token_native_operation_claim_v1,
            validate_stream_token_reviewed_claim_v1,
        },
    },
    transaction::TransactionEntrypoint,
};
use iroha_executor_data_model::permission::sorafs::{
    CanCheckSorafsStreamToken, CanManageSorafsStreamTokenCustody, CanOperateSorafsStreamToken,
};
use mv::storage::StorageReadOnly;

fn rejected(error: Error) -> InstructionExecutionError {
    InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
        error.to_string(),
    ))
}

fn has_permission(
    world: &impl WorldReadOnly,
    authority: &AccountId,
    permission: Permission,
) -> bool {
    world.accounts().get(authority).is_some()
        && (world.account_contains_inherent_permission(authority, &permission)
            || world
                .account_roles_iter(authority)
                .filter_map(|id| world.roles().get(id))
                .any(|role| role.permissions().any(|token| token == &permission)))
}

fn authorized(
    tx: &StateTransaction<'_, '_>,
    authority: &AccountId,
    provider: ProviderId,
    action: &Action,
) -> bool {
    let world = tx.world();
    let is_owner = tx.world.provider_owners.get(&provider) == Some(authority);
    let can_operate = has_permission(
        world,
        authority,
        CanOperateSorafsStreamToken {
            provider_id: provider,
        }
        .into(),
    );
    match action {
        Action::Reserve(_) | Action::Complete(_) => is_owner && can_operate,
        Action::Expire(_) => {
            is_owner && can_operate
                || has_permission(
                    world,
                    authority,
                    CanManageSorafsStreamTokenCustody {
                        provider_id: provider,
                    }
                    .into(),
                )
        }
        Action::Check(check) => {
            authority == &check.expected_observer
                && authority != &check.expected_operator
                && tx.world.provider_owners.get(&provider) == Some(&check.expected_operator)
                && has_permission(
                    world,
                    &check.expected_operator,
                    CanOperateSorafsStreamToken {
                        provider_id: provider,
                    }
                    .into(),
                )
                && has_permission(
                    world,
                    authority,
                    CanCheckSorafsStreamToken {
                        provider_id: provider,
                    }
                    .into(),
                )
        }
    }
}

/// Consume one exact directly signed role-11 instruction position before any mutation.
fn execution(
    tx: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
) -> Result<StreamTokenExecutionV1, Error> {
    let instruction_index = tx
        .current_direct_stream_token_instruction_index
        .take()
        .ok_or(Error::Execution)?;
    let outer: HashOf<TransactionEntrypoint> =
        tx.current_network_entrypoint_hash.ok_or(Error::Execution)?;
    let inner = tx.tx_call_hash.ok_or(Error::Execution)?;
    if outer != HashOf::from_untyped_unchecked(inner) || tx.current_tx_hash.is_none() {
        // SealedReveal uses an outer result entry and an inner signed call; no role-11
        // operation may claim the outer execution under the inner call hash.
        return Err(Error::Execution);
    }
    let height = tx._curr_block.height().get();
    let parent = u64::try_from(tx.block_hashes().len()).map_err(|_| Error::Execution)?;
    let entry_index = tx
        .current_entrypoint_index
        .and_then(|index| u32::try_from(index).ok())
        .ok_or(Error::Execution)?;
    let recorded_at_unix_ms = tx.block_unix_timestamp_ms();
    if height != parent.checked_add(1).ok_or(Error::Execution)?
        || recorded_at_unix_ms == 0
        || recorded_at_unix_ms == u64::MAX
    {
        return Err(Error::Execution);
    }
    Ok(StreamTokenExecutionV1 {
        height,
        transaction_hash: *outer.as_ref(),
        entry_index,
        instruction_index,
        recorded_at_unix_ms,
        authority: authority.clone(),
    })
}

fn current_custody(
    tx: &StateTransaction<'_, '_>,
    provider: ProviderId,
    revision: u64,
    digest: [u8; 32],
) -> Result<crate::query::stream_token_custody::NativeControl, Error> {
    let current = read_active(tx.world(), provider)
        .map_err(|_| Error::Custody)?
        .ok_or(Error::Custody)?;
    if current.index.revision != revision
        || current.index.digest != digest
        || validate_state_binding(tx, &current.state.policy.binding).map_err(|_| Error::Custody)?
            != provider
    {
        return Err(Error::Custody);
    }
    let parent = u64::try_from(tx.block_hashes().len()).map_err(|_| Error::Custody)?;
    let committed =
        read_stream_token_custody_control_at_v1(tx, &current.state.policy.binding, parent)
            .map_err(|_| Error::Custody)?
            .ok_or(Error::Custody)?;
    if committed.anchor.state_digest != current.index.digest || committed.state != current.state {
        return Err(Error::Custody);
    }
    verify_signer_finality_v1(tx, parent, committed.anchor.block_hash)
        .map_err(|_| Error::Custody)?;
    Ok(current)
}

fn eligible_custody(
    current: &crate::query::stream_token_custody::NativeControl,
    original_record_digest: [u8; 32],
    now: u64,
) -> Result<(), Error> {
    let state = &current.state;
    let active = state.active_head.ok_or(Error::Custody)?;
    if state.signer_revoked
        || state.attester_revoked
        || now < state.policy.active_from_unix_ms
        || now >= state.policy.active_until_unix_ms
        || active.record_digest != original_record_digest
        || active.key_revision != state.policy.binding.key_revision
        || active.policy_revision != state.policy.binding.policy_revision
        || active.policy_digest != state.policy.binding.policy_digest
    {
        return Err(Error::Custody);
    }
    Ok(())
}

fn stage_transition(
    tx: &mut StateTransaction<'_, '_>,
    provider: ProviderId,
    record: OperationRecordV1,
    next: OperationHeadV1,
    first_admission: bool,
) -> Result<(), Error> {
    validate_stream_token_native_operation_claim_v1(
        &record.operation,
        provider,
        record.operation.custody_control_revision,
        record.operation.custody_control_digest,
        &record.operation.reserved_execution.authority,
    )
    .map_err(|_| Error::Invalid)?;
    let id = record.operation.operation.reviewed.request.operation_id;
    let record_path = journal::record_key(provider, record.revision);
    let admission_path = journal::admission_key(provider, id);
    let slot_path = journal::slot_key(provider, id);
    if tx.world.smart_contract_state.get(&record_path).is_some()
        || first_admission
            && (tx.world.smart_contract_state.get(&admission_path).is_some()
                || tx.world.smart_contract_state.get(&slot_path).is_some())
        || !first_admission && tx.world.smart_contract_state.get(&slot_path).is_none()
    {
        return Err(Error::CorruptHistory);
    }
    let record_bytes = journal::encode(&record)?;
    let revision_bytes = journal::encode(&record.revision)?;
    let head_bytes = journal::encode(&next)?;
    // All fallible CAS checks and encodes precede publication to the transaction overlay.
    tx.world
        .smart_contract_state
        .insert(record_path, record_bytes);
    if first_admission {
        tx.world
            .smart_contract_state
            .insert(admission_path, revision_bytes.clone());
    }
    tx.world
        .smart_contract_state
        .insert(slot_path, revision_bytes);
    tx.world
        .smart_contract_state
        .insert(journal::head_key(provider), head_bytes);
    Ok(())
}

fn apply(
    instruction: MutateSorafsStreamTokenAuthority,
    authority: &AccountId,
    tx: &mut StateTransaction<'_, '_>,
    execution: StreamTokenExecutionV1,
) -> Result<(), Error> {
    let request = &instruction.request;
    let provider = request.provider_id;
    if request.network_id != *tx.network_id().as_bytes()
        || request.network_id == [0; 32]
        || provider.as_bytes() == &[0; 32]
    {
        return Err(Error::BindingMismatch);
    }
    if let Action::Check(check) = &request.action {
        return check::evaluate_check(&instruction, authority, tx, &execution, check);
    }
    let current = current_custody(
        tx,
        provider,
        request.expected_control_revision,
        request.expected_control_digest,
    )?;
    let head = journal::read_head(tx.world(), provider)?;
    let digest = journal::request_digest(&instruction, authority)?;
    let now = execution.recorded_at_unix_ms;
    let (row, next_audit, next_fence, next_total, active, first_admission) = match &request.action {
        Action::Reserve(reviewed) => {
            validate_stream_token_reviewed_claim_v1(reviewed, &reviewed.request, head.audit)
                .map_err(|_| Error::Invalid)?;
            eligible_custody(
                &current,
                reviewed.request.original_custody.record_digest,
                now,
            )?;
            if reviewed.request.original_custody.control_state_digest != current.index.digest
                || now < reviewed.request.issued_at_unix_ms
                || now >= reviewed.request.expires_at_unix_ms
                || head.active_operation.is_some()
                || journal::read_slot(tx.world(), provider, reviewed.request.operation_id)?
                    .is_some()
            {
                return Err(Error::Conflict);
            }
            if head.total_admissions >= STREAM_TOKEN_NATIVE_MAX_OPERATIONS_V1
                || head
                    .revision
                    .checked_add(2)
                    .is_none_or(|revision| revision > STREAM_TOKEN_NATIVE_MAX_OPERATIONS_V1 * 2)
            {
                return Err(Error::Capacity);
            }
            let fence = head.fence.checked_add(1).ok_or(Error::Capacity)?;
            let expires = now
                .checked_add(STREAM_TOKEN_NATIVE_RESERVATION_MS_V1)
                .ok_or(Error::Capacity)?
                .min(reviewed.request.expires_at_unix_ms)
                .min(current.state.policy.active_until_unix_ms);
            if expires <= now || expires == u64::MAX {
                return Err(Error::Custody);
            }
            let reservation_id = {
                let frame = journal::encode(&(
                    provider,
                    *reviewed,
                    current.index.digest,
                    fence,
                    expires,
                    execution.clone(),
                ))?;
                let mut preimage = b"iroha.sorafs.stream-token.reservation.v1\0".to_vec();
                preimage.extend_from_slice(&frame);
                *Hash::new(preimage).as_ref()
            };
            let operation = StreamTokenOperationV1 {
                reviewed: *reviewed,
                reservation: sorafs_manifest::signer::protocol::SignerOperationReservationV1 {
                    reservation_id,
                    fence,
                    expires_at_unix_ms: expires,
                },
                outcome: StreamTokenOutcomeV1::Reserved,
            };
            (
                StreamTokenNativeOperationV1 {
                    provider_id: provider,
                    custody_control_revision: current.index.revision,
                    custody_control_digest: current.index.digest,
                    operation,
                    reserved_execution: execution,
                    terminal_execution: None,
                },
                head.audit,
                fence,
                fence,
                Some(reviewed.request.operation_id),
                true,
            )
        }
        Action::Complete(completion) => {
            let id = completion.reviewed.request.operation_id;
            let original = journal::read_slot(tx.world(), provider, id)?.ok_or(Error::Conflict)?;
            let mut row = original.operation;
            if head.active_operation != Some(id)
                || row.operation.outcome != StreamTokenOutcomeV1::Reserved
                || row.custody_control_revision != current.index.revision
                || row.custody_control_digest != current.index.digest
                || row.reserved_execution.authority != *authority
                || execution.height <= row.reserved_execution.height
                || original.revision != head.revision
                || now >= row.operation.reservation.expires_at_unix_ms
                || now >= row.operation.reviewed.request.expires_at_unix_ms
            {
                return Err(Error::Conflict);
            }
            eligible_custody(
                &current,
                row.operation
                    .reviewed
                    .request
                    .original_custody
                    .record_digest,
                now,
            )?;
            validate_stream_token_complete_request_claim_v1(completion, &row.operation)
                .map_err(|_| Error::Invalid)?;
            let completed = StreamTokenCompleteV1 {
                reviewed: completion.reviewed,
                reservation: completion.reservation,
                commitment: completion.commitment,
                signatures_digest: completion.signatures_digest,
                completed_at_unix_ms: now,
            };
            row.operation.outcome = StreamTokenOutcomeV1::Completed(completed);
            row.terminal_execution = Some(execution);
            (
                row,
                completion.commitment.audit,
                head.fence,
                head.total_admissions,
                None,
                false,
            )
        }
        Action::Expire(expired) => {
            let id = expired.operation_id;
            let original = journal::read_slot(tx.world(), provider, id)?.ok_or(Error::Conflict)?;
            let mut row = original.operation;
            if head.active_operation != Some(id)
                || row.operation.outcome != StreamTokenOutcomeV1::Reserved
                || original.revision != head.revision
            {
                return Err(Error::Conflict);
            }
            validate_stream_token_expire_claim_v1(expired, &row.operation)
                .map_err(|_| Error::Invalid)?;
            let stale_control = row.custody_control_revision != current.index.revision
                || row.custody_control_digest != current.index.digest
                || current.state.signer_revoked
                || current.state.attester_revoked;
            if !stale_control && now < row.operation.reservation.expires_at_unix_ms {
                return Err(Error::Custody);
            }
            row.operation.outcome = StreamTokenOutcomeV1::Expired;
            row.terminal_execution = Some(execution);
            (
                row,
                head.audit,
                head.fence,
                head.total_admissions,
                None,
                false,
            )
        }
        Action::Check(_) => return Err(Error::CheckUnavailable),
    };
    let revision = head.revision.checked_add(1).ok_or(Error::Capacity)?;
    let record = OperationRecordV1 {
        revision,
        predecessor_digest: head.digest,
        request_digest: digest,
        operation: row,
    };
    let next = OperationHeadV1 {
        revision,
        digest: journal::record_digest(&record)?,
        fence: next_fence,
        audit: next_audit,
        active_operation: active,
        total_admissions: next_total,
    };
    stage_transition(tx, provider, record, next, first_admission)
}

impl Execute for MutateSorafsStreamTokenAuthority {
    fn execute(
        self,
        authority: &AccountId,
        tx: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let exact_execution = execution(tx, authority).map_err(rejected)?;
        if !authorized(
            tx,
            authority,
            self.request.provider_id,
            &self.request.action,
        ) {
            return Err(rejected(Error::BindingMismatch));
        }
        apply(self, authority, tx, exact_execution).map_err(rejected)
    }
}

#[cfg(test)]
#[path = "sorafs_stream_token_authority/tests.rs"]
mod tests;

mod check;
