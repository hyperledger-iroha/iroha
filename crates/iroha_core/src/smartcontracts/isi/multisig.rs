//! Built-in handling for multisig instructions without requiring an executor upgrade.

use std::{
    collections::{BTreeMap, BTreeSet},
    str::FromStr,
};

use iroha_crypto::HashOf;
use iroha_data_model::{
    ValidationFail,
    account::{AccountId, MultisigMember, MultisigPolicy},
    isi::{
        AddSignatory, CustomInstruction, InstructionBox, RemoveSignatory, SetAccountQuorum,
        error::{InstructionExecutionError, InvalidParameterError},
    },
    metadata::Metadata,
    name::Name,
    prelude::{Grant, Json, Level, Log, Register, RemoveKeyValue, Revoke, SetKeyValue},
    query::error::{FindError, QueryExecutionFail},
    role::{Role, RoleId},
};
use iroha_executor_data_model::isi::multisig::{
    MultisigApprove, MultisigInstructionBox, MultisigProposalValue, MultisigPropose,
    MultisigRegister, MultisigSpec,
};
use mv::storage::StorageReadOnly;

use crate::{
    smartcontracts::Execute,
    smartcontracts::isi::domain::isi::ensure_controller_capabilities,
    state::{StateTransaction, WorldReadOnly},
};

const DELIMITER: char = '/';
const MULTISIG: &str = "multisig";
const MULTISIG_SIGNATORY: &str = "MULTISIG_SIGNATORY";

/// Execute a multisig instruction directly in the initial executor.
///
/// # Errors
///
/// Propagates [`ValidationFail`] when validation or execution of the instruction fails.
pub fn execute_multisig_instruction(
    state_transaction: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    instruction: MultisigInstructionBox,
) -> Result<(), ValidationFail> {
    match instruction {
        MultisigInstructionBox::Register(instruction) => {
            execute_register(state_transaction, authority, instruction)
        }
        MultisigInstructionBox::Propose(instruction) => {
            execute_propose(state_transaction, authority, &instruction)
        }
        MultisigInstructionBox::Approve(instruction) => {
            execute_approve(state_transaction, authority, &instruction)
        }
    }
}

impl Execute for AddSignatory {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let AddSignatory { account, signatory } = self;
        let signatory_account = AccountId::new(account.domain().clone(), signatory);
        let mut spec = multisig_spec_strict(state_transaction, &account)?;
        if spec.signatories.contains_key(&signatory_account) {
            return Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(format!(
                    "signatory `{signatory_account}` already present in multisig spec for `{account}`"
                )),
            ));
        }
        spec.signatories.insert(signatory_account.clone(), 1);
        validate_registration(state_transaction, &account, &spec).map_err(map_validation_fail)?;
        SetKeyValue::account(account.clone(), spec_key(), Json::new(spec.clone()))
            .execute(authority, state_transaction)?;
        let updated_account = rekey_multisig_account(state_transaction, &account, &spec)?;
        let domain_owner =
            domain_owner(state_transaction, account.domain()).map_err(map_validation_fail)?;
        configure_roles(state_transaction, &domain_owner, &updated_account, &spec)
            .map_err(map_validation_fail)?;
        Ok(())
    }
}

impl Execute for RemoveSignatory {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let RemoveSignatory { account, signatory } = self;
        let signatory_account = AccountId::new(account.domain().clone(), signatory);
        let mut spec = multisig_spec_strict(state_transaction, &account)?;
        if spec.signatories.remove(&signatory_account).is_none() {
            return Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(format!(
                    "signatory `{signatory_account}` not present in multisig spec for `{account}`"
                )),
            ));
        }
        let total_weight: u32 = spec
            .signatories
            .values()
            .map(|weight| u32::from(*weight))
            .sum();
        let quorum = u32::from(spec.quorum.get());
        if total_weight > 0 && total_weight < quorum {
            // Keep the quorum reachable after removing a signatory.
            let adjusted = u16::try_from(total_weight).map_err(|_| {
                InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                    format!("multisig total weight {total_weight} exceeds u16"),
                ))
            })?;
            spec.quorum = std::num::NonZeroU16::new(adjusted)
                .expect("total_weight > 0 implies nonzero quorum");
        }
        validate_registration(state_transaction, &account, &spec).map_err(map_validation_fail)?;
        SetKeyValue::account(account.clone(), spec_key(), Json::new(spec.clone()))
            .execute(authority, state_transaction)?;
        let updated_account = rekey_multisig_account(state_transaction, &account, &spec)?;

        let multisig_role_id = multisig_role_for(&updated_account);
        if has_role(state_transaction, &signatory_account, &multisig_role_id)
            .map_err(map_validation_fail)?
        {
            Revoke::account_role(multisig_role_id.clone(), signatory_account.clone())
                .execute(authority, state_transaction)?;
        }
        let signatory_role_id = multisig_role_for(&signatory_account);
        if has_role(state_transaction, &updated_account, &signatory_role_id)
            .map_err(map_validation_fail)?
        {
            Revoke::account_role(signatory_role_id, updated_account.clone())
                .execute(authority, state_transaction)?;
        }

        Ok(())
    }
}

impl Execute for SetAccountQuorum {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let SetAccountQuorum { account, quorum } = self;
        let mut spec = multisig_spec_strict(state_transaction, &account)?;
        spec.quorum = quorum;
        validate_registration(state_transaction, &account, &spec).map_err(map_validation_fail)?;
        SetKeyValue::account(account.clone(), spec_key(), Json::new(spec.clone()))
            .execute(authority, state_transaction)?;
        let _ = rekey_multisig_account(state_transaction, &account, &spec)?;
        Ok(())
    }
}

pub(crate) fn spec_key() -> Name {
    Name::from_str(&format!("{MULTISIG}{DELIMITER}spec"))
        .expect("constant string must be a valid name")
}

fn proposal_key(hash: &HashOf<Vec<InstructionBox>>) -> Name {
    Name::from_str(&format!("{MULTISIG}{DELIMITER}proposals{DELIMITER}{hash}"))
        .expect("constant string must be a valid name")
}

fn multisig_role_for(account: &AccountId) -> RoleId {
    let suffix = account
        .canonical_ih58()
        .unwrap_or_else(|_| HashOf::new(account).to_string());
    format!(
        "{MULTISIG_SIGNATORY}{DELIMITER}{}{DELIMITER}{}",
        account.domain(),
        suffix,
    )
    .parse()
    .expect("multisig role name must be valid")
}

fn rekey_multisig_account(
    state_transaction: &mut StateTransaction<'_, '_>,
    account: &AccountId,
    spec: &MultisigSpec,
) -> Result<AccountId, InstructionExecutionError> {
    ensure_signatories_are_single(spec).map_err(map_validation_fail)?;
    let policy = multisig_policy_from_spec(spec)?;
    let updated_account = AccountId::new_multisig(account.domain().clone(), policy);
    ensure_controller_capabilities(
        updated_account.controller(),
        &state_transaction.crypto.allowed_signing,
        &state_transaction.crypto.allowed_curve_ids,
    )?;

    if &updated_account == account {
        return Ok(account.clone());
    }

    if account_exists(state_transaction, &updated_account).map_err(map_validation_fail)? {
        return Err(InstructionExecutionError::InvariantViolation(
            format!("multisig account `{updated_account}` already exists").into(),
        ));
    }

    rekey_account_id(state_transaction, account, &updated_account)?;
    Ok(updated_account)
}

fn multisig_policy_from_spec(
    spec: &MultisigSpec,
) -> Result<MultisigPolicy, InstructionExecutionError> {
    let mut members = Vec::with_capacity(spec.signatories.len());
    for (account, weight) in &spec.signatories {
        let Some(signatory) = account.controller().single_signatory() else {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("multisig signatory `{account}` must be a single-key account").into(),
            ));
        };
        let member = MultisigMember::new(signatory.clone(), u16::from(*weight)).map_err(|err| {
            InstructionExecutionError::InvariantViolation(format!("{err}").into())
        })?;
        members.push(member);
    }
    MultisigPolicy::new(spec.quorum.get(), members)
        .map_err(|err| InstructionExecutionError::InvariantViolation(format!("{err}").into()))
}

fn rekey_account_id(
    state_transaction: &mut StateTransaction<'_, '_>,
    old_account: &AccountId,
    new_account: &AccountId,
) -> Result<(), InstructionExecutionError> {
    if state_transaction.world.accounts.get(new_account).is_some() {
        return Err(InstructionExecutionError::InvariantViolation(
            format!("account `{new_account}` already exists").into(),
        ));
    }

    let account_value = state_transaction
        .world
        .remove_account_with_links(old_account)
        .ok_or_else(|| InstructionExecutionError::Find(FindError::Account(old_account.clone())))?;

    state_transaction
        .world
        .insert_account_with_links(new_account.clone(), account_value.clone());

    if let Some(label) = account_value.label().cloned() {
        state_transaction
            .world
            .account_aliases
            .insert(label.clone(), new_account.clone());
        state_transaction.world.account_rekey_records.remove(label);
    }

    if let Some(uaid) = account_value.uaid().copied() {
        state_transaction
            .world
            .uaid_accounts
            .insert(uaid, new_account.clone());
        state_transaction.rebuild_space_directory_bindings(uaid);
    }

    if let Some(sequence) = state_transaction
        .world
        .tx_sequences
        .remove(old_account.clone())
    {
        state_transaction
            .world
            .tx_sequences
            .insert(new_account.clone(), sequence);
    }

    if let Some(perms) = state_transaction
        .world
        .account_permissions
        .remove(old_account.clone())
    {
        state_transaction
            .world
            .account_permissions
            .insert(new_account.clone(), perms);
    }

    let old_multisig_role = multisig_role_for(old_account);
    let new_multisig_role = multisig_role_for(new_account);
    if old_multisig_role != new_multisig_role
        && state_transaction
            .world
            .roles
            .get(&new_multisig_role)
            .is_some()
    {
        return Err(InstructionExecutionError::InvariantViolation(
            format!("role `{new_multisig_role}` already exists").into(),
        ));
    }

    if old_multisig_role != new_multisig_role {
        if let Some(mut role) = state_transaction
            .world
            .roles
            .remove(old_multisig_role.clone())
        {
            role.id = new_multisig_role.clone();
            state_transaction
                .world
                .roles
                .insert(new_multisig_role.clone(), role);
        }
    }

    let mut role_updates = Vec::new();
    for (role_id, _) in state_transaction.world.account_roles.iter() {
        let mut updated = role_id.clone();
        if updated.account == *old_account {
            updated.account = new_account.clone();
        }
        if updated.id == old_multisig_role {
            updated.id = new_multisig_role.clone();
        }
        if &updated != role_id {
            role_updates.push((role_id.clone(), updated));
        }
    }
    for (old_key, new_key) in role_updates {
        state_transaction.world.account_roles.remove(old_key);
        state_transaction.world.account_roles.insert(new_key, ());
    }

    let assets_to_move: Vec<_> = state_transaction
        .world
        .assets_in_account_iter(old_account)
        .map(|asset| asset.id().clone())
        .collect();
    for asset_id in assets_to_move {
        let new_asset_id = iroha_data_model::asset::AssetId::new(
            asset_id.definition().clone(),
            new_account.clone(),
        );
        if state_transaction.world.assets.get(&new_asset_id).is_some() {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("asset `{new_asset_id}` already exists").into(),
            ));
        }
        if let Some(value) = state_transaction.world.assets.remove(asset_id.clone()) {
            state_transaction
                .world
                .assets
                .insert(new_asset_id.clone(), value);
        }
        if let Some(meta) = state_transaction
            .world
            .asset_metadata
            .remove(asset_id.clone())
        {
            state_transaction
                .world
                .asset_metadata
                .insert(new_asset_id, meta);
        }
    }

    let nft_ids: Vec<_> = state_transaction
        .world
        .nfts
        .iter()
        .filter(|(_, value)| value.owned_by == *old_account)
        .map(|(id, _)| id.clone())
        .collect();
    for nft_id in nft_ids {
        if let Some(value) = state_transaction.world.nfts.get_mut(&nft_id) {
            value.owned_by = new_account.clone();
        }
    }

    let domain_ids: Vec<_> = state_transaction
        .world
        .domains
        .iter()
        .filter(|(_, domain)| domain.owned_by == *old_account)
        .map(|(id, _)| id.clone())
        .collect();
    for domain_id in domain_ids {
        if let Some(domain) = state_transaction.world.domains.get_mut(&domain_id) {
            domain.owned_by = new_account.clone();
        }
    }

    let asset_def_ids: Vec<_> = state_transaction
        .world
        .asset_definitions
        .iter()
        .filter(|(_, definition)| definition.owned_by == *old_account)
        .map(|(id, _)| id.clone())
        .collect();
    for asset_def_id in asset_def_ids {
        if let Some(definition) = state_transaction
            .world
            .asset_definitions
            .get_mut(&asset_def_id)
        {
            definition.owned_by = new_account.clone();
        }
    }

    let provider_ids: Vec<_> = state_transaction
        .world
        .provider_owners
        .iter()
        .filter(|(_, owner)| *owner == old_account)
        .map(|(id, _)| id.clone())
        .collect();
    for provider_id in provider_ids {
        state_transaction
            .world
            .provider_owners
            .insert(provider_id, new_account.clone());
    }

    replace_account_id_in_offline(state_transaction, old_account, new_account);
    replace_account_id_in_public_lane(state_transaction, old_account, new_account);
    replace_account_id_in_repo_agreements(state_transaction, old_account, new_account);
    replace_account_id_in_settlements(state_transaction, old_account, new_account);
    replace_account_id_in_citizens(state_transaction, old_account, new_account);
    replace_account_id_in_governance(state_transaction, old_account, new_account);
    replace_account_id_in_oracle(state_transaction, old_account, new_account);
    replace_account_id_in_content_bundles(state_transaction, old_account, new_account);

    state_transaction
        .world
        .triggers
        .replace_account_id(old_account, new_account);

    state_transaction.invalidate_permission_cache_for_account(old_account);
    state_transaction.invalidate_permission_cache_for_account(new_account);

    Ok(())
}

