//! Shared no-write account-custody eligibility at execution and the authenticated applied cut.
use super::*;
use crate::state::WorldReadOnly;
use iroha_data_model::{
    account::AccountId,
    isi::sorafs::MutateSorafsFinalPromotionAccountCustody,
    permission::Permission,
    sorafs::final_promotion_account_custody::{
        FinalPromotionAccountCustodyActionV1 as Action, FinalPromotionAccountCustodyCheckV1,
    },
};
use iroha_executor_data_model::permission::sorafs::{
    CanCheckSorafsFinalPromotionAccountCustody, CanManageSorafsFinalPromotionAccountCustody,
    CanOperateSorafsFinalPromotion,
};
use mv::storage::StorageReadOnly;
use sorafs_manifest::signer::custody::{SignerCustodyUseContextV1, verify_signer_custody_use_v1};

fn has_permission(world: &impl WorldReadOnly, account: &AccountId, permission: Permission) -> bool {
    world.accounts().get(account).is_some()
        && (world.account_contains_inherent_permission(account, &permission)
            || world
                .account_roles_iter(account)
                .filter_map(|id| world.roles().get(id))
                .any(|role| role.permissions().any(|token| token == &permission)))
}

pub(crate) fn authorized(
    world: &impl WorldReadOnly,
    authority: &AccountId,
    deployment: &str,
    action: &Action,
) -> bool {
    let permission: Permission = match action {
        Action::Configure(_) | Action::Enroll(_) | Action::Revoke(_) => {
            CanManageSorafsFinalPromotionAccountCustody {
                deployment_id: deployment.to_owned(),
            }
            .into()
        }
        Action::Check(check) => {
            if authority == &check.expected_account {
                return false;
            }
            CanCheckSorafsFinalPromotionAccountCustody {
                deployment_id: deployment.to_owned(),
            }
            .into()
        }
    };
    has_permission(world, authority, permission)
}

pub(crate) fn check_floor(
    state: &impl StateReadOnly,
    check: &FinalPromotionAccountCustodyCheckV1,
    execution_height: u64,
) -> Result<(), HistoryError> {
    if check.challenge == [0; 32]
        || check.network_id == [0; 32]
        || check.minimum_block_hash == [0; 32]
        || check.transaction_payload_digest == [0; 32]
        || check.network_id != *state.network_id().as_bytes()
        || check.minimum_height >= execution_height
    {
        return Err(HistoryError::BindingMismatch);
    }
    let offset = check
        .minimum_height
        .checked_sub(1)
        .and_then(|height| usize::try_from(height).ok())
        .ok_or(HistoryError::HeightUnavailable)?;
    if state.block_hashes().get(offset).map(|hash| *hash.as_ref()) != Some(check.minimum_block_hash)
    {
        return Err(HistoryError::HeightUnavailable);
    }
    Ok(())
}

pub(crate) fn accounts_eligible(
    world: &impl WorldReadOnly,
    instruction: &MutateSorafsFinalPromotionAccountCustody,
    binding: &SignerCustodyBindingV1,
    observer: &AccountId,
) -> Result<(), HistoryError> {
    let Action::Check(check) = &instruction.action else {
        return Err(HistoryError::Invalid);
    };
    let target = AccountId::new(binding.public_key.clone());
    if observer == &target
        || check.expected_account != target
        || !authorized(
            world,
            observer,
            &instruction.deployment_id,
            &instruction.action,
        )
        || !has_permission(
            world,
            &target,
            CanOperateSorafsFinalPromotion {
                deployment_id: instruction.deployment_id.clone(),
            }
            .into(),
        )
    {
        return Err(HistoryError::BindingMismatch);
    }
    Ok(())
}

/// Recheck both registered accounts and exact live permissions from one captured applied State.
pub(crate) fn check_applied_snapshot_v1(
    view: &impl StateReadOnly,
    instruction: &MutateSorafsFinalPromotionAccountCustody,
    binding: &SignerCustodyBindingV1,
    observer: &AccountId,
    now: u64,
) -> Result<FinalPromotionAccountCustodySnapshotV1, HistoryError> {
    history::encode(instruction)?;
    let Action::Check(check) = &instruction.action else {
        return Err(HistoryError::Invalid);
    };
    history::validate_binding::<AccountPurpose>(view, binding, &instruction.deployment_id)?;
    let height =
        u64::try_from(view.block_hashes().len()).map_err(|_| HistoryError::HeightUnavailable)?;
    check_floor(view, check, height)?;
    accounts_eligible(view.world(), instruction, binding, observer)?;
    let snapshot = super::read_at(view, binding, height)?.ok_or(HistoryError::Conflict)?;
    check_snapshot_eligibility_v1(&snapshot, instruction, binding, observer, now, now)?;
    Ok(snapshot)
}

/// Evaluate current time against the same previously authenticated native snapshot and accounts.
/// This does not qualify an arbitrary snapshot, its permissions, clock or finality.
/// Preserve the independently observed earliest time when reusing a captured snapshot.
pub(crate) fn check_snapshot_eligibility_v1(
    snapshot: &FinalPromotionAccountCustodySnapshotV1,
    instruction: &MutateSorafsFinalPromotionAccountCustody,
    binding: &SignerCustodyBindingV1,
    observer: &AccountId,
    now: u64,
    anchor_observed_at_unix_ms: u64,
) -> Result<(), HistoryError> {
    let Action::Check(check) = &instruction.action else {
        return Err(HistoryError::Invalid);
    };
    let target = AccountId::new(snapshot.control.policy.binding.public_key.clone());
    if snapshot.control.policy.binding != *binding
        || observer == &target
        || check.expected_account != target
        || now == 0
        || now == u64::MAX
        || snapshot.control_record.deployment_id != instruction.deployment_id
        || snapshot.control_record.revision != instruction.expected_control_revision
        || snapshot.custody_anchor.state_digest != instruction.expected_control_digest
    {
        return Err(HistoryError::Conflict);
    }
    // Issuance can precede native admission. Neither endpoint may move custody use
    // before the exact retained transition's execution, even if the attestation is valid.
    if now < snapshot.control_record.execution.recorded_at_unix_ms {
        return Err(HistoryError::Custody);
    }
    verify_signer_custody_use_v1(
        snapshot
            .control_record
            .enrollment
            .as_deref()
            .ok_or(HistoryError::Custody)?,
        binding,
        &snapshot.control.policy.custody_trust(),
        &SignerCustodyUseContextV1 {
            now_unix_ms: now,
            anchor_observed_at_unix_ms,
            current_anchor: snapshot.custody_anchor,
            active_head: snapshot.control.active_head.ok_or(HistoryError::Custody)?,
            signer_revoked: snapshot.control.signer_revoked,
            attester_revoked: snapshot.control.attester_revoked,
        },
    )
    .map_err(|_| HistoryError::Custody)?;
    Ok(())
}
