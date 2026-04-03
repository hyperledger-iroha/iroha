//! Built-in handling for multisig instructions without requiring an executor upgrade.

use std::{
    collections::{BTreeMap, BTreeSet},
    str::FromStr,
    sync::LazyLock,
};

use iroha_crypto::HashOf;
use iroha_data_model::{
    ValidationFail,
    account::{AccountId, MultisigMember, MultisigPolicy, rekey::AccountRekeyRecord},
    isi::{
        AddSignatory, CustomInstruction, InstructionBox, RemoveSignatory, SetAccountQuorum,
        error::{InstructionExecutionError, InvalidParameterError},
    },
    metadata::Metadata,
    name::Name,
    prelude::{Grant, Json, Level, Log, Register, Revoke},
    query::error::{FindError, QueryExecutionFail},
    role::{Role, RoleId},
};
use iroha_executor_data_model::isi::multisig::{
    DEFAULT_MULTISIG_TTL_MS, MultisigAccountState, MultisigApprove, MultisigCancel,
    MultisigInstructionBox, MultisigProposalState, MultisigProposalTerminalState,
    MultisigProposalTerminalStatus, MultisigProposalValue, MultisigPropose, MultisigRegister,
    MultisigSpec,
};
use mv::storage::StorageReadOnly;

use crate::{
    smartcontracts::Execute,
    smartcontracts::isi::domain::isi::ensure_controller_capabilities,
    state::{StateTransaction, WorldReadOnly},
};

const DELIMITER: char = '/';
const MULTISIG: &str = "multisig";
const MULTISIG_ACCOUNT_STATE: &str = "account";
const MULTISIG_PROPOSAL_STATE: &str = "proposal";
const MULTISIG_PROPOSAL_TERMINAL_STATE: &str = "proposal-terminal";
const MULTISIG_SIGNATORY_INDEX_STATE: &str = "signatory";
const MULTISIG_SIGNATORY: &str = "MULTISIG_SIGNATORY";
const DOMAINLESS_NAMESPACE: &str = "domainless";
static MULTISIG_CREATED_VIA_KEY: LazyLock<Name> = LazyLock::new(|| {
    "iroha:created_via"
        .parse()
        .expect("multisig created_via metadata key must be valid")
});
static MULTISIG_HOME_DOMAIN_KEY: LazyLock<Name> = LazyLock::new(|| {
    "iroha:multisig_home_domain"
        .parse()
        .expect("multisig home-domain metadata key must be valid")
});
static MULTISIG_PROPOSAL_METADATA_PREFIX: LazyLock<String> =
    LazyLock::new(|| format!("{MULTISIG}{DELIMITER}proposals{DELIMITER}"));

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
        MultisigInstructionBox::Cancel(instruction) => {
            execute_cancel(state_transaction, authority, &instruction)
        }
    }
}

pub(crate) fn is_reserved_multisig_metadata_key(key: &Name) -> bool {
    let literal = key.as_ref();
    literal == spec_key().as_ref()
        || literal == home_domain_key().as_ref()
        || literal.starts_with(MULTISIG_PROPOSAL_METADATA_PREFIX.as_str())
}

impl Execute for AddSignatory {
    fn execute(
        self,
        _authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let AddSignatory {
            account: input_account,
            signatory,
        } = self;
        let account = resolve_account_for_instruction(state_transaction, &input_account)?;
        let home_domain =
            multisig_home_domain(state_transaction, &account).map_err(map_validation_fail)?;
        let previous_account_state =
            load_multisig_account_state_optional(state_transaction, &account)
                .map_err(map_validation_fail)?;
        let mut spec = multisig_spec_strict(state_transaction, &account)?;
        let signatory_account = AccountId::new(signatory);
        if spec_contains_signatory_subject(&spec, &signatory_account) {
            return Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(format!(
                    "signatory `{signatory_account}` already present in multisig spec for `{account}`"
                )),
            ));
        }
        spec.signatories.insert(signatory_account.clone(), 1);
        validate_registration(state_transaction, &account, &spec).map_err(map_validation_fail)?;
        let updated_account =
            rekey_multisig_account(state_transaction, &account, home_domain.as_ref(), &spec)?;
        persist_multisig_account_state(
            state_transaction,
            previous_account_state.as_ref(),
            &MultisigAccountState::new(updated_account.clone(), home_domain.clone(), spec.clone()),
        )
        .map_err(map_validation_fail)?;
        materialize_missing_signatory_accounts(
            state_transaction,
            home_domain.as_ref(),
            &updated_account,
            &spec,
        )
        .map_err(map_validation_fail)?;
        let role_owner = if let Some(home_domain) = home_domain.as_ref() {
            domain_owner(state_transaction, home_domain).map_err(map_validation_fail)?
        } else {
            updated_account.clone()
        };
        configure_roles(
            state_transaction,
            &role_owner,
            home_domain.as_ref(),
            &updated_account,
            &spec,
        )
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
        let RemoveSignatory {
            account: input_account,
            signatory,
        } = self;
        let account = resolve_account_for_instruction(state_transaction, &input_account)?;
        let home_domain =
            multisig_home_domain(state_transaction, &account).map_err(map_validation_fail)?;
        let previous_account_state =
            load_multisig_account_state_optional(state_transaction, &account)
                .map_err(map_validation_fail)?;
        let mut spec = multisig_spec_strict(state_transaction, &account)?;
        let signatory_candidate = AccountId::new(signatory);
        let Some(signatory_account) = spec
            .signatories
            .keys()
            .find(|existing| existing.subject_id() == signatory_candidate.subject_id())
            .cloned()
        else {
            return Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(format!(
                    "signatory `{signatory_candidate}` not present in multisig spec for `{account}`"
                )),
            ));
        };
        let _removed = spec.signatories.remove(&signatory_account);
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
        let updated_account =
            rekey_multisig_account(state_transaction, &account, home_domain.as_ref(), &spec)?;
        persist_multisig_account_state(
            state_transaction,
            previous_account_state.as_ref(),
            &MultisigAccountState::new(updated_account.clone(), home_domain.clone(), spec.clone()),
        )
        .map_err(map_validation_fail)?;
        let resolved_signatory_account =
            resolve_signatory_account(state_transaction, &signatory_account)
                .map_err(map_validation_fail)?;

        let multisig_role_id = multisig_role_for(home_domain.as_ref(), &updated_account);
        if has_role(
            state_transaction,
            &resolved_signatory_account,
            &multisig_role_id,
        )
        .map_err(map_validation_fail)?
        {
            Revoke::account_role(multisig_role_id.clone(), resolved_signatory_account.clone())
                .execute(authority, state_transaction)?;
        }
        let signatory_role_id =
            multisig_role_for(home_domain.as_ref(), &resolved_signatory_account);
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
        _authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let SetAccountQuorum {
            account: input_account,
            quorum,
        } = self;
        let account = resolve_account_for_instruction(state_transaction, &input_account)?;
        let home_domain =
            multisig_home_domain(state_transaction, &account).map_err(map_validation_fail)?;
        let previous_account_state =
            load_multisig_account_state_optional(state_transaction, &account)
                .map_err(map_validation_fail)?;
        let mut spec = multisig_spec_strict(state_transaction, &account)?;
        spec.quorum = quorum;
        validate_registration(state_transaction, &account, &spec).map_err(map_validation_fail)?;
        let updated_account =
            rekey_multisig_account(state_transaction, &account, home_domain.as_ref(), &spec)?;
        persist_multisig_account_state(
            state_transaction,
            previous_account_state.as_ref(),
            &MultisigAccountState::new(updated_account, home_domain, spec),
        )
        .map_err(map_validation_fail)?;
        Ok(())
    }
}

pub(crate) fn spec_key() -> Name {
    Name::from_str(&format!("{MULTISIG}{DELIMITER}spec"))
        .expect("constant string must be a valid name")
}

fn home_domain_key() -> Name {
    (*MULTISIG_HOME_DOMAIN_KEY).clone()
}

pub(crate) fn multisig_account_state_key(account: &AccountId) -> Name {
    Name::from_str(&format!(
        "{MULTISIG}{DELIMITER}{MULTISIG_ACCOUNT_STATE}{DELIMITER}{}",
        HashOf::new(account)
    ))
    .expect("multisig account state path must be a valid name")
}

fn multisig_signatory_index_key(signatory: &AccountId) -> Name {
    Name::from_str(&format!(
        "{MULTISIG}{DELIMITER}{MULTISIG_SIGNATORY_INDEX_STATE}{DELIMITER}{}",
        HashOf::new(&signatory.subject_id())
    ))
    .expect("multisig signatory state path must be a valid name")
}

fn multisig_proposal_state_prefix(account: &AccountId) -> Name {
    Name::from_str(&format!(
        "{MULTISIG}{DELIMITER}{MULTISIG_PROPOSAL_STATE}{DELIMITER}{}{DELIMITER}",
        HashOf::new(account)
    ))
    .expect("multisig proposal state prefix must be a valid name")
}

fn multisig_proposal_state_key(
    multisig_account: &AccountId,
    instructions_hash: &HashOf<Vec<InstructionBox>>,
) -> Name {
    Name::from_str(&format!(
        "{}{}",
        multisig_proposal_state_prefix(multisig_account),
        instructions_hash
    ))
    .expect("constant string must be a valid name")
}

fn multisig_proposal_terminal_state_prefix(account: &AccountId) -> Name {
    Name::from_str(&format!(
        "{MULTISIG}{DELIMITER}{MULTISIG_PROPOSAL_TERMINAL_STATE}{DELIMITER}{}{DELIMITER}",
        HashOf::new(account)
    ))
    .expect("multisig proposal terminal state prefix must be a valid name")
}

fn multisig_proposal_terminal_state_key(
    multisig_account: &AccountId,
    instructions_hash: &HashOf<Vec<InstructionBox>>,
) -> Name {
    Name::from_str(&format!(
        "{}{}",
        multisig_proposal_terminal_state_prefix(multisig_account),
        instructions_hash
    ))
    .expect("multisig proposal terminal state path must be a valid name")
}

fn account_role_suffix(account: &AccountId) -> String {
    const MAX_CANONICAL_SUFFIX_LEN: usize = 128;
    if let Ok(canonical_suffix) = account.canonical_i105() {
        if canonical_suffix.len() <= MAX_CANONICAL_SUFFIX_LEN {
            return canonical_suffix;
        }
    }
    HashOf::new(account).to_string()
}

fn multisig_role_for(
    home_domain: Option<&iroha_data_model::domain::DomainId>,
    account: &AccountId,
) -> RoleId {
    let suffix = account_role_suffix(account);
    let literal = if let Some(home_domain) = home_domain {
        format!(
            "{MULTISIG_SIGNATORY}{DELIMITER}{}{DELIMITER}{}",
            home_domain, suffix,
        )
    } else {
        format!("{MULTISIG_SIGNATORY}{DELIMITER}{DOMAINLESS_NAMESPACE}{DELIMITER}{suffix}")
    };
    literal.parse().expect("multisig role name must be valid")
}