fn replace_account_id(target: &mut AccountId, old: &AccountId, new: &AccountId) -> bool {
    if target == old {
        *target = new.clone();
        true
    } else {
        false
    }
}

fn replace_account_id_in_asset_id(
    asset_id: &iroha_data_model::asset::AssetId,
    old: &AccountId,
    new: &AccountId,
) -> iroha_data_model::asset::AssetId {
    if asset_id.account() == old {
        iroha_data_model::asset::AssetId::new(asset_id.definition().clone(), new.clone())
    } else {
        asset_id.clone()
    }
}

fn replace_account_id_in_vec(accounts: &mut Vec<AccountId>, old: &AccountId, new: &AccountId) {
    for account in accounts.iter_mut() {
        replace_account_id(account, old, new);
    }
}

fn replace_account_id_in_set(accounts: &mut BTreeSet<AccountId>, old: &AccountId, new: &AccountId) {
    if accounts.remove(old) {
        accounts.insert(new.clone());
    }
}

fn replace_account_id_in_offline(
    state_transaction: &mut StateTransaction<'_, '_>,
    old: &AccountId,
    new: &AccountId,
) {
    let allowance_ids: Vec<_> = state_transaction
        .world
        .offline_allowances
        .iter()
        .map(|(id, _)| *id)
        .collect();
    for allowance_id in allowance_ids {
        if let Some(record) = state_transaction
            .world
            .offline_allowances
            .get_mut(&allowance_id)
        {
            replace_account_id_in_offline_allowance(record, old, new);
        }
    }

    let transfer_ids: Vec<_> = state_transaction
        .world
        .offline_to_online_transfers
        .iter()
        .map(|(id, _)| *id)
        .collect();
    for transfer_id in transfer_ids {
        if let Some(record) = state_transaction
            .world
            .offline_to_online_transfers
            .get_mut(&transfer_id)
        {
            replace_account_id_in_offline_transfer_record(record, old, new);
        }
    }

    let revocation_ids: Vec<_> = state_transaction
        .world
        .offline_verdict_revocations
        .iter()
        .map(|(id, _)| *id)
        .collect();
    for revocation_id in revocation_ids {
        if let Some(record) = state_transaction
            .world
            .offline_verdict_revocations
            .get_mut(&revocation_id)
        {
            replace_account_id(&mut record.issuer, old, new);
        }
    }

    if let Some(mut transfers) = state_transaction
        .world
        .offline_transfer_sender_index
        .remove(old.clone())
    {
        if let Some(existing) = state_transaction
            .world
            .offline_transfer_sender_index
            .get_mut(new)
        {
            existing.append(&mut transfers);
        } else {
            state_transaction
                .world
                .offline_transfer_sender_index
                .insert(new.clone(), transfers);
        }
    }

    if let Some(mut transfers) = state_transaction
        .world
        .offline_transfer_receiver_index
        .remove(old.clone())
    {
        if let Some(existing) = state_transaction
            .world
            .offline_transfer_receiver_index
            .get_mut(new)
        {
            existing.append(&mut transfers);
        } else {
            state_transaction
                .world
                .offline_transfer_receiver_index
                .insert(new.clone(), transfers);
        }
    }
}

fn replace_account_id_in_offline_allowance(
    record: &mut iroha_data_model::offline::OfflineAllowanceRecord,
    old: &AccountId,
    new: &AccountId,
) {
    replace_account_id_in_offline_wallet_certificate(&mut record.certificate, old, new);
}

fn replace_account_id_in_offline_wallet_certificate(
    certificate: &mut iroha_data_model::offline::OfflineWalletCertificate,
    old: &AccountId,
    new: &AccountId,
) {
    replace_account_id(&mut certificate.controller, old, new);
    replace_account_id(&mut certificate.operator, old, new);
    certificate.allowance.asset =
        replace_account_id_in_asset_id(&certificate.allowance.asset, old, new);
}

fn replace_account_id_in_offline_spend_receipt(
    receipt: &mut iroha_data_model::offline::OfflineSpendReceipt,
    old: &AccountId,
    new: &AccountId,
) {
    replace_account_id(&mut receipt.from, old, new);
    replace_account_id(&mut receipt.to, old, new);
    receipt.asset = replace_account_id_in_asset_id(&receipt.asset, old, new);
}

fn replace_account_id_in_offline_transfer(
    transfer: &mut iroha_data_model::offline::OfflineToOnlineTransfer,
    old: &AccountId,
    new: &AccountId,
) {
    replace_account_id(&mut transfer.receiver, old, new);
    replace_account_id(&mut transfer.deposit_account, old, new);
    for receipt in &mut transfer.receipts {
        replace_account_id_in_offline_spend_receipt(receipt, old, new);
    }
}

fn replace_account_id_in_offline_transfer_record(
    record: &mut iroha_data_model::offline::OfflineTransferRecord,
    old: &AccountId,
    new: &AccountId,
) {
    replace_account_id(&mut record.controller, old, new);
    replace_account_id_in_offline_transfer(&mut record.transfer, old, new);
}

fn replace_account_id_in_public_lane(
    state_transaction: &mut StateTransaction<'_, '_>,
    old: &AccountId,
    new: &AccountId,
) {
    let mut validator_updates = Vec::new();
    for (key, _) in state_transaction.world.public_lane_validators.iter() {
        if key.1 == *old {
            validator_updates.push((key.clone(), (key.0, new.clone())));
        }
    }
    for (old_key, new_key) in validator_updates {
        if let Some(mut record) = state_transaction
            .world
            .public_lane_validators
            .remove(old_key)
        {
            replace_account_id(&mut record.validator, old, new);
            replace_account_id(&mut record.stake_account, old, new);
            state_transaction
                .world
                .public_lane_validators
                .insert(new_key, record);
        }
    }

    let validator_keys: Vec<_> = state_transaction
        .world
        .public_lane_validators
        .iter()
        .map(|(key, _)| key.clone())
        .collect();
    for key in validator_keys {
        if let Some(record) = state_transaction.world.public_lane_validators.get_mut(&key) {
            replace_account_id(&mut record.validator, old, new);
            replace_account_id(&mut record.stake_account, old, new);
        }
    }

    let mut stake_updates = Vec::new();
    for (key, _) in state_transaction.world.public_lane_stake_shares.iter() {
        if key.1 == *old || key.2 == *old {
            let new_validator = if key.1 == *old {
                new.clone()
            } else {
                key.1.clone()
            };
            let new_staker = if key.2 == *old {
                new.clone()
            } else {
                key.2.clone()
            };
            stake_updates.push((key.clone(), (key.0, new_validator, new_staker)));
        }
    }
    for (old_key, new_key) in stake_updates {
        if let Some(mut record) = state_transaction
            .world
            .public_lane_stake_shares
            .remove(old_key)
        {
            replace_account_id(&mut record.validator, old, new);
            replace_account_id(&mut record.staker, old, new);
            state_transaction
                .world
                .public_lane_stake_shares
                .insert(new_key, record);
        }
    }

    let reward_keys: Vec<_> = state_transaction
        .world
        .public_lane_rewards
        .iter()
        .map(|(key, _)| key.clone())
        .collect();
    for key in reward_keys {
        if let Some(record) = state_transaction.world.public_lane_rewards.get_mut(&key) {
            record.asset = replace_account_id_in_asset_id(&record.asset, old, new);
            for share in &mut record.shares {
                replace_account_id(&mut share.account, old, new);
            }
        }
    }

    let mut claim_updates = Vec::new();
    for (key, value) in state_transaction.world.public_lane_reward_claims.iter() {
        let (lane_id, account_id, asset_id) = key;
        let mut updated = false;
        let updated_account = if account_id == old {
            updated = true;
            new.clone()
        } else {
            account_id.clone()
        };
        let updated_asset = replace_account_id_in_asset_id(asset_id, old, new);
        if &updated_asset != asset_id {
            updated = true;
        }
        if updated {
            claim_updates.push((
                key.clone(),
                (lane_id.clone(), updated_account, updated_asset),
                *value,
            ));
        }
    }
    for (old_key, new_key, value) in claim_updates {
        state_transaction
            .world
            .public_lane_reward_claims
            .remove(old_key);
        state_transaction
            .world
            .public_lane_reward_claims
            .insert(new_key, value);
    }
}

fn replace_account_id_in_repo_agreements(
    state_transaction: &mut StateTransaction<'_, '_>,
    old: &AccountId,
    new: &AccountId,
) {
    let agreement_ids: Vec<_> = state_transaction
        .world
        .repo_agreements
        .iter()
        .map(|(id, _)| id.clone())
        .collect();
    for agreement_id in agreement_ids {
        if let Some(agreement) = state_transaction
            .world
            .repo_agreements
            .get_mut(&agreement_id)
        {
            replace_account_id(&mut agreement.initiator, old, new);
            replace_account_id(&mut agreement.counterparty, old, new);
            if let Some(custodian) = agreement.custodian.as_mut() {
                replace_account_id(custodian, old, new);
            }
        }
    }
}

fn replace_account_id_in_settlements(
    state_transaction: &mut StateTransaction<'_, '_>,
    old: &AccountId,
    new: &AccountId,
) {
    let ledger_ids: Vec<_> = state_transaction
        .world
        .settlement_ledgers
        .iter()
        .map(|(id, _)| id.clone())
        .collect();
    for ledger_id in ledger_ids {
        if let Some(ledger) = state_transaction
            .world
            .settlement_ledgers
            .get_mut(&ledger_id)
        {
            for entry in &mut ledger.entries {
                replace_account_id(&mut entry.authority, old, new);
                for leg in &mut entry.legs {
                    replace_account_id(&mut leg.leg.from, old, new);
                    replace_account_id(&mut leg.leg.to, old, new);
                }
            }
        }
    }
}

