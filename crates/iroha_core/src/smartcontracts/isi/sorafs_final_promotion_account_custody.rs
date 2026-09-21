//! Native account-custody transitions and a no-write independent-observer Check.
use super::Execute;
use crate::{
    query::{
        final_promotion_account_custody::{
            FinalPromotionAccountCustodyErrorV1, FinalPromotionAccountCustodySnapshotV1, check,
        },
        signer_custody_history::{
            self as history, AccountPurpose, ControlAction, ControlTransition, HistoryError,
            NativeControl, encode,
        },
    },
    state::StateTransaction,
};
use iroha_crypto::Hash;
use iroha_data_model::{
    account::AccountId,
    isi::{
        error::{InstructionExecutionError, InvalidParameterError},
        sorafs::MutateSorafsFinalPromotionAccountCustody,
    },
    sorafs::final_promotion_account_custody::FinalPromotionAccountCustodyActionV1 as Action,
};

fn rejected(error: HistoryError) -> InstructionExecutionError {
    InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
        FinalPromotionAccountCustodyErrorV1::from(error).to_string(),
    ))
}

fn request_digest(
    instruction: &MutateSorafsFinalPromotionAccountCustody,
    authority: &AccountId,
) -> Result<[u8; 32], HistoryError> {
    let mut bytes = b"iroha.sorafs.final-promotion-account.custody-request.v1\0".to_vec();
    bytes.extend_from_slice(&encode(instruction)?);
    bytes.extend_from_slice(&encode(authority)?);
    Ok(*Hash::new(bytes).as_ref())
}

fn execute_check(
    instruction: &MutateSorafsFinalPromotionAccountCustody,
    authority: &AccountId,
    tx: &StateTransaction<'_, '_>,
    current: &NativeControl<AccountPurpose>,
) -> Result<(), HistoryError> {
    let Action::Check(request) = &instruction.action else {
        return Err(HistoryError::Invalid);
    };
    let height = u64::try_from(tx.block_hashes().len())
        .map_err(|_| HistoryError::HeightUnavailable)?
        .checked_add(1)
        .ok_or(HistoryError::HeightUnavailable)?;
    if height != tx._curr_block.height().get() {
        return Err(HistoryError::HeightUnavailable);
    }
    check::check_floor(tx, request, height)?;
    check::accounts_eligible(
        tx.world(),
        instruction,
        &current.state.policy.binding,
        authority,
    )?;
    let anchor = history::committed_control::<AccountPurpose>(tx, current)?;
    let snapshot = FinalPromotionAccountCustodySnapshotV1 {
        control_record: current.record.clone(),
        control: current.state.clone(),
        custody_anchor: anchor,
    };
    check::check_snapshot_eligibility_v1(
        &snapshot,
        instruction,
        &current.state.policy.binding,
        authority,
        tx.block_unix_timestamp_ms(),
        tx.block_unix_timestamp_ms(),
    )
}

fn apply(
    instruction: MutateSorafsFinalPromotionAccountCustody,
    authority: &AccountId,
    tx: &mut StateTransaction<'_, '_>,
) -> Result<(), HistoryError> {
    // Bound every action before history reads; Check deliberately has no mutation digest.
    encode(&instruction)?;
    let current = history::read_control::<AccountPurpose>(tx.world(), &instruction.deployment_id)?;
    if current.as_ref().map_or(0, |row| row.index.revision) != instruction.expected_control_revision
        || current.as_ref().map_or([0; 32], |row| row.index.digest)
            != instruction.expected_control_digest
    {
        return Err(HistoryError::Conflict);
    }
    if let Some(current) = &current {
        history::validate_binding::<AccountPurpose>(
            tx,
            &current.state.policy.binding,
            &instruction.deployment_id,
        )?;
    }
    if matches!(&instruction.action, Action::Check(_)) {
        return execute_check(
            &instruction,
            authority,
            tx,
            current.as_ref().ok_or(HistoryError::Conflict)?,
        );
    }
    let action = match &instruction.action {
        Action::Configure(bytes) => ControlAction::Configure(bytes),
        Action::Enroll(bytes) => ControlAction::Enroll(bytes),
        Action::Revoke(value) => ControlAction::Revoke {
            signer: value.signer,
            attester: value.attester,
        },
        Action::Check(_) => return Err(HistoryError::Invalid),
    };
    let prepared = history::prepare_control::<AccountPurpose>(
        tx,
        authority,
        current.as_ref(),
        ControlTransition {
            deployment: &instruction.deployment_id,
            expected_revision: instruction.expected_control_revision,
            expected_digest: instruction.expected_control_digest,
            request_digest: request_digest(&instruction, authority)?,
            action,
        },
    )?;
    // Shared history stages every immutable row/index before any write becomes visible.
    for (key, bytes) in prepared {
        tx.world.smart_contract_state.insert(key, bytes);
    }
    Ok(())
}
impl Execute for MutateSorafsFinalPromotionAccountCustody {
    fn execute(
        self,
        authority: &AccountId,
        tx: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        if !history::valid_deployment::<AccountPurpose>(&self.deployment_id)
            || !check::authorized(tx.world(), authority, &self.deployment_id, &self.action)
        {
            return Err(rejected(HistoryError::BindingMismatch));
        }
        apply(self, authority, tx).map_err(rejected)
    }
}

#[cfg(test)]
mod tests;