fn rekey_multisig_account(
    state_transaction: &mut StateTransaction<'_, '_>,
    account: &AccountId,
    home_domain: Option<&iroha_data_model::domain::DomainId>,
    spec: &MultisigSpec,
) -> Result<AccountId, InstructionExecutionError> {
    ensure_signatories_are_single(spec).map_err(map_validation_fail)?;
    let policy = multisig_policy_from_spec(spec)?;
    let updated_account = AccountId::new_multisig(policy);
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

    rekey_account_id(state_transaction, account, &updated_account, home_domain)?;
    state_transaction
        .world
        .smart_contract_state
        .remove(multisig_account_state_key(account));
    move_multisig_proposals(state_transaction, account, &updated_account)
        .map_err(map_validation_fail)?;
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
    home_domain: Option<&iroha_data_model::domain::DomainId>,
) -> Result<(), InstructionExecutionError> {
    if state_transaction.world.accounts.get(new_account).is_some() {
        return Err(InstructionExecutionError::InvariantViolation(
            format!("account `{new_account}` already exists").into(),
        ));
    }

    let account_value = state_transaction
        .world
        .accounts
        .remove(old_account.clone())
        .ok_or_else(|| InstructionExecutionError::Find(FindError::Account(old_account.clone())))?;

    state_transaction
        .world
        .accounts
        .insert(new_account.clone(), account_value.clone());

    let mut labels_to_repoint: BTreeSet<_> = state_transaction
        .world
        .account_aliases_by_account
        .get(old_account)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .collect();
    labels_to_repoint.extend(
        state_transaction
            .world
            .account_rekey_records
            .view()
            .iter()
            .filter(|(_, record)| &record.active_account_id == old_account)
            .map(|(label, _)| label.clone()),
    );
    if let Some(label) = account_value.label().cloned() {
        labels_to_repoint.insert(label);
    }

    for label in labels_to_repoint {
        state_transaction
            .world
            .insert_account_alias_binding(label.clone(), new_account.clone());
        let record = match state_transaction
            .world
            .account_rekey_records
            .get(&label)
            .cloned()
        {
            Some(record) => record.repoint_to_account(new_account.clone()),
            None => AccountRekeyRecord::new(label.clone(), new_account.clone()),
        };
        state_transaction
            .world
            .account_rekey_records
            .insert(label, record);
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

    let old_multisig_role = multisig_role_for(home_domain, old_account);
    let new_multisig_role = multisig_role_for(home_domain, new_account);
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
        let new_asset_id = iroha_data_model::asset::AssetId::with_scope(
            asset_id.definition().clone(),
            new_account.clone(),
            *asset_id.scope(),
        );
        if state_transaction.world.assets.get(&new_asset_id).is_some() {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("asset `{new_asset_id}` already exists").into(),
            ));
        }
        if let Some(value) = state_transaction.world.assets.remove(asset_id.clone()) {
            state_transaction
                .world
                .untrack_asset_holder_if_empty(&asset_id);
            state_transaction
                .world
                .assets
                .insert(new_asset_id.clone(), value);
            state_transaction.world.track_asset_holder(&new_asset_id);
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

pub(crate) fn replace_account_controller(
    authority: &AccountId,
    state_transaction: &mut StateTransaction<'_, '_>,
    old_account: &AccountId,
    new_controller: iroha_data_model::account::AccountController,
) -> Result<AccountId, InstructionExecutionError> {
    let new_account = match new_controller {
        iroha_data_model::account::AccountController::Single(signatory) => {
            AccountId::new(signatory)
        }
        iroha_data_model::account::AccountController::Multisig(policy) => {
            AccountId::new_multisig(policy)
        }
    };
    ensure_controller_capabilities(
        new_account.controller(),
        &state_transaction.crypto.allowed_signing,
        &state_transaction.crypto.allowed_curve_ids,
    )?;

    if &new_account == old_account {
        return Err(InstructionExecutionError::InvalidParameter(
            InvalidParameterError::SmartContract(format!(
                "replacement controller for `{old_account}` must change the canonical account id"
            )),
        ));
    }

    if account_exists(state_transaction, &new_account).map_err(map_validation_fail)? {
        return Err(InstructionExecutionError::InvariantViolation(
            format!("account `{new_account}` already exists").into(),
        ));
    }

    let previous_state = load_multisig_account_state_optional(state_transaction, old_account)
        .and_then(|state| match state {
            Some(state) => Ok(Some(state)),
            None => reconstruct_multisig_account_state(state_transaction, old_account),
        })
        .map_err(map_validation_fail)?;
    let home_domain = previous_state
        .as_ref()
        .and_then(|state| state.home_domain.clone());

    rekey_account_id(
        state_transaction,
        old_account,
        &new_account,
        home_domain.as_ref(),
    )?;

    if previous_state.is_some() {
        state_transaction
            .world
            .smart_contract_state
            .remove(multisig_account_state_key(old_account));
        move_multisig_proposals(state_transaction, old_account, &new_account)
            .map_err(map_validation_fail)?;
    }

    let next_state = if let Some(policy) = new_account.multisig_policy() {
        Some(
            multisig_state_from_policy(
                state_transaction,
                &new_account,
                home_domain.clone(),
                policy,
            )
            .map_err(map_validation_fail)?,
        )
    } else {
        None
    };

    reconcile_multisig_transition(
        authority,
        state_transaction,
        &new_account,
        previous_state.as_ref(),
        next_state.as_ref(),
    )
    .map_err(map_validation_fail)?;

    Ok(new_account)
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
        iroha_data_model::asset::AssetId::with_scope(
            asset_id.definition().clone(),
            new.clone(),
            *asset_id.scope(),
        )
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

fn reconcile_multisig_transition(
    authority: &AccountId,
    state_transaction: &mut StateTransaction<'_, '_>,
    active_account: &AccountId,
    previous_state: Option<&MultisigAccountState>,
    next_state: Option<&MultisigAccountState>,
) -> Result<(), ValidationFail> {
    let home_domain = previous_state
        .and_then(|state| state.home_domain.clone())
        .or_else(|| next_state.and_then(|state| state.home_domain.clone()));

    let previous_members = previous_state
        .map(|state| resolved_signatory_accounts(state_transaction, &state.spec))
        .transpose()?
        .unwrap_or_default();
    let next_members = next_state
        .map(|state| resolved_signatory_accounts(state_transaction, &state.spec))
        .transpose()?
        .unwrap_or_default();

    if previous_state.is_some() {
        let multisig_role_id = multisig_role_for(home_domain.as_ref(), active_account);

        for removed in previous_members
            .iter()
            .filter(|candidate| !next_members.contains(candidate))
        {
            revoke_role_if_present(state_transaction, &multisig_role_id, removed, authority)?;
            let signatory_role_id = multisig_role_for(home_domain.as_ref(), removed);
            revoke_role_if_present(
                state_transaction,
                &signatory_role_id,
                active_account,
                authority,
            )?;
        }

        if next_state.is_none() {
            revoke_role_if_present(
                state_transaction,
                &multisig_role_id,
                active_account,
                authority,
            )?;
            sync_multisig_signatory_index(state_transaction, previous_state, None)?;
            clear_multisig_account_metadata(state_transaction, active_account)
                .map_err(map_find_error)?;
        }
    }

    if let Some(next_state) = next_state {
        persist_multisig_account_state(state_transaction, previous_state, next_state)?;
        let role_owner = if let Some(home_domain) = next_state.home_domain.as_ref() {
            domain_owner(state_transaction, home_domain)?
        } else {
            next_state.account_id.clone()
        };
        configure_roles(
            state_transaction,
            &role_owner,
            next_state.home_domain.as_ref(),
            &next_state.account_id,
            &next_state.spec,
        )?;
    }

    Ok(())
}

fn clear_multisig_account_metadata(
    state_transaction: &mut StateTransaction<'_, '_>,
    account: &AccountId,
) -> Result<(), FindError> {
    let account = state_transaction.world.account_mut(account)?;
    let _ = account.remove(&spec_key());
    let _ = account.remove(&home_domain_key());
    Ok(())
}

fn revoke_role_if_present(
    state_transaction: &mut StateTransaction<'_, '_>,
    role_id: &RoleId,
    account: &AccountId,
    authority: &AccountId,
) -> Result<(), ValidationFail> {
    if has_role(state_transaction, account, role_id)? {
        Revoke::account_role(role_id.clone(), account.clone())
            .execute(authority, state_transaction)
            .map_err(ValidationFail::InstructionFailed)?;
    }
    Ok(())
}

fn multisig_state_from_policy(
    state_transaction: &mut StateTransaction<'_, '_>,
    multisig_account: &AccountId,
    home_domain: Option<iroha_data_model::domain::DomainId>,
    policy: &MultisigPolicy,
) -> Result<MultisigAccountState, ValidationFail> {
    let spec = multisig_spec_from_policy(multisig_account, policy)?;
    materialize_missing_signatory_accounts(
        state_transaction,
        home_domain.as_ref(),
        multisig_account,
        &spec,
    )?;

    Ok(MultisigAccountState::new(
        multisig_account.clone(),
        home_domain,
        spec,
    ))
}

fn multisig_spec_from_policy(
    multisig_account: &AccountId,
    policy: &MultisigPolicy,
) -> Result<MultisigSpec, ValidationFail> {
    let mut signatories = BTreeMap::new();
    for member in policy.members() {
        let signatory_account = AccountId::new(member.public_key().clone());
        let weight = u8::try_from(member.weight()).map_err(|_| {
            ValidationFail::QueryFailed(QueryExecutionFail::Conversion(format!(
                "multisig member weight {} exceeds u8 for `{multisig_account}`",
                member.weight()
            )))
        })?;
        signatories.insert(signatory_account, weight);
    }

    let quorum = std::num::NonZeroU16::new(policy.threshold()).ok_or_else(|| {
        ValidationFail::QueryFailed(QueryExecutionFail::Conversion(format!(
            "multisig threshold is zero for `{multisig_account}`"
        )))
    })?;

    let transaction_ttl_ms = std::num::NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS)
        .expect("default multisig ttl must be non-zero");

    Ok(MultisigSpec {
        signatories,
        quorum,
        transaction_ttl_ms,
    })
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
    let MultisigRegister {
        account: multisig_account_id,
        home_domain,
        spec,
    } = instruction;
    validate_registration(state_transaction, &multisig_account_id, &spec)?;

    if account_exists(state_transaction, &multisig_account_id)? {
        return Err(ValidationFail::NotPermitted(format!(
            "multisig account `{multisig_account_id}` already exists"
        )));
    }

    let register_account = iroha_data_model::account::NewAccount::new(multisig_account_id.clone());
    Register::account(register_account)
        .execute(authority, state_transaction)
        .map_err(ValidationFail::InstructionFailed)?;

    let updated_account = rekey_multisig_account(
        state_transaction,
        &multisig_account_id,
        home_domain.as_ref(),
        &spec,
    )
    .map_err(ValidationFail::InstructionFailed)?;
    persist_multisig_account_state(
        state_transaction,
        None,
        &MultisigAccountState::new(updated_account.clone(), home_domain.clone(), spec.clone()),
    )?;
    let role_owner = if let Some(home_domain) = home_domain.as_ref() {
        domain_owner(state_transaction, home_domain)?
    } else {
        updated_account.clone()
    };
    materialize_missing_signatory_accounts(
        state_transaction,
        home_domain.as_ref(),
        &updated_account,
        &spec,
    )?;
    configure_roles(
        state_transaction,
        &role_owner,
        home_domain.as_ref(),
        &updated_account,
        &spec,
    )?;

    Ok(())
}

fn execute_propose(
    state_transaction: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    instruction: &MultisigPropose,
) -> Result<(), ValidationFail> {
    let proposer = authority.clone();
    let multisig_account = resolve_signatory_account(state_transaction, &instruction.account)?;
    ensure_multisig_account_state_materialized(state_transaction, &multisig_account)?;
    let home_domain = multisig_home_domain(state_transaction, &multisig_account)?;
    let instructions_hash = HashOf::new(&instruction.instructions);
    let multisig_spec = multisig_spec(state_transaction, &multisig_account)?;
    let proposer_role = multisig_role_for(home_domain.as_ref(), &proposer);
    let multisig_role = multisig_role_for(home_domain.as_ref(), &multisig_account);
    let is_downward_proposal = state_transaction
        .world
        .account_roles_iter(&multisig_account)
        .any(|role| role == &proposer_role);
    let has_multisig_role = state_transaction
        .world
        .account_roles_iter(&proposer)
        .any(|role| role == &multisig_role);
    let is_signatory = spec_contains_signatory_subject(&multisig_spec, &proposer);
    let is_self_proposal = proposer.subject_id() == multisig_account.subject_id();
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

    match proposal_state(state_transaction, &multisig_account, &instructions_hash) {
        Ok(existing) if now_ms(state_transaction) < existing.expires_at_ms => {
            return Err(ValidationFail::NotPermitted(
                "multisig proposal duplicates".to_owned(),
            ));
        }
        Ok(_) => {}
        Err(ValidationFail::QueryFailed(QueryExecutionFail::NotFound)) => {}
        Err(err) => return Err(err),
    }

    let now_ms = now_ms(state_transaction);
    if proposal_state(state_transaction, &multisig_account, &instructions_hash).is_ok() {
        prune_expired(state_transaction, &multisig_account, &instructions_hash)?;
    }
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

    store_multisig_proposal_state(
        state_transaction,
        &MultisigProposalState::new(
            multisig_account,
            instructions_hash,
            proposal_value.instructions,
            proposal_value.proposed_at_ms,
            proposal_value.expires_at_ms,
            proposal_value.approvals,
            proposal_value.is_relayed,
        ),
    )
}

fn execute_approve(
    state_transaction: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    instruction: &MultisigApprove,
) -> Result<(), ValidationFail> {
    let approver = authority.clone();
    let multisig_account = resolve_signatory_account(state_transaction, &instruction.account)?;
    ensure_multisig_account_state_materialized(state_transaction, &multisig_account)?;
    let home_domain = multisig_home_domain(state_transaction, &multisig_account)?;
    let instructions_hash = instruction.instructions_hash;

    let spec = multisig_spec(state_transaction, &multisig_account)?;
    let has_multisig_role = state_transaction
        .world
        .account_roles_iter(&approver)
        .any(|role| role == &multisig_role_for(home_domain.as_ref(), &multisig_account));
    let is_signatory = spec_contains_signatory_subject(&spec, &approver);
    let is_self_approval = approver.subject_id() == multisig_account.subject_id();

    if !(has_multisig_role || is_signatory || is_self_approval) {
        return Err(ValidationFail::NotPermitted(
            "not qualified to approve multisig".to_owned(),
        ));
    }
    prune_expired(state_transaction, &multisig_account, &instructions_hash)?;

    let Ok(mut proposal_state) =
        proposal_state(state_transaction, &multisig_account, &instructions_hash)
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
    if let Some(true) = proposal_state.is_relayed {
        return Ok(());
    }

    upsert_subject_approval(&mut proposal_state.approvals, approver);
    iroha_logger::info!(
        multisig_account = %multisig_account,
        instructions_hash = %instructions_hash,
        approvals = proposal_state.approvals.len(),
        "multisig approval storing updated proposal state"
    );
    store_multisig_proposal_state(state_transaction, &proposal_state)?;
    iroha_logger::info!(
        multisig_account = %multisig_account,
        instructions_hash = %instructions_hash,
        "multisig approval stored updated proposal state"
    );

    let approved_weight = approved_weight_by_subject(&spec, &proposal_state.approvals);
    let is_authenticated = approved_weight >= u32::from(spec.quorum.get());
    iroha_logger::info!(
        multisig_account = %multisig_account,
        instructions_hash = %instructions_hash,
        approved_weight,
        quorum = u32::from(spec.quorum.get()),
        is_authenticated,
        "multisig approval evaluated quorum"
    );

    if is_authenticated {
        match proposal_state.is_relayed {
            None => {
                iroha_logger::info!(
                    multisig_account = %multisig_account,
                    instructions_hash = %instructions_hash,
                    "multisig approval pruning proposal tree"
                );
                maybe_store_terminal_proposal_state(
                    state_transaction,
                    &proposal_state,
                    MultisigProposalTerminalStatus::Finalized,
                )?;
                prune_down(state_transaction, &multisig_account, &instructions_hash)?;
                iroha_logger::info!(
                    multisig_account = %multisig_account,
                    instructions_hash = %instructions_hash,
                    "multisig approval pruned proposal tree"
                );
            }
            Some(false) => {
                proposal_state.is_relayed = Some(true);
                store_multisig_proposal_state(state_transaction, &proposal_state)?;
            }
            _ => unreachable!("proposal_state.is_relayed checked above"),
        }

        for instruction in proposal_state.instructions {
            iroha_logger::info!(
                multisig_account = %multisig_account,
                instructions_hash = %instructions_hash,
                approver = %authority,
                instruction = ?instruction,
                "multisig approval executing authenticated instruction"
            );
            if let Ok(multisig) = MultisigInstructionBox::try_from(&instruction) {
                execute_multisig_instruction(state_transaction, &multisig_account, multisig)?;
            } else {
                instruction
                    .execute(&multisig_account, state_transaction)
                    .map_err(ValidationFail::from)?;
            }
            iroha_logger::info!(
                multisig_account = %multisig_account,
                instructions_hash = %instructions_hash,
                approver = %authority,
                "multisig approval finished authenticated instruction"
            );
        }
    }

    Ok(())
}

fn canceler_is_authorized(multisig_account: &AccountId, canceler: &AccountId) -> bool {
    canceler.subject_id() == multisig_account.subject_id()
}

fn execute_cancel(
    state_transaction: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    instruction: &MultisigCancel,
) -> Result<(), ValidationFail> {
    let canceler = authority.clone();
    let multisig_account = resolve_signatory_account(state_transaction, &instruction.account)?;
    ensure_multisig_account_state_materialized(state_transaction, &multisig_account)?;
    let instructions_hash = instruction.instructions_hash;

    if !canceler_is_authorized(&multisig_account, &canceler) {
        return Err(ValidationFail::NotPermitted(
            "multisig cancel must execute as the multisig account".to_owned(),
        ));
    }

    prune_expired(state_transaction, &multisig_account, &instructions_hash)?;

    let proposal_state = proposal_state(state_transaction, &multisig_account, &instructions_hash)?;
    if let Some(true) = proposal_state.is_relayed {
        return Err(ValidationFail::NotPermitted(
            "cannot cancel an executed relayed approval".to_owned(),
        ));
    }

    maybe_store_terminal_proposal_state(
        state_transaction,
        &proposal_state,
        MultisigProposalTerminalStatus::Canceled,
    )?;
    prune_down(state_transaction, &multisig_account, &instructions_hash)
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
    store_multisig_proposal_state(
        state_transaction,
        &MultisigProposalState::new(
            relayer.clone(),
            relay_hash,
            relay_value.instructions,
            relay_value.proposed_at_ms,
            relay_value.expires_at_ms,
            relay_value.approvals,
            relay_value.is_relayed,
        ),
    )
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
    let proposal_state = proposal_state(state_transaction, multisig_account, instructions_hash)?;

    if now_ms(state_transaction) < proposal_state.expires_at_ms {
        return Ok(());
    }

    for instruction in &proposal_state.instructions {
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

    maybe_store_terminal_proposal_state(
        state_transaction,
        &proposal_state,
        MultisigProposalTerminalStatus::Expired,
    )?;
    prune_down(state_transaction, multisig_account, instructions_hash)
}

fn prune_down(
    state_transaction: &mut StateTransaction<'_, '_>,
    multisig_account: &AccountId,
    instructions_hash: &HashOf<Vec<InstructionBox>>,
) -> Result<(), ValidationFail> {
    let spec = multisig_spec(state_transaction, multisig_account)?;

    state_transaction
        .world
        .smart_contract_state
        .remove(multisig_proposal_state_key(
            multisig_account,
            instructions_hash,
        ));

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
    ensure_signatories_are_single(spec)?;
    let roots = spec.signatories.keys().cloned();
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
    state_transaction
        .world
        .account(signatory)
        .map(|account| account.id().clone())
        .map_err(map_find_error)
}

fn resolve_account_for_instruction(
    state_transaction: &StateTransaction<'_, '_>,
    account: &AccountId,
) -> Result<AccountId, InstructionExecutionError> {
    resolve_signatory_account(state_transaction, account).map_err(map_validation_fail)
}

fn materialize_missing_signatory_accounts(
    state_transaction: &mut StateTransaction<'_, '_>,
    home_domain: Option<&iroha_data_model::domain::DomainId>,
    multisig_account: &AccountId,
    spec: &MultisigSpec,
) -> Result<(), ValidationFail> {
    let authority = if let Some(home_domain) = home_domain {
        domain_owner(state_transaction, home_domain)?
    } else {
        multisig_account.clone()
    };
    for signatory in spec.signatories.keys() {
        if signatory.subject_id() == multisig_account.subject_id() {
            continue;
        }
        ensure_signatory_account_exists(state_transaction, signatory, &authority, home_domain)?;
    }
    Ok(())
}

fn ensure_signatory_account_exists(
    state_transaction: &mut StateTransaction<'_, '_>,
    signatory: &AccountId,
    authority: &AccountId,
    _home_domain: Option<&iroha_data_model::domain::DomainId>,
) -> Result<(), ValidationFail> {
    match resolve_signatory_account(state_transaction, signatory) {
        Ok(_) => Ok(()),
        Err(ValidationFail::InstructionFailed(InstructionExecutionError::Find(
            FindError::Account(_),
        )))
        | Err(ValidationFail::QueryFailed(QueryExecutionFail::Find(FindError::Account(_)))) => {
            let mut metadata = Metadata::default();
            metadata.insert((*MULTISIG_CREATED_VIA_KEY).clone(), Json::new("multisig"));
            let register_account = iroha_data_model::account::NewAccount::new(signatory.clone());
            Register::account(register_account.with_metadata(metadata))
                .execute(authority, state_transaction)
                .map_err(ValidationFail::InstructionFailed)
        }
        Err(err) => Err(err),
    }
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
    Ok(load_multisig_account_state(state_transaction, multisig_account)?.spec)
}

fn is_multisig(
    state_transaction: &StateTransaction<'_, '_>,
    account: &AccountId,
) -> Result<bool, ValidationFail> {
    match load_multisig_account_state_optional(state_transaction, account) {
        Ok(Some(_)) => Ok(true),
        Ok(None) => Ok(false),
        Err(ValidationFail::InstructionFailed(InstructionExecutionError::Find(
            FindError::Account(_),
        ))) => Ok(false),
        Err(ValidationFail::QueryFailed(QueryExecutionFail::Find(FindError::Account(_)))) => {
            Ok(false)
        }
        Err(ValidationFail::QueryFailed(QueryExecutionFail::NotFound)) => Ok(false),
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
    role_owner: &AccountId,
    home_domain: Option<&iroha_data_model::domain::DomainId>,
    multisig_account: &AccountId,
    spec: &MultisigSpec,
) -> Result<(), ValidationFail> {
    let signatories = resolved_signatory_accounts(state_transaction, spec)?;

    let multisig_role_id = multisig_role_for(home_domain, multisig_account);
    ensure_role_available(
        state_transaction,
        role_owner,
        &multisig_role_id,
        &signatories,
    )?;
    grant_role_if_needed(
        state_transaction,
        &multisig_role_id,
        multisig_account,
        role_owner,
    )?;

    for signatory in &signatories {
        let signatory_role_id = multisig_role_for(home_domain, signatory);
        let delegates = [signatory.clone(), multisig_account.clone()];

        ensure_role_available(
            state_transaction,
            role_owner,
            &signatory_role_id,
            &delegates,
        )?;
        grant_role_if_needed(state_transaction, &signatory_role_id, signatory, role_owner)?;
        grant_role_if_needed(
            state_transaction,
            &signatory_role_id,
            multisig_account,
            role_owner,
        )?;
        grant_role_if_needed(state_transaction, &multisig_role_id, signatory, role_owner)?;
    }

    Ok(())
}

fn multisig_spec_strict(
    state_transaction: &StateTransaction<'_, '_>,
    multisig_account: &AccountId,
) -> Result<MultisigSpec, InstructionExecutionError> {
    load_multisig_account_state(state_transaction, multisig_account)
        .map(|state| state.spec)
        .map_err(map_validation_fail)
}

fn multisig_home_domain(
    state_transaction: &StateTransaction<'_, '_>,
    multisig_account: &AccountId,
) -> Result<Option<iroha_data_model::domain::DomainId>, ValidationFail> {
    Ok(load_multisig_account_state(state_transaction, multisig_account)?.home_domain)
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
    let resolved_account = match resolve_signatory_account(state_transaction, account) {
        Ok(account) => account,
        Err(ValidationFail::InstructionFailed(InstructionExecutionError::Find(
            FindError::Account(_),
        )))
        | Err(ValidationFail::QueryFailed(QueryExecutionFail::Find(FindError::Account(_)))) => {
            return Ok(false);
        }
        Err(err) => return Err(err),
    };

    state_transaction
        .world
        .account(&resolved_account)
        .map_err(map_find_error)?;

    Ok(state_transaction
        .world
        .account_roles_iter(&resolved_account)
        .any(|role| role == role_id))
}

fn role_exists(state_transaction: &StateTransaction<'_, '_>, role_id: &RoleId) -> bool {
    state_transaction.world.roles.get(role_id).is_some()
}

fn persist_multisig_account_state(
    state_transaction: &mut StateTransaction<'_, '_>,
    previous_account_state: Option<&MultisigAccountState>,
    account_state: &MultisigAccountState,
) -> Result<(), ValidationFail> {
    let account = state_transaction
        .world
        .accounts
        .get_mut(&account_state.account_id)
        .ok_or_else(|| {
            ValidationFail::InstructionFailed(InstructionExecutionError::Find(FindError::Account(
                account_state.account_id.clone(),
            )))
        })?;
    account
        .metadata
        .insert(spec_key(), Json::new(account_state.spec.clone()));
    account.metadata.insert(
        home_domain_key(),
        Json::new(account_state.home_domain.clone()),
    );

    let bytes = norito::to_bytes(account_state).map_err(multisig_state_encode_error)?;
    state_transaction
        .world
        .smart_contract_state
        .insert(multisig_account_state_key(&account_state.account_id), bytes);
    sync_multisig_signatory_index(
        state_transaction,
        previous_account_state,
        Some(account_state),
    )?;
    Ok(())
}

fn multisig_signatory_index_members(account_state: &MultisigAccountState) -> BTreeSet<AccountId> {
    account_state
        .spec
        .signatories
        .keys()
        .map(AccountId::subject_id)
        .collect()
}

fn load_multisig_signatory_memberships(
    state_transaction: &StateTransaction<'_, '_>,
    signatory: &AccountId,
) -> Result<BTreeSet<AccountId>, ValidationFail> {
    let key = multisig_signatory_index_key(signatory);
    let Some(bytes) = state_transaction.world.smart_contract_state.get(&key) else {
        return Ok(BTreeSet::new());
    };
    norito::decode_from_bytes(bytes).map_err(multisig_state_decode_error)
}

fn store_multisig_signatory_memberships(
    state_transaction: &mut StateTransaction<'_, '_>,
    signatory: &AccountId,
    memberships: &BTreeSet<AccountId>,
) -> Result<(), ValidationFail> {
    let key = multisig_signatory_index_key(signatory);
    if memberships.is_empty() {
        state_transaction.world.smart_contract_state.remove(key);
        return Ok(());
    }
    let bytes = norito::to_bytes(memberships).map_err(multisig_state_encode_error)?;
    state_transaction
        .world
        .smart_contract_state
        .insert(key, bytes);
    Ok(())
}

fn sync_multisig_signatory_index(
    state_transaction: &mut StateTransaction<'_, '_>,
    previous_account_state: Option<&MultisigAccountState>,
    next_account_state: Option<&MultisigAccountState>,
) -> Result<(), ValidationFail> {
    let removed_members = previous_account_state
        .map(multisig_signatory_index_members)
        .unwrap_or_default();
    let added_members = next_account_state
        .map(multisig_signatory_index_members)
        .unwrap_or_default();
    let previous_multisig_account_id = previous_account_state.map(|state| state.account_id.clone());
    let next_multisig_account_id = next_account_state.map(|state| state.account_id.clone());

    let member_ids: BTreeSet<_> = removed_members
        .iter()
        .chain(added_members.iter())
        .cloned()
        .collect();

    for signatory in member_ids {
        let mut memberships = load_multisig_signatory_memberships(state_transaction, &signatory)?;
        if let Some(previous_multisig_account_id) = previous_multisig_account_id.as_ref()
            && removed_members.contains(&signatory)
        {
            memberships.remove(previous_multisig_account_id);
        }
        if let Some(next_multisig_account_id) = next_multisig_account_id.as_ref()
            && added_members.contains(&signatory)
        {
            memberships.insert(next_multisig_account_id.clone());
        }
        store_multisig_signatory_memberships(state_transaction, &signatory, &memberships)?;
    }

    Ok(())
}

fn load_multisig_account_state_optional(
    state_transaction: &StateTransaction<'_, '_>,
    multisig_account: &AccountId,
) -> Result<Option<MultisigAccountState>, ValidationFail> {
    let resolved_account = resolve_signatory_account(state_transaction, multisig_account)?;
    let key = multisig_account_state_key(&resolved_account);
    let Some(bytes) = state_transaction.world.smart_contract_state.get(&key) else {
        return Ok(None);
    };
    let state = norito::decode_from_bytes::<MultisigAccountState>(bytes)
        .map_err(multisig_state_decode_error)?;
    Ok(Some(state))
}

fn ensure_multisig_account_state_materialized(
    state_transaction: &mut StateTransaction<'_, '_>,
    multisig_account: &AccountId,
) -> Result<(), ValidationFail> {
    let resolved_account = resolve_signatory_account(state_transaction, multisig_account)?;
    let key = multisig_account_state_key(&resolved_account);
    if state_transaction
        .world
        .smart_contract_state
        .get(&key)
        .is_some()
    {
        return Ok(());
    }
    let Some(reconstructed) =
        reconstruct_multisig_account_state(state_transaction, &resolved_account)?
    else {
        return Ok(());
    };
    iroha_logger::warn!(
        multisig_account = %resolved_account,
        "reconstructing missing multisig account state from controller metadata"
    );
    persist_multisig_account_state(state_transaction, None, &reconstructed)?;
    Ok(())
}

fn reconstruct_multisig_account_state(
    state_transaction: &StateTransaction<'_, '_>,
    multisig_account: &AccountId,
) -> Result<Option<MultisigAccountState>, ValidationFail> {
    let resolved_account = resolve_signatory_account(state_transaction, multisig_account)?;
    let account = state_transaction
        .world
        .account(&resolved_account)
        .map_err(map_find_error)?;

    let home_domain = if let Some(raw) = account.metadata().get(&home_domain_key()) {
        norito::json::from_str::<Option<iroha_data_model::domain::DomainId>>(raw.as_ref()).map_err(
            |err| {
                ValidationFail::QueryFailed(QueryExecutionFail::Conversion(format!(
                    "multisig home_domain malformed for `{resolved_account}`: {err}"
                )))
            },
        )?
    } else {
        None
    };

    if let Some(raw) = account.metadata().get(&spec_key()) {
        let spec = norito::json::from_str::<MultisigSpec>(raw.as_ref()).map_err(|err| {
            ValidationFail::QueryFailed(QueryExecutionFail::Conversion(format!(
                "multisig spec malformed for `{resolved_account}`: {err}"
            )))
        })?;
        return Ok(Some(MultisigAccountState::new(
            resolved_account,
            home_domain,
            spec,
        )));
    }

    let Some(policy) = account.id().multisig_policy() else {
        return Ok(None);
    };
    let mut signatories = BTreeMap::new();
    for member in policy.members() {
        let seeded = AccountId::new(member.public_key().clone());
        let Some(signatory_account) = state_transaction
            .world
            .accounts_iter()
            .find(|candidate| candidate.id().subject_id() == seeded.subject_id())
            .map(|candidate| candidate.id().clone())
        else {
            return Err(ValidationFail::QueryFailed(QueryExecutionFail::NotFound));
        };
        let weight = u8::try_from(member.weight()).map_err(|_| {
            ValidationFail::QueryFailed(QueryExecutionFail::Conversion(format!(
                "multisig member weight {} exceeds u8 for `{resolved_account}`",
                member.weight()
            )))
        })?;
        signatories.insert(signatory_account, weight);
    }
    let quorum = std::num::NonZeroU16::new(policy.threshold()).ok_or_else(|| {
        ValidationFail::QueryFailed(QueryExecutionFail::Conversion(format!(
            "multisig threshold is zero for `{resolved_account}`"
        )))
    })?;
    let transaction_ttl_ms = std::num::NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS)
        .expect("default multisig ttl must be non-zero");

    Ok(Some(MultisigAccountState::new(
        resolved_account,
        home_domain,
        MultisigSpec {
            signatories,
            quorum,
            transaction_ttl_ms,
        },
    )))
}

fn load_multisig_account_state(
    state_transaction: &StateTransaction<'_, '_>,
    multisig_account: &AccountId,
) -> Result<MultisigAccountState, ValidationFail> {
    load_multisig_account_state_optional(state_transaction, multisig_account)?
        .ok_or(ValidationFail::QueryFailed(QueryExecutionFail::NotFound))
}

fn store_multisig_proposal_state(
    state_transaction: &mut StateTransaction<'_, '_>,
    proposal_state: &MultisigProposalState,
) -> Result<(), ValidationFail> {
    let bytes = norito::to_bytes(proposal_state).map_err(multisig_state_encode_error)?;
    state_transaction.world.smart_contract_state.insert(
        multisig_proposal_state_key(
            &proposal_state.multisig_account_id,
            &proposal_state.instructions_hash,
        ),
        bytes,
    );
    Ok(())
}

fn store_multisig_proposal_terminal_state(
    state_transaction: &mut StateTransaction<'_, '_>,
    terminal_state: &MultisigProposalTerminalState,
) -> Result<(), ValidationFail> {
    let bytes = norito::to_bytes(terminal_state).map_err(multisig_state_encode_error)?;
    state_transaction.world.smart_contract_state.insert(
        multisig_proposal_terminal_state_key(
            &terminal_state.multisig_account_id,
            &terminal_state.instructions_hash,
        ),
        bytes,
    );
    Ok(())
}

fn proposal_state(
    state_transaction: &StateTransaction<'_, '_>,
    multisig_account: &AccountId,
    instructions_hash: &HashOf<Vec<InstructionBox>>,
) -> Result<MultisigProposalState, ValidationFail> {
    let resolved_account = resolve_signatory_account(state_transaction, multisig_account)?;
    let key = multisig_proposal_state_key(&resolved_account, instructions_hash);
    let bytes = state_transaction
        .world
        .smart_contract_state
        .get(&key)
        .ok_or(ValidationFail::QueryFailed(QueryExecutionFail::NotFound))?;
    norito::decode_from_bytes::<MultisigProposalState>(bytes).map_err(multisig_state_decode_error)
}

fn proposal_state_value(proposal_state: &MultisigProposalState) -> MultisigProposalValue {
    MultisigProposalValue::new(
        proposal_state.instructions.clone(),
        proposal_state.proposed_at_ms,
        proposal_state.expires_at_ms,
        proposal_state.approvals.clone(),
        proposal_state.is_relayed,
    )
}

fn proposal_is_cancel_wrapper(proposal_state: &MultisigProposalState) -> bool {
    matches!(
        proposal_state.instructions.as_slice(),
        [instruction]
            if matches!(
                MultisigInstructionBox::try_from(instruction),
                Ok(MultisigInstructionBox::Cancel(_))
            )
    )
}

fn maybe_store_terminal_proposal_state(
    state_transaction: &mut StateTransaction<'_, '_>,
    proposal_state: &MultisigProposalState,
    status: MultisigProposalTerminalStatus,
) -> Result<(), ValidationFail> {
    if proposal_state.is_relayed.is_some() || proposal_is_cancel_wrapper(proposal_state) {
        return Ok(());
    }
    let terminal_state = MultisigProposalTerminalState::new(
        proposal_state.multisig_account_id.clone(),
        proposal_state.instructions_hash,
        proposal_state_value(proposal_state),
        status,
        now_ms(state_transaction),
    );
    store_multisig_proposal_terminal_state(state_transaction, &terminal_state)
}

fn move_multisig_proposals(
    state_transaction: &mut StateTransaction<'_, '_>,
    old_account: &AccountId,
    new_account: &AccountId,
) -> Result<(), ValidationFail> {
    let prefix = multisig_proposal_state_prefix(old_account);
    let prefix_literal = prefix.as_ref().to_owned();
    let mut entries = Vec::new();
    for (key, value) in state_transaction
        .world
        .smart_contract_state
        .range(prefix.clone()..)
    {
        if !key.as_ref().starts_with(prefix_literal.as_str()) {
            break;
        }
        let state = norito::decode_from_bytes::<MultisigProposalState>(value)
            .map_err(multisig_state_decode_error)?;
        entries.push((key.clone(), state));
    }

    for (old_key, mut proposal_state) in entries {
        proposal_state.multisig_account_id = new_account.clone();
        store_multisig_proposal_state(state_transaction, &proposal_state)?;
        state_transaction.world.smart_contract_state.remove(old_key);
    }

    let terminal_prefix = multisig_proposal_terminal_state_prefix(old_account);
    let terminal_prefix_literal = terminal_prefix.as_ref().to_owned();
    let mut terminal_entries = Vec::new();
    for (key, value) in state_transaction
        .world
        .smart_contract_state
        .range(terminal_prefix.clone()..)
    {
        if !key.as_ref().starts_with(terminal_prefix_literal.as_str()) {
            break;
        }
        let state = norito::decode_from_bytes::<MultisigProposalTerminalState>(value)
            .map_err(multisig_state_decode_error)?;
        terminal_entries.push((key.clone(), state));
    }

    for (old_key, mut terminal_state) in terminal_entries {
        terminal_state.multisig_account_id = new_account.clone();
        store_multisig_proposal_terminal_state(state_transaction, &terminal_state)?;
        state_transaction.world.smart_contract_state.remove(old_key);
    }

    Ok(())
}

#[cfg(test)]
fn proposal_value(
    state_transaction: &StateTransaction<'_, '_>,
    multisig_account: &AccountId,
    instructions_hash: &HashOf<Vec<InstructionBox>>,
) -> Result<MultisigProposalValue, ValidationFail> {
    let proposal_state = proposal_state(state_transaction, multisig_account, instructions_hash)?;
    Ok(MultisigProposalValue::new(
        proposal_state.instructions,
        proposal_state.proposed_at_ms,
        proposal_state.expires_at_ms,
        proposal_state.approvals,
        proposal_state.is_relayed,
    ))
}

fn now_ms(state_transaction: &StateTransaction<'_, '_>) -> u64 {
    state_transaction
        ._curr_block
        .creation_time()
        .as_millis()
        .try_into()
        .expect("block creation time must fit into u64")
}

fn multisig_state_encode_error(err: norito::Error) -> ValidationFail {
    ValidationFail::InternalError(format!("failed to encode multisig state:\n{err}"))
}

fn multisig_state_decode_error(err: norito::Error) -> ValidationFail {
    ValidationFail::QueryFailed(QueryExecutionFail::Conversion(format!(
        "multisig state malformed:\n{err}"
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
        ChainId, IntoKeyValue,
        account::{
            AccountController, AccountId, MultisigMember, MultisigPolicy,
            rekey::{AccountAlias, AccountAliasDomain},
        },
        block::BlockHeader,
        isi::{AddSignatory, RemoveSignatory, SetAccountQuorum},
        nexus::DataSpaceId,
        prelude::{Domain, InstructionBox, Register},
    };
    use iroha_executor_data_model::isi::multisig::{
        DEFAULT_MULTISIG_TTL_MS, MultisigApprove, MultisigCancel, MultisigPropose,
        MultisigRegister, MultisigSpec,
    };
    use nonzero_ext::nonzero;

    use super::*;
    use crate::{
        executor::Executor,
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };

    fn new_account_id(key_pair: &KeyPair) -> AccountId {
        AccountId::new(key_pair.public_key().clone())
    }

    fn register_account_in_domain(
        state_transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        _domain_id: &iroha_data_model::domain::DomainId,
        account_id: &AccountId,
        label: &str,
    ) {
        Register::account(iroha_data_model::account::NewAccount::new(
            account_id.clone(),
        ))
        .execute(authority, state_transaction)
        .expect(label);
    }

    fn register_multisig_account(
        state_transaction: &mut StateTransaction<'_, '_>,
        owner_id: &AccountId,
        domain_id: &iroha_data_model::domain::DomainId,
        spec: &MultisigSpec,
        label: &str,
    ) -> AccountId {
        let multisig_key = KeyPair::random();
        let multisig_id = new_account_id(&multisig_key);
        let mut metadata = Metadata::default();
        metadata.insert(spec_key(), Json::new(spec.clone()));
        metadata.insert(
            (*MULTISIG_HOME_DOMAIN_KEY).clone(),
            Json::new(Some(domain_id.clone())),
        );
        Register::account(
            iroha_data_model::account::NewAccount::new(multisig_id.clone()).with_metadata(metadata),
        )
        .execute(owner_id, state_transaction)
        .expect(label);
        let updated_account =
            rekey_multisig_account(state_transaction, &multisig_id, Some(domain_id), spec)
                .expect("rekey multisig account");
        persist_multisig_account_state(
            state_transaction,
            None,
            &MultisigAccountState::new(updated_account.clone(), domain_id.clone(), spec.clone()),
        )
        .expect("persist multisig account state");
        materialize_missing_signatory_accounts(
            state_transaction,
            Some(domain_id),
            &updated_account,
            spec,
        )
        .expect("materialize signatory accounts");
        configure_roles(
            state_transaction,
            owner_id,
            Some(domain_id),
            &updated_account,
            spec,
        )
        .expect("configure multisig roles");
        updated_account
    }

    fn bind_account_label(
        state_transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        account_id: &AccountId,
        domain_id: &iroha_data_model::domain::DomainId,
        label: &str,
    ) -> AccountAlias {
        let _ = authority;
        let _ = domain_id;
        let label = AccountAlias::new(
            label.parse().expect("account label name"),
            Some(AccountAliasDomain::new(domain_id.name().clone())),
            DataSpaceId::GLOBAL,
        );
        state_transaction
            .world
            .account_mut(account_id)
            .expect("registered account")
            .set_label(Some(label.clone()));
        state_transaction
            .world
            .insert_account_alias_binding(label.clone(), account_id.clone());
        state_transaction.world.account_rekey_records.insert(
            label.clone(),
            iroha_data_model::account::rekey::AccountRekeyRecord::new(
                label.clone(),
                account_id.clone(),
            ),
        );
        label
    }

    fn load_signatory_memberships(
        state_transaction: &StateTransaction<'_, '_>,
        signatory: &AccountId,
    ) -> BTreeSet<AccountId> {
        load_multisig_signatory_memberships(state_transaction, signatory)
            .expect("load signatory memberships")
    }

    fn multisig_policy_for_members(members: &[(&KeyPair, u16)]) -> MultisigPolicy {
        MultisigPolicy::new(
            u16::try_from(members.len()).expect("member count fits u16"),
            members
                .iter()
                .map(|(key_pair, weight)| {
                    MultisigMember::new(key_pair.public_key().clone(), *weight)
                        .expect("valid multisig member")
                })
                .collect(),
        )
        .expect("valid multisig policy")
    }

    fn seed_domain_name_lease(
        world: &mut World,
        owner: &AccountId,
        domain_id: &iroha_data_model::domain::DomainId,
    ) {
        let selector = crate::sns::selector_for_domain(domain_id).expect("selector");
        let address =
            iroha_data_model::account::AccountAddress::from_account_id(owner).expect("address");
        let record = iroha_data_model::sns::NameRecordV1::new(
            selector.clone(),
            owner.clone(),
            vec![iroha_data_model::sns::NameControllerV1::account(&address)],
            0,
            0,
            u64::MAX,
            u64::MAX,
            u64::MAX,
            Metadata::default(),
        );
        world.smart_contract_state_mut_for_testing().insert(
            crate::sns::record_storage_key(&selector),
            norito::codec::Encode::encode(&record),
        );
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
        let signer1_id = new_account_id(&signer1);
        let signer2_id = new_account_id(&signer2);

        Register::domain(Domain::new(domain_id.clone()))
            .execute(&signer1_id, &mut state_transaction)
            .expect("domain registration");

        register_account_in_domain(
            &mut state_transaction,
            &signer1_id,
            &domain_id,
            &signer1_id,
            "register signer1",
        );
        register_account_in_domain(
            &mut state_transaction,
            &signer1_id,
            &domain_id,
            &signer2_id,
            "register signer2",
        );

        let spec = MultisigSpec {
            signatories: BTreeMap::from([(signer1_id.clone(), 1), (signer2_id.clone(), 1)]),
            quorum: NonZeroU16::new(2).unwrap(),
            transaction_ttl_ms: NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).unwrap(),
        };
        let multisig_account_key = KeyPair::random();
        let multisig_id = new_account_id(&multisig_account_key);
        let register =
            MultisigRegister::with_account(multisig_id.clone(), domain_id.clone(), spec.clone());
        let executor = Executor::Initial;
        executor
            .execute_instruction(
                &mut state_transaction,
                &signer1_id,
                InstructionBox::from(register),
            )
            .expect("multisig register");

        let policy = multisig_policy_from_spec(&spec).expect("policy");
        let expected_id = AccountId::new_multisig(policy);
        state_transaction
            .world
            .account(&expected_id)
            .expect("multisig account registered");
        assert!(
            state_transaction
                .world
                .smart_contract_state
                .get(&multisig_account_state_key(&expected_id))
                .is_some(),
            "multisig account state must be stored on registration"
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
        assert_eq!(
            stored_spec.quorum, spec.quorum,
            "spec quorum must roundtrip through metadata"
        );
        assert_eq!(
            stored_spec.transaction_ttl_ms, spec.transaction_ttl_ms,
            "spec ttl must roundtrip through metadata"
        );
        assert_eq!(
            stored_spec.signatories.len(),
            spec.signatories.len(),
            "stored spec must preserve signatory cardinality"
        );
        for (expected_signatory, expected_weight) in &spec.signatories {
            let actual_weight = stored_spec
                .signatories
                .iter()
                .find_map(|(stored_signatory, stored_weight)| {
                    (stored_signatory.subject_id() == expected_signatory.subject_id())
                        .then_some(*stored_weight)
                })
                .expect("stored spec must include expected signatory subject");
            assert_eq!(actual_weight, *expected_weight);
        }
    }

    #[test]
    fn register_materializes_missing_signatory_accounts() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(
            World::new(),
            kura,
            query_handle,
            ChainId::from("multisig-register-materialize"),
        );
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();
        let domain_id: iroha_data_model::domain::DomainId = "acme".parse().unwrap();

        let owner = KeyPair::random();
        let owner_id = new_account_id(&owner);
        Register::domain(Domain::new(domain_id.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("domain registration");
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &owner_id,
            "register owner",
        );

        let missing_signer = KeyPair::random();
        let missing_signer_id = new_account_id(&missing_signer);
        let spec = MultisigSpec {
            signatories: BTreeMap::from([(owner_id.clone(), 1), (missing_signer_id.clone(), 1)]),
            quorum: NonZeroU16::new(2).unwrap(),
            transaction_ttl_ms: NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).unwrap(),
        };

        let multisig_seed = new_account_id(&KeyPair::random());
        execute_register(
            &mut state_transaction,
            &owner_id,
            MultisigRegister::with_account(multisig_seed, domain_id.clone(), spec.clone()),
        )
        .expect("register should materialize missing signatories");

        let created = state_transaction
            .world
            .account(&missing_signer_id)
            .expect("missing signatory should be auto-created");
        assert!(
            created.metadata().get(&*MULTISIG_CREATED_VIA_KEY).is_some(),
            "auto-created signatory must carry multisig created_via marker"
        );
    }

    #[test]
    fn register_allows_non_owner_without_permission() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(
            World::new(),
            kura,
            query_handle,
            ChainId::from("multisig-register-authority-reject"),
        );
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();
        let domain_id: iroha_data_model::domain::DomainId = "acme".parse().unwrap();

        let owner = KeyPair::random();
        let owner_id = new_account_id(&owner);
        Register::domain(Domain::new(domain_id.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("domain registration");
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &owner_id,
            "register owner",
        );

        let registrar = KeyPair::random();
        let registrar_id = new_account_id(&registrar);
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &registrar_id,
            "register registrar",
        );

        let spec = MultisigSpec {
            signatories: BTreeMap::from([(owner_id.clone(), 1)]),
            quorum: NonZeroU16::new(1).unwrap(),
            transaction_ttl_ms: NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).unwrap(),
        };
        let multisig_seed = new_account_id(&KeyPair::random());

        execute_register(
            &mut state_transaction,
            &registrar_id,
            MultisigRegister::with_account(multisig_seed, domain_id.clone(), spec),
        )
        .expect("registrar without permission should register multisig");
    }

    #[test]
    fn register_persists_multisig_metadata_on_authority_account() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(
            World::new(),
            kura,
            query_handle,
            ChainId::from("multisig-register-persists-metadata"),
        );
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();
        let domain_id: iroha_data_model::domain::DomainId = "acme".parse().unwrap();

        let owner = KeyPair::random();
        let owner_id = new_account_id(&owner);
        Register::domain(Domain::new(domain_id.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("domain registration");
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &owner_id,
            "register owner",
        );

        let signer = KeyPair::random();
        let signer_id = new_account_id(&signer);
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &signer_id,
            "register signer",
        );

        let spec = MultisigSpec {
            signatories: BTreeMap::from([(signer_id.clone(), 1)]),
            quorum: NonZeroU16::new(1).unwrap(),
            transaction_ttl_ms: NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).unwrap(),
        };
        let multisig_seed = new_account_id(&KeyPair::random());
        execute_register(
            &mut state_transaction,
            &owner_id,
            MultisigRegister::with_account(multisig_seed, domain_id.clone(), spec.clone()),
        )
        .expect("register multisig");

        let registered_multisig_id = state_transaction
            .world
            .accounts_iter()
            .find(|account| account.id().multisig_policy().is_some())
            .map(|account| account.id().clone())
            .expect("registered multisig account");
        let account = state_transaction
            .world
            .account(&registered_multisig_id)
            .expect("multisig account present");
        let stored_spec = account
            .metadata()
            .get(&spec_key())
            .cloned()
            .expect("multisig/spec metadata");
        let stored_spec: MultisigSpec = stored_spec
            .try_into_any_norito()
            .expect("multisig/spec should decode");
        let stored_home_domain = account
            .metadata()
            .get(&home_domain_key())
            .cloned()
            .expect("multisig home-domain metadata");
        let stored_home_domain: Option<iroha_data_model::domain::DomainId> = stored_home_domain
            .try_into_any_norito()
            .expect("home-domain should decode");

        assert_eq!(
            stored_spec, spec,
            "registered authority must expose spec metadata"
        );
        assert_eq!(
            stored_home_domain,
            Some(domain_id),
            "registered authority must expose home-domain metadata"
        );
    }

    #[test]
    fn register_invalid_spec_does_not_materialize_missing_signatory() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(
            World::new(),
            kura,
            query_handle,
            ChainId::from("multisig-register-invalid-no-materialize"),
        );
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();
        let domain_id: iroha_data_model::domain::DomainId = "acme".parse().unwrap();

        let owner = KeyPair::random();
        let owner_id = new_account_id(&owner);
        Register::domain(Domain::new(domain_id.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("domain registration");
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &owner_id,
            "register owner",
        );

        let missing_signer = KeyPair::random();
        let missing_signer_id = new_account_id(&missing_signer);
        let invalid_spec = MultisigSpec {
            signatories: BTreeMap::from([(owner_id.clone(), 1), (missing_signer_id.clone(), 1)]),
            quorum: NonZeroU16::new(3).unwrap(),
            transaction_ttl_ms: NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).unwrap(),
        };
        let multisig_seed = new_account_id(&KeyPair::random());

        let err = execute_register(
            &mut state_transaction,
            &owner_id,
            MultisigRegister::with_account(multisig_seed, domain_id.clone(), invalid_spec),
        )
        .expect_err("invalid quorum should reject registration");
        assert!(
            matches!(err, ValidationFail::NotPermitted(_)),
            "unexpected validation error for invalid quorum: {err:?}"
        );
        assert!(
            matches!(
                state_transaction.world.account(&missing_signer_id),
                Err(FindError::Account(_))
            ),
            "failed registration must not materialize missing signatories"
        );
    }

    #[test]
    fn register_existing_account_does_not_materialize_missing_signatory() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(
            World::new(),
            kura,
            query_handle,
            ChainId::from("multisig-register-existing-account-no-materialize"),
        );
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();
        let domain_id: iroha_data_model::domain::DomainId = "acme".parse().unwrap();

        let owner = KeyPair::random();
        let owner_id = new_account_id(&owner);
        Register::domain(Domain::new(domain_id.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("domain registration");
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &owner_id,
            "register owner",
        );

        let missing_signer = KeyPair::random();
        let missing_signer_id = new_account_id(&missing_signer);
        let spec = MultisigSpec {
            signatories: BTreeMap::from([(owner_id.clone(), 1), (missing_signer_id.clone(), 1)]),
            quorum: NonZeroU16::new(2).unwrap(),
            transaction_ttl_ms: NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).unwrap(),
        };
        let multisig_seed = new_account_id(&KeyPair::random());
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &multisig_seed,
            "pre-register multisig seed account",
        );

        execute_register(
            &mut state_transaction,
            &owner_id,
            MultisigRegister::with_account(multisig_seed, domain_id.clone(), spec),
        )
        .expect_err("existing multisig seed account must reject register");
        assert!(
            matches!(
                state_transaction.world.account(&missing_signer_id),
                Err(FindError::Account(_))
            ),
            "failed registration must not materialize missing signatories"
        );
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
        let owner_id = new_account_id(&owner_key);
        Register::domain(Domain::new(domain_id.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("domain registration");
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &owner_id,
            "register owner",
        );

        let signer1 = KeyPair::random();
        let signer2 = KeyPair::random();
        let signer1_id = new_account_id(&signer1);
        let signer2_id = new_account_id(&signer2);
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &signer1_id,
            "register signer1",
        );
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &signer2_id,
            "register signer2",
        );

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
        let updated_account = AccountId::new_multisig(updated_policy);
        let updated = multisig_spec(&state_transaction, &updated_account)
            .expect("spec must decode after add");
        assert!(
            updated
                .signatories
                .keys()
                .any(|account| account.subject_id() == signer2_id.subject_id()),
            "added signatory must appear in spec"
        );
        assert!(
            matches!(
                state_transaction.world.account(&multisig_id),
                Err(FindError::Account(_))
            ),
            "multisig account should be rekeyed after add"
        );
        let multisig_role = multisig_role_for(Some(&domain_id), &updated_account);
        let signer_role = multisig_role_for(Some(&domain_id), &signer2_id);
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
        let updated_account_data = state_transaction
            .world
            .account(&updated_account)
            .expect("updated multisig account");
        let metadata_spec = updated_account_data
            .metadata()
            .get(&spec_key())
            .cloned()
            .expect("updated multisig/spec metadata");
        let metadata_spec: MultisigSpec = metadata_spec
            .try_into_any_norito()
            .expect("updated multisig/spec should decode");
        assert_eq!(
            metadata_spec, updated_spec,
            "rekeyed authority should keep metadata spec in sync"
        );
    }

    #[test]
    fn add_signatory_keeps_alias_record_and_pending_proposal_approvable() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(
            World::new(),
            kura,
            query_handle,
            ChainId::from("multisig-add-signatory-alias-continuity"),
        );
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();
        let domain_id: iroha_data_model::domain::DomainId = "wonderland".parse().unwrap();

        let owner_key = KeyPair::random();
        let owner_id = new_account_id(&owner_key);
        Register::domain(Domain::new(domain_id.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("domain registration");
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &owner_id,
            "register owner",
        );

        let signer1 = KeyPair::random();
        let signer2 = KeyPair::random();
        let signer3 = KeyPair::random();
        let signer1_id = new_account_id(&signer1);
        let signer2_id = new_account_id(&signer2);
        let signer3_id = new_account_id(&signer3);
        for signer_id in [&signer1_id, &signer2_id, &signer3_id] {
            register_account_in_domain(
                &mut state_transaction,
                &owner_id,
                &domain_id,
                signer_id,
                "register signer",
            );
        }

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
        let alias = bind_account_label(
            &mut state_transaction,
            &owner_id,
            &multisig_id,
            &domain_id,
            "cbdc",
        );

        let instructions = Vec::<InstructionBox>::new();
        let instructions_hash = HashOf::new(&instructions);
        execute_propose(
            &mut state_transaction,
            &signer1_id,
            &MultisigPropose::new(multisig_id.clone(), instructions, None),
        )
        .expect("initial propose");

        AddSignatory::new(multisig_id.clone(), signer3.public_key().clone())
            .execute(&owner_id, &mut state_transaction)
            .expect("add signatory");

        let updated_spec = MultisigSpec {
            signatories: BTreeMap::from([
                (signer1_id.clone(), 1),
                (signer2_id.clone(), 1),
                (signer3_id.clone(), 1),
            ]),
            quorum: spec.quorum,
            transaction_ttl_ms: spec.transaction_ttl_ms,
        };
        let updated_account =
            AccountId::new_multisig(multisig_policy_from_spec(&updated_spec).expect("policy"));
        let rekey_record = state_transaction
            .world
            .account_rekey_records
            .get(&alias)
            .expect("alias rekey record");
        assert_eq!(
            rekey_record.active_account_id, updated_account,
            "alias should resolve to the rekeyed multisig account"
        );
        assert_eq!(
            rekey_record.previous_account_ids,
            vec![multisig_id.clone()],
            "rekey record should retain the prior concrete multisig account"
        );
        let proposal = proposal_value(&state_transaction, &updated_account, &instructions_hash)
            .expect("proposal should move to the rekeyed account");
        assert_eq!(
            proposal.approvals,
            BTreeSet::from([signer1_id.clone()]),
            "existing approvals should survive add-signatory rekey"
        );

        execute_approve(
            &mut state_transaction,
            &signer2_id,
            &MultisigApprove::new(updated_account.clone(), instructions_hash),
        )
        .expect("approval through rekeyed account");
        match proposal_value(&state_transaction, &updated_account, &instructions_hash) {
            Ok(proposal) => {
                assert!(
                    matches!(proposal.is_relayed, Some(true)),
                    "executed proposal should be marked relayed when not pruned immediately"
                );
            }
            Err(ValidationFail::QueryFailed(QueryExecutionFail::Find(FindError::MetadataKey(
                _,
            ))))
            | Err(ValidationFail::QueryFailed(QueryExecutionFail::NotFound)) => {}
            Err(err) => panic!("unexpected proposal state after approval: {err:?}"),
        }
    }

    #[test]
    fn add_signatory_materializes_missing_account() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(
            World::new(),
            kura,
            query_handle,
            ChainId::from("multisig-add-signatory-materialize"),
        );
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();
        let domain_id: iroha_data_model::domain::DomainId = "wonderland".parse().unwrap();

        let owner_key = KeyPair::random();
        let owner_id = new_account_id(&owner_key);
        Register::domain(Domain::new(domain_id.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("domain registration");
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &owner_id,
            "register owner",
        );

        let signer1 = KeyPair::random();
        let signer1_id = new_account_id(&signer1);
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &signer1_id,
            "register signer1",
        );

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

        let missing_signer = KeyPair::random();
        let missing_signer_id = new_account_id(&missing_signer);
        assert!(
            matches!(
                state_transaction.world.account(&missing_signer_id),
                Err(FindError::Account(_))
            ),
            "precondition: signatory account must be missing"
        );

        AddSignatory::new(multisig_id.clone(), missing_signer.public_key().clone())
            .execute(&owner_id, &mut state_transaction)
            .expect("add signatory should materialize missing account");

        let created = state_transaction
            .world
            .account(&missing_signer_id)
            .expect("missing signatory should be materialized");
        assert!(
            created.metadata().get(&*MULTISIG_CREATED_VIA_KEY).is_some(),
            "materialized account should be tagged as multisig-created"
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
        let owner_id = new_account_id(&owner_key);
        Register::domain(Domain::new(domain_id.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("domain registration");
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &owner_id,
            "register owner",
        );

        let signer1 = KeyPair::random();
        let signer2 = KeyPair::random();
        let signer1_id = new_account_id(&signer1);
        let signer2_id = new_account_id(&signer2);
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &signer1_id,
            "register signer1",
        );
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &signer2_id,
            "register signer2",
        );

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
        configure_roles(
            &mut state_transaction,
            &owner_id,
            Some(&domain_id),
            &multisig_id,
            &spec,
        )
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
        let updated_account = AccountId::new_multisig(updated_policy);
        let updated = multisig_spec(&state_transaction, &updated_account)
            .expect("spec must decode after remove");
        assert!(
            !updated
                .signatories
                .keys()
                .any(|account| account.subject_id() == signer2_id.subject_id()),
            "removed signatory must be absent from spec"
        );
        assert!(
            matches!(
                state_transaction.world.account(&multisig_id),
                Err(FindError::Account(_))
            ),
            "multisig account should be rekeyed after removal"
        );
        let multisig_role = multisig_role_for(Some(&domain_id), &updated_account);
        let signer_role = multisig_role_for(Some(&domain_id), &signer2_id);
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
    fn remove_signatory_keeps_alias_record_and_pending_proposal_approvable() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(
            World::new(),
            kura,
            query_handle,
            ChainId::from("multisig-remove-signatory-alias-continuity"),
        );
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();
        let domain_id: iroha_data_model::domain::DomainId = "wonderland".parse().unwrap();

        let owner_key = KeyPair::random();
        let owner_id = new_account_id(&owner_key);
        Register::domain(Domain::new(domain_id.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("domain registration");
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &owner_id,
            "register owner",
        );

        let signer1 = KeyPair::random();
        let signer2 = KeyPair::random();
        let signer3 = KeyPair::random();
        let signer1_id = new_account_id(&signer1);
        let signer2_id = new_account_id(&signer2);
        let signer3_id = new_account_id(&signer3);
        for signer_id in [&signer1_id, &signer2_id, &signer3_id] {
            register_account_in_domain(
                &mut state_transaction,
                &owner_id,
                &domain_id,
                signer_id,
                "register signer",
            );
        }

        let spec = MultisigSpec {
            signatories: BTreeMap::from([
                (signer1_id.clone(), 1),
                (signer2_id.clone(), 1),
                (signer3_id.clone(), 1),
            ]),
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
        let alias = bind_account_label(
            &mut state_transaction,
            &owner_id,
            &multisig_id,
            &domain_id,
            "cbdc",
        );

        let instructions = Vec::<InstructionBox>::new();
        let instructions_hash = HashOf::new(&instructions);
        execute_propose(
            &mut state_transaction,
            &signer1_id,
            &MultisigPropose::new(multisig_id.clone(), instructions, None),
        )
        .expect("initial propose");

        RemoveSignatory::new(multisig_id.clone(), signer3.public_key().clone())
            .execute(&owner_id, &mut state_transaction)
            .expect("remove signatory");

        let updated_spec = MultisigSpec {
            signatories: BTreeMap::from([(signer1_id.clone(), 1), (signer2_id.clone(), 1)]),
            quorum: spec.quorum,
            transaction_ttl_ms: spec.transaction_ttl_ms,
        };
        let updated_account =
            AccountId::new_multisig(multisig_policy_from_spec(&updated_spec).expect("policy"));
        let rekey_record = state_transaction
            .world
            .account_rekey_records
            .get(&alias)
            .expect("alias rekey record");
        assert_eq!(
            rekey_record.active_account_id, updated_account,
            "alias should resolve to the rekeyed multisig account"
        );
        assert_eq!(
            rekey_record.previous_account_ids,
            vec![multisig_id.clone()],
            "rekey record should retain the prior concrete multisig account"
        );
        let proposal = proposal_value(&state_transaction, &updated_account, &instructions_hash)
            .expect("proposal should move to the rekeyed account");
        assert_eq!(
            proposal.approvals,
            BTreeSet::from([signer1_id.clone()]),
            "existing approvals should survive remove-signatory rekey"
        );

        execute_approve(
            &mut state_transaction,
            &signer2_id,
            &MultisigApprove::new(updated_account.clone(), instructions_hash),
        )
        .expect("approval through rekeyed account");
        match proposal_value(&state_transaction, &updated_account, &instructions_hash) {
            Ok(proposal) => {
                assert!(
                    matches!(proposal.is_relayed, Some(true)),
                    "executed proposal should be marked relayed when not pruned immediately"
                );
            }
            Err(ValidationFail::QueryFailed(QueryExecutionFail::Find(FindError::MetadataKey(
                _,
            ))))
            | Err(ValidationFail::QueryFailed(QueryExecutionFail::NotFound)) => {}
            Err(err) => panic!("unexpected proposal state after approval: {err:?}"),
        }
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
        let owner_id = new_account_id(&owner_key);
        Register::domain(Domain::new(domain_id.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("domain registration");
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &owner_id,
            "register owner",
        );

        let signer1 = KeyPair::random();
        let signer2 = KeyPair::random();
        let signer1_id = new_account_id(&signer1);
        let signer2_id = new_account_id(&signer2);
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &signer1_id,
            "register signer1",
        );
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &signer2_id,
            "register signer2",
        );

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
        let updated_account = AccountId::new_multisig(updated_policy);
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
    fn set_account_quorum_keeps_alias_record_and_pending_proposal_approvable() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(
            World::new(),
            kura,
            query_handle,
            ChainId::from("multisig-set-quorum-alias-continuity"),
        );
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();
        let domain_id: iroha_data_model::domain::DomainId = "wonderland".parse().unwrap();

        let owner_key = KeyPair::random();
        let owner_id = new_account_id(&owner_key);
        Register::domain(Domain::new(domain_id.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("domain registration");
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &owner_id,
            "register owner",
        );

        let signer1 = KeyPair::random();
        let signer2 = KeyPair::random();
        let signer3 = KeyPair::random();
        let signer1_id = new_account_id(&signer1);
        let signer2_id = new_account_id(&signer2);
        let signer3_id = new_account_id(&signer3);
        for signer_id in [&signer1_id, &signer2_id, &signer3_id] {
            register_account_in_domain(
                &mut state_transaction,
                &owner_id,
                &domain_id,
                signer_id,
                "register signer",
            );
        }

        let spec = MultisigSpec {
            signatories: BTreeMap::from([
                (signer1_id.clone(), 1),
                (signer2_id.clone(), 1),
                (signer3_id.clone(), 1),
            ]),
            quorum: NonZeroU16::new(3).unwrap(),
            transaction_ttl_ms: NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).unwrap(),
        };
        let multisig_id = register_multisig_account(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &spec,
            "register multisig account",
        );
        let alias = bind_account_label(
            &mut state_transaction,
            &owner_id,
            &multisig_id,
            &domain_id,
            "cbdc",
        );

        let instructions = Vec::<InstructionBox>::new();
        let instructions_hash = HashOf::new(&instructions);
        execute_propose(
            &mut state_transaction,
            &signer1_id,
            &MultisigPropose::new(multisig_id.clone(), instructions, None),
        )
        .expect("initial propose");

        let new_quorum = NonZeroU16::new(2).unwrap();
        SetAccountQuorum::new(multisig_id.clone(), new_quorum)
            .execute(&owner_id, &mut state_transaction)
            .expect("set quorum");

        let updated_spec = MultisigSpec {
            signatories: spec.signatories.clone(),
            quorum: new_quorum,
            transaction_ttl_ms: spec.transaction_ttl_ms,
        };
        let updated_account =
            AccountId::new_multisig(multisig_policy_from_spec(&updated_spec).expect("policy"));
        let rekey_record = state_transaction
            .world
            .account_rekey_records
            .get(&alias)
            .expect("alias rekey record");
        assert_eq!(
            rekey_record.active_account_id, updated_account,
            "alias should resolve to the rekeyed multisig account"
        );
        assert_eq!(
            rekey_record.previous_account_ids,
            vec![multisig_id.clone()],
            "rekey record should retain the prior concrete multisig account"
        );
        let proposal = proposal_value(&state_transaction, &updated_account, &instructions_hash)
            .expect("proposal should move to the rekeyed account");
        assert_eq!(
            proposal.approvals,
            BTreeSet::from([signer1_id.clone()]),
            "existing approvals should survive quorum-change rekey"
        );

        execute_approve(
            &mut state_transaction,
            &signer2_id,
            &MultisigApprove::new(updated_account.clone(), instructions_hash),
        )
        .expect("approval through rekeyed account");
        match proposal_value(&state_transaction, &updated_account, &instructions_hash) {
            Ok(proposal) => {
                assert!(
                    matches!(proposal.is_relayed, Some(true)),
                    "executed proposal should be marked relayed when not pruned immediately"
                );
            }
            Err(ValidationFail::QueryFailed(QueryExecutionFail::Find(FindError::MetadataKey(
                _,
            ))))
            | Err(ValidationFail::QueryFailed(QueryExecutionFail::NotFound)) => {}
            Err(err) => panic!("unexpected proposal state after approval: {err:?}"),
        }
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
        let old_account = new_account_id(&old_key);
        Register::domain(Domain::new(domain_id.clone()))
            .execute(&old_account, &mut state_transaction)
            .expect("domain registration");
        register_account_in_domain(
            &mut state_transaction,
            &old_account,
            &domain_id,
            &old_account,
            "register old account",
        );

        let new_key = KeyPair::random();
        let new_account = new_account_id(&new_key);

        rekey_account_id(
            &mut state_transaction,
            &old_account,
            &new_account,
            Some(&domain_id),
        )
        .expect("rekey should succeed");

        assert!(
            matches!(
                state_transaction.world.account(&old_account),
                Err(FindError::Account(_))
            ),
            "old canonical account should be removed after rekey"
        );
        assert!(
            state_transaction.world.account(&new_account).is_ok(),
            "new canonical account should be present after rekey"
        );

        let _ = domain_id;
    }

    #[test]
    fn rekey_account_id_moves_asset_holder_index_to_new_account() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(
            World::new(),
            kura,
            query_handle,
            ChainId::from("multisig-rekey-asset-holder-index"),
        );
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();
        let domain_id: iroha_data_model::domain::DomainId = "default".parse().unwrap();

        let old_key = KeyPair::random();
        let old_account = new_account_id(&old_key);
        Register::domain(Domain::new(domain_id.clone()))
            .execute(&old_account, &mut state_transaction)
            .expect("domain registration");
        register_account_in_domain(
            &mut state_transaction,
            &old_account,
            &domain_id,
            &old_account,
            "register old account",
        );

        let asset_def_id: iroha_data_model::asset::AssetDefinitionId =
            iroha_data_model::asset::AssetDefinitionId::new(
                domain_id.clone(),
                "rose".parse().unwrap(),
            );
        Register::asset_definition({
            let __asset_definition_id = asset_def_id.clone();
            iroha_data_model::asset::AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        })
        .execute(&old_account, &mut state_transaction)
        .expect("register asset definition");

        let old_asset_id =
            iroha_data_model::asset::AssetId::new(asset_def_id.clone(), old_account.clone());
        let (_, old_asset_value) = iroha_data_model::asset::Asset::new(
            old_asset_id.clone(),
            iroha_primitives::numeric::Numeric::new(5, 0),
        )
        .into_key_value();
        state_transaction
            .world
            .assets
            .insert(old_asset_id.clone(), old_asset_value);
        state_transaction.world.track_asset_holder(&old_asset_id);

        let new_key = KeyPair::random();
        let new_account = new_account_id(&new_key);

        rekey_account_id(
            &mut state_transaction,
            &old_account,
            &new_account,
            Some(&domain_id),
        )
        .expect("rekey should succeed");

        let new_asset_id = iroha_data_model::asset::AssetId::with_scope(
            asset_def_id.clone(),
            new_account.clone(),
            *old_asset_id.scope(),
        );
        assert!(
            state_transaction.world.assets.get(&old_asset_id).is_none(),
            "old account asset row should be removed"
        );
        assert!(
            state_transaction.world.assets.get(&new_asset_id).is_some(),
            "new account asset row should exist"
        );

        let holders = state_transaction
            .world
            .asset_definition_holders
            .get(&asset_def_id)
            .expect("holder index should exist after rekey");
        assert!(
            holders.contains(&new_account),
            "new account should be present in holder index"
        );
        assert!(
            !holders.contains(&old_account),
            "old account should be removed from holder index"
        );
    }

    #[test]
    fn multisig_register_preserves_explicit_home_domain() {
        let source_domain: iroha_data_model::domain::DomainId = "default".parse().unwrap();
        let signer = new_account_id(&KeyPair::random());
        let spec = MultisigSpec {
            signatories: BTreeMap::from([(signer.clone(), 1)]),
            quorum: NonZeroU16::new(1).unwrap(),
            transaction_ttl_ms: NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).unwrap(),
        };
        let register = MultisigRegister::with_account(
            new_account_id(&KeyPair::random()),
            source_domain.clone(),
            spec,
        );

        let signer_in_spec = register
            .spec
            .signatories
            .keys()
            .next()
            .expect("signatory exists");
        assert_eq!(register.home_domain.as_ref(), Some(&source_domain));
        assert_eq!(signer_in_spec.controller(), signer.controller());
    }

    #[test]
    fn multisig_register_supports_domainless_home_domain() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(
            World::new(),
            kura,
            query_handle,
            ChainId::from("multisig-domainless-register"),
        );
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();

        let owner_id = new_account_id(&KeyPair::random());
        Register::account(iroha_data_model::account::NewAccount::new(owner_id.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("register domainless owner");

        let signer = new_account_id(&KeyPair::random());
        let spec = MultisigSpec {
            signatories: BTreeMap::from([(signer.clone(), 1)]),
            quorum: NonZeroU16::new(1).unwrap(),
            transaction_ttl_ms: NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).unwrap(),
        };
        let multisig_seed = new_account_id(&KeyPair::random());

        execute_register(
            &mut state_transaction,
            &owner_id,
            MultisigRegister::with_account(
                multisig_seed,
                None::<iroha_data_model::domain::DomainId>,
                spec,
            ),
        )
        .expect("register domainless multisig");

        let registered_multisig_id = state_transaction
            .world
            .accounts_iter()
            .find(|account| account.id().multisig_policy().is_some())
            .map(|account| account.id().clone())
            .expect("registered multisig account");
        assert_eq!(
            multisig_home_domain(&state_transaction, &registered_multisig_id)
                .expect("multisig home domain"),
            None,
            "domainless multisig should persist an empty home domain",
        );
        assert!(
            state_transaction
                .world
                .account(&registered_multisig_id)
                .is_ok(),
            "registered multisig should remain present"
        );
        assert!(
            state_transaction.world.account(&signer).is_ok(),
            "materialized signatory should remain present"
        );
    }

    #[test]
    fn multisig_spec_uses_domainless_subject_identity() {
        let shared_key = KeyPair::random().public_key().clone();

        let first = AccountId::new(shared_key.clone());
        let second = AccountId::new(shared_key);
        let spec = MultisigSpec {
            signatories: BTreeMap::from([(first, 1), (second, 1)]),
            quorum: NonZeroU16::new(1).unwrap(),
            transaction_ttl_ms: NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).unwrap(),
        };

        assert_eq!(
            spec.signatories.len(),
            1,
            "domainless account ids must collapse identical subjects"
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
        let owner_id = new_account_id(&owner_key);
        Register::domain(Domain::new(domain_id.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("domain registration");
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &owner_id,
            "register owner",
        );

        let signer1 = KeyPair::random();
        let signer2 = KeyPair::random();
        let signer1_id = new_account_id(&signer1);
        let signer2_id = new_account_id(&signer2);
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &signer1_id,
            "register signer1",
        );
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &signer2_id,
            "register signer2",
        );

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
        let signer1_id = new_account_id(&signer1);
        let signer2_id = new_account_id(&signer2);

        Register::domain(Domain::new(domain_id.clone()))
            .execute(&signer1_id, &mut state_transaction)
            .expect("domain registration");

        register_account_in_domain(
            &mut state_transaction,
            &signer1_id,
            &domain_id,
            &signer1_id,
            "register signer1",
        );
        register_account_in_domain(
            &mut state_transaction,
            &signer1_id,
            &domain_id,
            &signer2_id,
            "register signer2",
        );

        let spec = MultisigSpec {
            signatories: BTreeMap::from([(signer1_id.clone(), 1), (signer2_id.clone(), 1)]),
            quorum: NonZeroU16::new(2).unwrap(),
            transaction_ttl_ms: NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).unwrap(),
        };
        let multisig_account_key = KeyPair::random();
        let multisig_id = new_account_id(&multisig_account_key);
        let register =
            MultisigRegister::with_account(multisig_id.clone(), domain_id.clone(), spec.clone());
        Executor::Initial
            .execute_instruction(
                &mut state_transaction,
                &signer1_id,
                InstructionBox::from(register),
            )
            .expect("multisig register");

        let policy = multisig_policy_from_spec(&spec).expect("policy");
        let expected_id = AccountId::new_multisig(policy);
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
        let signer1_id = new_account_id(&signer1);
        let signer2_id = new_account_id(&signer2);

        Register::domain(Domain::new(domain_id.clone()))
            .execute(&signer1_id, &mut state_transaction)
            .expect("domain registration");
        register_account_in_domain(
            &mut state_transaction,
            &signer1_id,
            &domain_id,
            &signer1_id,
            "register signer1",
        );
        register_account_in_domain(
            &mut state_transaction,
            &signer1_id,
            &domain_id,
            &signer2_id,
            "register signer2",
        );

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
    fn multisig_propose_repairs_missing_state_from_controller() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let domain_id: iroha_data_model::domain::DomainId = "repairable".parse().unwrap();

        let signer1 = KeyPair::random();
        let signer2 = KeyPair::random();
        let signer1_id = new_account_id(&signer1);
        let signer2_id = new_account_id(&signer2);
        let mut world = World::new();
        let selector = crate::sns::selector_for_domain(&domain_id).expect("selector");
        let address = iroha_data_model::account::AccountAddress::from_account_id(&signer1_id)
            .expect("signer address");
        let lease = iroha_data_model::sns::NameRecordV1::new(
            selector.clone(),
            signer1_id.clone(),
            vec![iroha_data_model::sns::NameControllerV1::account(&address)],
            0,
            0,
            u64::MAX,
            u64::MAX,
            u64::MAX,
            Metadata::default(),
        );
        world.smart_contract_state_mut_for_testing().insert(
            crate::sns::record_storage_key(&selector),
            norito::codec::Encode::encode(&lease),
        );
        let state = State::new_with_chain(
            world,
            kura,
            query_handle,
            ChainId::from("multisig-repair-from-controller"),
        );
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();

        Register::domain(Domain::new(domain_id.clone()))
            .execute(&signer1_id, &mut state_transaction)
            .expect("domain registration");
        register_account_in_domain(
            &mut state_transaction,
            &signer1_id,
            &domain_id,
            &signer1_id,
            "register signer1",
        );
        register_account_in_domain(
            &mut state_transaction,
            &signer1_id,
            &domain_id,
            &signer2_id,
            "register signer2",
        );

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

        state_transaction
            .world
            .smart_contract_state
            .remove(multisig_account_state_key(&multisig_id));
        state_transaction
            .world
            .smart_contract_state
            .remove(multisig_signatory_index_key(&signer1_id));
        state_transaction
            .world
            .smart_contract_state
            .remove(multisig_signatory_index_key(&signer2_id));
        let account = state_transaction
            .world
            .accounts
            .get_mut(&multisig_id)
            .expect("registered multisig");
        account.metadata = Metadata::default();
        account.insert((*MULTISIG_CREATED_VIA_KEY).clone(), Json::new("implicit"));

        let instructions: Vec<InstructionBox> = Vec::new();
        let propose = MultisigPropose::new(multisig_id.clone(), instructions, None);
        execute_propose(&mut state_transaction, &signer1_id, &propose)
            .expect("state repair should allow signatory proposal");

        let repaired =
            load_multisig_account_state(&state_transaction, &multisig_id).expect("repaired state");
        assert_eq!(
            repaired.home_domain,
            Some(domain_id.clone()),
            "repair should infer the home domain from linked domains"
        );
        assert!(
            repaired
                .spec
                .signatories
                .keys()
                .any(|account| account.subject_id() == signer1_id.subject_id()),
            "repaired spec should include signer1"
        );
        assert!(
            repaired
                .spec
                .signatories
                .keys()
                .any(|account| account.subject_id() == signer2_id.subject_id()),
            "repaired spec should include signer2"
        );
        assert!(
            !load_signatory_memberships(&state_transaction, &signer1_id).is_empty(),
            "repair should repopulate the signatory index for signer1"
        );
        assert!(
            !load_signatory_memberships(&state_transaction, &signer2_id).is_empty(),
            "repair should repopulate the signatory index for signer2"
        );
    }

    #[test]
    fn multisig_register_indexes_signatory_memberships() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(
            World::new(),
            kura,
            query_handle,
            ChainId::from("multisig-signatory-index-register"),
        );
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();
        let domain_id: iroha_data_model::domain::DomainId = "signatory-index".parse().unwrap();

        let signer1 = KeyPair::random();
        let signer2 = KeyPair::random();
        let signer1_id = new_account_id(&signer1);
        let signer2_id = new_account_id(&signer2);

        Register::domain(Domain::new(domain_id.clone()))
            .execute(&signer1_id, &mut state_transaction)
            .expect("domain registration");
        register_account_in_domain(
            &mut state_transaction,
            &signer1_id,
            &domain_id,
            &signer1_id,
            "register signer1",
        );
        register_account_in_domain(
            &mut state_transaction,
            &signer1_id,
            &domain_id,
            &signer2_id,
            "register signer2",
        );

        let spec = MultisigSpec {
            signatories: BTreeMap::from([(signer1_id.clone(), 1), (signer2_id.clone(), 1)]),
            quorum: NonZeroU16::new(2).unwrap(),
            transaction_ttl_ms: NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).unwrap(),
        };
        let multisig_account_id = register_multisig_account(
            &mut state_transaction,
            &signer1_id,
            &domain_id,
            &spec,
            "register multisig account",
        );

        assert_eq!(
            load_signatory_memberships(&state_transaction, &signer1_id),
            BTreeSet::from([multisig_account_id.clone()])
        );
        assert_eq!(
            load_signatory_memberships(&state_transaction, &signer2_id),
            BTreeSet::from([multisig_account_id])
        );
    }

    #[test]
    fn multisig_rekey_repoints_signatory_memberships() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(
            World::new(),
            kura,
            query_handle,
            ChainId::from("multisig-signatory-index-rekey"),
        );
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();
        let domain_id: iroha_data_model::domain::DomainId = "signatory-rekey".parse().unwrap();

        let signer1 = KeyPair::random();
        let signer2 = KeyPair::random();
        let signer3 = KeyPair::random();
        let signer1_id = new_account_id(&signer1);
        let signer2_id = new_account_id(&signer2);
        let signer3_id = new_account_id(&signer3);

        Register::domain(Domain::new(domain_id.clone()))
            .execute(&signer1_id, &mut state_transaction)
            .expect("domain registration");
        for (account_id, label) in [
            (&signer1_id, "register signer1"),
            (&signer2_id, "register signer2"),
            (&signer3_id, "register signer3"),
        ] {
            register_account_in_domain(
                &mut state_transaction,
                &signer1_id,
                &domain_id,
                account_id,
                label,
            );
        }

        let initial_spec = MultisigSpec {
            signatories: BTreeMap::from([(signer1_id.clone(), 1), (signer2_id.clone(), 1)]),
            quorum: NonZeroU16::new(2).unwrap(),
            transaction_ttl_ms: NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).unwrap(),
        };
        let initial_multisig_account_id = register_multisig_account(
            &mut state_transaction,
            &signer1_id,
            &domain_id,
            &initial_spec,
            "register multisig account",
        );

        AddSignatory {
            account: initial_multisig_account_id.clone(),
            signatory: signer3.public_key().clone(),
        }
        .execute(&signer1_id, &mut state_transaction)
        .expect("add signatory");

        let updated_spec = MultisigSpec {
            signatories: BTreeMap::from([
                (signer1_id.clone(), 1),
                (signer2_id.clone(), 1),
                (signer3_id.clone(), 1),
            ]),
            quorum: NonZeroU16::new(2).unwrap(),
            transaction_ttl_ms: NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).unwrap(),
        };
        let updated_multisig_account_id =
            AccountId::new_multisig(multisig_policy_from_spec(&updated_spec).expect("policy"));

        assert_ne!(initial_multisig_account_id, updated_multisig_account_id);
        assert_eq!(
            load_signatory_memberships(&state_transaction, &signer1_id),
            BTreeSet::from([updated_multisig_account_id.clone()])
        );
        assert_eq!(
            load_signatory_memberships(&state_transaction, &signer2_id),
            BTreeSet::from([updated_multisig_account_id.clone()])
        );
        assert_eq!(
            load_signatory_memberships(&state_transaction, &signer3_id),
            BTreeSet::from([updated_multisig_account_id])
        );
    }

    #[test]
    fn multisig_approval_preserves_contract_call_trigger_metadata_for_non_default_entrypoints() {
        use iroha_data_model::{
            HasMetadata,
            events::execute_trigger::ExecuteTriggerEventFilter,
            isi::ExecuteTrigger,
            metadata::Metadata,
            name::Name,
            prelude::Json,
            transaction::{Executable, IvmBytecode},
            trigger::{
                Trigger,
                action::{Action, Repeats},
            },
        };
        use ivm::KotodamaCompiler;

        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(
            World::new(),
            kura,
            query_handle,
            ChainId::from("multisig-trigger-contract-entrypoint"),
        );

        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();

        let signer1 = KeyPair::random();
        let signer2 = KeyPair::random();
        let signer1_id = new_account_id(&signer1);
        let signer2_id = new_account_id(&signer2);
        let owner_id = new_account_id(&KeyPair::random());

        Register::account(iroha_data_model::account::NewAccount::new(owner_id.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("register owner");
        Register::account(iroha_data_model::account::NewAccount::new(
            signer1_id.clone(),
        ))
        .execute(&owner_id, &mut state_transaction)
        .expect("register signer1");
        Register::account(iroha_data_model::account::NewAccount::new(
            signer2_id.clone(),
        ))
        .execute(&owner_id, &mut state_transaction)
        .expect("register signer2");

        let spec = MultisigSpec {
            signatories: BTreeMap::from([(signer1_id.clone(), 1), (signer2_id.clone(), 1)]),
            quorum: NonZeroU16::new(2).unwrap(),
            transaction_ttl_ms: NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).unwrap(),
        };
        let multisig_id = new_account_id(&KeyPair::random());
        execute_register(
            &mut state_transaction,
            &owner_id,
            MultisigRegister::with_account(
                multisig_id.clone(),
                None::<iroha_data_model::domain::DomainId>,
                spec.clone(),
            ),
        )
        .expect("register multisig account");
        let multisig_id = state_transaction
            .world
            .accounts_iter()
            .find(|account| account.id().multisig_policy().is_some())
            .map(|account| account.id().clone())
            .expect("registered multisig account");

        let program = KotodamaCompiler::new()
            .compile_source(
                r#"
seiyaku TriggerDispatch {
  #[access(read="*", write="*")]
  kotoage fn main() permission(Admin) {
    set_account_detail(authority(), name("entrypoint"), json("1"));
  }

  #[access(read="*", write="*")]
  kotoage fn alternate() permission(Admin) {
    set_account_detail(authority(), name("entrypoint"), json("2"));
  }
}
"#,
            )
            .expect("compile trigger dispatch contract");
        let bytecode = IvmBytecode::from_compiled(program);

        let trigger_id: iroha_data_model::trigger::TriggerId = "contract_dispatch".parse().unwrap();
        let mut trigger_metadata = Metadata::default();
        trigger_metadata.insert(
            Name::from_str("contract_entrypoint").expect("static metadata key"),
            Json::new("alternate"),
        );
        let trigger = Trigger::new(
            trigger_id.clone(),
            Action::new(
                Executable::Ivm(bytecode),
                Repeats::Exactly(1),
                multisig_id.clone(),
                ExecuteTriggerEventFilter::new().for_trigger(trigger_id.clone()),
            )
            .with_metadata(trigger_metadata),
        );

        let instructions = vec![
            InstructionBox::from(Register::trigger(trigger)),
            InstructionBox::from(ExecuteTrigger::new(trigger_id.clone())),
        ];
        let instructions_hash = HashOf::new(&instructions);
        execute_propose(
            &mut state_transaction,
            &signer1_id,
            &MultisigPropose::new(multisig_id.clone(), instructions, None),
        )
        .expect("signatory propose");

        let proposal = proposal_value(&state_transaction, &multisig_id, &instructions_hash)
            .expect("proposal exists after propose");
        let register = proposal
            .instructions
            .first()
            .expect("proposal should register trigger")
            .as_any()
            .downcast_ref::<iroha_data_model::isi::RegisterBox>()
            .expect("first instruction must be register");
        let iroha_data_model::isi::RegisterBox::Trigger(register_trigger) = register else {
            panic!("first instruction must be register trigger");
        };
        let stored_entrypoint = register_trigger
            .object()
            .action()
            .metadata()
            .get("contract_entrypoint")
            .expect("stored trigger metadata should keep contract_entrypoint")
            .clone()
            .try_into_any_norito::<String>()
            .expect("entrypoint metadata should decode as string");
        assert_eq!(stored_entrypoint, "alternate");

        execute_approve(
            &mut state_transaction,
            &signer2_id,
            &MultisigApprove::new(multisig_id.clone(), instructions_hash),
        )
        .expect("signatory approve should execute alternate entrypoint");

        let entrypoint_key: Name = "entrypoint".parse().expect("entrypoint metadata key");
        let executed_value = state_transaction
            .world
            .account(&multisig_id)
            .expect("multisig account should exist")
            .metadata()
            .get(&entrypoint_key)
            .expect("alternate entrypoint should write account metadata")
            .clone()
            .try_into_any_norito::<norito::json::Value>()
            .expect("entrypoint account metadata should decode");
        assert_eq!(executed_value, norito::json!(2));
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
        let signer1_id = new_account_id(&signer1);
        let signer2_id = new_account_id(&signer2);

        Register::domain(Domain::new(domain_id.clone()))
            .execute(&signer1_id, &mut state_transaction)
            .expect("domain registration");
        register_account_in_domain(
            &mut state_transaction,
            &signer1_id,
            &domain_id,
            &signer1_id,
            "register signer1",
        );
        register_account_in_domain(
            &mut state_transaction,
            &signer1_id,
            &domain_id,
            &signer2_id,
            "register signer2",
        );

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
    fn multisig_propose_replaces_expired_duplicate() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(
            World::new(),
            kura,
            query_handle,
            ChainId::from("multisig-expired-duplicate-replace"),
        );

        let domain_id: iroha_data_model::domain::DomainId = "retryable".parse().unwrap();
        let signer1 = KeyPair::random();
        let signer2 = KeyPair::random();
        let signer1_id = new_account_id(&signer1);
        let signer2_id = new_account_id(&signer2);
        let spec = MultisigSpec {
            signatories: BTreeMap::from([(signer1_id.clone(), 1), (signer2_id.clone(), 1)]),
            quorum: NonZeroU16::new(2).unwrap(),
            transaction_ttl_ms: NonZeroU64::new(1).unwrap(),
        };

        let multisig_id = {
            let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 1, 0);
            let mut block = state.block(block_header);
            let mut state_transaction = block.transaction();

            Register::domain(Domain::new(domain_id.clone()))
                .execute(&signer1_id, &mut state_transaction)
                .expect("domain registration");
            register_account_in_domain(
                &mut state_transaction,
                &signer1_id,
                &domain_id,
                &signer1_id,
                "register signer1",
            );
            register_account_in_domain(
                &mut state_transaction,
                &signer1_id,
                &domain_id,
                &signer2_id,
                "register signer2",
            );

            let multisig_id = register_multisig_account(
                &mut state_transaction,
                &signer1_id,
                &domain_id,
                &spec,
                "register multisig account",
            );
            let instructions = Vec::<InstructionBox>::new();
            execute_propose(
                &mut state_transaction,
                &signer1_id,
                &MultisigPropose::new(multisig_id.clone(), instructions, None),
            )
            .expect("initial propose");

            drop(state_transaction);
            block.commit().expect("commit first block");
            multisig_id
        };

        let block_header = BlockHeader::new(nonzero!(2_u64), None, None, None, 3, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();

        let instructions = Vec::<InstructionBox>::new();
        let instructions_hash = HashOf::new(&instructions);
        execute_propose(
            &mut state_transaction,
            &signer2_id,
            &MultisigPropose::new(multisig_id.clone(), instructions, None),
        )
        .expect("expired duplicate should be replaced");

        let proposal = proposal_value(&state_transaction, &multisig_id, &instructions_hash)
            .expect("replacement proposal");
        assert_eq!(proposal.proposed_at_ms, 3);
        assert_eq!(proposal.expires_at_ms, 4);
        assert_eq!(
            proposal.approvals,
            BTreeSet::from([signer2_id]),
            "replacement proposal should record only the new proposer approval"
        );
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

        let owner_id = new_account_id(&owner);
        let signer1_remote = new_account_id(&signer1);
        let signer2_remote = new_account_id(&signer2);

        Register::domain(Domain::new(multisig_domain.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("register multisig domain");
        Register::domain(Domain::new(signer_domain.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("register signer domain");

        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &multisig_domain,
            &owner_id,
            "register owner",
        );
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &signer_domain,
            &signer1_remote,
            "register signer1 remote",
        );
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &signer_domain,
            &signer2_remote,
            "register signer2 remote",
        );
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
        let multisig_seed = new_account_id(&KeyPair::random());
        let register = MultisigRegister::with_account(
            multisig_seed.clone(),
            multisig_domain.clone(),
            spec.clone(),
        );

        execute_register(&mut state_transaction, &owner_id, register)
            .expect("register multisig from cross-domain signatories");

        let registered_multisig_id = state_transaction
            .world
            .accounts_iter()
            .find(|account| account.id().multisig_policy().is_some())
            .map(|account| account.id().clone())
            .expect("registered multisig account");

        let stored_spec =
            multisig_spec(&state_transaction, &registered_multisig_id).expect("stored spec");
        assert!(
            stored_spec
                .signatories
                .keys()
                .any(|account| account.subject_id() == signer1_remote.subject_id()),
            "remote signatory subject must be preserved in multisig spec"
        );
        assert!(
            stored_spec
                .signatories
                .keys()
                .any(|account| account.subject_id() == signer2_remote.subject_id()),
            "remote signatory subject must be preserved in multisig spec"
        );
        assert_eq!(
            multisig_home_domain(&state_transaction, &registered_multisig_id)
                .expect("multisig home domain"),
            Some(multisig_domain),
            "registered multisig must retain the explicit home domain"
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
    fn multisig_approval_counts_subject_once_with_multiple_domain_links() {
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

        let owner_id = new_account_id(&owner);
        let shared_account = new_account_id(&shared_subject);
        let signer_b_id = new_account_id(&signer_b);
        let signer_c_id = new_account_id(&signer_c);

        Register::domain(Domain::new(home_domain.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("register home domain");
        Register::domain(Domain::new(alt_domain.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("register alt domain");

        for account in [
            owner_id.clone(),
            shared_account.clone(),
            signer_b_id.clone(),
            signer_c_id.clone(),
        ] {
            register_account_in_domain(
                &mut state_transaction,
                &owner_id,
                &home_domain,
                &account,
                "register account",
            );
        }
        assert_eq!(
            domain_owner(&state_transaction, &home_domain).expect("domain owner lookup"),
            owner_id,
            "home domain owner should follow registering authority",
        );

        let spec = MultisigSpec {
            signatories: BTreeMap::from([
                (shared_account.clone(), 1),
                (signer_b_id.clone(), 1),
                (signer_c_id.clone(), 1),
            ]),
            quorum: NonZeroU16::new(3).unwrap(),
            transaction_ttl_ms: NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).unwrap(),
        };
        let multisig_seed = new_account_id(&KeyPair::random());
        execute_register(
            &mut state_transaction,
            &owner_id,
            MultisigRegister::with_account(multisig_seed, home_domain.clone(), spec.clone()),
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
            &shared_account,
            &MultisigApprove::new(multisig_account.clone(), instructions_hash),
        )
        .expect("approve from subject home account");
        let approved_once =
            proposal_value(&state_transaction, &multisig_account, &instructions_hash)
                .expect("proposal exists after first subject approval");
        assert_eq!(
            approved_weight_by_subject(&loaded_spec, &approved_once.approvals),
            2,
            "subject approval should increase distinct subject weight"
        );
        execute_approve(
            &mut state_transaction,
            &shared_account,
            &MultisigApprove::new(multisig_account.clone(), instructions_hash),
        )
        .expect("approve from subject with additional domain link");
        let approved_twice =
            proposal_value(&state_transaction, &multisig_account, &instructions_hash)
                .expect("proposal should persist after duplicate-subject approval");
        assert_eq!(
            approved_weight_by_subject(&loaded_spec, &approved_twice.approvals),
            2,
            "same subject with multiple domain links must not satisfy quorum twice"
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

        let owner_id = new_account_id(&owner);
        let first_leaf_account_id = new_account_id(&leaf_a);
        let second_leaf_account_id = new_account_id(&leaf_b);

        Register::domain(Domain::new(domain_id.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("domain registration");
        for (account_id, label) in [
            (owner_id.clone(), "register owner"),
            (first_leaf_account_id.clone(), "register leaf a"),
            (second_leaf_account_id.clone(), "register leaf b"),
        ] {
            register_account_in_domain(
                &mut state_transaction,
                &owner_id,
                &domain_id,
                &account_id,
                label,
            );
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
        let parent_id = new_account_id(&parent_key);
        let register = MultisigRegister::with_account(parent_id, domain_id.clone(), parent_spec);
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
        let owner_id = new_account_id(&owner_key);
        Register::domain(Domain::new(domain_id.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("domain registration");
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &owner_id,
            "register owner",
        );

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
        let account = AccountId::new_multisig(policy);
        let canonical = account
            .canonical_i105()
            .expect("large multisig policy should encode into canonical I105");

        let role_id = multisig_role_for(Some(&domain_id), &account);
        let role_name = role_id.name().to_string();
        let expected_suffix = HashOf::new(&account).to_string();
        assert!(
            role_name.ends_with(&expected_suffix),
            "role name should use hash suffix for large multisig policy"
        );
        assert!(
            !role_name.ends_with(&canonical),
            "large multisig role ids should still fall back to the hash suffix when the canonical literal is too long"
        );
    }

    #[test]
    fn multisig_cancel_requires_quorum_and_prunes_target_proposal() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(
            World::new(),
            kura,
            query_handle,
            ChainId::from("multisig-cancel-prunes-target"),
        );
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();
        let domain_id: iroha_data_model::domain::DomainId = "cancel".parse().unwrap();

        let owner_key = KeyPair::random();
        let owner_id = new_account_id(&owner_key);
        Register::domain(Domain::new(domain_id.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("domain registration");
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &owner_id,
            "register owner",
        );

        let signer1_key = KeyPair::random();
        let signer1_id = new_account_id(&signer1_key);
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &signer1_id,
            "register signer1",
        );
        let signer2_key = KeyPair::random();
        let signer2_id = new_account_id(&signer2_key);
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &signer2_id,
            "register signer2",
        );

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

        let target_instructions: Vec<InstructionBox> = Vec::new();
        let target_hash = HashOf::new(&target_instructions);
        let target_proposal =
            MultisigPropose::new(multisig_id.clone(), target_instructions.clone(), None);
        Executor::Initial
            .execute_instruction(
                &mut state_transaction,
                &signer1_id,
                InstructionBox::from(target_proposal),
            )
            .expect("create target proposal");

        let cancel = MultisigCancel::new(multisig_id.clone(), target_hash);
        let direct_err = Executor::Initial
            .execute_instruction(
                &mut state_transaction,
                &signer1_id,
                InstructionBox::from(cancel.clone()),
            )
            .expect_err("direct cancel by signer must be rejected");
        match direct_err {
            ValidationFail::NotPermitted(message) => {
                assert!(
                    message.contains("must execute as the multisig account"),
                    "unexpected cancel rejection: {message}"
                );
            }
            other => panic!("unexpected direct cancel error: {other:?}"),
        }
        assert!(
            proposal_value(&state_transaction, &multisig_id, &target_hash).is_ok(),
            "target proposal should remain after rejected direct cancel"
        );

        let cancel_instructions = vec![InstructionBox::from(cancel)];
        let cancel_hash = HashOf::new(&cancel_instructions);
        let cancel_proposal = MultisigPropose::new(multisig_id.clone(), cancel_instructions, None);
        Executor::Initial
            .execute_instruction(
                &mut state_transaction,
                &signer1_id,
                InstructionBox::from(cancel_proposal),
            )
            .expect("create cancel proposal");
        Executor::Initial
            .execute_instruction(
                &mut state_transaction,
                &signer2_id,
                InstructionBox::from(MultisigApprove::new(multisig_id.clone(), cancel_hash)),
            )
            .expect("approve cancel proposal");

        assert!(
            proposal_value(&state_transaction, &multisig_id, &target_hash).is_err(),
            "target proposal should be pruned once cancel reaches quorum"
        );
        assert!(
            proposal_value(&state_transaction, &multisig_id, &cancel_hash).is_err(),
            "cancel proposal should also be pruned after execution"
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
        let owner_id = new_account_id(&owner_key);
        Register::domain(Domain::new(domain_id.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("domain registration");
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &owner_id,
            "register owner",
        );

        let weight = u8::MAX;
        let signatory_count = (u16::MAX as usize / weight as usize) + 1;
        let mut signatories = BTreeMap::new();
        for _ in 0..signatory_count {
            let signer_key = KeyPair::random();
            let signer_id = new_account_id(&signer_key);
            register_account_in_domain(
                &mut state_transaction,
                &owner_id,
                &domain_id,
                &signer_id,
                "register signatory",
            );
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
        store_multisig_proposal_state(
            &mut state_transaction,
            &MultisigProposalState::new(
                multisig_id.clone(),
                instructions_hash,
                seeded_value.instructions,
                seeded_value.proposed_at_ms,
                seeded_value.expires_at_ms,
                seeded_value.approvals,
                seeded_value.is_relayed,
            ),
        )
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

    #[test]
    fn replace_account_controller_single_to_multisig_materializes_members_and_preserves_alias() {
        let domain_id: iroha_data_model::domain::DomainId = "replace".parse().unwrap();
        let owner_key = KeyPair::random();
        let owner_id = new_account_id(&owner_key);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let mut world = World::new();
        seed_domain_name_lease(&mut world, &owner_id, &domain_id);
        let state = State::new_with_chain(
            world,
            kura,
            query_handle,
            ChainId::from("replace-single-to-multisig"),
        );
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();

        Register::domain(Domain::new(domain_id.clone()))
            .execute(&owner_id, &mut state_transaction)
            .expect("domain registration");
        register_account_in_domain(
            &mut state_transaction,
            &owner_id,
            &domain_id,
            &owner_id,
            "register single-key account",
        );
        let alias = bind_account_label(
            &mut state_transaction,
            &owner_id,
            &owner_id,
            &domain_id,
            "treasury",
        );

        let member1 = KeyPair::random();
        let member2 = KeyPair::random();
        let policy = multisig_policy_for_members(&[(&member1, 1), (&member2, 1)]);

        let updated_account = replace_account_controller(
            &owner_id,
            &mut state_transaction,
            &owner_id,
            AccountController::multisig(policy),
        )
        .expect("replace single-key controller with multisig");

        assert!(
            multisig_spec(&state_transaction, &updated_account).is_ok(),
            "multisig replacement should persist native multisig state"
        );
        assert!(
            state_transaction
                .world
                .account(&AccountId::new(member1.public_key().clone()))
                .is_ok(),
            "first signatory account should be materialized"
        );
        assert!(
            state_transaction
                .world
                .account(&AccountId::new(member2.public_key().clone()))
                .is_ok(),
            "second signatory account should be materialized"
        );
        assert_eq!(
            state_transaction.world.account_aliases.get(&alias),
            Some(&updated_account)
        );
        assert_eq!(
            state_transaction
                .world
                .account_rekey_records
                .get(&alias)
                .expect("rekey record should remain")
                .active_account_id,
            updated_account
        );
    }

    #[test]
    fn replace_account_controller_multisig_to_single_clears_memberships() {
        let domain_id: iroha_data_model::domain::DomainId = "single".parse().unwrap();
        let signer1 = KeyPair::random();
        let signer2 = KeyPair::random();
        let signer1_id = new_account_id(&signer1);
        let signer2_id = new_account_id(&signer2);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let mut world = World::new();
        seed_domain_name_lease(&mut world, &signer1_id, &domain_id);
        let state = State::new_with_chain(
            world,
            kura,
            query_handle,
            ChainId::from("replace-multisig-to-single"),
        );
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();

        Register::domain(Domain::new(domain_id.clone()))
            .execute(&signer1_id, &mut state_transaction)
            .expect("domain registration");
        for (account_id, label) in [
            (&signer1_id, "register signer1"),
            (&signer2_id, "register signer2"),
        ] {
            register_account_in_domain(
                &mut state_transaction,
                &signer1_id,
                &domain_id,
                account_id,
                label,
            );
        }

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
        let alias = bind_account_label(
            &mut state_transaction,
            &signer1_id,
            &multisig_id,
            &domain_id,
            "payments",
        );

        let replacement_key = KeyPair::random();
        let replacement_account = AccountId::new(replacement_key.public_key().clone());
        let updated_account = replace_account_controller(
            &signer1_id,
            &mut state_transaction,
            &multisig_id,
            AccountController::single(replacement_key.public_key().clone()),
        )
        .expect("replace multisig controller with single-key");

        assert_eq!(updated_account, replacement_account);
        assert_eq!(
            load_signatory_memberships(&state_transaction, &signer1_id),
            BTreeSet::new()
        );
        assert_eq!(
            load_signatory_memberships(&state_transaction, &signer2_id),
            BTreeSet::new()
        );
        assert!(
            state_transaction
                .world
                .smart_contract_state
                .get(&multisig_account_state_key(&updated_account))
                .is_none(),
            "single-key replacement should clear native multisig state"
        );
        assert_eq!(
            state_transaction.world.account_aliases.get(&alias),
            Some(&updated_account)
        );
    }

    #[test]
    fn replace_account_controller_multisig_to_multisig_repoints_memberships() {
        let domain_id: iroha_data_model::domain::DomainId = "repoint".parse().unwrap();
        let signer1 = KeyPair::random();
        let signer2 = KeyPair::random();
        let signer3 = KeyPair::random();
        let signer1_id = new_account_id(&signer1);
        let signer2_id = new_account_id(&signer2);
        let signer3_id = new_account_id(&signer3);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let mut world = World::new();
        seed_domain_name_lease(&mut world, &signer1_id, &domain_id);
        let state = State::new_with_chain(
            world,
            kura,
            query_handle,
            ChainId::from("replace-multisig-to-multisig"),
        );
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_transaction = block.transaction();

        Register::domain(Domain::new(domain_id.clone()))
            .execute(&signer1_id, &mut state_transaction)
            .expect("domain registration");
        for (account_id, label) in [
            (&signer1_id, "register signer1"),
            (&signer2_id, "register signer2"),
            (&signer3_id, "register signer3"),
        ] {
            register_account_in_domain(
                &mut state_transaction,
                &signer1_id,
                &domain_id,
                account_id,
                label,
            );
        }

        let initial_spec = MultisigSpec {
            signatories: BTreeMap::from([(signer1_id.clone(), 1), (signer2_id.clone(), 1)]),
            quorum: NonZeroU16::new(2).unwrap(),
            transaction_ttl_ms: NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).unwrap(),
        };
        let multisig_id = register_multisig_account(
            &mut state_transaction,
            &signer1_id,
            &domain_id,
            &initial_spec,
            "register multisig account",
        );

        let replacement_policy = multisig_policy_for_members(&[(&signer2, 1), (&signer3, 1)]);
        let updated_account = replace_account_controller(
            &signer1_id,
            &mut state_transaction,
            &multisig_id,
            AccountController::multisig(replacement_policy),
        )
        .expect("replace multisig controller with new multisig policy");

        assert_eq!(
            load_signatory_memberships(&state_transaction, &signer1_id),
            BTreeSet::new()
        );
        assert_eq!(
            load_signatory_memberships(&state_transaction, &signer2_id),
            BTreeSet::from([updated_account.clone()])
        );
        assert_eq!(
            load_signatory_memberships(&state_transaction, &signer3_id),
            BTreeSet::from([updated_account.clone()])
        );

        let updated_spec = multisig_spec(&state_transaction, &updated_account)
            .expect("updated multisig spec should be available");
        assert_eq!(
            updated_spec
                .signatories
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([signer2_id, signer3_id])
        );
    }
}