fn replace_account_id_in_citizens(
    state_transaction: &mut StateTransaction<'_, '_>,
    old: &AccountId,
    new: &AccountId,
) {
    if let Some(mut record) = state_transaction.world.citizens.remove(old.clone()) {
        replace_account_id(&mut record.owner, old, new);
        state_transaction.world.citizens.insert(new.clone(), record);
    }

    let citizen_ids: Vec<_> = state_transaction
        .world
        .citizens
        .iter()
        .map(|(id, _)| id.clone())
        .collect();
    for citizen_id in citizen_ids {
        if let Some(record) = state_transaction.world.citizens.get_mut(&citizen_id) {
            replace_account_id(&mut record.owner, old, new);
        }
    }
}

fn replace_account_id_in_governance(
    state_transaction: &mut StateTransaction<'_, '_>,
    old: &AccountId,
    new: &AccountId,
) {
    let proposal_ids: Vec<_> = state_transaction
        .world
        .governance_proposals
        .iter()
        .map(|(id, _)| *id)
        .collect();
    for proposal_id in proposal_ids {
        if let Some(record) = state_transaction
            .world
            .governance_proposals
            .get_mut(&proposal_id)
        {
            replace_account_id(&mut record.proposer, old, new);
        }
    }

    let approval_ids: Vec<_> = state_transaction
        .world
        .governance_stage_approvals
        .iter()
        .map(|(id, _)| id.clone())
        .collect();
    for approval_id in approval_ids {
        if let Some(approvals) = state_transaction
            .world
            .governance_stage_approvals
            .get_mut(&approval_id)
        {
            for stage in approvals.stages.values_mut() {
                replace_account_id_in_set(&mut stage.approvers, old, new);
            }
        }
    }

    let lock_ids: Vec<_> = state_transaction
        .world
        .governance_locks
        .iter()
        .map(|(id, _)| id.clone())
        .collect();
    for lock_id in lock_ids {
        if let Some(locks) = state_transaction.world.governance_locks.get_mut(&lock_id) {
            let mut updated = BTreeMap::new();
            for (account, mut record) in std::mem::take(&mut locks.locks) {
                let key = if account == *old {
                    new.clone()
                } else {
                    account
                };
                replace_account_id(&mut record.owner, old, new);
                updated.insert(key, record);
            }
            locks.locks = updated;
        }
    }

    let slash_ids: Vec<_> = state_transaction
        .world
        .governance_slashes
        .iter()
        .map(|(id, _)| id.clone())
        .collect();
    for slash_id in slash_ids {
        if let Some(slashes) = state_transaction
            .world
            .governance_slashes
            .get_mut(&slash_id)
        {
            let mut updated = BTreeMap::new();
            for (account, record) in std::mem::take(&mut slashes.slashes) {
                let key = if account == *old {
                    new.clone()
                } else {
                    account
                };
                updated.insert(key, record);
            }
            slashes.slashes = updated;
        }
    }

    let council_epochs: Vec<_> = state_transaction
        .world
        .council
        .iter()
        .map(|(epoch, _)| *epoch)
        .collect();
    for epoch in council_epochs {
        if let Some(term) = state_transaction.world.council.get_mut(&epoch) {
            replace_account_id_in_vec(&mut term.members, old, new);
            replace_account_id_in_vec(&mut term.alternates, old, new);
        }
    }

    let body_epochs: Vec<_> = state_transaction
        .world
        .parliament_bodies
        .iter()
        .map(|(epoch, _)| *epoch)
        .collect();
    for epoch in body_epochs {
        if let Some(bodies) = state_transaction.world.parliament_bodies.get_mut(&epoch) {
            for roster in bodies.rosters.values_mut() {
                replace_account_id_in_vec(&mut roster.members, old, new);
                replace_account_id_in_vec(&mut roster.alternates, old, new);
            }
        }
    }
}

fn replace_account_id_in_oracle(
    state_transaction: &mut StateTransaction<'_, '_>,
    old: &AccountId,
    new: &AccountId,
) {
    let feed_ids: Vec<_> = state_transaction
        .world
        .oracle_feeds
        .iter()
        .map(|(id, _)| id.clone())
        .collect();
    for feed_id in feed_ids {
        if let Some(feed) = state_transaction.world.oracle_feeds.get_mut(&feed_id) {
            replace_account_id_in_vec(&mut feed.providers, old, new);
        }
    }

    let change_ids: Vec<_> = state_transaction
        .world
        .oracle_changes
        .iter()
        .map(|(id, _)| id.clone())
        .collect();
    for change_id in change_ids {
        if let Some(change) = state_transaction.world.oracle_changes.get_mut(&change_id) {
            replace_account_id(&mut change.proposer, old, new);
            replace_account_id_in_vec(&mut change.feed.providers, old, new);
            for stage in &mut change.stages {
                replace_account_id_in_set(&mut stage.approvals, old, new);
                replace_account_id_in_set(&mut stage.rejections, old, new);
            }
        }
    }

    let dispute_ids: Vec<_> = state_transaction
        .world
        .oracle_disputes
        .iter()
        .map(|(id, _)| id.clone())
        .collect();
    for dispute_id in dispute_ids {
        if let Some(dispute) = state_transaction.world.oracle_disputes.get_mut(&dispute_id) {
            replace_account_id(&mut dispute.challenger, old, new);
            replace_account_id(&mut dispute.target, old, new);
        }
    }

    let mut provider_updates = Vec::new();
    for (key, value) in state_transaction.world.oracle_provider_stats.iter() {
        if key.provider_id == *old {
            let new_key =
                iroha_data_model::oracle::OracleProviderKey::new(key.feed_id.clone(), new.clone());
            provider_updates.push((key.clone(), new_key, *value));
        }
    }
    for (old_key, new_key, value) in provider_updates {
        state_transaction
            .world
            .oracle_provider_stats
            .remove(old_key);
        state_transaction
            .world
            .oracle_provider_stats
            .insert(new_key, value);
    }

    let observation_keys: Vec<_> = state_transaction
        .world
        .oracle_observations
        .iter()
        .map(|(id, _)| id.clone())
        .collect();
    for observation_key in observation_keys {
        if let Some(window) = state_transaction
            .world
            .oracle_observations
            .get_mut(&observation_key)
        {
            if window.observations.contains_key(old) {
                let mut updated = BTreeMap::new();
                for (provider, observation) in std::mem::take(&mut window.observations) {
                    let provider = if provider == *old {
                        new.clone()
                    } else {
                        provider
                    };
                    updated.insert(provider, observation);
                }
                window.observations = updated;
            }
        }
    }
}

fn replace_account_id_in_content_bundles(
    state_transaction: &mut StateTransaction<'_, '_>,
    old: &AccountId,
    new: &AccountId,
) {
    let bundle_ids: Vec<_> = state_transaction
        .world
        .content_bundles
        .iter()
        .map(|(id, _)| id.clone())
        .collect();
    for bundle_id in bundle_ids {
        if let Some(bundle) = state_transaction.world.content_bundles.get_mut(&bundle_id) {
            replace_account_id(&mut bundle.created_by, old, new);
        }
    }
}

fn execute_register(
    state_transaction: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    instruction: MultisigRegister,
) -> Result<(), ValidationFail> {
    let multisig_account_id = instruction.account;
    let spec = rebind_multisig_spec_domain(instruction.spec, multisig_account_id.domain())?;
    validate_registration(state_transaction, &multisig_account_id, &spec)?;
    let domain_owner = domain_owner(state_transaction, multisig_account_id.domain())?;

    if account_exists(state_transaction, &multisig_account_id)? {
        return Err(ValidationFail::NotPermitted(format!(
            "multisig account `{multisig_account_id}` already exists"
        )));
    }

    let spec_json = Json::new(spec.clone());
    let mut metadata = Metadata::default();
    metadata.insert(spec_key(), Json::new(spec.clone()));

    Register::account(
        iroha_data_model::account::Account::new(multisig_account_id.clone())
            .with_metadata(metadata),
    )
    .execute(authority, state_transaction)
    .map_err(ValidationFail::InstructionFailed)?;

    // Safeguard in case the builder drops metadata during registration.
    state_transaction
        .world
        .account_mut(&multisig_account_id)
        .map_err(map_find_error)?
        .insert(spec_key(), spec_json.clone());

    let updated_account = rekey_multisig_account(state_transaction, &multisig_account_id, &spec)
        .map_err(ValidationFail::InstructionFailed)?;
    configure_roles(state_transaction, &domain_owner, &updated_account, &spec)?;

    Ok(())
}

fn execute_propose(
    state_transaction: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    instruction: &MultisigPropose,
) -> Result<(), ValidationFail> {
    let proposer = authority.clone();
    let multisig_account = instruction.account.clone();
    let instructions_hash = HashOf::new(&instruction.instructions);
    let multisig_spec = multisig_spec(state_transaction, &multisig_account)?;
    let proposer_role = multisig_role_for(&proposer);
    let multisig_role = multisig_role_for(&multisig_account);
    let is_downward_proposal = state_transaction
        .world
        .account_roles_iter(&multisig_account)
        .any(|role| role == &proposer_role);
    let has_multisig_role = state_transaction
        .world
        .account_roles_iter(&proposer)
        .any(|role| role == &multisig_role);
    let is_signatory = spec_contains_signatory_subject(&multisig_spec, &proposer);
    let is_self_proposal = proposer == multisig_account;
    let has_not_longer_ttl = instruction
        .transaction_ttl_ms
        .is_none_or(|override_ttl_ms| override_ttl_ms <= multisig_spec.transaction_ttl_ms);

    if !has_not_longer_ttl {
        return Err(ValidationFail::NotPermitted(
            "ttl violates the restriction".to_owned(),
        ));
    }

    if !(is_downward_proposal || has_multisig_role || is_signatory || is_self_proposal) {
        return Err(ValidationFail::NotPermitted(
            "not qualified to propose multisig".to_owned(),
        ));
    }

    let proposal_key_name = proposal_key(&instructions_hash);
    match load_account_metadata(state_transaction, &multisig_account, &proposal_key_name) {
        Ok(_) => {
            return Err(ValidationFail::NotPermitted(
                "multisig proposal duplicates".to_owned(),
            ));
        }
        Err(ValidationFail::QueryFailed(QueryExecutionFail::Find(FindError::MetadataKey(_)))) => {}
        Err(err) => return Err(err),
    }

    let now_ms = now_ms(state_transaction);
    let expires_at_ms = {
        let ttl_ms = instruction
            .transaction_ttl_ms
            .unwrap_or(multisig_spec.transaction_ttl_ms);
        now_ms.saturating_add(ttl_ms.into())
    };
    let proposal_value = MultisigProposalValue::new(
        instruction.instructions.clone(),
        now_ms,
        expires_at_ms,
        BTreeSet::from([proposer.clone()]),
        None,
    );

    let approve_me = MultisigApprove::new(multisig_account.clone(), instructions_hash);
    for signatory in resolved_signatory_accounts(state_transaction, &multisig_spec)? {
        if is_multisig(state_transaction, &signatory)? {
            deploy_relayer(
                state_transaction,
                &signatory,
                &approve_me,
                now_ms,
                expires_at_ms,
            )?;
        }
    }

    SetKeyValue::account(
        multisig_account.clone(),
        proposal_key(&instructions_hash),
        proposal_value,
    )
    .execute(&multisig_account, state_transaction)
    .map_err(ValidationFail::InstructionFailed)
}

fn execute_approve(
    state_transaction: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    instruction: &MultisigApprove,
) -> Result<(), ValidationFail> {
    let approver = authority.clone();
    let multisig_account = instruction.account.clone();
    let instructions_hash = instruction.instructions_hash;

    let spec = multisig_spec(state_transaction, &multisig_account)?;
    let has_multisig_role = state_transaction
        .world
        .account_roles_iter(&approver)
        .any(|role| role == &multisig_role_for(&multisig_account));
    let is_signatory = spec_contains_signatory_subject(&spec, &approver);
    let is_self_approval = approver == multisig_account;

    if !(has_multisig_role || is_signatory || is_self_approval) {
        return Err(ValidationFail::NotPermitted(
            "not qualified to approve multisig".to_owned(),
        ));
    }
    prune_expired(state_transaction, &multisig_account, &instructions_hash)?;

    let Ok(mut proposal_value) =
        proposal_value(state_transaction, &multisig_account, &instructions_hash)
    else {
        let log = Log::new(
            Level::INFO,
            format!(
                "multisig proposal expired:\naccount: {multisig_account}\ninstructions hash: {instructions_hash}"
            ),
        );
        return log
            .execute(&multisig_account, state_transaction)
            .map_err(ValidationFail::InstructionFailed);
    };
    if let Some(true) = proposal_value.is_relayed {
        return Ok(());
    }

    upsert_subject_approval(&mut proposal_value.approvals, approver);
    SetKeyValue::account(
        multisig_account.clone(),
        proposal_key(&instructions_hash),
        proposal_value.clone(),
    )
    .execute(&multisig_account, state_transaction)
    .map_err(ValidationFail::InstructionFailed)?;

    let approved_weight = approved_weight_by_subject(&spec, &proposal_value.approvals);
    let is_authenticated = approved_weight >= u32::from(spec.quorum.get());

    if is_authenticated {
        match proposal_value.is_relayed {
            None => prune_down(state_transaction, &multisig_account, &instructions_hash)?,
            Some(false) => {
                proposal_value.is_relayed = Some(true);
                SetKeyValue::account(
                    multisig_account.clone(),
                    proposal_key(&instructions_hash),
                    proposal_value.clone(),
                )
                .execute(&multisig_account, state_transaction)
                .map_err(ValidationFail::InstructionFailed)?;
            }
            _ => unreachable!("proposal_value.is_relayed checked above"),
        }

        for instruction in proposal_value.instructions {
            instruction
                .execute(&multisig_account, state_transaction)
                .map_err(ValidationFail::from)?;
        }
    }

    Ok(())
}

fn deploy_relayer(
    state_transaction: &mut StateTransaction<'_, '_>,
    relayer: &AccountId,
    relay: &MultisigApprove,
    now_ms: u64,
    parent_expires_at_ms: u64,
) -> Result<(), ValidationFail> {
    let spec = multisig_spec(state_transaction, relayer)?;
    let relay_expires_at_ms =
        capped_relay_expiry(now_ms, parent_expires_at_ms, spec.transaction_ttl_ms.get());

    let relay_hash = HashOf::new(&vec![InstructionBox::from(relay.clone())]);
    let sub_relay = MultisigApprove::new(relayer.clone(), relay_hash);

    for signatory in resolved_signatory_accounts(state_transaction, &spec)? {
        if is_multisig(state_transaction, &signatory)? {
            deploy_relayer(
                state_transaction,
                &signatory,
                &sub_relay,
                now_ms,
                relay_expires_at_ms,
            )?;
        }
    }

    let relay_value = MultisigProposalValue::new(
        vec![InstructionBox::from(relay.clone())],
        now_ms,
        relay_expires_at_ms,
        BTreeSet::new(),
        Some(false),
    );
    SetKeyValue::account(relayer.clone(), proposal_key(&relay_hash), relay_value)
        .execute(relayer, state_transaction)
        .map_err(ValidationFail::InstructionFailed)
}

fn capped_relay_expiry(now_ms: u64, parent_expires_at_ms: u64, relayer_ttl_ms: u64) -> u64 {
    let local_expiry = now_ms.saturating_add(relayer_ttl_ms);
    local_expiry.min(parent_expires_at_ms)
}

fn prune_expired(
    state_transaction: &mut StateTransaction<'_, '_>,
    multisig_account: &AccountId,
    instructions_hash: &HashOf<Vec<InstructionBox>>,
) -> Result<(), ValidationFail> {
    let proposal_value = proposal_value(state_transaction, multisig_account, instructions_hash)?;

    if now_ms(state_transaction) < proposal_value.expires_at_ms {
        return Ok(());
    }

    for instruction in proposal_value.instructions {
        if let Some(custom) = instruction.as_any().downcast_ref::<CustomInstruction>()
            && let Ok(MultisigInstructionBox::Approve(approve)) = custom.payload().try_into()
        {
            return prune_expired(
                state_transaction,
                &approve.account,
                &approve.instructions_hash,
            );
        }
    }

    prune_down(state_transaction, multisig_account, instructions_hash)
}

fn prune_down(
    state_transaction: &mut StateTransaction<'_, '_>,
    multisig_account: &AccountId,
    instructions_hash: &HashOf<Vec<InstructionBox>>,
) -> Result<(), ValidationFail> {
    let spec = multisig_spec(state_transaction, multisig_account)?;

    RemoveKeyValue::account(multisig_account.clone(), proposal_key(instructions_hash))
        .execute(multisig_account, state_transaction)
        .map_err(ValidationFail::InstructionFailed)?;

    for signatory in resolved_signatory_accounts(state_transaction, &spec)? {
        let relay_hash = {
            let relay = MultisigApprove::new(multisig_account.clone(), *instructions_hash);
            HashOf::new(&vec![InstructionBox::from(relay)])
        };
        if is_multisig(state_transaction, &signatory)? {
            prune_down(state_transaction, &signatory, &relay_hash)?;
        }
    }

    Ok(())
}

fn validate_registration(
    state_transaction: &mut StateTransaction<'_, '_>,
    _multisig_account: &AccountId,
    spec: &MultisigSpec,
) -> Result<(), ValidationFail> {
    ensure_quorum_reachable(spec)?;
    ensure_signatories_exist(state_transaction, spec)?;
    ensure_signatories_are_single(spec)?;
    let roots = resolved_signatory_accounts(state_transaction, spec)?;
    ensure_multisig_graph_is_acyclic(roots, state_transaction)?;
    Ok(())
}

fn ensure_quorum_reachable(spec: &MultisigSpec) -> Result<(), ValidationFail> {
    let total_weight: u32 = spec
        .signatories
        .values()
        .map(|weight| u32::from(*weight))
        .sum();
    let quorum = u32::from(spec.quorum.get());

    if total_weight < quorum {
        return Err(ValidationFail::NotPermitted(format!(
            "multisig quorum {quorum} exceeds total signatory weight {total_weight}"
        )));
    }

    Ok(())
}

fn ensure_signatories_exist(
    state_transaction: &StateTransaction<'_, '_>,
    spec: &MultisigSpec,
) -> Result<(), ValidationFail> {
    for account in spec.signatories.keys() {
        let _ = resolve_signatory_account(state_transaction, account)?;
    }
    Ok(())
}

fn ensure_signatories_are_single(spec: &MultisigSpec) -> Result<(), ValidationFail> {
    for account in spec.signatories.keys() {
        if account.controller().single_signatory().is_none() {
            return Err(ValidationFail::NotPermitted(format!(
                "multisig signatory `{account}` must be a single-key account"
            )));
        }
    }
    Ok(())
}

fn ensure_multisig_graph_is_acyclic(
    roots: impl IntoIterator<Item = AccountId>,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<(), ValidationFail> {
    ensure_multisig_graph_is_acyclic_with(roots.into_iter().collect(), |account| {
        if !is_multisig(state_transaction, &account)? {
            return Ok(Vec::new());
        }
        let spec = multisig_spec(state_transaction, &account)?;
        Ok(spec.signatories.keys().cloned().collect())
    })
}

fn spec_contains_signatory_subject(spec: &MultisigSpec, account: &AccountId) -> bool {
    let subject = account.subject_id();
    spec.signatories
        .keys()
        .any(|signatory| signatory.subject_id() == subject)
}

fn approved_weight_by_subject(spec: &MultisigSpec, approvals: &BTreeSet<AccountId>) -> u32 {
    let approved_subjects: BTreeSet<_> = approvals.iter().map(AccountId::subject_id).collect();
    spec.signatories
        .iter()
        .filter(|(signatory, _)| approved_subjects.contains(&signatory.subject_id()))
        .map(|(_, weight)| u32::from(*weight))
        .sum()
}

fn upsert_subject_approval(approvals: &mut BTreeSet<AccountId>, approver: AccountId) {
    let approver_subject = approver.subject_id();
    approvals.retain(|approved| approved.subject_id() != approver_subject);
    approvals.insert(approver);
}

fn resolve_signatory_account(
    state_transaction: &StateTransaction<'_, '_>,
    signatory: &AccountId,
) -> Result<AccountId, ValidationFail> {
    match state_transaction.world.account(signatory) {
        Ok(_) => return Ok(signatory.clone()),
        Err(FindError::Account(_)) => {}
        Err(err) => return Err(map_find_error(err)),
    }

    let subject = signatory.subject_id();
    state_transaction
        .world
        .accounts_for_subject_iter(&subject)
        .next()
        .map(|account| account.id().clone())
        .ok_or_else(|| map_find_error(FindError::Account(signatory.clone())))
}

fn resolved_signatory_accounts(
    state_transaction: &StateTransaction<'_, '_>,
    spec: &MultisigSpec,
) -> Result<Vec<AccountId>, ValidationFail> {
    let mut accounts = Vec::new();
    let mut seen = BTreeSet::new();
    for signatory in spec.signatories.keys() {
        let subject = signatory.subject_id();
        if !seen.insert(subject) {
            continue;
        }
        accounts.push(resolve_signatory_account(state_transaction, signatory)?);
    }
    Ok(accounts)
}

fn ensure_multisig_graph_is_acyclic_with<F>(
    roots: Vec<AccountId>,
    mut next: F,
) -> Result<(), ValidationFail>
where
    F: FnMut(AccountId) -> Result<Vec<AccountId>, ValidationFail>,
{
    let mut stack: Vec<(AccountId, Vec<AccountId>)> = roots
        .into_iter()
        .map(|root| {
            let path = vec![root.clone()];
            (root, path)
        })
        .collect();

    while let Some((current, path)) = stack.pop() {
        let children = next(current.clone())?;
        for child in children {
            if let Some(idx) = path.iter().position(|seen| seen == &child) {
                let mut cycle = path[idx..].to_vec();
                cycle.push(child.clone());
                let message = cycle
                    .into_iter()
                    .map(|id| id.to_string())
                    .collect::<Vec<_>>()
                    .join(" -> ");
                return Err(ValidationFail::NotPermitted(format!(
                    "multisig spec forms a cycle: {message}"
                )));
            }
            let mut next_path = path.clone();
            next_path.push(child.clone());
            stack.push((child, next_path));
        }
    }

    Ok(())
}

fn multisig_spec(
    state_transaction: &StateTransaction<'_, '_>,
    multisig_account: &AccountId,
) -> Result<MultisigSpec, ValidationFail> {
    let key = spec_key();
    match load_account_metadata(state_transaction, multisig_account, &key) {
        Ok(raw) => {
            let spec = raw
                .try_into_any_norito()
                .map_err(|err| metadata_conversion_error(&err))?;
            rebind_multisig_spec_domain(spec, multisig_account.domain())
        }
        Err(ValidationFail::QueryFailed(QueryExecutionFail::Find(FindError::MetadataKey(_)))) => {
            if let Ok(account) = state_transaction.world.account(multisig_account) {
                let keys: Vec<Name> = account.metadata().iter().map(|(k, _)| k.clone()).collect();
                iroha_logger::error!(
                    account = %multisig_account,
                    ?keys,
                    "multisig spec metadata missing"
                );
            }
            Err(ValidationFail::QueryFailed(QueryExecutionFail::Find(
                FindError::MetadataKey(key.clone()),
            )))
        }
        Err(err) => Err(err),
    }
}

fn rebind_multisig_spec_domain(
    spec: MultisigSpec,
    domain: &iroha_data_model::domain::DomainId,
) -> Result<MultisigSpec, ValidationFail> {
    let MultisigSpec {
        signatories,
        quorum,
        transaction_ttl_ms,
    } = spec;
    let mut rebased_signatories = BTreeMap::new();
    for (account, weight) in signatories {
        let rebased = if account.domain() == domain {
            account
        } else {
            AccountId {
                domain: domain.clone(),
                controller: account.controller().clone(),
            }
        };
        if rebased_signatories
            .insert(rebased.clone(), weight)
            .is_some()
        {
            return Err(ValidationFail::NotPermitted(format!(
                "multisig signatory collision after domain normalization for `{rebased}`"
            )));
        }
    }

    Ok(MultisigSpec {
        signatories: rebased_signatories,
        quorum,
        transaction_ttl_ms,
    })
}

fn is_multisig(
    state_transaction: &StateTransaction<'_, '_>,
    account: &AccountId,
) -> Result<bool, ValidationFail> {
    match load_account_metadata(state_transaction, account, &spec_key()) {
        Ok(_) => Ok(true),
        Err(ValidationFail::QueryFailed(QueryExecutionFail::Find(FindError::MetadataKey(_)))) => {
            Ok(false)
        }
        Err(err) => Err(err),
    }
}

fn domain_owner(
    state_transaction: &StateTransaction<'_, '_>,
    domain_id: &iroha_data_model::domain::DomainId,
) -> Result<AccountId, ValidationFail> {
    state_transaction
        .world
        .domain(domain_id)
        .map(|domain| domain.owned_by().clone())
        .map_err(map_find_error)
}

fn account_exists(
    state_transaction: &StateTransaction<'_, '_>,
    account_id: &AccountId,
) -> Result<bool, ValidationFail> {
    match state_transaction.world.account(account_id) {
        Ok(_) => Ok(true),
        Err(FindError::Account(_)) => Ok(false),
        Err(err) => Err(map_find_error(err)),
    }
}

fn configure_roles(
    state_transaction: &mut StateTransaction<'_, '_>,
    domain_owner: &AccountId,
    multisig_account: &AccountId,
    spec: &MultisigSpec,
) -> Result<(), ValidationFail> {
    let signatories = resolved_signatory_accounts(state_transaction, spec)?;

    let multisig_role_id = multisig_role_for(multisig_account);
    ensure_role_available(
        state_transaction,
        domain_owner,
        &multisig_role_id,
        &signatories,
    )?;
    grant_role_if_needed(
        state_transaction,
        &multisig_role_id,
        multisig_account,
        domain_owner,
    )?;

    for signatory in &signatories {
        let signatory_role_id = multisig_role_for(signatory);
        let delegates = [signatory.clone(), multisig_account.clone()];

        ensure_role_available(
            state_transaction,
            domain_owner,
            &signatory_role_id,
            &delegates,
        )?;
        grant_role_if_needed(
            state_transaction,
            &signatory_role_id,
            signatory,
            domain_owner,
        )?;
        grant_role_if_needed(
            state_transaction,
            &signatory_role_id,
            multisig_account,
            domain_owner,
        )?;
        grant_role_if_needed(
            state_transaction,
            &multisig_role_id,
            signatory,
            domain_owner,
        )?;
    }

    Ok(())
}

fn multisig_spec_strict(
    state_transaction: &StateTransaction<'_, '_>,
    multisig_account: &AccountId,
) -> Result<MultisigSpec, InstructionExecutionError> {
    let key = spec_key();
    let account = state_transaction
        .world
        .account(multisig_account)
        .map_err(InstructionExecutionError::Find)?;
    let raw = account
        .metadata()
        .get(&key)
        .cloned()
        .ok_or_else(|| InstructionExecutionError::Find(FindError::MetadataKey(key.clone())))?;
    let spec = raw.try_into_any_norito().map_err(|err| {
        InstructionExecutionError::Conversion(format!("multisig spec metadata malformed:\n{err}"))
    })?;
    rebind_multisig_spec_domain(spec, multisig_account.domain()).map_err(map_validation_fail)
}

fn ensure_role_available(
    state_transaction: &mut StateTransaction<'_, '_>,
    domain_owner: &AccountId,
    role_id: &RoleId,
    delegates: &[AccountId],
) -> Result<(), ValidationFail> {
    if !role_exists(state_transaction, role_id) {
        Register::role(Role::new(role_id.clone(), domain_owner.clone()))
            .execute(domain_owner, state_transaction)
            .map_err(ValidationFail::InstructionFailed)?;
        return Ok(());
    }

    if has_role(state_transaction, domain_owner, role_id)? {
        return Ok(());
    }

    for delegate in delegates {
        if delegate == domain_owner || !has_role(state_transaction, delegate, role_id)? {
            continue;
        }

        Grant::account_role(role_id.clone(), domain_owner.clone())
            .execute(delegate, state_transaction)
            .map_err(ValidationFail::InstructionFailed)?;

        if has_role(state_transaction, domain_owner, role_id)? {
            return Ok(());
        }
    }

    Err(ValidationFail::NotPermitted(format!(
        "domain owner `{domain_owner}` must hold role `{role_id}` to configure multisig"
    )))
}

fn grant_role_if_needed(
    state_transaction: &mut StateTransaction<'_, '_>,
    role_id: &RoleId,
    account: &AccountId,
    authority: &AccountId,
) -> Result<(), ValidationFail> {
    if has_role(state_transaction, account, role_id)? {
        return Ok(());
    }

    Grant::account_role(role_id.clone(), account.clone())
        .execute(authority, state_transaction)
        .map_err(ValidationFail::InstructionFailed)
}

fn has_role(
    state_transaction: &StateTransaction<'_, '_>,
    account: &AccountId,
    role_id: &RoleId,
) -> Result<bool, ValidationFail> {
    state_transaction
        .world
        .account(account)
        .map_err(map_find_error)?;

    Ok(state_transaction
        .world
        .account_roles_iter(account)
        .any(|role| role == role_id))
}

fn role_exists(state_transaction: &StateTransaction<'_, '_>, role_id: &RoleId) -> bool {
    state_transaction.world.roles.get(role_id).is_some()
}

fn load_account_metadata(
    state_transaction: &StateTransaction<'_, '_>,
    account_id: &AccountId,
    key: &Name,
) -> Result<Json, ValidationFail> {
    state_transaction
        .world
        .account(account_id)
        .map_err(map_find_error)?
        .metadata()
        .get(key)
        .cloned()
        .ok_or_else(|| {
            ValidationFail::QueryFailed(QueryExecutionFail::Find(FindError::MetadataKey(
                key.clone(),
            )))
        })
}

fn proposal_value(
    state_transaction: &StateTransaction<'_, '_>,
    multisig_account: &AccountId,
    instructions_hash: &HashOf<Vec<InstructionBox>>,
) -> Result<MultisigProposalValue, ValidationFail> {
    let key = proposal_key(instructions_hash);
    load_account_metadata(state_transaction, multisig_account, &key)?
        .try_into_any_norito()
        .map_err(|err| metadata_conversion_error(&err))
}

fn now_ms(state_transaction: &StateTransaction<'_, '_>) -> u64 {
    state_transaction
        ._curr_block
        .creation_time()
        .as_millis()
        .try_into()
        .expect("block creation time must fit into u64")
}

fn metadata_conversion_error(err: &norito::Error) -> ValidationFail {
    ValidationFail::QueryFailed(QueryExecutionFail::Conversion(format!(
        "multisig account metadata malformed:\n{err}"
    )))
}

fn map_find_error(err: FindError) -> ValidationFail {
    ValidationFail::InstructionFailed(InstructionExecutionError::Find(err))
}

fn map_validation_fail(err: ValidationFail) -> InstructionExecutionError {
    match err {
        ValidationFail::InstructionFailed(err) => err,
        ValidationFail::QueryFailed(QueryExecutionFail::Find(err)) => {
            InstructionExecutionError::Find(err)
        }
        ValidationFail::QueryFailed(QueryExecutionFail::Conversion(msg)) => {
            InstructionExecutionError::Conversion(msg)
        }
        ValidationFail::QueryFailed(err) => InstructionExecutionError::Query(err),
        ValidationFail::NotPermitted(msg) => {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(msg))
        }
        other => InstructionExecutionError::InvariantViolation(other.to_string().into()),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        num::{NonZeroU16, NonZeroU64},
    };

    use iroha_crypto::KeyPair;
    use iroha_data_model::{
        ChainId,
        account::AccountId,
        block::BlockHeader,
        isi::{AddSignatory, RemoveSignatory, SetAccountQuorum},
        prelude::{Domain, InstructionBox, Register},
    };
    use iroha_executor_data_model::isi::multisig::{
        DEFAULT_MULTISIG_TTL_MS, MultisigApprove, MultisigPropose, MultisigRegister, MultisigSpec,
    };
    use nonzero_ext::nonzero;

    use super::*;
    use crate::{
        executor::Executor,
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };

    fn register_multisig_account(
        state_transaction: &mut StateTransaction<'_, '_>,
        owner_id: &AccountId,
        domain_id: &iroha_data_model::domain::DomainId,
        spec: &MultisigSpec,
        label: &str,
    ) -> AccountId {
        let multisig_key = KeyPair::random();
        let multisig_id = AccountId::new(domain_id.clone(), multisig_key.public_key().clone());
        let mut metadata = Metadata::default();
        metadata.insert(spec_key(), Json::new(spec.clone()));
        Register::account(
            iroha_data_model::account::Account::new(multisig_id.clone()).with_metadata(metadata),
        )
        .execute(owner_id, state_transaction)
        .expect(label);
        rekey_multisig_account(state_transaction, &multisig_id, spec)
            .expect("rekey multisig account")
    }

    #[test]
    fn initial_executor_runs_multisig_flow() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(
            World::new(),
            kura,
            query_handle,
            ChainId::from("multisig-test-chain"),
        );
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();
        let domain_id: iroha_data_model::domain::DomainId = "acme".parse().unwrap();

        let signer1 = KeyPair::random();
        let signer2 = KeyPair::random();
        let signer1_id = AccountId::new(domain_id.clone(), signer1.public_key().clone());
        let signer2_id = AccountId::new(domain_id.clone(), signer2.public_key().clone());

        Register::domain(Domain::new(domain_id.clone()))
            .execute(&signer1_id, &mut state_transaction)
            .expect("domain registration");

        Register::account(iroha_data_model::account::Account::new(signer1_id.clone()))
            .execute(&signer1_id, &mut state_transaction)
            .expect("register signer1");
        Register::account(iroha_data_model::account::Account::new(signer2_id.clone()))
            .execute(&signer1_id, &mut state_transaction)
            .expect("register signer2");

        let spec = MultisigSpec {
            signatories: BTreeMap::from([(signer1_id.clone(), 1), (signer2_id.clone(), 1)]),
            quorum: NonZeroU16::new(2).unwrap(),
            transaction_ttl_ms: NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).unwrap(),
        };
        let multisig_account_key = KeyPair::random();
        let multisig_id =
            AccountId::new(domain_id.clone(), multisig_account_key.public_key().clone());
        let register = MultisigRegister::new(multisig_id.clone(), spec.clone());
        let executor = Executor::Initial;
        executor
            .execute_instruction(
                &mut state_transaction,
                &signer1_id,
                InstructionBox::from(register),
            )
            .expect("multisig register");

        let spec_key = spec_key();
        let policy = multisig_policy_from_spec(&spec).expect("policy");
        let expected_id = AccountId::new_multisig(domain_id.clone(), policy);
        let account_after_register = state_transaction
            .world
            .account(&expected_id)
            .expect("multisig account registered");
        assert!(
            account_after_register.metadata().get(&spec_key).is_some(),
            "multisig spec metadata must be stored on registration"
        );
        assert!(
            matches!(
                state_transaction.world.account(&multisig_id),
                Err(FindError::Account(_))
            ),
            "initial controller id should be rekeyed"
        );
        let stored_spec =
            multisig_spec(&state_transaction, &expected_id).expect("spec must decode");
        assert_eq!(stored_spec, spec, "spec roundtrip through metadata");
    }

    #[test]
    fn add_signatory_updates_multisig_spec_and_roles() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(
            World::new(),
            kura,
            query_handle,
            ChainId::from("multisig-add-signatory"),
        );
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();
        let domain_id: iroha_data_model::domain::DomainId = "wonderland".parse().unwrap();

        let owner_key = KeyPair::random();
        let owner_id = AccountId::new(domain_id.clone(), owner_key.public_key().clone());
        Register::domain(Domain::new(domain_id.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("domain registration");
        Register::account(iroha_data_model::account::Account::new(owner_id.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("register owner");

        let signer1 = KeyPair::random();
        let signer2 = KeyPair::random();
        let signer1_id = AccountId::new(domain_id.clone(), signer1.public_key().clone());
        let signer2_id = AccountId::new(domain_id.clone(), signer2.public_key().clone());
        Register::account(iroha_data_model::account::Account::new(signer1_id.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("register signer1");
        Register::account(iroha_data_model::account::Account::new(signer2_id.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("register signer2");

        let spec = MultisigSpec {
            signatories: BTreeMap::from([(signer1_id.clone(), 1)]),
            quorum: NonZeroU16::new(1).unwrap(),
            transaction_ttl_ms: NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).unwrap(),
        };
        let multisig_id = register_multisig_account(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &spec,
            "register multisig account",
        );

        AddSignatory::new(multisig_id.clone(), signer2.public_key().clone())
            .execute(&owner_id, &mut state_transaction)
            .expect("add signatory");

        let mut updated_spec = spec.clone();
        updated_spec.signatories.insert(signer2_id.clone(), 1);
        let updated_policy = multisig_policy_from_spec(&updated_spec).expect("policy");
        let updated_account = AccountId::new_multisig(domain_id.clone(), updated_policy);
        let updated = multisig_spec(&state_transaction, &updated_account)
            .expect("spec must decode after add");
        assert!(
            updated.signatories.contains_key(&signer2_id),
            "added signatory must appear in spec"
        );
        assert!(
            matches!(
                state_transaction.world.account(&multisig_id),
                Err(FindError::Account(_))
            ),
            "multisig account should be rekeyed after add"
        );
        let multisig_role = multisig_role_for(&updated_account);
        let signer_role = multisig_role_for(&signer2_id);
        assert!(
            state_transaction
                .world
                .account_roles_iter(&signer2_id)
                .any(|role| role == &multisig_role),
            "added signatory should gain multisig role"
        );
        assert!(
            state_transaction
                .world
                .account_roles_iter(&updated_account)
                .any(|role| role == &signer_role),
            "multisig account should receive the signatory role"
        );
    }

    #[test]
    fn remove_signatory_updates_multisig_spec_and_revokes_roles() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(
            World::new(),
            kura,
            query_handle,
            ChainId::from("multisig-remove-signatory"),
        );
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();
        let domain_id: iroha_data_model::domain::DomainId = "wonderland".parse().unwrap();

        let owner_key = KeyPair::random();
        let owner_id = AccountId::new(domain_id.clone(), owner_key.public_key().clone());
        Register::domain(Domain::new(domain_id.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("domain registration");
        Register::account(iroha_data_model::account::Account::new(owner_id.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("register owner");

        let signer1 = KeyPair::random();
        let signer2 = KeyPair::random();
        let signer1_id = AccountId::new(domain_id.clone(), signer1.public_key().clone());
        let signer2_id = AccountId::new(domain_id.clone(), signer2.public_key().clone());
        Register::account(iroha_data_model::account::Account::new(signer1_id.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("register signer1");
        Register::account(iroha_data_model::account::Account::new(signer2_id.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("register signer2");

        let spec = MultisigSpec {
            signatories: BTreeMap::from([(signer1_id.clone(), 1), (signer2_id.clone(), 1)]),
            quorum: NonZeroU16::new(2).unwrap(),
            transaction_ttl_ms: NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).unwrap(),
        };
        let multisig_id = register_multisig_account(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &spec,
            "register multisig account",
        );
        configure_roles(&mut state_transaction, &owner_id, &multisig_id, &spec)
            .expect("configure roles");

        RemoveSignatory::new(multisig_id.clone(), signer2.public_key().clone())
            .execute(&owner_id, &mut state_transaction)
            .expect("remove signatory");

        let updated_spec = MultisigSpec {
            signatories: BTreeMap::from([(signer1_id.clone(), 1)]),
            quorum: NonZeroU16::new(1).unwrap(),
            transaction_ttl_ms: spec.transaction_ttl_ms,
        };
        let updated_policy = multisig_policy_from_spec(&updated_spec).expect("policy");
        let updated_account = AccountId::new_multisig(domain_id.clone(), updated_policy);
        let updated = multisig_spec(&state_transaction, &updated_account)
            .expect("spec must decode after remove");
        assert!(
            !updated.signatories.contains_key(&signer2_id),
            "removed signatory must be absent from spec"
        );
        assert!(
            matches!(
                state_transaction.world.account(&multisig_id),
                Err(FindError::Account(_))
            ),
            "multisig account should be rekeyed after removal"
        );
        let multisig_role = multisig_role_for(&updated_account);
        let signer_role = multisig_role_for(&signer2_id);
        assert!(
            !state_transaction
                .world
                .account_roles_iter(&signer2_id)
                .any(|role| role == &multisig_role),
            "removed signatory should lose multisig role"
        );
        assert!(
            !state_transaction
                .world
                .account_roles_iter(&updated_account)
                .any(|role| role == &signer_role),
            "multisig account should drop removed signatory role"
        );
    }

    #[test]
    fn set_account_quorum_updates_multisig_spec() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(
            World::new(),
            kura,
            query_handle,
            ChainId::from("multisig-set-quorum"),
        );
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();
        let domain_id: iroha_data_model::domain::DomainId = "wonderland".parse().unwrap();

        let owner_key = KeyPair::random();
        let owner_id = AccountId::new(domain_id.clone(), owner_key.public_key().clone());
        Register::domain(Domain::new(domain_id.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("domain registration");
        Register::account(iroha_data_model::account::Account::new(owner_id.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("register owner");

        let signer1 = KeyPair::random();
        let signer2 = KeyPair::random();
        let signer1_id = AccountId::new(domain_id.clone(), signer1.public_key().clone());
        let signer2_id = AccountId::new(domain_id.clone(), signer2.public_key().clone());
        Register::account(iroha_data_model::account::Account::new(signer1_id.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("register signer1");
        Register::account(iroha_data_model::account::Account::new(signer2_id.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("register signer2");

        let spec = MultisigSpec {
            signatories: BTreeMap::from([(signer1_id.clone(), 1), (signer2_id.clone(), 1)]),
            quorum: NonZeroU16::new(1).unwrap(),
            transaction_ttl_ms: NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).unwrap(),
        };
        let multisig_id = register_multisig_account(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &spec,
            "register multisig account",
        );

        let new_quorum = NonZeroU16::new(2).unwrap();
        SetAccountQuorum::new(multisig_id.clone(), new_quorum)
            .execute(&owner_id, &mut state_transaction)
            .expect("set quorum");

        let updated_spec = MultisigSpec {
            signatories: spec.signatories.clone(),
            quorum: new_quorum,
            transaction_ttl_ms: spec.transaction_ttl_ms,
        };
        let updated_policy = multisig_policy_from_spec(&updated_spec).expect("policy");
        let updated_account = AccountId::new_multisig(domain_id.clone(), updated_policy);
        let updated = multisig_spec(&state_transaction, &updated_account)
            .expect("spec must decode after set quorum");
        assert_eq!(updated.quorum, new_quorum, "quorum update should persist");
        assert!(
            matches!(
                state_transaction.world.account(&multisig_id),
                Err(FindError::Account(_))
            ),
            "multisig account should be rekeyed after quorum update"
        );
    }

    #[test]
    fn rekey_account_id_updates_subject_domain_indexes() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(
            World::new(),
            kura,
            query_handle,
            ChainId::from("multisig-rekey-indexes"),
        );
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();
        let domain_id: iroha_data_model::domain::DomainId = "default".parse().unwrap();

        let old_key = KeyPair::random();
        let old_account = AccountId::new(domain_id.clone(), old_key.public_key().clone());
        Register::domain(Domain::new(domain_id.clone()))
            .execute(&old_account, &mut state_transaction)
            .expect("domain registration");
        Register::account(iroha_data_model::account::Account::new(old_account.clone()))
            .execute(&old_account, &mut state_transaction)
            .expect("register old account");

        let new_key = KeyPair::random();
        let new_account = AccountId::new(domain_id.clone(), new_key.public_key().clone());

        rekey_account_id(&mut state_transaction, &old_account, &new_account)
            .expect("rekey should succeed");

        assert!(
            matches!(
                state_transaction.world.account(&old_account),
                Err(FindError::Account(_))
            ),
            "old scoped account should be removed after rekey"
        );
        assert!(
            state_transaction.world.account(&new_account).is_ok(),
            "new scoped account should be present after rekey"
        );

        let old_subject = old_account.subject_id();
        let old_subject_domains = state_transaction
            .world
            .account_subject_domains
            .get(&old_subject)
            .expect("old subject record should remain materialized");
        assert!(
            old_subject_domains.is_empty(),
            "old subject should have no linked domains after rekey"
        );

        let new_subject = new_account.subject_id();
        let new_subject_domains = state_transaction
            .world
            .account_subject_domains
            .get(&new_subject)
            .expect("new subject should be linked to the account domain");
        assert_eq!(
            new_subject_domains,
            &std::collections::BTreeSet::from([domain_id.clone()]),
            "new subject should be linked to the rekeyed domain"
        );

        let domain_subjects = state_transaction
            .world
            .domain_account_subjects
            .get(&domain_id)
            .expect("domain membership index should exist");
        assert!(
            !domain_subjects.contains(&old_subject),
            "domain subject index must not keep old subject after rekey"
        );
        assert!(
            domain_subjects.contains(&new_subject),
            "domain subject index should contain new subject after rekey"
        );
    }

    #[test]
    fn rebind_multisig_spec_domain_rewrites_signatories_to_account_domain() {
        let source_domain: iroha_data_model::domain::DomainId = "default".parse().unwrap();
        let target_domain: iroha_data_model::domain::DomainId = "wonderland".parse().unwrap();
        let signer = AccountId::new(source_domain, KeyPair::random().public_key().clone());
        let spec = MultisigSpec {
            signatories: BTreeMap::from([(signer.clone(), 1)]),
            quorum: NonZeroU16::new(1).unwrap(),
            transaction_ttl_ms: NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).unwrap(),
        };

        let rebased =
            rebind_multisig_spec_domain(spec, &target_domain).expect("domain rebind should work");
        let rebased_signer = rebased
            .signatories
            .keys()
            .next()
            .expect("rebased signatory exists");
        assert_eq!(rebased_signer.domain(), &target_domain);
        assert_eq!(rebased_signer.controller(), signer.controller());
    }

    #[test]
    fn rebind_multisig_spec_domain_rejects_colliding_signatories() {
        let source_a: iroha_data_model::domain::DomainId = "default".parse().unwrap();
        let source_b: iroha_data_model::domain::DomainId = "acme".parse().unwrap();
        let target_domain: iroha_data_model::domain::DomainId = "wonderland".parse().unwrap();
        let shared_key = KeyPair::random().public_key().clone();

        let first = AccountId::new(source_a, shared_key.clone());
        let second = AccountId::new(source_b, shared_key);
        let spec = MultisigSpec {
            signatories: BTreeMap::from([(first, 1), (second, 1)]),
            quorum: NonZeroU16::new(1).unwrap(),
            transaction_ttl_ms: NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).unwrap(),
        };

        let err = rebind_multisig_spec_domain(spec, &target_domain)
            .expect_err("rebasing colliding controllers should fail");
        assert!(
            matches!(err, ValidationFail::NotPermitted(_)),
            "unexpected error kind: {err:?}"
        );
    }

    #[test]
    fn set_account_quorum_rejects_unreachable_quorum() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(
            World::new(),
            kura,
            query_handle,
            ChainId::from("multisig-set-quorum-invalid"),
        );
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();
        let domain_id: iroha_data_model::domain::DomainId = "wonderland".parse().unwrap();

        let owner_key = KeyPair::random();
        let owner_id = AccountId::new(domain_id.clone(), owner_key.public_key().clone());
        Register::domain(Domain::new(domain_id.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("domain registration");
        Register::account(iroha_data_model::account::Account::new(owner_id.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("register owner");

        let signer1 = KeyPair::random();
        let signer2 = KeyPair::random();
        let signer1_id = AccountId::new(domain_id.clone(), signer1.public_key().clone());
        let signer2_id = AccountId::new(domain_id.clone(), signer2.public_key().clone());
        Register::account(iroha_data_model::account::Account::new(signer1_id.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("register signer1");
        Register::account(iroha_data_model::account::Account::new(signer2_id.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("register signer2");

        let spec = MultisigSpec {
            signatories: BTreeMap::from([(signer1_id.clone(), 1), (signer2_id.clone(), 1)]),
            quorum: NonZeroU16::new(1).unwrap(),
            transaction_ttl_ms: NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).unwrap(),
        };
        let multisig_id = register_multisig_account(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &spec,
            "register multisig account",
        );

        let unreachable_quorum = NonZeroU16::new(3).unwrap();
        let err = SetAccountQuorum::new(multisig_id, unreachable_quorum)
            .execute(&owner_id, &mut state_transaction)
            .expect_err("quorum above total weight should fail");
        assert!(
            matches!(
                err,
                InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                    _
                ))
            ),
            "unexpected error for unreachable quorum: {err:?}"
        );
    }

    #[test]
    fn multisig_propose_rejects_ttl_above_default() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(
            World::new(),
            kura,
            query_handle,
            ChainId::from("multisig-ttl-chain"),
        );
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();
        let domain_id: iroha_data_model::domain::DomainId = "ttl".parse().unwrap();

        let signer1 = KeyPair::random();
        let signer2 = KeyPair::random();
        let signer1_id = AccountId::new(domain_id.clone(), signer1.public_key().clone());
        let signer2_id = AccountId::new(domain_id.clone(), signer2.public_key().clone());

        Register::domain(Domain::new(domain_id.clone()))
            .execute(&signer1_id, &mut state_transaction)
            .expect("domain registration");

        Register::account(iroha_data_model::account::Account::new(signer1_id.clone()))
            .execute(&signer1_id, &mut state_transaction)
            .expect("register signer1");
        Register::account(iroha_data_model::account::Account::new(signer2_id.clone()))
            .execute(&signer1_id, &mut state_transaction)
            .expect("register signer2");

        let spec = MultisigSpec {
            signatories: BTreeMap::from([(signer1_id.clone(), 1), (signer2_id.clone(), 1)]),
            quorum: NonZeroU16::new(2).unwrap(),
            transaction_ttl_ms: NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).unwrap(),
        };
        let multisig_account_key = KeyPair::random();
        let multisig_id =
            AccountId::new(domain_id.clone(), multisig_account_key.public_key().clone());
        let register = MultisigRegister::new(multisig_id.clone(), spec.clone());
        Executor::Initial
            .execute_instruction(
                &mut state_transaction,
                &signer1_id,
                InstructionBox::from(register),
            )
            .expect("multisig register");

        let policy = multisig_policy_from_spec(&spec).expect("policy");
        let expected_id = AccountId::new_multisig(domain_id.clone(), policy);
        let override_ttl =
            NonZeroU64::new(spec.transaction_ttl_ms.get().saturating_add(1)).unwrap();
        let propose = MultisigPropose::new(expected_id.clone(), Vec::new(), Some(override_ttl));

        let result = Executor::Initial.execute_instruction(
            &mut state_transaction,
            &signer1_id,
            InstructionBox::from(propose),
        );
        match result {
            Err(ValidationFail::NotPermitted(msg)) => {
                assert!(
                    msg.contains("ttl violates the restriction"),
                    "unexpected error: {msg}"
                );
            }
            other => panic!("expected TTL violation, got {other:?}"),
        }
    }

    #[test]
    fn multisig_signatory_can_propose_without_roles() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(
            World::new(),
            kura,
            query_handle,
            ChainId::from("multisig-signatory-propose"),
        );
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();
        let domain_id: iroha_data_model::domain::DomainId = "signatory".parse().unwrap();

        let signer1 = KeyPair::random();
        let signer2 = KeyPair::random();
        let signer1_id = AccountId::new(domain_id.clone(), signer1.public_key().clone());
        let signer2_id = AccountId::new(domain_id.clone(), signer2.public_key().clone());

        Register::domain(Domain::new(domain_id.clone()))
            .execute(&signer1_id, &mut state_transaction)
            .expect("domain registration");
        Register::account(iroha_data_model::account::Account::new(signer1_id.clone()))
            .execute(&signer1_id, &mut state_transaction)
            .expect("register signer1");
        Register::account(iroha_data_model::account::Account::new(signer2_id.clone()))
            .execute(&signer1_id, &mut state_transaction)
            .expect("register signer2");

        let spec = MultisigSpec {
            signatories: BTreeMap::from([(signer1_id.clone(), 1), (signer2_id.clone(), 1)]),
            quorum: NonZeroU16::new(2).unwrap(),
            transaction_ttl_ms: NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).unwrap(),
        };
        let multisig_id = register_multisig_account(
            &mut state_transaction,
            &signer1_id,
            &domain_id,
            &spec,
            "register multisig account",
        );

        let instructions: Vec<InstructionBox> = Vec::new();
        let propose = MultisigPropose::new(multisig_id.clone(), instructions, None);
        execute_propose(&mut state_transaction, &signer1_id, &propose)
            .expect("signatory propose without roles");
    }

    #[test]
    fn multisig_signatory_can_approve_without_roles() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(
            World::new(),
            kura,
            query_handle,
            ChainId::from("multisig-signatory-approve"),
        );
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();
        let domain_id: iroha_data_model::domain::DomainId = "signatory-approve".parse().unwrap();

        let signer1 = KeyPair::random();
        let signer2 = KeyPair::random();
        let signer1_id = AccountId::new(domain_id.clone(), signer1.public_key().clone());
        let signer2_id = AccountId::new(domain_id.clone(), signer2.public_key().clone());

        Register::domain(Domain::new(domain_id.clone()))
            .execute(&signer1_id, &mut state_transaction)
            .expect("domain registration");
        Register::account(iroha_data_model::account::Account::new(signer1_id.clone()))
            .execute(&signer1_id, &mut state_transaction)
            .expect("register signer1");
        Register::account(iroha_data_model::account::Account::new(signer2_id.clone()))
            .execute(&signer1_id, &mut state_transaction)
            .expect("register signer2");

        let spec = MultisigSpec {
            signatories: BTreeMap::from([(signer1_id.clone(), 1), (signer2_id.clone(), 1)]),
            quorum: NonZeroU16::new(2).unwrap(),
            transaction_ttl_ms: NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).unwrap(),
        };
        let multisig_id = register_multisig_account(
            &mut state_transaction,
            &signer1_id,
            &domain_id,
            &spec,
            "register multisig account",
        );

        let instructions: Vec<InstructionBox> = Vec::new();
        let instructions_hash = HashOf::new(&instructions);
        let propose = MultisigPropose::new(multisig_id.clone(), instructions, None);
        execute_propose(&mut state_transaction, &signer1_id, &propose).expect("signatory propose");

        let approve = MultisigApprove::new(multisig_id.clone(), instructions_hash);
        execute_approve(&mut state_transaction, &signer2_id, &approve)
            .expect("signatory approve without roles");
    }

    #[test]
    fn multisig_register_accepts_cross_domain_signatory_subjects() {
        let state = State::new_with_chain(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
            ChainId::from("multisig-cross-domain-signatories"),
        );
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();

        let multisig_domain: iroha_data_model::domain::DomainId = "multisig-home".parse().unwrap();
        let signer_domain: iroha_data_model::domain::DomainId = "signatory-remote".parse().unwrap();

        let owner = KeyPair::random();
        let signer1 = KeyPair::random();
        let signer2 = KeyPair::random();

        let owner_id = AccountId::new(multisig_domain.clone(), owner.public_key().clone());
        let signer1_remote = AccountId::new(signer_domain.clone(), signer1.public_key().clone());
        let signer2_remote = AccountId::new(signer_domain.clone(), signer2.public_key().clone());

        Register::domain(Domain::new(multisig_domain.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("register multisig domain");
        Register::domain(Domain::new(signer_domain.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("register signer domain");

        Register::account(iroha_data_model::account::Account::new(owner_id.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("register owner");
        Register::account(iroha_data_model::account::Account::new(
            signer1_remote.clone(),
        ))
        .execute(&owner_id, &mut state_transaction)
        .expect("register signer1 remote");
        Register::account(iroha_data_model::account::Account::new(
            signer2_remote.clone(),
        ))
        .execute(&owner_id, &mut state_transaction)
        .expect("register signer2 remote");
        assert_eq!(
            domain_owner(&state_transaction, &multisig_domain).expect("domain owner lookup"),
            owner_id,
            "multisig domain owner should follow registering authority",
        );

        let spec = MultisigSpec {
            signatories: BTreeMap::from([(signer1_remote.clone(), 1), (signer2_remote.clone(), 1)]),
            quorum: NonZeroU16::new(2).unwrap(),
            transaction_ttl_ms: NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).unwrap(),
        };
        let multisig_seed = AccountId::new(
            multisig_domain.clone(),
            KeyPair::random().public_key().clone(),
        );
        let register = MultisigRegister::new(multisig_seed.clone(), spec.clone());

        execute_register(&mut state_transaction, &owner_id, register)
            .expect("register multisig from cross-domain signatories");

        let signer1_scoped = AccountId::new(multisig_domain.clone(), signer1.public_key().clone());
        let signer2_scoped = AccountId::new(multisig_domain.clone(), signer2.public_key().clone());
        let registered_multisig_id = state_transaction
            .world
            .accounts_iter()
            .find(|account| account.id().multisig_policy().is_some())
            .map(|account| account.id().clone())
            .expect("registered multisig account");

        let stored_spec =
            multisig_spec(&state_transaction, &registered_multisig_id).expect("stored spec");
        assert!(
            stored_spec.signatories.contains_key(&signer1_scoped),
            "signatory subject should be normalized to the multisig domain"
        );
        assert!(
            stored_spec.signatories.contains_key(&signer2_scoped),
            "signatory subject should be normalized to the multisig domain"
        );

        let instructions: Vec<InstructionBox> = Vec::new();
        let instructions_hash = HashOf::new(&instructions);
        execute_propose(
            &mut state_transaction,
            &signer1_remote,
            &MultisigPropose::new(registered_multisig_id.clone(), instructions, None),
        )
        .expect("remote-domain signatory should be able to propose by subject");
        execute_approve(
            &mut state_transaction,
            &signer2_remote,
            &MultisigApprove::new(registered_multisig_id.clone(), instructions_hash),
        )
        .expect("remote-domain signatory should be able to approve by subject");
    }

    #[test]
    fn multisig_approval_counts_subject_once_across_domains() {
        let state = State::new_with_chain(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
            ChainId::from("multisig-unique-subject-approvals"),
        );
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();

        let home_domain: iroha_data_model::domain::DomainId = "subject-home".parse().unwrap();
        let alt_domain: iroha_data_model::domain::DomainId = "subject-alt".parse().unwrap();

        let owner = KeyPair::random();
        let shared_subject = KeyPair::random();
        let signer_b = KeyPair::random();
        let signer_c = KeyPair::random();

        let owner_id = AccountId::new(home_domain.clone(), owner.public_key().clone());
        let shared_home = AccountId::new(home_domain.clone(), shared_subject.public_key().clone());
        let shared_alt = AccountId::new(alt_domain.clone(), shared_subject.public_key().clone());
        let signer_b_id = AccountId::new(home_domain.clone(), signer_b.public_key().clone());
        let signer_c_id = AccountId::new(home_domain.clone(), signer_c.public_key().clone());

        Register::domain(Domain::new(home_domain.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("register home domain");
        Register::domain(Domain::new(alt_domain.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("register alt domain");

        for account in [
            owner_id.clone(),
            shared_home.clone(),
            shared_alt.clone(),
            signer_b_id.clone(),
            signer_c_id.clone(),
        ] {
            Register::account(iroha_data_model::account::Account::new(account))
                .execute(&owner_id, &mut state_transaction)
                .expect("register account");
        }
        assert_eq!(
            domain_owner(&state_transaction, &home_domain).expect("domain owner lookup"),
            owner_id,
            "home domain owner should follow registering authority",
        );

        let spec = MultisigSpec {
            signatories: BTreeMap::from([
                (shared_home.clone(), 1),
                (signer_b_id.clone(), 1),
                (signer_c_id.clone(), 1),
            ]),
            quorum: NonZeroU16::new(3).unwrap(),
            transaction_ttl_ms: NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).unwrap(),
        };
        let multisig_seed =
            AccountId::new(home_domain.clone(), KeyPair::random().public_key().clone());
        execute_register(
            &mut state_transaction,
            &owner_id,
            MultisigRegister::new(multisig_seed, spec.clone()),
        )
        .expect("register multisig");

        let multisig_account = state_transaction
            .world
            .accounts_iter()
            .find(|account| account.id().multisig_policy().is_some())
            .map(|account| account.id().clone())
            .expect("registered multisig account");

        let instructions: Vec<InstructionBox> = Vec::new();
        let instructions_hash = HashOf::new(&instructions);
        let loaded_spec =
            multisig_spec(&state_transaction, &multisig_account).expect("load multisig spec");
        execute_propose(
            &mut state_transaction,
            &signer_b_id,
            &MultisigPropose::new(multisig_account.clone(), instructions, None),
        )
        .expect("propose");
        let proposed = proposal_value(&state_transaction, &multisig_account, &instructions_hash)
            .expect("proposal exists after propose");
        assert_eq!(
            approved_weight_by_subject(&loaded_spec, &proposed.approvals),
            1,
            "proposer should contribute one distinct subject weight"
        );
        execute_approve(
            &mut state_transaction,
            &shared_home,
            &MultisigApprove::new(multisig_account.clone(), instructions_hash),
        )
        .expect("approve from subject home account");
        let approved_once =
            proposal_value(&state_transaction, &multisig_account, &instructions_hash)
                .expect("proposal exists after first subject approval");
        assert_eq!(
            approved_weight_by_subject(&loaded_spec, &approved_once.approvals),
            2,
            "home-domain subject approval should increase distinct subject weight"
        );
        execute_approve(
            &mut state_transaction,
            &shared_alt,
            &MultisigApprove::new(multisig_account.clone(), instructions_hash),
        )
        .expect("approve from subject alt account");
        let approved_twice =
            proposal_value(&state_transaction, &multisig_account, &instructions_hash)
                .expect("proposal should persist after duplicate-subject approval");
        assert_eq!(
            approved_weight_by_subject(&loaded_spec, &approved_twice.approvals),
            2,
            "same subject approving from another domain must not satisfy quorum twice"
        );

        execute_approve(
            &mut state_transaction,
            &signer_c_id,
            &MultisigApprove::new(multisig_account.clone(), instructions_hash),
        )
        .expect("approve from third signatory");

        assert!(
            matches!(
                proposal_value(&state_transaction, &multisig_account, &instructions_hash),
                Err(ValidationFail::QueryFailed(QueryExecutionFail::Find(
                    FindError::MetadataKey(_)
                )))
            ),
            "proposal should be pruned after quorum is reached by distinct subjects"
        );
    }

    #[test]
    fn multisig_signatories_must_be_single_accounts() {
        let state = State::new_with_chain(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
            ChainId::from("multisig-signatories-single"),
        );
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();
        let domain_id: iroha_data_model::domain::DomainId = "signatory-single".parse().unwrap();

        let (owner, leaf_a, leaf_b) = (KeyPair::random(), KeyPair::random(), KeyPair::random());

        let owner_id = AccountId::new(domain_id.clone(), owner.public_key().clone());
        let first_leaf_account_id = AccountId::new(domain_id.clone(), leaf_a.public_key().clone());
        let second_leaf_account_id = AccountId::new(domain_id.clone(), leaf_b.public_key().clone());

        Register::domain(Domain::new(domain_id.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("domain registration");
        for (account_id, label) in [
            (owner_id.clone(), "register owner"),
            (first_leaf_account_id.clone(), "register leaf a"),
            (second_leaf_account_id.clone(), "register leaf b"),
        ] {
            Register::account(iroha_data_model::account::Account::new(account_id))
                .execute(&owner_id, &mut state_transaction)
                .expect(label);
        }

        let child_spec = MultisigSpec {
            signatories: BTreeMap::from([
                (first_leaf_account_id.clone(), 1),
                (second_leaf_account_id.clone(), 1),
            ]),
            quorum: NonZeroU16::new(2).unwrap(),
            transaction_ttl_ms: NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).unwrap(),
        };
        let child_id = register_multisig_account(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &child_spec,
            "register child multisig account",
        );

        let parent_spec = MultisigSpec {
            signatories: BTreeMap::from([(owner_id.clone(), 1), (child_id.clone(), 1)]),
            quorum: NonZeroU16::new(2).unwrap(),
            transaction_ttl_ms: NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).unwrap(),
        };
        let parent_key = KeyPair::random();
        let parent_id = AccountId::new(domain_id.clone(), parent_key.public_key().clone());
        let register = MultisigRegister::new(parent_id, parent_spec);
        let err = Executor::Initial
            .execute_instruction(
                &mut state_transaction,
                &owner_id,
                InstructionBox::from(register),
            )
            .expect_err("multisig signatory must be single");
        match err {
            ValidationFail::NotPermitted(msg) => {
                assert!(
                    msg.contains("single-key account"),
                    "unexpected error message: {msg}"
                );
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn multisig_spec_missing_metadata_returns_error() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(
            World::new(),
            kura,
            query_handle,
            ChainId::from("multisig-missing-spec"),
        );
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();
        let domain_id: iroha_data_model::domain::DomainId = "missing".parse().unwrap();

        let owner_key = KeyPair::random();
        let owner_id = AccountId::new(domain_id.clone(), owner_key.public_key().clone());
        Register::domain(Domain::new(domain_id.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("domain registration");
        Register::account(iroha_data_model::account::Account::new(owner_id.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("register owner");

        let err = multisig_spec(&state_transaction, &owner_id)
            .expect_err("missing multisig spec should error");
        match err {
            ValidationFail::QueryFailed(QueryExecutionFail::Find(FindError::MetadataKey(_))) => {}
            other => panic!("unexpected error for missing multisig spec: {other:?}"),
        }
    }

    #[test]
    fn multisig_role_for_large_policy_uses_hash_suffix() {
        let domain_id: iroha_data_model::domain::DomainId = "weights".parse().unwrap();
        let member_count = (u8::MAX as usize) + 1;
        let mut members = Vec::with_capacity(member_count);
        for _ in 0..member_count {
            let key = KeyPair::random();
            let member = MultisigMember::new(key.public_key().clone(), 1).expect("multisig member");
            members.push(member);
        }
        let policy = MultisigPolicy::new(1, members).expect("multisig policy");
        let account = AccountId::new_multisig(domain_id, policy);
        assert!(
            account.canonical_ih58().is_err(),
            "large multisig policy should not encode into IH58"
        );

        let role_id = multisig_role_for(&account);
        let role_name = role_id.name().to_string();
        let expected_suffix = HashOf::new(&account).to_string();
        assert!(
            role_name.ends_with(&expected_suffix),
            "role name should use hash suffix for large multisig policy"
        );
    }

    #[test]
    fn multisig_approval_weight_sum_does_not_overflow() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(
            World::new(),
            kura,
            query_handle,
            ChainId::from("multisig-weight-overflow"),
        );
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();
        let domain_id: iroha_data_model::domain::DomainId = "weights".parse().unwrap();

        let owner_key = KeyPair::random();
        let owner_id = AccountId::new(domain_id.clone(), owner_key.public_key().clone());
        Register::domain(Domain::new(domain_id.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("domain registration");
        Register::account(iroha_data_model::account::Account::new(owner_id.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("register owner");

        let weight = u8::MAX;
        let signatory_count = (u16::MAX as usize / weight as usize) + 1;
        let mut signatories = BTreeMap::new();
        for _ in 0..signatory_count {
            let signer_key = KeyPair::random();
            let signer_id = AccountId::new(domain_id.clone(), signer_key.public_key().clone());
            Register::account(iroha_data_model::account::Account::new(signer_id.clone()))
                .execute(&owner_id, &mut state_transaction)
                .expect("register signatory");
            signatories.insert(signer_id, weight);
        }

        let spec = MultisigSpec {
            signatories: signatories.clone(),
            quorum: NonZeroU16::new(u16::MAX).unwrap(),
            transaction_ttl_ms: NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).unwrap(),
        };
        let multisig_id = register_multisig_account(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &spec,
            "register multisig account",
        );

        let instructions: Vec<InstructionBox> = Vec::new();
        let instructions_hash = HashOf::new(&instructions);
        let proposer = signatories
            .keys()
            .next()
            .expect("signatories present")
            .clone();
        let proposal = MultisigPropose::new(multisig_id.clone(), instructions, None);
        Executor::Initial
            .execute_instruction(
                &mut state_transaction,
                &proposer,
                InstructionBox::from(proposal),
            )
            .expect("multisig propose");

        let mut seeded_value = proposal_value(&state_transaction, &multisig_id, &instructions_hash)
            .expect("proposal value");
        seeded_value.approvals = signatories.keys().cloned().collect();
        SetKeyValue::account(
            multisig_id.clone(),
            proposal_key(&instructions_hash),
            seeded_value,
        )
        .execute(&multisig_id, &mut state_transaction)
        .expect("seed approvals");

        let approver = signatories
            .keys()
            .next_back()
            .expect("signatories present")
            .clone();
        let approve = MultisigApprove::new(multisig_id.clone(), instructions_hash);
        Executor::Initial
            .execute_instruction(
                &mut state_transaction,
                &approver,
                InstructionBox::from(approve),
            )
            .expect("multisig approve");

        assert!(
            proposal_value(&state_transaction, &multisig_id, &instructions_hash).is_err(),
            "proposal should be pruned after reaching quorum"
        );
    }
}
